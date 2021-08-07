use color_eyre::eyre::{eyre, Result};
use color_eyre::Report;

use super::block::{Block, BlockHint, If, IfExpr, IfOp, Var, VarEnv, VarEnvSet};
use super::diagnostic::{Diagnositic, DiagnositicBuilder, DiagnositicLevel};
use super::session::{ParseState, Session};
use super::span::{ByteSpan, Pos, Spanned};
use super::Template;
use crate::template::block::BlockKind;

// TODO:
// - give mutable source as param
// - record error on source
// - try to recover on next block opening/closing

#[derive(Debug, Clone)]
pub struct Parser<'a> {
	session: Session<'a, ParseState>,
	blocks: BlockIter<'a>,
}

impl<'a> Parser<'a> {
	pub fn new(session: Session<'a, ParseState>) -> Self {
		let blocks = BlockIter::new(session.source.content);
		Self { session, blocks }
	}

	pub fn parse(mut self) -> Result<Template<'a>> {
		let mut blocks = Vec::new();

		while let Some(res) = self.next_top_level_block() {
			match res {
				Ok(block) => blocks.push(block),
				Err(builder) => self.report_diagnostic(builder.build()),
			};
		}

		self.session.emit();
		let session = self.session.try_finish()?;

		Ok(Template { session, blocks })
	}

	fn report_diagnostic(&mut self, diagnostic: Diagnositic) {
		if diagnostic.level() == &DiagnositicLevel::Error {
			self.session.mark_failed();
		}

		self.session.report(diagnostic);
	}

	fn next_top_level_block(&mut self) -> Option<Result<Block, DiagnositicBuilder>> {
		// TODO-BM: cant handle these errors for now as information is missing
		let Spanned { span, value: hint } = match self.next_block()? {
			Ok(x) => x,
			Err(err) => return Some(Err(err)),
		};

		log::trace!("{:?}: {}", hint, &self.session.source[span]);

		let block = match hint {
			BlockHint::Text => Ok(self.parse_text(span)),
			BlockHint::Comment => Ok(self.parse_comment(span)),
			BlockHint::Escaped => Ok(self.parse_escaped(span)),
			BlockHint::Variable => self
				.parse_variable(span)
				.map(|var| Block::new(span, BlockKind::Var(var))),
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

	fn try_recover(&mut self) -> bool {
		todo!()
	}

	fn next_block(&mut self) -> Option<Result<Spanned<BlockHint>, DiagnositicBuilder>> {
		let (span, hint, content) = match self.blocks.next()? {
			Ok(x) => x,
			Err(err) => {
				return Some(Err(err));
			}
		};

		if let Some(hint) = hint {
			return Some(Ok(span.span(hint)));
		}

		// Check if its a text block (no opening and closing `{{\}}`)
		if !matches!(content.as_bytes(), &[b'{', b'{', .., b'}', b'}']) {
			return Some(Ok(span.span(BlockHint::Text)));
		}

		// Content without block opening and closing
		let content = &content[2..content.len() - 2];

		// Check for escaped
		if let (Some(b'{'), Some(b'}')) = (content.as_bytes().get(0), content.as_bytes().last()) {
			return Some(Ok(span.span(BlockHint::Escaped)));
		}

		// Check for comment
		if let (Some(b"!--"), Some(b"--")) = (
			content.as_bytes().get(..3),
			content
				.as_bytes()
				.get(content.as_bytes().len().saturating_sub(3)..),
		) {
			return Some(Ok(span.span(BlockHint::Comment)));
		}

		// Check for if
		if let Some(b"@if ") = content.as_bytes().get(..4) {
			return Some(Ok(span.span(BlockHint::IfStart)));
		}

		// Check for elif
		if let Some(b"@elif ") = content.as_bytes().get(..6) {
			return Some(Ok(span.span(BlockHint::ElIf)));
		}

		// Check for else
		if let Some(b"@else") = content.as_bytes().get(..5) {
			return Some(Ok(span.span(BlockHint::Else)));
		}

		// Check for else
		if let Some(b"@fi") = content.as_bytes().get(..3) {
			return Some(Ok(span.span(BlockHint::IfEnd)));
		}

		Some(Ok(span.span(BlockHint::Variable)))
	}

	fn parse_text(&self, span: ByteSpan) -> Block {
		Block::new(span, BlockKind::Text)
	}

	fn parse_comment(&self, span: ByteSpan) -> Block {
		// {{!-- ... --}}
		Block::new(span, BlockKind::Comment)
	}

	fn parse_escaped(&self, span: ByteSpan) -> Block {
		// {{{ ... }}}
		Block::new(span, BlockKind::Escaped(span.offset_low(3).offset_high(-3)))
	}

	fn parse_variable(&self, span: ByteSpan) -> Result<Var, DiagnositicBuilder> {
		let span_inner = span.offset_low(2).offset_high(-2);
		let content_inner = &self.session.source[span_inner];

		// +2 for block opening
		let offset = span.low().as_usize() + 2;

		parse_var(content_inner, offset).map_err(|err| {
			DiagnositicBuilder::new(DiagnositicLevel::Error)
				.message("failed to parse variable block")
				.description(err.to_string())
				.primary_span(span)
		})
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
			.next_block()
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
				.next_block()
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
				.next_block()
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
		if &self.session.source[span] != "{{@else}}" {
			Err(DiagnositicBuilder::new(DiagnositicLevel::Error)
				.message("expected a `else` block")
				.primary_span(span))
		} else {
			Ok(span)
		}
	}

	fn parse_if_end(&self, span: ByteSpan) -> Result<ByteSpan, DiagnositicBuilder> {
		if &self.session.source[span] != "{{@fi}}" {
			Err(DiagnositicBuilder::new(DiagnositicLevel::Error)
				.message("expected a `fi` block")
				.primary_span(span))
		} else {
			Ok(span)
		}
	}

	fn parse_if_expr(&self, span: ByteSpan) -> Result<IfExpr, DiagnositicBuilder> {
		// {{VAR}} (!=|==) "OTHER" OR {{VAR}}
		let content = &self.session.source[span];

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
		// TODO-BM: improve
		let mut peek = self.clone();
		peek.next_block()?.ok().map(|spanned| spanned.into_value())
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
				Some(b'&') => VarEnv::Item,
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
			// TODO: could be invalid for unicode
			*invalid as char
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
	fn new(content: &'a str) -> Self {
		Self { content, index: 0 }
	}
}

impl<'a> Iterator for BlockIter<'a> {
	type Item = Result<(ByteSpan, Option<BlockHint>, &'a str), DiagnositicBuilder>;

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

				log::debug!("SPAN: {}/{}", span, err);

				return Some(Err(DiagnositicBuilder::new(DiagnositicLevel::Error)
					.message("failed to parse block")
					.description(err.to_string())
					.primary_span(span)));
			}
		};

		span = span.offset(self.index as i32);
		self.index = span.high().as_usize();

		Some(Ok((span, hint, &self.content[span])))
	}
}

#[cfg(test)]
mod tests {
	use pretty_assertions::assert_eq;

	use super::*;
	use crate::template::source::Source;
	use crate::template::span::ByteSpan;

	#[test]
	fn parse_single_text() -> Result<()> {
		let content = r#"Hello World this is a text block"#;

		let source = Source::anonymous(content);
		let mut parser = Parser::new(Session::new(source));
		let block = parser
			.next_top_level_block()
			.expect("Found no block")
			.expect("Encountered a parse error");

		assert_eq!(
			block,
			Block::new(ByteSpan::new(0usize, content.len()), BlockKind::Text)
		);

		Ok(())
	}

	#[test]
	fn parse_single_comment() -> Result<()> {
		let content = r#"{{!-- Hello World this is a comment block --}}"#;

		let source = Source::anonymous(content);
		let mut parser = Parser::new(Session::new(source));
		let block = parser
			.next_top_level_block()
			.expect("Found no block")
			.expect("Encountered a parse error");

		assert_eq!(
			block,
			Block::new(ByteSpan::new(0usize, content.len()), BlockKind::Comment)
		);

		Ok(())
	}

	#[test]
	fn parse_single_escaped() -> Result<()> {
		let content = r#"{{{ Hello World this is a comment block }}}"#;

		let source = Source::anonymous(content);
		let mut parser = Parser::new(Session::new(source));
		let block = parser
			.next_top_level_block()
			.expect("Found no block")
			.expect("Encountered a parse error");

		assert_eq!(block.span(), &ByteSpan::new(0usize, content.len()));

		let inner = ByteSpan::new(3usize, content.len() - 3);
		assert_eq!(&content[inner], " Hello World this is a comment block ");
		assert_eq!(block.kind(), &BlockKind::Escaped(inner));

		Ok(())
	}

	#[test]
	fn parse_single_var_default() -> Result<()> {
		let content = r#"{{OS}}"#;

		let source = Source::anonymous(content);
		let mut parser = Parser::new(Session::new(source));
		let block = parser
			.next_top_level_block()
			.expect("Found no block")
			.expect("Encountered a parse error");

		assert_eq!(block.span(), &ByteSpan::new(0usize, content.len()));

		let name = ByteSpan::new(2usize, content.len() - 2);
		assert_eq!(&content[name], "OS");
		let envs = VarEnvSet([Some(VarEnv::Item), Some(VarEnv::Profile), None]);
		assert_eq!(block.kind(), &BlockKind::Var(Var { envs, name }));

		Ok(())
	}

	#[test]
	fn parse_single_var_env() -> Result<()> {
		let content = r#"{{$ENV}}"#;

		let source = Source::anonymous(content);
		let mut parser = Parser::new(Session::new(source));
		let block = parser
			.next_top_level_block()
			.expect("Found no block")
			.expect("Encountered a parse error");

		assert_eq!(block.span(), &ByteSpan::new(0usize, content.len()));

		let name = ByteSpan::new(3usize, content.len() - 2);
		assert_eq!(&content[name], "ENV");
		let envs = VarEnvSet([Some(VarEnv::Environment), None, None]);
		assert_eq!(block.kind(), &BlockKind::Var(Var { envs, name }));

		Ok(())
	}

	#[test]
	fn parse_single_var_profile() -> Result<()> {
		let content = r#"{{#PROFILE}}"#;

		let source = Source::anonymous(content);
		let mut parser = Parser::new(Session::new(source));
		let block = parser
			.next_top_level_block()
			.expect("Found no block")
			.expect("Encountered a parse error");

		assert_eq!(block.span(), &ByteSpan::new(0usize, content.len()));

		let name = ByteSpan::new(3usize, content.len() - 2);
		assert_eq!(&content[name], "PROFILE");
		let envs = VarEnvSet([Some(VarEnv::Profile), None, None]);
		assert_eq!(block.kind(), &BlockKind::Var(Var { envs, name }));

		Ok(())
	}

	#[test]
	fn parse_single_var_item() -> Result<()> {
		let content = r#"{{&ITEM}}"#;

		let source = Source::anonymous(content);
		let mut parser = Parser::new(Session::new(source));
		let block = parser
			.next_top_level_block()
			.expect("Found no block")
			.expect("Encountered a parse error");

		assert_eq!(block.span(), &ByteSpan::new(0usize, content.len()));

		let name = ByteSpan::new(3usize, content.len() - 2);
		assert_eq!(&content[name], "ITEM");
		let envs = VarEnvSet([Some(VarEnv::Item), None, None]);
		assert_eq!(block.kind(), &BlockKind::Var(Var { envs, name }));

		Ok(())
	}

	#[test]
	fn parse_single_var_mixed() -> Result<()> {
		let content = r#"{{$&#MIXED}}"#;

		let source = Source::anonymous(content);
		let mut parser = Parser::new(Session::new(source));
		let block = parser
			.next_top_level_block()
			.expect("Found no block")
			.expect("Encountered a parse error");

		assert_eq!(block.span(), &ByteSpan::new(0usize, content.len()));

		let name = ByteSpan::new(5usize, content.len() - 2);
		assert_eq!(&content[name], "MIXED");
		let envs = VarEnvSet([
			Some(VarEnv::Environment),
			Some(VarEnv::Item),
			Some(VarEnv::Profile),
		]);
		assert_eq!(block.kind(), &BlockKind::Var(Var { envs, name }));

		Ok(())
	}

	#[test]
	fn parse_single_vars() -> Result<()> {
		// duplicate variable environment
		let content = r#"{{##OS}}"#;

		let source = Source::anonymous(content);
		let mut parser = Parser::new(Session::new(source));
		let block = parser
			.next_top_level_block()
			.ok_or(eyre!("No block found"))?;

		assert!(block.is_err());

		Ok(())
	}

	#[test]
	fn parse_single_if_eq() -> Result<()> {
		let content = r#"{{@if {{OS}} == "windows"}}{{@fi}}"#;

		let source = Source::anonymous(content);
		let mut parser = Parser::new(Session::new(source));
		let block = parser
			.next_top_level_block()
			.expect("Found no block")
			.expect("Encountered a parse error");

		assert_eq!(block.span(), &ByteSpan::new(0usize, content.len()));

		let if_span = ByteSpan::new(0usize, 27usize);
		assert_eq!(&content[if_span], r#"{{@if {{OS}} == "windows"}}"#);

		let name = ByteSpan::new(8usize, 10usize);
		assert_eq!(&content[name], "OS");
		let envs = VarEnvSet([Some(VarEnv::Item), Some(VarEnv::Profile), None]);

		let op = IfOp::Eq;

		let other = ByteSpan::new(17usize, 24usize);
		assert_eq!(&content[other], "windows");

		let end_span = ByteSpan::new(27usize, 34usize);
		assert_eq!(&content[end_span], r#"{{@fi}}"#);

		assert_eq!(
			block.kind(),
			&BlockKind::If(If {
				head: (
					if_span.span(IfExpr::Compare {
						var: Var { envs, name },
						op,
						other
					}),
					vec![]
				),
				elifs: vec![],
				els: None,
				end: end_span
			})
		);

		Ok(())
	}

	#[test]
	fn parse_single_if_neq() -> Result<()> {
		let content = r#"{{@if {{OS}} != "windows"}}{{@fi}}"#;

		let source = Source::anonymous(content);
		let mut parser = Parser::new(Session::new(source));
		let block = parser
			.next_top_level_block()
			.expect("Found no block")
			.expect("Encountered a parse error");

		assert_eq!(block.span(), &ByteSpan::new(0usize, content.len()));

		let if_span = ByteSpan::new(0usize, 27usize);
		assert_eq!(&content[if_span], r#"{{@if {{OS}} != "windows"}}"#);

		let name = ByteSpan::new(8usize, 10usize);
		assert_eq!(&content[name], "OS");
		let envs = VarEnvSet([Some(VarEnv::Item), Some(VarEnv::Profile), None]);

		let op = IfOp::NotEq;

		let other = ByteSpan::new(17usize, 24usize);
		assert_eq!(&content[other], "windows");

		let end_span = ByteSpan::new(27usize, 34usize);
		assert_eq!(&content[end_span], r#"{{@fi}}"#);

		assert_eq!(
			block.kind(),
			&BlockKind::If(If {
				head: (
					if_span.span(IfExpr::Compare {
						var: Var { envs, name },
						op,
						other
					}),
					vec![]
				),
				elifs: vec![],
				els: None,
				end: end_span
			})
		);

		Ok(())
	}

	#[test]
	fn parse_single_if_exists() -> Result<()> {
		let content = r#"{{@if {{$#EXISTS}}}}{{@fi}}"#;

		let source = Source::anonymous(content);
		let mut parser = Parser::new(Session::new(source));
		let block = parser
			.next_top_level_block()
			.expect("Found no block")
			.expect("Encountered a parse error");

		assert_eq!(block.span(), &ByteSpan::new(0usize, content.len()));

		let if_span = ByteSpan::new(0usize, 20usize);
		assert_eq!(&content[if_span], r#"{{@if {{$#EXISTS}}}}"#);

		let name = ByteSpan::new(10usize, 16usize);
		assert_eq!(&content[name], "EXISTS");
		let envs = VarEnvSet([Some(VarEnv::Environment), Some(VarEnv::Profile), None]);

		let end_span = ByteSpan::new(20usize, 27usize);
		assert_eq!(&content[end_span], r#"{{@fi}}"#);

		assert_eq!(
			block.kind(),
			&BlockKind::If(If {
				head: (
					if_span.span(IfExpr::Exists {
						var: Var { envs, name }
					}),
					vec![]
				),
				elifs: vec![],
				els: None,
				end: end_span
			})
		);

		Ok(())
	}

	#[test]
	fn find_blocks() {
		let content = r#"{{ Hello World }} {{{ Escaped {{ }} }} }}}
		{{!-- Hello World {{}} {{{ asdf }}} this is a comment --}}
		{{@if {{}} }} }}
		"#;

		println!("{}", content);

		let iter = BlockIter::new(content);

		// Hello World
		// Text: SPACE
		// Escaped
		// Text: LF SPACES
		// Comment
		// Text: LF SPACES
		// If
		// Text: Closing LF SPACES
		assert_eq!(iter.count(), 8);
	}

	#[test]
	fn find_blocks_unicode() {
		let content = "\u{1f600}{{{ \u{1f600} }}}\u{1f600}";

		let iter = BlockIter::new(content);

		// Text: Smiley
		// Escaped
		// Text: Smiley
		assert_eq!(iter.count(), 3);
	}

	#[test]
	fn parse_comment() -> Result<()> {
		let content = r#"{{!-- Hello World this {{}} is a comment {{{{{{ }}}--}}"#;

		let source = Source::anonymous(content);
		let mut parser = Parser::new(Session::new(source));
		let token = parser
			.next_top_level_block()
			.expect("Found no block")
			.expect("Encountered a parse error");

		assert_eq!(
			token,
			Block::new(ByteSpan::new(0usize, content.len()), BlockKind::Comment)
		);

		Ok(())
	}

	#[test]
	fn parse_escaped() -> Result<()> {
		let content = r#"{{{!-- Hello World this {{}} is a comment {{{{{{ }}--}}}"#;

		let source = Source::anonymous(content);
		let mut parser = Parser::new(Session::new(source));
		let token = parser
			.next_top_level_block()
			.expect("Found no block")
			.expect("Encountered a parse error");

		assert_eq!(
			token,
			Block::new(
				ByteSpan::new(0usize, content.len()),
				BlockKind::Escaped(ByteSpan::new(3usize, content.len() - 3))
			)
		);

		Ok(())
	}

	#[test]
	fn parse_if_cmp() -> Result<()> {
		let content = r#"{{@if {{&OS}} == "windows" }}
		DEMO
		{{@elif {{&OS}} == "linux"  }}
		LINUX
		{{@else}}
		ASD
		{{@fi}}"#;

		let source = Source::anonymous(content);
		let mut parser = Parser::new(Session::new(source));
		let token = parser
			.next_top_level_block()
			.expect("Found no block")
			.expect("Encountered a parse error");

		assert_eq!(token.span, ByteSpan::new(0usize, content.len()));
		println!("{:#?}", &token.kind);

		Ok(())
	}

	#[test]
	fn parse_if_cmp_nested() -> Result<()> {
		let content = r#"{{@if {{&OS}} == "windows" }}
		{{!-- This is a nested comment --}}
		{{{ Escaped {{}} }}}
		{{@elif {{&OS}} == "linux"  }}
		{{!-- Below is a nested variable --}}
		{{ OS }}
		{{@else}}
		ASD
		{{@fi}}"#;

		let source = Source::anonymous(content);
		let mut parser = Parser::new(Session::new(source));
		let token = parser
			.next_top_level_block()
			.expect("Found no block")
			.expect("Encountered a parse error");

		assert_eq!(token.span, ByteSpan::new(0usize, content.len()));
		println!("{:#?}", &token.kind);

		Ok(())
	}

	#[test]
	fn parse_if_exists() -> Result<()> {
		let content = r#"{{@if {{&OS}}  }}
		DEMO
		ASD
		{{@fi}}"#;

		let source = Source::anonymous(content);
		let mut parser = Parser::new(Session::new(source));
		let token = parser
			.next_top_level_block()
			.expect("Found no block")
			.expect("Encountered a parse error");

		assert_eq!(token.span, ByteSpan::new(0usize, content.len()));
		println!("{:#?}", &token.kind);

		Ok(())
	}

	#[test]
	fn parse_if_mixed() -> Result<()> {
		let content = r#"{{@if {{OS}}}}
	print("No value for variable `OS` set")
{{@elif {{&OS}} != "windows"}}
	print("OS is not windows")
{{@elif {{OS}} == "windows"}}
	{{{!-- This is a nested comment. Below it is a nested variable block. --}}}
	print("OS is {{OS}}")
{{@else}}
	{{{!-- This is a nested comment. --}}}
	print("Can never get here. {{{ {{OS}} is neither `windows` nor not `windows`. }}}")
{{@fi}}"#;

		let source = Source::anonymous(content);
		let mut parser = Parser::new(Session::new(source));
		let token = parser
			.next_top_level_block()
			.expect("Found no block")
			.expect("Encountered a parse error");

		assert_eq!(token.span, ByteSpan::new(0usize, content.len()));
		println!("{:#?}", &token.kind);

		Ok(())
	}

	#[test]
	fn parse_variables() -> Result<()> {
		assert_eq!(
			parse_var("$#&FOO_BAR", 0)?,
			Var {
				envs: VarEnvSet([
					Some(VarEnv::Environment),
					Some(VarEnv::Profile),
					Some(VarEnv::Item)
				]),
				name: ByteSpan::new(3usize, 10usize),
			}
		);

		assert_eq!(
			parse_var("&BAZ_1", 0)?,
			Var {
				envs: VarEnvSet([Some(VarEnv::Item), None, None]),
				name: ByteSpan::new(1usize, 6usize),
			}
		);

		assert_eq!(
			parse_var("$#&FOO_BAR", 10)?,
			Var {
				envs: VarEnvSet([
					Some(VarEnv::Environment),
					Some(VarEnv::Profile),
					Some(VarEnv::Item)
				]),
				name: ByteSpan::new(13usize, 20usize),
			}
		);

		// invalid env / var_name
		assert!(parse_var("!FOO_BAR", 10).is_err());
		// duplicate env
		assert!(parse_var("&&FOO_BAR", 0).is_err());

		Ok(())
	}

	#[test]
	fn parse_others() -> Result<()> {
		assert_eq!(parse_other("\"BAZ_1\"", 0)?, ByteSpan::new(1usize, 6usize));
		assert_eq!(
			parse_other("This is a test \"Hello World How are you today\"", 0)?,
			ByteSpan::new(16usize, 45usize)
		);

		assert!(parse_other("This is a test \"Hello World How are you today", 0).is_err());
		assert!(parse_other("This is a test", 0).is_err());

		Ok(())
	}
}
