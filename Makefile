.PHONY: default buildd buildr build check test clippy checkfmt lint clean
.PHONY: install cic

# Is set to the directory which contains the Makefile regardless from where
# the make command is called.
ROOT_DIR := $(dir $(abspath $(firstword $(MAKEFILE_LIST))))

default: check

buildd:
	cargo +nightly build --debug

buildr:
	cargo +nightly build --release

build: buildr

check:
	cargo +nightly check --all

test:
	cargo +nightly test --all

clippy:
	cargo +nightly clippy --all -- -Dwarnings

checkfmt:
	cargo +nightly fmt --all -- --check

lint: checkfmt clippy

clean:
	cargo +nightly clean

install:
	cargo +nightly install --path $(ROOT_DIR)

# utility
cic: test lint
