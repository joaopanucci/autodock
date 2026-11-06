# AutoDock

Etapa 004.3: workspace desktop com análise local, diagnóstico, inventário/monitoramento Docker e consultas controladas de logs, compartilhando o core Rust com a CLI. Tauri 2 + React/TypeScript. Licença Apache-2.0.

A interface oferece Visão geral, Projetos, Ambiente, Containers, Monitoramento, Console e Configurações. As operações Docker são somente leitura. No Console, a aba **Logs de containers** permite carregar a lista, selecionar um container e consultar as últimas 100, 300 ou 500 linhas. Logs ficam apenas na memória, separados do histórico de até 20 consultas do Console. Veja o [relatório atual da Etapa 004.3](docs/ETAPA-004-3.md), incluindo testes, arquivos alterados e limites.

O scanner lê manifests e não executa scripts. O **Assistente Docker** gera `Dockerfile`, `docker-compose.yml`, `.dockerignore` e `.env.docker.example` para projetos existentes (Node.js, Java/Spring Boot com Maven, Python e PHP/Laravel), com serviços opcionais (PostgreSQL, MySQL, MongoDB, Redis, RabbitMQ, Adminer e MailHog). Tudo passa por preview: nada é gravado sem confirmação e arquivos existentes nunca são sobrescritos. Também disponível na CLI: `autodock generate <pasta> --stack nodejs --port 3000 --service postgres` (preview; use `--write` para gravar).

## Pré-requisitos

- Rust stable e Cargo; Windows usa toolchain MSVC.
- Node.js e npm para o frontend (verificado com Node 24.20.0).
- Desktop Windows: ferramentas C++, Windows SDK e WebView2. macOS: ferramentas Xcode e SDK do sistema. Consulte os [pré-requisitos oficiais do Tauri 2](https://v2.tauri.app/start/prerequisites/).
- Docker Engine e `docker compose` são diagnosticados, mas não são necessários para analisar projetos. O produto visa Compose v2; o ambiente desta entrega possui Compose v5.6.1, somente consultado.

## Bootstrap Windows

Dentro desta distribuição completa (o script não é autônomo):

```powershell
cd C:\projetos\autodock
pwsh -NoProfile -File .\bootstrap-autodock.ps1
```

A estrutura é preparada antes das verificações. `bootstrap-files.json` lista os fontes distribuídos. Por padrão preserva o próprio checkout; `-Destination <pasta>` permite preparar outra cópia quando solicitado. Arquivos existentes diferentes são preservados e reportados. Não instala ferramentas nem executa Git. Não copia caches, builds ou node_modules. Cada execução gera JSON e Markdown em `.autodock-bootstrap/<execução>/`, inclusive com dependências faltantes. Prazo padrão de 8 segundos por check; alterável com `-TimeoutMs`.

Saída: 0 = checks disponíveis, 1 = erro na preparação, 2 = estrutura preparada com pendências. Isso não certifica um build Tauri.

## CLI

```powershell
cargo run -p autodock-cli -- scan ./fixtures/node-basic
cargo run -p autodock-cli -- scan ./fixtures/spring-boot-basic --json
cargo run -p autodock-cli -- doctor --json
```

O binário se chama `autodock`: `autodock scan <caminho> [--json]` e `autodock doctor [--json]`. Scan retorna 0 para relatório concluído (avisos podem indicar análise parcial) e 1 para entrada inválida. Em JSON, erros são objetos `{"error":"..."}`. Doctor retorna 2 quando há qualquer check indisponível, inclusive ferramentas opcionais para scan; isso não impede a análise. Estados: `available`, `missing`, `timeout`, `execution_error`. `missing` significa não resolvido nas buscas realizadas, não prova ausência da instalação.

## Desktop

```powershell
npm --prefix apps/desktop ci
npm --prefix apps/desktop run tauri dev
```

Selecionar pasta → analisar → consultar tecnologias, evidências e avisos. O diagnóstico aparece em tabela. Operações Rust rodam fora da thread da interface. O seletor usa o [plugin oficial Dialog](https://v2.tauri.app/plugin/dialog/) com `dialog:allow-open`.

```powershell
npm --prefix apps/desktop run build       # TypeScript + bundle web
npm --prefix apps/desktop run tauri build # aplicativo nativo, requer Rust/SDK
```

O bundle web isolado não possui comandos Tauri ou seletor nativo: o fluxo completo precisa do desktop. Instaladores estão desativados nesta etapa.

## Validação

```powershell
cargo test -p autodock-core -p autodock-cli
cargo fmt --all -- --check
cargo check -p autodock-desktop
pwsh -NoProfile -File tests/Diagnostics.Tests.ps1
pwsh -NoProfile -File tests/Bootstrap.Tests.ps1
npm --prefix apps/desktop run build
npm --prefix apps/desktop test
npm --prefix apps/desktop run test:ui
```

Os testes PowerShell exigem Windows e deixam artefatos em `.autodock-bootstrap`. Testes de modelo frontend usam Node 22.18+; UI usa Playwright com Edge instalado (canal configurável por AUTODOCK_BROWSER_CHANNEL). Veja o [relatório atual](docs/ETAPA-002-3.md) e o [histórico da Etapa 001](docs/ETAPA-001.md). Rust, build nativo e integração WebView2 foram validados no Windows na Etapa 002.3; macOS ainda requer validação.

## Organização

- `crates/autodock-core`: scanner e doctor independentes de interface.
- `crates/autodock-cli`: parsing Clap e apresentação.
- `apps/desktop`: React/Vite e adaptador Tauri.
- `fixtures`: manifests pequenos e inertes.
- `scripts` e `tests`: infraestrutura e regressões do bootstrap.
- `docs`: arquitetura, investigação, limites e entrega.
