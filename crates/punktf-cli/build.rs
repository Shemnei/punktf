fn main() -> std::io::Result<()> {
	// Bundle `VCRUNTIME140.DLL` on Windows:
	#[cfg(all(windows, feature = "windows-static"))]
	static_vcruntime::metabuild();
	Ok(())
}
