/// Tests for cases discovered by fuzzing this crate.
/// The only checks done are, that no panic occurs.

const TEMPLATES: &[&[u8]] = &[
	// fuzz/artifacts/fuzz_template_parse/minimized-from-99658ac1fce12b1bd80cfc1d5219cf49284b473a
	&[9, 123, 123, 10, 125, 125, 26]
];

use punktf_lib::template::source::Source;
use punktf_lib::template::Template;

#[test]
fn parse_fuzzed_templates() {
	for template in TEMPLATES {
		let s = unsafe { std::str::from_utf8_unchecked(template) };
		let source = Source::anonymous(s);
		let _ = Template::parse(source);
	}
}
