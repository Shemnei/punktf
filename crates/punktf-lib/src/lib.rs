#![allow(
	dead_code,
	rustdoc::private_intra_doc_links,
	clippy::needless_lifetimes
)]
#![deny(
	deprecated_in_future,
	exported_private_dependencies,
	future_incompatible,
	missing_copy_implementations,
	rustdoc::missing_crate_level_docs,
	rustdoc::broken_intra_doc_links,
	missing_docs,
	clippy::missing_docs_in_private_items,
	missing_debug_implementations,
	private_in_public,
	rust_2018_compatibility,
	rust_2018_idioms,
	trivial_casts,
	trivial_numeric_casts,
	unsafe_code,
	unstable_features,
	unused_import_braces,
	unused_qualifications,

	// clippy attributes
	clippy::missing_const_for_fn,
	clippy::redundant_pub_crate,
	// 2022-05-31: Disabled as this lint appears to have many false positives
	// clippy::use_self
)]
#![cfg_attr(docsrs, feature(doc_cfg), feature(doc_alias))]

//! This is the library powering `punktf`, a cross-platform multi-target dotfiles manager.

pub mod profile;
pub mod template;
pub mod visit;

#[cfg(test)]
mod tests {
	use std::sync::Once;

	static SETUP_GATE: Once = Once::new();

	pub fn setup_test_env() {
		SETUP_GATE.call_once(|| {
			env_logger::Builder::from_env(
				env_logger::Env::default().default_filter_or(log::Level::Debug.as_str()),
			)
			.is_test(true)
			.try_init()
			.unwrap();

			color_eyre::install().unwrap();
		})
	}
}
