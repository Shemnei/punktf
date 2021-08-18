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
mod resolve;
mod session;
pub mod source;
mod span;

use color_eyre::eyre::Result;

use self::block::Block;
use self::parse::Parser;
use self::resolve::Resolver;
use self::source::Source;
use crate::variables::Variables;

#[derive(Debug, Clone)]
pub struct Template<'a> {
	source: Source<'a>,
	blocks: Vec<Block>,
}

impl<'a> Template<'a> {
	pub fn parse(source: Source<'a>) -> Result<Self> {
		Parser::new(source).parse()
	}

	pub fn resolve<PV: Variables, DV: Variables>(
		&self,
		profile_vars: Option<&PV>,
		dotfile_vars: Option<&DV>,
	) -> Result<String> {
		Resolver::new(self, profile_vars, dotfile_vars).resolve()
	}
}

#[cfg(test)]
mod tests {
	use std::collections::HashMap;

	use super::*;
	use crate::variables::UserVars;

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
