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

"${punktf_binary}" \
	--verbose \
	diff \
	--source . \
	--profile simple
