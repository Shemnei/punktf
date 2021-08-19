//! Everting related to user facing diagnostics a process may report.

use std::borrow::Cow;
use std::collections::btree_map::Entry;
use std::collections::{BTreeMap, HashSet};

use color_eyre::owo_colors::OwoColorize;

use super::source::Location;
use super::span::ByteSpan;
use crate::template::source::Source;

// COPYRIGHT by Rust project contributors
// <https://github.com/rust-lang/rust/graphs/contributors>
//
// Copied from <https://github.com/rust-lang/rust/blob/362e0f55eb1f36d279e5c4a58fb0fe5f9a2c579d/compiler/rustc_span/src/lib.rs#L474>.
/// Represents all spans related to one diagnostic.
///
/// The primary spans indicate where to "real" problem lies, while the labeled
/// spans are there for hints and extra information.
#[derive(Default, Debug, Clone, PartialEq, Eq)]
pub struct DiagnosticSpan {
	/// A primary span is displayed with `^` bellow the spanned text.
	pub(super) primary: Vec<ByteSpan>,

	/// A label is displayed with `-` bellow the spanned text together with the label.
	pub(super) labels: Vec<(ByteSpan, Cow<'static, str>)>,
}

/// The level of severity a diagnostic can have.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DiagnosticLevel {
	/// The diagnostic is an error.
	Error,

	/// The diagnostic is a warning.
	Warning,
}

// COPYRIGHT by Rust project contributors
// <https://github.com/rust-lang/rust/graphs/contributors>
//
// Copied from by <https://github.com/rust-lang/rust/blob/362e0f55eb1f36d279e5c4a58fb0fe5f9a2c579d/compiler/rustc_errors/src/diagnostic.rs#L15> with slight adaptations.
/// A diagnostic is something a task wants to communicate to the user.
///
/// It has a main message with a span, indicating which position in the source
/// code the diagnostic want to reference.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
	/// The severity level associated with this diagnostic.
	level: DiagnosticLevel,

	/// The main message this diagnostic is about.
	msg: Cow<'static, str>,

	/// The spans this diagnostic is about.
	span: Option<DiagnosticSpan>,

	/// An optional extensive description.
	description: Option<Cow<'static, str>>,
}

impl Diagnostic {
	/// Creates a new diagnostic.
	pub fn new<M: Into<Cow<'static, str>>, D: Into<Option<impl Into<Cow<'static, str>>>>>(
		level: DiagnosticLevel,
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

	/// Formats the diagnostic with [`DiagnosticFormatter`] and emits it with
	/// the crate [`log`].
	pub fn emit(&self, source: &'_ Source<'_>) {
		let mut fmt = DiagnosticFormatter::new(source, &self.msg);

		if let Some(span) = &self.span {
			for primary in &span.primary {
				fmt.primary_span(primary);
			}

			for (span, label) in &span.labels {
				fmt.label_span(span, label);
			}
		}

		if let Some(description) = &self.description {
			for line in description.lines() {
				fmt.description(line);
			}
		}

		let out = fmt.finish();

		match self.level {
			DiagnosticLevel::Error => {
				log::error!("{}{} {}", "error".bright_red().bold(), ':'.bold(), out)
			}
			DiagnosticLevel::Warning => log::warn!("{}", out),
		};
	}

	/// Returns the [`DiagnosticLevel`] associated with this diagnostic.
	pub const fn level(&self) -> &DiagnosticLevel {
		&self.level
	}
}

/// A builder for a [`Diagnostic`].
///
/// The advantage over directly constructing a diagnostic is the ability to
/// easily add spans to the builder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagnosticBuilder {
	/// The severity level associated with the diagnostic.
	level: DiagnosticLevel,

	/// The main message this diagnostic is about.
	msg: Cow<'static, str>,

	/// The spans this diagnostic is about.
	span: Option<DiagnosticSpan>,

	/// An optional extensive description.
	description: Option<Cow<'static, str>>,
}

impl DiagnosticBuilder {
	/// Creates a new diagnostic builder.
	pub const fn new(level: DiagnosticLevel) -> Self {
		Self {
			level,
			msg: Cow::Borrowed(""),
			span: None,
			description: None,
		}
	}

	/// Sets the diagnostic level on the builder.
	pub const fn level(mut self, level: DiagnosticLevel) -> Self {
		self.level = level;
		self
	}

	/// Sets the message on the builder.
	pub fn message<M: Into<Cow<'static, str>>>(mut self, msg: M) -> Self {
		self.msg = msg.into();
		self
	}

	/// Sets the description on the builder.
	pub fn description<D: Into<Option<impl Into<Cow<'static, str>>>>>(
		mut self,
		description: D,
	) -> Self {
		self.description = description.into().map(|d| d.into());
		self
	}

	/// Adds a primary span to the builder.
	pub fn primary_span(mut self, span: ByteSpan) -> Self {
		self.span.get_or_insert_default().primary.push(span);
		self
	}

	/// Adds a label span to the builder.
	pub fn label_span<L: Into<Cow<'static, str>>>(mut self, span: ByteSpan, label: L) -> Self {
		self.span
			.get_or_insert_default()
			.labels
			.push((span, label.into()));
		self
	}

	/// Consumes self and creates a diagnostic from it.
	// Destructors can not be run at compile time.
	#[allow(clippy::missing_const_for_fn)]
	pub fn build(self) -> Diagnostic {
		Diagnostic {
			level: self.level,
			msg: self.msg,
			span: self.span,
			description: self.description,
		}
	}
}

/// An reference to either a primary or label span.
///
/// The reference is just a index into a vector. This is used to keep track of
/// what spans are located on a line without having to copy/clone them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum SpanRef {
	Primary(usize),
	Label(usize),
}

/// This struct holds all lines necessary to resolve/format a diagnostic.
///
/// It also holds additional information like:
///
/// - For each line which span is located on it
#[derive(Debug, Clone, PartialEq, Eq)]
struct LineMap<'a> {
	/// The source from which to resolve the lines needed for format a
	/// diagnostic.
	source: &'a Source<'a>,

	/// All lines necessary for formatting the diagnostic.
	///
	/// The lines are saved here to avoid multiple lookups to the same line.
	/// The `key` is the zero-indexed line number/index.
	lines: BTreeMap<usize, &'a str>,

	/// This maps all spans of a diagnostic to the lines they occur on.
	///
	/// A span can be on multiple lines, that's why the "cheap" [`SpanRef`] is
	/// used here instead of cloning/coping the span multiple times.
	line_spans: BTreeMap<usize, HashSet<SpanRef>>,

	/// All primary span of a diagnostic, resolved to a start and end
	/// [location](`super::source::Location`).
	primary_spans: Vec<(Location, Location)>,

	/// All label span of a diagnostic, resolved to a start and end
	/// [location](`super::source::Location`).
	label_spans: Vec<(Location, Location, &'a str)>,
}

impl<'a> LineMap<'a> {
	/// Creates a new line map for the given `source`.
	pub fn new(source: &'a Source<'a>) -> Self {
		let (lines, line_spans, primary_spans, label_spans) = <_>::default();

		Self {
			source,
			lines,
			line_spans,
			primary_spans,
			label_spans,
		}
	}

	/// Adds a primary span to the line map.
	///
	/// This will also intern all lines the `span` spans if it is not already
	/// present.
	pub fn insert_primary(&mut self, span: &ByteSpan) -> SpanRef {
		// get index of next vacant entry
		let span_ref = SpanRef::Primary(self.primary_spans.len());
		self.primary_spans.push(self.locations(span));

		self.intern_span(span, span_ref);

		span_ref
	}

	/// Adds a label span to the line map.
	///
	/// This will also intern all lines the `span` spans if it is not already
	/// present.
	pub fn insert_label(&mut self, span: &ByteSpan, label: &'a str) -> SpanRef {
		// get index of next vacant entry
		let span_ref = SpanRef::Label(self.label_spans.len());
		let (loc_low, loc_high) = self.locations(span);
		self.label_spans.push((loc_low, loc_high, label));

		self.intern_span(span, span_ref);

		span_ref
	}

	/// Returns the lowest one-indexed line number this line map has interned.
	pub fn min_line_nr(&self) -> Option<usize> {
		self.lines.first_key_value().map(|(idx, _)| idx + 1)
	}

	/// Returns the highest one-indexed line number this line map has interned.
	pub fn max_line_nr(&self) -> Option<usize> {
		self.lines.last_key_value().map(|(idx, _)| idx + 1)
	}

	/// Returns the smallest (location)[`super::source::Location`] any span
	/// within this struct has.
	///
	/// The search is done in this order:
	///
	/// 1) First searches the primary spans for the smallest location and
	///       returns any if found.
	/// 2) After that it searches the label spans for the smallest location and
	///       returns any if found.
	pub fn min_location(&self) -> Option<Location> {
		self.primary_spans
			.iter()
			.map(|(low, _)| low)
			.min()
			.or_else(|| self.label_spans.iter().map(|(low, _, _)| low).min())
			.copied()
	}

	/// Returns a vector containing all one-indexed line numbers interned by
	/// this line map.
	///
	/// The line numbers are sorted from low to high.
	pub fn line_nrs(&self) -> Vec<usize> {
		self.lines.keys().copied().map(|nr| nr + 1).collect()
	}

	/// Returns the contents of a line for a given one-indexed line number.
	pub fn line(&self, line_nr: usize) -> Option<&str> {
		self.lines.get(&(line_nr - 1)).copied()
	}

	/// Returns an iterator over all spans (primary and labeled) located on the
	/// one-indexed line number.
	///
	/// The spans are sorted by their start location, low to high.
	pub fn line_spans_sorted(
		&self,
		line_nr: usize,
	) -> Option<Vec<(Location, Location, Option<&'a str>)>> {
		let refs_iter = self.line_spans.get(&(line_nr - 1))?;

		let mut items = refs_iter
			.iter()
			.map(|&span_ref| match span_ref {
				SpanRef::Primary(idx) => {
					let (low, high) = self.primary_spans[idx];
					(low, high, None)
				}
				SpanRef::Label(idx) => {
					let (low, high, label) = self.label_spans[idx];
					(low, high, Some(label))
				}
			})
			.collect::<Vec<_>>();

		items.sort_by_key(|item| item.0);

		Some(items)
	}

	/// Resolves a span to a start and end [location][`super::source::Location`].
	fn locations(&self, span: &ByteSpan) -> (Location, Location) {
		(
			self.source.get_pos_location(span.low),
			self.source.get_pos_location(span.high),
		)
	}

	/// Interns all lines this `span` spans.
	///
	/// If the line is already present, it is skipped.
	fn intern_span(&mut self, span: &ByteSpan, span_ref: SpanRef) {
		let line_idx_low = self.source.get_pos_line_idx(span.low);
		let line_idx_high = self.source.get_pos_line_idx(span.high);

		for line_idx in line_idx_low..=line_idx_high {
			self.intern_line(line_idx, span_ref);
		}
	}

	/// Interns a line which is located at the zero-indexed `line_idx` if it is
	/// not already present.
	///
	/// It also adds `span_ref` to the reference [`LineMap::line_spans`] keeps
	/// for each line.
	fn intern_line(&mut self, line_idx: usize, span_ref: SpanRef) {
		if let Entry::Vacant(e) = self.lines.entry(line_idx) {
			e.insert(self.source.get_idx_line(line_idx));
		}

		// add span ref to line where it occured
		self.line_spans
			.entry(line_idx)
			.or_default()
			.insert(span_ref);
	}
}

/// This struct is responsible for formatting a [`Diagnostic`].
pub struct DiagnosticFormatter<'a> {
	/// The source the diagnostic references.
	source: &'a Source<'a>,

	/// The message from the diagnostic.
	msg: &'a str,

	/// The description from the diagnostic.
	descriptions: Vec<&'a str>,

	/// All lines referenced by the diagnostic.
	line_map: LineMap<'a>,
}

impl<'a> DiagnosticFormatter<'a> {
	/// Creates a new formatter for the given `source` and primary `msg`
	/// (related: [`Diagnostic::msg`]).
	pub fn new(source: &'a Source<'a>, msg: &'a str) -> Self {
		let descriptions = <_>::default();

		Self {
			source,
			msg,
			descriptions,
			line_map: LineMap::new(source),
		}
	}

	/// Adds a description to the formatter (related: [`Diagnostic::description`]).
	pub fn description(&mut self, description: &'a str) -> &mut Self {
		self.descriptions.push(description);
		self
	}

	/// Adds primary span to the formatter (related: [`DiagnosticSpan::primary`]).
	pub fn primary_span(&mut self, span: &ByteSpan) -> &mut Self {
		let _ = self.line_map.insert_primary(span);
		self
	}

	/// Adds label span to the formatter (related: [`DiagnosticSpan::label`]).
	pub fn label_span(&mut self, span: &ByteSpan, label: &'a str) -> &mut Self {
		let _ = self.line_map.insert_label(span, label);
		self
	}

	/// Consumes self and formats all attributes set on this formatter into a
	/// string.
	pub fn finish(self) -> String {
		// Rust check example:
		// error: 1 positional argument in format string, but no arguments were given
		//   --> src/template/source.rs:28:27
		//    |
		// 28 |         out.push_str(format!(" |{}", ));
		//    |                                 ^^

		fn style<S: AsRef<str>>(s: S) -> String {
			s.as_ref().bright_blue().bold().to_string()
		}

		let separator = style("|");

		let mut out = String::new();
		let mut left_pad = String::from("");

		out.push_str(&self.msg.bold().to_string());

		// check if there are spans to format
		if let Some(line_nr) = self.line_map.max_line_nr() {
			left_pad = " ".repeat(line_nr.to_string().len());

			// add file information
			out.push_str(&format!(
				"\n {}{} {}",
				left_pad,
				style("-->"),
				self.source.origin(),
			));
			if let Some(min_loc) = self.line_map.min_location() {
				out.push_str(&format!(":{}", min_loc.display()));
			}

			// add code lines and spans
			out.push_str(&format!("\n {} {}", left_pad, separator));

			let mut last_line_nr: Option<usize> = None;
			for line_nr in self.line_map.line_nrs() {
				if let Some(line) = self.line_map.line(line_nr) {
					let line_nr_str = line_nr.to_string();

					if matches!(last_line_nr, Some(lln) if (line_nr - lln) > 1) {
						out.push_str(&format!("\n {}", style("...")));
					}

					out.push_str(&format!(
						"\n {}{} {} {}",
						&left_pad[line_nr_str.len()..],
						line_nr,
						separator,
						line.replace('\t', "    ")
					));

					if let Some(spans) = self.line_map.line_spans_sorted(line_nr) {
						for (low_loc, high_loc, label) in spans {
							let low_cpos = if low_loc.line() != line_nr {
								0
							} else {
								low_loc.column()
							};

							let (ends_on_line, high_cpos) = if high_loc.line() != line_nr {
								(false, line.len())
							} else {
								(true, high_loc.column())
							};

							let highlight_left_pad = " ".repeat(low_cpos);

							let highlight = if label.is_some() { "-" } else { "^" }
								.repeat(high_cpos - low_cpos);

							out.push_str(&format!(
								"\n {} {} {}{}",
								&left_pad,
								separator,
								highlight_left_pad,
								style(highlight)
							));

							if ends_on_line {
								if let Some(label) = label {
									out.push(' ');
									out.push_str(&style(label));
								}
							}
						}
					}

					last_line_nr = Some(line_nr);
				}
			}

			out.push_str(&format!("\n {} {}", left_pad, separator));
		}

		for description in self.descriptions {
			out.push_str(&format!("\n {} {} {}", left_pad, style("="), description));
		}

		out
	}
}
