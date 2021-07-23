#![feature(exit_status_error)]

use std::borrow::Cow;
use std::collections::HashMap;
use std::error::Error;
use std::fmt;
use std::path::PathBuf;
use std::process::Command;

pub trait Environment {
	fn var<K: AsRef<str>>(&self, key: K) -> Option<Cow<'_, String>>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Env {
	inner: HashMap<String, String>,
}

impl Environment for Env {
	fn var<K>(&self, key: K) -> Option<Cow<'_, String>>
	where
		K: AsRef<str>,
	{
		self.inner.get(key.as_ref()).map(Cow::Borrowed)
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SystemEnv;

impl Environment for SystemEnv {
	fn var<K>(&self, key: K) -> Option<Cow<'_, String>>
	where
		K: AsRef<str>,
	{
		std::env::var(key.as_ref()).ok().map(Cow::Owned)
	}
}

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

#[derive(Default, Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Hook(String);

impl Hook {
	pub fn execute(&self) -> Result<(), HookError> {
		let _ = Command::new(&self.0).output()?.status.exit_ok()?;
		Ok(())
	}
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Profile {
	/// Environment of the profile. Each item will have this environment.
	env: Option<Env>,

	/// Target root path of the deployment. Will be used as file stem for the items
	/// when not overwritten by [Item::target].
	target: PathBuf,

	/// Hook will be executed once before the deployment begins. If the hook fails
	/// the deployment will not be continued.
	pre_hook: Option<Hook>,

	/// Hook will be executed once after the deployment begins.
	post_hook: Option<Hook>,

	/// Items which will be deployed.
	items: Vec<Item>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
	/// Relative path inside the `source` folder.
	path: PathBuf,

	/// Priority of the item. Items with higher priority as others are allowed
	/// to overwrite an item deployed in this deployment.
	priority: Priority,

	/// Environment for the item. If a key is not found here, [Profile::env]
	/// will be searched.
	env: Option<Env>,

	/// Deployment target for the item. If not given it will be [Profile::target] + [Item::path]`.
	target: Option<DeployTarget>,

	/// Merge operation for already existing items.
	merge: Option<MergeMode>,

	/// Indicates if the item should be treated as a template. If this is `false`
	/// no template processing will be done.
	template: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum DeployTarget {
	/// Target will be deployed under [Profile::target] + Alias.
	Alias(PathBuf),
	/// Target will be deployed under the given path.
	Path(PathBuf),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MergeMode {
	/// Overwrites the existing item.
	Overwrite,

	/// Keeps the existing item.
	Keep,

	/// Asks the user for input to decide what to do.
	Ask,
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Priority(Option<u32>);

impl Priority {
	pub fn new<P: Into<Option<u32>>>(priority: P) -> Self {
		Self(priority.into())
	}
}

fn main() {
	println!("Hello, world!");
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn priority_order() {
		assert!(Priority::default() == Priority::new(None));
		assert!(Priority::new(None) < Priority::new(0));
		assert!(Priority::new(0) == Priority::new(0));
		assert!(Priority::new(2) > Priority::new(1));
	}
}
