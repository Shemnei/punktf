//! Everthing related to parsing/resolving templates is located in this module or it's submodules.
//!
//! # Syntax
//!
//! The syntax is heavily inspired by <https://handlebarsjs.com/>.
//!
//! ## Comment blocks
//!
//! Document template blocks. Will not be copied over to final output.
//!
//! ### Syntax
//!
//! `{{!-- This is a comment --}}`
//!
//! ## Escape blocks
//!
//! Everything inside will be copied over as is. It can be used to  copied over `{{` or `}}` without it being interpreted as a template block.
//!
//! ### Syntax
//!
//! `{{{ This will be copied over {{ as is }} even with the "{{" inside }}}`
//!
//! ## Variable blocks
//!
//! Define a variable which will be inserted instead of the block. The value of the variable can be gotten from three different environments which can be defined by specifying a prefix:
//!
//! 1) `$`: System environment
//! 2) `#`: Variables defined in the `profile` section
//! 2) `&`: Variables defined in the `profile.dotfile` section
//!
//! To search in more than one environment, these prefixes can be combined. The order they appear in is important, as they will be searched in order of appearance. If one environment does not have a value set for the variable, the next one is searched.
//!
//! If no prefixes are defined, it will default to `&#`.
//!
//! Valid symbols/characters for a variable name are: `(a..z|A..Z|0-9|_)`
//!
//! ### Syntax
//!
//! `{{$&#OS}}`
//!
//! ## Print blocks
//!
//! Print blocks will simply print everything contained within the block to the command line. The content of the print block **won't** be resolved, meaning it will be printed 1 to 1 (e.g. no variables are resolved).
//!
//! ### Syntax
//!
//! `{{@print Hello World}}`
//!
//! ## If blocks
//!
//! Supported are `if`, `elif`, `else` and `fi`. Each `if` block must have a `fi` block as a final closing block.
//! In between the `if` and `fi` block can be zero or multiple `elif` blocks with a final optional `else` block.
//! Each if related block must be prefixed with `{{@` and end with `}}`.
//!
//! Currently the only supported if syntax is:
//!
//! - Check if the value of a variable is (not) equal to the literal given: `{{VAR}} (==|!=) "LITERAL"`
//! - Check if a value for a variable exists: `{{VAR}}`
//!
//! Other blocks can be nested inside the `if`, `elif` and `else` bodies.
//!
//! ### Syntax
//!
//! ```text
//! {{@if {{OS}}}}
//!         {{@if {{&OS}} != "windows"}}
//!             print("OS is not windows")
//!         {{@elif {{OS}} == "windows"}}
//!             {{{!-- This is a nested comment. Below it is a nested variable block. --}}}
//!             print("OS is {{OS}}")
//!         {{@else}}
//!             {{{!-- This is a nested comment. --}}}
//!             print("Can never get here. {{{ {{OS}} is neither `windows` nor not `windows`. }}}")
//!         {{@fi}}
//! {{@else}}
//!     print("No value for variable `OS` set")
//! {{@fi}}
//! ```
//!
//! # Copyright Notice
//!
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

/// A `Template` is a file from the Source folder that is not yet deployed. It might contain statements and variables.
#[derive(Debug, Clone)]
pub struct Template<'a> {
	/// The source from which the template was parsed from.
	source: Source<'a>,

	/// All parsed blocks contained in `source`.
	///
	/// These are sorted in the order they occur in `source`.
	blocks: Vec<Block>,
}

impl<'a> Template<'a> {
	/// Parses the source file and returns a `Template` object.
	pub fn parse(source: Source<'a>) -> Result<Self> {
		Parser::new(source).parse()
	}

	/// Resolves the variables in the template and returns a `Template` object.
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
