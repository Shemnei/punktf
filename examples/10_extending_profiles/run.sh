#!/usr/bin/env bash

set -eoux pipefail

punktf_binary="${EXAMPLES_BINARY:-punktf}"

"${punktf_binary}" \
	--source . \
	--verbose \
	deploy \
	--profile base \
	--target "${EXAMPLES_TARGET:-/tmp}" \
	--dry-run

"${punktf_binary}" \
	--source . \
	--verbose \
	deploy \
	--profile extending \
	--target "${EXAMPLES_TARGET:-/tmp}" \
	--dry-run
