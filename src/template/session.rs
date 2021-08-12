use std::marker::PhantomData;

use color_eyre::eyre::{eyre, Result};

use super::diagnostic::Diagnositic;
use super::source::Source;

pub trait SessionState {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseState {}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolveState {}

impl SessionState for ParseState {}
impl SessionState for ResolveState {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Session<'src, S> {
	pub source: Source<'src>,
	failed: bool,
	diagnostics: Vec<Diagnositic>,

	_marker: PhantomData<S>,
}

impl<'src> Session<'src, ParseState> {
	pub fn new(source: Source<'src>) -> Self {
		Self {
			source,
			failed: false,
			diagnostics: vec![],

			_marker: PhantomData,
		}
	}

	pub fn try_finish(self) -> Result<Session<'src, ResolveState>> {
		if self.failed {
			Err(eyre!("Parse session has failed"))
		} else {
			let new = Session {
				source: self.source,
				failed: false,
				diagnostics: vec![],

				_marker: PhantomData,
			};

			Ok(new)
		}
	}
}

impl<'src> Session<'src, ResolveState> {
	pub fn try_finish(self) -> Result<()> {
		if self.failed {
			Err(eyre!("Resolve session has failed"))
		} else {
			Ok(())
		}
	}
}

impl<'src, S> Session<'src, S>
where
	S: SessionState,
{
	pub fn report(&mut self, diagnostic: Diagnositic) {
		self.diagnostics.push(diagnostic);
	}

	pub fn mark_failed(&mut self) {
		self.failed = true;
	}

	pub fn emit(&self) {
		for diagnostic in &self.diagnostics {
			diagnostic.emit(&self.source);
		}
	}
}
