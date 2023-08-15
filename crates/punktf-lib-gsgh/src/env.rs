use std::{
	collections::{btree_set, BTreeMap, BTreeSet, HashSet},
	ops::Deref,
};

use serde::{Deserialize, Serialize};

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Environment(pub BTreeMap<String, serde_yaml::Value>);

impl Environment {
	pub fn is_empty(&self) -> bool {
		self.0.is_empty()
	}
}

#[derive(Default, Debug, Clone, PartialEq, Eq)]
pub struct LayeredEnvironment(Vec<(&'static str, Environment)>);

impl LayeredEnvironment {
	pub fn push(&mut self, name: &'static str, env: Environment) {
		self.0.push((name, env));
	}

	pub fn pop(&mut self) -> Option<(&'static str, Environment)> {
		self.0.pop()
	}

	pub fn keys(&self) -> BTreeSet<&str> {
		self.0
			.iter()
			.flat_map(|(_, layer)| layer.0.keys())
			.map(|key| key.as_str())
			.collect()
	}

	pub fn get(&self, key: &str) -> Option<&serde_yaml::Value> {
		for (_, layer) in self.0.iter() {
			if let Some(value) = layer.0.get(key) {
				return Some(value);
			}
		}

		return None;
	}

	pub fn iter(&self) -> LayeredIter<'_> {
		LayeredIter::new(self)
	}

	pub fn as_str_map(&self) -> BTreeMap<&str, String> {
		self.iter()
			// TODO: Optimize
			// `trim` to remove trailing `\n`
			.map(|(k, v)| (k, serde_yaml::to_string(v).unwrap().trim().into()))
			.collect()
	}
}

pub struct LayeredIter<'a> {
	env: &'a LayeredEnvironment,
	keys: btree_set::IntoIter<&'a str>,
}

impl<'a> LayeredIter<'a> {
	pub fn new(env: &'a LayeredEnvironment) -> Self {
		let keys = env.keys().into_iter();
		Self { env, keys }
	}
}

impl<'a> Iterator for LayeredIter<'a> {
	type Item = (&'a str, &'a serde_yaml::Value);

	fn next(&mut self) -> Option<Self::Item> {
		let key = self.keys.next()?;
		Some((key, self.env.get(key)?))
	}
}
