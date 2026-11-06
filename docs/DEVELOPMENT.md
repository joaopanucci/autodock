🌐 **English** · [Português (Brasil)](DEVELOPMENT.pt-BR.md)

# Development guide

How to run, test and understand the AutoDock code. For the product overview see the [README](../README.md); for the contribution workflow see [CONTRIBUTING](../CONTRIBUTING.md).

## Prerequisites

- **Rust** stable and Cargo (CI pins the toolchain in `.github/workflows`). On Windows use the MSVC toolchain.
- **Node.js** and npm for the frontend (Node 22.18+; CI uses 24).
- **Tauri 2 system dependencies:** Xcode tools on macOS; the C++ build tools, Windows SDK and WebView2 on Windows; `libwebkit2gtk-4.1-dev` and friends on Linux. See the [official Tauri 2 prerequisites](https://v2.tauri.app/start/prerequisites/).
- **Docker** (Engine and `docker compose` v2). Analysis works without it, but creating framework projects and running the generated environments needs it.

## Project layout

```
crates/autodock-core   Rust core (no UI): scanner, generator, framework installers,
                       IDEs, ports, secrets, Docker queries, i18n
crates/autodock-cli    Command line (Clap)
apps/desktop           React + Vite interface; src-tauri is the Tauri adapter
fixtures               Small, inert manifests used by tests
scripts                install.sh / install.ps1, screenshots.mjs, legacy Windows scripts
docs                   Security principles, architecture and stage reports
```

The core must not depend on Tauri. The Tauri commands (`apps/desktop/src-tauri/src/lib.rs`) only wrap core functions and pass the caller's language.

## Run the app

```bash
npm --prefix apps/desktop ci
npm --prefix apps/desktop run tauri dev      # desktop app with hot reload
```

```bash
npm --prefix apps/desktop run build          # TypeScript + web bundle
npm --prefix apps/desktop run tauri build    # native installers (see below)
```

The web bundle alone has no Tauri commands, so the complete flow needs the desktop app.

## Command line

```bash
cargo run -p autodock-cli -- scan ./fixtures/node-basic
cargo run -p autodock-cli -- doctor --json
cargo run -p autodock-cli -- generate ./my-project --stack nodejs --port 3000 --service postgres
cargo run -p autodock-cli -- --lang pt scaffold ./empty-folder --framework laravel --name shop
```

`generate` and `scaffold` only preview unless you pass `--write` / `--run`. Set `AUTODOCK_HOME` to keep the port registry (`ports.json`) away from your real one while experimenting.

## Tests and checks

```bash
cargo fmt --all -- --check
cargo check --workspace --locked
cargo clippy -p autodock-core -p autodock-cli --all-targets --locked
cargo test -p autodock-core -p autodock-cli --locked

npm --prefix apps/desktop run typecheck
npm --prefix apps/desktop test                # unit tests (Node), includes i18n parity
npm --prefix apps/desktop run test:ui         # Playwright; AUTODOCK_BROWSER_CHANNEL=chrome|msedge
```

- Rust tests that touch the file system use temporary directories; port tests never assume a fixed free port.
- UI tests run in `pt-BR` by default (the older tests assert Portuguese); `tests/ui/i18n.spec.ts` runs in English.
- Tests that need Docker are not part of CI: validate generated projects by hand with `docker compose up --build`.

## Installers and releases

```bash
npm --prefix apps/desktop run tauri build -- --bundles app,dmg     # macOS
npm --prefix apps/desktop run tauri build -- --bundles nsis        # Windows
npm --prefix apps/desktop run tauri build -- --bundles deb,appimage # Linux
```

Releasing:

1. Open an issue/PR that bumps the version in `Cargo.toml` (workspace crates), `apps/desktop/src-tauri/tauri.conf.json`, `apps/desktop/package.json` (+ lock), `Cargo.lock`, and the download links in `README.md` / `README.pt-BR.md`.
2. After it is merged, push a tag `vX.Y.Z` on `main`. The **Build installers** workflow builds macOS (universal), Windows and Linux and creates the release (or attaches to an existing one) with a `SHA256SUMS.txt`.
3. Check the files and checksums, write the release notes in English and publish.

The **Smoke test installers** workflow installs a published release on clean Windows, macOS and Linux runners (through `scripts/install.ps1` and `scripts/install.sh`), opens the app and saves a screenshot.

## Translations and screenshots

See the *Translations* and *Updating the README screenshots* sections of [CONTRIBUTING](../CONTRIBUTING.md).

## Legacy Windows bootstrap

`bootstrap-autodock.ps1` and the PowerShell tests in `tests/` predate the CI matrix. They only apply to a full checkout on Windows, write their reports to `.autodock-bootstrap/` and do not install anything:

```powershell
pwsh -NoProfile -File .\bootstrap-autodock.ps1
pwsh -NoProfile -File tests/Diagnostics.Tests.ps1
pwsh -NoProfile -File tests/Bootstrap.Tests.ps1
```
