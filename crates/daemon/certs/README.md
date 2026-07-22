# Bundled TLS material for `local.diffthing.dev`

Drop the real, publicly-trusted certificate here to enable the zero-prompt
HTTPS flow (Drizzle Studio model). Two files, PEM:

- `local.diffthing.dev.pem` — full chain (leaf + intermediates)
- `local.diffthing.dev.key.pem` — private key

`build.rs` detects both and sets the `bundled_cert` cfg; `src/tls.rs` then
`include_bytes!`s them and serves them by default. When absent, the daemon
falls back to a per-install self-signed cert, so the crate always builds.

## Obtaining the cert

`local.diffthing.dev` resolves to `127.0.0.1` via public DNS. Issue a cert for
that name against a public CA — e.g. Let's Encrypt via a DNS-01 challenge (no
inbound port needed since the name is loopback):

```
# example, using acme.sh with your DNS provider's API
acme.sh --issue --dns dns_cf -d local.diffthing.dev
```

Then copy the fullchain and key into this directory with the names above.

## Security tradeoff (accepted)

This ships a private key inside the released binary — reversing the earlier
"no shared key ships" stance (ROADMAP item 15). The cert is only ever valid for
a name that resolves to loopback, and the session token is ephemeral per run.
The residual risk: anyone who can bend a victim's DNS/hosts for
`local.diffthing.dev` could serve trusted JS under that name and read the
fragment token. `--offline` (plain HTTP on `127.0.0.1`) remains the
zero-shared-trust path. This matches how Drizzle Studio ships `local.drizzle.studio`.

## Do not commit test/self-signed material here

Only a genuinely CA-trusted cert belongs in this directory. A self-signed cert
placed here would be embedded and served as if trusted, defeating the point and
re-triggering the Safari failure. For local testing use `mkcert` + the
`DIFFTHING_TLS_CERT` / `DIFFTHING_TLS_KEY` env override instead.
