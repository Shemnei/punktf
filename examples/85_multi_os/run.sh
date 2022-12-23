#!/usr/bin/env bash

set -eoux pipefail

punktf_binary="${EXAMPLES_BINARY:-punktf}"

"${punktf_binary}" \
	--verbose \
	deploy \
	--source . \
	--profile linux \
	--target "${EXAMPLES_TARGET:-/tmp}" \
	--dry-run
