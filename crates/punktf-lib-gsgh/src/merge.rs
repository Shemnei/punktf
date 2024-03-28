use serde::{Deserialize, Serialize};

use crate::hook::Hook;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "with", rename_all = "snake_case")]
pub enum MergeMode {
	Hook(Hook),
}
