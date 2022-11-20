use similar::{ChangeTag, TextDiff, udiff::unified_diff};

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

#[derive(Default, Debug, Clone, Copy)]
pub struct Diff;

impl Diff {
	pub fn diff(self, source: &PunktfSource, profile: &mut LayeredProfile) {
		let mut resolver = ResolvingVisitor::new(self);
		let walker = Walker::new(profile);
		walker.walk(source, &mut resolver).unwrap();
	}
}

impl Visitor for Diff {
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

			diff(&file.target_path, &old, &new);
		} else {
			println!("[{}]: New file", file.target_path.display());
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
			println!("[{}]: New directory", directory.target_path.display());
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

impl TemplateVisitor for Diff {
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

			diff(&file.target_path, &old, &new);
		} else {
			println!("[{}]: New file", file.target_path.display());
		}

		Ok(())
	}
}

fn diff(target: &Path, old: &str, new: &str) {
	let diff = TextDiff::from_lines(old, new);

	for (idx, group) in diff.grouped_ops(3).iter().enumerate() {
		if idx == 0 {
			println!("{}", ">".repeat(80));
			println!("Diff of {}", target.display());
			println!("{}", ">".repeat(80));

			let diff = unified_diff(similar::Algorithm::Lcs, old, new, 3, None);

			println!("{}", "=".repeat(80));
			println!("{diff}");
			println!("{}", "=".repeat(80));
		}

		if idx > 0 {
			println!("{:-^1$}", "-", 80);
		}

		for op in group {
			for change in diff.iter_changes(op) {
				let sign = match change.tag() {
					ChangeTag::Delete => "-",
					ChangeTag::Insert => "+",
					ChangeTag::Equal => " ",
				};

				print!(
					"{} {} |{} {}",
					change
						.old_index()
						.map(|d| d.to_string())
						.unwrap_or_else(|| " ".into()),
					change
						.new_index()
						.map(|d| d.to_string())
						.unwrap_or_else(|| " ".into()),
					sign,
					change.to_string_lossy()
				);

				if change.missing_newline() {
					println!();
				}
			}
		}
	}
}
