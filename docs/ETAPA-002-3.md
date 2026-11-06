# Etapa 002.3 — Interface e diagnóstico por evidências

Entrega de 05/11/2026, America/Sao_Paulo. Alterações restritas ao AutoDock; sem commit, push, deploy ou operação mutável Docker.

## Auditoria anterior às alterações

- Frontend: React 19/TypeScript/Vite 6, com um App no main.tsx e CSS único. Não existiam testes frontend.
- Identidade existente: logo AutoDock em src-tauri/icons, preservado e reutilizado.
- IPC: `scan_project(path)` e `doctor_environment`, ambos mantidos com os mesmos nomes e argumentos.
- Contratos: ScanReport e DoctorReport em camelCase, schemaVersion 1. CLI Clap independente; core sem Tauri.
- Scanner: Node/Maven/Spring Boot, limites de leitura/travessia, exclusões incluindo .local-backups. Os 17 testes Rust existentes passaram antes da implementação.
- Cargo/Rust e Cargo.lock já estavam disponíveis, diferentemente da entrega histórica da Etapa 001.
- Backup dos principais arquivos a substituir: `.local-backups/etapa-002-3-20261105-150504/`.

## Implementação

Interface grafite/azul com sidebar, navegação contextual, cards, ícones Lucide, estados vazios, carregamento, erro, sucesso e avisos. Layout responsivo, foco visível, skip link, navegação por teclado, mensagens anunciadas e respeito a prefers-reduced-motion. A janela padrão passou a 1280×850, mínimo 760×600; layout estreito também foi verificado no navegador.

- **Visão geral:** totais da sessão, módulos agrupados por diretório, arquivos Docker encontrados e checks de ferramentas. Sem séries históricas ou métricas simuladas.
- **Projetos:** seletor nativo preservado, caminho editável, scan, evidências por módulo, declarações de versões, frameworks, dependências e avisos. Reanálises substituem a entrada correspondente na sessão.
- **Ambiente:** respostas reais do doctor, versões/saídas, detalhes técnicos expansíveis, recomendações que distinguem erro, timeout e não encontrado. Requisitos gerais locais/Docker separados de compatibilidade.
- **Configurações:** descrição do comportamento atual e política futura de permissões; sem controles fictícios ou persistência.
- **Containers, Monitoramento e Console:** navegação funcional, conteúdo explicitamente “Em desenvolvimento”. Nenhuma ação Docker implementada.

Não há chamadas IPC automáticas ao iniciar. Diagnóstico é solicitado pelo usuário. Memória da sessão não é salva em localStorage ou banco de dados.

## Contrato e arquitetura

O core mantém a travessia existente e acrescenta `findings` ao ScanReport. Campos anteriores e schemaVersion 1 foram mantidos. O frontend aceita relatórios antigos sem findings (`findings?`). CLI texto continua exibindo tecnologias; CLI JSON recebe o campo aditivo. Nenhum comando Tauri ou permissão IPC foi ampliado.

Exemplo de evidência:

```json
{
  "module": "web",
  "kind": "runtime",
  "name": "node",
  "value": ">=22",
  "source": "web/package.json → engines.node",
  "status": "declared"
}
```

`findings.rs` extrai dados já lidos e parseados pelo scanner. Não executa código. `kind` distingue runtime, package_manager, framework, build_tool, dependency, parent e container_config. `status` distingue declared/detected; instalação é proveniente exclusivamente do doctor.

- package.json: engines, packageManager e dependências diretas, dev, peer e opcionais. Uma lista explícita identifica nomes de frameworks/build tools. Declaração de pacote não é prova de uso efetivo ou instalação.
- pom.xml: parent/dependências diretas e propriedades Java/compiler. Mantém expressões e versões literais; não resolve herança, perfis, propriedades ou repositórios remotos.
- Dockerfile e nomes convencionais Compose: detecta somente arquivo regular presente, sem parse YAML ou inferência de serviços, portas ou imagens. Arquivo inválido pode ser reportado como presente; a UI informa que validade não foi verificada.
- Diretórios ignorados preservados; `.autodock-bootstrap` também foi excluído para não analisar cópias e relatórios internos.

`model.ts` contém apresentação, agrupamento, identidade de projetos e comparação conservadora de versão Node. Apenas engines.node canônico x.y.z é comparado com stdout Node canônico de um check available. Divergência confirma somente que a instalação não satisfaz aquela declaração exata; não afirma que o projeto será incapaz de executar. Ranges, prereleases, versões inválidas e outros runtimes ficam não verificados. Essa comparação de apresentação não modifica o contrato ou o comportamento CLI.

Uma futura comparação completa deve ficar no core com regras SemVer/Maven bem definidas, resultado estruturado e fontes de evidência, mantendo findings e os campos atuais durante a migração. Não incluir instalador ou executor no scanner.

## Arquivos

Criados:

- `crates/autodock-core/src/findings.rs` e `tests/findings.rs`.
- `apps/desktop/src/App.tsx`, `types.ts`, `model.ts`, `vite-env.d.ts`.
- `apps/desktop/src/components/Layout.tsx` e `ui.tsx`.
- `apps/desktop/src/pages/Overview.tsx`, `Projects.tsx`, `Environment.tsx`.
- `apps/desktop/tests/model.test.mjs`, `tests/ui/workspace.spec.ts` e `playwright.config.ts`.
- Este relatório.

Modificados:

- Core: `src/lib.rs`, `src/scan.rs`, `tests/scan.rs`; rustfmt aplicado ao workspace.
- Frontend: `src/main.tsx`, `src/style.css`, `package.json`, `package-lock.json`.
- Tauri: somente dimensões em `tauri.conf.json`; handlers e capabilities mantidos.
- `README.md`, `docs/ARCHITECTURE.md`, `.gitignore`, `bootstrap-files.json`.

Dependências adicionadas: lucide-react (ícones) e @playwright/test (somente desenvolvimento, regressões de fluxo/layout). Testes de modelo usam node:test, sem framework adicional; requerem Node com suporte nativo TypeScript (22.18+ ou Node 24). Nenhuma dependência Rust nova. Prettier foi usado pontualmente para formatar arquivos, sem dependência de runtime.

## Validação executada

| Verificação | Resultado |
| --- | --- |
| Baseline cargo test --workspace | 17 testes existentes aprovados |
| cargo fmt --all -- --check | Aprovado |
| cargo test --workspace após alterações | 23 aprovados: 11 scan, 6 doctor, 6 findings |
| cargo check --workspace | Aprovado |
| npm --prefix apps/desktop run typecheck | Aprovado |
| npm --prefix apps/desktop run build | Aprovado |
| npm --prefix apps/desktop test | 10 testes de modelo aprovados |
| npm --prefix apps/desktop run test:ui | 5 cenários Edge/Playwright aprovados |
| npm --prefix apps/desktop run tauri build -- --debug --no-bundle | Binário nativo gerado |
| Smoke test no WebView2 nativo | Scan real da fixture Spring Boot, agrupamento em um módulo e doctor real com 9 checks; sem mock IPC |
| CLI scan apps/desktop --json | Executado; campos antigos preservados e findings reais de React/Vite/TypeScript |

Os testes Playwright usam fixtures IPC **somente no processo de teste**, para cobrir cancelamento, falha, timeout e incompatibilidade de maneira determinística. Dados de teste não são carregados pelo aplicativo. Também foram conferidos layout de 390 px, navegação por teclado e preferência de redução de movimento. Um foco inicial indevido encontrado durante o teste foi corrigido e a suíte passou novamente.

O smoke nativo usou um WebView2 isolado e porta de depuração loopback temporária, somente no processo de teste; processo encerrado ao terminar. Não alterou tauri.conf para habilitar depuração permanente. Evidência: `.cache/native-smoke-result.json`; screenshots reais `.cache/etapa-002-3-native.png` e `.cache/etapa-002-3-native-environment.png`. Screenshot da visão geral vazia: `.cache/etapa-002-3-overview.png`. Screenshot de Projetos do teste browser contém fixture, não é dado de usuário.

## Limites e riscos conhecidos

- macOS não executado nesta máquina; código e testes usam paths portáveis, com normalização Windows na apresentação. Os testes Unix específicos de links não rodaram no Windows.
- Seletor de pasta preserva o plugin oficial; cancelamento/seleção foram automatizados via fixture. No smoke nativo o caminho foi digitado, sem automação do diálogo do sistema.
- Não resolve todo SemVer, instalações de dependências do projeto, propriedades Maven, validity Compose/Dockerfile ou requisitos internos de containers. Java/Maven não são sondados pelo doctor atual: explicitamente não verificados.
- Não é sandbox contra trocas hostis concorrentes no filesystem. Limites anteriores de 1 MiB, profundidade e quantidade de diretórios permanecem.
- Dados da sessão são perdidos ao fechar. Lista de dependências é apresentada sob detalhes; manifests muito extensos podem gerar listas longas.
- A dependência preexistente command-group foi mantida; migração fica para tarefa própria.
- Nenhum gerenciador de operações/backup automático do produto foi implementado. A política de confirmação, comandos previstos, arquivos afetados, backups e logs sem segredos está documentada para operações futuras.

## Executar

```powershell
cd C:\projetos\autodock
npm --prefix apps/desktop run tauri dev
```

Em checkout novo, executar `npm --prefix apps/desktop ci` primeiro. Rust/Cargo e pré-requisitos Tauri são necessários. Para testes UI, Edge é o canal padrão usado aqui; pode-se definir AUTODOCK_BROWSER_CHANNEL para outro canal Playwright disponível. Não há instalação automática de navegador.

## Próximos passos

Validar macOS e diálogo nativo manualmente; ampliar contratos de comparação no core com testes de SemVer e Maven; projetar operações autorizadas com preview/backup/auditoria antes de implementar geração ou orquestração Docker.

Git ainda não inicializado: git status retorna not a git repository. Nenhum commit, push, PR ou deploy realizado. Dakiali e outros projetos não foram alterados.
