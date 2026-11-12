# Créno relay — API contract v1

Base URL is user-configured in the app (e.g. `https://relais.example.fr`). Single reference for the PWA, the extension, the docs and the server.

| Request | Success | Headers |
| --- | --- | --- |
| `GET /healthz` | `200` `{"status":"ok"}` (nothing else) | `Content-Type: application/json; charset=utf-8` |
| `GET /ics?url=<percent-encoded absolute https URL>` | `200`, upstream body verbatim | `Content-Type: text/calendar; charset=utf-8`, `ETag: "sha256-<hex>"`, `Cache-Control: private, max-age=300`, `X-Content-Type-Options: nosniff` |
| `GET /ics` + `If-None-Match: <ETag>` or `*` | `304`, no body | `ETag`, `Cache-Control`, `X-Content-Type-Options` |
| `OPTIONS` any path | `204`, no body | CORS headers below |

`If-None-Match` uses the RFC 9110 weak comparison: the `W/` prefix is ignored, the value may be a comma-separated list, surrounding whitespace is trimmed, and the bare `*` matches any current representation (so it always revalidates into a `304`). A quoted `"*"` is an ordinary opaque tag and does not match. Errors are never cached (`Cache-Control: no-store`).

CORS: allowed origins are `null` (file://) **only when `CRENO_ALLOW_FILE_ORIGIN=1`**, `moz-extension://<anything>` only when `CRENO_ALLOW_EXTENSION=1`, `http://localhost:<port>`, `http://127.0.0.1:<port>` and the exact origins of `CRENO_ALLOWED_ORIGINS`. Never `*`. A refused origin is served normally but without `Access-Control-Allow-Origin`; `Vary: Origin` is always sent. Methods `GET, OPTIONS`, allowed request header `If-None-Match`, exposed response header `ETag`. `OPTIONS` preflights are answered before the rate limiter, so a preflight is never charged nor throttled.

`Origin: null` is off by default because it is not "the local file": every sandboxed iframe (or `data:`/`blob:` document) of any site sends it too.

Query parsing: the `url` parameter is read from the raw query string, never through a catch-all extractor.

| input | result |
| --- | --- |
| `?url=https%3A%2F%2F…` | the decoded absolute URL |
| `?url=a&url=b` | the **first** `url` wins, the rest is ignored |
| `?url=+` | `+` is a space (form encoding); the value trims to nothing → `missing_url` |
| `?url=%2B` | `%2B` is a literal `+`, which is not an absolute URL → `invalid_url` |
| `?url=`, `?url=%20%20` | `missing_url` (empty value, or whitespace only) |
| `?URL=…` (other casing) or `?other=…` | `missing_url`: the parameter name is matched exactly |
| no query string | `missing_url` |

Errors: body `{"error":"<code>"}`, `Content-Type: application/json; charset=utf-8`, never upstream text, never the query string. There is no bodiless error response anywhere, including `404`.

| code | status | note |
| --- | --- | --- |
| `missing_url` | 400 | `url` absent or empty |
| `invalid_url` | 400 | not an absolute URL, or longer than 2048 bytes, or carries userinfo, a fragment or a port other than 443 |
| `forbidden_host` | 403 | scheme/host not allowlisted, or target resolves privately |
| `not_found` | 404 | no route matches the path (e.g. `GET /nope`) |
| `method_not_allowed` | 405 | plus `Allow: GET, OPTIONS` |
| `upstream_too_large` | 413 | body above `CRENO_MAX_BYTES` |
| `rate_limited` | 429 | plus `Retry-After: <secs>` |
| `upstream_error` | 502 | fetch failed, upstream not 2xx |
| `busy` | 503 | every fetch slot taken (16 concurrent), plus `Retry-After: 1` |
| `timeout` | 504 | above `CRENO_TIMEOUT_SECS`, **or** DNS unresolved within `min(CRENO_TIMEOUT_SECS, 5 s)` |

A malformed request is answered before a fetch slot is taken, so it is never reported as `busy`. `/healthz` never takes a slot and always answers, even when every fetch is saturated.

Config (env, validated at startup, defaults): `CRENO_BIND=127.0.0.1:8080`, `CRENO_ALLOWED_HOSTS=api.skello.io` (each entry needs at least two labels, so a bare TLD such as `io` is refused), `CRENO_MAX_BYTES=2097152`, `CRENO_TIMEOUT_SECS=12` (the DNS step never gets more than 5 s of it), `CRENO_RATE_LIMIT=30` (per minute per IP, burst 10), `CRENO_ALLOW_EXTENSION=0`, `CRENO_ALLOW_FILE_ORIGIN=0`, `CRENO_ALLOWED_ORIGINS=` (empty).
