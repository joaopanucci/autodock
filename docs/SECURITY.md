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
