use std::borrow::Cow;
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
	fn new(source: PathBuf, target: PathBuf) -> Self {
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
	fn new(root_source: PathBuf, root_target: PathBuf) -> Self {
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

	pub fn is_root(&self) -> bool {
		self.child.is_none()
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
	fn from_paths<'b>(paths: &'b Paths, dotfile: &'a Dotfile) -> Self {
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

	pub fn dotfile(&self) -> &Dotfile {
		match self {
			Self::Root(dotfile) => dotfile,
			Self::Child { root: dotfile, .. } => dotfile,
		}
	}
}

#[derive(Debug)]
pub struct File<'a> {
	pub source_path: PathBuf,
	pub target_path: PathBuf,
	pub kind: Kind<'a>,
}

impl File<'_> {
	pub fn dotfile(&self) -> &Dotfile {
		self.kind.dotfile()
	}
}

#[derive(Debug)]
pub struct Directory<'a> {
	pub source_path: PathBuf,
	pub target_path: PathBuf,
	pub kind: Kind<'a>,
}

impl Directory<'_> {
	pub fn dotfile(&self) -> &Dotfile {
		self.kind.dotfile()
	}
}

#[derive(Debug)]
pub struct Symlink {
	pub source_path: PathBuf,
	pub target_path: PathBuf,
}

#[derive(Debug)]
pub struct Rejected<'a> {
	pub source_path: PathBuf,
	pub target_path: PathBuf,
	pub kind: Kind<'a>,
	pub reason: Cow<'static, str>,
}

impl Rejected<'_> {
	pub fn dotfile(&self) -> &Dotfile {
		self.kind.dotfile()
	}
}

#[derive(Debug)]
pub struct Errored<'a> {
	pub source_path: Option<PathBuf>,
	pub target_path: Option<PathBuf>,
	pub kind: Kind<'a>,
	pub error: Box<dyn std::error::Error>,
	pub context: &'static str,
}

impl Errored<'_> {
	pub fn dotfile(&self) -> &Dotfile {
		self.kind.dotfile()
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
					source,
					visitor,
					None,
					dotfile,
					Box::new(err),
					"Failed to resolve source path",
				);
			}
		};

		let target_path = match self.resolve_target_path(dotfile, source_path.is_dir()) {
			Ok(path) => path,
			Err(err) => {
				return self.walk_errored(
					source,
					visitor,
					None,
					dotfile,
					Box::new(err),
					"Failed to resolve target path",
				);
			}
		};

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
		let source_path = paths.child_source_path();
		let target_path = paths.child_target_path();

		if !self.accept(&source_path) {
			return self.walk_rejected(source, visitor, paths, dotfile);
		}

		let file = File {
			source_path: source_path.into_owned(),
			target_path: target_path.into_owned(),
			kind: Kind::from_paths(&paths, dotfile),
		};

		visitor.accept_file(source, &self.profile, &file)
	}

	fn walk_directory(
		&self,
		source: &PunktfSource,
		visitor: &mut impl Visitor,
		paths: Paths,
		dotfile: &Dotfile,
	) -> Result {
		let source_path = paths.child_source_path();
		let target_path = paths.child_target_path();

		if !self.accept(&source_path) {
			return self.walk_rejected(source, visitor, paths, dotfile);
		}

		let directory = Directory {
			source_path: source_path.to_path_buf(),
			target_path: target_path.to_path_buf(),
			kind: Kind::from_paths(&paths, dotfile),
		};

		visitor.accept_directory(source, &self.profile, &directory)?;

		let read_dir = match std::fs::read_dir(&source_path) {
			Ok(path) => path,
			Err(err) => {
				return self.walk_errored(
					source,
					visitor,
					Some(paths),
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
						Some(paths),
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
		let source_path = paths.child_source_path();
		let target_path = paths.child_target_path();

		let rejected = Rejected {
			source_path: source_path.into_owned(),
			target_path: target_path.into_owned(),
			kind: Kind::from_paths(&paths, dotfile),
			reason: Cow::Borrowed("Rejected by filter"),
		};

		visitor.accept_rejected(source, &self.profile, &rejected)
	}

	fn walk_errored(
		&self,
		source: &PunktfSource,
		visitor: &mut impl Visitor,
		paths: Option<Paths>,
		dotfile: &Dotfile,
		error: Box<dyn std::error::Error>,
		context: &'static str,
	) -> Result {
		let errored = if let Some(paths) = paths {
			let source_path = paths.child_source_path();
			let target_path = paths.child_target_path();

			Errored {
				source_path: Some(source_path.into_owned()),
				target_path: Some(target_path.into_owned()),
				kind: Kind::from_paths(&paths, dotfile),
				error,
				context,
			}
		} else {
			Errored {
				source_path: None,
				target_path: None,
				kind: Kind::Root(dotfile),
				error,
				context,
			}
		};

		visitor.accept_errored(source, &self.profile, &errored)
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

	fn resolve_target_path(&self, dotfile: &Dotfile, is_dir: bool) -> std::io::Result<PathBuf> {
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
	pub fn new(visitor: V) -> Self {
		Self { visitor }
	}

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
