use std::borrow::Cow;
use std::collections::HashMap;
use std::fmt;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};

use super::dotfile::{DeployedDotfile, DeployedDotfileKind, DotfileStatus};
use crate::{Dotfile, Priority};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeploymentStatus {
	Success,
	Failed(Cow<'static, str>),
}

impl DeploymentStatus {
	pub fn success() -> Self {
		Self::Success
	}

	pub fn failed<S: Into<Cow<'static, str>>>(reason: S) -> Self {
		Self::Failed(reason.into())
	}

	pub fn is_success(&self) -> bool {
		self == &Self::Success
	}

	pub fn is_failed(&self) -> bool {
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Deployment {
	time_start: DateTime<Utc>,
	time_end: DateTime<Utc>,
	status: DeploymentStatus,
	dotfiles: HashMap<PathBuf, DeployedDotfile>,
}

impl Deployment {
	pub fn time_start(&self) -> &DateTime<Utc> {
		&self.time_start
	}

	pub fn time_end(&self) -> &DateTime<Utc> {
		&self.time_end
	}

	pub fn duration(&self) -> Duration {
		self.time_end - self.time_start
	}

	pub fn status(&self) -> &DeploymentStatus {
		&self.status
	}

	pub fn build() -> DeploymentBuilder {
		DeploymentBuilder::default()
	}
}

#[must_use]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeploymentBuilder {
	time_start: DateTime<Utc>,
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

	pub fn success(self) -> Deployment {
		Deployment {
			time_start: self.time_start,
			time_end: Utc::now(),
			status: DeploymentStatus::Success,
			dotfiles: self.dotfiles,
		}
	}

	pub fn failed<S: Into<Cow<'static, str>>>(self, reason: S) -> Deployment {
		Deployment {
			time_start: self.time_start,
			time_end: Utc::now(),
			status: DeploymentStatus::Failed(reason.into()),
			dotfiles: self.dotfiles,
		}
	}
}

impl Default for DeploymentBuilder {
	fn default() -> Self {
		Self {
			time_start: Utc::now(),
			dotfiles: HashMap::new(),
			// TODO: INVESTIGATE - Causes stack overflow???
			//..Default::default()
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn deployment_builder() {
		let builder = Deployment::build();
		let deployment = builder.success();

		assert!(deployment.status().is_success());
		assert!(deployment.duration() >= Duration::seconds(0));
	}
}
