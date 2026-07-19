# diffthing

> Your agent writes code. You still own judgment.

Local-first diff review for AI-assisted development. diffthing turns
working-tree changes into a prioritized walkthrough, keeps review state stable
while agents keep editing, and stages files only after human approval.

AI organizes and executes. It never approves code.

## Usage

Run inside any Git repository:

```bash
npx diffthing
```

```text
  diffthing 0.1.0
  reviewing /path/to/project against HEAD
  llm       your-llm (your login)
  ✓ ready   0 files, 0 changes, 1 AI-organized scopes

  open  https://local.diffthing.dev:58826/#port=58826&token=…
```

Open the printed URL to review uncommitted changes against `HEAD`.
`local.diffthing.dev` resolves to `127.0.0.1` — the review UI runs entirely on
your machine, served over HTTPS from the embedded local daemon. Nothing to
install or trust.

Install globally instead:

```bash
npm install -g diffthing
diffthing
```

## Options

```text
diffthing [OPTIONS]

--base <BASE>  Diff base; default HEAD
--offline      Serve over plain HTTP on 127.0.0.1 instead of HTTPS via
               local.diffthing.dev
--port <PORT>  Fixed port; default first free port
--repo <REPO>  Repository root; default current directory
--llm <LLM>    claude | codex | gemini | kimi | qwen | opencode | none | auto
```

The npm build ships a prebuilt binary with the review UI embedded and serves it
over HTTPS via `local.diffthing.dev` by default. If your network can't resolve
that domain, use `npx diffthing --offline` for plain HTTP on loopback. See
[how it works](https://github.com/rahXephonz/diffthing/blob/main/docs/LOCAL_DOMAIN.md).

## Agent support

diffthing uses coding-agent CLIs already installed and authenticated on your
machine. No provider key is stored by diffthing. Supported: `claude`, `codex`,
`gemini`, `kimi`, `qwen`, `opencode`. Force one with `--llm`; use `--llm none`
for deterministic file-order fallback.

## Links

- Source: https://github.com/rahXephonz/diffthing
- License: [MIT](https://github.com/rahXephonz/diffthing/blob/main/LICENSE)
