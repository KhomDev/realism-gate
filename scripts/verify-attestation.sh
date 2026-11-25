#!/usr/bin/env bash
# Verify a release archive's build provenance before anything runs it.
#
# Usage: scripts/verify-attestation.sh ARCHIVE
#
# The archive and the SHA256SUMS file are downloaded from the same release, so
# the checksum proves the download is intact: it does not prove who built the
# bytes. `gh attestation verify` checks the Sigstore attestation that the
# release workflow wrote, and `--repo` pins it to this repository. Without the
# attestation an attacker who can replace release assets (or a mirror) can
# replace the checksum too.
#
# RG_VERIFY_ATTESTATION is the Action input. Unset or empty means on; only the
# exact string "false" turns it off. This script fails closed: a missing `gh`
# or a failed verification is an error, never a pass.
#
# Only bash builtins run before the `gh` lookup, so the missing-`gh` case works
# with an empty PATH.
set -euo pipefail

archive="${1:-}"
if [ -z "$archive" ]; then
  echo "usage: $0 ARCHIVE" >&2
  exit 2
fi

case "${RG_VERIFY_ATTESTATION:-true}" in
  true)
    ;;
  false)
    echo "::notice::verify-attestation is false: the release archive was not provenance-checked"
    exit 0
    ;;
  *)
    echo "::error::verify-attestation must be true or false, not '${RG_VERIFY_ATTESTATION}'" >&2
    exit 2
    ;;
esac

repo="${RG_REPO:-KhomDev/realism-gate}"

if ! command -v gh >/dev/null 2>&1; then
  echo "::error::gh is required to verify the release attestation (verify-attestation: true) but is not on PATH. Install gh, or set verify-attestation: false only for a release published without attestations." >&2
  exit 1
fi

if ! gh attestation verify "$archive" --repo "$repo"; then
  echo "::error::the build provenance of $archive did not verify against $repo; refusing to run it" >&2
  exit 1
fi

echo "::notice::verified the build provenance of $(basename "$archive") against $repo"
