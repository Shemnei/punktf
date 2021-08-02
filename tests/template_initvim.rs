pub const TEMPLATE: &str = r#"{{!-- This a test for `init.vim` --}}
{{@if {{OS}} == "windows"}}
set fileformat=dos
{{@elif {{OS}} == "macos"}}
set fileformat=mac
{{@else}}
set fileformat=unix
{{@fi}}
set ttyfast
set relativenumber
set number
set encoding={{SYS_ENCODING}}
set colorcolumn=80
set nowrap

{{@if {{OS}} == "windows"}}
set undodir='{{$APPDATA}}\nvim\vimdid'
{{@else}}
set undodir='{{$HOME}}/.config/nvim/vimdid'
{{@fi}}"#;

use color_eyre::Result;
use pretty_assertions::assert_eq;
use punktf::template::Template;
use punktf::variables::UserVars;

#[test]
fn parse_initvim_win() -> Result<()> {
	let template = Template::parse(TEMPLATE)?;

	let vars = UserVars::from_items(vec![("OS", "windows"), ("SYS_ENCODING", "windows1252")]);

	// set temporary env variable
	std::env::set_var("APPDATA", "C:\\Users\\Demo\\Appdata\\Local");

	let output = template.fill(Some(&vars), None)?;

	assert_eq!(
		output.trim(),
		r#"set fileformat=dos

set ttyfast
set relativenumber
set number
set encoding=windows1252
set colorcolumn=80
set nowrap


set undodir='C:\Users\Demo\Appdata\Local\nvim\vimdid'"#
	);

	Ok(())
}

#[test]
fn template_initvim_linux() -> Result<()> {
	let template = Template::parse(TEMPLATE)?;

	let vars = UserVars::from_items(vec![("OS", "linux"), ("SYS_ENCODING", "utf-8")]);

	// set temporary env variable
	std::env::set_var("HOME", "/home/Demo");

	let output = template.fill(Some(&vars), None)?;

	assert_eq!(
		output.trim(),
		r#"set fileformat=unix

set ttyfast
set relativenumber
set number
set encoding=utf-8
set colorcolumn=80
set nowrap


set undodir='/home/Demo/.config/nvim/vimdid'"#
	);

	Ok(())
}