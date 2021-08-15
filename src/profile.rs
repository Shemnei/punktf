use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::ffi::OsString;
use std::fs::File;
use std::ops::Deref;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::hook::Hook;
use crate::variables::{UserVars, Variables};
use crate::{Dotfile, PunktfSource};

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SimpleProfile {
	/// Defines the base profile. All settings from the base are merged with the
	/// current profile. The settings from the current profile take precendence.
	/// Dotfiles are merged on the dotfile level (not specific dotfile settings level).
	#[serde(skip_serializing_if = "Vec::is_empty", default)]
	pub extends: Vec<String>,

	/// Variables of the profile. Each dotfile will have this environment.
	#[serde(skip_serializing_if = "Option::is_none", default)]
	pub variables: Option<UserVars>,

	/// Target root path of the deployment. Will be used as file stem for the dotfiles
	/// when not overwritten by [Dotfile::target].
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
}

impl SimpleProfile {
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
}

#[derive(Default, Debug, Clone, PartialEq, Eq)]
pub struct LayeredUserVars {
	pub inner: HashMap<String, (usize, String)>,
}

impl Variables for LayeredUserVars {
	fn var<K>(&self, key: K) -> Option<Cow<'_, str>>
	where
		K: AsRef<str>,
	{
		self.inner
			.get(key.as_ref())
			.map(|(_, value)| Cow::Borrowed(value.as_ref()))
	}
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayeredProfile {
	profile_names: Vec<String>,
	target: Option<(usize, PathBuf)>,
	variables: LayeredUserVars,
	pre_hooks: Vec<(usize, Hook)>,
	post_hooks: Vec<(usize, Hook)>,
	dotfiles: Vec<(usize, Dotfile)>,
}

impl LayeredProfile {
	pub fn build() -> LayeredProfileBuilder {
		LayeredProfileBuilder::default()
	}

	pub fn target(&self) -> Option<(&str, &Path)> {
		self.target
			.as_ref()
			.map(|(name_idx, path)| (self.profile_names[*name_idx].as_ref(), path.deref()))
	}

	pub fn target_path(&self) -> Option<&Path> {
		self.target.as_ref().map(|(_, path)| path.deref())
	}

	pub const fn variables(&self) -> &LayeredUserVars {
		&self.variables
	}

	pub fn pre_hooks(&self) -> impl Iterator<Item = &Hook> {
		self.pre_hooks.iter().map(|(_, hook)| hook)
	}

	pub fn post_hooks(&self) -> impl Iterator<Item = &Hook> {
		self.post_hooks.iter().map(|(_, hook)| hook)
	}

	pub fn dotfiles(&self) -> impl Iterator<Item = &Dotfile> {
		self.dotfiles.iter().map(|(_, dotfile)| dotfile)
	}
}

#[derive(Default, Debug, Clone, PartialEq, Eq)]
pub struct LayeredProfileBuilder {
	profile_names: Vec<String>,
	profiles: Vec<SimpleProfile>,
}

impl LayeredProfileBuilder {
	pub fn add(&mut self, name: String, profile: SimpleProfile) -> &mut Self {
		self.profiles.push(profile);
		self.profile_names.push(name);

		self
	}

	pub fn finish(self) -> LayeredProfile {
		let target = self.profiles.iter().enumerate().find_map(|(idx, profile)| {
			profile
				.target
				.as_ref()
				.map(move |target| (idx, target.to_path_buf()))
		});

		let mut variables = LayeredUserVars::default();

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

		let mut pre_hooks = Vec::new();

		for (idx, hooks) in self
			.profiles
			.iter()
			.enumerate()
			.map(|(idx, profile)| (idx, &profile.pre_hooks))
		{
			for hook in hooks.iter().cloned() {
				pre_hooks.push((idx, hook));
			}
		}

		let mut post_hooks = Vec::new();

		for (idx, hooks) in self
			.profiles
			.iter()
			.enumerate()
			.map(|(idx, profile)| (idx, &profile.post_hooks))
		{
			for hook in hooks.iter().cloned() {
				post_hooks.push((idx, hook));
			}
		}

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

		LayeredProfile {
			profile_names: self.profile_names,
			target,
			variables,
			pre_hooks,
			post_hooks,
			dotfiles,
		}
	}
}

pub fn resolve_profile(
	builder: &mut LayeredProfileBuilder,
	source: &PunktfSource,
	name: &str,
	resolved_profiles: &mut Vec<OsString>,
) -> std::io::Result<()> {
	log::trace!("Resolving profile {}", name);

	let path = source.find_profile_path(name)?;
	let file_name = path
		.file_name()
		.unwrap_or_else(|| panic!("Profile path has no file name ({:?})", path))
		.to_os_string();

	let mut profile = SimpleProfile::from_file(&path)?;

	if !profile.extends.is_empty() && resolved_profiles.contains(&file_name) {
		// profile was already resolve and has "childre" which will lead to
		// a loop while resolving
		return Err(std::io::Error::new(
			std::io::ErrorKind::FilesystemLoop,
			format!(
				"Circular dependency detected while parsing `{}` (requiered by: `{:?}`) (Stack: \
				 {:?})",
				name,
				resolved_profiles.last(),
				resolved_profiles
			),
		));
	}

	let mut extends = Vec::new();
	std::mem::swap(&mut extends, &mut profile.extends);

	builder.add(name.to_string(), profile);

	resolved_profiles.push(file_name);

	for child in extends {
		let _ = resolve_profile(builder, source, &child, resolved_profiles)?;
	}

	let _ = resolved_profiles
		.pop()
		.expect("Missaligned push/pop operation");

	Ok(())
}

#[cfg(test)]
mod tests {
	use std::collections::HashMap;

	use super::*;
	use crate::hook::Hook;
	use crate::profile::SimpleProfile;
	use crate::variables::UserVars;
	use crate::{MergeMode, Priority};

	#[test]
	fn profile_serde() {
		let mut profile_vars = HashMap::new();
		profile_vars.insert(String::from("RUSTC_VERSION"), String::from("XX.YY"));
		profile_vars.insert(String::from("RUSTC_PATH"), String::from("/usr/bin/rustc"));

		let mut dotfile_vars = HashMap::new();
		dotfile_vars.insert(String::from("RUSTC_VERSION"), String::from("55.22"));
		dotfile_vars.insert(String::from("USERNAME"), String::from("demo"));

		let profile = SimpleProfile {
			extends: Vec::new(),
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

		let parsed: SimpleProfile =
			serde_json::from_str(&json).expect("Profile to be deserializable");

		assert_eq!(parsed, profile);
	}
}
