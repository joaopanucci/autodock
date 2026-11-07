# Security

Never execute scripts from scanned projects during detection. Do not read `.env` or secret files. Never overwrite generated files without review and consent. Never delete unrelated Docker resources.

## Generated secrets and ports

- Database and broker passwords are 32 alphanumeric characters derived from a seed taken from the operating system CSPRNG (`getrandom`). They are written only to `.env.docker`, created exclusively (never overwritten) and with mode `0600` on Unix. They never appear in `docker-compose.yml` or in the example file.
- The seed is returned with the preview and sent back on apply, so the written files are identical to what the user reviewed. A tampered seed or port list is rejected.
- Host ports are checked by binding them locally and against a registry of ports reserved by other projects (`~/.autodock/ports.json`, overridable with `AUTODOCK_HOME`). Service ports stay published on `127.0.0.1` only.

## Running the compose project (multi-app)

AutoDock can run `docker compose` for a project, which is the only operation that executes something beyond reading and writing files:

- It needs an explicit confirmation on every call (`confirmed: true` from the interface, `--yes` in the CLI).
- The compose file must be one of the usual names (`docker-compose.yml`, `docker-compose.yaml`, `compose.yml`, `compose.yaml`) or `docker-compose.autodock.yml`, as a regular file at the root of the project (not a symbolic link). No other path or text reaches the command line.
- The commands are fixed: `up -d --build`, `stop` and `down`. `down` never passes `--volumes` or `--rmi`, so data volumes and images are kept.
- There is no shell: the arguments are passed as an array, with a time limit, and the process group is stopped when the limit is reached.
- Building the images runs the steps of the Dockerfiles and starting the containers runs the application; both happen inside Docker, as they would if you ran the same command yourself.
- Generating files for several apps validates every app folder (relative, plain characters, inside the project, no symbolic links), names, ports and versions against allowlists before anything is written.

## Expo SDK alerts and upgrades

Inspecting an Expo project only reads `package.json`, the lock file, `.gitignore` and `node_modules/expo/package.json` as data. Nothing of the project is executed to find its SDK.

**Looking up the latest SDK** is the only network access of this feature:

- It runs `curl` with a fixed HTTPS URL (`https://registry.npmjs.org/-/package/expo/dist-tags`), `--proto =https`, a 64 KB size limit and a 20 s timeout. No project data, path or identifier is sent.
- Only the stable `latest` tag counts (`next`, `canary` and other tags are ignored) and an `sdk-N` tag must point to major N.
- It happens when the user presses *Check now* (or runs `autodock expo check`) or, if the user turned it on (off by default), at most once a day when the app opens and only when at least one project is watched. A failure never blocks anything.

**Watched projects** are kept in `~/.autodock/expo.json`: only folder paths, only projects the user added (at most 50), written through a temporary file. A corrupted file is renamed to `expo.json.invalid` and never overwritten.

**Upgrading** executes the package manager, so it is held to the same rules as the other Docker operations, plus a few of its own:

- It needs an explicit confirmation on every call (`confirmed: true` from the interface, `--yes` in the CLI); the plan can be read without running anything (`autodock expo plan`).
- Everything that runs project code (`npm install`, `expo install --fix`, `expo-doctor`) runs **inside a container** (`node:22-bookworm-slim`). The project folder is bind-mounted, but `node_modules` is an anonymous volume that hides the one on the host, so install scripts never run on the user's computer and the host `node_modules` is neither read nor written. The container has no privileged mode, no extra mounts and no Docker socket.
- Only the SDK number (an integer), the package manager (an allowlisted word) and, on Linux and macOS, the user and group ids (integers) reach the script that runs in the container. The folder path goes in a comma-separated `--mount` value, so a path with a comma, a quote or a control character is refused, and the Windows `\\?\` prefix that Docker rejects is removed.
- Projects whose upgrade cannot be done safely are refused with the reason: `bun` lock files, monorepo roots (`workspaces`) and projects inside a workspace.
- Only `package.json` and the lock file may change. Before the container starts, copies of them are written to `~/.autodock/backups/expo/<id>/` (outside the project) with a manifest; if the upgrade fails, times out or Docker is missing, they are put back automatically and a lock file that did not exist before is removed again. The generated `android/` and `ios/` folders are never deleted or edited: the report lists what to do about them.
- A backup can be restored later (`autodock expo restore`, with confirmation). The backup id is validated (`<digits>-<digits>-<digits>`), the backup must belong to the same project, and the interface warns when `package.json` changed after the upgrade because restoring discards that work.
- npm falls back to `--legacy-peer-deps` only after an `ERESOLVE` error, and the report says so.

## Java upgrades

Inspecting a Java project only reads `pom.xml` files (DTDs are not expanded) and the text of the `.java` files in each module folder (wherever the project keeps them; `target`, hidden folders, `node_modules` and other modules' folders are skipped) as data, to find the Java level, the Spring Boot version and the removed JDK APIs the sources use. Nothing of the project is executed to do it, and symbolic links are never followed.

**The project is never modified.** The upgrade works on a copy:

- The copy is created **next to the project**, or in a folder the person chooses (`--into`, or the folder picker; it must exist, be a real folder and lie outside the project, because a copy inside the project would be copied into itself), as `<name>-java25` (then `-2`, `-3`… up to 20) with `create_dir`, which fails when the folder exists, so nothing is ever copied over something. `.git`, `.idea`, `node_modules`, `.gradle`, `.svn`, `.hg`, the `target` folder of a Maven module, `.env` and `.env.*` files (except `.env.example`, `.env.sample` and `.env.template`), `*.iml` files and symbolic links are not copied. The copy is refused above 1 GB or 100,000 files. If copying fails, the new folder is removed.
- Everything that runs project code (the build plugins in the `pom.xml`, the tests, the OpenRewrite recipes) runs **inside a container** (`maven:3.9-eclipse-temurin-25`) that sees **only the copy**. The original folder is not mounted. The container has no privileged mode, no extra mounts and no Docker socket. The Maven artifacts it downloads (from Maven Central, by Maven itself) are kept in a named Docker volume (`autodock-maven-cache`) that never mixes with the `~/.m2` of the computer; `docker volume rm autodock-maven-cache` removes it.
- The OpenRewrite plugin and the recipe libraries are **pinned to exact versions** in the code (`rewrite-maven-plugin`, `rewrite-spring`, `rewrite-migrate-java`), the combination that was run end to end, and the plugin is told to fail on an unknown recipe name. Only these constants, the step names and, on Linux and macOS, the user and group ids (integers) reach the script that runs in the container. The folder path goes in a comma-separated `--mount` value, so a path with a comma, a quote or a control character is refused.
- It needs an explicit confirmation on every call (`confirmed: true` from the interface, `--yes` in the CLI); the plan can be read without running anything (`autodock java plan`).
- Before the container starts, AutoDock itself adds `<dependency>` entries to the `pom.xml` of the module that uses an API the JDK stopped shipping (JAXB, `javax.annotation`, `javax.activation`, JAX-WS, JTA). The pom is rewritten through a temporary file and an artifact that is already declared is skipped. APIs with no replacement (`sun.*`, CORBA) are only reported.
- Before every step the container keeps a snapshot of the copy (`/snap`, inside the container only); a step that fails, in the recipe or in the compilation after it, is undone by restoring that snapshot, so the copy is never left half upgraded. The original is not involved at any point.
- The build output (`target`) is removed from the copy afterwards. On failure the copy stays for inspection (the report names the step and shows the compiler output); if Docker is missing, nothing ran and the copy is removed again.
- Gradle projects, and folders that are a module of a larger Maven tree (a parent pom outside the folder), are refused with the reason.

## Code editor

The editor reads and writes files, so it is held to strict rules (`autodock-core::editor_fs`):

- It only sees the folder the person chose. Every path from the interface must be relative and made of plain names: no `..`, no drive or leading separator, no control characters, at most 64 parts and 255 bytes per name. A backslash is always a separator.
- Every part of a path that exists must be a real folder or file. **Links are never followed**: symbolic links and, on Windows, junctions and any other reparse point are refused for reading, listing, writing and creating, so nothing can lead outside the chosen folder. A link is listed as a link and cannot be opened.
- Files above 2 MB, binary files (a NUL byte in the first 8,000 bytes) and files that are not UTF-8 are refused with the reason instead of being cut or converted, because saving them would corrupt them. A UTF-8 byte order mark is kept on save.
- Saving writes a temporary file next to the original and renames it over it (a crash never leaves half a file), keeps the permissions, and only works on an existing regular file. The editor sends back the modified time it read; if the file changed on disk since, nothing is written unless the person chooses to overwrite.
- New files and folders are created exclusively (`create_new`): nothing is ever overwritten, and the name must be one plain component.
- Names that usually hold secrets (`.env` and variants other than examples, keys, certificates, `id_rsa`…) show a notice. Their content is only shown on the screen and never sent anywhere. Nothing in the editor executes files.
- Monaco is bundled with the app: its workers are files of the app, the page's security policy (`default-src 'self'`) is unchanged and no code or font comes from a network. It is loaded on the first visit to the page.

## In-app updates

AutoDock updates itself with the official Tauri updater, with these limits:

- **Signed updates only.** Every installer of a release is signed in the release workflow with a private key that exists only as repository secrets (`TAURI_SIGNING_PRIVATE_KEY` and its password) and on the maintainer's computer. The matching **public key is built into the app** (`plugins.updater.pubkey` in `tauri.conf.json`). The updater verifies the signature of the downloaded file against it and refuses anything else, so a changed file, a compromised download host or a wrong `latest.json` cannot install code.
- **One endpoint, HTTPS only.** The app reads `https://github.com/joaopanucci/autodock/releases/latest/download/latest.json` (fixed in the config; plain HTTP is refused) and downloads the installer named there from the same release.
- **What is sent.** One HTTPS request to GitHub when the app opens (at most once a day, and only while *Check for updates when AutoDock opens* is on, which is the default; it can be turned off in Settings, where *Check now* is also available). No project data, path or identifier is added; GitHub sees the request like any download.
- **Nothing installs by itself.** A new version only shows a notice; the download starts when the user clicks *Update now*. *Later* and *Skip this version* are remembered in `~/.autodock/update.json`, which holds only these choices and the time of the last check.
- **Notes are text.** The release notes in the notice are shown as plain text, never as HTML.
- **The webview cannot call the updater.** The updater plugin's own commands are not granted in the capabilities; the interface only calls AutoDock's commands (`update_auto_check`, `update_check_now`, `update_install`, `update_restart` and three for the preferences), which use the fixed endpoint and public key.
- **Release side.** The *Build installers* workflow fails the release when an installer or its signature is missing, and generates `latest.json` with `scripts/make-latest-json.mjs` (tested in CI).
