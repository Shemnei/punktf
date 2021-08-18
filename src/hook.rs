use std::io::{BufRead as _, BufReader};
use std::path::Path;
use std::process::{Command, Stdio};

use color_eyre::eyre::Result;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum HookError {
	#[error("IO Error")]
	IoError(#[from] std::io::Error),
	#[error("Process failed")]
	ExitStatusError(#[from] std::process::ExitStatusError),
}

/// Implements the `Hook` trait, which is used to run a command after or before a build.
#[derive(Default, Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Hook(String);

impl Hook {
	/// Creates a new Hook for the given command. The command must be executable by the native shell.
	pub fn new<S: Into<String>>(command: S) -> Self {
		Self(command.into())
	}

	/// Runs the hook command.
	pub fn command(&self) -> &str {
		&self.0
	}

	/// Executes the hook command.
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

	/// Prepares the command for execution depending on the platform.
	fn prepare_command(&self) -> Result<Command> {
		cfg_if::cfg_if! {
			if #[cfg(target_family = "windows")] {
				let mut cmd = Command::new("cmd");
				cmd.args(&["/C", &self.0]);
				Ok(cmd)
			} else if #[cfg(target_family = "unix")] {
				let mut cmd = Command::new("sh");
				cmd.args(&["-c", &self.0]);
				Ok(cmd)
			} else {
				Err(std::io::Error::new(std::io::ErrorKind::Other, "Hooks are only supported on Windows and Unix-based systems"))
			}
		}
	}
}
