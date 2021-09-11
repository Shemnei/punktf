//! Transforms run once for each defined dotfile during the deploy process.
//!
//! They can either be specified for a whole profile, in which case each dotfile
//! is transformed by them or they can be attached to a specific dotfile.
//!
//! The transformation takes place after the template resolving and takes the
//! contents in a textual representation. After processing the text a new text
//! must be returned.

use std::fmt;

use color_eyre::Result;

/// A transform takes the contents of a dotfile, processes it and returns a new
/// version of the content.
///
/// The dotfile is either the text of a resolved template or a non-template
/// dotfile.
pub trait Transform {
	/// Takes a string as input, processes it and returns a new version of it.
	///
	/// # Errors
	///
	/// If any error occurs during the processing it can be returned.
	fn transform(&self, content: String) -> Result<String>;
}

/// List of all available [`Transform`]s.
///
/// These can be added to a [`Profile`](`crate::profile::Profile`) or a
/// [`Dotfile`](`crate::Dotfile`) to modify the text content.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum ContentTransformer {
	/// Transformer which replaces line termination characters with either unix
	/// style (`\n`) or windows style (`\r\b`).
	LineTerminator(LineTerminator),
}

impl Transform for ContentTransformer {
	fn transform(&self, content: String) -> Result<String> {
		match self {
			Self::LineTerminator(lt) => lt.transform(content),
		}
	}
}

impl fmt::Display for ContentTransformer {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
		fmt::Display::fmt(&self, f)
	}
}

/// Transformer which replaces line termination characters with either unix
/// style (`\n`) or windows style (`\r\b`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum LineTerminator {
	/// Replaces all occurrences of `\r\n` with `\n` (unix style).
	Lf,

	/// Replaces all occurrences of `\n` with `\r\n` (windows style).
	Crlf,
}

impl Transform for LineTerminator {
	fn transform(&self, mut content: String) -> Result<String> {
		match self {
			Self::Lf => Ok(content.replace("\r\n", "\n")),
			Self::Crlf => {
				let lf_idxs = content.match_indices('\n');
				let mut cr_idxs = content.match_indices('\r').peekable();

				// Allowed as it not needless here, the index iterator have a immutable ref
				// and are still alive when the string gets modified. To "unborrow" the
				// collect is necessary.
				#[allow(clippy::needless_collect)]
				let lf_idxs = lf_idxs
					.filter_map(|(lf_idx, _)| {
						while matches!(cr_idxs.peek(), Some((cr_idx,_)) if cr_idx + 1 < lf_idx) {
							// pop standalone `\r`
							let _ = cr_idxs.next().expect("Failed to advance peeked iterator");
						}

						if matches!(cr_idxs.peek(), Some((cr_idx, _)) if cr_idx + 1 == lf_idx) {
							// pop matched cr_idx
							let _ = cr_idxs.next().expect("Failed to advance peeked iterator");
							None
						} else {
							Some(lf_idx)
						}
					})
					.collect::<Vec<_>>();

				for (offset, lf_idx) in lf_idxs.into_iter().enumerate() {
					content.insert(lf_idx + offset, '\r');
				}

				Ok(content)
			}
		}
	}
}

impl fmt::Display for LineTerminator {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
		fmt::Debug::fmt(&self, f)
	}
}

#[cfg(test)]
mod tests {
	use pretty_assertions::assert_eq;

	use super::*;

	#[test]
	fn line_terminator_lf() -> Result<()> {
		const CONTENT: &str = "Hello\r\nWorld\nHow\nare\r\nyou today?\r\r\r\nLast line\r";

		assert_eq!(
			LineTerminator::Lf.transform(String::from(CONTENT))?,
			"Hello\nWorld\nHow\nare\nyou today?\r\r\nLast line\r"
		);

		Ok(())
	}

	#[test]
	fn line_terminator_crlf() -> Result<()> {
		const CONTENT: &str = "Hello\r\nWorld\nHow\nare\r\nyou today?\r\r\r\nLast line\r";

		assert_eq!(
			LineTerminator::Crlf.transform(String::from(CONTENT))?,
			"Hello\r\nWorld\r\nHow\r\nare\r\nyou today?\r\r\r\nLast line\r"
		);

		Ok(())
	}
}
