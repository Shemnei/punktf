use crate::{
	profile::{visit::*, LayeredProfile},
	transform::Transform,
	PunktfSource,
};
use std::path::Path;

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

#[derive(Debug)]
pub enum Event<'a> {
	NewFile(&'a Path),
	NewDirectory(&'a Path),
	Diff {
		target_path: &'a Path,
		old_content: String,
		new_contnet: String,
	},
}

impl Event<'_> {
	pub const fn target_path(&self) -> &Path {
		match self {
			Self::NewFile(p) => p,
			Self::NewDirectory(p) => p,
			Self::Diff { target_path, .. } => target_path,
		}
	}
}

#[derive(Debug, Clone, Copy)]
pub struct Diff<F>(F);

impl<F> Diff<F>
where
	F: Fn(Event<'_>),
{
	pub const fn new(f: F) -> Self {
		Self(f)
	}

	pub fn diff(self, source: &PunktfSource, profile: &mut LayeredProfile) {
		let mut resolver = ResolvingVisitor::new(self);
		let walker = Walker::new(profile);
		walker.walk(source, &mut resolver).unwrap();
	}

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

	fn accept_link(&mut self, _: &PunktfSource, _: &LayeredProfile, _: &Symlink) -> Result {
		todo!()
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
			"[{}] Error - {}: {}",
			errored.relative_source_path.display(),
			errored.context,
			errored.error
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
