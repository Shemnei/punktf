use color_eyre::eyre::{eyre, Result};

use super::diagnostic::Diagnostic;
use super::source::Source;

/// A session collects (diagnostics)[`crate::template::Diagnostic`] for a
/// task. Additionally it keeps track if the task failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Session {
	/// Indicates if the session is considered to be failed.
	failed: bool,

	/// Collected diagnostics.
	diagnostics: Vec<Diagnostic>,
}

impl Session {
	/// Creates a new session. [`Session::failed`] is set to false.
	pub const fn new() -> Self {
		Self {
			failed: false,
			diagnostics: Vec::new(),
		}
	}

	/// Report a [`template::Diagnostic`] which will be added to the session.
	pub fn report(&mut self, diagnostic: Diagnostic) {
		self.diagnostics.push(diagnostic);
	}

	/// Mark the session as failed.
	pub fn mark_failed(&mut self) {
		self.failed = true;
	}

	/// Emit all collected diagnostics. `source` should be the
	/// [`template::source::Source`] from which all the
	/// [`template::Diagnostic`]'s are collected.
	pub fn emit(&self, source: &Source<'_>) {
		for diagnostic in &self.diagnostics {
			diagnostic.emit(source);
		}
	}

	/// This will consume the session and return [`Result::Ok`] if
	/// [`Session::failed`] is `false`. If `failed` is `true` it will return an
	/// error.
	///
	/// # Errors
	///
	/// Returns an error if the session is marked as `failed`.
	pub fn try_finish(self) -> Result<()> {
		if self.failed {
			Err(eyre!("Session contains errors"))
		} else {
			Ok(())
		}
	}
}

impl Default for Session {
	fn default() -> Self {
		Self {
			failed: false,
			diagnostics: Vec::new(),
		}
	}
}
