#!/usr/bin/env bash

set -eoux pipefail

punktf_binary="${EXAMPLES_BINARY:-punktf}"

"${punktf_binary}" \
	--source . \
	--verbose \
	deploy \
	--profile linux \
	--target "${EXAMPLES_TARGET:-/tmp}" \
	--dry-run
