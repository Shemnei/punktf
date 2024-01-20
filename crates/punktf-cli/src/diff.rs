//! Functions and utilities for the [`Diff`](`crate::opt::Diff`) command
//! and [`Diff`](`punktf_lib::visit::diff::Diff`) visitor.

use crate::opt::DiffFormat;
use console::{style, Style};
use punktf_lib::visit::diff::Event;
use similar::{ChangeTag, TextDiff};
use std::{fmt, path::Path};

/// Processes diff [`Event`s](`punktf_lib::visit::diff::Event`) from the visitor.
pub fn diff(format: DiffFormat, event: Event<'_>) {
	match event {
		Event::NewFile {
			relative_source_path,
			target_path,
		} => println!(
			"[{} => {}] New file",
			style(relative_source_path.display())
				.bold()
				.black()
				.bright(),
			style(target_path.display()).bold().bright()
		),
		Event::NewDirectory {
			relative_source_path,
			target_path,
		} => println!(
			"[{} => {}] New directory",
			style(relative_source_path.display())
				.bold()
				.black()
				.bright(),
			style(target_path.display()).bold().bright()
		),
		Event::Diff {
			relative_source_path,
			target_path,
			old_content,
			new_content,
		} => {
			if format == DiffFormat::Unified {
				print_udiff(target_path, &old_content, &new_content);
			} else {
				print_pretty(
					relative_source_path,
					target_path,
					&old_content,
					&new_content,
				);
			}
		}
	}
}

/// Prints a file diff with the gnu unified format.
fn print_udiff(target: &Path, old: &str, new: &str) {
	let diff = TextDiff::from_lines(old, new);

	println!("--- {path}\r\n+++ {path}", path = target.display());

	diff.unified_diff()
		.to_writer(std::io::stdout())
		.expect("Writing to stdout to never fail");
}

/// Used to pretty print diff line numbers.
struct Line(Option<usize>);

impl fmt::Display for Line {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self.0 {
			None => write!(f, "    "),
			Some(idx) => write!(f, "{:<4}", idx + 1),
		}
	}
}

/// Prints a file diff with ansii escape codes.
fn print_pretty(source: &Path, target: &Path, old: &str, new: &str) {
	let diff = TextDiff::from_lines(old, new);

	for (idx, group) in diff.grouped_ops(3).iter().enumerate() {
		if idx == 0 {
			println!(
				">> {} => {}",
				style(source.display()).bold().black().bright(),
				style(target.display()).bold().bright()
			);
		}

		if idx > 0 {
			println!("{:-^1$}", "-", 80);
		}

		for op in group {
			for change in diff.iter_inline_changes(op) {
				let (sign, s) = match change.tag() {
					ChangeTag::Delete => ("-", Style::new().red()),
					ChangeTag::Insert => ("+", Style::new().green()),
					ChangeTag::Equal => (" ", Style::new().dim()),
				};
				print!(
					"{}{} |{}",
					style(Line(change.old_index())).dim(),
					style(Line(change.new_index())).dim(),
					s.apply_to(sign).bold(),
				);
				for (emphasized, value) in change.iter_strings_lossy() {
					if emphasized {
						print!("{}", s.apply_to(value).underlined().on_black());
					} else {
						print!("{}", s.apply_to(value));
					}
				}
				if change.missing_newline() {
					println!();
				}
			}
		}
	}
}
