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

The workflow also signs the installers for the in-app updater and uploads their `.sig` files plus a `latest.json` (the update manifest). Publishing the release is what makes the new version visible to installed apps, because the app reads `releases/latest/download/latest.json`.

### Update signing key

- The key pair is made once with `npx tauri signer generate -w ~/.tauri/autodock-updater.key` (inside `apps/desktop`). The **public** key is in `plugins.updater.pubkey` of `apps/desktop/src-tauri/tauri.conf.json`; the **private** key and its password are the repository secrets `TAURI_SIGNING_PRIVATE_KEY` and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`. Never commit the private key.
- **Keep a backup of the private key file and its password** (a password manager). If they are lost, installed apps cannot accept updates signed with a new key: people would have to install that version by hand once.
- Local builds need the key too, because `createUpdaterArtifacts` is on: set `TAURI_SIGNING_PRIVATE_KEY` (the file content) and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` before `tauri build`.
- Testing an update without publishing: build two versions with `--config` pointing `plugins.updater.endpoints` to `http://127.0.0.1:<port>/latest.json` and `dangerousInsecureTransportProtocol: true` (test builds only), serve a `latest.json` that names the newer installer and its `.sig`, install the older one and let it update.

The **Smoke test installers** workflow installs a published release on clean Windows, macOS and Linux runners (through `scripts/install.ps1` and `scripts/install.sh`), opens the app and saves a screenshot.

## Translations and screenshots

See the *Translations* and *Updating the README screenshots* sections of [CONTRIBUTING](../CONTRIBUTING.md).

## Theme, icon and visual assets

- **Theme.** The app has a light and a dark theme (`apps/desktop/src/theme.ts`, switch in the top bar and in Settings). The choice is saved in `localStorage` (`autodock.theme`); the default follows the operating system. Colors live in tokens: `src/theme-tokens.css` defines every `--c-*` color twice (light in `:root`, dark in `:root[data-theme="dark"]`) and the hand-written tokens (`--bg`, `--panel`, `--ink`, `--line`…) sit at the end of `src/style.css`. To change a color, edit it in **both** places; do not write new hex colors in rules.
- **Icon.** The source is `apps/desktop/src-tauri/icons/app-icon.svg`. Render it to a 1024 px PNG (`app-icon.png`, transparent corners) and run `npx tauri icon src-tauri/icons/app-icon.png` from `apps/desktop` to regenerate every size.
- **Windows installer.** `bundle.windows.nsis` in `tauri.conf.json` sets the per-user install, the license page and the artwork in `src-tauri/installer/` (`sidebar.bmp` 164×314 and `header.bmp` 150×57, 24-bit). `installer/hooks.nsh` refreshes the shell icon cache after installing.
- **Brand images.** `node scripts/brand-assets.mjs` regenerates the macOS DMG background and `docs/social-preview.png`. Run `scripts/screenshots.mjs` first (it needs `cargo build -p autodock-cli` and the dev server); use `AUTODOCK_THEME=dark` for the dark set. Both scripts work on Windows, macOS and Linux (set `AUTODOCK_BROWSER_CHANNEL` if you do not have Edge or Chrome).

## Java upgrade engine

`crates/autodock-core/src/java.rs` reads a Maven project (Java level, Spring Boot, removed JDK APIs); `java_upgrade.rs` plans and runs the upgrade of a copy inside `maven:3.9-eclipse-temurin-25`. The order of the steps (`hops_for`) is what keeps every intermediate build working: Spring Boot 2.7 first (it still builds on the old level), Java 17 (Spring Boot 3 needs it), Spring Boot 3.5 (`javax` → `jakarta`), Spring Boot 4.0, Java 25.

- The OpenRewrite plugin and the recipe libraries are pinned (`REWRITE_PLUGIN`, `RECIPE_LIBRARIES`) together with the image and the target versions (`TARGET_JAVA`, `TARGET_BOOT`). To move to a newer release, change them together and run the fixtures below end to end; a recipe name that does not exist fails the run (`failOnInvalidActiveRecipes`).
- `rewrite:runNoFork` is used on purpose: `rewrite:run` compiles first, and a legacy build cannot compile on the new JDK. In a multi-module project `runNoFork` cannot resolve a module that depends on a sibling, so the script runs the `compile` phase with `-Dmaven.main.skip=true` first (see the comment on `script`).
- Each step is wrapped by `snap` and `restore` in the script: a failing recipe or a failing compilation puts the copy back (`ROLLBACK <step>` in the output), the steps after it do not run, and the tests run on the last state that worked (skipped when no step worked). The report has `rolledBack` and `reached`.
- `PINNING_BOMS` (in `java.rs`) lists BOMs that decide the Spring Boot version on their own (today only `tech.jhipster:jhipster-dependencies`). For such a project `hops_pinned` plans only Java steps, capped at Java 17 while Spring Boot is older than 3.5. A real JHipster Online project (Spring Boot 2.7.3) is what showed this: the Spring Boot 3.5 recipe applied but left an invalid pom.
- The unit tests use a fake runner (no Docker). To check a real upgrade, build small fixtures: a Spring Boot 1.5 project on Java 1.7 (JPA, a REST controller, a JUnit 4 test that uses `javax.xml.bind`), a plain Java 1.6 library with JUnit 4, a multi-module project, and one that uses `sun.misc.BASE64Encoder` (it must stop at the Java 17 step and name it). Then run `autodock java upgrade <folder> --yes` and check that the original is untouched and the copy compiles and passes its tests.

## Code editor

`crates/autodock-core/src/editor_fs.rs` is the only code that touches files for the editor (list, read, write, create; see the security notes for its rules) and the Tauri commands `editor_*` are thin wrappers. The interface is `pages/Editor.tsx` (explorer, tabs, save, conflicts) around `components/CodeEditor.tsx` (one Monaco editor with one text model per open file, controlled through a handle). `editor/monacoSetup.ts` loads Monaco on demand with its workers as files of the app (`monaco-editor/...?worker`; the package's `exports` map has no `esm/vs` in the path) and switches the JavaScript/TypeScript language service off, which keeps its very large worker out. `monaco-editor` is pinned to an exact version. Search: `editor_fs::search` walks the folder depth first in name order (so the same search always answers the same), compares characters (a case-insensitive search folds each character to one, which keeps columns right) and reports columns in UTF-16 units like Monaco. The explorer has Files / Search / Changes views; a result opens the file and `reveal`s the match through the editor handles. Reviewing an upgrade: the Java report asks the editor (through `App`, as an `EditorReview` with an id handled once) to open the upgraded copy as the project with a *Changes* list; a changed file opens as a `diff` tab in `components/DiffEditor.tsx` (Monaco's side-by-side editor, the original read with `editor_read` from the original folder and never editable, the copy editable and saved through the same `editor_write`), an added file as a normal tab, a removed one as a read-only comparison. A path has one tab only, and replacing the folder asks first when there are unsaved changes. The page is built on the first visit and then kept mounted, because Monaco adds `role="alert"` live regions to the page: tests that look for alerts must not run with it mounted.

Terminal: `autodock-core::terminal` builds the `docker run` command for a listed environment (`plan`) and runs it on a pseudo-terminal (`portable-pty`: ConPTY on Windows, a pty on Unix) as a `Session` with `write`, `resize` and `stop`. ConPTY asks for the cursor position (`ESC[6n`) and waits for the answer, which the emulator in the interface (xterm.js) gives; the tests answer it themselves. The Tauri commands `terminal_*` keep the open sessions and carry output to the webview through a `Channel` as base64 (so a character split between two reads is never broken). `components/TerminalPanel.tsx` starts one shell per folder, environment and restart, sends keystrokes in order (queued until the backend answered with the session id, big pastes in 8,000-character pieces), refits on resize and ends the container when the panel goes away. The panel is hidden, not closed, by the toggle, so the shell keeps running.

## Legacy Windows bootstrap

`bootstrap-autodock.ps1` and the PowerShell tests in `tests/` predate the CI matrix. They only apply to a full checkout on Windows, write their reports to `.autodock-bootstrap/` and do not install anything:

```powershell
pwsh -NoProfile -File .\bootstrap-autodock.ps1
pwsh -NoProfile -File tests/Diagnostics.Tests.ps1
pwsh -NoProfile -File tests/Bootstrap.Tests.ps1
```
