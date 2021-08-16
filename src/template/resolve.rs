use std::ops::Deref;

use color_eyre::eyre::Result;

use super::block::{Block, BlockKind, If, IfExpr, Var, VarEnv};
use super::session::Session;
use super::Template;
use crate::template::diagnostic::{Diagnositic, DiagnositicBuilder, DiagnositicLevel};
use crate::variables::Variables;

macro_rules! arch {
	() => {{
		cfg_if::cfg_if! {
			if #[cfg(target_arch = "x86")] {
				"x86"
			} else if #[cfg(target_arch = "x86_64")] {
				"x86_64"
			} else if #[cfg(target_arch = "mips")] {
				"mips"
			} else if #[cfg(target_arch = "powerpc")] {
				"powerpc"
			} else if #[cfg(target_arch = "powerpc64")] {
				"powerpc64"
			} else if #[cfg(target_arch = "arm")] {
				"arm"
			} else if #[cfg(target_arch = "aarch64")] {
				"aarch64"
			} else {
				"unknown"
			}
		}
	}};
}

macro_rules! os {
	() => {{
		cfg_if::cfg_if! {
			if #[cfg(target_os = "windows")] {
				"windows"
			} else if #[cfg(target_os = "macos")] {
				"macos"
			} else if #[cfg(target_os = "ios")] {
				"ios"
			} else if #[cfg(target_os = "linux")] {
				"linux"
			} else if #[cfg(target_os = "android")] {
				"android"
			} else if #[cfg(target_os = "freebsd")] {
				"freebsd"
			} else if #[cfg(target_os = "dragonfly")] {
				"dragonfly"
			} else if #[cfg(target_os = "openbsd")] {
				"openbsd"
			} else if #[cfg(target_os = "netbsd")] {
				"netbsd"
			} else {
				"unknown"
			}
		}
	}};
}

macro_rules! family {
	() => {{
		cfg_if::cfg_if! {
			if #[cfg(target_family = "unix")] {
				"unix"
			} else if #[cfg(target_os = "windows")] {
				"windows"
			} else if #[cfg(target_os = "wasm")] {
				"wasm"
			} else {
				"unknown"
			}
		}
	}};
}

pub struct Resolver<'a, PV, DV> {
	template: &'a Template<'a>,
	profile_vars: Option<&'a PV>,
	dotfile_vars: Option<&'a DV>,
	session: Session,
}

impl<'a, PV, DV> Resolver<'a, PV, DV>
where
	PV: Variables,
	DV: Variables,
{
	pub fn new(
		template: &'a Template<'a>,
		profile_vars: Option<&'a PV>,
		dotfile_vars: Option<&'a DV>,
	) -> Self {
		Self {
			template,
			profile_vars,
			dotfile_vars,
			session: Session::new(),
		}
	}

	pub fn resolve(mut self) -> Result<String> {
		let mut output = String::new();

		for block in &self.template.blocks {
			if let Err(builder) = self.process_block(&mut output, block) {
				self.report_diagnostic(builder.build());
			}
		}

		self.session.emit(&self.template.source);

		let Resolver { session, .. } = self;

		session.try_finish().map(|_| output)
	}

	fn report_diagnostic(&mut self, diagnostic: Diagnositic) {
		if diagnostic.level() == &DiagnositicLevel::Error {
			self.session.mark_failed();
		}

		self.session.report(diagnostic);
	}

	fn process_block(
		&mut self,
		output: &mut String,
		block: &Block,
	) -> Result<(), DiagnositicBuilder> {
		let Block { span, kind } = block;

		match kind {
			BlockKind::Text => {
				output.push_str(&self.template.source[span]);
			}
			BlockKind::Comment => {
				// NOP
			}
			BlockKind::Escaped(inner) => {
				output.push_str(&self.template.source[inner]);
			}
			BlockKind::Var(var) => {
				output.push_str(&self.resolve_var(var)?);
			}
			BlockKind::Print(inner) => {
				log::info!("Print: {}", &self.template.source[inner]);
			}
			BlockKind::If(If {
				head,
				elifs,
				els,
				end: _,
			}) => {
				let mut if_output = String::new();

				let (head, head_nested) = head;

				let matched = match self.resolve_if_expr(head.value()) {
					Ok(x) => x,
					Err(builder) => {
						return Err(
							builder.label_span(*head.span(), "while resolving this `if` block")
						)
					}
				};

				if matched {
					for block in head_nested {
						let _ = self.process_block(&mut if_output, block)?;
					}
				} else {
					let mut found_elif = false;
					for (elif, elif_nested) in elifs {
						let matched = match self.resolve_if_expr(elif.value()) {
							Ok(x) => x,
							Err(builder) => {
								return Err(builder
									.label_span(*elif.span(), "while resolving this `elif` block"))
							}
						};

						if matched {
							found_elif = true;

							for block in elif_nested {
								let _ = self.process_block(&mut if_output, block)?;
							}

							break;
						}
					}

					if !found_elif {
						if let Some((_, els_nested)) = els {
							for block in els_nested {
								let _ = self.process_block(&mut if_output, block)?;
							}
						}
					}
				}

				let mut if_output_prepared = if_output.deref();

				// Check if characters before first line feed are all considered
				// to be white spaces and if so omit them from the output.
				if let Some(idx) = if_output_prepared.find('\n') {
					// include line feed
					if if_output_prepared[..idx].trim_start().is_empty() {
						// Also trim line feed
						if_output_prepared = &if_output_prepared[idx + 1..];
					}
				}

				// Check if characters after last line feed are all considered
				// to be white spaces and if so omit them from the output.
				if let Some(idx) = if_output_prepared.rfind('\n') {
					// include line feed
					if if_output_prepared[idx..].trim_start().is_empty() {
						if_output_prepared = &if_output_prepared[..idx];
					}
				}

				output.push_str(if_output_prepared);
			}
		};

		Ok(())
	}

	fn resolve_if_expr(&self, expr: &IfExpr) -> Result<bool, DiagnositicBuilder> {
		match expr {
			IfExpr::Compare { var, op, other } => {
				let var = self.resolve_var(var)?;
				Ok(op.eval(&var, &self.template.source[other]))
			}
			IfExpr::Exists { var } => Ok(self.resolve_var(var).is_ok()),
		}
	}

	fn resolve_var(&self, var: &Var) -> Result<String, DiagnositicBuilder> {
		let name = &self.template.source[var.name];

		for env in var.envs.envs() {
			match env {
				VarEnv::Environment => {
					match (name, std::env::var(name)) {
						("PUNKTF_TARGET_ARCH", Err(std::env::VarError::NotPresent)) => {
							return Ok(arch!().into())
						}
						("PUNKTF_TARGET_OS", Err(std::env::VarError::NotPresent)) => {
							return Ok(os!().into())
						}
						("PUNKTF_TARGET_FAMILY", Err(std::env::VarError::NotPresent)) => {
							return Ok(family!().into())
						}
						(_, Ok(val)) => return Ok(val),
						(_, Err(_)) => continue,
					};
				}
				VarEnv::Profile => {
					if let Some(Some(val)) = self.profile_vars.map(|vars| vars.var(name)) {
						return Ok(val.into());
					}
				}
				VarEnv::Dotfile => {
					if let Some(Some(val)) = self.dotfile_vars.map(|vars| vars.var(name)) {
						return Ok(val.into());
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
	use pretty_assertions::assert_eq;

	use super::*;
	use crate::template::source::Source;
	use crate::template::Template;
	use crate::variables::UserVars;

	#[rustfmt::skip]
	const IF_FMT_TEST_CASES: &[(&str, &str)] = &[
		(
			r#"Hello {{@if {{NAME}}}}{{NAME}}{{@else}}there{{@fi}} !"#,
			r#"Hello there !"#
		),
	(
			r#"Hello {{@if {{NAME}}}}
{{NAME}}
{{@else}}
there
{{@fi}} !"#,
			r#"Hello there !"#
		),
		(
			r#"Hello {{@if {{NAME}}}}
{{NAME}}
{{@else}}
there
{{@fi}}
!"#,
			"Hello there\n!"
		),
		(
			r#"Hello
{{@if {{NAME}}}}
{{NAME}}
{{@else}}
there
{{@fi}}
!"#,
			"Hello\nthere\n!"
		),
		(
			r#"Hello
{{@if {{NAME}}}}
	{{NAME}}
{{@else}}
	there
{{@fi}}
!"#,
			"Hello\n\tthere\n!"
		)
	];

	#[test]
	fn if_fmt() -> Result<()> {
		for (content, should) in IF_FMT_TEST_CASES {
			let source = Source::anonymous(content);
			let template = Template::parse(source)?;

			assert_eq!(&template.resolve::<UserVars, UserVars>(None, None)?, should);
		}

		Ok(())
	}
}
