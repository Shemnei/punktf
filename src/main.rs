use core::fmt;
use std::ops::Deref;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use clap::Clap;
use punktf::deploy::executor::{Executor, ExecutorOptions};
use punktf::{resolve_profile, Profile};

// Used so that it defaults to current_dir if no value is given.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct SourcePath(PathBuf);

impl Default for SourcePath {
	fn default() -> Self {
		Self(std::env::current_dir().expect(
			"Failed to get `current_dir`. Please either use the `-s/--source` argument or the \
			 environment variable `PUNKTF_SOURCE` to set the source directory.",
		))
	}
}

impl fmt::Display for SourcePath {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		fmt::Display::fmt(&self.0.display(), f)
	}
}

impl Deref for SourcePath {
	type Target = PathBuf;

	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

impl FromStr for SourcePath {
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
	#[clap(short, long, env = "PUNKTF_SOURCE", default_value)]
	source: SourcePath,
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

// TODO: check/remove unwrap's
// TODO: target path as cli arg

fn main() {
	env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("debug")).init();

	let opts: Opts = Opts::parse();

	println!("{:#?}", opts);

	match opts.command {
		Command::Deploy(cmd) => {
			let profile_path = opts.shared.source.join("profiles");

			let profile: Profile = resolve_profile(&profile_path, &cmd.profile);

			println!("{:#?}", profile);
			//println!("{}", serde_yaml::to_string(&profile).unwrap());

			let options = ExecutorOptions {
				dry_run: cmd.dry_run,
			};

			let deployer = Executor::new(options, ask_user_merge);

			let deployment = deployer
				.deploy(opts.shared.source.join("items"), profile)
				.unwrap();

			println!("{:#?}", deployment);
		}
	}
}

fn ask_user_merge(deploy_path: &Path, source_path: &Path) -> bool {
	use std::io::Write;

	let stdin = std::io::stdin();
	let mut stdout = std::io::stdout();
	let mut line = String::new();

	loop {
		stdout
			.write_all(
				format!(
					"Overwrite `{}` with `{}` [y/N]:",
					deploy_path.display(),
					source_path.display()
				)
				.as_bytes(),
			)
			.unwrap();

		stdout.flush().unwrap();

		stdin.read_line(&mut line).unwrap();

		line.make_ascii_lowercase();

		return match line.trim() {
			"y" => true,
			"n" => false,
			_ => {
				line.clear();
				continue;
			}
		};
	}
}
