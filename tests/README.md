# Créno — QA harness

Headless, offline, deterministic verification for the private relay sync route and the Firefox extension package.

## Run
```bash
bash tools/run-tests.sh          # syntax + E2E + extension
node tests/e2e/relay-sync.mjs    # E2E only (real browser, all external fetches intercepted)
node tests/extension/check.mjs   # extension zip only
```
`CRENO_REPO=/tmp/copy` points the harness at another app copy (used for the self-test). `CRENO_BROWSER` / `PLAYWRIGHT_MODULE` override auto-detection. Playwright comes from the npm npx cache and drives the installed Brave binary — nothing is downloaded, no request leaves the machine.

The suite is fully offline: `page.route` intercepts every external request (Skello API, private relay, the three public relays) and answers it from the harness. Anything else is recorded as an unexpected external request and fails the case. The browser also runs with `--host-resolver-rules=MAP * ~NOTFOUND` as a second safety net. The app is served locally with `python3 -m http.server` on `127.0.0.1`.

## What each case proves
- **A** relay configured + proxy allowed: `direct` fails, private `<relayUrl>/ics?url=…` succeeds, public relays are **not** called, shifts imported, `lastIcsSync` advances.
- **B** no relay: chain is exactly `direct → allorigins → corsproxy → codetabs`; the last one imports.
- **C** no relay + `allowProxy=false`: only `direct`, visible error toast, no import, no other request.
- **D1** relay configured + returns 403 (+ proxy allowed): request list is exactly `[direct, privé]`, no import, visible error; **no** request to `api.allorigins.win`, `corsproxy.io` or `api.codetabs.com`.
- **D2** same as D1 with `allowProxy=false`: same chain `[direct, privé]`, same privacy guarantee.
- **E** relay returns 200 HTML without `BEGIN:VCALENDAR`: rejected, no shift imported, error shown.
- **F** localStorage schema after import: baseline `state`/`settings` keys are all present, `relayUrl` is the only addition, nothing renamed or removed.
- **G** relay configured but invalid (`http://relais.local`, non-loopback): only the direct attempt, error toast, public relays **not** called, and the invalid relay is never contacted.
- **H** loopback exception: `http://127.0.0.1:8080` is accepted as a relay base — chain `[direct, privé]`, ICS imported, no public relay.
- **I** sync budget: a private relay that never answers is abandoned on the app's own budget (asserted between 8 s and 25 s, ~20 s expected), error shown, no public relay. Playwright fake timers advance virtual time, so the case costs tens of milliseconds in real time.

## Notes
- Cases A, D1, D2, E and I are relay-dependent: if the app under test has no private relay route at all, the harness reports `relay route not present (agent/app-relay not merged)` instead of a crash, so the suite still fails gracefully with an actionable message.
- No case is skipped: every case either passes or fails with a line naming the expected behaviour.
