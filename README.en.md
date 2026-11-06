<div align="center">

<img src="apps/desktop/src-tauri/icons/128x128@2x.png" alt="AutoDock" width="128" height="128" />

# AutoDock

**Put any project in Docker in a few clicks.**<br/>
Answer a few questions. AutoDock writes the `Dockerfile`, the `docker-compose.yml` and even the project itself if you don't have one yet. You review everything before anything is written.

[![License](https://img.shields.io/badge/license-Apache--2.0-7bc6a8)](LICENSE)
[![Release](https://img.shields.io/github/v/release/joaopanucci/autodock?color=709fe5)](https://github.com/joaopanucci/autodock/releases/latest)
![Platforms](https://img.shields.io/badge/macOS%20%C2%B7%20Windows%20%C2%B7%20Linux-4d6fa8)

[Português](README.md)

<img src="docs/screenshots/en/06-assistente-revisao.png" alt="AutoDock Docker assistant previewing a docker-compose.yml" width="920" />

</div>

> The interface and messages are available in **English and Portuguese** (Brazil). They follow your system language and can be switched in **Settings**.

## What it does

- **Existing project:** detects the stack from its manifests and generates `Dockerfile`, `docker-compose.yml`, `.dockerignore` and `.env.docker.example`.
- **New project in an empty folder:** a clean starter, or the **official framework installer** (Laravel, Django, Vite + React/Vue) run *inside* Docker, so nothing but Docker has to be installed.
- **Ready-made services:** PostgreSQL, MySQL, MongoDB, Redis, RabbitMQ, Adminer and MailHog, wired to your app with healthchecks.
- **Strong secrets:** every service gets a random 32-character password (OS CSPRNG) stored in `.env.docker`; `docker-compose.yml` contains no passwords.
- **No port clashes:** free host ports are picked automatically and remembered across your projects.
- **Open in your IDE:** VS Code, Cursor, Windsurf, IntelliJ, WebStorm, PyCharm, PhpStorm, Zed or Sublime, whichever is installed.

Supported stacks: **Node.js** (npm/pnpm/yarn, Vite), **Java/Spring Boot** (Maven), **Python** (FastAPI, Django, Flask) and **PHP/Laravel**.

## How it works

1. **Pick the project folder.** AutoDock only reads configuration files (`package.json`, `pom.xml`, `requirements.txt`, `composer.json`) to suggest the technology. Nothing of your code runs. *Empty folder? It offers to create a new project: clean, or with the official framework installer.*

<img src="docs/screenshots/en/03-assistente-projeto.png" alt="Step 1: choose the folder" width="820" />

2. **Choose the services** you need. Each one becomes a container next to your app, already connected.

<img src="docs/screenshots/en/05-assistente-servicos.png" alt="Step 3: services" width="820" />

3. **Review before anything is written.** Strong passwords go to a private `.env.docker`, busy ports are replaced by free ones, and existing files are never overwritten.

<img src="docs/screenshots/en/06-assistente-revisao.png" alt="Step 4: review" width="820" />

4. **Open it in your IDE** and run `docker compose up --build`.

<img src="docs/screenshots/en/07-assistente-pronto.png" alt="Step 5: done" width="820" />

## Download

**Quick install, no security warning.** One command downloads the latest release, **verifies its SHA-256** and installs it:

```bash
# macOS
curl -fsSL https://raw.githubusercontent.com/joaopanucci/autodock/main/scripts/install.sh | bash
```

```powershell
# Windows (PowerShell)
irm https://raw.githubusercontent.com/joaopanucci/autodock/main/scripts/install.ps1 | iex
```

Because the download does not go through a browser, the file never gets the "downloaded from the internet" mark that triggers macOS Gatekeeper and Windows SmartScreen. No protection of your system is turned off, and the app stays unsigned. The scripts are short and open: [`install.sh`](scripts/install.sh), [`install.ps1`](scripts/install.ps1).

Or get the installer from the [releases page](https://github.com/joaopanucci/autodock/releases/latest): macOS (`.dmg`, universal), Windows (`.exe`), Linux (`.deb`, `.AppImage`). Docker must be installed and running.

The installers are **not code-signed yet**, so your OS shows a warning on first launch (macOS: *System Settings → Privacy & Security → Open Anyway*; Windows: *More info → Run anyway*). Each release ships a `SHA256SUMS.txt`.

## Safety principles

Preview before writing (the previewed passwords are exactly what gets written); never overwrites files; passwords only in a private `.env.docker`; allowlisted inputs (no injection into Dockerfile/YAML/commands); no host shell; framework installers run in a throwaway container with only the empty folder mounted; service ports bound to `127.0.0.1`; never reads `.env` files or runs project scripts.

## CLI and development

```bash
autodock generate ./my-project --stack nodejs --port 3000 --service postgres          # preview
autodock generate ./my-project --stack nodejs --port 3000 --service postgres --write  # write new files only
```

See the [development guide](docs/DEVELOPMENT.md) (Portuguese) to build from source: `npm --prefix apps/desktop ci && npm --prefix apps/desktop run tauri dev`.

## Contributing

Stars, issues, pull requests and Windows/Linux testing are all welcome. See [CONTRIBUTING](CONTRIBUTING.md). Licensed under [Apache-2.0](LICENSE).
