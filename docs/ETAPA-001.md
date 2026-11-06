# Entrega técnica — Etapa 001

Data: 05/11/2026, America/Sao_Paulo. Diretório: C:\projetos\autodock.

## Resultado

Fundação implementada no código: Cargo Workspace, scanner Rust independente, CLI Clap e desktop Tauri 2/React/TypeScript. Frontend e bootstrap foram executados e validados. Compilação/testes Rust e execução nativa Tauri estão bloqueados porque Rust/Cargo não foram encontrados. Não é uma entrega desktop certificada ou release.

## Árvore de fontes

```text
autodock/
├── Cargo.toml
├── package.json
├── bootstrap-autodock.ps1
├── bootstrap-files.json
├── crates/
│   ├── autodock-core/
│   │   ├── Cargo.toml
│   │   ├── src/{lib,scan,doctor}.rs
│   │   └── tests/{scan,doctor}.rs
│   └── autodock-cli/
│       ├── Cargo.toml
│       └── src/main.rs
├── apps/desktop/
│   ├── package.json, package-lock.json
│   ├── index.html, tsconfig.json, vite.config.ts
│   ├── src/{main.tsx,style.css}
│   └── src-tauri/
│       ├── Cargo.toml, build.rs, tauri.conf.json
│       ├── src/{main,lib}.rs
│       ├── capabilities/default.json
│       └── icons/icon.png
├── fixtures/{node-basic,maven-basic,spring-boot-basic}/
├── scripts/Diagnostics.ps1
├── tests/{Diagnostics.Tests,Bootstrap.Tests}.ps1
├── docs/{ARCHITECTURE,BOOTSTRAP,SECURITY,ROADMAP,ETAPA-001}.md
├── README.md
├── CONTRIBUTING.md
├── LICENSE
└── .gitignore
```

Caches, node_modules, dist e relatórios de execução permanecem dentro do AutoDock e são ignorados pelo Git. Cargo.lock ainda não foi gerado.

## Arquivos e responsabilidades

| Arquivo | Responsabilidade |
| --- | --- |
| crates/autodock-core/src/scan.rs | Travessia limitada, JSON/XML, evidências e avisos |
| crates/autodock-core/src/doctor.rs | Resolução, execução limitada, quatro estados e relatórios |
| crates/autodock-cli/src/main.rs | scan/doctor, texto/JSON e exit codes |
| apps/desktop/src-tauri/src/lib.rs | Adaptador IPC, tarefas fora da thread da UI |
| apps/desktop/src/main.tsx | Seleção nativa, análise, avisos/erros e diagnóstico |
| bootstrap-autodock.ps1 | Preparação segura e relatórios por execução |
| scripts/Diagnostics.ps1 | Processo .NET, streams concorrentes, timeout e Job Windows |
| bootstrap-files.json | Lista de distribuição, sem duplicar os fontes em templates |

## Correções principais do bootstrap

Colisão de `$Args` confirmada por reprodução isolada; substituído por `$Arguments`. npm.cmd é resolvido e executado por node/npm-cli.js. Streams são drenados antes de esperar, com timeout incluindo EOF e encerramento de filhos. Estrutura é preparada antes das verificações e sobrevive a pendências. Repetições preservam arquivos e produzem relatórios distintos. MSBuild é procurado fora do PATH com vswhere. Investigação completa em BOOTSTRAP.md.

## Comandos

Na raiz do AutoDock:

```powershell
pwsh -NoProfile -File bootstrap-autodock.ps1
cargo run -p autodock-cli -- scan ./fixtures/node-basic --json
cargo run -p autodock-cli -- doctor --json
npm --prefix apps/desktop ci
npm --prefix apps/desktop run tauri dev
```

Cargo requer disponibilizar Rust antes. O bootstrap não instala ferramentas. Dependências npm foram instaladas somente no projeto para validar o frontend.

## Validações reais

| Verificação | Resultado |
| --- | --- |
| Diagnostics.Tests.ps1 | 10 verificações aprovadas |
| Bootstrap.Tests.ps1 | 3 verificações aprovadas |
| npm run build no frontend | Aprovado: tsc --noEmit e Vite, 31 módulos |
| Bootstrap no próprio workspace | Executado, relatórios gerados; código 2 por Rust/Cargo |
| Bootstrap em destino de teste interno e repetido | Aprovado; edição preservada, dois relatórios e conflito registrado |
| Testes Rust | Não executados: Cargo não resolvido |
| cargo fmt / cargo check / build Tauri | Não executados: ferramenta Rust ausente nas buscas |
| Execução da CLI compilada / janela nativa | Não executada |
| macOS | Não executado; sem host macOS |

Os testes Rust foram escritos: 10 de scan no Windows, mais 1 de symlinks em Unix, e 6 de doctor. Cobrem Node, Maven, Spring parent/dependency, falsos positivos, vazio, caminho inválido, serialização, recursão/exclusões, manifest inválido/grande, quatro estados, captura e timeout. Não são declarados aprovados.

A suíte PowerShell inclui buffer grande nos dois streams, não-zero, ausência de resolução, falha de inicialização, timeout, filho real encerrado e argumentos com espaços/Unicode UTF-8. O teste Unicode revelou diferenças de codepage, inclusive quando executado por outro processo: emissão dos scripts e decodificação dos streams agora usam UTF-8 explicitamente. Após a correção, a execução aninhada passou nos 10 checks. A limitação de ferramentas legadas está documentada.

Evidência bruta preservada em `.autodock-bootstrap/validation-final/results.json` (inclui uma falha intermediária de Unicode) e `powershell-final.json` (revalidação final aprovada após correção). O frontend passou no primeiro arquivo; Cargo permanece não resolvido. Os 41 arquivos do manifesto foram conferidos e os JSONs foram analisados com sucesso.

## Ambiente encontrado

| Ferramenta | Evidência |
| --- | --- |
| PowerShell | 7.6.9 |
| Node | v24.20.0 |
| npm | 11.19.0 |
| Git | 2.56.0.windows.2 |
| Docker CLI / Engine | 29.9.2 / 29.9.2 |
| Docker Compose | v5.6.1; diferente do alvo v2, apenas consultado |
| MSBuild | 18.9.1.35102, via vswhere |
| C++ Build Tools | Instalação retornada por vswhere com componente C++ |
| Rust / Cargo | Não encontrados no PATH nem .cargo/CARGO_HOME consultados |

Presença de C++/MSBuild não certifica o build Tauri. WebView2/SDK e integração nativa ainda precisam ser validados. Nenhuma instalação global foi feita. Consultas ao Docker foram somente leitura e não alteraram containers do Dakiali.

## Pendências e Git

Antes da Etapa 002: disponibilizar Rust, gerar Cargo.lock, executar testes/formatação/build nativo e validar Windows/macOS. Avaliar o sucessor process-wrap para command-group. Só então definir geração de Dockerfiles/Compose com revisão prévia; nada disso foi implementado nesta etapa.

`git -C C:\projetos\autodock status --short --branch` retornou “not a git repository”. O diretório não foi inicializado como repositório. Não houve commit, push, branch, worktree, PR, merge, deploy ou release. Os dois scripts originais fora do AutoDock foram preservados.
