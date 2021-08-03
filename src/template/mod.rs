mod block;
mod diagnostic;
mod parse;
mod session;
pub(crate) mod source;
mod span;

use color_eyre::eyre::{eyre, Result};

use self::block::{Block, BlockKind, If, IfExpr, Var, VarEnv};
use self::parse::Parser;
use self::session::Session;
use self::source::Source;
use crate::variables::{UserVars, Variables};

// TODO: handle unicode

#[derive(Debug, Clone)]
pub struct Template<'a> {
	source: Source<'a>,
	blocks: Vec<Block>,
}

impl<'a> Template<'a> {
	pub fn parse(source: Source<'a>) -> Result<Self> {
		let session = Session::new(source);

		Parser::new(session).parse()
	}

	// TODO: trim `\r\n` when span start/ends with it
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

		match kind {
			BlockKind::Escaped(inner) => {
				output.push_str(&self.source[inner]);
			}
			BlockKind::Comment => {
				// NOP
			}
			BlockKind::Text => {
				output.push_str(&self.source[span]);
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

				if self.resolve_if_expr(head.value(), profile_vars, item_vars)? {
					for block in head_nested {
						// TODO: if first block is text (trim lf start)
						// TODO: if last block is text (trim lf end)
						self.process_block(profile_vars, item_vars, output, block)?;
					}
				} else {
					for (elif, elif_nested) in elifs {
						if self.resolve_if_expr(elif.value(), profile_vars, item_vars)? {
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

	fn resolve_if_expr(
		&self,
		expr: &IfExpr,
		profile_vars: Option<&UserVars>,
		item_vars: Option<&UserVars>,
	) -> Result<bool> {
		match expr {
			IfExpr::Compare { var, op, other } => {
				let var = self.resolve_var(var, profile_vars, item_vars)?;
				Ok(op.eval(&var, &self.source[other]))
			}
			IfExpr::Exists { var } => Ok(self.resolve_var(var, profile_vars, item_vars).is_ok()),
		}
	}

	fn resolve_var(
		&self,
		var: &Var,
		profile_vars: Option<&UserVars>,
		item_vars: Option<&UserVars>,
	) -> Result<String> {
		let name = &self.source[var.name];

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

		let source = Source::anonymous(content);
		let template = Template::parse(source)?;

		println!("{:#?}", template);

		let mut vars = HashMap::new();
		vars.insert(String::from("BUZZ"), String::from("Hello World"));
		vars.insert(String::from("OS"), String::from("linux"));
		let vars = UserVars { inner: vars };

		println!("{}", template.fill(Some(&vars), Some(&vars))?);

		Ok(())
	}
}
