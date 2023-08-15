pub mod env;
pub mod hook;
pub mod item;
pub mod merge;
pub mod prio;
pub mod profile;
pub mod template;
pub mod transform;
pub mod value;
pub mod version;

#[test]
#[ignore = "debugging"]
fn main() -> Result<(), Box<dyn std::error::Error>> {
	use std::str::FromStr;

	let profile = std::fs::read_to_string("profile.yaml")?;
	let p = profile::Profile::from_str(&profile)?;

	println!("{p:#?}");

	Ok(())
}

#[test]
#[ignore = "debugging"]
fn prnp() {
	use crate::hook::Hook;
	use crate::{item::Item, prio::Priority};
	use env::Environment;
	use profile::{Profile, ProfileVersion};
	use std::path::PathBuf;
	use transform::Transformer;
	use value::Value;

	use crate::profile::Shared;

	let p = Profile {
		version: ProfileVersion {
			version: Profile::VERSION,
		},
		aliases: vec!["Foo".into(), "Bar".into()],
		extends: vec!["Parent".into()],
		target: Some(PathBuf::from("Test")),
		shared: Shared {
			environment: Environment(
				[
					("Foo".into(), Value::String("Bar".into())),
					("Bool".into(), Value::Bool(true)),
				]
				.into_iter()
				.collect(),
			),
			transformers: vec![Transformer::LineTerminator(transform::LineTerminator::LF)],
			pre_hook: Some(Hook::Inline("set -eoux pipefail\necho 'Foo'".into())),
			post_hook: Some(Hook::File("Test".into())),
			priority: Some(Priority(5)),
		},

		items: vec![Item {
			shared: Shared {
				environment: Environment(
					[
						("Foo".into(), Value::String("Bar".into())),
						("Bool".into(), Value::Bool(true)),
					]
					.into_iter()
					.collect(),
				),
				transformers: vec![Transformer::LineTerminator(transform::LineTerminator::LF)],
				pre_hook: None,
				post_hook: None,
				priority: Some(Priority(5)),
			},
			path: PathBuf::from("/dev/null"),
			rename: None,
			overwrite_target: None,
			merge: None,
		}],
	};

	serde_yaml::to_writer(std::io::stdout(), &p).unwrap();
}

#[test]
#[ignore = "debugging"]
fn prni() {
	use crate::hook::Hook;
	use crate::{item::Item, prio::Priority};
	use env::Environment;
	use std::path::PathBuf;
	use transform::Transformer;
	use value::Value;

	use crate::profile::Shared;

	let i = Item {
		shared: Shared {
			environment: Environment(
				[
					("Foo".into(), Value::String("Bar".into())),
					("Bool".into(), Value::Bool(true)),
				]
				.into_iter()
				.collect(),
			),
			transformers: vec![Transformer::LineTerminator(transform::LineTerminator::LF)],
			pre_hook: Some(Hook::Inline("set -eoux pipefail\necho 'Foo'".into())),
			post_hook: None,
			priority: Some(Priority(5)),
		},
		path: PathBuf::from("/dev/null"),
		rename: None,
		overwrite_target: None,
		merge: None,
	};

	serde_yaml::to_writer(std::io::stdout(), &i).unwrap();
}
