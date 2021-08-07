#![cfg(test)]

use color_eyre::eyre::{eyre, Result};
use pretty_assertions::assert_eq;

use super::*;
use crate::template::block::{Block, BlockKind, If, IfExpr, IfOp, Var, VarEnv, VarEnvSet};
use crate::template::session::Session;
use crate::template::source::Source;
use crate::template::span::ByteSpan;

#[test]
fn parse_single_text() -> Result<()> {
	let content = r#"Hello World this is a text block"#;

	let source = Source::anonymous(content);
	let mut parser = Parser::new(Session::new(source));
	let block = parser
		.next_top_level_block()
		.expect("Found no block")
		.expect("Encountered a parse error");

	assert_eq!(
		block,
		Block::new(ByteSpan::new(0usize, content.len()), BlockKind::Text)
	);

	Ok(())
}

#[test]
fn parse_single_comment() -> Result<()> {
	let content = r#"{{!-- Hello World this is a comment block --}}"#;

	let source = Source::anonymous(content);
	let mut parser = Parser::new(Session::new(source));
	let block = parser
		.next_top_level_block()
		.expect("Found no block")
		.expect("Encountered a parse error");

	assert_eq!(
		block,
		Block::new(ByteSpan::new(0usize, content.len()), BlockKind::Comment)
	);

	Ok(())
}

#[test]
fn parse_single_escaped() -> Result<()> {
	let content = r#"{{{ Hello World this is a comment block }}}"#;

	let source = Source::anonymous(content);
	let mut parser = Parser::new(Session::new(source));
	let block = parser
		.next_top_level_block()
		.expect("Found no block")
		.expect("Encountered a parse error");

	assert_eq!(block.span(), &ByteSpan::new(0usize, content.len()));

	let inner = ByteSpan::new(3usize, content.len() - 3);
	assert_eq!(&content[inner], " Hello World this is a comment block ");
	assert_eq!(block.kind(), &BlockKind::Escaped(inner));

	Ok(())
}

#[test]
fn parse_single_var_default() -> Result<()> {
	let content = r#"{{OS}}"#;

	let source = Source::anonymous(content);
	let mut parser = Parser::new(Session::new(source));
	let block = parser
		.next_top_level_block()
		.expect("Found no block")
		.expect("Encountered a parse error");

	assert_eq!(block.span(), &ByteSpan::new(0usize, content.len()));

	let name = ByteSpan::new(2usize, content.len() - 2);
	assert_eq!(&content[name], "OS");
	let envs = VarEnvSet([Some(VarEnv::Item), Some(VarEnv::Profile), None]);
	assert_eq!(block.kind(), &BlockKind::Var(Var { envs, name }));

	Ok(())
}

#[test]
fn parse_single_var_env() -> Result<()> {
	let content = r#"{{$ENV}}"#;

	let source = Source::anonymous(content);
	let mut parser = Parser::new(Session::new(source));
	let block = parser
		.next_top_level_block()
		.expect("Found no block")
		.expect("Encountered a parse error");

	assert_eq!(block.span(), &ByteSpan::new(0usize, content.len()));

	let name = ByteSpan::new(3usize, content.len() - 2);
	assert_eq!(&content[name], "ENV");
	let envs = VarEnvSet([Some(VarEnv::Environment), None, None]);
	assert_eq!(block.kind(), &BlockKind::Var(Var { envs, name }));

	Ok(())
}

#[test]
fn parse_single_var_profile() -> Result<()> {
	let content = r#"{{#PROFILE}}"#;

	let source = Source::anonymous(content);
	let mut parser = Parser::new(Session::new(source));
	let block = parser
		.next_top_level_block()
		.expect("Found no block")
		.expect("Encountered a parse error");

	assert_eq!(block.span(), &ByteSpan::new(0usize, content.len()));

	let name = ByteSpan::new(3usize, content.len() - 2);
	assert_eq!(&content[name], "PROFILE");
	let envs = VarEnvSet([Some(VarEnv::Profile), None, None]);
	assert_eq!(block.kind(), &BlockKind::Var(Var { envs, name }));

	Ok(())
}

#[test]
fn parse_single_var_item() -> Result<()> {
	let content = r#"{{&ITEM}}"#;

	let source = Source::anonymous(content);
	let mut parser = Parser::new(Session::new(source));
	let block = parser
		.next_top_level_block()
		.expect("Found no block")
		.expect("Encountered a parse error");

	assert_eq!(block.span(), &ByteSpan::new(0usize, content.len()));

	let name = ByteSpan::new(3usize, content.len() - 2);
	assert_eq!(&content[name], "ITEM");
	let envs = VarEnvSet([Some(VarEnv::Item), None, None]);
	assert_eq!(block.kind(), &BlockKind::Var(Var { envs, name }));

	Ok(())
}

#[test]
fn parse_single_var_mixed() -> Result<()> {
	let content = r#"{{$&#MIXED}}"#;

	let source = Source::anonymous(content);
	let mut parser = Parser::new(Session::new(source));
	let block = parser
		.next_top_level_block()
		.expect("Found no block")
		.expect("Encountered a parse error");

	assert_eq!(block.span(), &ByteSpan::new(0usize, content.len()));

	let name = ByteSpan::new(5usize, content.len() - 2);
	assert_eq!(&content[name], "MIXED");
	let envs = VarEnvSet([
		Some(VarEnv::Environment),
		Some(VarEnv::Item),
		Some(VarEnv::Profile),
	]);
	assert_eq!(block.kind(), &BlockKind::Var(Var { envs, name }));

	Ok(())
}

#[test]
fn parse_single_vars() -> Result<()> {
	// duplicate variable environment
	let content = r#"{{##OS}}"#;

	let source = Source::anonymous(content);
	let mut parser = Parser::new(Session::new(source));
	let block = parser
		.next_top_level_block()
		.ok_or(eyre!("No block found"))?;

	assert!(block.is_err());

	Ok(())
}

#[test]
fn parse_single_if_eq() -> Result<()> {
	let content = r#"{{@if {{OS}} == "windows"}}{{@fi}}"#;

	let source = Source::anonymous(content);
	let mut parser = Parser::new(Session::new(source));
	let block = parser
		.next_top_level_block()
		.expect("Found no block")
		.expect("Encountered a parse error");

	assert_eq!(block.span(), &ByteSpan::new(0usize, content.len()));

	let if_span = ByteSpan::new(0usize, 27usize);
	assert_eq!(&content[if_span], r#"{{@if {{OS}} == "windows"}}"#);

	let name = ByteSpan::new(8usize, 10usize);
	assert_eq!(&content[name], "OS");
	let envs = VarEnvSet([Some(VarEnv::Item), Some(VarEnv::Profile), None]);

	let op = IfOp::Eq;

	let other = ByteSpan::new(17usize, 24usize);
	assert_eq!(&content[other], "windows");

	let end_span = ByteSpan::new(27usize, 34usize);
	assert_eq!(&content[end_span], r#"{{@fi}}"#);

	assert_eq!(
		block.kind(),
		&BlockKind::If(If {
			head: (
				if_span.span(IfExpr::Compare {
					var: Var { envs, name },
					op,
					other
				}),
				vec![]
			),
			elifs: vec![],
			els: None,
			end: end_span
		})
	);

	Ok(())
}

#[test]
fn parse_single_if_neq() -> Result<()> {
	let content = r#"{{@if {{OS}} != "windows"}}{{@fi}}"#;

	let source = Source::anonymous(content);
	let mut parser = Parser::new(Session::new(source));
	let block = parser
		.next_top_level_block()
		.expect("Found no block")
		.expect("Encountered a parse error");

	assert_eq!(block.span(), &ByteSpan::new(0usize, content.len()));

	let if_span = ByteSpan::new(0usize, 27usize);
	assert_eq!(&content[if_span], r#"{{@if {{OS}} != "windows"}}"#);

	let name = ByteSpan::new(8usize, 10usize);
	assert_eq!(&content[name], "OS");
	let envs = VarEnvSet([Some(VarEnv::Item), Some(VarEnv::Profile), None]);

	let op = IfOp::NotEq;

	let other = ByteSpan::new(17usize, 24usize);
	assert_eq!(&content[other], "windows");

	let end_span = ByteSpan::new(27usize, 34usize);
	assert_eq!(&content[end_span], r#"{{@fi}}"#);

	assert_eq!(
		block.kind(),
		&BlockKind::If(If {
			head: (
				if_span.span(IfExpr::Compare {
					var: Var { envs, name },
					op,
					other
				}),
				vec![]
			),
			elifs: vec![],
			els: None,
			end: end_span
		})
	);

	Ok(())
}

#[test]
fn parse_single_if_exists() -> Result<()> {
	let content = r#"{{@if {{$#EXISTS}}}}{{@fi}}"#;

	let source = Source::anonymous(content);
	let mut parser = Parser::new(Session::new(source));
	let block = parser
		.next_top_level_block()
		.expect("Found no block")
		.expect("Encountered a parse error");

	assert_eq!(block.span(), &ByteSpan::new(0usize, content.len()));

	let if_span = ByteSpan::new(0usize, 20usize);
	assert_eq!(&content[if_span], r#"{{@if {{$#EXISTS}}}}"#);

	let name = ByteSpan::new(10usize, 16usize);
	assert_eq!(&content[name], "EXISTS");
	let envs = VarEnvSet([Some(VarEnv::Environment), Some(VarEnv::Profile), None]);

	let end_span = ByteSpan::new(20usize, 27usize);
	assert_eq!(&content[end_span], r#"{{@fi}}"#);

	assert_eq!(
		block.kind(),
		&BlockKind::If(If {
			head: (
				if_span.span(IfExpr::Exists {
					var: Var { envs, name }
				}),
				vec![]
			),
			elifs: vec![],
			els: None,
			end: end_span
		})
	);

	Ok(())
}

#[test]
fn find_blocks() {
	let content = r#"{{ Hello World }} {{{ Escaped {{ }} }} }}}
		{{!-- Hello World {{}} {{{ asdf }}} this is a comment --}}
		{{@if {{}} }} }}
		"#;

	println!("{}", content);

	let iter = BlockIter::new(content);

	// Hello World
	// Text: SPACE
	// Escaped
	// Text: LF SPACES
	// Comment
	// Text: LF SPACES
	// If
	// Text: Closing LF SPACES
	assert_eq!(iter.count(), 8);
}

#[test]
fn find_blocks_unicode() {
	let content = "\u{1f600}{{{ \u{1f600} }}}\u{1f600}";

	let iter = BlockIter::new(content);

	// Text: Smiley
	// Escaped
	// Text: Smiley
	assert_eq!(iter.count(), 3);
}

#[test]
fn parse_comment() -> Result<()> {
	let content = r#"{{!-- Hello World this {{}} is a comment {{{{{{ }}}--}}"#;

	let source = Source::anonymous(content);
	let mut parser = Parser::new(Session::new(source));
	let token = parser
		.next_top_level_block()
		.expect("Found no block")
		.expect("Encountered a parse error");

	assert_eq!(
		token,
		Block::new(ByteSpan::new(0usize, content.len()), BlockKind::Comment)
	);

	Ok(())
}

#[test]
fn parse_escaped() -> Result<()> {
	let content = r#"{{{!-- Hello World this {{}} is a comment {{{{{{ }}--}}}"#;

	let source = Source::anonymous(content);
	let mut parser = Parser::new(Session::new(source));
	let token = parser
		.next_top_level_block()
		.expect("Found no block")
		.expect("Encountered a parse error");

	assert_eq!(
		token,
		Block::new(
			ByteSpan::new(0usize, content.len()),
			BlockKind::Escaped(ByteSpan::new(3usize, content.len() - 3))
		)
	);

	Ok(())
}

#[test]
fn parse_if_cmp() -> Result<()> {
	let content = r#"{{@if {{&OS}} == "windows" }}
		DEMO
		{{@elif {{&OS}} == "linux"  }}
		LINUX
		{{@else}}
		ASD
		{{@fi}}"#;

	let source = Source::anonymous(content);
	let mut parser = Parser::new(Session::new(source));
	let token = parser
		.next_top_level_block()
		.expect("Found no block")
		.expect("Encountered a parse error");

	assert_eq!(token.span, ByteSpan::new(0usize, content.len()));
	println!("{:#?}", &token.kind);

	Ok(())
}

#[test]
fn parse_if_cmp_nested() -> Result<()> {
	let content = r#"{{@if {{&OS}} == "windows" }}
		{{!-- This is a nested comment --}}
		{{{ Escaped {{}} }}}
		{{@elif {{&OS}} == "linux"  }}
		{{!-- Below is a nested variable --}}
		{{ OS }}
		{{@else}}
		ASD
		{{@fi}}"#;

	let source = Source::anonymous(content);
	let mut parser = Parser::new(Session::new(source));
	let token = parser
		.next_top_level_block()
		.expect("Found no block")
		.expect("Encountered a parse error");

	assert_eq!(token.span, ByteSpan::new(0usize, content.len()));
	println!("{:#?}", &token.kind);

	Ok(())
}

#[test]
fn parse_if_exists() -> Result<()> {
	let content = r#"{{@if {{&OS}}  }}
		DEMO
		ASD
		{{@fi}}"#;

	let source = Source::anonymous(content);
	let mut parser = Parser::new(Session::new(source));
	let token = parser
		.next_top_level_block()
		.expect("Found no block")
		.expect("Encountered a parse error");

	assert_eq!(token.span, ByteSpan::new(0usize, content.len()));
	println!("{:#?}", &token.kind);

	Ok(())
}

#[test]
fn parse_if_mixed() -> Result<()> {
	let content = r#"{{@if {{OS}}}}
	print("No value for variable `OS` set")
{{@elif {{&OS}} != "windows"}}
	print("OS is not windows")
{{@elif {{OS}} == "windows"}}
	{{{!-- This is a nested comment. Below it is a nested variable block. --}}}
	print("OS is {{OS}}")
{{@else}}
	{{{!-- This is a nested comment. --}}}
	print("Can never get here. {{{ {{OS}} is neither `windows` nor not `windows`. }}}")
{{@fi}}"#;

	let source = Source::anonymous(content);
	let mut parser = Parser::new(Session::new(source));
	let token = parser
		.next_top_level_block()
		.expect("Found no block")
		.expect("Encountered a parse error");

	assert_eq!(token.span, ByteSpan::new(0usize, content.len()));
	println!("{:#?}", &token.kind);

	Ok(())
}

#[test]
fn parse_variables() -> Result<()> {
	assert_eq!(
		parse_var("$#&FOO_BAR", 0)?,
		Var {
			envs: VarEnvSet([
				Some(VarEnv::Environment),
				Some(VarEnv::Profile),
				Some(VarEnv::Item)
			]),
			name: ByteSpan::new(3usize, 10usize),
		}
	);

	assert_eq!(
		parse_var("&BAZ_1", 0)?,
		Var {
			envs: VarEnvSet([Some(VarEnv::Item), None, None]),
			name: ByteSpan::new(1usize, 6usize),
		}
	);

	assert_eq!(
		parse_var("$#&FOO_BAR", 10)?,
		Var {
			envs: VarEnvSet([
				Some(VarEnv::Environment),
				Some(VarEnv::Profile),
				Some(VarEnv::Item)
			]),
			name: ByteSpan::new(13usize, 20usize),
		}
	);

	// invalid env / var_name
	assert!(parse_var("!FOO_BAR", 10).is_err());
	// duplicate env
	assert!(parse_var("&&FOO_BAR", 0).is_err());

	Ok(())
}

#[test]
fn parse_others() -> Result<()> {
	assert_eq!(parse_other("\"BAZ_1\"", 0)?, ByteSpan::new(1usize, 6usize));
	assert_eq!(
		parse_other("This is a test \"Hello World How are you today\"", 0)?,
		ByteSpan::new(16usize, 45usize)
	);

	assert!(parse_other("This is a test \"Hello World How are you today", 0).is_err());
	assert!(parse_other("This is a test", 0).is_err());

	Ok(())
}
