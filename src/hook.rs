use std::collections::VecDeque;
use std::error::Error;
use std::fmt;
use std::io::{BufRead as _, BufReader};
use std::process::{Command, Stdio};

use serde::{Deserialize, Serialize};

use crate::RangeMap;

#[derive(Debug)]
pub enum HookError {
	IoError(std::io::Error),
	ExitStatusError(std::process::ExitStatusError),
}

impl fmt::Display for HookError {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
		match self {
			Self::IoError(err) => fmt::Display::fmt(err, f),
			Self::ExitStatusError(err) => fmt::Display::fmt(err, f),
		}
	}
}

impl Error for HookError {
	fn source(&self) -> Option<&(dyn Error + 'static)> {
		match self {
			Self::IoError(err) => Some(err),
			Self::ExitStatusError(err) => Some(err),
		}
	}
}

impl From<std::io::Error> for HookError {
	fn from(value: std::io::Error) -> Self {
		Self::IoError(value)
	}
}
impl From<std::process::ExitStatusError> for HookError {
	fn from(value: std::process::ExitStatusError) -> Self {
		Self::ExitStatusError(value)
	}
}

#[derive(Default, Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Hook(String);

impl Hook {
	pub fn new<S: Into<String>>(command: S) -> Self {
		Self(command.into())
	}

	pub fn execute(&self) -> Result<(), HookError> {
		let mut child = self
			.prepare_command()
			.stdout(Stdio::piped())
			.stderr(Stdio::piped())
			.spawn()?;

		for line in BufReader::new(child.stdout.take().unwrap()).lines() {
			println!("{}", line.unwrap());
		}

		for line in BufReader::new(child.stderr.take().unwrap()).lines() {
			println!("{}", line.unwrap());
		}

		child
			.wait_with_output()?
			.status
			.exit_ok()
			.map_err(|err| err.into())
	}

	fn prepare_command(&self) -> Command {
		// Flow:
		//	- detect `\"` (future maybe: `'`, `$(`, ```)
		//	- split by ` `, `\"`
		let mut escape_idxs = Vec::new();
		let mut start_idx = 0;

		// find escape sequences
		while let Some(escape_idx) = self.0[start_idx..].find('\"') {
			start_idx += escape_idx;
			escape_idxs.push(start_idx);
			start_idx += 1;
		}

		let ranges = RangeMap::new(escape_idxs);

		let mut parts = VecDeque::new();
		let mut split_idx = 0;
		let mut start_idx = 0;

		while let Some(space_idx) = self.0[start_idx..].find(' ') {
			start_idx += space_idx;

			// If not in range means we need to split as the space is not in a
			// escaped part
			if !ranges.in_range(&start_idx) {
				parts.push_back(&self.0[split_idx..start_idx]);

				split_idx = start_idx + 1;
			}

			start_idx += 1;
		}

		if split_idx < self.0.len() {
			parts.push_back(&self.0[split_idx..]);
		}

		log::debug!("Hook parts: {:?}", parts);

		let mut cmd = Command::new(parts.pop_front().unwrap());
		cmd.args(parts);
		cmd
	}
}
