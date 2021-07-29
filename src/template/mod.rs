mod block;
mod parse;
mod span;

use color_eyre::eyre::{eyre, Result};

use self::block::{Block, BlockKind, If, IfExpr, Var, VarEnv};
use self::parse::Parser;
use self::span::{ByteSpan, Spanned};
use crate::variables::{UserVars, Variables};

// TODO: handle unicode

#[derive(Debug, Clone)]
pub struct Template<'a> {
	content: &'a str,
	blocks: Vec<Block>,
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

		for Block { span, kind } in &self.blocks {
			match kind {
				BlockKind::Var(var) => {
					output.push_str(&self.resolve_var(var, profile_vars, item_vars)?);
				}
				BlockKind::If(If {
					head,
					elifs,
					els,
					end,
				}) => {
					let head_val = self.resolve_var(&head.var, profile_vars, item_vars)?;
					if head.op.eval(&head_val, &self.content[head.other]) {
						let span = ByteSpan::new(
							head.span().high().as_usize(),
							elifs
								.first()
								.map(|elif| elif.span())
								.unwrap_or_else(|| els.as_ref().unwrap_or(end))
								.low()
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
								let span = ByteSpan::new(
									span.high().as_usize(),
									elifs
										.get(idx + 1)
										.map(|elif| elif.span())
										.unwrap_or_else(|| els.as_ref().unwrap_or(end))
										.low()
										.as_usize(),
								);
								output.push_str(&self.content[span]);
								found = true;
							}
						}

						if !found {
							if let Some(span) = els {
								let span =
									ByteSpan::new(span.high().as_usize(), end.low().as_usize());
								output.push_str(&self.content[span]);
							}
						}
					}
				}
				BlockKind::Escaped(inner) => {
					output.push_str(&self.content[inner]);
				}
				BlockKind::Comment => {
					// NOP
				}
				BlockKind::Text => {
					output.push_str(&self.content[span]);
				}
			};
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

#[cfg(test)]
mod tests {
	use std::collections::HashMap;

	use super::*;

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
