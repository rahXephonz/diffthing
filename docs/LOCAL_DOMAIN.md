# How `local.diffthing.dev` works

diffthing serves its review UI over HTTPS from `https://local.diffthing.dev`,
but nothing runs on a remote server. The SPA and its WebSocket both run on the
local daemon on `127.0.0.1`. The public domain only exists to give that local
server a stable HTTPS origin that resolves to loopback.

## Why HTTPS at all

A page served from a real `https://` origin that then talks to `ws://127.0.0.1`
hits two browser walls:

- **Mixed content** — an HTTPS page cannot open an insecure `ws://` socket.
- **Private Network Access** — Chrome/Safari gate requests from public origins
  to loopback behind a permission prompt.

Serving _everything_ (page + WebSocket) same-origin over HTTPS on loopback
sidesteps both.

## The mechanism

1. **DNS.** `local.diffthing.dev` has a public `A` record at `127.0.0.1` (and
   `AAAA` at `::1`). Every machine that resolves the name reaches its own
   loopback.
2. **Per-install certificate.** On the first hosted-mode run, the daemon
   generates a self-signed certificate for `local.diffthing.dev`, `localhost`,
   and `127.0.0.1` and caches it in `~/.config/diffthing/tls` (private key is
   owner-only, `0600`). No key ever ships in the binary, the npm package, or
   this repository.
3. **Serve.** The daemon binds `127.0.0.1:PORT` over TLS with that cert and
   serves the embedded SPA plus `wss://local.diffthing.dev:PORT/ws`,
   same-origin.
4. **Open.** The browser resolves the domain to `127.0.0.1` and connects. The
   first time, it shows a trust prompt for the self-signed certificate —
   accept it once per install; every later run is silent.

## Security model

Earlier releases bundled one shared, publicly-trusted certificate and its
private key in every download. That key was distributed to the world, so
anyone who could bend a victim's DNS, `/etc/hosts`, or network path for
`local.diffthing.dev` could serve **browser-trusted** JavaScript under that
name and read the fragment session token. Per-install generation removes that
class of attack:

- Each install has its own key; compromising one machine's key gains nothing
  anywhere else.
- The key never leaves the machine it was generated on and is never in source
  control or release artifacts.
- The session token (URL fragment) and the loopback-only bind remain the
  primary access controls, exactly as in `--offline` mode.
- Bring your own certificate any time:

  ```bash
  DIFFTHING_TLS_CERT=/path/fullchain.pem \
  DIFFTHING_TLS_KEY=/path/privkey.pem \
  diffthing
  ```

If a network can't resolve the domain — or you don't want the one-time trust
prompt — `npx diffthing --offline` serves plain HTTP on `127.0.0.1` with no
domain or cert dependency. `http://127.0.0.1` is a browser "secure context",
so that path is fully functional too.

## Maintainer setup

One one-time step: DNS on the `diffthing.dev` zone.

```text
local    A       127.0.0.1
local    AAAA    ::1
```

There is no certificate issuance, renewal, or rotation to operate — every
install mints and caches its own.
