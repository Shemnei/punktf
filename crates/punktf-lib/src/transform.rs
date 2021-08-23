//! TODO

use color_eyre::Result;

/// TODO
pub trait Transform {
	/// TODO
	fn transform(&self, content: String) -> Result<String>;
}

/// TODO
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineTerminator {
	/// TODO
	Lf,

	/// TODO
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
					let lf_idx = lf_idx + offset;
					content.insert(lf_idx, '\r');
				}

				Ok(content)
			}
		}
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
