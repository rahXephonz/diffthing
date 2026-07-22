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
that name against a public CA — Let's Encrypt via a DNS-01 challenge (no inbound
port needed since the name is loopback). Use the helper, which issues and drops
both files here in one step (and is renew-safe for the ~90-day rotation):

```
DNS_PROVIDER=dns_cf ./scripts/cert-prod.sh    # provider creds via acme.sh env
```

Prereqs: `acme.sh` installed and your DNS provider's API credentials exported
(see the acme.sh dnsapi wiki). Also make sure an `A` record
`local.diffthing.dev → 127.0.0.1` exists so users actually reach loopback.

After it runs: `cargo build -p diffthing` (turns on `bundled_cert`), then commit
the two files and cut a release.

## Security tradeoff (accepted)

This ships a private key inside the released binary — reversing the earlier
"no shared key ships" stance (ROADMAP item 15). The cert is only ever valid for
a name that resolves to loopback, and the session token is ephemeral per run.
The residual risk: anyone who can bend a victim's DNS/hosts for
`local.diffthing.dev` could serve trusted JS under that name and read the
fragment token. `--offline` (plain HTTP on `127.0.0.1`) remains the
zero-shared-trust path. This matches how Drizzle Studio ships `local.drizzle.studio`.

## Local development

To test the trusted-cert flow on your own machine without a real CA cert, run:

```
pnpm cert:dev            # requires mkcert; extra args forward: pnpm cert:dev -- --base main
```

`scripts/cert-dev.sh` runs `mkcert -install`, mints a locally-trusted cert for
`local.diffthing.dev` into `.certs-dev/` (gitignored), and starts the daemon
with the `DIFFTHING_TLS_CERT` / `DIFFTHING_TLS_KEY` env override. Safari then
opens the page zero-prompt — but only on machines where `mkcert -install` ran.

## Do not commit test/self-signed material here

Only a genuinely CA-trusted cert belongs in this directory. A self-signed or
mkcert cert placed here would be embedded and served as if publicly trusted,
defeating the point and re-triggering the Safari failure on other machines.
