use std::collections::VecDeque;
use std::io::{BufRead as _, BufReader};
use std::process::{Command, Stdio};

use color_eyre::Result;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::RangeMap;

#[derive(Error, Debug)]
pub enum HookError {
	#[error("IO Error")]
	IoError(#[from] std::io::Error),
	#[error("Process failed")]
	ExitStatusError(#[from] std::process::ExitStatusError),
}

#[derive(Default, Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Hook(String);

impl Hook {
	pub fn new<S: Into<String>>(command: S) -> Self {
		Self(command.into())
	}

	pub fn execute(&self) -> Result<()> {
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
