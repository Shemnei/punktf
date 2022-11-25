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

# The `render` command can be used to print the resolved contents of a template
# to stdout.
"${punktf_binary}" \
	--source . \
	render \
	--profile simple \
	"hello.template"
