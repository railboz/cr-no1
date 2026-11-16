#!/usr/bin/env bash
# Créno — single test entry point (offline, deterministic).
#
#   bash tools/run-tests.sh
#
# Suites: syntax (node --check), app relay E2E (real browser + route interception),
#         UI contract E2E (18 assertions: week nav, salary/stats, Skello panel,
#         persistence, template invariants — real browser, offline),
#         extension package (builds the zip itself; SKIP only if tools/build-extension.sh
#         is absent).
# Env: CRENO_REPO, CRENO_BROWSER, PLAYWRIGHT_MODULE (forwarded to the E2E harness).
# Exit: 0 every executed suite passed, 1 at least one failed. A SKIP is not a failure,
#       but it is counted and printed separately: it is not a pass.

set -uo pipefail

ROOT=$(cd "$(dirname "$0")/.." && pwd)
cd "$ROOT" || exit 2

PASSED=0
FAILED=0
SKIPPED=0
TOTAL=0

record() { # record <PASS|FAIL|SKIP> <label> <detail>
  TOTAL=$((TOTAL + 1))
  case "$1" in
    PASS) PASSED=$((PASSED + 1)); printf 'PASS %s%s\n' "$2" "${3:+ — $3}" ;;
    SKIP) SKIPPED=$((SKIPPED + 1)); printf 'SKIP %s%s\n' "$2" "${3:+ — $3}" ;;
    *)    FAILED=$((FAILED + 1)); printf 'FAIL %s%s\n' "$2" "${3:+ — $3}" ;;
  esac
}

# ---------------------------------------------------------------- syntax
SYNTAX_OK=1
for f in app.js extension/background.js tests/e2e/relay-sync.mjs tests/e2e/ui-contract.mjs tests/extension/check.mjs; do
  if ! node --check "$f"; then
    echo "  syntax error: $f"
    SYNTAX_OK=0
  fi
done
if ! bash -n tools/run-tests.sh; then
  echo "  syntax error: tools/run-tests.sh"
  SYNTAX_OK=0
fi
if [ "$SYNTAX_OK" -eq 1 ]; then
  record PASS "syntax (node --check app.js + background.js + 3 harness files, bash -n run-tests.sh)"
else
  record FAIL "syntax"
fi

# ---------------------------------------------------------------- E2E
echo "--- app relay E2E ---"
if node tests/e2e/relay-sync.mjs; then
  record PASS "e2e relay sync"
else
  rc=$?
  if [ "$rc" -eq 2 ]; then
    record FAIL "e2e relay sync (harness/setup error, see above)"
  else
    record FAIL "e2e relay sync (see failed cases above)"
  fi
fi

# ---------------------------------------------------------------- UI contract
echo "--- UI contract E2E ---"
if node tests/e2e/ui-contract.mjs; then
  record PASS "e2e ui contract"
else
  rc=$?
  if [ "$rc" -eq 2 ]; then
    record FAIL "e2e ui contract (harness/setup error, see above)"
  else
    record FAIL "e2e ui contract (see failed assertions above)"
  fi
fi

# ---------------------------------------------------------------- extension
echo "--- extension package ---"
EXT_OUT=$(node tests/extension/check.mjs 2>&1)
EXT_RC=$?
printf '%s\n' "$EXT_OUT"
if [ "$EXT_RC" -eq 0 ]; then
  # check.mjs exits 0 both for PASS and for its explicit SKIP line
  # (SKIP = tools/build-extension.sh absent, not a pass).
  if printf '%s\n' "$EXT_OUT" | grep -q '^SKIP '; then
    record SKIP "extension package (build script absent)"
  else
    record PASS "extension package"
  fi
else
  record FAIL "extension package"
fi

echo "----------------------------------------------------------------"
printf '%d passed, %d failed, %d skipped (of %d suites)\n' "$PASSED" "$FAILED" "$SKIPPED" "$TOTAL"

[ "$FAILED" -eq 0 ] || exit 1
exit 0
