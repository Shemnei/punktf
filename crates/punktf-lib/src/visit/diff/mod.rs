//! A [`Visitor`](`crate::visit::Visitor`) implementation which creates events for
//! files which differ from the content it would have once deployed.

use crate::{
	profile::LayeredProfile,
	profile::{source::PunktfSource, transform::Transform},
	visit::*,
};
use std::path::Path;

/// Applies any relevant [`Transform`](`crate::profile::transform::Transform`)
/// for the given file.
fn transform_content(profile: &LayeredProfile, file: &File<'_>, content: String) -> String {
	let mut content = content;

	// Copy so we exec_dotfile is not referenced by this in case an error occurs.
	let exec_transformers: Vec<_> = file.dotfile().transformers.to_vec();

	// Apply transformers.
	// Order:
	//   - Transformers which are specified in the profile root
	//   - Transformers which are specified on a specific dotfile of a profile
	for transformer in profile.transformers().chain(exec_transformers.iter()) {
		content = transformer.transform(content).unwrap();
	}

	content
}

/// An event which is emitted for every differing item.
#[derive(Debug)]
pub enum Event<'a> {
	/// File does currently not exist but would be created.
	NewFile(&'a Path),

	/// Directory does currently not exist but would be created.
	NewDirectory(&'a Path),

	/// File does exist but the contents would changed.
	Diff {
		/// Absoulte path to the target location.
		target_path: &'a Path,

		/// Contents of the current file on the filesystem.
		old_content: String,

		/// Contents of the file after a deployment.
		new_contnet: String,
	},
}

impl Event<'_> {
	/// Returns the absolute target path for the diff.
	pub const fn target_path(&self) -> &Path {
		match self {
			Self::NewFile(p) => p,
			Self::NewDirectory(p) => p,
			Self::Diff { target_path, .. } => target_path,
		}
	}
}

/// A [`Visitor`](`crate::visit::Visitor`) implementation which checks for
/// changes which would be made by a deployment.
/// For each change an [`Event`] is emitted which can be processed by [`Diff.0`].
#[derive(Debug, Clone, Copy)]
pub struct Diff<F>(F);

impl<F> Diff<F>
where
	F: Fn(Event<'_>),
{
	/// Creates a new instance of the visitor.
	pub const fn new(f: F) -> Self {
		Self(f)
	}

	/// Runs the visitor to completion for a given profile.
	pub fn diff(self, source: &PunktfSource, profile: &mut LayeredProfile) {
		let mut resolver = ResolvingVisitor(self);
		let walker = Walker::new(profile);
		walker.walk(source, &mut resolver).unwrap();
	}

	/// Emits the given event.
	fn dispatch(&self, event: Event<'_>) {
		(self.0)(event)
	}
}

impl<F> Visitor for Diff<F>
where
	F: Fn(Event<'_>),
{
	fn accept_file<'a>(
		&mut self,
		_: &PunktfSource,
		profile: &LayeredProfile,
		file: &File<'a>,
	) -> Result {
		if file.target_path.exists() {
			let new = transform_content(
				profile,
				file,
				std::fs::read_to_string(&file.source_path).unwrap(),
			);
			let old = std::fs::read_to_string(&file.target_path).unwrap();

			if new != old {
				self.dispatch(Event::Diff {
					target_path: &file.target_path,
					old_content: old,
					new_contnet: new,
				});
			}
		} else {
			self.dispatch(Event::NewFile(&file.target_path))
		}

		Ok(())
	}

	fn accept_directory<'a>(
		&mut self,
		_: &PunktfSource,
		_: &LayeredProfile,
		directory: &Directory<'a>,
	) -> Result {
		if !directory.target_path.exists() {
			self.dispatch(Event::NewDirectory(&directory.target_path))
		}

		Ok(())
	}

	fn accept_link(&mut self, _: &PunktfSource, _: &LayeredProfile, link: &Symlink) -> Result {
		log::info!(
			"[{}] Symlinks are not supported for diffs",
			link.source_path.display()
		);

		Ok(())
	}

	fn accept_rejected<'a>(
		&mut self,
		_: &PunktfSource,
		_: &LayeredProfile,
		rejected: &Rejected<'a>,
	) -> Result {
		log::info!(
			"[{}] Rejected - {}",
			rejected.relative_source_path.display(),
			rejected.reason,
		);

		Ok(())
	}

	fn accept_errored<'a>(
		&mut self,
		_: &PunktfSource,
		_: &LayeredProfile,
		errored: &Errored<'a>,
	) -> Result {
		log::error!(
			"[{}] Error - {}",
			errored.relative_source_path.display(),
			errored
		);

		Ok(())
	}
}

impl<F> TemplateVisitor for Diff<F>
where
	F: Fn(Event<'_>),
{
	fn accept_template<'a>(
		&mut self,
		_: &PunktfSource,
		profile: &LayeredProfile,
		file: &File<'a>,
		// Returns a function to resolve the content to make the resolving lazy
		// for upstream visitors.
		resolve_content: impl FnOnce(&str) -> color_eyre::Result<String>,
	) -> Result {
		if file.target_path.exists() {
			let new = transform_content(
				profile,
				file,
				resolve_content(&std::fs::read_to_string(&file.source_path).unwrap()).unwrap(),
			);
			let old = std::fs::read_to_string(&file.target_path).unwrap();

			if new != old {
				self.dispatch(Event::Diff {
					target_path: &file.target_path,
					old_content: old,
					new_contnet: new,
				});
			}
		} else {
			self.dispatch(Event::NewFile(&file.target_path))
		}

		Ok(())
	}
}
