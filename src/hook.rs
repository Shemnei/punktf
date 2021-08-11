use std::collections::VecDeque;
use std::io::{BufRead as _, BufReader};
use std::path::Path;
use std::process::{Command, Stdio};

use color_eyre::eyre::{eyre, Result};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Default, Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RangeMap(Vec<usize>);

impl RangeMap {
	pub fn new<I: IntoIterator<Item = usize>>(items: I) -> Result<Self> {
		let items: Vec<usize> = items.into_iter().collect();

		if items.len() % 2 != 0 {
			return Err(eyre!("RangeMap must have an even number of items"));
		}

		Ok(Self(items))
	}

	pub fn in_range(&self, value: &usize) -> bool {
		match self.0.binary_search(value) {
			// value is at start or at the end of a range
			Ok(_) => true,
			// value is in range if the index is uneven
			// e.g. (0 1) (2 3)
			// idx = 1 => (0 [1] 2) (3 4)
			Err(idx) => idx % 2 == 1,
		}
	}
}

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

	pub fn execute(&self, cwd: &Path) -> Result<()> {
		let mut child = self
			.prepare_command()?
			.current_dir(cwd)
			.stdout(Stdio::piped())
			.stderr(Stdio::piped())
			.spawn()?;

		// No need to call kill here as the program will immediately exit
		// and thereby kill all spawned children
		let stdout = child.stdout.take().expect("Failed to get stdout from hook");

		for line in BufReader::new(stdout).lines() {
			match line {
				Ok(line) => log::info!("hook::stdout > {}", line),
				Err(err) => {
					// Result is explicitly ignored as an error was already
					// encountered
					let _ = child.kill();
					return Err(err.into());
				}
			}
		}

		// No need to call kill here as the program will immediately exit
		// and thereby kill all spawned children
		let stderr = child.stderr.take().expect("Failed to get stderr from hook");

		for line in BufReader::new(stderr).lines() {
			match line {
				Ok(line) => log::error!("hook::stderr > {}", line),
				Err(err) => {
					// Result is explicitly ignored as an error was already
					// encountered
					let _ = child.kill();
					return Err(err.into());
				}
			}
		}

		child
			.wait_with_output()?
			.status
			.exit_ok()
			.map_err(Into::into)
	}

	fn prepare_command(&self) -> Result<Command> {
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

		let ranges = RangeMap::new(escape_idxs)?;

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

		let program = parts.pop_front().ok_or_else(|| eyre!("Hook is empty"))?;
		let mut cmd = Command::new(program);
		cmd.args(parts);
		Ok(cmd)
	}
}
