# Bundled TLS certificate for `local.diffthing.dev`

These files are embedded into the daemon at build time (`src/tls.rs`) and
served over HTTPS on `127.0.0.1`. `local.diffthing.dev` resolves via public DNS
to loopback, so the browser reaches the local daemon over a trusted origin.

- `local.diffthing.dev.pem` — certificate chain
- `local.diffthing.dev.key` — private key

## This key is intentionally public

Because `local.diffthing.dev` only ever resolves to `127.0.0.1`, this key can
only serve HTTPS on a user's own loopback — it cannot authenticate any remote
host. It is committed on purpose, the same tradeoff Drizzle Studio makes with
`local.drizzle.studio`. See [docs/LOCAL_DOMAIN.md](../../../docs/LOCAL_DOMAIN.md).

## The committed file is a self-signed placeholder

The checked-in cert is **self-signed**, so browsers show a trust warning. For
the zero-warning experience, a maintainer must replace it with a real,
publicly-trusted certificate for `local.diffthing.dev` and commit that.

### Issuing the real certificate (one-time + renewals)

The domain points at loopback, so HTTP-01 validation can't reach it — issuance
uses a DNS-01 challenge, which needs control of the `diffthing.dev` DNS zone.
Uses [lego](https://go-acme.github.io/lego) (`brew install lego`). One command
writes the result here:

```bash
# automated (diffthing.dev is on Cloudflare):
CF_DNS_API_TOKEN=<zone-dns-edit-token> ACME_EMAIL=you@x.com pnpm cert:issue

# or interactive on any DNS host (prints a TXT record to add):
ACME_EMAIL=you@x.com pnpm cert:issue manual
```

See [scripts/issue-cert.sh](../../../scripts/issue-cert.sh). It refuses to
install a self-signed cert, so a successful run means browsers will trust it.

Let's Encrypt certs last 90 days: re-run and commit a new release before
expiry. Users get the new cert with the next `npx diffthing` upgrade.

Users who can't wait for a renewal, or don't want the bundled cert, can always
run `npx diffthing --offline` (plain HTTP loopback) or supply their own cert
via `DIFFTHING_TLS_CERT` / `DIFFTHING_TLS_KEY`.
