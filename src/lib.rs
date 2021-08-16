#![feature(exit_status_error)]
#![feature(option_get_or_insert_default)]
#![feature(map_first_last)]
#![feature(path_try_exists)]
#![feature(io_error_more)]
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
    // TODO(future): would be nice in the future but not possible for now
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
pub mod profile;
pub mod template;
pub mod variables;

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use variables::UserVars;

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

	pub fn find_profile_path(&self, name: &str) -> std::io::Result<PathBuf> {
		let name = name.to_lowercase();

		let mut matching_profile_paths = walkdir::WalkDir::new(&self.profiles)
			.max_depth(1)
			.into_iter()
			.filter_map(|dent| {
				let dent = dent.ok()?;
				let dent_name = dent.file_name().to_string_lossy();

				if let Some(dot_idx) = dent_name.rfind('.') {
					(name == dent_name[..dot_idx].to_lowercase())
						.then(move || dent.path().to_path_buf())
				} else {
					None
				}
			})
			.collect::<Vec<_>>();

		if matching_profile_paths.len() > 1 {
			Err(std::io::Error::new(
				std::io::ErrorKind::InvalidData,
				format!("Found more than one profile with the name {}", name),
			))
		} else if let Some(profile_path) = matching_profile_paths.pop() {
			Ok(profile_path)
		} else {
			Err(std::io::Error::new(
				std::io::ErrorKind::NotFound,
				format!("Found no profile with the name {}", name),
			))
		}
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

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn priority_order() {
		assert!(Priority::default() == Priority::new(0));
		assert!(Priority::new(0) == Priority::new(0));
		assert!(Priority::new(2) > Priority::new(1));
	}
}
