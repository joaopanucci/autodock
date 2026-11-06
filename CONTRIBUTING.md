# Contribuindo

Mantenha a Etapa 001 pequena: scan somente leitura e diagnóstico. Core não pode depender de Tauri. Não execute scripts ou builds dos projetos inspecionados.

Execute cargo fmt --all -- --check, cargo test -p autodock-core -p autodock-cli, cargo check -p autodock-desktop e npm --prefix apps/desktop run build. No Windows, execute também tests/Diagnostics.Tests.ps1 e tests/Bootstrap.Tests.ps1 com PowerShell 7.

Registre checks bloqueados sem declarar sucesso. Valide em macOS antes de afirmar suporte testado. Gere e versione Cargo.lock quando Cargo estiver disponível. Ao adicionar fontes distribuídos, atualize bootstrap-files.json; exclua caches/builds/relatórios. Nunca inclua segredos.

Contribuições sob Apache-2.0; veja LICENSE. Geração de Dockerfiles/Compose e gerenciamento de containers exigem outra etapa aprovada.
