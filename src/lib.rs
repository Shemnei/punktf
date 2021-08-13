#![feature(exit_status_error)]
#![feature(option_get_or_insert_default)]
#![feature(map_first_last)]
#![feature(path_try_exists)]
#![allow(dead_code)]
#![deny(
    deprecated_in_future,
    exported_private_dependencies,
    future_incompatible,
    missing_copy_implementations,
    // TODO(doc): rustdoc::missing_crate_level_docs,
    missing_debug_implementations,
    // TODO(doc): enable missing_docs,
    private_in_public,
    rust_2018_compatibility,
    rust_2018_idioms,
    trivial_casts,
    trivial_numeric_casts,
    unsafe_code,
    // TODO: would be nice in the future but not possible for now
	// unstable_features,
    unused_import_braces,
    unused_qualifications,

	// clippy attributes
	clippy::missing_const_for_fn,
	clippy::redundant_pub_crate,
	clippy::use_self
)]
#![cfg_attr(docsrs, feature(doc_cfg), feature(doc_alias))]

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PunktfSource {
	root: PathBuf,
	profiles: PathBuf,
	dotfiles: PathBuf,
}

impl PunktfSource {
	pub fn from_root(root: PathBuf) -> std::io::Result<Self> {
		let _ = root.try_exists()?;
		let root = root.canonicalize()?;

		let profiles = root.join("profiles");
		let _ = profiles.try_exists()?;
		let profiles = profiles.canonicalize()?;

		let dotfiles = root.join("dotfiles");
		let _ = dotfiles.try_exists()?;
		let dotfiles = dotfiles.canonicalize()?;

		Ok(Self {
			root,
			profiles,
			dotfiles,
		})
	}

	pub fn root(&self) -> &Path {
		&self.root
	}

	pub fn profiles(&self) -> &Path {
		&self.profiles
	}

	pub fn dotfiles(&self) -> &Path {
		&self.dotfiles
	}
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Profile {
	/// Defines the base profile. All settings from the base are merged with the
	/// current profile. The settings from the current profile take precendence.
	/// Dotfiles are merged on the dotfile level (not specific dotfile settings level).
	#[serde(skip_serializing_if = "Option::is_none", default)]
	extends: Option<String>,

	/// Variables of the profile. Each dotfile will have this environment.
	#[serde(skip_serializing_if = "Option::is_none", default)]
	variables: Option<UserVars>,

	/// Target root path of the deployment. Will be used as file stem for the dotfiles
	/// when not overwritten by [Dotfile::target].
	#[serde(skip_serializing_if = "Option::is_none", default)]
	target: Option<PathBuf>,

	/// Hook will be executed once before the deployment begins. If the hook fails
	/// the deployment will not be continued.
	#[serde(skip_serializing_if = "Vec::is_empty", default)]
	pre_hooks: Vec<Hook>,

	/// Hook will be executed once after the deployment begins.
	#[serde(skip_serializing_if = "Vec::is_empty", default)]
	post_hooks: Vec<Hook>,

	/// Dotfiles which will be deployed.
	#[serde(skip_serializing_if = "Vec::is_empty", default)]
	dotfiles: Vec<Dotfile>,
}

impl Profile {
	pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, std::io::Error> {
		let path = path.as_ref();
		let file = File::open(path)?;

		let extension = path.extension().ok_or_else(|| {
			std::io::Error::new(
				std::io::ErrorKind::InvalidData,
				"Failed to get file extension for profile",
			)
		})?;

		match extension.to_string_lossy().as_ref() {
			"json" => serde_json::from_reader(file).map_err(|err| err.to_string()),
			"yaml" | "yml" => serde_yaml::from_reader(file).map_err(|err| err.to_string()),
			_ => Err(String::from("Unsupported file extension for profile")),
		}
		.map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidData, err))
	}

	/// Merges everything from `other` into `self`.
	/// Fields from `self` have precendence over `other`.
	pub fn merge(&mut self, other: Self) {
		let Profile {
			extends,
			target,
			pre_hooks,
			post_hooks,
			dotfiles,
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

		let pre_unique = pre_hooks
			.into_iter()
			.filter(|h| !self.pre_hooks.contains(h))
			.collect::<Vec<_>>();
		self.pre_hooks.extend(pre_unique);

		let post_unique = post_hooks
			.into_iter()
			.filter(|h| !self.post_hooks.contains(h))
			.collect::<Vec<_>>();
		self.post_hooks.extend(post_unique);

		let self_dotfile_paths = self
			.dotfiles
			.iter()
			.map(|dotfile| &dotfile.path)
			.collect::<HashSet<_>>();
		let dotfiles = dotfiles
			.into_iter()
			.filter(|dotfile| !self_dotfile_paths.contains(&dotfile.path))
			.collect::<Vec<_>>();
		self.dotfiles.extend(dotfiles);
	}

	pub fn target(&self) -> Option<&Path> {
		self.target.as_deref()
	}

	pub fn set_target(&mut self, target: Option<PathBuf>) {
		self.target = target;
	}
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Dotfile {
	/// Relative path inside the `source` folder.
	path: PathBuf,

	/// Alternative name for the dotfile. This name will be used instead of [`Dotfile::path`] when
	/// deploying. If this is set and the dotfile is a folder, it will be deployed under the given
	/// name and not in the root source directory.
	#[serde(skip_serializing_if = "Option::is_none", default)]
	rename: Option<PathBuf>,

	/// Alternative deploy target path. This will be used instead of [`Profile::target`] when
	/// deploying.
	#[serde(skip_serializing_if = "Option::is_none", default)]
	overwrite_target: Option<PathBuf>,

	/// Priority of the dotfile. Dotfiles with higher priority as others are allowed
	/// to overwrite an dotfile deployed in this deployment.
	#[serde(skip_serializing_if = "Option::is_none", default)]
	priority: Option<Priority>,

	/// Variables for the dotfile. If a key is not found here, [Profile::env]
	/// will be searched.
	#[serde(skip_serializing_if = "Option::is_none", default)]
	variables: Option<UserVars>,

	/// Merge operation for already existing dotfiles with the same priority.
	#[serde(skip_serializing_if = "Option::is_none", default)]
	merge: Option<MergeMode>,

	/// Indicates if the dotfile should be treated as a template. If this is `false`
	/// no template processing will be done.
	#[serde(skip_serializing_if = "Option::is_none", default)]
	template: Option<bool>,
}

impl Dotfile {
	pub fn is_template(&self) -> bool {
		self.template.unwrap_or(true)
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MergeMode {
	/// Overwrites the existing dotfile.
	Overwrite,

	/// Keeps the existing dotfile.
	Keep,

	/// Asks the user for input to decide what to do.
	Ask,
}

impl Default for MergeMode {
	fn default() -> Self {
		Self::Overwrite
	}
}

#[derive(
	Default, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
pub struct Priority(u32);

impl Priority {
	pub const fn new(priority: u32) -> Self {
		Self(priority)
	}
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

		let mut dotfile_vars = HashMap::new();
		dotfile_vars.insert(String::from("RUSTC_VERSION"), String::from("55.22"));
		dotfile_vars.insert(String::from("USERNAME"), String::from("demo"));

		let profile = Profile {
			extends: None,
			variables: Some(UserVars {
				inner: profile_vars,
			}),
			target: Some(PathBuf::from("/home/demo/.config")),
			pre_hooks: vec![Hook::new("echo \"Foo\"")],
			post_hooks: vec![Hook::new("profiles/test.sh")],
			dotfiles: vec![
				Dotfile {
					path: PathBuf::from("init.vim.ubuntu"),
					rename: Some(PathBuf::from("init.vim")),
					overwrite_target: None,
					priority: Some(Priority::new(2)),
					variables: None,
					merge: Some(MergeMode::Overwrite),
					template: None,
				},
				Dotfile {
					path: PathBuf::from(".bashrc"),
					rename: None,
					overwrite_target: Some(PathBuf::from("/home/demo")),
					priority: None,
					variables: Some(UserVars {
						inner: dotfile_vars,
					}),
					merge: Some(MergeMode::Overwrite),
					template: Some(false),
				},
			],
		};

		let json = serde_json::to_string(&profile).expect("Profile to be serializeable");

		let parsed: Profile = serde_json::from_str(&json).expect("Profile to be deserializable");

		assert_eq!(parsed, profile);
	}
}
