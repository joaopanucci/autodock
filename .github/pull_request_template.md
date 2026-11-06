## What changes and why

<!-- Explain in a few lines. Link the issue if there is one: Closes #123 -->

## How it was tested

- [ ] `cargo fmt --all -- --check`
- [ ] `cargo test -p autodock-core -p autodock-cli --locked`
- [ ] `npm --prefix apps/desktop run typecheck` and `npm --prefix apps/desktop test`
- [ ] I tried the flow in practice (e.g. generated the project and ran `docker compose up --build`)

Systems I tested manually on: <!-- macOS / Windows / Linux -->

## Security checklist

- [ ] Does not overwrite existing files or follow symlinks
- [ ] Values that end up in a Dockerfile/YAML/commands are validated (allowlist)
- [ ] Does not read `.env` or run scripts of analyzed projects
- [ ] Does not rely on assumptions about a single operating system
