use std::path::{Path, PathBuf};

use color_eyre::owo_colors::OwoColorize;
use color_eyre::Result;
use punktf_lib::deploy::deployment::{Deployment, DeploymentStatus};
use punktf_lib::deploy::dotfile::DotfileStatus;

pub fn get_target_path() -> PathBuf {
	std::env::var_os(super::PUNKTF_TARGET_ENVVAR)
		.unwrap_or_else(|| {
			panic!(
				"No environment variable `{}` set. Either set this variable, use the `-t/--target`
				argument or use the profile attribute `target`.",
				super::PUNKTF_TARGET_ENVVAR
			)
		})
		.into()
}

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

pub fn log_deployment(deployment: &Deployment) {
	let mut out = String::new();

	let mut files_success = 0;
	for (idx, (path, _)) in deployment
		.dotfiles()
		.iter()
		.filter(|(_, v)| v.status().is_success())
		.enumerate()
	{
		if idx == 0 {
			out.push_str(&format!("Dotfiles ({})", "SUCCESS".green()));
		}

		out.push_str(&format!("\n\t{}", path.display().bright_black()));
		files_success += 1;
	}

	if !out.is_empty() {
		log::info!("{}", out);
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
			out.push_str(&format!("Dotfiles ({})", "SKIPPED".yellow()));
		}

		out.push_str(&format!(
			"\n\t{}: {}",
			path.display(),
			reason.bright_black()
		));
		files_skipped += 1;
	}

	if !out.is_empty() {
		log::warn!("{}", out);
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
			out.push_str(&format!("Dotfiles ({})", "FAILED".red()));
		}

		out.push_str(&format!(
			"\n\t{}: {}",
			path.display(),
			reason.bright_black()
		));
		files_failed += 1;
	}

	if !out.is_empty() {
		log::error!("{}", out);
		out.clear();
	}

	match deployment.status() {
		DeploymentStatus::Success => {
			out.push_str(&format!("Status: {}", "SUCCESS".green()));
		}
		DeploymentStatus::Failed(reason) => {
			out.push_str(&format!("Status: {}\n\t{}", "FAILED".red(), reason));
		}
	};

	let files_total = files_success + files_skipped + files_failed;
	let elapsed = deployment
		.duration()
		.expect("Failed to get duration from deployment");

	out.push_str(&format!("\nTime            : {:?}", elapsed));
	out.push_str(&format!("\nFiles (deployed): {}", files_success));
	out.push_str(&format!("\nFiles (skipped) : {}", files_skipped));
	out.push_str(&format!("\nFiles (failed)  : {}", files_failed));
	out.push_str(&format!("\nFiles (total)   : {}", files_total));

	log::info!("{}", out);
}