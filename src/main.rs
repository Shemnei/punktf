use core::fmt;
use std::path::PathBuf;
use std::str::FromStr;

use clap::Clap;

// Used so that it defaults to current_dir if no value is given.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct HomePath(PathBuf);

impl Default for HomePath {
	fn default() -> Self {
		Self(std::env::current_dir().unwrap())
	}
}

impl fmt::Display for HomePath {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		fmt::Display::fmt(&self.0.display(), f)
	}
}

impl FromStr for HomePath {
	type Err = std::convert::Infallible;

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		Ok(Self(PathBuf::from(s)))
	}
}

#[derive(Debug, Clap)]
struct Opts {
	#[clap(flatten)]
	shared: Shared,

	#[clap(subcommand)]
	command: Command,
}

#[derive(Debug, Clap)]
struct Shared {
	#[clap(short, long, env = "PUNKTF_HOME", default_value)]
	home: HomePath,
}

#[derive(Debug, Clap)]
enum Command {
	Deploy(Deploy),
}

#[derive(Debug, Clap)]
struct Deploy {
	profile: String,

	#[clap(short, long)]
	dry_run: bool,
}

fn main() {
	let opts: Opts = Opts::parse();

	println!("{:?}", opts.shared.home);
}
