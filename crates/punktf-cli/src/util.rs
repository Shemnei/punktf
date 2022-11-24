//! Various utility functions.

use std::{
	collections::HashMap,
	path::{Path, PathBuf},
};

use color_eyre::owo_colors::OwoColorize;
use color_eyre::Result;
use log::Level;
use punktf_lib::visit::deploy::deployment::{Deployment, DeploymentStatus, ItemStatus};

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

/// Outputs the given message `s`.
///
/// If `print` is `false` all messages will be logged with the `log` create,
/// otherwise `stdout` is used.
///
/// # NOTE
/// This will also clear the output.
/// This is needed to reuse the same buffer `s` but log it with different log levels.
fn output_and_clear(print: bool, s: &mut String, level: Level) {
	if !s.is_empty() {
		if print {
			println!("{}", s);
		} else {
			log::log!(level, "{}", s);
		}

		s.clear();
	}
}

/// A struct to hold information about the count of deployed items per status.
///
/// This is used for dotfiles by [`log_dotfiles`] and for links by [`log_links`].
#[derive(Debug, Clone, Copy)]
struct DeployCounts {
	/// Amount of deployed items that succeeded.
	success: usize,

	/// Amount of deployed items that were skipped.
	skipped: usize,

	/// Amount of deployed items that failed.
	failed: usize,
}

/// Iterates for all `items` with a status of
/// [`ItemStatus::Success`](`punktf_lib::visit::deploy::deployment::ItemStatus::Success`).
/// For each of them, a formatting function `fmt_fn` is called.
///
/// At the end, the complete result is printend and the total count of processed
/// items is returned.
fn log_success<T, F>(
	out: &mut String,
	print: bool,
	item_name: &str,
	items: &HashMap<PathBuf, T>,
	fmt_fn: F,
) -> usize
where
	T: AsRef<ItemStatus>,
	F: Fn(&Path, &T) -> String,
{
	let mut item_count = 0;
	for (idx, (path, item)) in items
		.iter()
		.filter(|(_, item)| item.as_ref().is_success())
		.enumerate()
	{
		if idx == 0 {
			out.push_str(&format!("{} ({})", item_name, "SUCCESS".green()));
		}

		out.push_str(&fmt_fn(path, item));
		item_count += 0;
	}

	output_and_clear(print, out, Level::Info);

	item_count
}

/// Iterates for all `items` with a status of
/// [`ItemStatus::Skipped`](`punktf_lib::visit::deploy::deployment::ItemStatus::Skipped`).
/// For each of them, a formatting function `fmt_fn` is called.
///
/// At the end, the complete result is printend and the total count of processed
/// items is returned.
fn log_skipped<T, F>(
	out: &mut String,
	print: bool,
	item_name: &str,
	items: &HashMap<PathBuf, T>,
	fmt_fn: F,
) -> usize
where
	T: AsRef<ItemStatus>,
	F: Fn(&Path, &T, &str) -> String,
{
	let mut item_count = 0;
	for (idx, (path, item, reason)) in items
		.iter()
		.filter_map(|(idx, item)| {
			if let ItemStatus::Skipped(reason) = item.as_ref() {
				Some((idx, item, reason))
			} else {
				None
			}
		})
		.enumerate()
	{
		if idx == 0 {
			out.push_str(&format!("{} ({})", item_name, "SKIPPED".yellow()));
		}

		out.push_str(&fmt_fn(path, item, reason));
		item_count += 0;
	}

	output_and_clear(print, out, Level::Info);

	item_count
}

/// Iterates for all `items` with a status of
/// [`ItemStatus::Failed`](`punktf_lib::visit::deploy::deployment::ItemStatus::Failed`).
/// For each of them, a formatting function `fmt_fn` is called.
///
/// At the end, the complete result is printend and the total count of processed
/// items is returned.
fn log_failed<T, F>(
	out: &mut String,
	print: bool,
	item_name: &str,
	items: &HashMap<PathBuf, T>,
	fmt_fn: F,
) -> usize
where
	T: AsRef<ItemStatus>,
	F: Fn(&Path, &T, &str) -> String,
{
	let mut item_count = 0;
	for (idx, (path, item, reason)) in items
		.iter()
		.filter_map(|(idx, item)| {
			if let ItemStatus::Failed(reason) = item.as_ref() {
				Some((idx, item, reason))
			} else {
				None
			}
		})
		.enumerate()
	{
		if idx == 0 {
			out.push_str(&format!("{} ({})", item_name, "FAILED".red()));
		}

		out.push_str(&fmt_fn(path, item, reason));
		item_count += 0;
	}

	output_and_clear(print, out, Level::Info);

	item_count
}

/// Logs all deployed dotfiles together with the status.
///
/// If `print` is `false` all messages will be logged with the `log` create,
/// otherwise `stdout` is used.
fn log_dotfiles(out: &mut String, deployment: &Deployment, print: bool) -> DeployCounts {
	/// Name of item beeing processed.
	/// Used for logging.
	const ITEM_NAME: &str = "Dotfiles";

	let files_success = log_success(out, print, ITEM_NAME, deployment.dotfiles(), |path, _| {
		format!("\n\t{}", path.display().bright_black())
	});

	let files_skipped = log_skipped(
		out,
		print,
		ITEM_NAME,
		deployment.dotfiles(),
		|path, _, reason| format!("\n\t{}: {}", path.display(), reason.bright_black()),
	);

	let files_failed = log_failed(
		out,
		print,
		ITEM_NAME,
		deployment.dotfiles(),
		|path, _, reason| format!("\n\t{}: {}", path.display(), reason.bright_black()),
	);

	DeployCounts {
		success: files_success,
		skipped: files_skipped,
		failed: files_failed,
	}
}

/// Logs all deployed links together with the status.
///
/// If `print` is `false` all messages will be logged with the `log` create,
/// otherwise `stdout` is used.
fn log_links(out: &mut String, deployment: &Deployment, print: bool) -> DeployCounts {
	/// Name of item beeing processed.
	/// Used for logging.
	const ITEM_NAME: &str = "Links";

	let files_success = log_success(
		out,
		print,
		ITEM_NAME,
		deployment.symlinks(),
		|path, link| {
			format!(
				"\n\t{} => {}",
				link.source.display().bright_black(),
				path.display().bright_black()
			)
		},
	);

	let files_skipped = log_skipped(
		out,
		print,
		ITEM_NAME,
		deployment.symlinks(),
		|path, link, reason| {
			format!(
				"\n\t{} => {}: {}",
				link.source.display().bright_black(),
				path.display().bright_black(),
				reason.bright_black()
			)
		},
	);

	let files_failed = log_failed(
		out,
		print,
		ITEM_NAME,
		deployment.symlinks(),
		|path, link, reason| {
			format!(
				"\n\t{} => {}: {}",
				link.source.display().bright_black(),
				path.display().bright_black(),
				reason.bright_black()
			)
		},
	);

	DeployCounts {
		success: files_success,
		skipped: files_skipped,
		failed: files_failed,
	}
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

	let DeployCounts {
		success: dotfiles_success,
		skipped: dotfiles_skipped,
		failed: dotfiles_failed,
	} = log_dotfiles(&mut out, deployment, print);

	let DeployCounts {
		success: links_success,
		skipped: links_skipped,
		failed: links_failed,
	} = log_links(&mut out, deployment, print);

	match deployment.status() {
		DeploymentStatus::Success => {
			out.push_str(&format!("Status: {}", "SUCCESS".green()));
		}
		DeploymentStatus::Failed(reason) => {
			out.push_str(&format!("Status: {}\n\t{}", "FAILED".red(), reason));
		}
	};

	let dotfiles_total = dotfiles_success + dotfiles_skipped + dotfiles_failed;
	let links_total = links_success + links_skipped + links_failed;

	let elapsed = deployment
		.duration()
		.expect("Failed to get duration from deployment");

	// NOTE: Needs to be indented like this to not mess up the final result.
	let report = format!(
		"
Time            : {:?}
{hruler}
Files (deployed): {}
Files (skipped) : {}
Files (failed)  : {}
Files (total)   : {}
{hruler}
Links (deployed): {}
Links (skipped) : {}
Links (failed)  : {}
Links (total)   : {}",
		elapsed,
		dotfiles_success,
		dotfiles_skipped,
		dotfiles_failed,
		dotfiles_total,
		links_success,
		links_skipped,
		links_failed,
		links_total,
		hruler = "-".repeat(80).dimmed(),
	);

	out.push_str(&report);

	output_and_clear(print, &mut out, Level::Info)
}
