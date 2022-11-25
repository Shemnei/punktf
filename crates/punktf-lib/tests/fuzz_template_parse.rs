/// Tests for cases discovered by fuzzing this crate.
/// The only checks done are, that no panic occurs.

const TEMPLATES: &[&[u8]] = &[&[9, 123, 123, 10, 125, 125, 26]];

use punktf_lib::template::source::Source;
use punktf_lib::template::Template;

#[test]
fn parse_templates() {
	for template in TEMPLATES {
		let s = unsafe { std::str::from_utf8_unchecked(template) };
		let source = Source::anonymous(s);
		let _ = Template::parse(source);
	}
}
