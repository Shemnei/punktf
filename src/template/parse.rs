#[cfg(test)]
mod tests;

use color_eyre::eyre::{eyre, Result};
use color_eyre::Report;

use super::block::{Block, BlockHint, If, IfExpr, IfOp, Var, VarEnv, VarEnvSet};
use super::diagnostic::{Diagnositic, DiagnositicBuilder, DiagnositicLevel};
use super::session::Session;
use super::source::Source;
use super::span::{ByteSpan, Pos, Spanned};
use super::Template;
use crate::template::block::BlockKind;

#[derive(Debug, Clone)]
pub struct Parser<'a> {
	source: Source<'a>,
	session: Session,
	blocks: BlockIter<'a>,
}

impl<'a> Parser<'a> {
	pub const fn new(source: Source<'a>) -> Self {
		let blocks = BlockIter::new(source.content);

		Self {
			source,
			session: Session::new(),
			blocks,
		}
	}

	// TODO: return error struct instead of emitting here
	pub fn parse(mut self) -> Result<Template<'a>> {
		let mut blocks = Vec::new();

		while let Some(res) = self.next_top_level_block() {
			match res {
				Ok(block) => blocks.push(block),
				Err(builder) => self.report_diagnostic(builder.build()),
			};
		}

		self.session.emit(&self.source);
		let _ = self.session.try_finish()?;

		Ok(Template {
			source: self.source,
			blocks,
		})
	}

	fn report_diagnostic(&mut self, diagnostic: Diagnositic) {
		if diagnostic.level() == &DiagnositicLevel::Error {
			self.session.mark_failed();
		}

		self.session.report(diagnostic);
	}

	fn next_top_level_block(&mut self) -> Option<Result<Block, DiagnositicBuilder>> {
		let Spanned { span, value: hint } = match self.blocks.next()? {
			Ok(x) => x,
			Err(err) => return Some(Err(err)),
		};

		log::trace!("{:?}: {}", hint, &self.source[span]);

		let block = match hint {
			BlockHint::Text => Ok(self.parse_text(span)),
			BlockHint::Comment => Ok(self.parse_comment(span)),
			BlockHint::Escaped => Ok(self.parse_escaped(span)),
			BlockHint::Var => self
				.parse_variable(span)
				.map(|var| Block::new(span, BlockKind::Var(var))),
			BlockHint::Print => Ok(self.parse_print(span)),
			BlockHint::IfStart => self
				.parse_if(span)
				.map(|Spanned { span, value }| Block::new(span, BlockKind::If(value))),

			// Illegal top level blocks
			BlockHint::ElIf => Err(DiagnositicBuilder::new(DiagnositicLevel::Error)
				.message("top-level `elif` block")
				.description("an `elif` block must always come after an `if` block")
				.primary_span(span)),
			BlockHint::Else => Err(DiagnositicBuilder::new(DiagnositicLevel::Error)
				.message("top-level `else` block")
				.description("an `else` block must always come after an `if` or `elfi` block")
				.primary_span(span)),
			BlockHint::IfEnd => Err(DiagnositicBuilder::new(DiagnositicLevel::Error)
				.message("top-level `fi` block")
				.description("an `fi` can only be used to close an open `if` block")
				.primary_span(span)),
		};

		Some(block)
	}

	const fn parse_text(&self, span: ByteSpan) -> Block {
		Block::new(span, BlockKind::Text)
	}

	const fn parse_comment(&self, span: ByteSpan) -> Block {
		// {{!-- ... --}}
		Block::new(span, BlockKind::Comment)
	}

	fn parse_escaped(&self, span: ByteSpan) -> Block {
		// {{{ ... }}}
		Block::new(span, BlockKind::Escaped(span.offset_low(3).offset_high(-3)))
	}

	fn parse_variable(&self, span: ByteSpan) -> Result<Var, DiagnositicBuilder> {
		let span_inner = span.offset_low(2).offset_high(-2);
		let content_inner = &self.source[span_inner];

		// +2 for block opening
		let offset = span.low().as_usize() + 2;

		parse_var(content_inner, offset).map_err(|err| {
			DiagnositicBuilder::new(DiagnositicLevel::Error)
				.message("failed to parse variable block")
				.description(err.to_string())
				.primary_span(span)
		})
	}

	fn parse_print(&self, span: ByteSpan) -> Block {
		// {{@print ... }}
		Block::new(span, BlockKind::Print(span.offset_low(9).offset_high(-2)))
	}

	fn parse_if(&mut self, span: ByteSpan) -> Result<Spanned<If>, DiagnositicBuilder> {
		let head = span.span(
			self.parse_if_start(span)
				.map_err(|build| build.label_span(span, "while parsing this `if` block"))?,
		);

		// collect all nested blocks
		let head_nested = self
			.parse_if_enclosed_blocks()
			.into_iter()
			.filter_map(|res| match res {
				Ok(block) => Some(block),
				Err(builder) => {
					self.report_diagnostic(
						builder
							.label_span(*head.span(), "while parsing this `if` block")
							.build(),
					);
					None
				}
			})
			.collect();

		let Spanned {
			mut span,
			value: mut hint,
		} = self
			.blocks
			.next()
			.ok_or_else(|| {
				DiagnositicBuilder::new(DiagnositicLevel::Error)
					.message("unexpected end of `if` block")
					.description("close the `if` block with `{{@fi}}`")
					.primary_span(span)
					.label_span(*head.span(), "While parsing this `if` block")
			})?
			.map_err(|build| build.label_span(*head.span(), "while parsing this `if` block"))?;

		// check for elif
		let mut elifs = Vec::new();

		while hint == BlockHint::ElIf {
			let elif = span.span(self.parse_elif(span).map_err(|build| {
				build.label_span(*head.span(), "while parsing this `if` block")
			})?);

			let elif_nested = self
				.parse_if_enclosed_blocks()
				.into_iter()
				.filter_map(|res| match res {
					Ok(block) => Some(block),
					Err(builder) => {
						self.report_diagnostic(
							builder
								.label_span(span, "while parsing this `elif` block")
								.build(),
						);
						None
					}
				})
				.collect();

			elifs.push((elif, elif_nested));

			let Spanned {
				span: _span,
				value: _hint,
			} = self
				.blocks
				.next()
				.ok_or_else(|| {
					DiagnositicBuilder::new(DiagnositicLevel::Error)
						.message("unexpected end of `elif` block")
						.description("close the `if` block with `{{@fi}}`")
						.primary_span(span)
						.label_span(*head.span(), "While parsing this `if` block")
				})?
				.map_err(|build| build.label_span(*head.span(), "while parsing this `if` block"))?;

			span = _span;
			hint = _hint;
		}

		let els = if hint == BlockHint::Else {
			let els = self
				.parse_else(span)
				.map_err(|build| build.label_span(*head.span(), "while parsing this `if` block"))?;
			let els_nested = self
				.parse_if_enclosed_blocks()
				.into_iter()
				.filter_map(|res| match res {
					Ok(block) => Some(block),
					Err(builder) => {
						self.report_diagnostic(
							builder
								.label_span(span, "while parsing this `else` block")
								.build(),
						);
						None
					}
				})
				.collect();

			let Spanned {
				span: _span,
				value: _hint,
			} = self
				.blocks
				.next()
				.ok_or_else(|| {
					DiagnositicBuilder::new(DiagnositicLevel::Error)
						.message("unexpected end of `else` block")
						.description("close the `if` block with `{{@fi}}`")
						.primary_span(span)
						.label_span(*head.span(), "While parsing this `if` block")
				})?
				.map_err(|build| build.label_span(*head.span(), "while parsing this `if` block"))?;

			span = _span;
			hint = _hint;

			Some((els, els_nested))
		} else {
			None
		};

		let end = if hint == BlockHint::IfEnd {
			self.parse_if_end(span)
				.map_err(|build| build.label_span(*head.span(), "while parsing this `if` block"))?
		} else {
			return Err(DiagnositicBuilder::new(DiagnositicLevel::Error)
				.message("unexpected end of `if` block")
				.description("close the `if` block with `{{@fi}}`")
				.primary_span(span)
				.label_span(*head.span(), "While parsing this `if` block"));
		};

		let whole_if = head.span.union(&end);

		Ok(whole_if.span(If {
			head: (head, head_nested),
			elifs,
			els,
			end,
		}))
	}

	fn parse_if_start(&self, span: ByteSpan) -> Result<IfExpr, DiagnositicBuilder> {
		// {{@if {{VAR}} (!=|==) "LIT" }}
		let expr_span = span.offset_low(6).offset_high(-2);
		self.parse_if_expr(expr_span)
	}

	fn parse_elif(&self, span: ByteSpan) -> Result<IfExpr, DiagnositicBuilder> {
		// {{@elif {{VAR}} (!=|==) "LIT" }}
		let expr_span = span.offset_low(8).offset_high(-2);
		self.parse_if_expr(expr_span)
	}

	fn parse_else(&self, span: ByteSpan) -> Result<ByteSpan, DiagnositicBuilder> {
		if &self.source[span] != "{{@else}}" {
			Err(DiagnositicBuilder::new(DiagnositicLevel::Error)
				.message("expected a `else` block")
				.primary_span(span))
		} else {
			Ok(span)
		}
	}

	fn parse_if_end(&self, span: ByteSpan) -> Result<ByteSpan, DiagnositicBuilder> {
		if &self.source[span] != "{{@fi}}" {
			Err(DiagnositicBuilder::new(DiagnositicLevel::Error)
				.message("expected a `fi` block")
				.primary_span(span))
		} else {
			Ok(span)
		}
	}

	fn parse_if_expr(&self, span: ByteSpan) -> Result<IfExpr, DiagnositicBuilder> {
		// {{VAR}} (!=|==) "OTHER" OR {{VAR}}
		let content = &self.source[span];

		// read var
		let var_block_start = content.find("{{").ok_or_else(|| {
			DiagnositicBuilder::new(DiagnositicLevel::Error)
				.message("expected a variable block")
				.description("add a variable block with `{{VARIABLE_NAME}}`")
				.primary_span(span)
		})?;

		let var_block_end = content.find("}}").ok_or_else(|| {
			DiagnositicBuilder::new(DiagnositicLevel::Error)
				.message("variable block not closed")
				.description("add `}}` to the close the open variable block")
				.primary_span(ByteSpan::new(var_block_start, var_block_start + 2))
		})? + 2;

		let var_block_span = ByteSpan::new(
			span.low().as_usize() + var_block_start,
			span.low().as_usize() + var_block_end,
		);

		let var = self.parse_variable(var_block_span)?;

		// check if it is an exits expr
		// exclude the closing `}}` with -2.
		let remainder = &content[var_block_end..];

		if remainder.trim().is_empty() {
			Ok(IfExpr::Exists { var })
		} else {
			let op = parse_ifop(&content[var_block_end..]).map_err(|_| {
				DiagnositicBuilder::new(DiagnositicLevel::Error)
					.message("failed to find if operation")
					.description("add either `==` or `!=` after the variable block")
					.primary_span(var_block_span)
			})?;

			let other = parse_other(
				&content[var_block_end..],
				span.low().as_usize() + var_block_end,
			)
			.map_err(|_| {
				DiagnositicBuilder::new(DiagnositicLevel::Error)
					.message("failed to find right hand side of the if operation")
					.description("add a literal to compare againt with `\"LITERAL\"`")
					.primary_span(var_block_span)
			})?;

			Ok(IfExpr::Compare { var, op, other })
		}
	}

	fn parse_if_enclosed_blocks(&mut self) -> Vec<Result<Block, DiagnositicBuilder>> {
		let mut enclosed_blocks = Vec::new();

		while self
			.peek_block_hint()
			.map(|hint| !hint.is_if_subblock())
			.unwrap_or(true)
		{
			let next_block = self
				.next_top_level_block()
				.expect("Some block to be present after peek");

			enclosed_blocks.push(next_block);
		}

		enclosed_blocks
	}

	fn peek_block_hint(&self) -> Option<BlockHint> {
		// Create a copy of the block iter to not mess up the state while peeking
		let mut peek = self.blocks;
		peek.next()?.ok().map(|spanned| spanned.into_value())
	}
}

type NextBlock = (ByteSpan, Option<BlockHint>);
type NextBlockError = (Option<usize>, Report);

fn next_block(s: &str) -> Option<Result<NextBlock, NextBlockError>> {
	if s.is_empty() {
		return None;
	}

	if let Some(low) = s.find("{{") {
		if low > 0 {
			// found text block
			Some(Ok((ByteSpan::new(0usize, low), Some(BlockHint::Text))))
		} else if let Some(b'{') = s.as_bytes().get(low + 2) {
			// block is an escaped block
			if let Some(high) = s.find("}}}") {
				Some(Ok((ByteSpan::new(low, high + 3), Some(BlockHint::Escaped))))
			} else {
				Some(Err((
					Some(3),
					eyre!("Found opening for an escaped block but no closing"),
				)))
			}
		} else if let Some(b"!--") = s.as_bytes().get(low + 2..low + 5) {
			// block is an comment block
			if let Some(high) = s.find("--}}") {
				Some(Ok((ByteSpan::new(low, high + 4), Some(BlockHint::Comment))))
			} else {
				Some(Err((
					Some(5),
					eyre!("Found opening for a comment block but no closing"),
				)))
			}
		} else {
			// check depth
			let mut openings = s[low + 1..].match_indices("{{").map(|(idx, _)| idx);
			let closings = s[low + 1..].match_indices("}}").map(|(idx, _)| idx);

			for high in closings {
				// check the is a opening.
				if let Some(opening) = openings.next() {
					// check if opening comes before the closing.
					if opening < high {
						// opening lies before the closing. Continue to search
						// for the matching closing of low.
						continue;
					}
				}

				let high = high + 2 + (low + 1);
				return Some(Ok((ByteSpan::new(low, high), None)));
			}

			Some(Err((
				Some(2),
				eyre!("Found opening for a block but no closing"),
			)))
		}
	} else {
		// Found text block
		Some(Ok((ByteSpan::new(0usize, s.len()), Some(BlockHint::Text))))
	}
}

// inner should be without the `{{` and `}}`. Offset should include the starting `{{`.
fn parse_var(inner: &str, mut offset: usize) -> Result<Var> {
	// save original length to keep track of the offset
	let orig_len = inner.len();

	// remove preceding whitespaces
	let inner = inner.trim_start();

	// increase offset to account for removed whitespaces
	offset += orig_len - inner.len();

	// remove trailing whitespaces. Offset doesn't need to change.
	let mut inner = inner.trim_end();

	// check for envs
	let envs = if matches!(
		inner.as_bytes().get(0),
		Some(b'$') | Some(b'#') | Some(b'&')
	) {
		let mut env_set = VarEnvSet::empty();

		// try to read all available envs
		for idx in 0..env_set.capacity() {
			let env = match inner.as_bytes().get(idx) {
				Some(b'$') => VarEnv::Environment,
				Some(b'#') => VarEnv::Profile,
				Some(b'&') => VarEnv::Dotfile,
				_ => break,
			};

			// break if add fails (duplicate, no more space)
			if !env_set.add(env) {
				return Err(eyre!(
					"Specified duplicate variable environments at {}",
					offset
				));
			}
		}

		// adjust offset
		offset += env_set.len();
		inner = &inner[env_set.len()..];

		env_set
	} else {
		VarEnvSet::default()
	};

	// check var name
	//	- len > 0
	//	- only ascii + _
	if inner.is_empty() {
		Err(eyre!("Empty variable name at {}", offset))
	} else if let Some(invalid) = inner.as_bytes().iter().find(|&&b| !is_var_name_symbol(b)) {
		Err(eyre!(
			"Found invalid symbol in variable name: (b`{}`; c`{}`)",
			invalid,
			if invalid.is_ascii() {
				*invalid as char
			} else {
				'\0'
			}
		))
	} else {
		Ok(Var {
			envs,
			name: ByteSpan::new(offset, offset + inner.len()),
		})
	}
}

fn parse_ifop(inner: &str) -> Result<IfOp> {
	match (inner.find("=="), inner.find("!=")) {
		(Some(eq_idx), Some(noteq_idx)) => {
			if eq_idx < noteq_idx {
				Ok(IfOp::Eq)
			} else {
				Ok(IfOp::NotEq)
			}
		}
		(Some(_), None) => Ok(IfOp::Eq),
		(None, Some(_)) => Ok(IfOp::NotEq),
		_ => Err(eyre!("Failed to find any if operation")),
	}
}

// parses the right hand side of an if/elif. The `"` characters are not included.
// e.g. "windows"
fn parse_other(inner: &str, offset: usize) -> Result<ByteSpan> {
	let mut matches = inner.match_indices('"').map(|(idx, _)| idx);

	match (matches.next(), matches.next()) {
		(Some(low), Some(high)) => Ok(ByteSpan::new(offset + low + 1, offset + high)),
		(Some(low), None) => Err(eyre!(
			"Found opening `\"` at {} but no closing",
			offset + low
		)),
		_ => Err(eyre!("Found no other")),
	}
}

fn is_var_name_symbol(b: u8) -> bool {
	(b'a'..=b'z').contains(&b)
		|| (b'A'..=b'Z').contains(&b)
		|| (b'0'..=b'9').contains(&b)
		|| b == b'_'
}

#[derive(Debug, Clone, Copy)]
struct BlockIter<'a> {
	content: &'a str,
	index: usize,
}

impl<'a> BlockIter<'a> {
	const fn new(content: &'a str) -> Self {
		Self { content, index: 0 }
	}
}

impl<'a> Iterator for BlockIter<'a> {
	type Item = Result<Spanned<BlockHint>, DiagnositicBuilder>;

	fn next(&mut self) -> Option<Self::Item> {
		let (mut span, hint) = match next_block(&self.content[self.index..])? {
			Ok(x) => x,
			Err((skip, err)) => {
				// skip erroneous part to allow recovery and avoid infinite loops
				let span = ByteSpan::new(self.index, self.index);
				if let Some(skip) = skip {
					self.index += skip;
					log::debug!("Skipping: {} ({})", skip, &self.content[self.index..]);
				} else {
					self.index = self.content.len();
				}
				let span = span.with_high(self.index);

				log::debug!("Span: {}/{}", span, err);

				return Some(Err(DiagnositicBuilder::new(DiagnositicLevel::Error)
					.message("failed to parse block")
					.description(err.to_string())
					.primary_span(span)));
			}
		};

		span = span.offset(self.index as i32);
		self.index = span.high().as_usize();

		if let Some(hint) = hint {
			return Some(Ok(span.span(hint)));
		}

		let content = &self.content[span];

		// Check if its a text block (no opening and closing `{{\}}`)
		if !content.starts_with("{{") {
			return Some(Ok(span.span(BlockHint::Text)));
		}

		// Content without block opening and closing
		let content = &content[2..content.len() - 2];

		// Check for escaped
		// e.g. `{{{ Escaped }}}`
		if content.starts_with('{') && content.ends_with('}') {
			return Some(Ok(span.span(BlockHint::Escaped)));
		}

		// Check for comment
		// e.g. `{{!-- Comment --}}`
		if content.starts_with("!--") && content.ends_with("--") {
			return Some(Ok(span.span(BlockHint::Comment)));
		}

		// Check for print
		// e.g. `{{@print ... }}`
		if content.starts_with("@print ") {
			return Some(Ok(span.span(BlockHint::Print)));
		}

		// Check for if
		// e.g. `{{@if {{VAR}} == "LITERAL"}}`
		if content.starts_with("@if ") {
			return Some(Ok(span.span(BlockHint::IfStart)));
		}

		// Check for elif
		// e.g. `{{@elif {{VAR}} == "LITERAL"}}`
		if content.starts_with("@elif ") {
			return Some(Ok(span.span(BlockHint::ElIf)));
		}

		// Check for else
		// e.g. `{{@else}}`
		if content.starts_with("@else") {
			return Some(Ok(span.span(BlockHint::Else)));
		}

		// Check for fi
		// e.g. `{{@fi}}`
		if content.starts_with("@fi") {
			return Some(Ok(span.span(BlockHint::IfEnd)));
		}

		Some(Ok(span.span(BlockHint::Var)))
	}
}
