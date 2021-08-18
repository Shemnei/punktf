#![feature(exit_status_error)]
#![feature(option_get_or_insert_default)]
#![feature(map_first_last)]
#![feature(path_try_exists)]
#![feature(io_error_more)]
#![allow(dead_code, rustdoc::private_intra_doc_links)]
#![deny(
    deprecated_in_future,
    exported_private_dependencies,
    future_incompatible,
    missing_copy_implementations,
    //rustdoc::missing_crate_level_docs,
	rustdoc::broken_intra_doc_links,
    //missing_docs,
    //clippy::missing_docs_in_private_items,
    missing_debug_implementations,
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

//! This is the library powering `punktf`, a cross-platform multi-target dotfiles manager.
//!
//! The main features are:
//!
//! - [Templating engine](`template`)
//! - [Hooks](`hook`)

pub mod deploy;
pub mod hook;
pub mod profile;
pub mod template;
pub mod variables;

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use variables::UserVars;

/// This struct represents the source directory used by `punktf`. The source
/// directory is the central repository used to store
/// (profiles)[`profile::Profile`] and (dotfiles)[`Dotfile`]. `punktf` will
/// only read data from these directories but never write to them.
///
/// The current structure looks something like this:
///
/// ```text
/// root/
/// + profiles/
///   ...
/// + dotfiles/
///   ...
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PunktfSource {
	root: PathBuf,
	profiles: PathBuf,
	dotfiles: PathBuf,
}

impl PunktfSource {
	/// Creates a instance from a `root` directory. During instantiation it
	/// checks if the `root` exists and is a directory. These checks will also
	/// be run for the `root/profiles` and `root/dotfiles` subdirectories. All
	/// the above mentioned paths will also be resolved by calling
	/// [`std::path::Path::canonicalize`].
	///
	/// # Errors
	///
	/// If any of the checks fail an error will be returned.
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

	/// Returns the absolute path for the `root` directory.
	pub fn root(&self) -> &Path {
		&self.root
	}

	/// Returns the absolute path to the `root/profiles` directory.
	pub fn profiles(&self) -> &Path {
		&self.profiles
	}

	/// Returns the absolute path to the `root/dotfiles` directory.
	pub fn dotfiles(&self) -> &Path {
		&self.dotfiles
	}

	/// Tries to resolve a profile name to a path of a
	/// (profile)[profile::Profile]. The profile name must be given without any
	/// file extension attached (e.g. `demo` instead of `demo.json`).
	///
	/// # Errors
	///
	/// Errors if no profile matching the name was found.
	/// Errors if multiple profiles matching the name were found.
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
				format!("Found more than one profile with the name `{}`", name),
			))
		} else if let Some(profile_path) = matching_profile_paths.pop() {
			Ok(profile_path)
		} else {
			Err(std::io::Error::new(
				std::io::ErrorKind::NotFound,
				format!("Found no profile with the name `{}`", name),
			))
		}
	}
}

/// A dotfile represents a single item to be deployed by `punktf`. This can
/// either be a single file or a directory. This struct holds attributes to
/// control how the item will be deployed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Dotfile {
	/// Relative path inside the [`PunktfSource::dotfiles`] directory.
	path: PathBuf,

	/// Alternative relative name/path for the dotfile. This name will be used
	/// instead of [`Dotfile::path`] when deploying. If this is set and the
	/// dotfile is a directory, it will be deployed under the given name and
	/// not in the [`PunktfSource::root`] directory.
	#[serde(skip_serializing_if = "Option::is_none", default)]
	rename: Option<PathBuf>,

	/// Alternative absolute deploy target path. This will be used instead of
	/// [`profile::Profile::target`] when deploying.
	#[serde(skip_serializing_if = "Option::is_none", default)]
	overwrite_target: Option<PathBuf>,

	/// Priority of the dotfile. Dotfiles with higher priority as others are
	/// allowed to overwrite an already deployed dotfile if the
	/// [Dotfile::merge] allows for it.
	#[serde(skip_serializing_if = "Option::is_none", default)]
	priority: Option<Priority>,

	/// Variables specifically defined for this dotfile. These variables will
	/// take precendence over the ones defined in
	/// [`profile::Profile::variables`].
	#[serde(skip_serializing_if = "Option::is_none", default)]
	variables: Option<UserVars>,

	/// Merge operation for already existing dotfiles with the same or higher
	/// priority.
	#[serde(skip_serializing_if = "Option::is_none", default)]
	merge: Option<MergeMode>,

	/// Indicates if the dotfile should be treated as a template. If this is `false`
	/// no template processing will be done.
	#[serde(skip_serializing_if = "Option::is_none", default)]
	template: Option<bool>,
}

impl Dotfile {
	/// Checks if the dotfile is considered to be a template.
	pub fn is_template(&self) -> bool {
		self.template.unwrap_or(true)
	}
}

/// This enum represents all available merge modes `punktf` supports. The merge
/// mode is important when a file already exists at the target location of a
/// [`Dotfile`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MergeMode {
	/// Overwrites the existing file.
	Overwrite,

	/// Keeps the existing file.
	Keep,

	/// Asks the user for input to decide what to do.
	Ask,
}

impl Default for MergeMode {
	fn default() -> Self {
		Self::Overwrite
	}
}

/// This struct represents the priority a [`Dotfile`] can have.  A bigger value
/// means a higher priority. [`Dotfile`]'s with lower priority won't be able to
/// overwrite already deployed dotfiles with a higher one.
#[derive(
	Default, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
pub struct Priority(u32);

impl Priority {
	/// Creates a new instance with the given `priority`.
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
