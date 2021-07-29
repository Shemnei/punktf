use color_eyre::eyre::{eyre, Result};

use super::span::{CharSpan, Spanned};
use super::token::{If, IfExpr, IfOp, Token, Var, VarEnv, VarEnvSet};
use super::Template;

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
		let tokens =
			std::iter::from_fn(|| self.parse_next_block()).collect::<Result<Vec<_>, _>>()?;

		Ok(Template {
			content: self.content,
			tokens,
		})
	}

	fn parse_next_block(&mut self) -> Option<Result<Spanned<Token>>> {
		let (span, content) = self.blocks.next()?;
		let content = &content[2..content.len() - 2];

		println!("Content: `{}`", content);

		if let Some(b'{') = content.as_bytes().get(0) {
			let escaped = span.offset_low(3).offset_high(-3);
			return Some(Ok(span.span(Token::Escaped(escaped))));
		}

		if let Some(b"!--") = content.as_bytes().get(..3) {
			return Some(Ok(span.span(Token::Comment)));
		}

		if let Some(b"@if") = content.as_bytes().get(..3) {
			return Some(self.parse_if_blocks(span.span(content)));
		}

		match parse_var(content, span.low().as_usize() + 2) {
			Ok(var) => Some(Ok(span.span(Token::Var(var)))),
			Err(err) => Some(Err(err)),
		}
	}

	fn parse_if_blocks(&mut self, head: Spanned<&str>) -> Result<Spanned<Token>> {
		// TODO: parse head
		// check if next block is else / elif / fi

		let (span, content) = head.into_inner();

		let head = {
			let start = content
				.find("{{")
				.ok_or_else(|| eyre!("Found no variable block in if at {}", span))?;
			let end = content
				.find("}}")
				.ok_or_else(|| eyre!("Found no closing for variable block in if at {}", span))?;

			// +2 for if opening `{{` the other +2 for var opening
			let var_start = span.low().as_usize() + 2 + start + 2;

			let var = parse_var(&content[start + 2..end], var_start)?;
			let op = parse_ifop(&content[end + 2..])?;
			let other = parse_other(&content[end + 2..], var_start)?;

			span.span(IfExpr { var, op, other })
		};

		println!("{:#?}", head);

		println!("VAR: {}", &self.content[head.value().var.name]);
		println!("Other: {}", &self.content[head.value().other]);

		// check next block for elif
		let mut elifs = Vec::new();

		let (mut span, mut content) = self
			.blocks
			.next()
			.ok_or_else(|| eyre!("Unexpected end of if at {:?}", span))?;

		loop {
			content = &content[2..content.len() - 2];

			if let Some(b"@elif") = content.as_bytes().get(..5) {
				let elif = {
					let start = content
						.find("{{")
						.ok_or_else(|| eyre!("Found no variable block in elif at {}", span))?;
					let end = content.find("}}").ok_or_else(|| {
						eyre!("Found no closing for variable block in elif at {}", span)
					})?;

					// +2 for if opening `{{` the other +2 for var opening
					let var_start = span.low().as_usize() + 2 + start + 2;

					let var = parse_var(&content[start + 2..end], var_start)?;
					let op = parse_ifop(&content[end + 2..])?;
					let other = parse_other(&content[end + 2..], var_start)?;

					span.span(IfExpr { var, op, other })
				};

				println!("{:#?}", elif);

				println!("VAR: {}", &self.content[elif.value().var.name]);
				println!("Other: {}", &self.content[elif.value().other]);

				elifs.push(elif);
			} else {
				break;
			}

			let (_span, _content) = self
				.blocks
				.next()
				.ok_or_else(|| eyre!("Unexpected end of if at {:?}", span))?;

			span = _span;
			content = _content;
		}

		// check next block for else
		let els = if content == "@else" {
			let (_span, _content) = self
				.blocks
				.next()
				.ok_or_else(|| eyre!("Unexpected end of if at {:?}", span))?;

			let ret = Some(span);

			span = _span;
			content = &_content[2.._content.len() - 2];

			ret
		} else {
			None
		};

		// check next block for fi
		if content != "@fi" {
			return Err(eyre!("Unexpected end of if at {:?}", span));
		}

		let end = span;

		let whole_span = head.span().union(&end);

		Ok(whole_span.span(Token::If(If {
			head,
			elifs,
			els,
			end,
		})))
	}
}

fn find_block(s: &str) -> Option<Result<CharSpan>> {
	if let Some(low) = s.find("{{") {
		if let Some(b'{') = s.as_bytes().get(low + 2) {
			// block is an escaped block
			if let Some(high) = s.find("}}}") {
				Some(Ok(CharSpan::new(low, high + 3)))
			} else {
				Some(Err(eyre!(
					"Found opening for an escaped block at {} but no closing",
					low
				)))
			}
		} else if let Some(b"!--") = s.as_bytes().get(low + 2..low + 5) {
			// block is an comment block
			if let Some(high) = s.find("--}}") {
				Some(Ok(CharSpan::new(low, high + 4)))
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
				return Some(Ok(CharSpan::new(low, high)));
			}

			Some(Err(eyre!(
				"Found opening for a block at {} but no closing",
				low
			)))
		}
	} else {
		None
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
			"Found invalid symbol in variable name: `{}`",
			invalid
		))
	} else {
		Ok(Var {
			envs,
			name: CharSpan::new(offset, offset + inner.len()),
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
fn parse_other(inner: &str, offset: usize) -> Result<CharSpan> {
	let mut matches = inner.match_indices('"').map(|(idx, _)| idx);

	match (matches.next(), matches.next()) {
		(Some(low), Some(high)) => Ok(CharSpan::new(offset + low + 1, offset + high)),
		(Some(low), None) => Err(eyre!(
			"Found opening `\"` at {} but no closing",
			low + offset
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
	type Item = (CharSpan, &'a str);

	fn next(&mut self) -> Option<Self::Item> {
		let mut span = find_block(&self.content[self.index..])?.unwrap();

		span = span.offset(self.index as i32);
		self.index = span.high().as_usize();

		Some((span, &self.content[span]))
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::template::span::{CharPos, CharSpan};

	#[test]
	fn find_blocks() {
		let content = r#"{{ Hello World }} {{{ Escaped {{ }} }} }}}
		{{!-- Hello World {{}} {{{ asdf }}} this is a comment --}}
		{{@if {{}} }} }}
		"#;

		println!("{}", content);

		let iter = BlockIter::new(content);

		for (span, content) in iter {
			println!("{:?}: {}", span, content);
		}
	}

	#[test]
	fn parse_comment() -> Result<()> {
		let content = r#"{{!-- Hello World this {{}} is a comment {{{{{{ }}}--}}"#;

		let mut parser = Parser::new(content);
		let token = parser.parse_next_block().ok_or(eyre!("No token found"))??;

		assert_eq!(
			token,
			Spanned::new(CharSpan::new(0usize, content.len()), Token::Comment)
		);

		Ok(())
	}

	#[test]
	fn parse_escaped() -> Result<()> {
		let content = r#"{{{!-- Hello World this {{}} is a comment {{{{{{ }}--}}}"#;

		let mut parser = Parser::new(content);
		let token = parser.parse_next_block().ok_or(eyre!("No token found"))??;

		assert_eq!(
			token,
			Spanned::new(
				CharSpan::new(0usize, content.len()),
				Token::Escaped(CharSpan::new(3usize, content.len() - 3))
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
		let token = parser.parse_next_block().ok_or(eyre!("No token found"))??;

		assert!(matches!(
			token,
			Spanned {
				span: CharSpan {
					low: CharPos(l),
					high: CharPos(h)
				},
				value: Token::If(_)
			} if l == 0 && h as usize == content.len()
		));

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
				name: CharSpan::new(3usize, 10usize),
			}
		);

		assert_eq!(
			parse_var("&BAZ_1", 0)?,
			Var {
				envs: VarEnvSet([Some(VarEnv::Item), None, None]),
				name: CharSpan::new(1usize, 6usize),
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
				name: CharSpan::new(13usize, 20usize),
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
		assert_eq!(parse_other("\"BAZ_1\"", 0)?, CharSpan::new(1usize, 6usize));
		assert_eq!(
			parse_other("This is a test \"Hello World How are you today\"", 0)?,
			CharSpan::new(16usize, 45usize)
		);

		assert!(parse_other("This is a test \"Hello World How are you today", 0).is_err());
		assert!(parse_other("This is a test", 0).is_err());

		Ok(())
	}
}
