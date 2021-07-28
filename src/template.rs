use std::fmt;
use std::ops::{Deref, Index};

use color_eyre::eyre::{eyre, Result};

use crate::variables::{UserVars, Variables};

// TODO: handle unicode

type CharPosType = u32;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CharPos(CharPosType);

impl CharPos {
	pub fn from_usize(value: usize) -> Self {
		Self(value as CharPosType)
	}

	pub fn as_usize(&self) -> usize {
		self.0 as usize
	}
}

impl From<usize> for CharPos {
	fn from(value: usize) -> Self {
		CharPos::from_usize(value)
	}
}

impl From<CharPosType> for CharPos {
	fn from(value: CharPosType) -> Self {
		Self(value)
	}
}

impl From<CharPos> for usize {
	fn from(value: CharPos) -> Self {
		value.as_usize()
	}
}

impl From<CharPos> for CharPosType {
	fn from(value: CharPos) -> Self {
		value.0
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CharSpan {
	low: CharPos,
	high: CharPos,
}

impl CharSpan {
	pub fn new<L: Into<CharPos>, H: Into<CharPos>>(low: L, high: H) -> Self {
		let mut low = low.into();
		let mut high = high.into();

		if low > high {
			std::mem::swap(&mut low, &mut high);
		}

		Self { low, high }
	}

	pub fn span<T>(self, value: T) -> Spanned<T> {
		Spanned::new(self, value)
	}
}

impl Index<CharSpan> for str {
	type Output = str;

	fn index(&self, index: CharSpan) -> &Self::Output {
		&self[index.low.as_usize()..index.high.as_usize()]
	}
}

impl Index<&CharSpan> for str {
	type Output = str;

	fn index(&self, index: &CharSpan) -> &Self::Output {
		&self[index.low.as_usize()..index.high.as_usize()]
	}
}

pub struct Spanned<T> {
	span: CharSpan,
	value: T,
}

impl<T> Spanned<T> {
	fn new(span: CharSpan, value: T) -> Self {
		Self { span, value }
	}

	fn span(&self) -> &CharSpan {
		&self.span
	}

	fn value(&self) -> &T {
		&self.value
	}

	fn into_span(self) -> CharSpan {
		self.span
	}

	fn into_value(self) -> T {
		self.value
	}

	fn into_inner(self) -> (CharSpan, T) {
		(self.span, self.value)
	}
}

impl<T> fmt::Debug for Spanned<T>
where
	T: fmt::Debug,
{
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_struct("Spanned")
			.field("span", &self.span)
			.field("value", &self.value)
			.finish()
	}
}

impl<T> Clone for Spanned<T>
where
	T: Clone,
{
	fn clone(&self) -> Self {
		Self {
			span: self.span,
			value: self.value.clone(),
		}
	}
}

impl<T> Copy for Spanned<T> where T: Copy {}

impl<T> PartialEq for Spanned<T>
where
	T: PartialEq,
{
	fn eq(&self, other: &Self) -> bool {
		self.span.eq(&other.span) && self.value.eq(&other.value)
	}
}

impl<T> Eq for Spanned<T> where T: Eq {}

impl<T> Deref for Spanned<T> {
	type Target = T;

	fn deref(&self) -> &Self::Target {
		&self.value
	}
}

#[derive(Debug, Clone, PartialEq)]
enum Token {
	Var(Var),
	If(If),
	Escaped(CharSpan),
	Comment,
}

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Hash)]
enum VarEnv {
	Environment,
	Profile,
	Item,
}

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Hash)]
struct VarEnvSet([Option<VarEnv>; 3]);

impl VarEnvSet {
	fn empty() -> Self {
		Self([None; 3])
	}

	fn add(&mut self, value: VarEnv) -> bool {
		if self.0.contains(&Some(value)) {
			false
		} else if let Some(slot) = self.0.iter_mut().find(|x| x.is_none()) {
			*slot = Some(value);
			true
		} else {
			false
		}
	}

	fn envs(&self) -> impl Iterator<Item = &VarEnv> {
		self.0.iter().filter_map(|x| x.as_ref())
	}

	fn len(&self) -> usize {
		self.envs().count()
	}

	fn capacity(&self) -> usize {
		self.0.len()
	}
}

impl Default for VarEnvSet {
	fn default() -> Self {
		Self([
			Some(VarEnv::Item),
			Some(VarEnv::Profile),
			Some(VarEnv::Environment),
		])
	}
}

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Hash)]
struct Var {
	envs: VarEnvSet,
	name: CharSpan,
}

// For now only eq.
#[derive(Debug, Clone, PartialEq)]
struct If {
	head: Spanned<IfExpr>,
	elifs: Vec<Spanned<IfExpr>>,
	els: Option<CharSpan>,
	end: CharSpan,
}

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Hash)]
enum IfOp {
	Eq,
	NotEq,
}

impl IfOp {
	fn eval(&self, lhs: &str, rhs: &str) -> bool {
		match self {
			Self::Eq => lhs == rhs,
			Self::NotEq => lhs != rhs,
		}
	}
}

// For now only eq.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Hash)]
struct IfExpr {
	var: Var,
	op: IfOp,
	other: CharSpan,
}

struct Parser<'a> {
	content: &'a str,
	blocks: BlockIter<'a>,
}

impl<'a> Parser<'a> {
	fn new(s: &'a str) -> Self {
		Self {
			content: s,
			blocks: BlockIter::new(s),
		}
	}

	fn parse(mut self) -> Result<Template<'a>> {
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
			let escaped = CharSpan::new(span.low.as_usize() + 3, span.high.as_usize() - 3);
			return Some(Ok(Spanned::new(span, Token::Escaped(escaped))));
		}

		if let Some(b"!--") = content.as_bytes().get(..3) {
			return Some(Ok(Spanned::new(span, Token::Comment)));
		}

		if let Some(b"@if") = content.as_bytes().get(..3) {
			return Some(self.parse_if_blocks(span.span(content)));
		}

		match parse_var(content, span.low.as_usize() + 2) {
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
				.ok_or_else(|| eyre!("Found no variable block in if at {:?}", span.low))?;
			let end = content
				.find("}}")
				.ok_or_else(|| eyre!("Found no closing for variable block in if at {:?}", start))?;

			// +2 for if opening `{{` the other +2 for var opening
			let var = parse_var(
				&content[start + 2..end],
				span.low.as_usize() + 2 + start + 2,
			)?;
			let op = parse_ifop(&content[end + 2..])?;
			let other = parse_other(&content[end + 2..], span.low.as_usize() + 2 + end + 2)?;

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
					let start = content.find("{{").ok_or_else(|| {
						eyre!("Found no variable block in elif at {:?}", span.low)
					})?;
					let end = content.find("}}").ok_or_else(|| {
						eyre!("Found no closing for variable block in elif at {:?}", start)
					})?;

					// +2 for if opening `{{` the other +2 for var opening
					let var = parse_var(
						&content[start + 2..end],
						span.low.as_usize() + 2 + start + 2,
					)?;
					let op = parse_ifop(&content[end + 2..])?;
					let other =
						parse_other(&content[end + 2..], span.low.as_usize() + 2 + end + 2)?;

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

		let whole_span = CharSpan::new(head.span().low, end.high);

		Ok(whole_span.span(Token::If(If {
			head,
			elifs,
			els,
			end,
		})))
	}
}

#[derive(Debug, Clone)]
pub struct Template<'a> {
	content: &'a str,
	tokens: Vec<Spanned<Token>>,
}

impl<'a> Template<'a> {
	pub fn parse(content: &'a str) -> Result<Self> {
		Parser::new(content).parse()
	}

	// TODO: trim `\r\n` when span start/ends with it
	pub fn fill(
		&self,
		profile_vars: Option<&UserVars>,
		item_vars: Option<&UserVars>,
	) -> Result<String> {
		let mut output = String::new();
		let mut idx = 0;

		for Spanned { span, value: token } in &self.tokens {
			if idx < span.low.as_usize() {
				output.push_str(&self.content[idx..span.low.as_usize()])
			}

			match token {
				Token::Var(var) => {
					output.push_str(&self.resolve_var(var, profile_vars, item_vars)?);
				}
				Token::If(If {
					head,
					elifs,
					els,
					end,
				}) => {
					let head_val = self.resolve_var(&head.var, profile_vars, item_vars)?;
					if head.op.eval(&head_val, &self.content[head.other]) {
						let span = CharSpan::new(
							head.span().high.as_usize(),
							elifs
								.first()
								.map(|elif| elif.span())
								.unwrap_or_else(|| els.as_ref().unwrap_or(end))
								.low
								.as_usize(),
						);
						output.push_str(&self.content[span]);
					} else {
						let mut found = false;
						for idx in 0..elifs.len() {
							let Spanned {
								span,
								value: IfExpr { var, op, other },
							} = &elifs[idx];

							let elif_val = self.resolve_var(var, profile_vars, item_vars)?;

							if op.eval(&elif_val, &self.content[other]) {
								let span = CharSpan::new(
									span.high.as_usize(),
									elifs
										.get(idx + 1)
										.map(|elif| elif.span())
										.unwrap_or_else(|| els.as_ref().unwrap_or(end))
										.low
										.as_usize(),
								);
								output.push_str(&self.content[span]);
								found = true;
							}
						}

						if !found {
							if let Some(span) = els {
								let span = CharSpan::new(span.high.as_usize(), end.low.as_usize());
								output.push_str(&self.content[span]);
							}
						}
					}
				}
				Token::Escaped(inner) => {
					output.push_str(&self.content[inner]);
				}
				Token::Comment => {
					// NOP
				}
			};

			idx = span.high.as_usize();
		}

		if idx < self.content.len() {
			output.push_str(&self.content[idx..]);
		}

		Ok(output)
	}

	fn resolve_var(
		&self,
		var: &Var,
		profile_vars: Option<&UserVars>,
		item_vars: Option<&UserVars>,
	) -> Result<String> {
		let name = &self.content[var.name];

		for env in var.envs.envs() {
			match env {
				VarEnv::Environment => {
					if let Ok(val) = std::env::var(name) {
						return Ok(val);
					}
				}
				VarEnv::Profile => {
					if let Some(Some(val)) = profile_vars.map(|vars| vars.var(name)) {
						return Ok(val.to_string());
					}
				}
				VarEnv::Item => {
					if let Some(Some(val)) = item_vars.map(|vars| vars.var(name)) {
						return Ok(val.to_string());
					}
				}
			};
		}

		Err(eyre!(
			"Failed to resolve variable `{}` (Envs: {:?})",
			name,
			var.envs
		))
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

		// TODO: make better
		span.low.0 += self.index as CharPosType;
		span.high.0 += self.index as CharPosType;

		self.index = span.high.as_usize();

		Some((span, &self.content[span]))
	}
}

#[cfg(test)]
mod tests {
	use std::collections::HashMap;

	use super::*;

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

	#[test]
	fn parse_template() -> Result<()> {
		let content = r#"
			[some settings]
			var = 2
			foo = "bar"
			fizz = {{BUZZ}}
			escaped = {{{42}}}

			{{!--
				Sets the message of the day for a specific operating system
				If no os matches it defaults to a generic one.
			--}}
			{{@if {{&OS}} == "linux" }}
			[linux]
			motd = "very nice"
			{{@elif {{&#OS}} == "windows" }}
			[windows]
			motd = "nice"
			{{@else}}
			[other]
			motd = "who knows"
			{{@fi}}

			{{!-- Check if not windows --}}
			{{@if {{&OS}} != "windows"}}
			windows = false
			{{@fi}}

			[last]
			num = 23
			threads = 1337
			os_str = "_unkown"
			"#;

		let template = Template::parse(content)?;

		println!("{:#?}", template);

		let mut vars = HashMap::new();
		vars.insert(String::from("BUZZ"), String::from("Hello World"));
		vars.insert(String::from("OS"), String::from("linux"));
		let vars = UserVars { inner: vars };

		println!("{}", template.fill(Some(&vars), Some(&vars))?);

		Ok(())
	}
}
