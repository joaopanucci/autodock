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
