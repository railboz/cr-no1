# Créno — QA harness

Headless, offline, deterministic verification for the private relay sync route and the Firefox extension package.

## Run
```bash
bash tools/run-tests.sh          # syntax + E2E + extension
node tests/e2e/relay-sync.mjs    # E2E only (real browser, all external fetches intercepted)
node tests/extension/check.mjs   # extension zip only (SKIP until tools/build-extension.sh exists)
```
`CRENO_REPO=/tmp/copy` points the harness at another app copy (used for the self-test). `CRENO_BROWSER` / `PLAYWRIGHT_MODULE` override auto-detection. Playwright comes from the npm npx cache and drives the installed Brave binary — nothing is downloaded, no request leaves the machine.
## What each case proves
- **A** relay configured + proxy allowed: `direct` fails, private `<relayUrl>/ics?url=…` succeeds, public relays are **not** called, shifts imported, `lastIcsSync` advances.
- **B** no relay: chain is exactly `direct → allorigins → corsproxy → codetabs`; the last one imports.
- **C** no relay + `allowProxy=false`: only `direct`, visible error toast, no import, no other request.
- **D1/D2** private relay 403: falls through to public relays when allowed; clean error and no public call otherwise.
- **E** private relay 200 HTML without `BEGIN:VCALENDAR`: rejected, no shift imported, error shown.
- **F** localStorage schema after import: baseline `state`/`settings` keys are all present, `relayUrl` is the only addition, nothing renamed or removed.
## Known limitation
The harness runs against the checked-out worktree. Until `agent/app-relay` is merged, relay-dependent cases (A, D1, D2, E) fail with `relay route not present (agent/app-relay not merged)`; the extension suite prints `SKIP` until `tools/build-extension.sh` lands. Cases B, C and F already run against today's app and guard the existing behaviour.
