use color_eyre::eyre::{eyre, Result};

use super::diagnostic::Diagnositic;
use super::source::Source;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Session {
	failed: bool,
	diagnostics: Vec<Diagnositic>,
}

impl Session {
	pub const fn new() -> Self {
		Self {
			failed: false,
			diagnostics: Vec::new(),
		}
	}

	pub fn report(&mut self, diagnostic: Diagnositic) {
		self.diagnostics.push(diagnostic);
	}

	pub fn mark_failed(&mut self) {
		self.failed = true;
	}

	pub fn emit(&self, source: &Source<'_>) {
		for diagnostic in &self.diagnostics {
			diagnostic.emit(source);
		}
	}

	pub fn try_finish(self) -> Result<()> {
		if self.failed {
			Err(eyre!("Session has failed"))
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
