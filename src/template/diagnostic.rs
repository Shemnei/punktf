//! The code for error/diagnostics handling is heavily inspiered by
//! [rust's](https://github.com/rust-lang/rust) compiler. While some code is adpated for use with
//! punktf, some of it is also a plain copy of it.
//!
//! Specifically from those files:
//! - https://github.com/rust-lang/rust/blob/master/compiler/rustc_span/src/lib.rs
//! - https://github.com/rust-lang/rust/blob/master/compiler/rustc_span/src/analyze_source_file.rs
//! - https://github.com/rust-lang/rust/blob/master/compiler/rustc_parse/src/parser/diagnostics.rs
//! - https://github.com/rust-lang/rust/blob/master/compiler/rustc_errors/src/diagnostic.rs
//! - https://github.com/rust-lang/rust/blob/master/compiler/rustc_errors/src/diagnostic_builder.rs
//! - https://github.com/rust-lang/rust/blob/master/compiler/rustc_errors/src/emitter.rs

use std::borrow::Cow;
use std::collections::btree_map::Entry;
use std::collections::{BTreeMap, HashSet};

use color_eyre::owo_colors::OwoColorize;

use super::source::Location;
use super::span::ByteSpan;
use crate::template::source::Source;

#[derive(Default, Debug, Clone, PartialEq, Eq)]
pub struct DiagnosticSpan {
	/// A primary span is displayed with `^` bellow the spanned text.
	pub(super) primary: Vec<ByteSpan>,

	/// A label is displayed as the spanned text together with the label.
	pub(super) labels: Vec<(ByteSpan, Cow<'static, str>)>,
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
		let mut fmt = DiagnositicFormatter::new(source, &self.msg);

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
			DiagnositicLevel::Error => {
				log::error!("{}{} {}", "error".bright_red().bold(), ':'.bold(), out)
			}
			DiagnositicLevel::Warning => log::warn!("{}", out),
		};
	}

	pub fn level(&self) -> &DiagnositicLevel {
		&self.level
	}
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagnositicBuilder {
	level: DiagnositicLevel,
	msg: Cow<'static, str>,
	span: Option<DiagnosticSpan>,
	description: Option<Cow<'static, str>>,
}

impl DiagnositicBuilder {
	pub fn new(level: DiagnositicLevel) -> Self {
		Self {
			level,
			msg: Cow::Borrowed(""),
			span: None,
			description: None,
		}
	}

	pub fn level(mut self, level: DiagnositicLevel) -> Self {
		self.level = level;
		self
	}

	pub fn message<M: Into<Cow<'static, str>>>(mut self, msg: M) -> Self {
		self.msg = msg.into();
		self
	}

	pub fn description<D: Into<Option<impl Into<Cow<'static, str>>>>>(
		mut self,
		description: D,
	) -> Self {
		self.description = description.into().map(|d| d.into());
		self
	}

	pub fn primary_span(mut self, span: ByteSpan) -> Self {
		self.span.get_or_insert_default().primary.push(span);
		self
	}

	pub fn label_span<L: Into<Cow<'static, str>>>(mut self, span: ByteSpan, label: L) -> Self {
		self.span
			.get_or_insert_default()
			.labels
			.push((span, label.into()));
		self
	}

	#[must_use]
	pub fn build(self) -> Diagnositic {
		Diagnositic {
			level: self.level,
			msg: self.msg,
			span: self.span,
			description: self.description,
		}
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum SpanRef {
	Primary(usize),
	Label(usize),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LineMap<'a> {
	source: &'a Source<'a>,

	// the lines are stored as line index (meaning 0 indexed)
	lines: BTreeMap<usize, &'a str>,
	line_spans: BTreeMap<usize, HashSet<SpanRef>>,
	primary_spans: Vec<(Location, Location)>,
	label_spans: Vec<(Location, Location, &'a str)>,
}

impl<'a> LineMap<'a> {
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

	pub fn insert_primary(&mut self, span: &ByteSpan) -> SpanRef {
		// get index of next vacant entry
		let span_ref = SpanRef::Primary(self.primary_spans.len());
		self.primary_spans.push(self.locations(span));

		self.intern_span(span, span_ref);

		span_ref
	}

	pub fn insert_label(&mut self, span: &ByteSpan, label: &'a str) -> SpanRef {
		// get index of next vacant entry
		let span_ref = SpanRef::Label(self.label_spans.len());
		let (loc_low, loc_high) = self.locations(span);
		self.label_spans.push((loc_low, loc_high, label));

		self.intern_span(span, span_ref);

		span_ref
	}

	pub fn min_line_nr(&self) -> Option<usize> {
		self.lines.first_key_value().map(|(idx, _)| idx + 1)
	}

	pub fn max_line_nr(&self) -> Option<usize> {
		self.lines.last_key_value().map(|(idx, _)| idx + 1)
	}

	// Searches for the lowest primary location.
	// If none is found it will search the labels for the lowest.
	pub fn min_location(&self) -> Option<Location> {
		self.primary_spans
			.iter()
			.map(|(low, _)| low)
			.min()
			.or_else(|| self.label_spans.iter().map(|(low, _, _)| low).min())
			.copied()
	}

	pub fn line_nrs(&self) -> Vec<usize> {
		self.lines.keys().copied().map(|nr| nr + 1).collect()
	}

	pub fn line(&self, line_nr: usize) -> Option<&str> {
		self.lines.get(&(line_nr - 1)).copied()
	}

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

	fn locations(&self, span: &ByteSpan) -> (Location, Location) {
		(
			self.source.get_pos_location(span.low),
			self.source.get_pos_location(span.high),
		)
	}

	fn intern_span(&mut self, span: &ByteSpan, span_ref: SpanRef) {
		let line_idx_low = self.source.get_pos_line_idx(span.low);
		let line_idx_high = self.source.get_pos_line_idx(span.high);

		for line_idx in line_idx_low..=line_idx_high {
			self.intern_line(line_idx, span_ref);
		}
	}

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

pub struct DiagnositicFormatter<'a> {
	source: &'a Source<'a>,
	msg: &'a str,
	descriptions: Vec<&'a str>,
	line_map: LineMap<'a>,
}

impl<'a> DiagnositicFormatter<'a> {
	pub fn new(source: &'a Source<'a>, msg: &'a str) -> Self {
		let descriptions = <_>::default();

		Self {
			source,
			msg,
			descriptions,
			line_map: LineMap::new(source),
		}
	}

	pub fn description(&mut self, description: &'a str) -> &mut Self {
		self.descriptions.push(description);
		self
	}

	pub fn primary_span(&mut self, span: &ByteSpan) -> &mut Self {
		let _ = self.line_map.insert_primary(span);
		self
	}

	pub fn label_span(&mut self, span: &ByteSpan, label: &'a str) -> &mut Self {
		let _ = self.line_map.insert_label(span, label);
		self
	}

	#[must_use]
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
