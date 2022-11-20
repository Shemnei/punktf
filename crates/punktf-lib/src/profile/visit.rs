use std::borrow::Cow;
use std::path::{Path, PathBuf};

use crate::profile::LayeredProfile;
use crate::{Dotfile, PunktfSource};

#[derive(Debug)]
pub struct File<'a> {
	pub source_path: PathBuf,
	pub target_path: PathBuf,
	pub dotfile: &'a Dotfile,
}

#[derive(Debug)]
pub struct Directory<'a> {
	pub source_path: PathBuf,
	pub target_path: PathBuf,
	pub dotfile: &'a Dotfile,
}

#[derive(Debug)]
pub struct Symlink {
	pub source_path: PathBuf,
	pub target_path: PathBuf,
}

#[derive(Debug)]
pub struct Rejected<'a> {
	pub source_path: PathBuf,
	pub dotfile: &'a Dotfile,
	pub reason: Cow<'static, str>,
}

#[derive(Debug)]
pub struct Errored<'a> {
	pub source_path: Option<PathBuf>,
	pub target_path: Option<PathBuf>,
	pub dotfile: &'a Dotfile,
	pub error: Box<dyn std::error::Error>,
	pub context: &'static str,
}

pub type Result = std::result::Result<(), Box<dyn std::error::Error>>;

pub trait Visitor {
	fn accept_file<'a>(&mut self, profile: &LayeredProfile, file: &File<'a>) -> Result;

	fn accept_directory<'a>(
		&mut self,
		profile: &LayeredProfile,
		directory: &Directory<'a>,
	) -> Result;

	fn accept_link(&mut self, profile: &LayeredProfile, symlink: &Symlink) -> Result;

	fn accept_rejected<'a>(&mut self, profile: &LayeredProfile, rejected: &Rejected<'a>) -> Result;

	fn accept_errored<'a>(&mut self, profile: &LayeredProfile, errored: &Errored<'a>) -> Result;
}

#[derive(Debug)]
pub struct Walker {
	// Filter? "--filter='name=*'"
	// Sort by priority and eliminate duplicate lower ones
	profile: LayeredProfile,
}

impl Walker {
	pub const fn new(profile: LayeredProfile) -> Self {
		Self { profile }
	}

	pub fn walk(mut self, source: &PunktfSource, visitor: &mut impl Visitor) -> Result {
		{
			let dotfiles = &mut self.profile.dotfiles;
			// Sorty highest to lowest by priority
			dotfiles.sort_by_key(|(_, d)| -(d.priority.map(|p| p.0).unwrap_or(0) as i64));
		};

		for dotfile in self.profile.dotfiles() {
			self.walk_dotfile(source, visitor, dotfile)?;
		}

		// TODO: Do links

		Ok(())
	}

	fn walk_dotfile(
		&self,
		source: &PunktfSource,
		visitor: &mut impl Visitor,
		dotfile: &Dotfile,
	) -> Result {
		let source_path = match self.resolve_source_path(source, dotfile) {
			Ok(path) => path,
			Err(err) => {
				return self.walk_errored(
					visitor,
					None,
					None,
					dotfile,
					Box::new(err),
					"Failed to resolve source path",
				);
			}
		};

		let target_path = match self.resolve_target_path(dotfile) {
			Ok(path) => path,
			Err(err) => {
				return self.walk_errored(
					visitor,
					Some(source_path),
					None,
					dotfile,
					Box::new(err),
					"Failed to resolve target path",
				);
			}
		};

		self.walk_path(visitor, source_path, target_path, dotfile)
	}

	fn walk_path(
		&self,
		visitor: &mut impl Visitor,
		source_path: PathBuf,
		target_path: PathBuf,
		dotfile: &Dotfile,
	) -> Result {
		if source_path.is_file() {
			self.walk_file(visitor, source_path, target_path, dotfile)
		} else if source_path.is_dir() {
			self.walk_directory(visitor, source_path, target_path, dotfile)
		} else {
			// TODO: Better handling
			panic!("Symlinks are not supported")
		}
	}

	fn walk_file(
		&self,
		visitor: &mut impl Visitor,
		source_path: PathBuf,
		target_path: PathBuf,
		dotfile: &Dotfile,
	) -> Result {
		if !self.accept(&source_path) {
			return self.walk_rejected(visitor, source_path, dotfile);
		}

		let file = File {
			source_path,
			target_path,
			dotfile,
		};

		visitor.accept_file(&self.profile, &file)
	}

	fn walk_directory(
		&self,
		visitor: &mut impl Visitor,
		source_path: PathBuf,
		target_path: PathBuf,
		dotfile: &Dotfile,
	) -> Result {
		// Directory special path logic
		let target_path = if dotfile.rename.is_some() {
			target_path
		} else {
			dotfile.overwrite_target.clone().unwrap_or_else(|| {
				self.profile
					.target_path()
					.expect("No target path set")
					.to_path_buf()
			})
		};

		if !self.accept(&source_path) {
			return self.walk_rejected(visitor, source_path, dotfile);
		}

		let directory = Directory {
			source_path: source_path.clone(),
			target_path: target_path.clone(),
			dotfile,
		};

		visitor.accept_directory(&self.profile, &directory)?;

		let read_dir = match std::fs::read_dir(&source_path) {
			Ok(path) => path,
			Err(err) => {
				return self.walk_errored(
					visitor,
					Some(source_path),
					Some(target_path),
					dotfile,
					Box::new(err),
					"Failed to read directory",
				);
			}
		};

		for dent in read_dir {
			let dent = match dent {
				Ok(path) => path,
				Err(err) => {
					return self.walk_errored(
						visitor,
						Some(source_path),
						Some(target_path),
						dotfile,
						Box::new(err),
						"Failed to read directory",
					);
				}
			};

			self.walk_path(
				visitor,
				dent.path(),
				target_path.join(dent.file_name()),
				dotfile,
			)?;
		}

		Ok(())
	}

	fn walk_rejected(
		&self,
		visitor: &mut impl Visitor,
		source_path: PathBuf,
		dotfile: &Dotfile,
	) -> Result {
		let rejected = Rejected {
			source_path,
			dotfile,
			reason: Cow::Borrowed("Rejected by filter"),
		};

		visitor.accept_rejected(&self.profile, &rejected)
	}

	fn walk_errored(
		&self,
		visitor: &mut impl Visitor,
		source_path: Option<PathBuf>,
		target_path: Option<PathBuf>,
		dotfile: &Dotfile,
		error: Box<dyn std::error::Error>,
		context: &'static str,
	) -> Result {
		let errored = Errored {
			source_path,
			target_path,
			dotfile,
			error,
			context,
		};

		visitor.accept_errored(&self.profile, &errored)
	}

	const fn resolve_path(&self, path: PathBuf) -> std::io::Result<PathBuf> {
		// TODO: Replace envs/~
		Ok(path)
	}

	fn resolve_source_path(
		&self,
		source: &PunktfSource,
		dotfile: &Dotfile,
	) -> std::io::Result<PathBuf> {
		self.resolve_path(source.dotfiles.join(&dotfile.path).canonicalize()?)
	}

	fn resolve_target_path(&self, dotfile: &Dotfile) -> std::io::Result<PathBuf> {
		let path = dotfile
			.overwrite_target
			.as_deref()
			.unwrap_or_else(|| self.profile.target_path().expect("No target path set"))
			.join(dotfile.rename.as_ref().unwrap_or(&dotfile.path));

		// NOTE: Do not call canonicalize as the path migh not exist which would cause an error.

		self.resolve_path(path)
	}

	const fn accept(&self, _path: &Path) -> bool {
		// TODO: Apply filter
		true
	}
}
