#![feature(exit_status_error)]

use std::borrow::Cow;
use std::collections::HashMap;
use std::error::Error;
use std::fmt;
use std::path::PathBuf;
use std::process::Command;

use serde::{Deserialize, Serialize};

pub trait Environment {
	fn var<K: AsRef<str>>(&self, key: K) -> Option<Cow<'_, String>>;
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Env {
	#[serde(flatten)]
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

#[derive(Default, Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Hook(String);

impl Hook {
	pub fn new<S: Into<String>>(command: S) -> Self {
		Self(command.into())
	}

	pub fn execute(&self) -> Result<(), HookError> {
		let mut cmd = if cfg!(target_os = "windows") {
			let mut cmd = Command::new("cmd");
			cmd.arg("/C");
			cmd
		} else {
			let mut cmd = Command::new("sh");
			cmd.arg("-c");
			cmd
		};

		let _ = cmd.arg(&self.0).output()?.status.exit_ok()?;

		Ok(())
	}
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Profile {
	/// Environment of the profile. Each item will have this environment.
	#[serde(skip_serializing_if = "Option::is_none")]
	env: Option<Env>,

	/// Target root path of the deployment. Will be used as file stem for the items
	/// when not overwritten by [Item::target].
	target: PathBuf,

	/// Hook will be executed once before the deployment begins. If the hook fails
	/// the deployment will not be continued.
	#[serde(skip_serializing_if = "Option::is_none")]
	pre_hook: Option<Hook>,

	/// Hook will be executed once after the deployment begins.
	#[serde(skip_serializing_if = "Option::is_none")]
	post_hook: Option<Hook>,

	/// Items which will be deployed.
	items: Vec<Item>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Item {
	/// Relative path inside the `source` folder.
	path: PathBuf,

	/// Priority of the item. Items with higher priority as others are allowed
	/// to overwrite an item deployed in this deployment.
	#[serde(skip_serializing_if = "Option::is_none")]
	priority: Option<Priority>,

	/// Environment for the item. If a key is not found here, [Profile::env]
	/// will be searched.
	#[serde(skip_serializing_if = "Option::is_none")]
	env: Option<Env>,

	/// Deployment target for the item. If not given it will be [Profile::target] + [Item::path]`.
	#[serde(skip_serializing_if = "Option::is_none")]
	target: Option<DeployTarget>,

	/// Merge operation for already existing items.
	#[serde(skip_serializing_if = "Option::is_none")]
	merge: Option<MergeMode>,

	/// Indicates if the item should be treated as a template. If this is `false`
	/// no template processing will be done.
	#[serde(skip_serializing_if = "Option::is_none")]
	template: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DeployTarget {
	/// Target will be deployed under [Profile::target] + Alias.
	Alias(PathBuf),
	/// Target will be deployed under the given path.
	Path(PathBuf),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MergeMode {
	/// Overwrites the existing item.
	Overwrite,

	/// Keeps the existing item.
	Keep,

	/// Asks the user for input to decide what to do.
	Ask,
}

#[derive(
	Default, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
pub struct Priority(u32);

impl Priority {
	pub fn new(priority: u32) -> Self {
		Self(priority)
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn priority_order() {
		assert!(Priority::default() == Priority::new(0));
		assert!(Priority::new(0) == Priority::new(0));
		assert!(Priority::new(2) > Priority::new(1));
	}

	#[test]
	fn profile_serde() {
		let mut profile_env = HashMap::new();
		profile_env.insert(String::from("RUSTC_VERSION"), String::from("XX.YY"));
		profile_env.insert(String::from("RUSTC_PATH"), String::from("/usr/bin/rustc"));

		let mut item_env = HashMap::new();
		item_env.insert(String::from("RUSTC_VERSION"), String::from("55.22"));
		item_env.insert(String::from("USERNAME"), String::from("demo"));

		let profile = Profile {
			env: Some(Env { inner: profile_env }),
			target: PathBuf::from("/home/demo/.config"),
			pre_hook: Some(Hook::new("echo \"Foo\"")),
			post_hook: Some(Hook::new("profiles/test.sh")),
			items: vec![
				Item {
					path: PathBuf::from("init.vim.ubuntu"),
					priority: Some(Priority::new(2)),
					env: None,
					target: Some(DeployTarget::Alias(PathBuf::from("init.vim"))),
					merge: Some(MergeMode::Overwrite),
					template: None,
				},
				Item {
					path: PathBuf::from(".bashrc"),
					priority: None,
					env: Some(Env { inner: item_env }),
					target: Some(DeployTarget::Path(PathBuf::from("/home/demo/.bashrc"))),
					merge: Some(MergeMode::Overwrite),
					template: Some(false),
				},
			],
		};

		let json = serde_json::to_string(&profile).unwrap();

		let parsed: Profile = serde_json::from_str(&json).unwrap();

		assert_eq!(parsed, profile);
	}
}
