🌐 **English** · [Português (Brasil)](SECURITY.pt-BR.md)

# Security policy

## How to report a vulnerability

**Do not open a public issue.** Use GitHub's [private vulnerability reporting](https://github.com/joaopanucci/autodock/security/advisories/new). Please include:

- the AutoDock version and your operating system;
- the steps to reproduce and the impact you see;
- if possible, a suggested fix.

We reply as soon as we can and agree on disclosure after the fix.

## Scope

AutoDock generates Docker files, runs framework installers inside containers and opens editors on your computer. Especially relevant are:

- overwriting or writing outside the chosen folder (path traversal, symlinks);
- injection into a Dockerfile, YAML or command line from user input or project files;
- execution of code from analyzed projects;
- reading sensitive files (`.env`, keys);
- exposing services outside `127.0.0.1`.

The project's security principles are in [`docs/SECURITY.md`](docs/SECURITY.md).

## Supported versions

Only the latest version published in [Releases](https://github.com/joaopanucci/autodock/releases) receives fixes.

## About the installers

The installers are **not code-signed yet** (Apple/Microsoft). Always check the `SHA256SUMS.txt` of each release and download only from the official repository. The one-command install scripts (`scripts/install.sh`, `scripts/install.ps1`) verify the checksum for you.
