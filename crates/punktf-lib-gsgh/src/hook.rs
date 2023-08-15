use std::{
	io::{BufRead, BufReader},
	path::{Path, PathBuf},
	process::{Command, Stdio},
};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::env::LayeredEnvironment;

// Have special syntax for skipping deployment on pre_hook
// Analog: <https://learn.microsoft.com/en-us/azure/devops/pipelines/scripts/logging-commands?view=azure-devops>
// e.g. punktf:skip_deployment

#[derive(Error, Debug)]
pub enum HookError {
	#[error("IO Error")]
	IoError(#[from] std::io::Error),

	#[error("Process failed with status `{0}`")]
	ExitStatusError(std::process::ExitStatus),
}

impl From<std::process::ExitStatus> for HookError {
	fn from(value: std::process::ExitStatus) -> Self {
		Self::ExitStatusError(value)
	}
}

pub type Result<T, E = HookError> = std::result::Result<T, E>;

// TODO: Replace once `exit_ok` becomes stable
trait ExitOk {
	type Error;

	fn exit_ok(self) -> Result<(), Self::Error>;
}

impl ExitOk for std::process::ExitStatus {
	type Error = HookError;

	fn exit_ok(self) -> Result<(), <Self as ExitOk>::Error> {
		if self.success() {
			Ok(())
		} else {
			Err(self.into())
		}
	}
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "with", rename_all = "snake_case")]
pub enum Hook {
	Inline(String),
	File(PathBuf),
}

impl Hook {
	pub fn run(self, cwd: &Path, env: LayeredEnvironment) -> Result<()> {
		let mut child = self
			.prepare_command()?
			.current_dir(cwd)
			.stdout(Stdio::piped())
			.stderr(Stdio::piped())
			.envs(env.as_str_map())
			.spawn()?;

		// No need to call kill here as the program will immediately exit
		// and thereby kill all spawned children
		let stdout = child.stdout.take().expect("Failed to get stdout from hook");

		for line in BufReader::new(stdout).lines() {
			match line {
				Ok(line) => println!("hook::stdout > {}", line),
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
				Ok(line) => println!("hook::stderr > {}", line),
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
		#[allow(unused_assignments)]
		let mut cmd = None;

		#[cfg(target_family = "windows")]
		{
			let mut c = Command::new("cmd");
			c.arg("/C");
			cmd = Some(c);
		}

		#[cfg(target_family = "unix")]
		{
			let mut c = Command::new("sh");
			c.arg("-c");
			cmd = Some(c)
		}

		let Some(mut cmd) = cmd else {
				return Err(HookError::IoError(std::io::Error::new(std::io::ErrorKind::Other, "Hooks are only supported on Windows and Unix-based systems")));
			};

		match self {
			Self::Inline(s) => {
				cmd.arg(s);
			}
			Self::File(path) => {
				let s = std::fs::read_to_string(path)?;
				cmd.arg(s);
			}
		}

		Ok(cmd)
	}
}

#[cfg(test)]
mod tests {
	use crate::env::Environment;

	use super::*;

	#[test]
	fn echo_hello_world() {
		let env = Environment(
			[
				("TEST", serde_yaml::Value::Bool(true)),
				("FOO", serde_yaml::Value::String(" BAR Test".into())),
			]
			.into_iter()
			.map(|(k, v)| (k.to_string(), v))
			.collect(),
		);

		let mut lenv = LayeredEnvironment::default();
		lenv.push("test", env);

		println!("{:#?}", lenv.as_str_map());

		let hook = Hook::Inline(r#"echo "Hello World""#.to_string());
		hook.run(Path::new("/tmp"), lenv).unwrap();
	}
}
