mod block;
mod parse;
mod span;

use color_eyre::eyre::{eyre, Result};

use self::block::{Block, BlockKind, If, Var, VarEnv};
use self::parse::Parser;
use self::span::Spanned;
use crate::variables::{UserVars, Variables};

#[derive(Debug, Clone)]
pub struct Template<'a> {
	content: &'a str,
	blocks: Vec<Block>,
}

impl<'a> Template<'a> {
	pub fn parse(content: &'a str) -> Result<Self> {
		Parser::new(content).parse()
	}

	pub fn fill(
		&self,
		profile_vars: Option<&UserVars>,
		item_vars: Option<&UserVars>,
	) -> Result<String> {
		let mut output = String::new();

		for block in &self.blocks {
			self.process_block(profile_vars, item_vars, &mut output, block)?;
		}

		Ok(output)
	}

	fn process_block(
		&self,
		profile_vars: Option<&UserVars>,
		item_vars: Option<&UserVars>,
		output: &mut String,
		block: &Block,
	) -> Result<()> {
		let Block { span, kind } = block;

		// TODO: trim `\r\n` when span start/ends with it
		match kind {
			BlockKind::Escaped(inner) => {
				output.push_str(&self.content[inner]);
			}
			BlockKind::Comment => {
				// NOP
			}
			BlockKind::Text => {
				output.push_str(&self.content[span]);
			}
			BlockKind::Var(var) => {
				output.push_str(&self.resolve_var(var, profile_vars, item_vars)?);
			}
			BlockKind::If(If {
				head,
				elifs,
				els,
				end: _,
			}) => {
				let (head, head_nested) = head;

				let head_val = self.resolve_var(&head.var, profile_vars, item_vars)?;

				if head.op.eval(&head_val, &self.content[head.other]) {
					for block in head_nested {
						self.process_block(profile_vars, item_vars, output, block)?;
					}
				} else {
					for (elif, elif_nested) in elifs {
						let Spanned {
							span: _,
							value: elif,
						} = elif;
						let elif_val = self.resolve_var(&elif.var, profile_vars, item_vars)?;

						if elif.op.eval(&elif_val, &self.content[elif.other]) {
							// return if matching elif arm was found
							for block in elif_nested {
								self.process_block(profile_vars, item_vars, output, block)?;
							}

							return Ok(());
						}
					}

					if let Some((_, els_nested)) = els {
						for block in els_nested {
							self.process_block(profile_vars, item_vars, output, block)?;
						}
					}
				}
			}
		};

		Ok(())
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
