# How `local.diffthing.dev` works

diffthing serves its review UI over HTTPS from `https://local.diffthing.dev`,
but nothing runs on a remote server. The SPA and its WebSocket both run on the
local daemon on `127.0.0.1`. The public domain only exists to give that local
server a **browser-trusted HTTPS origin**, the same technique Drizzle Studio
uses with `local.drizzle.studio`.

## Why HTTPS at all

A page served from a real `https://` origin that then talks to `ws://127.0.0.1`
hits two browser walls:

- **Mixed content** — an HTTPS page cannot open an insecure `ws://` socket.
- **Private Network Access** — Chrome/Safari gate requests from public origins
  to loopback behind a permission prompt.

Serving _everything_ (page + WebSocket) same-origin over HTTPS on loopback
sidesteps both. To do that with no certificate warning, that origin needs a
publicly-trusted certificate — hence a real domain.

## The mechanism

1. **DNS.** `local.diffthing.dev` has a public `A` record at `127.0.0.1` (and
   `AAAA` at `::1`). Every machine that resolves the name reaches its own
   loopback.
2. **Bundled certificate.** A certificate for `local.diffthing.dev` is embedded
   in the published binary (`crates/daemon/certs`).
3. **Serve.** The daemon binds `127.0.0.1:PORT` over TLS with that cert and
   serves the embedded SPA plus `wss://local.diffthing.dev:PORT/ws`,
   same-origin.
4. **Open.** The browser resolves the domain to `127.0.0.1`, sees a valid cert,
   and connects. No warning, no prompt, no mixed content, and nothing to
   install — `npx diffthing` just works.

## Security model

The bundled TLS **private key ships in the public package, so it is not
secret** — the same posture as Drizzle Studio. This is acceptable because the
domain resolves _only_ to `127.0.0.1`:

- The key can only serve HTTPS on someone's own loopback.
- It cannot authenticate a remote host or intercept traffic to any real server.
- To misuse it, an attacker must already control the victim's network path and
  DNS (spoofing / on-path MITM) to point `local.diffthing.dev` somewhere other
  than loopback. That is a narrow, network-level attack on one dev-tool domain.
- The session token (URL fragment) and the loopback-only bind remain the actual
  access controls, exactly as in `--offline` mode.

If a network can't resolve the domain, `npx diffthing --offline` serves plain
HTTP on `127.0.0.1` with no domain or cert dependency. `http://127.0.0.1` is a
browser "secure context", so that path is fully functional too.

## Maintainer setup

Two one-time steps, then periodic cert renewal.

### 1. DNS (once)

On the `diffthing.dev` zone:

```text
local    A       127.0.0.1
local    AAAA    ::1
```

### 2. Certificate (once, then every ~90 days)

Issue a real, publicly-trusted cert for `local.diffthing.dev` and commit it over
the placeholder in `crates/daemon/certs`. Uses [lego](https://go-acme.github.io/lego)
(`brew install lego`), DNS-01, needs `diffthing.dev` DNS access:

```bash
# automated (Cloudflare):
CF_DNS_API_TOKEN=<zone-dns-edit-token> ACME_EMAIL=you@example.com pnpm cert:issue
# or interactive on any DNS host:
ACME_EMAIL=you@example.com pnpm cert:issue manual
```

Full steps: [crates/daemon/certs/README.md](../crates/daemon/certs/README.md).
This is the **only** step that produces browser trust, and only the domain
owner can perform it — issuing a trusted cert requires proving control of
`diffthing.dev`. After it's committed, every user's browser trusts
`https://local.diffthing.dev` with nothing to install.

Let's Encrypt certs last 90 days — re-issue and cut a new release before expiry.
No secrets, no CI cert steps, no per-user setup.

## Local development

The checked-in cert is a **self-signed placeholder**, so `cargo run` (before a
real cert is committed) shows a browser trust warning. Options:

- Accept the warning once.
- Bring your own trusted cert:

  ```bash
  DIFFTHING_TLS_CERT=/path/fullchain.pem \
  DIFFTHING_TLS_KEY=/path/privkey.pem \
  cargo run -p diffthing
  ```

- Or use `cargo run -p diffthing -- --offline`.
