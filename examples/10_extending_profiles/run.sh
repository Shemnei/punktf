#!/usr/bin/env bash

set -eoux pipefail

punktf_binary="${EXAMPLES_BINARY:-punktf}"

"${punktf_binary}" \
	--verbose \
	deploy \
	--source . \
	--profile base \
	--target "${EXAMPLES_TARGET:-/tmp}" \
	--dry-run

"${punktf_binary}" \
	--verbose \
	deploy \
	--source . \
	--profile extending \
	--target "${EXAMPLES_TARGET:-/tmp}" \
	--dry-run
