use color_eyre::eyre::{eyre, Result};

use super::block::{Block, BlockHint, If, IfExpr, IfOp, Var, VarEnv, VarEnvSet};
use super::span::{ByteSpan, Spanned};
use super::Template;
use crate::template::block::BlockKind;

#[derive(Debug, Clone, Copy)]
pub struct Parser<'a> {
	content: &'a str,
	blocks: BlockIter<'a>,
}

impl<'a> Parser<'a> {
	pub fn new(s: &'a str) -> Self {
		Self {
			content: s,
			blocks: BlockIter::new(s),
		}
	}

	pub fn parse(mut self) -> Result<Template<'a>> {
		let blocks =
			std::iter::from_fn(|| self.parse_next_block()).collect::<Result<Vec<_>, _>>()?;

		// TODO: validate structure (e.g. if/elif/fi)

		Ok(Template {
			content: self.content,
			blocks,
		})
	}

	fn parse_next_block(&mut self) -> Option<Result<Block>> {
		let Spanned { span, value: hint } = match self.next_block()? {
			Ok(x) => x,
			Err(err) => return Some(Err(err)),
		};

		log::trace!("{:?}: {}", hint, &self.content[span]);

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
			BlockHint::ElIf => Err(eyre!("Found invalid top level block elif at {}",)),
			BlockHint::Else => Err(eyre!("Found invalid top level block else at {}", span)),
			BlockHint::IfEnd => Err(eyre!("Found invalid top level block fi at {}", span)),
		};

		Some(block)
	}

	fn next_block(&mut self) -> Option<Result<Spanned<BlockHint>>> {
		// TODO: ceck perf as_bytes().get() vs starts_with/ends_with

		let (span, hint, content) = match self.blocks.next()? {
			Ok(x) => x,
			Err(err) => return Some(Err(err)),
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

	fn parse_variable(&self, span: ByteSpan) -> Result<Var> {
		let span_inner = span.offset_low(2).offset_high(-2);
		let content_inner = &self.content[span_inner];

		// +2 for block opening
		let offset = span.low().as_usize() + 2;

		parse_var(content_inner, offset)
	}

	fn parse_if(&mut self, span: ByteSpan) -> Result<Spanned<If>> {
		let head = span.span(self.parse_if_start(span)?);

		// collect all nested blocks
		let head_nested = self.parse_if_enclosed_blocks(span)?;

		let Spanned {
			mut span,
			value: mut hint,
		} = self
			.next_block()
			.ok_or_else(|| eyre!("Unexpected end of if at {:?}", span))??;

		// check for elif
		let mut elifs = Vec::new();

		while hint == BlockHint::ElIf {
			let elif = span.span(self.parse_elif(span)?);
			let elif_nested = self.parse_if_enclosed_blocks(span)?;
			elifs.push((elif, elif_nested));

			let Spanned {
				span: _span,
				value: _hint,
			} = self
				.next_block()
				.ok_or_else(|| eyre!("Unexpected end of elif at {:?}", span))??;

			span = _span;
			hint = _hint;
		}

		let els = if hint == BlockHint::Else {
			let els = self.parse_else(span)?;
			let els_nested = self.parse_if_enclosed_blocks(span)?;

			let Spanned {
				span: _span,
				value: _hint,
			} = self
				.next_block()
				.ok_or_else(|| eyre!("Unexpected end of elif at {:?}", span))??;

			span = _span;
			hint = _hint;

			Some((els, els_nested))
		} else {
			None
		};

		let end = if hint == BlockHint::IfEnd {
			self.parse_if_end(span)?
		} else {
			return Err(eyre!("No end (fi) for if at {}", head.span()));
		};

		let whole_if = head.span.union(&end);

		Ok(whole_if.span(If {
			head: (head, head_nested),
			elifs,
			els,
			end,
		}))
	}

	fn parse_if_start(&self, span: ByteSpan) -> Result<IfExpr> {
		// {{@if {{VAR}} (!=|==) "LIT" }}
		let expr_span = span.offset_low(6).offset_high(-2);
		self.parse_if_expr(expr_span)
	}

	fn parse_elif(&self, span: ByteSpan) -> Result<IfExpr> {
		// {{@elif {{VAR}} (!=|==) "LIT" }}
		let expr_span = span.offset_low(8).offset_high(-2);
		self.parse_if_expr(expr_span)
	}

	fn parse_else(&self, span: ByteSpan) -> Result<ByteSpan> {
		if &self.content[span] != "{{@else}}" {
			Err(eyre!("Invalid else block at {}", span))
		} else {
			Ok(span)
		}
	}

	fn parse_if_end(&self, span: ByteSpan) -> Result<ByteSpan> {
		if &self.content[span] != "{{@fi}}" {
			Err(eyre!("Invalid fi block at {}", span))
		} else {
			Ok(span)
		}
	}

	fn parse_if_expr(&self, span: ByteSpan) -> Result<IfExpr> {
		// {{VAR}} (!=|==) "OTHER"
		let content = &self.content[span];

		// read var
		let var_block_start = content
			.find("{{")
			.ok_or_else(|| eyre!("Found no variable block in if at {}", span))?;
		let var_block_end = content
			.find("}}")
			.ok_or_else(|| eyre!("Found no closing for variable block in if at {}", span))?
			+ 2;

		let var_block_span = ByteSpan::new(
			span.low().as_usize() + var_block_start,
			span.low().as_usize() + var_block_end,
		);

		let var = self.parse_variable(var_block_span)?;

		let op = parse_ifop(&content[var_block_end..])?;

		let other = parse_other(
			&content[var_block_end..],
			span.low().as_usize() + var_block_end,
		)?;

		Ok(IfExpr { var, op, other })
	}

	fn parse_if_enclosed_blocks(&mut self, start_span: ByteSpan) -> Result<Vec<Block>> {
		let mut enclosed_blocks = Vec::new();

		while !self
			.peek_block_hint()
			.ok_or_else(|| eyre!("Unexpected end of if at {:?}", start_span))??
			.is_if_subblock()
		{
			let next_block = self
				.parse_next_block()
				.ok_or_else(|| eyre!("Unexpected end of if at {:?}", start_span))??;

			enclosed_blocks.push(next_block);
		}

		Ok(enclosed_blocks)
	}

	fn peek_block_hint(&self) -> Option<Result<BlockHint>> {
		let mut peek = *self;
		peek.next_block()
			.map(|opt| opt.map(|spanned| spanned.into_value()))
	}
}

fn next_block(s: &str) -> Option<Result<(ByteSpan, Option<BlockHint>)>> {
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
				Some(Err(eyre!(
					"Found opening for an escaped block at {} but no closing",
					low
				)))
			}
		} else if let Some(b"!--") = s.as_bytes().get(low + 2..low + 5) {
			// block is an comment block
			if let Some(high) = s.find("--}}") {
				Some(Ok((ByteSpan::new(low, high + 4), Some(BlockHint::Comment))))
			} else {
				Some(Err(eyre!(
					"Found opening for a comment block at {} but no closing",
					low
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

			Some(Err(eyre!(
				"Found opening for a block at {} but no closing",
				low
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
				break;
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
	type Item = Result<(ByteSpan, Option<BlockHint>, &'a str)>;

	fn next(&mut self) -> Option<Self::Item> {
		let (mut span, hint) = match next_block(&self.content[self.index..])? {
			Ok(x) => x,
			Err(err) => return Some(Err(err)),
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
	use crate::template::span::ByteSpan;

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

		let mut parser = Parser::new(content);
		let token = parser.parse_next_block().ok_or(eyre!("No block found"))??;

		assert_eq!(
			token,
			Block::new(ByteSpan::new(0usize, content.len()), BlockKind::Comment)
		);

		Ok(())
	}

	#[test]
	fn parse_escaped() -> Result<()> {
		let content = r#"{{{!-- Hello World this {{}} is a comment {{{{{{ }}--}}}"#;

		let mut parser = Parser::new(content);
		let token = parser.parse_next_block().ok_or(eyre!("No block found"))??;

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
	fn parse_if() -> Result<()> {
		let content = r#"{{@if {{&OS}} == "windows" }}
		DEMO
		{{@elif {{&OS}} == "linux"  }}
		LINUX
		{{@else}}
		ASD
		{{@fi}}"#;

		let mut parser = Parser::new(content);
		let token = parser.parse_next_block().ok_or(eyre!("No block found"))??;

		assert_eq!(token.span, ByteSpan::new(0usize, content.len()));
		println!("{:#?}", &token.kind);

		Ok(())
	}

	#[test]
	fn parse_if_nested() -> Result<()> {
		let content = r#"{{@if {{&OS}} == "windows" }}
		{{!-- This is a nested comment --}}
		{{{ Escaped {{}} }}}
		{{@elif {{&OS}} == "linux"  }}
		{{!-- Below is a nested variable --}}
		{{ OS }}
		{{@else}}
		ASD
		{{@fi}}"#;

		let mut parser = Parser::new(content);
		let token = parser.parse_next_block().ok_or(eyre!("No block found"))??;

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