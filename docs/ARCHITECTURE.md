# Arquitetura da Etapa 001

Cargo Workspace com core, CLI e desktop. Os membros padrão são core e CLI, para separar a compilação das bibliotecas da interface. O core não depende de Tauri.

CLI → `scan(Path)` / `doctor()` → relatórios serializáveis.
React → `invoke` → comandos Tauri assíncronos → `spawn_blocking` → mesmo core.

`ScanReport`: schemaVersion, project, components, warnings e o campo aditivo findings (Etapa 002.3). Componentes: path relativo, technology e evidence. Findings preservam módulo, tipo, nome, valor literal, fonte e estado declarado/detectado. Entrada inválida retorna Result; falhas em manifests/subdiretórios geram avisos de análise parcial.

## Detecção

Travessia determinística de até 10000 diretórios, profundidade 32 e 1 MiB por manifesto. Exclui node_modules, target, dist, build, .git, .next, .venv e vendor. Não segue links internos de diretórios ou manifests. A raiz escolhida é canonicalizada. Não lê .env nem executa npm/Maven ou acessa rede durante scan.

- Node.js: package.json válido com objeto JSON.
- Maven: pom.xml válido com raiz project.
- Spring Boot: parent direto org.springframework.boot:spring-boot-starter-parent ou dependência direta Boot/autoconfigure/starter desse grupo. Comentários, dependencyManagement e perfis não ativados não bastam. Propriedades Maven e herança remota não são resolvidas nesta etapa.

Resultados vazios são válidos. O scanner não é sandbox contra trocas concorrentes hostis de arquivos; checar links não elimina essas corridas.

## Diagnóstico

Versões de Rust/Cargo/Node/npm/Git/Docker/Compose e ferramentas de plataforma. Docker Engine é separado: `docker info --format {{.ServerVersion}}`. Windows procura Rust também em CARGO_HOME/.cargo e MSBuild via vswhere. macOS verifica xcode-select. Não altera containers.

Core usa arquivos temporários para evitar bloqueio de pipes; grupos POSIX/Jobs Windows permitem cancelar processos. Prazo de 8 segundos por check, mais até 2 segundos de limpeza. Captura até 64 KiB por stream; interrompe saída acima de 2 MiB durante polling. Isso não é quota rígida de disco. Doctor pressupõe ferramentas instaladas confiáveis.

PowerShell drena ambos os streams assincronamente antes de esperar. O deadline inclui EOF; Job Object e Kill(entireProcessTree) encerram os filhos. A associação ao Job ocorre logo após Start, sem isolamento garantido contra executáveis hostis que criem filhos nessa pequena janela. Saída é decodificada como UTF-8; ferramentas legadas podem exibir caracteres substituídos.

## Dependências

| Dependência | Motivo |
| --- | --- |
| serde / serde_json | Relatórios e JSON Node |
| roxmltree | XML estruturado, sem busca por substring |
| which | Resolução de executáveis |
| tempfile | Captura sem pipes e fixtures isoladas |
| command-group 5 | Grupos de processos e cancelamento |
| Clap | Parsing e ajuda CLI |
| Tauri 2 / dialog | Janela, IPC e seletor |
| React / TypeScript / Vite | Interface e build |

O upstream de command-group recomenda process-wrap como sucessor. A API pequena existente foi usada nesta fundação; avaliar migração após validar a suíte Rust. Referência: https://github.com/watchexec/command-group.

package-lock.json fixa o frontend. Cargo.lock já está presente; Rust e Cargo foram disponibilizados após a Etapa 001. Na Etapa 002.3, testes/check do workspace, build nativo e smoke WebView2 Windows passaram. Compatibilidade macOS ainda requer execução no sistema alvo.

## Interface da Etapa 002.3

App coordena estado da sessão e chama os mesmos dois comandos Tauri. Layout/ui oferecem componentes visuais reutilizáveis; páginas Overview, Projects e Environment apresentam dados reais. model.ts centraliza agrupamento, apresentação e comparação conservadora de Node exato. Navegação não exige roteador, e não há persistência. Lucide fornece ícones; Playwright é dependência exclusiva de desenvolvimento. Regras, contrato aditivo e proposta de evolução estão em ETAPA-002-3.md.
