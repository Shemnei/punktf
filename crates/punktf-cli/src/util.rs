//! Various utility functions.

use std::fmt::Write as _; // Needed for `write!` calls
use std::path::{Path, PathBuf};

use color_eyre::owo_colors::OwoColorize;
use color_eyre::Result;
use punktf_lib::visit::deploy::deployment::{Deployment, DeploymentStatus, DotfileStatus};

/// Retrieves the target path for the deployment by reading the environment
/// variable with the name determined by [`super::PUNKTF_TARGET_ENVVAR`].
pub fn get_target_path() -> Option<PathBuf> {
	std::env::var_os(super::PUNKTF_TARGET_ENVVAR).map(|val| val.into())
}

/// Function which get's called when a merge conflict arises and the merge mode
/// of the [dotfile](`punktf_lib::profile::dotfile::Dotfile`) is set to
/// [MergeMode::Ask](`punktf_lib::profile::MergeMode::Ask`). The function will
/// ask the user to accept the merge (`y`) or deny it (`n`) via the command line
/// ([`std::io::stdout`]/[`std::io::stdin`]). If an invalid answer is given it
/// will ask again until a valid answer is given.
pub fn ask_user_merge(source_path: &Path, deploy_path: &Path) -> Result<bool> {
	use std::io::Write;

	let stdin = std::io::stdin();
	let mut stdout = std::io::stdout();
	let mut line = String::new();

	loop {
		stdout.write_all(
			format!(
				"Overwrite `{}` with `{}` [y/N]: ",
				deploy_path.display(),
				source_path.display()
			)
			.as_bytes(),
		)?;

		stdout.flush()?;

		stdin.read_line(&mut line)?;

		line.make_ascii_lowercase();

		return match line.trim() {
			"y" => Ok(true),
			"n" => Ok(false),
			_ => {
				line.clear();
				continue;
			}
		};
	}
}

/// Logs all deployed dotfiles together with the status.
fn log_dotfiles(out: &mut String, deployment: &Deployment, print: bool) -> (usize, usize, usize) {
	let mut files_success = 0;
	for (idx, (path, _)) in deployment
		.dotfiles()
		.iter()
		.filter(|(_, v)| v.status().is_success())
		.enumerate()
	{
		if idx == 0 {
			write!(out, "Dotfiles ({})", "SUCCESS".green()).expect("Write to String failed");
		}

		write!(out, "\n\t{}", path.display().bright_black()).expect("Write to String failed");
		files_success += 1;
	}

	if !out.is_empty() {
		if print {
			println!("{}", out);
		} else {
			log::info!("{}", out);
		}

		out.clear();
	}

	let mut files_skipped = 0;
	for (idx, (path, reason)) in deployment
		.dotfiles()
		.iter()
		.filter_map(|(k, v)| {
			if let DotfileStatus::Skipped(reason) = v.status() {
				Some((k, reason))
			} else {
				None
			}
		})
		.enumerate()
	{
		if idx == 0 {
			write!(out, "Dotfiles ({})", "SKIPPED".yellow()).expect("Write to String failed");
		}

		write!(out, "\n\t{}: {}", path.display(), reason.bright_black())
			.expect("Write to String failed");

		files_skipped += 1;
	}

	if !out.is_empty() {
		if print {
			println!("{}", out);
		} else {
			log::warn!("{}", out);
		}

		out.clear();
	}

	let mut files_failed = 0;
	for (idx, (path, reason)) in deployment
		.dotfiles()
		.iter()
		.filter_map(|(k, v)| {
			if let DotfileStatus::Failed(reason) = v.status() {
				Some((k, reason))
			} else {
				None
			}
		})
		.enumerate()
	{
		if idx == 0 {
			write!(out, "Dotfiles ({})", "FAILED".red()).expect("Write to String failed");
		}

		write!(out, "\n\t{}: {}", path.display(), reason.bright_black())
			.expect("Write to String failed");

		files_failed += 1;
	}

	if !out.is_empty() {
		if print {
			println!("{}", out);
		} else {
			log::error!("{}", out);
		}

		out.clear();
	}

	(files_success, files_skipped, files_failed)
}

/// Logs all deployed links together with the status.
fn log_links(out: &mut String, deployment: &Deployment, print: bool) -> (usize, usize, usize) {
	let mut files_success = 0;
	for (idx, (path, link)) in deployment
		.symlinks()
		.iter()
		.filter(|(_, v)| v.status().is_success())
		.enumerate()
	{
		if idx == 0 {
			write!(out, "Links ({})", "SUCCESS".green()).expect("Write to String failed");
		}

		write!(
			out,
			"\n\t{} => {}",
			link.source.display().bright_black(),
			path.display().bright_black(),
		)
		.expect("Write to String failed");
		files_success += 1;
	}

	if !out.is_empty() {
		if print {
			println!("{}", out);
		} else {
			log::info!("{}", out);
		}

		out.clear();
	}

	let mut files_skipped = 0;
	for (idx, (path, link, reason)) in deployment
		.symlinks()
		.iter()
		.filter_map(|(k, v)| {
			if let DotfileStatus::Skipped(reason) = v.status() {
				Some((k, v, reason))
			} else {
				None
			}
		})
		.enumerate()
	{
		if idx == 0 {
			write!(out, "Links ({})", "SKIPPED".yellow()).expect("Write to String failed");
		}

		write!(
			out,
			"\n\t{} => {}: {}",
			link.source.display().bright_black(),
			path.display().bright_black(),
			reason.bright_black()
		)
		.expect("Write to String failed");

		files_skipped += 1;
	}

	if !out.is_empty() {
		if print {
			println!("{}", out);
		} else {
			log::warn!("{}", out);
		}

		out.clear();
	}

	let mut files_failed = 0;
	for (idx, (path, link, reason)) in deployment
		.symlinks()
		.iter()
		.filter_map(|(k, v)| {
			if let DotfileStatus::Failed(reason) = v.status() {
				Some((k, v, reason))
			} else {
				None
			}
		})
		.enumerate()
	{
		if idx == 0 {
			write!(out, "Links ({})", "FAILED".red()).expect("Write to String failed");
		}

		write!(
			out,
			"\n\t{} => {}: {}",
			link.source.display().bright_black(),
			path.display().bright_black(),
			reason.bright_black()
		)
		.expect("Write to String failed");

		files_failed += 1;
	}

	if !out.is_empty() {
		if print {
			println!("{}", out);
		} else {
			log::error!("{}", out);
		}

		out.clear();
	}

	(files_success, files_skipped, files_failed)
}

/// Logs the finished state of the
/// [deployment](`punktf_lib::visit::deploy::deployment::Deployment`).
/// If the `print` argument is `true` then stdout will be used, otherwise the
/// crate [`log`] is used.
/// This includes amount, state and the names of the deployed
/// [dotfiles](`punktf_lib::profile::dotfile::Dotfile`) and also the total time
/// the deployment took to execute.
pub fn log_deployment(deployment: &Deployment, print: bool) {
	let mut out = String::new();

	let (dotfiles_success, dotfiles_skipped, dotfiles_failed) =
		log_dotfiles(&mut out, deployment, print);
	let (links_success, links_skipped, links_failed) = log_links(&mut out, deployment, print);

	match deployment.status() {
		DeploymentStatus::Success => {
			write!(out, "Status: {}", "SUCCESS".green()).expect("Write to String failed");
		}
		DeploymentStatus::Failed(reason) => {
			write!(out, "Status: {}\n\t{}", "FAILED".red(), reason)
				.expect("Write to String failed");
		}
	};

	let dotfiles_total = dotfiles_success + dotfiles_skipped + dotfiles_failed;
	let links_total = links_success + links_skipped + links_failed;

	let elapsed = deployment
		.duration()
		.expect("Failed to get duration from deployment");

	write!(out, "\nTime            : {:?}", elapsed).expect("Write to String failed");
	write!(out, "\n{}", "-".repeat(80).dimmed()).expect("Write to String failed");
	write!(out, "\nFiles (deployed): {}", dotfiles_success).expect("Write to String failed");
	write!(out, "\nFiles (skipped) : {}", dotfiles_skipped).expect("Write to String failed");
	write!(out, "\nFiles (failed)  : {}", dotfiles_failed).expect("Write to String failed");
	write!(out, "\nFiles (total)   : {}", dotfiles_total).expect("Write to String failed");
	write!(out, "\n{}", "-".repeat(80).dimmed()).expect("Write to String failed");
	write!(out, "\nLinks (deployed): {}", links_success).expect("Write to String failed");
	write!(out, "\nLinks (skipped) : {}", links_skipped).expect("Write to String failed");
	write!(out, "\nLinks (failed)  : {}", links_failed).expect("Write to String failed");
	write!(out, "\nLinks (total)   : {}", links_total).expect("Write to String failed");

	if print {
		println!("{}", out);
	} else {
		log::info!("{}", out);
	}
}
