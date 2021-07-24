.PHONY: run check test lint cic clean

run:
	cargo run

check:
	cargo check

test:
	cargo test

lint:
	cargo fmt --all -- --check
	cargo clippy -- -D warnings

# can i commit?
cic: check test lint

clean:
	cargo clean
