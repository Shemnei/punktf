//! All code related to command line argument parsing.

// We allow missing documentation for this module, as any documentation put on
// the cli struct will appear in the help message which, in most cases, is not
// what we want.
#![allow(missing_docs, clippy::missing_docs_in_private_items)]

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(author, version, about, long_about = None)]
pub struct Opts {
	#[command(flatten)]
	pub shared: Shared,

	#[command(subcommand)]
	pub command: Command,
}

#[derive(Debug, Args)]
pub struct Shared {
	/// The source directory where the profiles and dotfiles are located.
	#[arg(short, long, env = super::PUNKTF_SOURCE_ENVVAR)]
	pub source: PathBuf,

	/// Runs with specified level of verbosity which affects the log level.
	///
	/// The level can be set by repeating the flag `n` times (e.g. `-vv` for 2).
	/// Levels:
	///     1 - `Info`;
	///     2 - `Debug`;
	///     3 - `Trace`.
	#[arg(short, long, action = clap::ArgAction::Count)]
	pub verbose: u8,
}

#[derive(Debug, Subcommand)]
pub enum Command {
	Deploy(Deploy),
	Render(Render),
}

/// Deploys a profile.
#[derive(Debug, Parser)]
pub struct Deploy {
	/// Name of the profile to deploy.
	///
	/// The name should be the file name of the profile without an extension (e.g.
	/// `profiles/arch.json` should be given as `arch`).
	#[arg(short, long, env = super::PUNKTF_PROFILE_ENVVAR)]
	pub profile: String,

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
}

/// Prints the resolved dotfile to stdout.
///
/// This is mainly intended for template dotifles to see the what the real content
/// would look like once it is deployed.
#[derive(Debug, Parser)]
pub struct Render {
	/// Name of the profile to deploy.
	///
	/// The name should be the file name of the profile without an extension (e.g.
	/// `profiles/arch.json` should be given as `arch`).
	#[arg(short, long, env = super::PUNKTF_PROFILE_ENVVAR)]
	pub profile: String,

	/// Dotfile to render.
	///
	/// Relative path starting from the `dotfiles` directory.
	pub dotfile: PathBuf,
}
