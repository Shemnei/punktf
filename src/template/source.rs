use std::fmt;
use std::ops::Deref;
use std::path::Path;

use super::span::{BytePos, ByteSpan};

/// Describes a location within a source file. The line is 1 indexed the
/// character 0 indexed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Location {
	line: usize,
	character: usize,
}

impl Location {
	pub fn line(&self) -> usize {
		self.line
	}

	pub fn character(&self) -> usize {
		self.character
	}
}

impl fmt::Display for Location {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "{}:{}", self.line, self.character)
	}
}

/// Describes where some content came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SourceOrigin<'a> {
	File(&'a Path),
	Anonymous,
}

impl<'a> fmt::Display for SourceOrigin<'a> {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			Self::File(path) => fmt::Display::fmt(&path.display(), f),
			Self::Anonymous => f.write_str("anonymous"),
		}
	}
}

/// Holds the contents of a template file together with the origins where the
/// content came from. Besides the origin it also holds some information used
/// in error reporting.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Source<'a> {
	pub(crate) origin: SourceOrigin<'a>,
	pub(crate) content: &'a str,
	pub(crate) line_starts: Vec<usize>,
}

impl<'a> Source<'a> {
	pub fn new(origin: SourceOrigin<'a>, content: &'a str) -> Self {
		// find the indicies where a new line starts
		// +1 to get the character after the `\n`
		let line_starts = std::iter::once(0)
			.chain(
				content
					.match_indices('\n')
					.into_iter()
					.map(|(idx, _)| idx + 1),
			)
			.collect();

		Self {
			origin,
			content,
			line_starts,
		}
	}

	pub fn anonymous(content: &'a str) -> Self {
		Self::new(SourceOrigin::Anonymous, content)
	}

	pub fn file(path: &'a Path, content: &'a str) -> Self {
		Self::new(SourceOrigin::File(path), content)
	}

	pub fn get_pos_line_idx(&self, pos: BytePos) -> usize {
		match self.line_starts.binary_search(&pos.as_usize()) {
			Ok(idx) => idx,
			Err(idx) => idx - 1,
		}
	}

	pub fn get_pos_location(&self, pos: BytePos) -> Location {
		let line_idx = self.get_pos_line_idx(pos);
		let line_start = self.line_starts[line_idx];

		Location {
			line: line_idx + 1,
			character: (pos.as_usize() - line_start),
		}
	}

	pub fn get_pos_line(&self, pos: BytePos) -> &'a str {
		let line_start_idx = self.get_pos_line_idx(pos);

		let line_end_idx = self.line_starts.get(line_start_idx + 1);

		let line_start = BytePos::from_usize(self.line_starts[line_start_idx]);
		// end of the line (-1 to get the last char of the line)
		let line_end =
			BytePos::from_usize(line_end_idx.map_or_else(|| self.content.len(), |&idx| idx - 1));

		&self.content[ByteSpan::new(line_start, line_end)]
	}

	pub fn origin(&self) -> &SourceOrigin<'_> {
		&self.origin
	}

	pub fn content(&self) -> &str {
		self.content
	}
}

impl Deref for Source<'_> {
	type Target = str;

	fn deref(&self) -> &Self::Target {
		self.content
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn lines() {
		let content = r#"Hello
World
Foo
Bar"#;

		let src = Source::anonymous(content);

		assert_eq!(
			src.get_pos_location(BytePos::new(0)),
			Location {
				line: 1,
				character: 0
			}
		);
		assert_eq!(
			src.get_pos_location(BytePos::new(6)),
			Location {
				line: 2,
				character: 0
			}
		);
	}
}
