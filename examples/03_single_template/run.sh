#!/usr/bin/env bash

set -eoux pipefail

punktf_binary="${EXAMPLES_BINARY:-punktf}"

"${punktf_binary}" \
	--verbose \
	deploy \
	--source . \
	--profile simple \
	--target "${EXAMPLES_TARGET:-/tmp}" \
	--dry-run

# The `render` command can be used to print the resolved contents of a template
# to stdout.
"${punktf_binary}" \
	render \
	--source . \
	--profile simple \
	"hello.template"
