# Créno relay — API contract v1

Base URL is user-configured in the app (e.g. `https://relais.example.fr`). Single reference for the PWA, the extension, the docs and the server.

| Request | Success | Headers |
| --- | --- | --- |
| `GET /healthz` | `200` `{"status":"ok"}` (nothing else) | `Content-Type: application/json; charset=utf-8` |
| `GET /ics?url=<percent-encoded absolute https URL>` | `200`, upstream body verbatim | `Content-Type: text/calendar; charset=utf-8`, `ETag: "sha256-<hex>"`, `Cache-Control: private, max-age=300`, `X-Content-Type-Options: nosniff` |
| `GET /ics` + `If-None-Match: <ETag>` | `304`, no body | `ETag`, `Cache-Control`, `X-Content-Type-Options` |
| `OPTIONS` any path | `204`, no body | CORS headers below |

CORS: allowed origins are `null` (file://), `moz-extension://<anything>` only when `CRENO_ALLOW_EXTENSION=1`, `http://localhost:<port>`, `http://127.0.0.1:<port>` and the exact origins of `CRENO_ALLOWED_ORIGINS`. Never `*`. Methods `GET, OPTIONS`, allowed request header `If-None-Match`, exposed response header `ETag`, always `Vary: Origin`.

Errors: body `{"error":"<code>"}`, `Content-Type: application/json; charset=utf-8`, never upstream text, never the query string.

| code | status | note |
| --- | --- | --- |
| `missing_url` | 400 | `url` absent or empty |
| `invalid_url` | 400 | not an absolute URL, or longer than 2048 bytes |
| `forbidden_host` | 403 | scheme/host not allowlisted, or target resolves privately |
| `upstream_error` | 502 | fetch failed, upstream not 2xx |
| `upstream_too_large` | 413 | body above `CRENO_MAX_BYTES` |
| `timeout` | 504 | above `CRENO_TIMEOUT_SECS` |
| `rate_limited` | 429 | plus `Retry-After: <secs>` |
| `method_not_allowed` | 405 | plus `Allow: GET, OPTIONS` |

Config (env, validated at startup, defaults): `CRENO_BIND=127.0.0.1:8080`, `CRENO_ALLOWED_HOSTS=api.skello.io`, `CRENO_MAX_BYTES=2097152`, `CRENO_TIMEOUT_SECS=12`, `CRENO_RATE_LIMIT=30` (per minute per IP, burst 10), `CRENO_ALLOW_EXTENSION=0`, `CRENO_ALLOWED_ORIGINS=` (empty).
