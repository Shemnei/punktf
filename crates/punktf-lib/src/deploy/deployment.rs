//! Statistics and information about the final state of a deployed
//! [profile](`crate::profile::Profile`).

use std::borrow::Cow;
use std::collections::HashMap;
use std::fmt;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, SystemTimeError};

use serde::{Deserialize, Serialize};

use super::dotfile::{DeployedDotfile, DeployedDotfileKind, DotfileStatus};
use crate::{Dotfile, Priority};

/// Describes the status of a dotfile deployment.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeploymentStatus {
	/// The dotfile is deployed.
	Success,
	/// The deployment has failed.
	Failed(Cow<'static, str>),
}

impl DeploymentStatus {
	/// Returns success.
	pub const fn success() -> Self {
		Self::Success
	}

	/// Returns a failure.
	pub fn failed<S: Into<Cow<'static, str>>>(reason: S) -> Self {
		Self::Failed(reason.into())
	}

	/// Checks if the deployment was successful.
	pub fn is_success(&self) -> bool {
		self == &Self::Success
	}

	/// Checks if the deployment has failed.
	pub const fn is_failed(&self) -> bool {
		matches!(self, &Self::Failed(_))
	}
}

impl fmt::Display for DeploymentStatus {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			Self::Success => f.write_str("Success"),
			Self::Failed(reason) => write!(f, "Failed: {}", reason),
		}
	}
}

impl<E> From<E> for DeploymentStatus
where
	E: std::error::Error,
{
	fn from(value: E) -> Self {
		Self::failed(value.to_string())
	}
}

/// Describes the deployment of a dotfile.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Deployment {
	/// The time the deployment was started.
	time_start: SystemTime,
	/// The time the deployment was finished.
	time_end: SystemTime,
	/// The status of the deployment.
	status: DeploymentStatus,
	/// The dotfiles that were deployed.
	dotfiles: HashMap<PathBuf, DeployedDotfile>,
}

impl Deployment {
	/// Returns the time the deployment was started.
	pub const fn time_start(&self) -> &SystemTime {
		&self.time_start
	}

	/// Returns the time the deployment was finished.
	pub const fn time_end(&self) -> &SystemTime {
		&self.time_end
	}

	/// Returns the duration the deployment took.
	pub fn duration(&self) -> Result<Duration, SystemTimeError> {
		self.time_end.duration_since(self.time_start)
	}

	/// Returns the status of the deployment.
	pub const fn status(&self) -> &DeploymentStatus {
		&self.status
	}

	/// Returns the dotfiles.
	pub const fn dotfiles(&self) -> &HashMap<PathBuf, DeployedDotfile> {
		&self.dotfiles
	}

	/// Builds the deployment.
	pub fn build() -> DeploymentBuilder {
		DeploymentBuilder::default()
	}
}

#[must_use]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeploymentBuilder {
	time_start: SystemTime,
	dotfiles: HashMap<PathBuf, DeployedDotfile>,
}

impl DeploymentBuilder {
	pub fn add_dotfile(
		&mut self,
		path: PathBuf,
		dotfile: Dotfile,
		status: DotfileStatus,
	) -> &mut Self {
		self.dotfiles.insert(
			path,
			DeployedDotfile {
				kind: DeployedDotfileKind::Dotfile(dotfile),
				status,
			},
		);
		self
	}

	pub fn add_child(
		&mut self,
		path: PathBuf,
		parent: PathBuf,
		status: DotfileStatus,
	) -> &mut Self {
		self.dotfiles.insert(
			path,
			DeployedDotfile {
				kind: DeployedDotfileKind::Child(parent),
				status,
			},
		);
		self
	}

	pub fn contains<P: AsRef<Path>>(&self, path: P) -> bool {
		self.dotfiles.contains_key(path.as_ref())
	}

	pub fn get_dotfile<P: AsRef<Path>>(&self, path: P) -> Option<&Dotfile> {
		let mut value = self.dotfiles.get(path.as_ref())?;

		loop {
			match &value.kind {
				DeployedDotfileKind::Dotfile(dotfile) => return Some(dotfile),
				DeployedDotfileKind::Child(parent_path) => {
					value = self.dotfiles.get(parent_path)?
				}
			}
		}
	}

	/// Only gets the dotfile if all dotfiles in the chain are deployed
	pub fn get_deployed_dotfile<P: AsRef<Path>>(&self, path: P) -> Option<&Dotfile> {
		let mut value = self.dotfiles.get(path.as_ref())?;

		loop {
			if !value.status.is_success() {
				return None;
			}

			match &value.kind {
				DeployedDotfileKind::Dotfile(dotfile) => return Some(dotfile),
				DeployedDotfileKind::Child(parent_path) => {
					value = self.dotfiles.get(parent_path)?
				}
			}
		}
	}

	pub fn get_priority<P: AsRef<Path>>(&self, path: P) -> Option<Option<Priority>> {
		self.get_deployed_dotfile(path)
			.map(|dotfile| dotfile.priority)
	}

	pub fn is_deployed<P: AsRef<Path>>(&self, path: P) -> Option<bool> {
		self.dotfiles
			.get(path.as_ref())
			.map(|dotfile| dotfile.status.is_success())
	}

	pub fn finish(self) -> Deployment {
		let failed_dotfiles = self
			.dotfiles
			.values()
			.filter(|dotfile| dotfile.status().is_failed())
			.count();

		let status = if failed_dotfiles > 0 {
			DeploymentStatus::failed(format!("Deployment of {} dotfiles failed", failed_dotfiles))
		} else {
			DeploymentStatus::Success
		};

		Deployment {
			time_start: self.time_start,
			time_end: SystemTime::now(),
			status,
			dotfiles: self.dotfiles,
		}
	}

	pub fn success(self) -> Deployment {
		Deployment {
			time_start: self.time_start,
			time_end: SystemTime::now(),
			status: DeploymentStatus::Success,
			dotfiles: self.dotfiles,
		}
	}

	pub fn failed<S: Into<Cow<'static, str>>>(self, reason: S) -> Deployment {
		Deployment {
			time_start: self.time_start,
			time_end: SystemTime::now(),
			status: DeploymentStatus::Failed(reason.into()),
			dotfiles: self.dotfiles,
		}
	}
}

impl Default for DeploymentBuilder {
	fn default() -> Self {
		Self {
			time_start: SystemTime::now(),
			dotfiles: HashMap::new(),
		}
	}
}

#[cfg(test)]
mod tests {
	use color_eyre::Result;

	use super::*;

	#[test]
	fn deployment_builder() -> Result<()> {
		let builder = Deployment::build();
		let deployment = builder.success();

		assert!(deployment.status().is_success());
		assert!(deployment.duration()? >= Duration::from_secs(0));

		Ok(())
	}
}
