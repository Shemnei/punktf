mod opt;
mod util;

use clap::Clap;
use color_eyre::eyre::eyre;
use color_eyre::Result;
use punktf_lib::deploy::executor::{Executor, ExecutorOptions};
use punktf_lib::profile::{resolve_profile, LayeredProfile, SimpleProfile};
use punktf_lib::PunktfSource;

pub const PUNKTF_SOURCE_ENVVAR: &str = "PUNKTF_SOURCE";
pub const PUNKTF_TARGET_ENVVAR: &str = "PUNKTF_TARGET";
pub const PUNKTF_DEFAULT_PROFILE_ENVVAR: &str = "PUNKTF_PROFILE";

fn main() -> Result<()> {
	let _ = color_eyre::install()?;

	let opts = opt::Opts::parse();

	let log_level = match opts.shared.verbose {
		0 => log::Level::Info,
		1 => log::Level::Debug,
		_ => log::Level::Trace,
	};

	let _ = env_logger::Builder::from_env(
		env_logger::Env::default().default_filter_or(log_level.as_str()),
	)
	.init();

	log::debug!("Parsed Opts:\n{:#?}", opts);

	handle_commands(opts)
}

fn handle_commands(opts: opt::Opts) -> Result<()> {
	let opt::Opts {
		shared: opt::Shared { source, .. },
		command,
	} = opts;

	match command {
		opt::Command::Deploy(opt::Deploy {
			profile: profile_name,
			target,
			dry_run,
		}) => {
			let ptf_src = PunktfSource::from_root(source.into())?;

			let mut builder = LayeredProfile::build();

			// Add target cli argument to top
			let target_cli_profile = SimpleProfile {
				target,
				..Default::default()
			};
			builder.add(String::from("target_cli_argument"), target_cli_profile);

			resolve_profile(
				&mut builder,
				&ptf_src,
				&profile_name,
				&mut Default::default(),
			)?;

			// Add target environment variable to bottom
			let target_env_profile = SimpleProfile {
				target: Some(util::get_target_path()),
				..Default::default()
			};
			builder.add(
				String::from("target_environment_variable"),
				target_env_profile,
			);

			let profile = builder.finish();

			log::debug!("Profile:\n{:#?}", profile);
			log::debug!("Source: {}", ptf_src.root().display());
			log::debug!("Target: {:?}", profile.target_path());

			// Setup environment
			std::env::set_var("PUNKTF_CURRENT_SOURCE", ptf_src.root());
			if let Some(target) = profile.target_path() {
				std::env::set_var("PUNKTF_CURRENT_TARGET", target);
			}
			std::env::set_var("PUNKTF_CURRENT_PROFILE", profile_name);

			let options = ExecutorOptions { dry_run };

			let deployer = Executor::new(options, util::ask_user_merge);

			let deployment = deployer.deploy(ptf_src, &profile);

			match deployment {
				Ok(deployment) => {
					log::debug!("Deployment:\n{:#?}", deployment);
					util::log_deployment(&deployment);

					if deployment.status().is_failed() {
						Err(eyre!("Some dotfiles failed to deploy"))
					} else {
						Ok(())
					}
				}
				Err(err) => {
					log::error!("Deployment aborted: {}", err);
					Err(err)
				}
			}
		}
	}
}
