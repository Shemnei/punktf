use std::borrow::Cow;
use std::ops::Deref;
use std::path::{Path, PathBuf};

use crate::profile::LayeredProfile;
use crate::{Dotfile, PunktfSource};

use color_eyre::eyre::Context;

use crate::template::source::Source;
use crate::template::Template;

#[derive(Debug, Clone)]
struct PathLink {
	source: PathBuf,
	target: PathBuf,
}

impl PathLink {
	const fn new(source: PathBuf, target: PathBuf) -> Self {
		Self { source, target }
	}

	fn join(mut self, relative: &Path) -> Self {
		self.source = self.source.join(relative);
		self.target = self.target.join(relative);

		self
	}
}

#[derive(Debug, Clone)]
struct Paths {
	root: PathLink,
	child: Option<PathLink>,
}

impl Paths {
	const fn new(root_source: PathBuf, root_target: PathBuf) -> Self {
		Self {
			root: PathLink::new(root_source, root_target),
			child: None,
		}
	}

	fn with_child(self, rel_path: impl Into<PathBuf>) -> Self {
		let Paths { root, child } = self;
		let rel_path = rel_path.into();

		let child = if let Some(child) = child {
			child.join(&rel_path)
		} else {
			PathLink::new(rel_path.clone(), rel_path)
		};

		Self {
			root,
			child: Some(child),
		}
	}

	pub const fn is_root(&self) -> bool {
		self.child.is_none()
	}

	pub const fn is_child(&self) -> bool {
		self.child.is_some()
	}

	pub fn root_source_path(&self) -> &Path {
		&self.root.source
	}

	pub fn root_target_path(&self) -> &Path {
		&self.root.target
	}

	pub fn child_source_path(&self) -> Cow<'_, Path> {
		if let Some(child) = &self.child {
			Cow::Owned(self.root_source_path().join(&child.source))
		} else {
			Cow::Borrowed(self.root_source_path())
		}
	}

	pub fn child_target_path(&self) -> Cow<'_, Path> {
		if let Some(child) = &self.child {
			Cow::Owned(self.root_target_path().join(&child.target))
		} else {
			Cow::Borrowed(self.root_target_path())
		}
	}
}

#[derive(Debug)]
pub enum Kind<'a> {
	Root(&'a Dotfile),
	Child {
		root: &'a Dotfile,
		root_source_path: PathBuf,
		root_target_path: PathBuf,
	},
}

impl<'a> Kind<'a> {
	fn from_paths(paths: Paths, dotfile: &'a Dotfile) -> Self {
		if paths.is_root() {
			Self::Root(dotfile)
		} else {
			Self::Child {
				root: dotfile,
				root_source_path: paths.root_source_path().to_path_buf(),
				root_target_path: paths.root_target_path().to_path_buf(),
			}
		}
	}

	pub const fn dotfile(&self) -> &Dotfile {
		match self {
			Self::Root(dotfile) => dotfile,
			Self::Child { root: dotfile, .. } => dotfile,
		}
	}
}

#[derive(Debug)]
pub struct DeployableDotfile<'a> {
	pub relative_source_path: PathBuf,
	pub source_path: PathBuf,
	pub target_path: PathBuf,
	pub kind: Kind<'a>,
}

impl<'a> DeployableDotfile<'a> {
	fn new(source: &PunktfSource, paths: Paths, dotfile: &'a Dotfile) -> Self {
		let source_path = paths.child_source_path().into_owned();
		let target_path = paths.child_target_path().into_owned();
		let relative_source_path = source_path
			.strip_prefix(&source.dotfiles)
			.expect("Dotfile is not in the dotfile root")
			.to_path_buf();
		let kind = Kind::from_paths(paths, dotfile);

		Self {
			relative_source_path,
			source_path,
			target_path,
			kind,
		}
	}
}

impl DeployableDotfile<'_> {
	pub const fn dotfile(&self) -> &Dotfile {
		self.kind.dotfile()
	}
}

#[derive(Debug)]
pub struct File<'a>(DeployableDotfile<'a>);

impl<'a> Deref for File<'a> {
	type Target = DeployableDotfile<'a>;

	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

#[derive(Debug)]
pub struct Directory<'a>(DeployableDotfile<'a>);

impl<'a> Deref for Directory<'a> {
	type Target = DeployableDotfile<'a>;

	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

#[derive(Debug)]
pub struct Symlink {
	pub source_path: PathBuf,
	pub target_path: PathBuf,
}

#[derive(Debug)]
pub struct Rejected<'a> {
	pub dotfile: DeployableDotfile<'a>,
	pub reason: Cow<'static, str>,
}

impl<'a> Deref for Rejected<'a> {
	type Target = DeployableDotfile<'a>;

	fn deref(&self) -> &Self::Target {
		&self.dotfile
	}
}

#[derive(Debug)]
pub struct Errored<'a> {
	pub dotfile: DeployableDotfile<'a>,
	pub error: Box<dyn std::error::Error>,
	pub context: &'static str,
}

impl<'a> Deref for Errored<'a> {
	type Target = DeployableDotfile<'a>;

	fn deref(&self) -> &Self::Target {
		&self.dotfile
	}
}

pub type Result = std::result::Result<(), Box<dyn std::error::Error>>;

pub trait Visitor {
	fn accept_file<'a>(
		&mut self,
		source: &PunktfSource,
		profile: &LayeredProfile,
		file: &File<'a>,
	) -> Result;

	fn accept_directory<'a>(
		&mut self,
		source: &PunktfSource,
		profile: &LayeredProfile,
		directory: &Directory<'a>,
	) -> Result;

	fn accept_link(
		&mut self,
		source: &PunktfSource,
		profile: &LayeredProfile,
		symlink: &Symlink,
	) -> Result;

	fn accept_rejected<'a>(
		&mut self,
		source: &PunktfSource,
		profile: &LayeredProfile,
		rejected: &Rejected<'a>,
	) -> Result;

	fn accept_errored<'a>(
		&mut self,
		source: &PunktfSource,
		profile: &LayeredProfile,
		errored: &Errored<'a>,
	) -> Result;
}

#[derive(Debug)]
pub struct Walker<'a> {
	// Filter? "--filter='name=*'"
	// Sort by priority and eliminate duplicate lower ones
	profile: &'a LayeredProfile,
}

impl<'a> Walker<'a> {
	pub fn new(profile: &'a mut LayeredProfile) -> Self {
		{
			let dotfiles = &mut profile.dotfiles;
			// Sorty highest to lowest by priority
			dotfiles.sort_by_key(|(_, d)| -(d.priority.map(|p| p.0).unwrap_or(0) as i64));
		};

		Self { profile }
	}

	pub fn walk(&self, source: &PunktfSource, visitor: &mut impl Visitor) -> Result {
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
		let source_path = self.resolve_source_path(source, dotfile);
		let target_path = self.resolve_target_path(dotfile, source_path.is_dir());

		let paths = Paths::new(source_path, target_path);

		self.walk_path(source, visitor, paths, dotfile)
	}

	fn walk_path(
		&self,
		source: &PunktfSource,
		visitor: &mut impl Visitor,
		paths: Paths,
		dotfile: &Dotfile,
	) -> Result {
		let source_path = paths.child_source_path();

		if !self.accept(&source_path) {
			return self.walk_rejected(source, visitor, paths, dotfile);
		}

		if source_path.is_file() {
			self.walk_file(source, visitor, paths, dotfile)
		} else if source_path.is_dir() {
			self.walk_directory(source, visitor, paths, dotfile)
		} else {
			// TODO: Better handling
			panic!("Symlinks are not supported")
		}
	}

	fn walk_file(
		&self,
		source: &PunktfSource,
		visitor: &mut impl Visitor,
		paths: Paths,
		dotfile: &Dotfile,
	) -> Result {
		let file = File(DeployableDotfile::new(source, paths, dotfile));

		visitor.accept_file(source, self.profile, &file)
	}

	fn walk_directory(
		&self,
		source: &PunktfSource,
		visitor: &mut impl Visitor,
		paths: Paths,
		dotfile: &Dotfile,
	) -> Result {
		let source_path = paths.child_source_path();

		let directory = Directory(DeployableDotfile::new(source, paths.clone(), dotfile));

		visitor.accept_directory(source, self.profile, &directory)?;

		let read_dir = match std::fs::read_dir(&source_path) {
			Ok(path) => path,
			Err(err) => {
				return self.walk_errored(
					source,
					visitor,
					paths,
					dotfile,
					Box::new(err),
					"Failed to read directory",
				);
			}
		};

		for dent in read_dir {
			let dent = match dent {
				Ok(dent) => dent,
				Err(err) => {
					return self.walk_errored(
						source,
						visitor,
						paths,
						dotfile,
						Box::new(err),
						"Failed to read directory",
					);
				}
			};

			self.walk_path(
				source,
				visitor,
				paths.clone().with_child(dent.file_name()),
				dotfile,
			)?;
		}

		Ok(())
	}

	fn walk_rejected(
		&self,
		source: &PunktfSource,
		visitor: &mut impl Visitor,
		paths: Paths,
		dotfile: &Dotfile,
	) -> Result {
		let rejected = Rejected {
			dotfile: DeployableDotfile::new(source, paths, dotfile),
			reason: Cow::Borrowed("Rejected by filter"),
		};

		visitor.accept_rejected(source, self.profile, &rejected)
	}

	fn walk_errored(
		&self,
		source: &PunktfSource,
		visitor: &mut impl Visitor,
		paths: Paths,
		dotfile: &Dotfile,
		error: Box<dyn std::error::Error>,
		context: &'static str,
	) -> Result {
		let errored = Errored {
			dotfile: DeployableDotfile::new(source, paths, dotfile),
			error,
			context,
		};

		visitor.accept_errored(source, self.profile, &errored)
	}

	fn resolve_path(&self, path: PathBuf) -> PathBuf {
		// TODO: Replace envs/~
		path.canonicalize().unwrap_or(path)
	}

	fn resolve_source_path(&self, source: &PunktfSource, dotfile: &Dotfile) -> PathBuf {
		self.resolve_path(source.dotfiles.join(&dotfile.path))
	}

	fn resolve_target_path(&self, dotfile: &Dotfile, is_dir: bool) -> PathBuf {
		let path = if is_dir && dotfile.rename.is_none() && dotfile.overwrite_target.is_none() {
			self.profile
				.target_path()
				.expect("No target path set")
				.to_path_buf()
		} else {
			dotfile
				.overwrite_target
				.as_deref()
				.unwrap_or_else(|| self.profile.target_path().expect("No target path set"))
				.join(dotfile.rename.as_ref().unwrap_or(&dotfile.path))
		};

		// NOTE: Do not call canonicalize as the path migh not exist which would cause an error.

		self.resolve_path(path)
	}

	const fn accept(&self, _path: &Path) -> bool {
		// TODO: Apply filter
		true
	}
}

pub trait TemplateVisitor: Visitor {
	fn accept_template<'a>(
		&mut self,
		source: &PunktfSource,
		profile: &LayeredProfile,
		file: &File<'a>,
		// Returns a function to resolve the content to make the resolving lazy
		// for upstream visitors.
		resolve_content: impl FnOnce(&str) -> color_eyre::Result<String>,
	) -> Result;
}

#[derive(Debug)]
pub struct ResolvingVisitor<V> {
	visitor: V,
}

impl<V> ResolvingVisitor<V>
where
	V: TemplateVisitor,
{
	pub const fn new(visitor: V) -> Self {
		Self { visitor }
	}

	#[allow(clippy::missing_const_for_fn)]
	pub fn into_inner(self) -> V {
		self.visitor
	}
}

impl<V: TemplateVisitor> Visitor for ResolvingVisitor<V> {
	fn accept_file<'a>(
		&mut self,
		source: &PunktfSource,
		profile: &LayeredProfile,
		file: &File<'a>,
	) -> Result {
		if file.dotfile().is_template() {
			let resolve_fn = |content: &str| {
				let source = Source::file(&file.source_path, content);
				let template = Template::parse(source)
					.with_context(|| format!("File: {}", file.source_path.display()))?;

				template
					.resolve(Some(profile.variables()), file.dotfile().variables.as_ref())
					.with_context(|| format!("File: {}", file.source_path.display()))
			};

			self.visitor
				.accept_template(source, profile, file, resolve_fn)
		} else {
			self.visitor.accept_file(source, profile, file)
		}
	}

	fn accept_directory<'a>(
		&mut self,
		source: &PunktfSource,
		profile: &LayeredProfile,
		directory: &Directory<'a>,
	) -> Result {
		self.visitor.accept_directory(source, profile, directory)
	}

	fn accept_link(
		&mut self,
		source: &PunktfSource,
		profile: &LayeredProfile,
		symlink: &Symlink,
	) -> Result {
		self.visitor.accept_link(source, profile, symlink)
	}

	fn accept_rejected<'a>(
		&mut self,
		source: &PunktfSource,
		profile: &LayeredProfile,
		rejected: &Rejected<'a>,
	) -> Result {
		self.visitor.accept_rejected(source, profile, rejected)
	}

	fn accept_errored<'a>(
		&mut self,
		source: &PunktfSource,
		profile: &LayeredProfile,
		errored: &Errored<'a>,
	) -> Result {
		self.visitor.accept_errored(source, profile, errored)
	}
}
