#!/usr/bin/env bash
# Local test for scripts/verify-attestation.sh and the Action input that
# selects it. No network, no real `gh`: a fake `gh` on PATH records the call and
# returns a chosen exit code.
#
# Cases: default-on, explicit on, opt-out, a failed verification, a missing
# `gh`, and an invalid input value. The last block checks action.yml itself:
# the input exists, its default is true, the install step wires it, and the
# verification call comes before the archive is extracted.
set -euo pipefail

repo="$(cd "$(dirname "$0")/.." && pwd)"
script="$repo/scripts/verify-attestation.sh"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
# Resolve bash before a case runs with an empty PATH: an assignment prefix
# changes PATH for the command lookup too, so `PATH=x bash ...` would fail.
bash_bin="$(command -v bash)"
mkdir -p "$work/bin" "$work/empty" "$work/archive"
archive="$work/archive/realism-gate-1.0.4-x86_64-unknown-linux-musl.tar.gz"
printf 'not really an archive\n' > "$archive"
log="$work/gh.log"

# A fake `gh`. It records every argument list, then exits with FAKE_GH_EXIT.
cat > "$work/bin/gh" <<'EOF'
#!/usr/bin/env bash
printf '%s\n' "$*" >> "$FAKE_GH_LOG"
exit "${FAKE_GH_EXIT:-0}"
EOF

fail=0
ok() { printf 'ok    %s\n' "$1"; }
bad() { printf 'FAIL  %s\n' "$1"; fail=1; }

# run_script MODE: runs the verifier with PATH containing only the fake gh for
# MODEs that need it, or a directory with no gh at all for the missing case.
run_script() {
  case "$1" in
    with-gh)
      PATH="$work/bin:/usr/bin:/bin" \
        FAKE_GH_LOG="$log" FAKE_GH_EXIT="${FAKE_GH_EXIT:-0}" \
        "$bash_bin" "$script" "$archive"
      ;;
    without-gh)
      PATH="$work/empty" "$bash_bin" "$script" "$archive"
      ;;
  esac
}

# The default case runs with the variable genuinely absent, not empty.
run_default() {
  PATH="$work/bin:/usr/bin:/bin" FAKE_GH_LOG="$log" \
    env -u RG_VERIFY_ATTESTATION "$bash_bin" "$script" "$archive"
}

# 1. Default-on: no input at all, a working gh, must verify.
: > "$log"
if run_default >/dev/null 2>&1 && grep -q 'attestation verify' "$log"; then
  ok "default-on verifies when verify-attestation is unset"
else
  bad "default-on verifies when verify-attestation is unset"
fi

# 2. Explicit true, success, and the exact arguments gh receives.
: > "$log"
if RG_VERIFY_ATTESTATION=true run_script with-gh >/dev/null 2>&1 \
   && grep -q -- "--repo KhomDev/realism-gate" "$log" \
   && grep -q -- "$archive" "$log"; then
  ok "verify-attestation: true passes and pins the repository"
else
  bad "verify-attestation: true passes and pins the repository"
fi

# 3. A non-zero `gh attestation verify` must fail the step.
: > "$log"
if FAKE_GH_EXIT=1 RG_VERIFY_ATTESTATION=true run_script with-gh >/dev/null 2>&1; then
  bad "a failed verification fails the step"
else
  ok "a failed verification fails the step"
fi

# 4. Missing gh must fail closed, with an error that names the input.
out="$(RG_VERIFY_ATTESTATION=true run_script without-gh 2>&1)" && status=0 || status=$?
if [ "$status" -ne 0 ] && printf '%s' "$out" | grep -q 'gh is required'; then
  ok "a missing gh fails closed"
else
  bad "a missing gh fails closed (status $status)"
fi

# 5. Opt-out: only the exact "false" skips verification, and it needs no gh.
if RG_VERIFY_ATTESTATION=false run_script without-gh >/dev/null 2>&1; then
  ok "verify-attestation: false skips without gh"
else
  bad "verify-attestation: false skips without gh"
fi

# 6. Any other value is an input error, not a silent skip.
if RG_VERIFY_ATTESTATION=yes run_script without-gh >/dev/null 2>&1; then
  bad "an invalid verify-attestation value fails"
else
  ok "an invalid verify-attestation value fails"
fi

# 7. action.yml: the input exists with default true, the install step wires
#    it, and the verification call precedes extraction of the archive.
action="$repo/action.yml"
if grep -A4 '^  verify-attestation:$' "$action" | grep -q 'default: "true"'; then
  ok "action.yml declares verify-attestation default true"
else
  bad "action.yml declares verify-attestation default true"
fi
if grep -q 'RG_VERIFY_ATTESTATION: \${{ inputs.verify-attestation }}' "$action"; then
  ok "action.yml passes the input to the install step"
else
  bad "action.yml passes the input to the install step"
fi
if awk '
  /verify-attestation\.sh/ { verify = NR }
  /tar xzf|unzip -q/ { extract = NR }
  END { exit !(verify && extract && verify < extract) }
' "$action"; then
  ok "action.yml verifies the archive before extracting it"
else
  bad "action.yml verifies the archive before extracting it"
fi

if [ "$fail" -ne 0 ]; then
  echo "attestation test: FAILED"
  exit 1
fi
echo "attestation test: all cases passed"
