<div align="center">

**🌐 English · [Português (Brasil)](README.pt-BR.md)**

<img src="apps/desktop/src-tauri/icons/128x128@2x.png" alt="AutoDock" width="128" height="128" />

# AutoDock

**Put any project in Docker in a few clicks.**<br/>
Answer a few questions. AutoDock writes the `Dockerfile`, the `docker-compose.yml` and even the project itself if you don't have one yet. You review everything before anything is written.

[![CI](https://github.com/joaopanucci/autodock/actions/workflows/rust-ci.yml/badge.svg?branch=main)](https://github.com/joaopanucci/autodock/actions/workflows/rust-ci.yml)
[![Release](https://img.shields.io/github/v/release/joaopanucci/autodock?color=22252a&label=release)](https://github.com/joaopanucci/autodock/releases/latest)
[![License](https://img.shields.io/badge/license-Apache--2.0-22252a)](LICENSE)
![Platforms](https://img.shields.io/badge/macOS%20%C2%B7%20Windows%20%C2%B7%20Linux-22252a)
![Tauri](https://img.shields.io/badge/Tauri-2-24C8DB?logo=tauri&logoColor=white)
![Rust](https://img.shields.io/badge/Rust-core-CE422B?logo=rust&logoColor=white)
![React](https://img.shields.io/badge/React-19-61DAFB?logo=react&logoColor=black)
[![PRs welcome](https://img.shields.io/badge/PRs-welcome-22252a)](CONTRIBUTING.md)

[**Download**](#-download) · [**Step by step**](#-step-by-step) · [**CLI**](#-command-line) · [**Contribute**](#-contributing)

<br/>

<img src="docs/screenshots/en/06-assistente-revisao.png" alt="AutoDock's Docker assistant previewing a docker-compose.yml" width="920" />

</div>

<br/>

## ✨ Why AutoDock?

Setting up Docker for a local project is repetitive: which base image, which port, how to wire the database, what to leave out of the image. AutoDock turns that into a conversation.

- 🧭 **Guided.** It asks what matters (technology, port, version, databases, cache) and handles the rest.
- 👀 **Transparent.** It shows every file and every command **before** anything changes.
- 🛡️ **Safe by default.** It never overwrites files, never reads your `.env`, never runs your project's scripts.
- 🔐 **Really strong passwords.** Every service gets a random 32-character password (from the operating system's cryptographic generator), kept in `.env.docker`. `docker-compose.yml` contains no passwords.
- 🔌 **No port clashes.** AutoDock checks which ports are free and remembers the ones used by your other projects, so ten projects never fight over 5432.
- 🧰 **Nothing to install except Docker.** The frameworks' official installers run *inside* containers.
- 🖥️ **Local and open source.** Your code never leaves your computer. Apache-2.0 license.
- 🏷️ **Named after your project.** The compose project, the containers and the images carry your project's name (`shop`, `shop-postgres`…), never a generic one.
- 🌗 **Light and dark.** Switch the theme from any screen; it can also follow your system.
- 🎓 **Learn as you go.** 13 short lessons, a quick reference with commands for PowerShell, CMD and Bash, and a glossary, all inside the app.
- 🌍 **English and Portuguese.** The interface and messages follow your system language and can be switched in Settings.

## 🚀 What it does

| | |
|---|---|
| **Existing project** | Detects the technology from its manifests and generates `Dockerfile`, `docker-compose.yml`, `.dockerignore` and `.env.docker.example`. |
| **New project in an empty folder** | Creates a *clean* project, or uses the framework's **official installer**, all inside Docker. |
| **Ready-made services** | PostgreSQL, MySQL, MongoDB, Redis, RabbitMQ, Adminer and MailHog, already wired to your app through environment variables, with healthchecks, **generated strong passwords** and **automatically chosen free ports**. |
| **Several apps (monorepos, microservices)** | Finds every app in the folder (backend, front end, mobile…), writes **one Dockerfile per app** and **one compose file** that runs them together with the shared services, wired to each other (the front end receives the backend's address). Existing Dockerfiles and compose files are reused, never overwritten. |
| **Run everything** | One button (after you confirm) runs `docker compose up -d --build`, shows the state of every service and lets you stop or remove the containers. Volumes (your data) are never removed. |
| **Expo: SDK alerts and upgrade** | Shows the SDK of your Expo projects and tells you when a newer **stable** SDK is out ("your project is on SDK 57, but Expo released SDK 58: upgrade?"). The upgrade follows the official steps (`expo install`, `expo install --fix`, `expo-doctor`) **inside Docker**, one SDK at a time, with a backup of `package.json` and the lock file that is restored by itself if something fails. |
| **Open in your IDE** | After creating, opens the project in whichever of VS Code, Cursor, Windsurf, IntelliJ, WebStorm, PyCharm, PhpStorm, Zed or Sublime you have installed. |
| **Environment view** | Analyzes projects, diagnoses your environment, lists containers, shows metrics and logs, all read-only. |

### Technologies

| Technology | Existing project | New project (clean) | New project (official framework) |
|---|:---:|:---:|:---:|
| **Node.js** | ✅ npm · pnpm · yarn, Vite, Next.js, NestJS | ✅ Express | ✅ Vite + React · Vite + Vue · Next.js · NestJS (TypeScript) |
| **Java / Spring Boot** | ✅ Maven | ✅ Spring Boot + Maven | 🔜 Spring Initializr |
| **Python** | ✅ FastAPI · Django · Flask | ✅ FastAPI · Flask | ✅ Django |
| **PHP / Laravel** | ✅ Composer | — | ✅ Laravel |
| **Go** | ✅ `go.mod` (multi-stage build) | ✅ `net/http` | — |
| **.NET / ASP.NET Core** | ✅ `.csproj` (multi-stage build) | ✅ minimal API | — |
| **Ruby on Rails** | ✅ Gemfile | — | — |

## 📦 Download

Get the installer for your platform from the [releases page](https://github.com/joaopanucci/autodock/releases/latest):

| System | File | How to install |
|---|---|---|
| 🍎 **macOS** (Intel and Apple Silicon) | [`AutoDock_0.2.0_universal.dmg`](https://github.com/joaopanucci/autodock/releases/download/v0.2.0/AutoDock_0.2.0_universal.dmg) | Open the DMG and drag the app to *Applications*. |
| 🪟 **Windows** (64-bit) | [`AutoDock_0.2.0_x64-setup.exe`](https://github.com/joaopanucci/autodock/releases/download/v0.2.0/AutoDock_0.2.0_x64-setup.exe) | Run the installer. |
| 🐧 **Linux** (Debian/Ubuntu) | [`AutoDock_0.2.0_amd64.deb`](https://github.com/joaopanucci/autodock/releases/download/v0.2.0/AutoDock_0.2.0_amd64.deb) | `sudo apt install ./AutoDock_0.2.0_amd64.deb` |
| 🐧 **Linux** (any distro) | [`AutoDock_0.2.0_amd64.AppImage`](https://github.com/joaopanucci/autodock/releases/download/v0.2.0/AutoDock_0.2.0_amd64.AppImage) | `chmod +x AutoDock_*.AppImage && ./AutoDock_*.AppImage` |

> **Requirement:** [Docker](https://docs.docker.com/get-docker/) installed and running, to create and run the environments.

### ⚡ Quick install, no security warning

One command downloads the latest release, **verifies its SHA-256** and installs it:

```bash
# macOS
curl -fsSL https://raw.githubusercontent.com/joaopanucci/autodock/main/scripts/install.sh | bash
```

```powershell
# Windows (PowerShell)
irm https://raw.githubusercontent.com/joaopanucci/autodock/main/scripts/install.ps1 | iex
```

Because the download does not go through a browser, the file never gets the "downloaded from the internet" mark that triggers macOS Gatekeeper and Windows SmartScreen. No protection of your system is turned off, and the app stays unsigned. The scripts are short and open: [`install.sh`](scripts/install.sh) and [`install.ps1`](scripts/install.ps1). To pin a version: `AUTODOCK_VERSION=v0.2.0`.

<details>
<summary><b>⚠️ Downloaded with a browser and got "unidentified developer"? That is expected</b></summary>

<br/>

AutoDock is open source and the installers are **not code-signed yet** (signing requires paid Apple and Microsoft programs). That is why your system shows a warning on first launch. The code is public and every release ships a `SHA256SUMS.txt` so you can verify the file you downloaded.

- **macOS:** open the app once, then go to **System Settings → Privacy & Security** and click **Open Anyway**. Terminal alternative: `xattr -dr com.apple.quarantine /Applications/AutoDock.app`.
- **Windows:** on the SmartScreen screen, click **More info** and then **Run anyway**.
- **Linux:** there is no warning.

Verify the file's integrity:

```bash
shasum -a 256 AutoDock_0.2.0_universal.dmg   # compare with SHA256SUMS.txt
```

</details>

> 🧪 **Platform status:** CI builds and tests the core on macOS, Windows and Linux, and builds the three installers for every release. The complete app was tested manually on macOS; reports from Windows and Linux are very welcome.

## 🎬 Step by step

### 1. Pick the project folder

Select the folder. AutoDock reads **only** configuration files (`package.json`, `pom.xml`, `requirements.txt`, `composer.json`, `go.mod`, `*.csproj`, `Gemfile`) to suggest the technology. None of your code is run.

<img src="docs/screenshots/en/03-assistente-projeto.png" alt="Step 1: choose the folder and detect the technology" width="820" />

> 💡 **Empty folder?** The assistant offers **Create new project**: you pick the technology, name it and choose between a **clean** project or the framework's **official installer** (Laravel, Django, Vite + React/Vue). Before running it, it shows the exact command.

### 2. Confirm the technology and answer the essentials

Port, runtime version, package manager, start script. All with sensible defaults.

<img src="docs/screenshots/en/04-assistente-tecnologia.png" alt="Step 2: technology, port and version" width="820" />

### 3. Choose the services

Tick what the project needs. Each item becomes a container next to your app, already connected.

<img src="docs/screenshots/en/05-assistente-servicos.png" alt="Step 3: services such as PostgreSQL, Redis and Adminer" width="820" />

### 4. Review before creating

See the content of every file. Nothing has been written yet. Files that already exist are **never** overwritten.

<img src="docs/screenshots/en/06-assistente-revisao.png" alt="Step 4: review of the generated files" width="820" />

### 5. Done: open it in your IDE and start the environment

One click opens the project in the editor you have installed. Then just run:

```bash
docker compose up --build
```

<img src="docs/screenshots/en/07-assistente-pronto.png" alt="Step 5: project created, buttons to open it in an IDE" width="820" />

### Beyond the assistant

<table>
<tr>
<td width="50%"><b>Analyze a project</b><br/>Technologies, modules and evidence, with the source of every piece of information.<br/><img src="docs/screenshots/en/02-analise.png" alt="Project analysis" /></td>
<td width="50%"><b>See your containers</b><br/>Inventory of your local Docker, grouped by project, with state and health.<br/><img src="docs/screenshots/en/08-containers.png" alt="Container list" /></td>
</tr>
<tr>
<td width="50%"><b>Diagnose the environment</b><br/>Node, Git, Docker, Compose and Engine, without assuming anything.<br/><img src="docs/screenshots/en/09-ambiente.png" alt="Environment diagnostics" /></td>
<td width="50%"><b>Overview</b><br/>Your projects and the state of your environment in one place.<br/><img src="docs/screenshots/en/01-visao-geral.png" alt="Overview" /></td>
</tr>
</table>

### 🌗 Light and dark

<table>
<tr>
<td width="50%"><b>Light</b><br/><img src="docs/screenshots/en/04-assistente-tecnologia.png" alt="Technology step, light theme" /></td>
<td width="50%"><b>Dark</b><br/><img src="docs/screenshots/dark/en/04-assistente-tecnologia.png" alt="Technology step, dark theme" /></td>
</tr>
</table>

## 🔄 How it works

```mermaid
flowchart LR
  A[Project folder] --> B{What is in it?}
  B -->|Existing project| C[Detects the technology<br/>reading manifests only]
  B -->|Empty folder| D[New project<br/>clean or official framework<br/>inside Docker]
  C --> E[Questions: port, version, services]
  D --> E
  E --> F[Preview of every file]
  F -->|you confirm| G[Dockerfile · docker-compose.yml<br/>.dockerignore · .env.docker.example]
  G --> H[Open in your IDE<br/>docker compose up]
```

## 🛡️ Security principles

AutoDock touches your files, so trust comes first:

- **Preview before writing.** The content is generated in the backend from validated options; the interface never sends file contents.
- **Never overwrites.** Files are created exclusively; symlinks are not followed.
- **Passwords outside the compose file.** 32-character passwords (system CSPRNG) go to `.env.docker`, with restricted permissions, and the preview shows exactly what will be written. `.env.docker.example` only lists variable names.
- **Allowlisted inputs.** Ports, versions, names and scripts are validated, so there is no injection into Dockerfiles, YAML or command lines.
- **No shell.** Editors and installers are started with fixed arguments, never with assembled text.
- **Isolated framework installers.** They run in a throwaway container with **only** the empty folder mounted.
- **Service ports on `127.0.0.1` only.** Databases and queues are not exposed to your network.
- **Never reads `.env`** nor runs scripts of the analyzed projects.
- **Expo upgrades are isolated and reversible.** The package manager runs inside a throwaway container (`node:22-bookworm-slim`) that sees the project folder but gets its own empty `node_modules`, so install scripts never run on your computer. Only `package.json` and the lock file change; they are backed up outside the project first and put back automatically if the upgrade fails (or on demand, later).
- **One optional network request.** The latest Expo SDK is read from the npm registry (`registry.npmjs.org`, the version list of the `expo` package; nothing from your projects is sent), only when you press *Check now* or turned on the automatic check (off by default, at most once a day). Watched projects are stored as plain folder paths in `~/.autodock/expo.json`, and only the ones you add.
- **Running is explicit.** The only thing AutoDock executes beyond reading is `docker compose` (up, stop, down), and only after you tick a confirmation. It uses fixed arguments and a compose file of the project root, and it never removes volumes or images. Note that building the images runs the steps of the Dockerfiles, and starting the containers runs your app, both inside Docker.

Details in [`docs/SECURITY.md`](docs/SECURITY.md).

## ⌨️ Command line

Everything the assistant does is also available in the terminal (`cargo run -p autodock-cli -- …`):

```bash
# the CLI follows your system language; force it with --lang pt or --lang en
# analyze a project and diagnose the environment
autodock scan ./my-project
autodock doctor

# show the preview (nothing is written)...
autodock generate ./my-project --stack nodejs --port 3000 --service postgres --service redis
# ...and write only the new files
autodock generate ./my-project --stack nodejs --port 3000 --service postgres --write

# new project in an empty folder
autodock generate ./new --new --stack python --port 8000 --write
# stacks: nodejs, spring-boot, python, laravel, go, dotnet, rails
autodock generate ./api --stack go --port 8080 --service postgres
autodock scaffold ./new --framework laravel --name shop          # only shows the command
autodock scaffold ./new --framework laravel --name shop --run    # runs it
# frameworks: laravel, django, vite-react, vite-vue, nextjs, nestjs
autodock scaffold ./site --framework nextjs --name site

# projects with several apps (monorepo, microservices)
autodock apps ./shop                                   # which apps are there
autodock multi ./shop --service postgres               # preview: one Dockerfile per app + compose
autodock multi ./shop --service postgres --write       # write the new files
autodock compose ./shop up --yes                       # docker compose up -d --build (also: stop, down, status)

# Expo projects: SDK, alerts and upgrade
autodock expo inspect ./app                            # SDK, package manager, generated native folders
autodock expo watch ./app                              # keep an eye on this project (only its path is stored)
autodock expo check                                    # latest stable SDK on npm vs your watched projects
autodock expo plan ./app                               # what the upgrade to the next SDK would do (runs nothing)
autodock expo upgrade ./app --yes                      # runs it in Docker, with a backup (also: backups, restore --yes)
```

## 🧱 Architecture

```
crates/autodock-core   Rust core: scanner, generator, installers, IDEs, Docker (no UI)
crates/autodock-cli    Command line (Clap)
apps/desktop           React + Vite interface, packaged with Tauri 2
docs/                  Architecture, security, roadmap and development guide
```

To run from source, see the [development guide](docs/DEVELOPMENT.md).

```bash
npm --prefix apps/desktop ci
npm --prefix apps/desktop run tauri dev
```

## 🗺️ Roadmap

- [x] Projects with several apps: one container per app, run everything with one click
- [x] Docker assistant for existing projects (Node.js, Next.js, NestJS, Spring Boot, Python, Laravel, Go, .NET, Rails)
- [x] Services: PostgreSQL, MySQL, MongoDB, Redis, RabbitMQ, Adminer, MailHog
- [x] New project: clean or with the official installer (Laravel, Django, Vite, Next.js, NestJS)
- [x] Open the project in the installed IDE
- [x] Installers for macOS, Windows and Linux
- [x] Interface in English and Portuguese
- [x] Light and dark themes
- [x] Learn area (lessons, quick reference, glossary)
- [x] Containers and images named after your project
- [ ] Spring Initializr (choose dependencies)
- [x] Expo: detect the SDK, alert when a newer stable one is out and upgrade with backup
- [ ] Gradle, legacy Java upgrades and more services (Kafka, MinIO…)
- [ ] Production Dockerfiles (multi-stage)
- [ ] Code signing and notarization of the installers

## 🤝 Contributing

AutoDock is built in the open and every bit of help counts:

- ⭐ **Star the repository** to help other people find the project.
- 🐛 **Report problems** and 💡 **suggest ideas** in the [issues](https://github.com/joaopanucci/autodock/issues).
- 🔧 **Send a pull request.** Read [CONTRIBUTING](CONTRIBUTING.md) and the [development guide](docs/DEVELOPMENT.md).
- 🧪 **Test on Windows and Linux** and tell us how it went.
- 💙 **Support with a PIX.** The app has a **Contribute** tab with a QR code (PIX is a Brazilian instant payment system). It is voluntary and no feature depends on it.

## 📄 License

[Apache-2.0](LICENSE) © AutoDock contributors.

<div align="center"><sub>Made with 🦀 Rust, ⚛️ React and 🐳 Docker.</sub></div>
