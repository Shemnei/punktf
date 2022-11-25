#!/usr/bin/env bash

set -eoux pipefail

punktf_binary="${EXAMPLES_BINARY:-punktf}"

"${punktf_binary}" \
	--source . \
	--verbose \
	deploy \
	--profile simple \
	--target "${EXAMPLES_TARGET:-/tmp}" \
	--dry-run

"${punktf_binary}" \
	--source . \
	render \
	--profile simple \
	"hello.template"
