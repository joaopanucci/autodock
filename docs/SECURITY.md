# Security

Never execute scripts from scanned projects during detection. Do not read `.env` or secret files. Never overwrite generated files without review and consent. Never delete unrelated Docker resources.

## Generated secrets and ports

- Database and broker passwords are 32 alphanumeric characters derived from a seed taken from the operating system CSPRNG (`getrandom`). They are written only to `.env.docker`, created exclusively (never overwritten) and with mode `0600` on Unix. They never appear in `docker-compose.yml` or in the example file.
- The seed is returned with the preview and sent back on apply, so the written files are identical to what the user reviewed. A tampered seed or port list is rejected.
- Host ports are checked by binding them locally and against a registry of ports reserved by other projects (`~/.autodock/ports.json`, overridable with `AUTODOCK_HOME`). Service ports stay published on `127.0.0.1` only.
