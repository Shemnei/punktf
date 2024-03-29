//! All code related to command line argument parsing.

// We allow missing documentation for this module, as any documentation put on
// the cli struct will appear in the help message which, in most cases, is not
// what we want.
#![allow(
	missing_docs,
	clippy::missing_docs_in_private_items,
	unused_qualifications
)]

use std::path::PathBuf;

use clap::{ArgGroup, Args, Parser, Subcommand, ValueEnum};
use clap_complete::Shell;

#[derive(Debug, Parser)]
#[command(author, version, about, long_about = None)]
pub struct Opts {
	#[command(flatten)]
	pub shared: Shared,

	#[command(subcommand)]
	pub command: Command,
}

#[derive(Debug, Args)]
#[command(
	group(
		ArgGroup::new("verbosity")
			.required(false)
			.args(["verbose", "quite"]),
	)
)]
pub struct Shared {
	/// Runs with specified level of verbosity which affects the log level.
	///
	/// The level can be set by repeating the flag `n` times (e.g. `-vv` for 2).
	/// Levels:
	///     1 - `Info`;
	///     2 - `Debug`;
	///     3 - `Trace`.
	#[arg(short, long, action = clap::ArgAction::Count)]
	pub verbose: u8,

	/// Quite mode
	///
	/// Will only print errors
	#[arg(short, long)]
	pub quite: bool,
}

#[derive(Debug, Args)]
pub struct OutputShared {
	/// Writes the deployment status as json to the given path.
	#[arg(long)]
	pub json_output: Option<PathBuf>,

	/// Writes the deployment status as yaml to the given path.
	#[arg(long)]
	pub yaml_output: Option<PathBuf>,
}

#[derive(Debug, Subcommand)]
pub enum Command {
	Deploy(Deploy),
	Render(Render),
	Verify(Verify),
	Diff(Diff),
	Man(Man),
	Completions(Completions),
}

#[derive(Debug, Args)]
pub struct RepoShared {
	/// The source directory where the profiles and dotfiles are located.
	#[arg(short, long, env = super::PUNKTF_SOURCE_ENVVAR)]
	pub source: PathBuf,

	/// Name of the profile to deploy.
	///
	/// The name should be the file name of the profile without an extension (e.g.
	/// `profiles/arch.json` should be given as `arch`).
	#[arg(short, long, env = super::PUNKTF_PROFILE_ENVVAR)]
	pub profile: String,
}

/// Deploys a profile.
#[derive(Debug, Parser)]
pub struct Deploy {
	#[command(flatten)]
	pub shared: RepoShared,

	/// Alternative deployment target path.
	///
	/// This path will take precedence over all other ways to define a deployment
	/// path.
	#[arg(short, long)]
	pub target: Option<PathBuf>,

	/// Deploys the profile but without actually coping/creating the files.
	///
	/// This can be used to test and get an overview over the changes which would
	/// be applied when run without this flag.
	#[arg(short, long)]
	pub dry_run: bool,

	#[command(flatten)]
	pub output: OutputShared,
}

/// Prints the resolved dotfile to stdout.
///
/// This is mainly intended for template dotifles to see the what the real content
/// would look like once it is deployed.
#[derive(Debug, Parser)]
pub struct Render {
	#[command(flatten)]
	pub shared: RepoShared,

	/// Dotfile to render.
	///
	/// Relative path starting from the `dotfiles` directory.
	pub dotfile: PathBuf,
}

/// Verifies a profile.
///
/// This includes checking and resolving templates, running hooks.
/// No actual file operations will be executed.
///
/// # NOTE
/// This will run pre-, and post-hooks.
///
/// Similar to `deploy --dry-run` but does not require the `target` or `dry-run`
/// arguments.
#[derive(Debug, Parser)]
pub struct Verify {
	#[command(flatten)]
	pub shared: RepoShared,

	#[command(flatten)]
	pub output: OutputShared,
}

/// Format of the output of the [`Diff`] command.
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum DiffFormat {
	/// Pretty prints the diffs to stdout.
	#[default]
	Pretty,

	/// Print the diffs as the gnu unified diff format.
	///
	/// Can be used to pipe into pagers.
	Unified,
}

/// Prints differences to already deployed files for a profile.
///
/// Similar to `deploy --dry-run` but does not require the `target` or `dry-run`
/// arguments.
#[derive(Debug, Parser)]
pub struct Diff {
	#[command(flatten)]
	pub shared: RepoShared,

	/// Defines the output format for the diffs.
	#[arg(value_enum, short, long, default_value_t = DiffFormat::Pretty)]
	pub format: DiffFormat,
}

/// Generates man pages for this application.
#[derive(Debug, Parser)]
pub struct Man {
	/// Output path for the man pages.
	#[arg(short, long, default_value = ".")]
	pub output: PathBuf,
}

/// Generates shell completions for this application.
#[derive(Debug, Parser)]
pub struct Completions {
	/// Shell for which to generate the completions.
	pub shell: Shell,

	/// Output path for the man pages.
	#[arg(short, long, default_value = ".")]
	pub output: PathBuf,
}
