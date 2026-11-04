#!/usr/bin/env bash
# Prints the name and size in bytes of every asset of a published release, as
# JSON. Usage: scripts/release-assets.sh TAG
set -euo pipefail
tag="${1:?tag, e.g. v0.1.0}"
gh release view "$tag" --repo KhomDev/realism-gate --json tagName,assets \
    --jq '{tag: .tagName, assets: [.assets[] | {name, size}] | sort_by(.name)}'
