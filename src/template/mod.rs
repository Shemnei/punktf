mod block;
mod parse;
mod span;

use color_eyre::eyre::{eyre, Result};
use log::info;

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
			BlockKind::Comment => {
				// NOP
			}
			BlockKind::Print(inner) => {
				info!("[Print] {}", &self.content[inner]);
			}
			BlockKind::Text => {
				output.push_str(&self.content[span]);
			}
			BlockKind::Escaped(inner) => {
				output.push_str(&self.content[inner]);
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
							// Exit after first successful elif condition
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
		let _ = env_logger::Builder::from_env(
			env_logger::Env::default().default_filter_or(log::Level::Debug.to_string()),
		)
		.is_test(true)
		.try_init();

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
			{{@print Writing motd...}}
			{{@if {{&OS}} == "linux" }}
			{{@print Linux Motd!}}
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

		// println!("{:#?}", template);

		let mut vars = HashMap::new();
		vars.insert(String::from("BUZZ"), String::from("Hello World"));
		vars.insert(String::from("OS"), String::from("linux"));
		let vars = UserVars { inner: vars };

		println!("{}", template.fill(Some(&vars), Some(&vars))?);

		Ok(())
	}

	#[test]
	fn parse_template_vars() -> Result<()> {
		// Default
		let content = r#"{{OS}}"#;
		let template = Template::parse(content)?;

		let profile_vars = UserVars::from_items(vec![("OS", "windows")]);
		let item_vars = UserVars::from_items(vec![("OS", "unix")]);
		std::env::set_var("OS", "macos");

		assert_eq!(
			template.fill(Some(&profile_vars), Some(&item_vars))?,
			"unix"
		);

		// Profile
		let content = r#"{{#OS}}"#;
		let template = Template::parse(content)?;

		let profile_vars = UserVars::from_items(vec![("OS", "windows")]);
		let item_vars = UserVars::from_items(vec![("OS", "unix")]);
		std::env::set_var("OS", "macos");

		assert_eq!(
			template.fill(Some(&profile_vars), Some(&item_vars))?,
			"windows"
		);

		// Item
		let content = r#"{{&OS}}"#;
		let template = Template::parse(content)?;

		let profile_vars = UserVars::from_items(vec![("OS", "windows")]);
		let item_vars = UserVars::from_items(vec![("OS", "unix")]);
		std::env::set_var("OS", "macos");

		assert_eq!(
			template.fill(Some(&profile_vars), Some(&item_vars))?,
			"unix"
		);

		// Env
		let content = r#"{{$OS}}"#;
		let template = Template::parse(content)?;

		let profile_vars = UserVars::from_items(vec![("OS", "windows")]);
		let item_vars = UserVars::from_items(vec![("OS", "unix")]);
		std::env::set_var("OS", "macos");

		assert_eq!(
			template.fill(Some(&profile_vars), Some(&item_vars))?,
			"macos"
		);

		// Mixed - First
		let content = r#"{{$#OS}}"#;
		let template = Template::parse(content)?;

		let profile_vars = UserVars::from_items(vec![("OS", "windows")]);
		let item_vars = UserVars::from_items(vec![("OS", "unix")]);
		std::env::set_var("OS", "macos");

		assert_eq!(
			template.fill(Some(&profile_vars), Some(&item_vars))?,
			"macos"
		);

		// Mixed - Last
		let content = r#"{{$&OS}}"#;
		let template = Template::parse(content)?;

		let profile_vars = UserVars::from_items(vec![("OS", "windows")]);
		let item_vars = UserVars::from_items(vec![("OS", "unix")]);
		std::env::remove_var("OS");

		assert_eq!(
			template.fill(Some(&profile_vars), Some(&item_vars))?,
			"unix"
		);

		Ok(())
	}
}
