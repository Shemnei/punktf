#![feature(exit_status_error)]
#![allow(dead_code)]

pub mod deploy;
pub mod hook;
pub mod variables;

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use variables::UserVars;

use crate::hook::Hook;

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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Profile {
	/// Environment of the profile. Each item will have this environment.
	#[serde(skip_serializing_if = "Option::is_none")]
	variables: Option<UserVars>,

	/// Target root path of the deployment. Will be used as file stem for the items
	/// when not overwritten by [Item::target].
	target: PathBuf,

	/// Hook will be executed once before the deployment begins. If the hook fails
	/// the deployment will not be continued.
	#[serde(skip_serializing_if = "Vec::is_empty", default)]
	pre_hooks: Vec<Hook>,

	/// Hook will be executed once after the deployment begins.
	#[serde(skip_serializing_if = "Vec::is_empty", default)]
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
	variables: Option<UserVars>,

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
	use std::collections::HashMap;

	use super::*;
	use crate::variables::UserVars;

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
			variables: Some(UserVars { inner: profile_env }),
			target: PathBuf::from("/home/demo/.config"),
			pre_hooks: vec![Hook::new("echo \"Foo\"")],
			post_hooks: vec![Hook::new("profiles/test.sh")],
			items: vec![
				Item {
					path: PathBuf::from("init.vim.ubuntu"),
					priority: Some(Priority::new(2)),
					variables: None,
					target: Some(DeployTarget::Alias(PathBuf::from("init.vim"))),
					merge: Some(MergeMode::Overwrite),
					template: None,
				},
				Item {
					path: PathBuf::from(".bashrc"),
					priority: None,
					variables: Some(UserVars { inner: item_env }),
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
