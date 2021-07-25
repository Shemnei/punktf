#![feature(exit_status_error)]
#![feature(array_windows)]
#![allow(dead_code)]

pub mod deploy;

use std::borrow::Cow;
use std::collections::{HashMap, VecDeque};
use std::error::Error;
use std::fmt;
use std::io::{BufRead as _, BufReader};
use std::path::PathBuf;
use std::process::{Command, Stdio};

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

#[derive(Default, Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RangeMap(Vec<usize>);

impl RangeMap {
	pub fn new<I: IntoIterator<Item = usize>>(items: I) -> Self {
		let items: Vec<usize> = items.into_iter().collect();

		// TODO: make err
		assert_eq!(items.len() % 2, 0, "Unclosed range");

		Self(items)
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

		let mut cmd = Command::new(parts.pop_front().unwrap());
		cmd.args(parts);
		cmd
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
	#[serde(skip_serializing_if = "Vec::is_empty")]
	pre_hooks: Vec<Hook>,

	/// Hook will be executed once after the deployment begins.
	#[serde(skip_serializing_if = "Vec::is_empty")]
	post_hooks: Vec<Hook>,

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

impl Item {
	pub fn template_or_default(&self) -> bool {
		self.template.unwrap_or(false)
	}
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

impl Default for MergeMode {
	fn default() -> Self {
		Self::Keep
	}
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
