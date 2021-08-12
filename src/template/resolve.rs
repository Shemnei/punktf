use color_eyre::eyre::Result;

use super::block::{Block, BlockKind, If, IfExpr, Var, VarEnv};
use super::session::Session;
use super::Template;
use crate::template::diagnostic::{Diagnositic, DiagnositicBuilder, DiagnositicLevel};
use crate::variables::{UserVars, Variables};

pub struct Resolver<'a> {
	template: &'a Template<'a>,
	profile_vars: Option<&'a UserVars>,
	dotfile_vars: Option<&'a UserVars>,
	session: Session,
	output: String,
}

impl<'a> Resolver<'a> {
	pub const fn new(
		template: &'a Template<'a>,
		profile_vars: Option<&'a UserVars>,
		dotfile_vars: Option<&'a UserVars>,
	) -> Self {
		Self {
			template,
			profile_vars,
			dotfile_vars,
			session: Session::new(),
			output: String::new(),
		}
	}

	pub fn resolve(mut self) -> Result<String> {
		for block in &self.template.blocks {
			if let Err(builder) = self.process_block(block) {
				self.report_diagnostic(builder.build());
			}
		}

		self.session.emit(&self.template.source);

		let Resolver {
			session, output, ..
		} = self;

		session.try_finish().map(|_| output)
	}

	fn report_diagnostic(&mut self, diagnostic: Diagnositic) {
		if diagnostic.level() == &DiagnositicLevel::Error {
			self.session.mark_failed();
		}

		self.session.report(diagnostic);
	}

	fn process_block(&mut self, block: &Block) -> Result<(), DiagnositicBuilder> {
		let Block { span, kind } = block;

		match kind {
			BlockKind::Text => {
				self.output.push_str(&self.template.source[span]);
			}
			BlockKind::Comment => {
				// NOP
			}
			BlockKind::Escaped(inner) => {
				self.output.push_str(&self.template.source[inner]);
			}
			BlockKind::Var(var) => {
				self.output.push_str(&self.resolve_var(var)?);
			}
			BlockKind::Print(inner) => {
				log::info!("[Print] {}", &self.template.source[inner]);
			}
			BlockKind::If(If {
				head,
				elifs,
				els,
				end: _,
			}) => {
				// TODO: if if block starts on a new line trim it
				// TODO: if if block ends on a new line trim it
				let (head, head_nested) = head;

				let matched = match self.resolve_if_expr(head.value()) {
					Ok(x) => x,
					Err(builder) => {
						return Err(
							builder.label_span(*head.span(), "while resolving this `if` block")
						)
					}
				};

				if matched {
					for block in head_nested {
						let _ = self.process_block(block)?;
					}
				} else {
					for (elif, elif_nested) in elifs {
						let matched = match self.resolve_if_expr(elif.value()) {
							Ok(x) => x,
							Err(builder) => {
								return Err(builder
									.label_span(*elif.span(), "while resolving this `elif` block"))
							}
						};

						if matched {
							// return if matching elif arm was found
							for block in elif_nested {
								let _ = self.process_block(block)?;
							}

							return Ok(());
						}
					}

					if let Some((_, els_nested)) = els {
						for block in els_nested {
							let _ = self.process_block(block)?;
						}
					}
				}
			}
		};

		Ok(())
	}

	fn resolve_if_expr(&self, expr: &IfExpr) -> Result<bool, DiagnositicBuilder> {
		match expr {
			IfExpr::Compare { var, op, other } => {
				let var = self.resolve_var(var)?;
				Ok(op.eval(&var, &self.template.source[other]))
			}
			IfExpr::Exists { var } => Ok(self.resolve_var(var).is_ok()),
		}
	}

	fn resolve_var(&self, var: &Var) -> Result<String, DiagnositicBuilder> {
		let name = &self.template.source[var.name];

		for env in var.envs.envs() {
			match env {
				VarEnv::Environment => {
					if let Ok(val) = std::env::var(name) {
						return Ok(val);
					}
				}
				VarEnv::Profile => {
					if let Some(Some(val)) = self.profile_vars.map(|vars| vars.var(name)) {
						return Ok(val.to_string());
					}
				}
				VarEnv::Dotfile => {
					if let Some(Some(val)) = self.dotfile_vars.map(|vars| vars.var(name)) {
						return Ok(val.to_string());
					}
				}
			};
		}

		Err(DiagnositicBuilder::new(DiagnositicLevel::Error)
			.message("failed to resolve variable")
			.description(format!(
				"no variable `{}` found in environments {}",
				name, var.envs
			))
			.primary_span(var.name))
	}
}
