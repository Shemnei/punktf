//! This modules holds structures related to source files, ways to analyze them
//! and to convert between [byte positions](`super::span::BytePos`) and
//! [character positions](`super::span::CharPos`).

use std::ops::Deref;
use std::path::Path;
use std::{fmt, vec};

use unicode_width::UnicodeWidthChar;

use super::span::{BytePos, ByteSpan, CharPos, Pos};

/// Describes a location within a source file. The line is 1 indexed while
/// the column is 0 indexed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Location {
	/// One indexed line number.
	line: usize,

	/// Zero indexed column.
	column: usize,
}

impl Location {
	/// Returns the one indexed line number.
	pub const fn line(&self) -> usize {
		self.line
	}

	/// Returns the zero indexed column.
	pub const fn column(&self) -> usize {
		self.column
	}

	/// Returns the string representation of the location. For displaying
	/// purposes both the line number and column are one indexed.
	pub fn display(&self) -> String {
		format!("{}:{}", self.line, self.column + 1)
	}
}

// COPYRIGHT by Rust project contributors
// <https://github.com/rust-lang/rust/graphs/contributors>
//
// Inspired by <https://github.com/rust-lang/rust/blob/362e0f55eb1f36d279e5c4a58fb0fe5f9a2c579d/compiler/rustc_span/src/lib.rs#L273>.
/// This struct holds the origin from which a [`Source`] came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SourceOrigin<'a> {
	/// The origin is a file located at the path.
	File(&'a Path),

	/// An unknown/anonymous origin (mainly used for testing).
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

// COPYRIGHT by Rust project contributors
// <https://github.com/rust-lang/rust/graphs/contributors>
//
// Copied from <https://github.com/rust-lang/rust/blob/362e0f55eb1f36d279e5c4a58fb0fe5f9a2c579d/compiler/rustc_span/src/lib.rs#L1059>.
/// Identifies the [position](`super::span::BytePos`) and the width of a
/// multi-byte character in a [`Source`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MultiByteChar {
	/// Position of the character in the [`Source`].
	pos: BytePos,

	/// Width in bytes of the character, `>= 2`.
	bytes: u8,
}

impl MultiByteChar {
	/// Returns the [position](`super::span::BytePos`) of the character.
	pub const fn pos(&self) -> &BytePos {
		&self.pos
	}

	/// Returns the width in bytes of the character.
	pub const fn width(&self) -> u8 {
		self.bytes
	}
}

// COPYRIGHT by Rust project contributors
// <https://github.com/rust-lang/rust/graphs/contributors>
//
// Copied from <https://github.com/rust-lang/rust/blob/362e0f55eb1f36d279e5c4a58fb0fe5f9a2c579d/compiler/rustc_span/src/lib.rs#L1068>.
/// Identifies the [position](`super::span::BytePos`) of a
/// special width character in a [`Source`]. Special width in this context
/// means a character with `byte width != 1`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SpecialWidthChar {
	/// A zero width character. These are mostly ASCII control characters.
	ZeroWidth(BytePos),
	/// A full width char.
	Wide(BytePos),
	/// Tab byte `\t`/`0x09`
	Tab(BytePos),
}

impl SpecialWidthChar {
	/// Tries to create a new `SpecialWidthChar`. The kind is derived from the
	/// given `width`.
	///
	/// # Panics
	///
	/// Panics if `width` can not be mapped to a `SpecialWidthChar`.
	pub fn new(pos: BytePos, width: usize) -> Self {
		match width {
			0 => Self::ZeroWidth(pos),
			2 => Self::Wide(pos),
			4 => Self::Tab(pos),
			_ => panic!("Unsupported width for SpecialWidthChar: {}", width),
		}
	}

	/// Returns the width in bytes of the character.
	pub const fn width(&self) -> usize {
		match self {
			Self::ZeroWidth(_) => 0,
			Self::Wide(_) => 2,
			Self::Tab(_) => 4,
		}
	}

	/// Returns the [position](`super::span::BytePos`) of the character.
	pub const fn pos(&self) -> &BytePos {
		match self {
			Self::ZeroWidth(p) | Self::Wide(p) | Self::Tab(p) => p,
		}
	}
}

// COPYRIGHT by Rust project contributors
// <https://github.com/rust-lang/rust/graphs/contributors>
//
// Copied from <https://github.com/rust-lang/rust/blob/362e0f55eb1f36d279e5c4a58fb0fe5f9a2c579d/compiler/rustc_span/src/analyze_source_file.rs#L207> with slight adaptations.
/// Collects all [`SpecialWidthChar`], [`MultiByteChar`] and
/// [positions](`super::span::BytePos`) of line breaks contained within
/// `content`.
fn analyze_source(content: &'_ str) -> (Vec<BytePos>, Vec<SpecialWidthChar>, Vec<MultiByteChar>) {
	// start first line at index 0
	let mut i = 0;

	let mut lines = vec![BytePos::new(0)];
	let mut special_width_chars = Vec::new();
	let mut multi_byte_chars = Vec::new();

	while i < content.len() {
		let byte = content.as_bytes()[i];

		let mut char_len = 1;

		// all chars between 0-31 are ascii control characters
		if byte < 32 {
			match byte {
				b'\n' => lines.push(BytePos::from_usize(i + 1)),
				b'\t' => special_width_chars.push(SpecialWidthChar::Tab(BytePos::from_usize(i))),
				_ => special_width_chars.push(SpecialWidthChar::ZeroWidth(BytePos::from_usize(i))),
			}
		} else if byte > 127 {
			// bigger than `DEL`, could be multi-byte char
			let chr = (&content[i..]).chars().next().expect("A valid char");
			char_len = chr.len_utf8();

			let pos = BytePos::from_usize(i);

			if char_len > 1 {
				multi_byte_chars.push(MultiByteChar {
					pos,
					bytes: char_len as u8,
				})
			}

			let char_width = UnicodeWidthChar::width(chr).unwrap_or(0);

			if char_width != 1 {
				special_width_chars.push(SpecialWidthChar::new(pos, char_width));
			}
		}

		i += char_len;
	}

	(lines, special_width_chars, multi_byte_chars)
}

// COPYRIGHT by Rust project contributors
// <https://github.com/rust-lang/rust/graphs/contributors>
//
// Inspired by <https://github.com/rust-lang/rust/blob/362e0f55eb1f36d279e5c4a58fb0fe5f9a2c579d/compiler/rustc_span/src/lib.rs#L1246>.
/// Holds the contents of a file together with the origins where the content
/// came from. Besides the origin it also holds some information used in error
/// reporting.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Source<'a> {
	/// Origin of the source file.
	pub(crate) origin: SourceOrigin<'a>,

	/// Content of the source file.
	pub(crate) content: &'a str,

	/// [Positions](`super::span::BytePos`) of all characters which start a new line in [`Source::content`].
	pub(crate) lines: Vec<BytePos>,

	/// All [`SpecialWidthChar`] contained in [`Source::content`].
	pub(crate) special_width_chars: Vec<SpecialWidthChar>,

	/// All [`MultiByteChar`] contained in [`Source::content`].
	pub(crate) multi_byte_chars: Vec<MultiByteChar>,
}

impl<'a> Source<'a> {
	/// Creates a new source for the given `origin` and `content`.
	pub fn new(origin: SourceOrigin<'a>, content: &'a str) -> Self {
		let (lines, special_width_chars, multi_byte_chars) = analyze_source(content);

		Self {
			origin,
			content,
			lines,
			special_width_chars,
			multi_byte_chars,
		}
	}

	/// Creates a new source with [`SourceOrigin::Anonymous`] and the given
	/// `content`.
	pub fn anonymous(content: &'a str) -> Self {
		Self::new(SourceOrigin::Anonymous, content)
	}

	/// Creates a new source with [`SourceOrigin::File`] and the given
	/// `content`.
	pub fn file(path: &'a Path, content: &'a str) -> Self {
		Self::new(SourceOrigin::File(path), content)
	}

	/// Translates a (byte position)[`super::span::BytePos`] into a (character
	/// position)[`super::span::CharPos`]. These two positions may diverge to
	/// to [`MultiByteChar`] or [`SpecialWidthChar`], as they count as a single
	/// character but are made up of multiple/no bytes.
	pub fn get_charpos(&self, pos: BytePos) -> CharPos {
		let mut offset = 0;
		let mut count = 0;

		for swc in &self.special_width_chars {
			if swc.pos() < &pos {
				offset += swc.width();
				count += 1;
			} else {
				// as the pos's are sorted we can abort after the first bigger
				// pos
				break;
			}
		}

		for mbc in &self.multi_byte_chars {
			if mbc.pos() < &pos {
				offset += 1;
				count += mbc.width() as usize;
			} else {
				// as the pos's are sorted we can abort after the first bigger
				// pos
				break;
			}
		}

		let cpos = CharPos::from_usize((pos.as_usize() + offset) - count);

		log::trace!("Translating pos: {} > {}", pos, cpos,);

		cpos
	}

	/// Returns the zero indexed line index `pos` is located on.
	pub fn get_pos_line_idx(&self, pos: BytePos) -> usize {
		match self.lines.binary_search(&pos) {
			Ok(idx) => idx,
			Err(idx) => idx - 1,
		}
	}

	/// Converts a [position](`super::span::BytePos`) to a [`Location`].
	pub fn get_pos_location(&self, pos: BytePos) -> Location {
		let line_idx = self.get_pos_line_idx(pos);
		let line_start = self.lines[line_idx];

		let pos_cpos = self.get_charpos(pos);
		let line_start_cpos = self.get_charpos(line_start);

		Location {
			line: line_idx + 1,
			column: (pos_cpos.as_usize() - line_start_cpos.as_usize()),
		}
	}

	/// Get's the contents of a line which is located at the zero indexed `idx`.
	pub fn get_idx_line(&self, idx: usize) -> &'a str {
		let line_end_idx = self.lines.get(idx + 1);

		let line_start = self.lines[idx];

		// end of the line (-1 to get the last char of the line)
		let line_end = BytePos::from_usize(
			line_end_idx.map_or_else(|| self.content.len(), |&idx| idx.as_usize() - 1),
		);

		&self.content[ByteSpan::new(line_start, line_end)]
	}

	/// Get's the contents of a line on which `pos` is located on.
	pub fn get_pos_line(&self, pos: BytePos) -> &'a str {
		self.get_idx_line(self.get_pos_line_idx(pos))
	}

	/// Returns the origin of the source.
	pub const fn origin(&self) -> &SourceOrigin<'_> {
		&self.origin
	}

	/// Returns the whole content of the source.
	pub const fn content(&self) -> &str {
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
	fn location_lines() {
		crate::tests::setup_test_env();

		let content = r#"Hello
World
Foo
Bar"#;

		let src = Source::anonymous(content);

		assert_eq!(
			src.get_pos_location(BytePos::new(0)),
			Location { line: 1, column: 0 }
		);
		assert_eq!(
			src.get_pos_location(BytePos::new(6)),
			Location { line: 2, column: 0 }
		);
	}

	#[test]
	fn location_special() {
		crate::tests::setup_test_env();

		let content = "\tA\r\n\t\tHello";

		let src = Source::anonymous(content);

		assert_eq!(
			src.get_pos_location(BytePos::new(1)),
			Location { line: 1, column: 4 }
		);

		assert_eq!(
			src.get_pos_location(BytePos::new(6)),
			Location { line: 2, column: 8 }
		);
	}
}
