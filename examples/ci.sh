#!/usr/bin/env bash

set -eoux pipefail

# Iterate over all child directories of this directory
for path in *; do
    [[ -d "${path}" ]] || continue # if not a directory, skip
    dirname="$(basename "${path}")"

	pushd "${dirname}"
	bash -cu "./run.sh"
	popd
done
