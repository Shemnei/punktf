//! A session keeps track of [diagnostics](`super::diagnostic::Diagnostic`) for
//! a specific task and a specific [source](`super::source::Source`). It is
//! used to bundle the diagnostics and emit them after the task has finished.

use color_eyre::eyre::{eyre, Result};

use super::diagnostic::Diagnostic;
use super::source::Source;

/// A session collects [diagnostics](`super::diagnostic::Diagnostic`) for a
/// task. Additionally it keeps track if the task failed.
#[derive(Default, Debug, Clone, PartialEq, Eq)]
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

	/// Report a [Diagnostic](`super::diagnostic::Diagnostic`) which will be added to the session.
	pub fn report(&mut self, diagnostic: Diagnostic) {
		self.diagnostics.push(diagnostic);
	}

	/// Mark the session as failed.
	pub fn mark_failed(&mut self) {
		self.failed = true;
	}

	/// Emit all collected diagnostics. `source` should be the
	/// [source](`super::source::Source`) from which all the
	/// [diagnostics](`super::diagnostic::Diagnostic`) are collected.
	pub fn emit(&self, source: &Source<'_>) {
		for diagnostic in &self.diagnostics {
			diagnostic.emit(source);
		}
	}

	/// This will consume the session and return `Ok` if [`Session::failed`] is
	/// `false`. If `failed` is `true` it will return an error.
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
