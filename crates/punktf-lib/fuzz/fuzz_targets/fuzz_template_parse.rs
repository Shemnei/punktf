#![no_main]
use libfuzzer_sys::fuzz_target;
use punktf_lib::template::{
	source::{Source, SourceOrigin},
	Template,
};

fuzz_target!(|data: &[u8]| {
	if let Ok(s) = std::str::from_utf8(data) {
		let source = Source::new(SourceOrigin::Anonymous, s);
		let _ = Template::parse(source);
	}
});
