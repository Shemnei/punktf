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

{{@if {{OS}} == "windows"}}
set undodir='{{$APPDATA}}\nvim\vimdid'
{{@else}}
set undodir='{{$HOME}}/.config/nvim/vimdid'
{{@fi}}
set nowrap"#;

use color_eyre::Result;
use pretty_assertions::assert_eq;
use punktf_lib::template::source::Source;
use punktf_lib::template::Template;
use punktf_lib::variables::Variables;

#[test]
fn parse_initvim_win() -> Result<()> {
	let source = Source::anonymous(TEMPLATE);
	let template = Template::parse(source)?;

	let vars = Variables::from_items(vec![("OS", "windows"), ("SYS_ENCODING", "windows1252")]);

	// set temporary env variable
	std::env::set_var("APPDATA", "C:\\Users\\Demo\\Appdata\\Local");

	let output = template.resolve::<_, Variables>(Some(&vars), None)?;

	assert_eq!(
		output.trim(),
		r#"set fileformat=dos
set ttyfast
set relativenumber
set number
set encoding=windows1252
set colorcolumn=80

set undodir='C:\Users\Demo\Appdata\Local\nvim\vimdid'
set nowrap"#
	);

	Ok(())
}

#[test]
fn template_initvim_linux() -> Result<()> {
	let source = Source::anonymous(TEMPLATE);
	let template = Template::parse(source)?;

	let vars = Variables::from_items(vec![("OS", "linux"), ("SYS_ENCODING", "utf-8")]);

	// set temporary env variable
	std::env::set_var("HOME", "/home/Demo");

	let output = template.resolve::<_, Variables>(Some(&vars), None)?;

	assert_eq!(
		output.trim(),
		r#"set fileformat=unix
set ttyfast
set relativenumber
set number
set encoding=utf-8
set colorcolumn=80

set undodir='/home/Demo/.config/nvim/vimdid'
set nowrap"#
	);

	Ok(())
}
