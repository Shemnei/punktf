//! The code for error/diagnostics and source input handling is heavily inspired by
//! [rust's](https://github.com/rust-lang/rust) compiler, which is licensed under the MIT license.
//! While some code is adapted for use with `punktf`, some of it is also a plain copy of it. If a
//! portion of code was copied/adapted from the Rust project there will be an explicit notices
//! above it. For further information and the license please see the `COPYRIGHT` file in the root
//! of this project.
//!
//! Specifically but not limited to:
//! - <https://github.com/rust-lang/rust/blob/master/compiler/rustc_span/src/lib.rs>
//! - <https://github.com/rust-lang/rust/blob/master/compiler/rustc_span/src/analyze_source_file.rs>
//! - <https://github.com/rust-lang/rust/blob/master/compiler/rustc_parse/src/parser/diagnostics.rs>
//! - <https://github.com/rust-lang/rust/blob/master/compiler/rustc_errors/src/diagnostic.rs>
//! - <https://github.com/rust-lang/rust/blob/master/compiler/rustc_errors/src/diagnostic_builder.rs>
//! - <https://github.com/rust-lang/rust/blob/master/compiler/rustc_errors/src/emitter.rs>

mod block;
mod diagnostic;
mod parse;
mod session;
pub mod source;
mod span;

use color_eyre::eyre::Result;

use self::block::{Block, BlockKind, If, IfExpr, Var, VarEnv};
use self::parse::Parser;
use self::session::{ResolveState, Session};
use self::source::Source;
use crate::template::diagnostic::{Diagnositic, DiagnositicBuilder, DiagnositicLevel};
use crate::variables::{UserVars, Variables};

#[derive(Debug, Clone)]
pub struct Template<'a> {
	session: Session<'a, ResolveState>,
	blocks: Vec<Block>,
}

impl<'a> Template<'a> {
	pub fn parse(source: Source<'a>) -> Result<Self> {
		let session = Session::new(source);

		Parser::new(session).parse()
	}

	// TODO: trim `\r\n` when span start/ends with it
	// TODO: return error sturct instead of emitting here
	pub fn resolve(
		mut self,
		profile_vars: Option<&UserVars>,
		dotfile_vars: Option<&UserVars>,
	) -> Result<String> {
		let mut output = String::new();

		for idx in 0..self.blocks.len() {
			if let Err(builder) =
				self.process_block(profile_vars, dotfile_vars, &mut output, &self.blocks[idx])
			{
				self.report_diagnostic(builder.build());
			}
		}

		self.session.emit();

		self.session.try_finish().map(|_| output)
	}

	fn report_diagnostic(&mut self, diagnostic: Diagnositic) {
		if diagnostic.level() == &DiagnositicLevel::Error {
			self.session.mark_failed();
		}

		self.session.report(diagnostic);
	}

	fn process_block(
		&self,
		profile_vars: Option<&UserVars>,
		dotfile_vars: Option<&UserVars>,
		output: &mut String,
		block: &Block,
	) -> Result<(), DiagnositicBuilder> {
		let Block { span, kind } = block;

		// TODO: trim `\r\n` when span start/ends with it
		match kind {
			BlockKind::Text => {
				output.push_str(&self.session.source[span]);
				Ok(())
			}
			BlockKind::Comment => {
				// NOP
				Ok(())
			}
			BlockKind::Escaped(inner) => {
				output.push_str(&self.session.source[inner]);
				Ok(())
			}
			BlockKind::Var(var) => {
				output.push_str(&self.resolve_var(var, profile_vars, dotfile_vars)?);
				Ok(())
			}
			BlockKind::Print(inner) => {
				log::info!("[Print] {}", &self.session.source[inner]);
				Ok(())
			}
			BlockKind::If(If {
				head,
				elifs,
				els,
				end: _,
			}) => {
				let (head, head_nested) = head;

				let matched = match self.resolve_if_expr(head.value(), profile_vars, dotfile_vars) {
					Ok(x) => x,
					Err(builder) => {
						return Err(
							builder.label_span(*head.span(), "while resolving this `if` block")
						)
					}
				};

				if matched {
					for block in head_nested {
						// TODO: if first block is text (trim lf start)
						// TODO: if last block is text (trim lf end)
						let _ = self.process_block(profile_vars, dotfile_vars, output, block)?;
					}
				} else {
					for (elif, elif_nested) in elifs {
						let matched =
							match self.resolve_if_expr(elif.value(), profile_vars, dotfile_vars) {
								Ok(x) => x,
								Err(builder) => {
									return Err(builder.label_span(
										*elif.span(),
										"while resolving this `elif` block",
									))
								}
							};

						if matched {
							// return if matching elif arm was found
							for block in elif_nested {
								let _ =
									self.process_block(profile_vars, dotfile_vars, output, block)?;
							}

							return Ok(());
						}
					}

					if let Some((_, els_nested)) = els {
						for block in els_nested {
							let _ =
								self.process_block(profile_vars, dotfile_vars, output, block)?;
						}
					}
				}
				Ok(())
			}
		}
	}

	fn resolve_if_expr(
		&self,
		expr: &IfExpr,
		profile_vars: Option<&UserVars>,
		dotfile_vars: Option<&UserVars>,
	) -> Result<bool, DiagnositicBuilder> {
		match expr {
			IfExpr::Compare { var, op, other } => {
				let var = self.resolve_var(var, profile_vars, dotfile_vars)?;
				Ok(op.eval(&var, &self.session.source[other]))
			}
			IfExpr::Exists { var } => Ok(self.resolve_var(var, profile_vars, dotfile_vars).is_ok()),
		}
	}

	fn resolve_var(
		&self,
		var: &Var,
		profile_vars: Option<&UserVars>,
		dotfile_vars: Option<&UserVars>,
	) -> Result<String, DiagnositicBuilder> {
		let name = &self.session.source[var.name];

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
				VarEnv::Dotfile => {
					if let Some(Some(val)) = dotfile_vars.map(|vars| vars.var(name)) {
						return Ok(val.to_string());
					}
				}
			};
		}

		Err(DiagnositicBuilder::new(DiagnositicLevel::Error)
			.message("failed to resolve variable")
			.description(format!(
				"no variable `{}` found in environments {}",
				name, var.envs
			))
			.primary_span(var.name))
	}
}

#[cfg(test)]
mod tests {
	use std::collections::HashMap;

	use super::*;

	#[test]
	fn parse_template() -> Result<()> {
		let _ = env_logger::Builder::from_env(
			env_logger::Env::default().default_filter_or(log::Level::Debug.as_str()),
		)
		.is_test(true)
		.try_init()?;

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

		let source = Source::anonymous(content);
		let template = Template::parse(source)?;

		// println!("{:#?}", template);

		let mut vars = HashMap::new();
		vars.insert(String::from("BUZZ"), String::from("Hello World"));
		vars.insert(String::from("OS"), String::from("linux"));
		let vars = UserVars { inner: vars };

		println!("{}", template.resolve(Some(&vars), Some(&vars))?);

		Ok(())
	}

	#[test]
	fn parse_template_vars() -> Result<()> {
		// Default
		let content = r#"{{OS}}"#;
		let source = Source::anonymous(content);
		let template = Template::parse(source)?;

		let profile_vars = UserVars::from_items(vec![("OS", "windows")]);
		let item_vars = UserVars::from_items(vec![("OS", "unix")]);
		std::env::set_var("OS", "macos");

		assert_eq!(
			template.resolve(Some(&profile_vars), Some(&item_vars))?,
			"unix"
		);

		// Profile
		let content = r#"{{#OS}}"#;
		let source = Source::anonymous(content);
		let template = Template::parse(source)?;

		let profile_vars = UserVars::from_items(vec![("OS", "windows")]);
		let item_vars = UserVars::from_items(vec![("OS", "unix")]);
		std::env::set_var("OS", "macos");

		assert_eq!(
			template.resolve(Some(&profile_vars), Some(&item_vars))?,
			"windows"
		);

		// Item
		let content = r#"{{&OS}}"#;
		let source = Source::anonymous(content);
		let template = Template::parse(source)?;

		let profile_vars = UserVars::from_items(vec![("OS", "windows")]);
		let item_vars = UserVars::from_items(vec![("OS", "unix")]);
		std::env::set_var("OS", "macos");

		assert_eq!(
			template.resolve(Some(&profile_vars), Some(&item_vars))?,
			"unix"
		);

		// Env
		let content = r#"{{$OS}}"#;
		let source = Source::anonymous(content);
		let template = Template::parse(source)?;

		let profile_vars = UserVars::from_items(vec![("OS", "windows")]);
		let item_vars = UserVars::from_items(vec![("OS", "unix")]);
		std::env::set_var("OS", "macos");

		assert_eq!(
			template.resolve(Some(&profile_vars), Some(&item_vars))?,
			"macos"
		);

		// Mixed - First
		let content = r#"{{$#OS}}"#;
		let source = Source::anonymous(content);
		let template = Template::parse(source)?;

		let profile_vars = UserVars::from_items(vec![("OS", "windows")]);
		let item_vars = UserVars::from_items(vec![("OS", "unix")]);
		std::env::set_var("OS", "macos");

		assert_eq!(
			template.resolve(Some(&profile_vars), Some(&item_vars))?,
			"macos"
		);

		// Mixed - Last
		let content = r#"{{$&OS}}"#;
		let source = Source::anonymous(content);
		let template = Template::parse(source)?;

		let profile_vars = UserVars::from_items(vec![("OS", "windows")]);
		let item_vars = UserVars::from_items(vec![("OS", "unix")]);
		std::env::remove_var("OS");

		assert_eq!(
			template.resolve(Some(&profile_vars), Some(&item_vars))?,
			"unix"
		);

		Ok(())
	}
}
