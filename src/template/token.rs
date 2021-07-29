use super::span::{CharSpan, Spanned};

#[derive(Debug, Clone, PartialEq)]
pub enum Token {
	Var(Var),
	If(If),
	Escaped(CharSpan),
	Comment,
}

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Hash)]
pub enum VarEnv {
	Environment,
	Profile,
	Item,
}

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Hash)]
pub struct VarEnvSet(pub [Option<VarEnv>; 3]);

impl VarEnvSet {
	pub fn empty() -> Self {
		Self([None; 3])
	}

	pub fn add(&mut self, value: VarEnv) -> bool {
		if self.0.contains(&Some(value)) {
			false
		} else if let Some(slot) = self.0.iter_mut().find(|x| x.is_none()) {
			*slot = Some(value);
			true
		} else {
			false
		}
	}

	pub fn envs(&self) -> impl Iterator<Item = &VarEnv> {
		self.0.iter().filter_map(|x| x.as_ref())
	}

	pub fn len(&self) -> usize {
		self.envs().count()
	}

	pub fn capacity(&self) -> usize {
		self.0.len()
	}
}

impl Default for VarEnvSet {
	fn default() -> Self {
		Self([
			Some(VarEnv::Item),
			Some(VarEnv::Profile),
			Some(VarEnv::Environment),
		])
	}
}

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Hash)]
pub struct Var {
	pub envs: VarEnvSet,
	pub name: CharSpan,
}

#[derive(Debug, Clone, PartialEq)]
pub struct If {
	pub head: Spanned<IfExpr>,
	pub elifs: Vec<Spanned<IfExpr>>,
	pub els: Option<CharSpan>,
	pub end: CharSpan,
}

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Hash)]
pub enum IfOp {
	Eq,
	NotEq,
}

impl IfOp {
	pub fn eval(&self, lhs: &str, rhs: &str) -> bool {
		match self {
			Self::Eq => lhs == rhs,
			Self::NotEq => lhs != rhs,
		}
	}
}

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Hash)]
pub struct IfExpr {
	pub var: Var,
	pub op: IfOp,
	pub other: CharSpan,
}
