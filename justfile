build-dev:
	cargo build

build-release:
	cargo build --release

build: build-release

check:
	cargo check --all-targets

test:
	cargo test --all-targets

clippy:
	cargo clippy --all-targets -- -Dwarnings

checkfmt:
	cargo fmt --all -- --check

lint: checkfmt clippy

run:
	cargo run

clean:
	cargo clean

install:
	cargo install --path $(ROOT_DIR)

doc:
	cargo doc --all --document-private-items

#############
## Utility ##
#############

# Git - can i commit
cic: test lint doc

# Searches for things which need to be improved
todos:
	rg "(TODO|print(!|ln!)|unwrap\()"

# Compile timings
timings: clean
	cargo build -p punktf --bin punktf --timings --release

# Check for outdated dependencies
#
# REQUIRES: cargo-edit
outdated:
	cargo upgrade --dry-run
