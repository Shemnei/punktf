//! Defines profiles and ways to layer multiple of them.

pub mod dotfile;
pub mod hook;
pub mod link;
pub mod source;
pub mod transform;
pub mod variables;

use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::ops::Deref;
use std::path::{Path, PathBuf};

use color_eyre::eyre::{bail, eyre, Context};
use color_eyre::Result;
use serde::{Deserialize, Serialize};

use crate::profile::hook::Hook;
use crate::profile::link::Symlink;
use crate::profile::transform::ContentTransformer;
use crate::profile::variables::{Variables, Vars};
use crate::profile::{dotfile::Dotfile, source::PunktfSource};

/// This enum represents all available merge modes `punktf` supports. The merge
/// mode is important when a file already exists at the target location of a
/// [`Dotfile`](`crate::profile::dotfile::Dotfile`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
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

/// This struct represents the priority a
/// [`Dotfile`](`crate::profile::dotfile::Dotfile`)
/// can have. A bigger value means a higher priority. Dotfiles with lower priority
/// won't be able to overwrite already deployed dotfiles with a higher one.
#[derive(
	Default, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(deny_unknown_fields)]
pub struct Priority(pub u32);

impl Priority {
	/// Creates a new instance with the given `priority`.
	pub const fn new(priority: u32) -> Self {
		Self(priority)
	}
}

/// A profile is a collection of dotfiles and variables, options and hooks.
#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Profile {
	/// Aliases for this profile which can be used instead of the file name.
	#[serde(skip_serializing_if = "Vec::is_empty", default)]
	pub aliases: Vec<String>,

	/// Defines the base profile. All settings from the base are merged with the
	/// current profile. The settings from the current profile take precedence.
	/// Dotfiles are merged on the dotfile level (not specific dotfile settings level).
	#[serde(skip_serializing_if = "Vec::is_empty", default)]
	pub extends: Vec<String>,

	/// Variables of the profile. Each dotfile will have this environment.
	#[serde(skip_serializing_if = "Option::is_none", default)]
	pub variables: Option<Variables>,

	/// Content transform of the profile. Each dotfile will have these applied.
	#[serde(skip_serializing_if = "Vec::is_empty", default)]
	pub transformers: Vec<ContentTransformer>,

	/// Target root path of the deployment. Will be used as file stem for the dotfiles
	/// when not overwritten by
	/// [`Dotfile::overwrite_target`](`crate::profile::dotfile::Dotfile::overwrite_target`).
	#[serde(skip_serializing_if = "Option::is_none", default)]
	pub target: Option<PathBuf>,

	/// Hook will be executed once before the deployment begins. If the hook fails
	/// the deployment will not be continued.
	#[serde(skip_serializing_if = "Vec::is_empty", default)]
	pub pre_hooks: Vec<Hook>,

	/// Hook will be executed once after the deployment begins.
	#[serde(skip_serializing_if = "Vec::is_empty", default)]
	pub post_hooks: Vec<Hook>,

	/// Dotfiles which will be deployed.
	#[serde(skip_serializing_if = "Vec::is_empty", default)]
	pub dotfiles: Vec<Dotfile>,

	/// Symlinks which will be deployed.
	#[serde(rename = "links", skip_serializing_if = "Vec::is_empty", default)]
	pub symlinks: Vec<Symlink>,
}

impl Profile {
	/// Tries to load a profile from the file located at `path`.
	///
	/// This function will try to guess the correct deserializer by the file
	/// extension of `path`
	///
	/// # Errors
	///
	/// An error is returned if the file does not exist or could not be read.
	/// An error is returned if the file extension is unknown or missing.
	pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
		let path = path.as_ref();

		/// Inner function is used to reduce monomorphizes as path here is a
		/// concrete type and no generic one.
		fn from_file_inner(path: &Path) -> Result<Profile> {
			// Allowed in case no feature is present.
			#[allow(unused_variables)]
			let file = File::open(path)?;

			let extension = path.extension().ok_or_else(|| {
				std::io::Error::new(
					std::io::ErrorKind::InvalidData,
					"Failed to get file extension for profile",
				)
			})?;

			#[cfg(feature = "profile-json")]
			{
				if extension.eq_ignore_ascii_case("json") {
					return Profile::from_json_file(file);
				}
			}

			#[cfg(feature = "profile-yaml")]
			{
				if extension.eq_ignore_ascii_case("yaml") || extension.eq_ignore_ascii_case("yml") {
					return Profile::from_yaml_file(file);
				}
			}

			Err(eyre!(
				"Found unsupported file extension for profile (extension: {:?})",
				extension
			))
		}

		from_file_inner(path).wrap_err(format!(
			"Failed to process profile at path `{}`",
			path.display()
		))
	}

	/// Tries to load a profile from a json file.
	#[cfg(feature = "profile-json")]
	fn from_json_file(file: File) -> Result<Self> {
		serde_json::from_reader(&file).map_err(|err| {
			color_eyre::Report::msg(err).wrap_err("Failed to parse profile from json content.")
		})
	}

	/// Tries to load a profile from a yaml file.
	#[cfg(feature = "profile-yaml")]
	fn from_yaml_file(file: File) -> Result<Self> {
		serde_yaml::from_reader(file).map_err(|err| {
			color_eyre::Report::msg(err).wrap_err("Failed to parse profile from yaml content.")
		})
	}
}

/// Stores variables defined on different layers.
/// Layers are created when a profile is extended.
#[derive(Default, Debug, Clone, PartialEq, Eq)]
pub struct LayeredVariables {
	/// Stores the variables together with the index, which indexed
	/// [`LayeredProfile::profile_names`](`crate::profile::LayeredProfile::profile_names`)
	/// to retrieve the name of the profile, the variable came from.
	pub inner: HashMap<String, (usize, String)>,
}

impl Vars for LayeredVariables {
	fn var<K>(&self, key: K) -> Option<&str>
	where
		K: AsRef<str>,
	{
		self.inner.get(key.as_ref()).map(|(_, value)| value.deref())
	}
}

/// Defines a profile that appears on different layers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayeredProfile {
	/// All names of the profile which where collected from the extend chain.
	pub profile_names: Vec<String>,

	/// The target of the deployment.
	///
	/// This is the first value found by traversing the extend chain from the
	/// top.
	pub target: Option<(usize, PathBuf)>,

	/// The variables collected from all profiles of the extend chain.
	pub variables: LayeredVariables,

	/// The content transformer collected from all profiles of the extend chain.
	pub transformers: Vec<(usize, ContentTransformer)>,

	/// The pre-hooks collected from all profiles of the extend chain.
	pub pre_hooks: Vec<(usize, Hook)>,

	/// The post-hooks collected from all profiles of the extend chain.
	pub post_hooks: Vec<(usize, Hook)>,

	/// The dotfiles collected from all profiles of the extend chain.
	///
	/// The index indexes into
	/// [`LayeredProfile::profile_names`](`crate::profile::LayeredProfile::profile_names`)
	/// to retrieve the name of the profile from which the dotfile came from.
	pub dotfiles: Vec<(usize, Dotfile)>,

	/// The symlinks collected from all profiles of the extend chain.
	///
	/// The index indexes into
	/// [`LayeredProfile::profile_names`](`crate::profile::LayeredProfile::profile_names`)
	/// to retrieve the name of the profile from which the link came from.
	pub symlinks: Vec<(usize, Symlink)>,
}

impl LayeredProfile {
	/// Creates a new builder for a layered profile.
	pub fn build() -> LayeredProfileBuilder {
		LayeredProfileBuilder::default()
	}

	/// Returns the target path for the profile together with the index into
	/// [`LayeredProfile::profile_names`](`crate::profile::LayeredProfile::profile_names`).
	pub fn target(&self) -> Option<(&str, &Path)> {
		self.target
			.as_ref()
			.map(|(name_idx, path)| (self.profile_names[*name_idx].as_ref(), path.deref()))
	}

	/// Returns the target path for the profile.
	pub fn target_path(&self) -> Option<&Path> {
		self.target.as_ref().map(|(_, path)| path.deref())
	}

	/// Returns all collected variables for the profile.
	pub const fn variables(&self) -> &LayeredVariables {
		&self.variables
	}

	/// Returns all the count of collected transformers for the profile.
	pub fn transformers_len(&self) -> usize {
		self.transformers.len()
	}

	/// Returns all collected content transformer for the profile.
	pub fn transformers(&self) -> impl Iterator<Item = &ContentTransformer> {
		self.transformers.iter().map(|(_, transformer)| transformer)
	}

	/// Returns all collected pre-hooks for the profile.
	pub fn pre_hooks(&self) -> impl Iterator<Item = &Hook> {
		self.pre_hooks.iter().map(|(_, hook)| hook)
	}

	/// Returns all collected post-hooks for the profile.
	pub fn post_hooks(&self) -> impl Iterator<Item = &Hook> {
		self.post_hooks.iter().map(|(_, hook)| hook)
	}

	/// Returns all collected dotfiles for the profile.
	pub fn dotfiles(&self) -> impl Iterator<Item = &Dotfile> {
		self.dotfiles.iter().map(|(_, dotfile)| dotfile)
	}

	/// Returns all collected symlinks for the profile.
	pub fn symlinks(&self) -> impl Iterator<Item = &Symlink> {
		self.symlinks.iter().map(|(_, symlink)| symlink)
	}
}

/// Collects different profiles from multiple layers.
#[derive(Default, Debug, Clone, PartialEq, Eq)]
pub struct LayeredProfileBuilder {
	/// All names of the profile which where collected from the extend chain.
	profile_names: Vec<String>,

	/// The profiles which make up the layered profile. The first is the root
	/// profile from which the others where imported.
	profiles: Vec<Profile>,
}

impl LayeredProfileBuilder {
	/// Adds a new `profile` with the given `name` to the builder.
	pub fn add(&mut self, name: String, profile: Profile) -> &mut Self {
		self.profiles.push(profile);
		self.profile_names.push(name);

		self
	}

	/// Consumes self and returns a new layered profile.
	pub fn finish(self) -> LayeredProfile {
		let target = self.profiles.iter().enumerate().find_map(|(idx, profile)| {
			profile
				.target
				.as_ref()
				.map(move |target| (idx, target.to_path_buf()))
		});

		let mut variables = LayeredVariables::default();

		for (idx, vars) in self
			.profiles
			.iter()
			.enumerate()
			.filter_map(move |(idx, profile)| profile.variables.as_ref().map(|vars| (idx, vars)))
		{
			for (key, value) in vars.inner.iter() {
				if !variables.inner.contains_key(key) {
					variables
						.inner
						.insert(key.to_owned(), (idx, value.to_owned()));
				}
			}
		}

		let mut transformers = Vec::new();

		for (idx, transformer) in self
			.profiles
			.iter()
			.enumerate()
			.map(|(idx, profile)| (idx, &profile.transformers))
		{
			for t in transformer.iter() {
				if !transformers.iter().any(|(_, tt)| t == tt) {
					transformers.push((idx, *t));
				}
			}
		}

		let pre_hooks = self
			.profiles
			.iter()
			.enumerate()
			.flat_map(|(idx, profile)| {
				profile
					.pre_hooks
					.iter()
					.cloned()
					.map(move |hook| (idx, hook))
			})
			.collect();

		let post_hooks = self
			.profiles
			.iter()
			.enumerate()
			.flat_map(|(idx, profile)| {
				profile
					.post_hooks
					.iter()
					.cloned()
					.map(move |hook| (idx, hook))
			})
			.collect();

		let mut added_dotfile_paths = HashSet::new();
		let mut dotfiles = Vec::new();

		for (idx, dfiles) in self
			.profiles
			.iter()
			.enumerate()
			.map(|(idx, profile)| (idx, &profile.dotfiles))
		{
			for dotfile in dfiles.iter() {
				if !added_dotfile_paths.contains(&dotfile.path) {
					dotfiles.push((idx, dotfile.clone()));
					added_dotfile_paths.insert(dotfile.path.clone());
				}
			}
		}

		let symlinks = self
			.profiles
			.iter()
			.enumerate()
			.flat_map(|(idx, profile)| {
				profile
					.symlinks
					.iter()
					.cloned()
					.map(move |link| (idx, link))
			})
			.collect();

		LayeredProfile {
			profile_names: self.profile_names,
			target,
			variables,
			transformers,
			pre_hooks,
			post_hooks,
			dotfiles,
			symlinks,
		}
	}
}

/// A minimal struct to read the `aliases` from a profile file.
///
/// This is used for profile name resolution.
#[derive(Default, Debug, Serialize, Deserialize)]
#[serde(default)]
struct Aliases {
	/// Aliases of a profile.
	///
	/// These can be used in place of the profile name for cli and extend resolution.
	aliases: Vec<String>,
}

/// Collects all profile names and aliases from the `profiles` directory.
pub fn collect_profile_names(source: &PunktfSource) -> Result<HashMap<String, PathBuf>> {
	log::info!("Collecting profile names and aliases");

	/// Tries to read all alias from a given file.
	fn get_aliases(path: &Path, extension: &str) -> Option<Aliases> {
		let Ok(file) = File::open(path) else {
			log::debug!("[{}] Failed to read content", path.display());
			return None;
		};

		#[cfg(feature = "profile-json")]
		{
			if extension.eq_ignore_ascii_case("json") {
				let Ok(aliases) = serde_json::from_reader(file) else {
					log::debug!("[{}] Failed to read aliases", path.display());
					return None;
				};

				return Some(aliases);
			}
		}

		#[cfg(feature = "profile-yaml")]
		{
			if extension.eq_ignore_ascii_case("yaml") || extension.eq_ignore_ascii_case("yml") {
				let Ok(aliases) = serde_yaml::from_reader(file) else {
					log::debug!("[{}] Failed to read aliases", path.display());
					return None;
				};

				return Some(aliases);
			}
		}

		None
	}

	let mut names = HashMap::new();

	let dents = source.profiles().read_dir()?;
	for dent in dents {
		let dent = dent?;
		let path = dent.path();

		let Ok(ft) = dent.file_type() else {
			log::debug!("[{}] Failed to get file type", path.display());
			continue;
		};

		if !ft.is_file() {
			log::debug!("[{}] Not a file", path.display());
			continue;
		}

		let Some(extension) = path.extension().and_then(|e| e.to_str()) else {
			log::debug!("[{}] Failed to get file extension", path.display());
			continue;
		};

		let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
			log::debug!("[{}] Failed to get file name", path.display());
			continue;
		};
		// Remove extension and `.`
		let name = &name[..(name.len() - (extension.len() + 1))];

		for alias in get_aliases(&path, extension)
			.into_iter()
			.flat_map(|a| a.aliases.into_iter())
		{
			log::debug!("[{}] Adding alias {}", path.display(), alias);

			if let Some(evicted) = names.insert(alias.clone(), path.clone()) {
				bail!(
					"[{}] The profile alias {} is already taken by {}",
					path.display(),
					alias,
					evicted.display()
				);
			}
		}

		if let Some(evicted) = names.insert(name.to_string(), path.clone()) {
			bail!(
				"[{}] The profile name {} is already taken by {}",
				path.display(),
				name,
				evicted.display()
			);
		}
	}

	log::info!("Found {} profile names and aliases", names.len());

	Ok(names)
}

/// Recursively resolves a profile and it's [extend
/// chain](`crate::profile::Profile::extends`) and adds them to the layered
/// profile in order of occurrence.
pub fn resolve_profile(
	builder: &mut LayeredProfileBuilder,
	source: &PunktfSource,
	name: &str,
) -> Result<()> {
	/// Recursive resolution of all profiles needed.
	///
	/// Checks for cycles while resolving.
	fn _resolve_profile_inner(
		profiles: &HashMap<String, PathBuf>,
		builder: &mut LayeredProfileBuilder,
		name: &str,
		resolved_profiles: &mut Vec<String>,
	) -> Result<()> {
		log::trace!("Resolving profile `{}`", name);

		let path = profiles
			.get(name)
			.ok_or_else(|| eyre!("No profile found for name {}", name))?;

		let mut profile = Profile::from_file(path)?;
		let name = name.to_string();

		if !profile.extends.is_empty() && resolved_profiles.contains(&name) {
			// profile was already resolve and has "children" which will lead to
			// a loop while resolving
			return Err(eyre!(
			"Circular dependency detected while parsing `{}` (required by: `{:?}`) (Stack: {:#?})",
			name,
			resolved_profiles.last(),
			resolved_profiles
		));
		}

		let mut extends = Vec::new();
		std::mem::swap(&mut extends, &mut profile.extends);

		builder.add(name.clone(), profile);

		resolved_profiles.push(name);

		for child in extends {
			_resolve_profile_inner(profiles, builder, &child, resolved_profiles)?;
		}

		let _ = resolved_profiles
			.pop()
			.expect("Misaligned push/pop operation");

		Ok(())
	}

	let available_profiles = collect_profile_names(source)?;
	let mut resolved_profiles = Vec::new();

	_resolve_profile_inner(&available_profiles, builder, name, &mut resolved_profiles)
}

#[cfg(test)]
mod tests {
	use std::collections::HashMap;

	use super::*;
	use crate::profile::hook::Hook;
	use crate::profile::variables::Variables;
	use crate::profile::Profile;
	use crate::profile::{MergeMode, Priority};

	#[test]
	fn priority_order() {
		crate::tests::setup_test_env();

		assert!(Priority::default() == Priority::new(0));
		assert!(Priority::new(0) == Priority::new(0));
		assert!(Priority::new(2) > Priority::new(1));
	}

	#[test]
	#[cfg(feature = "profile-json")]
	fn profile_serde() {
		crate::tests::setup_test_env();

		let mut profile_vars = HashMap::new();
		profile_vars.insert(String::from("RUSTC_VERSION"), String::from("XX.YY"));
		profile_vars.insert(String::from("RUSTC_PATH"), String::from("/usr/bin/rustc"));

		let mut dotfile_vars = HashMap::new();
		dotfile_vars.insert(String::from("RUSTC_VERSION"), String::from("55.22"));
		dotfile_vars.insert(String::from("USERNAME"), String::from("demo"));

		let profile = Profile {
			extends: Vec::new(),
			aliases: vec![],
			variables: Some(Variables {
				inner: profile_vars,
			}),
			transformers: Vec::new(),
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
					transformers: Vec::new(),
					merge: Some(MergeMode::Overwrite),
					template: None,
				},
				Dotfile {
					path: PathBuf::from(".bashrc"),
					rename: None,
					overwrite_target: Some(PathBuf::from("/home/demo")),
					priority: None,
					variables: Some(Variables {
						inner: dotfile_vars,
					}),
					transformers: Vec::new(),
					merge: Some(MergeMode::Overwrite),
					template: Some(false),
				},
			],
			symlinks: vec![],
		};

		let json = serde_json::to_string(&profile).expect("Profile to be serializeable");

		let parsed: Profile = serde_json::from_str(&json).expect("Profile to be deserializable");

		assert_eq!(parsed, profile);
	}
}
