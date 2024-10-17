//! Defines definitions for a [`Dotfile`] which is the basic building block
//! to define required/deployable items in a [`Profile`](`crate::profile::Profile`).

use serde::{Deserialize, Serialize};

use crate::profile::{transform::ContentTransformer, variables::Variables, MergeMode, Priority};

use std::path::PathBuf;

/// A dotfile represents a single item to be deployed by `punktf`. This can
/// either be a single file or a directory. This struct holds attributes to
/// control how the item will be deployed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Dotfile {
	/// Relative path inside the
	/// [`PunktfSource::dotfiles`](`crate::profile::source::PunktfSource::dotfiles`)
	/// directory.
	pub path: PathBuf,

	/// Used to overwrite the default target location of a dotfile.
	/// The resolved/actual output path of the [`Dotfile`] depends on the given path:
	///
	/// - If the given path is absolute, [`super::Profile::target`] will be completely ignored and this path will be used instead
	/// - If the given path is relative, it will be appended to [`super::Profile::target`]
	///
	/// NOTE: Additionally, setting this option, will completely ignore the relative path of the dotfile within the
	/// `dotfiles` folder for target path resolution.
	#[serde(skip_serializing_if = "Option::is_none", default)]
	pub target: Option<PathBuf>,

	/// Priority of the dotfile. Dotfiles with higher priority as others are
	/// allowed to overwrite an already deployed dotfile if the
	/// [Dotfile::merge](`crate::profile::dotfile::Dotfile::merge`) allows for it.
	#[serde(skip_serializing_if = "Option::is_none", default)]
	pub priority: Option<Priority>,

	/// Variables specifically defined for this dotfile. These variables will
	/// take precedence over the ones defined in
	/// [`Profile::variables`](`crate::profile::Profile::variables`).
	#[serde(skip_serializing_if = "Option::is_none", default)]
	pub variables: Option<Variables>,

	/// Content transform defined for the dotfile. These variables will take
	/// precedence over the ones defined in
	/// [`profile::Profile::transformers`](`crate::profile::Profile::transformers`).
	#[serde(skip_serializing_if = "Vec::is_empty", default)]
	pub transformers: Vec<ContentTransformer>,

	/// Merge operation for already existing dotfiles with the same or higher
	/// priority.
	#[serde(skip_serializing_if = "Option::is_none", default)]
	pub merge: Option<MergeMode>,

	/// Indicates if the dotfile should be treated as a template. If this is `false`
	/// no template processing will be done.
	#[serde(skip_serializing_if = "Option::is_none", default)]
	pub template: Option<bool>,
}

impl Dotfile {
	/// Checks if the dotfile is considered to be a template.
	pub fn is_template(&self) -> bool {
		self.template.unwrap_or(true)
	}
}
