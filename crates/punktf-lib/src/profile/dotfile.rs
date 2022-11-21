use serde::{Deserialize, Serialize};

use crate::profile::{transform::ContentTransformer, variables::Variables, MergeMode, Priority};

use std::path::PathBuf;

/// A dotfile represents a single item to be deployed by `punktf`. This can
/// either be a single file or a directory. This struct holds attributes to
/// control how the item will be deployed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Dotfile {
	/// Relative path inside the
	/// [`PunktfSource::dotfiles`](`crate::profile::source::PunktfSource::dotfiles`)
	/// directory.
	pub path: PathBuf,

	/// Alternative relative name/path for the dotfile. This name will be used
	/// instead of [`Dotfile::path`](`crate::profile::dotfile::Dotfile::path`)
	/// when deploying. If this is set and the
	/// dotfile is a directory, it will be deployed under the given name and
	/// not in the
	/// [`PunktfSource::root`](`crate::profile::source::PunktfSource::root`)
	/// directory.
	#[serde(skip_serializing_if = "Option::is_none", default)]
	pub rename: Option<PathBuf>,

	/// Alternative absolute deploy target path. This will be used instead of
	/// [`Profile::target`](`crate::profile::Profile::target`) when deploying.
	#[serde(skip_serializing_if = "Option::is_none", default)]
	pub overwrite_target: Option<PathBuf>,

	/// Priority of the dotfile. Dotfiles with higher priority as others are
	/// allowed to overwrite an already deployed dotfile if the
	/// [Dotfile::merge](`crate::profile::dotfile::Dotfile::merge`) allows for it.
	#[serde(skip_serializing_if = "Option::is_none", default)]
	pub priority: Option<Priority>,

	/// Variables specifically defined for this dotfile. These variables will
	/// take precendence over the ones defined in
	/// [`Profile::variables`](`crate::profile::Profile::variables`).
	#[serde(skip_serializing_if = "Option::is_none", default)]
	pub variables: Option<Variables>,

	/// Content transform defined for the dotfile. These variables will take
	/// precendence over the ones defined in
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
