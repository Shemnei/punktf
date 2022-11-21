use color_eyre::eyre::Context as _;
use std::path::{Path, PathBuf};

/// This struct represents the source directory used by `punktf`. The source
/// directory is the central repository used to store
/// [`Profile`s](`crate::profile::Profile`) and [`Dotfile`s](`crate::profile::dotfile::Dotfile`).
/// `punktf` will only read data from these directories but never write to them.
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
	/// The absolute root source path.
	pub root: PathBuf,

	/// The absolute path to the `profiles` directory.
	pub profiles: PathBuf,

	/// The absolute path to the `dotfiles` directory.
	pub dotfiles: PathBuf,
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
	pub fn from_root(root: PathBuf) -> color_eyre::Result<Self> {
		/// Tries to create a directory if it does not exist.
		/// Bubbles up any error encountered and add some context to it.
		macro_rules! try_exists {
			( $var:ident ) => {
				// TODO: Replace once `try_exists` becomes stable
				if $var.exists() {
					// Should check if read/write is possible
				} else {
					let _ = std::fs::create_dir(&$var).wrap_err_with(|| {
						format!(
							"{} directory does not exist and could not be created (path: {})",
							stringify!($var),
							$var.display()
						)
					})?;
				}
			};
		}

		/// Tries to canonicalize/resolve a path.
		/// Bubbles up any error encountered and add some context to it.
		macro_rules! try_canonicalize {
			($var:ident) => {
				$var.canonicalize().wrap_err_with(|| {
					format!(
						"Failed to resolve punktf's {} directory (path: {})",
						stringify!($var),
						$var.display()
					)
				})?
			};
		}

		// Renames the `root` variable for better error messages
		let source = root;
		try_exists!(source);
		let source = try_canonicalize!(source);

		let profiles = source.join("profiles");
		try_exists!(profiles);
		let profiles = try_canonicalize!(profiles);

		let dotfiles = source.join("dotfiles");
		try_exists!(dotfiles);
		let dotfiles = try_canonicalize!(dotfiles);

		Ok(Self {
			root: source,
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
	/// [`Profile`](`crate::profile::Profile`). The profile name must be given
	/// without any file extension attached (e.g. `demo` instead of `demo.json`).
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
