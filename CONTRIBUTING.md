🌐 **English** · [Português (Brasil)](CONTRIBUTING.pt-BR.md)

# Contributing to AutoDock

Thanks for wanting to help! AutoDock is open source ([Apache-2.0](LICENSE)) and every contribution counts: code, tests, documentation, bug reports and testing on other systems.

## Ways to help

- 🐛 **Report a problem** or 💡 **suggest an idea** by opening an [issue](https://github.com/joaopanucci/autodock/issues/new/choose).
- 🧪 **Test on Windows and Linux** and tell us how it went (the complete app was mostly tested on macOS).
- 🔧 **Send a pull request** (workflow below).
- 📝 **Improve the documentation** or the translations.

## Contribution workflow

1. Open (or pick) an **issue** that describes the work. Issues, pull requests and commit messages are written in **English**.
2. **Fork** the repository and create a branch from `main` (`feat/my-idea`, `fix/my-bug`).
3. Keep changes **small and focused**, with tests.
4. Run the checks below.
5. Open a **pull request** to `main` that says *what* and *why* (`Closes #123`). CI runs on Linux, Windows and macOS.
6. A maintainer reviews it. Only maintainers merge.

> Commit messages follow [Conventional Commits](https://www.conventionalcommits.org/) (`feat:`, `fix:`, `docs:`, `test:`, `ci:`).

## Running locally

Prerequisites and the full command list are in the [development guide](docs/DEVELOPMENT.md). In short:

```bash
npm --prefix apps/desktop ci
npm --prefix apps/desktop run tauri dev      # opens the app
```

### Checks before opening the PR

```bash
cargo fmt --all -- --check
cargo check --workspace --locked
cargo test -p autodock-core -p autodock-cli --locked
npm --prefix apps/desktop run typecheck
npm --prefix apps/desktop test
npm --prefix apps/desktop run test:ui        # needs a Chromium/Chrome/Edge browser
```

If something cannot be tested on your system, **say so in the PR** instead of claiming it works.

## Principles every PR must respect

AutoDock changes users' files, so trust comes first (see also [`docs/SECURITY.md`](docs/SECURITY.md)):

- **`autodock-core` does not depend on Tauri.** All logic lives in the core; the UI only calls commands.
- **Preview before writing.** Nothing is written without confirmation, and content is generated in the backend, never sent by the interface.
- **Never overwrite** existing files and never follow symlinks when writing.
- **Allowlisted inputs.** Values that end up in a Dockerfile, YAML or command line (ports, versions, names, scripts) must be validated. No building commands from text and no host shell.
- **Never read `.env`** or run scripts of the analyzed projects.
- **Auxiliary service ports on `127.0.0.1` only.**
- **Cross-platform.** Avoid macOS/Linux assumptions in tests (file names, permissions, `\\?\` paths on Windows).

## Where to change things

| I want to… | File |
|---|---|
| Add a service (for example MariaDB) or adjust a Dockerfile/compose template | `crates/autodock-core/src/generate.rs` |
| Add an official framework installer | `crates/autodock-core/src/scaffold.rs` |
| Add an IDE | `crates/autodock-core/src/ide.rs` |
| Change a screen | `apps/desktop/src/pages/` |

Every core change needs tests. To really validate, generate the project and bring it up with `docker compose up --build` (the CLI helps: `autodock generate … --write`).

## Translations (EN / PT-BR)

The app is bilingual (English and Brazilian Portuguese). **Every visible text must exist in both languages.**

- **Interface (React):** texts live in `apps/desktop/src/i18n/pt.ts` and `en.ts`. Use `const { t } = useI18n()` and `t("key", { param })`; plurals use the `key.one` / `key.other` pair with `tn("key", n)`. TypeScript fails if a key exists in only one language, and `npm test` also checks the `{x}` parameters.
- **Core (Rust):** messages use `tr!("texto em português", "english text")`, with both texts side by side, so it does not compile without the translation. The language comes from the command (`lang`) and from the CLI's `--lang`.
- **New language:** copy `en.ts` to `<language>.ts`, register it in `i18n/core.ts` (`LANGUAGES`) and open a PR. For Rust, extend `Lang` in `crates/autodock-core/src/i18n.rs`.
- When you change texts, run `npm --prefix apps/desktop test` and the English UI test (`tests/ui/i18n.spec.ts`).

## Documentation in both languages

Repository documents exist in **English** (the main file, which GitHub shows) and **Portuguese** (`*.pt-BR.md`), each with a language switcher at the top. When you change one, update the other in the same PR: `README`, `CONTRIBUTING`, `SECURITY`, `CODE_OF_CONDUCT` and `docs/DEVELOPMENT`.

## Updating the README screenshots

```bash
cargo build -p autodock-cli
npm --prefix apps/desktop run dev &
node scripts/screenshots.mjs                 # Portuguese -> docs/screenshots
AUTODOCK_LANG=en node scripts/screenshots.mjs  # English -> docs/screenshots/en
```

## Code of conduct

By taking part you agree to follow the [Code of Conduct](CODE_OF_CONDUCT.md).

## License

By contributing, you agree that your contribution is licensed under [Apache-2.0](LICENSE).
