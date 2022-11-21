use crate::opt::DiffFormat;
use console::{style, Style};
use punktf_lib::action::diff::Event;
use similar::{udiff::unified_diff, ChangeTag, TextDiff};
use std::{fmt, path::Path};

pub(crate) fn diff(format: DiffFormat, event: Event<'_>) {
	match event {
		Event::NewFile(path) => println!("[{}] New file", path.display()),
		Event::NewDirectory(path) => println!("[{}] New directory", path.display()),
		Event::Diff {
			target_path,
			old_content,
			new_contnet,
		} => {
			if format == DiffFormat::Unified {
				print_udiff(target_path, &old_content, &new_contnet);
			} else {
				print_pretty(target_path, &old_content, &new_contnet);
			}
		}
	}
}

fn print_udiff(target: &Path, old: &str, new: &str) {
	let diff = TextDiff::from_lines(old, new);

	println!("--- {path}\r\n+++ {path}", path = target.display());
	diff.unified_diff().to_writer(std::io::stdout()).unwrap();
}

struct Line(Option<usize>);

impl fmt::Display for Line {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self.0 {
			None => write!(f, "    "),
			Some(idx) => write!(f, "{:<4}", idx + 1),
		}
	}
}

fn print_pretty(target: &Path, old: &str, new: &str) {
	let diff = TextDiff::from_lines(old, new);

	for (idx, group) in diff.grouped_ops(3).iter().enumerate() {
		if idx == 0 {
			println!(">> {}", style(target.display()).bold().bright());
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
