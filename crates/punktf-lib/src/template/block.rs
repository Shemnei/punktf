//! Basic block and tokens a [template](`super::Template`) is created from.

use std::fmt;

use super::span::{ByteSpan, Spanned};

/// A parsed instruction from a template.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BlockHint {
	/// Starts a `Text` block
	Text,
	/// Starts a `Comment` block
	Comment,
	/// Starts an escaped block
	Escaped,
	/// Starts a `Variable` block
	Var,
	/// Starts a `Print` block
	Print,
	/// Starts a `If` block
	IfStart,
	/// Continues an `If` block with an `ElIf` block
	ElIf,
	/// Continues an `If` block with an `Else` block
	Else,
	/// End an `If` block
	IfEnd,
}

impl BlockHint {
	/// Whether this instruction is part of a block.
	pub fn is_if_subblock(&self) -> bool {
		self == &Self::ElIf || self == &Self::Else || self == &Self::IfEnd
	}
}

/// A instruction that opens a new block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BlockKind {
	/// A `Text` block, that contains text that is copied to the output.
	Text,
	/// A `Comment` block, that contains text that is ignored.
	Comment,
	/// An escaped block, that contains escaped text that is copied to the output.
	Escaped(ByteSpan),
	/// A `Variable` block, that contains a variable name that is replaced with its value.
	Var(Var),
	/// A `Print` block, that contains text that is printed to the log.
	Print(ByteSpan),
	/// An `If` block, that contains a condition that is evaluated and compiles the block conditionally.
	If(If),
}

impl BlockKind {
	/// Returns the corresponding hint for this block.
	pub const fn as_hint(&self) -> BlockHint {
		match self {
			BlockKind::Text => BlockHint::Text,
			BlockKind::Comment => BlockHint::Comment,
			BlockKind::Escaped(_) => BlockHint::Escaped,
			BlockKind::Var(_) => BlockHint::Var,
			BlockKind::Print(_) => BlockHint::Print,
			BlockKind::If(_) => BlockHint::IfEnd,
		}
	}
}

/// A block can be a single construction or open up a multi-line block that contains sub-blocks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Block {
	/// The span of this block.
	pub span: ByteSpan,
	/// The type of this block.
	pub kind: BlockKind,
}

impl Block {
	/// Creates a new block.
	pub const fn new(span: ByteSpan, kind: BlockKind) -> Self {
		Self { span, kind }
	}

	/// Returns the span of the block.
	pub const fn span(&self) -> &ByteSpan {
		&self.span
	}

	/// Returns the type of this block.
	pub const fn kind(&self) -> &BlockKind {
		&self.kind
	}
}

/// The different types of sources for variables values
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VarEnv {
	/// A variable that is defined by the system's environment.
	Environment,
	/// A variable that is defined in the profile.
	Profile,
	/// A variable that is defined for a specific dotfile.
	Dotfile,
}

impl fmt::Display for VarEnv {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		fmt::Debug::fmt(&self, f)
	}
}

/// Defines a set of variables sources that can be used to resolve variables.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct VarEnvSet(pub [Option<VarEnv>; 3]);

impl VarEnvSet {
	/// Creates an empty `VarEnvSet`.
	pub const fn empty() -> Self {
		Self([None; 3])
	}

	/// Adds a new variable to the set if it is not already present.
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

	/// Returns the set of `VarEnv`s that are defined.
	pub fn envs(&self) -> impl Iterator<Item = &VarEnv> {
		self.0.iter().filter_map(|x| x.as_ref())
	}

	/// Returns the number of environments that are defined.
	pub fn len(&self) -> usize {
		self.envs().count()
	}

	/// Returns the capacity of the set.
	pub const fn capacity(&self) -> usize {
		self.0.len()
	}
}

impl Default for VarEnvSet {
	fn default() -> Self {
		Self([Some(VarEnv::Dotfile), Some(VarEnv::Profile), None])
	}
}

impl fmt::Display for VarEnvSet {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_list().entries(self.envs()).finish()
	}
}

/// A variable that is defined in the template.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Var {
	/// The `VarEnvSet` for the variable
	pub envs: VarEnvSet,
	/// The `ByteSpan` of the variable
	pub name: ByteSpan,
}

/// Defines an if block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct If {
	/// The head of an if statement.
	///
	/// `{{@if {{VAR}}}}`
	pub head: (Spanned<IfExpr>, Vec<Block>),

	/// All elif statements of the if.
	///
	/// `{{@elif {{VAR}}}}`
	pub elifs: Vec<(Spanned<IfExpr>, Vec<Block>)>,

	/// The else statement of the if.
	///
	/// `{{@else}}`
	pub els: Option<(ByteSpan, Vec<Block>)>,

	/// The closing fi statement.
	///
	/// `{{@fi}}`
	pub end: ByteSpan,
}

/// The different types of if expression operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IfOp {
	/// Operand to check for equality.
	Eq,

	/// Operand to check for inequality.
	NotEq,
}

impl IfOp {
	/// Evaluates an if expression.
	pub fn eval(&self, lhs: &str, rhs: &str) -> bool {
		match self {
			Self::Eq => lhs == rhs,
			Self::NotEq => lhs != rhs,
		}
	}
}

/// The different if expression types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IfExpr {
	/// An if expression that compares two values.
	Compare {
		/// Left hand side of the compare operation.
		var: Var,

		/// Compare operand.
		op: IfOp,

		/// Right hand side of the compare operation.
		other: ByteSpan,
	},

	/// An if expression that checks if a value is defined.
	Exists {
		/// Variable to check existence for.
		var: Var,
	},
}
