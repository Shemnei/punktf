//! This module provieds a [`Visitor`] trait and a [`Walker`] which iterates
//! over every item to be deployed in a given profile.
//! The visitor accepts items on different functions depending on status and type.

pub mod deploy;
pub mod diff;

use std::borrow::Cow;
use std::fmt;
use std::ops::Deref;
use std::path::{Path, PathBuf};

use crate::profile::link;
use crate::profile::LayeredProfile;
use crate::profile::{dotfile::Dotfile, source::PunktfSource};

use color_eyre::eyre::Context;

use crate::template::source::Source;
use crate::template::Template;

/// Result type for this module.
pub type Result = std::result::Result<(), Box<dyn std::error::Error>>;

/// A struct to keep two paths in sync while appending relative child paths.
#[derive(Debug, Clone)]
struct PathLink {
	/// Source path.
	source: PathBuf,

	/// Target path.
	target: PathBuf,
}

impl PathLink {
	/// Creates a new path link struct.
	const fn new(source: PathBuf, target: PathBuf) -> Self {
		Self { source, target }
	}

	/// Appends a child path to the path link.
	///
	/// The given path will be added in sync to both internal paths.
	fn join(mut self, relative: &Path) -> Self {
		self.source = self.source.join(relative);
		self.target = self.target.join(relative);

		self
	}
}

/// A struct to hold all paths relevant for a [`Item`].
#[derive(Debug, Clone)]
struct Paths {
	/// The root paths of the underlying [`Dotfile`](`crate::profile::dotfile::Dotfile`).
	///
	/// This will always be the path of the item.
	root: PathLink,

	/// The paths of the [`Item`].
	///
	/// If the dotfile is a directory, this contains the relevant path
	/// to the item which is included by the root dotfile.
	child: Option<PathLink>,
}

impl Paths {
	/// Creates a new paths instance.
	const fn new(root_source: PathBuf, root_target: PathBuf) -> Self {
		Self {
			root: PathLink::new(root_source, root_target),
			child: None,
		}
	}

	/// Appends a relative child path to instance.
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

	/// Checks if this instance points to a actual
	/// [`Dotfile`](`crate::profile::dotfile::Dotfile`).
	pub const fn is_root(&self) -> bool {
		self.child.is_none()
	}

	/// Checks if this instance points to a child of an
	/// [`Dotfile`](`crate::profile::dotfile::Dotfile`).
	pub const fn is_child(&self) -> bool {
		self.child.is_some()
	}

	/// Retrives the source path of the actual dotfile.
	pub fn root_source_path(&self) -> &Path {
		&self.root.source
	}

	/// Retrives the target path of the acutal dotfile.
	pub fn root_target_path(&self) -> &Path {
		&self.root.target
	}

	/// Retrives the target path of the child.
	///
	/// If this is not a child instance, the root path will be returned instead.
	pub fn child_source_path(&self) -> Cow<'_, Path> {
		if let Some(child) = &self.child {
			Cow::Owned(self.root_source_path().join(&child.source))
		} else {
			Cow::Borrowed(self.root_source_path())
		}
	}

	/// Retrives the source path of the child.
	///
	/// If this is not a child instance, the root path will be returned instead.
	pub fn child_target_path(&self) -> Cow<'_, Path> {
		if let Some(child) = &self.child {
			Cow::Owned(self.root_target_path().join(&child.target))
		} else {
			Cow::Borrowed(self.root_target_path())
		}
	}
}

/// Defines what kind the item is.
#[derive(Debug)]
pub enum Kind<'a> {
	/// The item stems directly from a [`Dotfile`](`crate::profile::dotfile::Dotfile`).
	Root(&'a Dotfile),

	/// The item is a child of a directory [`Dotfile`](`crate::profile::dotfile::Dotfile`).
	Child {
		/// The root [`Dotfile`](`crate::profile::dotfile::Dotfile`) from which
		/// this item stems.
		root: &'a Dotfile,

		/// Absoulte source path to the root dotfile.
		root_source_path: PathBuf,

		/// Absoulte target path to the root dotfile.
		root_target_path: PathBuf,
	},
}

impl<'a> Kind<'a> {
	/// Creates a new instance.
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

	/// Retrieves the underlying [`Dotfile`](`crate::profile::dotfile::Dotfile`).
	pub const fn dotfile(&self) -> &Dotfile {
		match self {
			Self::Root(dotfile) => dotfile,
			Self::Child { root: dotfile, .. } => dotfile,
		}
	}
}

/// Saves relevant information about an item to be processed.
#[derive(Debug)]
pub struct Item<'a> {
	/// Relative path to the item inside the `dotfiles` directly.
	pub relative_source_path: PathBuf,

	/// Absoulte source path for the item.
	pub source_path: PathBuf,

	/// Absoulte target path for the item.
	pub target_path: PathBuf,

	/// Kind of the item.
	pub kind: Kind<'a>,
}

impl<'a> Item<'a> {
	/// Creates a new instance.
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

impl Item<'_> {
	/// Retrives the underlying dotfile.
	pub const fn dotfile(&self) -> &Dotfile {
		self.kind.dotfile()
	}
}

/// A file to be processed.
#[derive(Debug)]
pub struct File<'a>(Item<'a>);

impl<'a> Deref for File<'a> {
	type Target = Item<'a>;

	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

/// A directory to be processed.
#[derive(Debug)]
pub struct Directory<'a>(Item<'a>);

impl<'a> Deref for Directory<'a> {
	type Target = Item<'a>;

	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

/// A symlink to be processed.
#[derive(Debug)]
pub struct Symlink {
	/// Absoulte source path of the link.
	pub source_path: PathBuf,

	/// Absoulte target path of the link.
	pub target_path: PathBuf,

	/// Indicates if any existing symlink at the [`Symlink::target_path`] should
	/// be replaced by this item.
	pub replace: bool,
}

/// Holds information about a rejected item.
#[derive(Debug)]
pub struct Rejected<'a> {
	/// The item which was rejected.
	pub item: Item<'a>,

	/// The reason why the item was rejected.
	pub reason: Cow<'static, str>,
}

impl<'a> Deref for Rejected<'a> {
	type Target = Item<'a>;

	fn deref(&self) -> &Self::Target {
		&self.item
	}
}

/// Holds information about a errored item.
#[derive(Debug)]
pub struct Errored<'a> {
	/// The item which was rejected.
	pub item: Item<'a>,

	/// The error which has occured.
	pub error: Option<Box<dyn std::error::Error>>,

	/// The context of the error.
	pub context: Option<Cow<'a, str>>,
}

impl<'a> Deref for Errored<'a> {
	type Target = Item<'a>;

	fn deref(&self) -> &Self::Target {
		&self.item
	}
}

impl fmt::Display for Errored<'_> {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		let has_context = if let Some(context) = &self.context {
			f.write_str(context)?;
			true
		} else {
			false
		};

		if let Some(err) = &self.error {
			if has_context {
				f.write_str(": ")?;
			}
			write!(f, "{}", err)?;
		}

		Ok(())
	}
}

/// Trait accepts [`Item`]s for further processing.
///
/// This is a kind of an iterator over all items which are included in a
/// [`Profile`](`crate::profile::Profile`).
pub trait Visitor {
	/// Accepts a [`File`] item for further processing.
	fn accept_file<'a>(
		&mut self,
		source: &PunktfSource,
		profile: &LayeredProfile,
		file: &File<'a>,
	) -> Result;

	/// Accepts a [`Directory`] item for further processing.
	fn accept_directory<'a>(
		&mut self,
		source: &PunktfSource,
		profile: &LayeredProfile,
		directory: &Directory<'a>,
	) -> Result;

	/// Accepts a [`Symlink`] item for further processing.
	fn accept_link(
		&mut self,
		source: &PunktfSource,
		profile: &LayeredProfile,
		symlink: &Symlink,
	) -> Result;

	/// Accepts a [`Rejected`] item for further processing.
	///
	/// This is called instead of [`Visitor::accept_file`],
	/// [`Visitor::accept_directory`] or [`Visitor::accept_link`] when
	/// the [`Item`] is rejected.
	fn accept_rejected<'a>(
		&mut self,
		source: &PunktfSource,
		profile: &LayeredProfile,
		rejected: &Rejected<'a>,
	) -> Result;

	/// Accepts a [`Errored`] item for further processing.
	///
	/// This is called instead of [`Visitor::accept_file`],
	/// [`Visitor::accept_directory`] or [`Visitor::accept_link`] when
	/// an error is encountered for an [`Item`] .
	fn accept_errored<'a>(
		&mut self,
		source: &PunktfSource,
		profile: &LayeredProfile,
		errored: &Errored<'a>,
	) -> Result;
}

/// Walks over each item of a [`LayeredProfile`](`crate::profile::LayeredProfile`)
/// and calls the appropiate functions of the given visitor.
#[derive(Debug)]
pub struct Walker<'a> {
	// Filter? "--filter='name=*'"
	// Sort by priority and eliminate duplicate lower ones
	/// The profile to walk.
	profile: &'a LayeredProfile,
}

impl<'a> Walker<'a> {
	/// Creates a new instance.
	///
	/// The [`LayeredProfile::dotfiles`](`crate::profile::LayeredProfile::dotfiles`)
	/// will be sorted by [`Dotfile::priority`](`crate::profile::dotfile::Dotfile::priority`)
	/// to avoid unneccessary read/write operations during a deployment.
	pub fn new(profile: &'a mut LayeredProfile) -> Self {
		{
			let dotfiles = &mut profile.dotfiles;
			// Sorty highest to lowest by priority
			dotfiles.sort_by_key(|(_, d)| -(d.priority.map(|p| p.0).unwrap_or(0) as i64));
		};

		Self { profile }
	}

	/// Walks the profile and calls the appropiate functions on the given [`Visitor`].
	pub fn walk(&self, source: &PunktfSource, visitor: &mut impl Visitor) -> Result {
		for dotfile in self.profile.dotfiles() {
			self.walk_dotfile(source, visitor, dotfile)?;
		}

		for link in self.profile.symlinks() {
			self.walk_link(source, visitor, link)?;
		}

		Ok(())
	}

	/// Walks each item of a [`Dotfile`](`crate::profile::dotfile::Dotfile`).
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

	/// Walks a specific path of a [`Dotfile`](`crate::profile::dotfile::Dotfile`).
	///
	/// This either calls [`Walker::walk_file`] or [`Walker::walk_directory`].
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

		// For now dont follow symlinks (`metadata()` would get the metadata of the target of a
		// link).
		let metadata = match source_path.symlink_metadata() {
			Ok(metadata) => metadata,
			Err(err) => {
				return self.walk_errored(
					source,
					visitor,
					paths,
					dotfile,
					Some(err),
					Some("Failed to resolve metadata"),
				);
			}
		};

		if metadata.is_file() {
			self.walk_file(source, visitor, paths, dotfile)
		} else if metadata.is_dir() {
			self.walk_directory(source, visitor, paths, dotfile)
		} else {
			let err = std::io::Error::new(std::io::ErrorKind::Unsupported, "Invalid file type");

			self.walk_errored(source, visitor, paths, dotfile, Some(err), None::<&str>)
		}
	}

	/// Calls [`Visitor::accept_file`].
	fn walk_file(
		&self,
		source: &PunktfSource,
		visitor: &mut impl Visitor,
		paths: Paths,
		dotfile: &Dotfile,
	) -> Result {
		let file = File(Item::new(source, paths, dotfile));

		visitor.accept_file(source, self.profile, &file)
	}

	/// Calls [`Visitor::accept_directory`].
	///
	/// After that it walks all child items of it.
	fn walk_directory(
		&self,
		source: &PunktfSource,
		visitor: &mut impl Visitor,
		paths: Paths,
		dotfile: &Dotfile,
	) -> Result {
		let source_path = paths.child_source_path();

		let directory = Directory(Item::new(source, paths.clone(), dotfile));

		visitor.accept_directory(source, self.profile, &directory)?;

		let read_dir = match std::fs::read_dir(&source_path) {
			Ok(path) => path,
			Err(err) => {
				return self.walk_errored(
					source,
					visitor,
					paths,
					dotfile,
					Some(err),
					Some("Failed to read directory"),
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
						Some(err),
						Some("Failed to read directory"),
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

	/// Calls [`Visitor::accept_link`].
	fn walk_link(
		&self,
		source: &PunktfSource,
		visitor: &mut impl Visitor,
		link: &link::Symlink,
	) -> Result {
		// DO NOT CANONICOLIZE THE PATHS AS THIS WOULD FOLLOW LINKS
		let link = Symlink {
			source_path: link.source_path.clone(),
			target_path: link.target_path.clone(),
			replace: link.replace,
		};

		visitor.accept_link(source, self.profile, &link)
	}

	/// Calls [`Visitor::accept_rejected`].
	fn walk_rejected(
		&self,
		source: &PunktfSource,
		visitor: &mut impl Visitor,
		paths: Paths,
		dotfile: &Dotfile,
	) -> Result {
		let rejected = Rejected {
			item: Item::new(source, paths, dotfile),
			reason: Cow::Borrowed("Rejected by filter"),
		};

		visitor.accept_rejected(source, self.profile, &rejected)
	}

	/// Calls [`Visitor::accept_errored`].
	fn walk_errored(
		&self,
		source: &PunktfSource,
		visitor: &mut impl Visitor,
		paths: Paths,
		dotfile: &Dotfile,
		error: Option<impl std::error::Error + 'static>,
		context: Option<impl Into<Cow<'a, str>>>,
	) -> Result {
		let errored = Errored {
			item: Item::new(source, paths, dotfile),
			error: error.map(|e| e.into()),
			context: context.map(|c| c.into()),
		};

		visitor.accept_errored(source, self.profile, &errored)
	}

	/// Applies final transformations for paths from [`Walker::resolve_source_path`]
	/// and [`Walker::resolve_target_path`].
	fn resolve_path(&self, path: PathBuf) -> PathBuf {
		// TODO: Replace envs/~
		path.canonicalize().unwrap_or(path)
	}

	/// Resolves the dotfile to a absolute source path.
	fn resolve_source_path(&self, source: &PunktfSource, dotfile: &Dotfile) -> PathBuf {
		self.resolve_path(source.dotfiles.join(&dotfile.path))
	}

	/// Resolves the dotfile to a absolute target path.
	///
	/// Some special logic is applied for directories.
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

		self.resolve_path(path)
	}

	/// TODO
	const fn accept(&self, _path: &Path) -> bool {
		// TODO: Apply filter
		true
	}
}

/// An extension trait to [`Visitor`] which adds a new function to accept
/// template items.
pub trait TemplateVisitor: Visitor {
	/// Accepts a template [`File`] item for further processing.
	///
	/// This also provides a function to resolve the contents of the template
	/// by calling it with the original template contents.
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

/// An extension for a base [`Visitor`] to split up files into normal files and
/// template files.
///
/// All accepted files are checked up on receiving and the either directly send
/// out with [`Visitor::accept_file`] if they are a normal file or with
/// [`TemplateVisitor::accept_template`] if it is a template.
#[derive(Debug)]
pub struct ResolvingVisitor<V>(V);

impl<V> ResolvingVisitor<V>
where
	V: TemplateVisitor,
{
	/// Gets the base [`Visitor`].
	#[allow(clippy::missing_const_for_fn)]
	pub fn into_inner(self) -> V {
		self.0
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

			self.0.accept_template(source, profile, file, resolve_fn)
		} else {
			self.0.accept_file(source, profile, file)
		}
	}

	fn accept_directory<'a>(
		&mut self,
		source: &PunktfSource,
		profile: &LayeredProfile,
		directory: &Directory<'a>,
	) -> Result {
		self.0.accept_directory(source, profile, directory)
	}

	fn accept_link(
		&mut self,
		source: &PunktfSource,
		profile: &LayeredProfile,
		symlink: &Symlink,
	) -> Result {
		self.0.accept_link(source, profile, symlink)
	}

	fn accept_rejected<'a>(
		&mut self,
		source: &PunktfSource,
		profile: &LayeredProfile,
		rejected: &Rejected<'a>,
	) -> Result {
		self.0.accept_rejected(source, profile, rejected)
	}

	fn accept_errored<'a>(
		&mut self,
		source: &PunktfSource,
		profile: &LayeredProfile,
		errored: &Errored<'a>,
	) -> Result {
		self.0.accept_errored(source, profile, errored)
	}
}
