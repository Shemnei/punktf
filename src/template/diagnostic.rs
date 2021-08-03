use std::borrow::Cow;

use color_eyre::owo_colors::OwoColorize;

use super::span::ByteSpan;
use crate::template::source::Source;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagnosticSpan {
	/// A primary span is displayed with `^` bellow the spanned text.
	primary: Vec<ByteSpan>,

	/// A label is displayed as the spanned text together with the label.
	labels: Vec<(ByteSpan, Cow<'static, str>)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DiagnositicLevel {
	Error,
	Warning,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnositic {
	level: DiagnositicLevel,
	msg: Cow<'static, str>,
	span: Option<DiagnosticSpan>,
	description: Option<Cow<'static, str>>,
}

impl Diagnositic {
	pub fn new<M: Into<Cow<'static, str>>, D: Into<Option<impl Into<Cow<'static, str>>>>>(
		level: DiagnositicLevel,
		msg: M,
		span: Option<DiagnosticSpan>,
		description: D,
	) -> Self {
		Self {
			level,
			msg: msg.into(),
			span,
			description: description.into().map(|d| d.into()),
		}
	}

	pub fn emit(&self, source: &'_ Source<'_>) {
		// Rust check example:
		// error: 1 positional argument in format string, but no arguments were given
		//   --> src/template/source.rs:28:27
		//    |
		// 28 |         out.push_str(format!(" |{}", ));
		//    |                                 ^^

		// TODO-PM: move into separate into extra formatter
		let mut out = String::new();

		// title
		out.push_str(&format!("{}", self.msg.bold()));

		if let Some(span) = &self.span {
			for primary in &span.primary {
				out.push('\n');

				// location
				let loc = source.get_pos_location(*primary.low());
				let lpad = " ".repeat(loc.line().to_string().len());

				out.push_str(&format!(
					" {}{} {}:{}\n",
					lpad,
					"-->".bright_blue().bold(),
					source.origin(),
					loc
				));

				// highlight
				let line = source.get_pos_line(*primary.low());

				let vsep = "|".bright_blue();
				let vsep = vsep.bold();

				let loc_end = source.get_pos_location(*primary.high());
				let highlight_len = if loc.line() == loc_end.line() {
					// on same line; get diff
					loc_end.character() - loc.character()
				} else {
					// on different lines; get until end of line
					line.len() - loc.character()
				};

				let highlight = format!(
					"{}{}",
					" ".repeat(loc.character()),
					"^".repeat(highlight_len).bright_blue().bold()
				);

				out.push_str(&format!(" {} {}\n", lpad, vsep));
				out.push_str(&format!(
					" {} {} {}\n",
					loc.line().bright_blue().bold(),
					vsep,
					line
				));
				out.push_str(&format!(" {} {} {}", lpad, vsep, highlight));
			}
		}

		// description
		if let Some(description) = &self.description {
			out.push('\n');
			out.push_str(description);
		}

		match self.level {
			DiagnositicLevel::Error => log::error!("{}", out),
			DiagnositicLevel::Warning => log::warn!("{}", out),
		};
	}
}
