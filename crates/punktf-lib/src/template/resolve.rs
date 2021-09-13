//! This module contains everything needed to resolve a
//! [template](`super::Template`) to a string.  This includes filling of
//! variable blocks and evaluation of if blocks.

use std::borrow::Cow;
use std::ops::Deref;

use color_eyre::eyre::Result;

use super::block::{Block, BlockKind, If, IfExpr, Var, VarEnv};
use super::session::Session;
use super::Template;
use crate::template::diagnostic::{Diagnostic, DiagnosticBuilder, DiagnosticLevel};
use crate::variables::Vars;

/// This macro resolves to the target architecture string of the compiling
/// system. All possible values can be found here
/// <https://doc.rust-lang.org/reference/conditional-compilation.html#target_arch>.
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

/// This macro resolves to the target operating system string of the compiling
/// system. All possible values can be found here
/// <https://doc.rust-lang.org/reference/conditional-compilation.html#target_os>.
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

/// This macro resolves to the target family string of the compiling
/// system. All possible values can be found here
/// <https://doc.rust-lang.org/reference/conditional-compilation.html#target_family>.
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

/// The resolver is responsible for evaluating and filling a
/// [template](`super::Template`). During the filling all found errors are
/// recorded in the [session](`super::session::Session`) and emitted after the
/// [resolve](`Resolver::resolve`) process.
pub struct Resolver<'a, PV, DV> {
	/// Template to resolve.
	template: &'a Template<'a>,

	/// Variables defined in the profile.
	profile_vars: Option<&'a PV>,

	/// Variables defined by the [dotfile](`crate::Dotfile`) which corresponds to the template.
	dotfile_vars: Option<&'a DV>,

	/// Session where all errors/diagnostic which occur during the resolving
	/// process are recorded to.
	session: Session,

	/// Flag that when it is set prevents a leading new line of a text block to
	/// be emitted.
	///
	/// This is implemented to avoid extra empty lines which could be created
	/// by either a `comment`, `print` or a not taken `if` block.
	should_skip_next_newline: bool,
}

impl<'a, PV, DV> Resolver<'a, PV, DV>
where
	PV: Vars,
	DV: Vars,
{
	/// Creates a new resolver for `template` with the given `profile_vars` and
	/// `dotfile_vars`.
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
			should_skip_next_newline: false,
		}
	}

	/// Consumes the resolver and tries to resolve all blocks defined by the
	/// template.
	///
	/// # Errors
	///
	/// An error is returned if a variable could not be resolved.
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

	/// Adds a diagnostic to the session.
	fn report_diagnostic(&mut self, diagnostic: Diagnostic) {
		if diagnostic.level() == &DiagnosticLevel::Error {
			self.session.mark_failed();
		}

		self.session.report(diagnostic);
	}

	/// Processes a [block](`super::block::Block`) and appends the resolved
	/// output to `output`.
	///
	/// # Errors
	///
	/// An error is returned if a variable could not be resolved.
	fn process_block(
		&mut self,
		output: &mut String,
		block: &Block,
	) -> Result<(), DiagnosticBuilder> {
		let Block { span, kind } = block;

		// NOTE: `self.should_skip_next_newline` must be reset on every match
		// branch which is not a text block to avoid remembering it even though
		// there already was a new non text block between the setting block and
		// an eventual text block.
		match kind {
			BlockKind::Text => {
				let mut content = &self.template.source[span];

				// If last block was a block which inserted no content and
				// started at the beginning of a new line THEN strip a leading
				// `\n` from the content.
				// (related #64)
				if self.should_skip_next_newline
					&& matches!(content.as_bytes(), &[b'\n', ..] | &[b'\r', b'\n', ..])
				{
					// In the if expression above we already checked that there
					// is an new line character. So if the find fails we messed
					// something up or the std lib has an error.
					let lf_idx = content
						.find('\n')
						.expect("Failed to find new line character");

					content = &content[lf_idx + 1..];

					self.should_skip_next_newline = false;
				}

				output.push_str(content);
			}
			BlockKind::Comment => {
				// Should skip new line if started at the beginning of a line.
				// As a `print` block has no final `content` is the above the
				// only condition.
				self.should_skip_next_newline =
					self.template.source.get_pos_location(span.low).column() == 0;

				// NOP
			}
			BlockKind::Escaped(inner) => {
				let content = &self.template.source[inner];

				// Should skip new line if started at the beginning of a line.
				// As a `print` block has no final `content` is the above the
				// only condition.
				self.should_skip_next_newline = content.is_empty()
					&& self.template.source.get_pos_location(span.low).column() == 0;

				output.push_str(content);
			}
			BlockKind::Var(var) => {
				self.should_skip_next_newline = false;

				output.push_str(&self.resolve_var(var)?);
			}
			BlockKind::Print(inner) => {
				// Should skip new line if started at the beginning of a line.
				// As a `print` block has no final `content` is the above the
				// only condition.
				self.should_skip_next_newline =
					self.template.source.get_pos_location(span.low).column() == 0;

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

				// Should skip new line if started at the beginning of a line
				// and no new content was added.
				self.should_skip_next_newline = if_output_prepared.is_empty()
					&& self.template.source.get_pos_location(span.low).column() == 0;

				output.push_str(if_output_prepared);
			}
		};

		Ok(())
	}

	/// Tries to resolve an [if expression](`super::block::IfExpr`) and returns
	/// the result of the evaluated expression.
	///
	/// # Errors
	///
	/// An error is returned if a variable could not be resolved.
	fn resolve_if_expr(&self, expr: &IfExpr) -> Result<bool, DiagnosticBuilder> {
		match expr {
			IfExpr::Compare { var, op, other } => {
				let var = self.resolve_var(var)?;
				Ok(op.eval(&var, &self.template.source[other]))
			}
			IfExpr::Exists { var } => Ok(self.resolve_var(var).is_ok()),
			IfExpr::NotExists { var } => Ok(self.resolve_var(var).is_err()),
		}
	}

	/// Tries to resolve a [variable](`super::block::Var`) by looking for the
	/// value in [`Resolver::profile_vars`], [`Resolver::dotfile_vars`] and the
	/// system environment.
	///
	/// This function injects the following environment
	/// variables if not present:
	///
	/// - `PUNKTF_TARGET_ARCH`: Architecture of the compiling system
	/// - `PUNKTF_TARGET_OS`: Operating system of the compiling system
	/// - `PUNKTF_TARGET_FAMILY`: Operating system family of the compiling system
	///
	/// # Errors
	///
	/// An error is returned if the variable could not be resolved.
	fn resolve_var(&self, var: &Var) -> Result<Cow<'_, str>, DiagnosticBuilder> {
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
						(_, Ok(val)) => return Ok(Cow::Owned(val)),
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

		Err(DiagnosticBuilder::new(DiagnosticLevel::Error)
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
	use crate::variables::Variables;

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
		),
		(
			r#"Hello

{{@if {{NAME}}}}
	{{NAME}}
{{@else}}
	there
{{@fi}}

!"#,
			"Hello\n\n\tthere\n\n!"
		),
		(
			r#"Hello
{{@if {{NAME}}}}

	{{NAME}}
{{@else}}

	there
{{@fi}}
!"#,
			"Hello\n\n\tthere\n!"
		),
		(
			r#"{{@if !{{OS}}}}
Hello World
{{@fi}}

Hello
"#,
			r#"Hello World

Hello
"#
		),
		// BUG #64: https://github.com/Shemnei/punktf/issues/64
		(
			r#"{{@if {{OS}}}} Hello World {{@fi}}
Hello
"#,
			r#"Hello
"#
		),
		(
			r#"{{@if {{OS}}}}
	Hello World
{{@fi}}
Hello
"#,
			r#"Hello
"#
		),
		(
			r#"{{@if {{OS}}}}
	Hello World
{{@fi}}

Hello
"#,
			r#"
Hello
"#
		),
		(
			r#"Hello

{{@if {{OS}}}}
	Hello World
{{@fi}}

World"#,
			r#"Hello


World"#
		),
		(
			r#"{{@print Hello World}}

Hello
"#,
			r#"
Hello
"#
		),
		(
			r#"Hello
{{@print Hello World}}
World"#,
			r#"Hello
World"#
		),
		(
			r#"Hello {{@print Hello World}}World"#,
			r#"Hello World"#
		),
		(
			r#"{{@if {{OS}}}}
	Hello World
{{@fi}}

{{DEMO_VAR}}
"#,
			r#"
DEMO
"#
		),
		(
			r#"Hello
{{{}}}
World"#,
			r#"Hello
World"#
		),
		(
			r#"Hello
{{!-- Comment --}}
World"#,
			r#"Hello
World"#
		),
	];

	#[test]
	fn if_fmt() -> Result<()> {
		crate::tests::setup_test_env();

		let vars = Variables::from_items([("DEMO_VAR", "DEMO")]);

		for (content, should) in IF_FMT_TEST_CASES {
			let source = Source::anonymous(content);
			let template = Template::parse(source)?;

			assert_eq!(
				&template.resolve::<Variables, Variables>(Some(&vars), None)?,
				should,
				"Format test failed for input `{}`",
				content.escape_debug()
			);
		}

		Ok(())
	}
}
