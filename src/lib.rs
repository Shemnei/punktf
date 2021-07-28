#![feature(exit_status_error)]
#![allow(dead_code)]

pub mod deploy;
pub mod hook;
pub mod template;
pub mod variables;

use std::collections::HashSet;
use std::fs::File;
use std::path::{Path, PathBuf};

use color_eyre::eyre::{eyre, Context, Result};
use serde::{Deserialize, Serialize};
use variables::UserVars;

use crate::hook::Hook;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Profile {
	/// Defines the base profile. All settings from the base are merged with the
	/// current profile. The settings from the current profile take precendence.
	/// Items are merged on the item level (not specific item settings level).
	extends: Option<String>,

	/// Variables of the profile. Each item will have this environment.
	#[serde(skip_serializing_if = "Option::is_none")]
	variables: Option<UserVars>,

	/// Target root path of the deployment. Will be used as file stem for the items
	/// when not overwritten by [Item::target].
	target: Option<PathBuf>,

	/// Hook will be executed once before the deployment begins. If the hook fails
	/// the deployment will not be continued.
	#[serde(skip_serializing_if = "Vec::is_empty", default)]
	pre_hooks: Vec<Hook>,

	/// Hook will be executed once after the deployment begins.
	#[serde(skip_serializing_if = "Vec::is_empty", default)]
	post_hooks: Vec<Hook>,

	/// Items which will be deployed.
	#[serde(skip_serializing_if = "Vec::is_empty", default)]
	items: Vec<Item>,
}

impl Profile {
	pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, std::io::Error> {
		// TODO: cleanup

		let path = path.as_ref();
		let file = File::open(path)?;

		let extension = path.extension().ok_or_else(|| {
			std::io::Error::new(
				std::io::ErrorKind::NotFound,
				"Failed to get file extension for profile",
			)
		})?;

		match extension.to_string_lossy().as_ref() {
			"json" => serde_json::from_reader(file).map_err(|err| err.to_string()),
			"yaml" | "yml" => serde_yaml::from_reader(file).map_err(|err| err.to_string()),
			&_ => Err(String::from("Invalid file extension for profile")),
		}
		.map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidData, err))
	}

	/// Merges everything from `other` into `self`.
	/// Fields from `self` have precendence over `other`.
	pub fn merge(&mut self, other: Profile) {
		let Profile {
			extends,
			target,
			pre_hooks,
			post_hooks,
			items,
			variables,
		} = other;

		self.extends = extends;

		match (&mut self.variables, variables) {
			(Some(self_vars), Some(other_vars)) => self_vars.merge(other_vars),
			(Some(_), None) => {}
			(None, Some(other_vars)) => self.variables = Some(other_vars),
			(None, None) => {}
		};

		if self.target.is_none() {
			self.target = target;
		}

		// TODO: elimiate same hooks???
		self.pre_hooks.extend(pre_hooks.into_iter());
		self.post_hooks.extend(post_hooks.into_iter());

		let self_item_paths = self
			.items
			.iter()
			.map(|item| &item.path)
			.collect::<HashSet<_>>();
		let items = items
			.into_iter()
			.filter(|item| !self_item_paths.contains(&item.path))
			.collect::<Vec<_>>();
		self.items.extend(items);
	}
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Item {
	/// Relative path inside the `source` folder.
	path: PathBuf,

	/// Priority of the item. Items with higher priority as others are allowed
	/// to overwrite an item deployed in this deployment.
	#[serde(skip_serializing_if = "Option::is_none")]
	priority: Option<Priority>,

	/// Variables for the item. If a key is not found here, [Profile::env]
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
	pub fn is_template(&self) -> bool {
		self.template.unwrap_or(true)
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

fn get_target_path() -> PathBuf {
	std::env::var_os("PUNKTF_TARGET")
		.expect(
			"No environment variable `PUNKTF_TARGET` set. Either set this variable or use the \
			 profile variable `target`.",
		)
		.into()
}

fn find_profile_path(profile_path: &Path, name: &str) -> Result<PathBuf> {
	let name = name.to_lowercase();

	profile_path
		.read_dir()
		.wrap_err("Failed to read profile directory")?
		.filter_map(|dent| dent.ok().map(|dent| dent.path()))
		.find(|path| {
			let file_name = match path.file_name() {
				Some(file_name) => file_name.to_string_lossy(),
				None => return false,
			};

			if let Some(dot_idx) = file_name.rfind('.') {
				name == file_name[..dot_idx].to_lowercase()
			} else {
				false
			}
		})
		.ok_or_else(|| eyre!("No matching profile found"))
}

pub fn resolve_profile(profile_path: &Path, name: &str) -> Result<Profile> {
	let mut profiles = HashSet::new();

	let mut root = Profile::from_file(find_profile_path(profile_path, name)?)?;
	profiles.insert(name.to_string().to_lowercase());

	while let Some(base_name) = root.extends.clone() {
		let base_name = base_name.to_lowercase();

		log::info!("Resolving dependency `{}`", base_name);

		if profiles.contains(&base_name) {
			log::warn!("Circular dependency on `{}` detect", base_name);
			break;
		}

		let path = find_profile_path(profile_path, &base_name)?;
		log::debug!("Path for profile `{}`: {}", base_name, path.display());

		let profile = Profile::from_file(path)?;

		log::debug!("Profile `{}`: {:#?}", base_name, profile);

		root.merge(profile);

		profiles.insert(base_name);
	}

	Ok(root)
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
		let mut profile_vars = HashMap::new();
		profile_vars.insert(String::from("RUSTC_VERSION"), String::from("XX.YY"));
		profile_vars.insert(String::from("RUSTC_PATH"), String::from("/usr/bin/rustc"));

		let mut item_vars = HashMap::new();
		item_vars.insert(String::from("RUSTC_VERSION"), String::from("55.22"));
		item_vars.insert(String::from("USERNAME"), String::from("demo"));

		let profile = Profile {
			extends: None,
			variables: Some(UserVars {
				inner: profile_vars,
			}),
			target: Some(PathBuf::from("/home/demo/.config")),
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
					variables: Some(UserVars { inner: item_vars }),
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
