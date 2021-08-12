use std::fmt;

use super::span::{ByteSpan, Spanned};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BlockHint {
	Text,
	Comment,
	Escaped,
	Variable,
	IfStart,
	ElIf,
	Else,
	IfEnd,
}

impl BlockHint {
	pub fn is_if_subblock(&self) -> bool {
		self == &Self::ElIf || self == &Self::Else || self == &Self::IfEnd
	}
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BlockKind {
	Text,
	Comment,
	Escaped(ByteSpan),
	Var(Var),
	If(If),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Block {
	pub span: ByteSpan,
	pub kind: BlockKind,
}

impl Block {
	pub fn new(span: ByteSpan, kind: BlockKind) -> Self {
		Self { span, kind }
	}

	pub fn span(&self) -> &ByteSpan {
		&self.span
	}

	pub fn kind(&self) -> &BlockKind {
		&self.kind
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VarEnv {
	Environment,
	Profile,
	Item,
}

impl fmt::Display for VarEnv {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		fmt::Debug::fmt(&self, f)
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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
		Self([Some(VarEnv::Item), Some(VarEnv::Profile), None])
	}
}

impl fmt::Display for VarEnvSet {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_list().entries(self.envs()).finish()
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Var {
	pub envs: VarEnvSet,
	pub name: ByteSpan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct If {
	pub head: (Spanned<IfExpr>, Vec<Block>),
	pub elifs: Vec<(Spanned<IfExpr>, Vec<Block>)>,
	pub els: Option<(ByteSpan, Vec<Block>)>,
	pub end: ByteSpan,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IfExpr {
	Compare { var: Var, op: IfOp, other: ByteSpan },
	Exists { var: Var },
}
