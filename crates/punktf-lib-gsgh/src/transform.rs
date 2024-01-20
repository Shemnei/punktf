use serde::{Deserialize, Serialize};

pub trait Transform {
	fn apply(&self, content: String) -> Result<String, Box<dyn std::error::Error>>;
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "with", rename_all = "snake_case")]
pub enum Transformer {
	/// Transformer which replaces line termination characters with either unix
	/// style (`\n`) or windows style (`\r\b`).
	LineTerminator(LineTerminator),
}

/// Transformer which replaces line termination characters with either unix
/// style (`\n`) or windows style (`\r\b`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum LineTerminator {
	/// Replaces all occurrences of `\r\n` with `\n` (unix style).
	LF,

	/// Replaces all occurrences of `\n` with `\r\n` (windows style).
	CRLF,
}
