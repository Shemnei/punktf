use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Priority(pub u32);

impl PartialOrd for Priority {
	fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
		// Reverse sort ordering (smaller = higher)
		other.0.partial_cmp(&self.0)
	}
}

impl Ord for Priority {
	fn cmp(&self, other: &Self) -> std::cmp::Ordering {
		// Reverse sort ordering (smaller = higher)
		other.0.cmp(&self.0)
	}
}
