use super::span::{ByteSpan, Spanned};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BlockHint {
	Text,
	Comment,
	Escaped,
	Variable,
	Print,
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
	Print(ByteSpan),
	If(If),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Block {
	pub span: ByteSpan,
	pub kind: BlockKind,
}

impl Block {
	pub const fn new(span: ByteSpan, kind: BlockKind) -> Self {
		Self { span, kind }
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VarEnv {
	Environment,
	Profile,
	Item,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct VarEnvSet(pub [Option<VarEnv>; 3]);

impl VarEnvSet {
	pub const fn empty() -> Self {
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

	pub const fn capacity(&self) -> usize {
		self.0.len()
	}
}

impl Default for VarEnvSet {
	fn default() -> Self {
		Self([Some(VarEnv::Item), Some(VarEnv::Profile), None])
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
pub struct IfExpr {
	pub var: Var,
	pub op: IfOp,
	pub other: ByteSpan,
}
