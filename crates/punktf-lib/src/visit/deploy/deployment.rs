//! Models and structs used by and for the deployment process.

use std::borrow::Cow;
use std::collections::HashMap;
use std::fmt;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, SystemTimeError};

use serde::{Deserialize, Serialize};

use crate::profile::dotfile::Dotfile;
use crate::profile::Priority;

/// Contains the status of a deployed item.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ItemStatus {
	/// The item was successfully created.
	Success,
	/// The item deployment failed.
	Failed(Cow<'static, str>),
	/// The item deployment was skipped.
	Skipped(Cow<'static, str>),
}

impl ItemStatus {
	/// Marks the item operation as successful.
	pub const fn success() -> Self {
		Self::Success
	}

	/// Marks the item operation as failed.
	pub fn failed<S: Into<Cow<'static, str>>>(reason: S) -> Self {
		Self::Failed(reason.into())
	}

	/// Indicates that the item opeartion was skipped.
	pub fn skipped<S: Into<Cow<'static, str>>>(reason: S) -> Self {
		Self::Skipped(reason.into())
	}

	/// Checks if the item operation was successful.
	pub fn is_success(&self) -> bool {
		self == &Self::Success
	}

	/// Checks if the item operation has failed.
	pub const fn is_failed(&self) -> bool {
		matches!(self, &Self::Failed(_))
	}

	/// Checks if the item operation was skipped.
	pub const fn is_skipped(&self) -> bool {
		matches!(self, &Self::Skipped(_))
	}
}

impl fmt::Display for ItemStatus {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			Self::Success => f.write_str("Success"),
			Self::Failed(reason) => write!(f, "Failed: {}", reason),
			Self::Skipped(reason) => write!(f, "Skipped: {}", reason),
		}
	}
}

impl<E> From<E> for ItemStatus
where
	E: std::error::Error,
{
	fn from(value: E) -> Self {
		Self::failed(value.to_string())
	}
}

/// Defines the type of dotfile.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeployedDotfileKind {
	/// A normal dotfile.
	Dotfile(Dotfile),
	/// A dotfile that is contained in a directory that is deployed.
	///
	/// PathBuf is the deploy path of the `parent` dotfile.
	/// The parent should always be of type `Dotfile(_)`.
	Child(PathBuf),
}

impl DeployedDotfileKind {
	/// Checks whether the deployed dotfile type is a normal dotfile.
	pub const fn is_dotfile(&self) -> bool {
		matches!(self, Self::Dotfile(_))
	}

	/// Checks whether the deployed dotfile type is a child dotfile.
	pub const fn is_child(&self) -> bool {
		matches!(self, Self::Child(_))
	}
}

/// Stores the result of a dotfile deployment operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeployedDotfile {
	/// The status of the deployed dotfile.
	pub status: ItemStatus,

	/// The kind of the deployed dotfile.
	pub kind: DeployedDotfileKind,
}

impl DeployedDotfile {
	/// Returns the status of the dotfile operation.
	pub const fn status(&self) -> &ItemStatus {
		&self.status
	}

	/// Retures the kind of the dotfile operation.
	pub const fn kind(&self) -> &DeployedDotfileKind {
		&self.kind
	}
}

impl AsRef<ItemStatus> for DeployedDotfile {
	fn as_ref(&self) -> &ItemStatus {
		self.status()
	}
}

/// Stores the result of a symlink deployment operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeployedSymlink {
	/// The status of the deployed symlink.
	pub status: ItemStatus,

	/// The source path of the link.
	pub source: PathBuf,
}

impl DeployedSymlink {
	/// Returns the status of the link operation.
	pub const fn status(&self) -> &ItemStatus {
		&self.status
	}

	/// Returns the source path of the link .
	pub fn source(&self) -> &Path {
		self.source.as_path()
	}
}

impl AsRef<ItemStatus> for DeployedSymlink {
	fn as_ref(&self) -> &ItemStatus {
		self.status()
	}
}

/// Describes the status of a profile deployment.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeploymentStatus {
	/// The profile is deployed successfully.
	Success,
	/// There were errors during the deployment.
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

/// Describes the deployment of a profile.
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

	/// The links that were deployed.
	symlinks: HashMap<PathBuf, DeployedSymlink>,
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

	/// Returns the symlinks.
	pub const fn symlinks(&self) -> &HashMap<PathBuf, DeployedSymlink> {
		&self.symlinks
	}

	/// Builds the deployment.
	pub fn build() -> DeploymentBuilder {
		DeploymentBuilder::default()
	}
}

/// A builder for a [`Deployment`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeploymentBuilder {
	/// The start time of the deployment.
	///
	/// This used to keep track of the total execution time of the deployment
	/// process.
	time_start: SystemTime,

	/// All dotfiles which were already process by the deployment process.
	dotfiles: HashMap<PathBuf, DeployedDotfile>,

	/// All symlinks which were already process by the deployment process.
	symlinks: HashMap<PathBuf, DeployedSymlink>,
}

impl DeploymentBuilder {
	/// Adds a dotfile with the given `status` to the builder.
	pub fn add_dotfile(
		&mut self,
		path: PathBuf,
		dotfile: Dotfile,
		status: ItemStatus,
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

	/// Adds the child of a dotfile directory with the given `status` to the
	/// builder.
	pub fn add_child(&mut self, path: PathBuf, parent: PathBuf, status: ItemStatus) -> &mut Self {
		self.dotfiles.insert(
			path,
			DeployedDotfile {
				kind: DeployedDotfileKind::Child(parent),
				status,
			},
		);

		self
	}

	/// Adds a symlink with the given `status` to the builder.
	pub fn add_link(&mut self, source: PathBuf, target: PathBuf, status: ItemStatus) -> &mut Self {
		self.symlinks
			.insert(target, DeployedSymlink { source, status });

		self
	}

	/// Checks if the builder already contains a dotfile for the given `path`.
	pub fn contains<P: AsRef<Path>>(&self, path: P) -> bool {
		self.dotfiles.contains_key(path.as_ref())
	}

	/// Gets any dotfile already deployed at `path`.
	///
	/// This function ignores the status of the dotfile.
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

	/// Gets any dotfile already deployed at `path`.
	///
	/// This function only returns a dotfile with [`ItemStatus::Success`].
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

	/// Gets the priority of the dotfile already deployed at `path`.
	///
	/// This function only evaluates a dotfile with [`ItemStatus::Success`].
	pub fn get_priority<P: AsRef<Path>>(&self, path: P) -> Option<&Priority> {
		self.get_deployed_dotfile(path)
			.and_then(|d| d.priority.as_ref())
	}

	/// Checks if a dotfile was already successfully deployed at `path`.
	///
	/// This function only evaluates a dotfile with [`ItemStatus::Success`].
	pub fn is_deployed<P: AsRef<Path>>(&self, path: P) -> Option<bool> {
		self.dotfiles
			.get(path.as_ref())
			.map(|dotfile| dotfile.status.is_success())
	}

	/// Consumes self and creates a [`Deployment`] from it.
	///
	/// This will try to guess the state of the deployment by looking for any
	/// failed deployed dotfile.
	pub fn finish(self) -> Deployment {
		let failed_dotfiles = self
			.dotfiles
			.values()
			.filter(|d| d.status.is_failed())
			.count();

		let failed_links = self
			.symlinks
			.values()
			.filter(|d| d.status.is_failed())
			.count();

		let status = if failed_dotfiles > 0 {
			DeploymentStatus::failed(format!(
				"Deployment of {} dotfiles and {} links failed",
				failed_dotfiles, failed_links
			))
		} else {
			DeploymentStatus::Success
		};

		Deployment {
			time_start: self.time_start,
			time_end: SystemTime::now(),
			status,
			dotfiles: self.dotfiles,
			symlinks: self.symlinks,
		}
	}

	/// Consumes self and creates a [`Deployment`] from it.
	///
	/// This will mark the deployment as success.
	pub fn success(self) -> Deployment {
		Deployment {
			time_start: self.time_start,
			time_end: SystemTime::now(),
			status: DeploymentStatus::Success,
			dotfiles: self.dotfiles,
			symlinks: self.symlinks,
		}
	}

	/// Consumes self and creates a [`Deployment`] from it.
	///
	/// This will mark the deployment as failed with the reason given with
	/// `reason`.
	pub fn failed<S: Into<Cow<'static, str>>>(self, reason: S) -> Deployment {
		Deployment {
			time_start: self.time_start,
			time_end: SystemTime::now(),
			status: DeploymentStatus::Failed(reason.into()),
			dotfiles: self.dotfiles,
			symlinks: self.symlinks,
		}
	}
}

impl Default for DeploymentBuilder {
	fn default() -> Self {
		Self {
			time_start: SystemTime::now(),
			dotfiles: HashMap::new(),
			symlinks: HashMap::new(),
		}
	}
}

#[cfg(test)]
mod tests {
	use color_eyre::Result;

	use super::*;

	#[test]
	fn deployment_builder() -> Result<()> {
		crate::tests::setup_test_env();

		let builder = Deployment::build();
		let deployment = builder.success();

		assert!(deployment.status().is_success());
		assert!(deployment.duration()? >= Duration::from_secs(0));

		Ok(())
	}
}
