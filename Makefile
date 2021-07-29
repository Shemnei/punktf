.PHONY: default buildd buildr build check test clippy checkfmt lint run clean
.PHONY: install cic todos

# Is set to the directory which contains the Makefile regardless from where
# the make command is called.
ROOT_DIR := $(dir $(abspath $(firstword $(MAKEFILE_LIST))))

default: check

buildd:
	cargo build

buildr:
	cargo build --release

build: buildr

check:
	cargo check --all

test:
	cargo test --all

clippy:
	cargo clippy --all -- -Dwarnings

checkfmt:
	cargo fmt --all -- --check

lint: checkfmt clippy

run:
	cargo run

clean:
	cargo clean

install:
	cargo install --path $(ROOT_DIR)

# utility
# can i commit
cic: test lint

# searches for things which need to be improved
todos:
	rg "(TODO|print|unwrap\()"
