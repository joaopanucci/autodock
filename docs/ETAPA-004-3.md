# AutoDock — Etapa 004.3: logs e atualização dos ícones

Entrega: 05/11/2026, America/Sao_Paulo. Trabalho incremental em C:\projetos\autodock.

## Auditoria e linha de base

Foram inspecionados core, frontend, handlers Tauri, recursos de ícones, Console 004.2.3, Containers, Monitoring e testes antes das alterações.

- Core: 40 testes aprovados antes das alterações.
- Frontend: 16 testes unitários e typecheck aprovados.
- Interface: 4 cenários aprovados e 1 falha preexistente. O teste ainda esperava Containers/Monitoramento/Console com “Em breve”, embora já estivessem implementados. Foi atualizado para o comportamento atual e ampliado.
- Console já era mantido montado durante navegação e limitava histórico a 20. Isso foi preservado.
- Containers podia duplicar consultas sob StrictMode e atualizar estado após desmontagem. Foram adicionadas trava por referência e proteção de montagem.
- Parsers Rust de listagem/monitoramento aceitavam objetos sem identidade válida; agora rejeitam formato inadequado e IDs duplicados.
- Monitoring já possuía timer com cleanup e trava de consulta; preservado. O estado de erro inicial deixou de chamar dados inexistentes de “Dados anteriores”.
- O executor de diagnóstico usa arquivos temporários. Ele não pode capturar logs sob o requisito de não persistência: foi adicionado um caminho restrito de captura em memória, reutilizando a resolução de executáveis, o status e command-group para Jobs Windows/grupos POSIX.

Backup dos arquivos existentes antes de edição/regeneração: `.local-backups/etapa-004-3-20261105-200734/`. Inclui os recursos de ícones e o novo arquivo fonte, sem restaurar qualquer versão anterior sobre ele.

## Identidade visual

Fonte oficial: `apps/desktop/src-tauri/icons/app-icon.png`, 1264×1264, PNG íntegro, RGBA com todos os pixels opacos. O fundo original foi preservado. SHA-256 antes e depois:

```text
83E0DA6ABE46B4B18965B606E9B93CC9E21D89AAB98A578F3383506861986724
```

Geração executada pelo [mecanismo oficial Tauri 2](https://v2.tauri.app/develop/icons/):

```powershell
npm.cmd --prefix apps/desktop run tauri -- icon src-tauri/icons/app-icon.png --output src-tauri/icons
```

Foram regenerados PNGs, ICO, ICNS, recursos Appx e os recursos móveis que o gerador oficial emite por padrão. Não foi habilitado nenhum target ou instalador adicional. `bundle.icon` agora explicita 32x32, 128x128, 128x128@2x, ICO e ICNS. Sidebar continua importando o PNG derivado existente; o favicon passou a usar o PNG de 32 pixels. A imagem fonte não foi alterada.

Validação: 49 PNGs (incluindo fonte) decodificados e dimensões nominais conferidas; estrutura ICO e assinatura ICNS verificadas. O ícone extraído do executável Windows foi comparado pixel a pixel ao ICO gerado, com resultado igual. A identidade foi vista no frontend nativo WebView2. Renderização/empacotamento macOS e Linux não executados neste host.

## Funcionalidade de logs

Console → **Logs de containers** → **Carregar containers** → selecionar → escolher 100/300/500 → **Consultar logs**. Há atualização manual, cópia explícita e limpeza do resultado.

- Seleção utiliza IDs retornados pela listagem, apresentando nome, estado observado e ID completo.
- Novo handler Tauri `query_container_logs({ request: { containerId, tail } })` chama Rust em spawn_blocking. Todos os handlers existentes foram mantidos.
- LogsRequest rejeita campos extras; valida ID hexadecimal com 12–64 caracteres e apenas três opções de tail antes de executar qualquer processo.
- Única operação possível nesse handler: `docker logs --tail N -- ID`. Não recebe executável, comando, shell, flags extras, caminhos, nomes de container ou follow.
- Prazo: 15 s para processo e captura, mais até 2 s de tentativa de encerramento/limpeza. stdout/stderr são drenados concorrentemente.
- Cada fluxo retém até 128 KiB de bytes e até N linhas. Ao exceder, encerra o grupo, retorna truncation explícita e invalida sucesso. O conteúdo parcial é o prefixo capturado da consulta solicitada, não uma resposta completa.
- Buffers e resultado apenas em memória; não usa tempfile, arquivos, logging, relatórios de diagnóstico ou histórico do Console para logs. A captura não imprime conteúdo. A ação Copiar escreve na área de transferência por solicitação do usuário; Limpar descarta o resultado da UI.
- stdout/stderr aparecem separadamente como texto escapado pelo React. Controles ANSI/terminal são omitidos também na cópia. TTY pode reunir os fluxos em stdout; ordem relativa entre fluxos não é reconstruída.
- Logs são uma consulta pontual conforme a [documentação Docker](https://docs.docker.com/reference/cli/docker/container/logs/). Nenhum container foi criado, interrompido, reiniciado, removido ou modificado.

## Consolidação do Console

As três operações existentes foram preservadas: listagem, informações do Engine e versão Docker. O histórico continua em memória, limitado a 20, sem novas consultas causadas por navegação. Logs têm somente o último resultado, separado desse histórico.

Visualizações Formatada/Bruta permanecem disponíveis conforme o tipo de saída. JSON inválido, resposta truncada ou consulta sem sucesso não se apresentam como tabela completa. O campo aditivo `truncated` torna essa condição explícita no contrato, mantendo os campos e nomes existentes. stderr e detalhes não ficam ocultos pela tabela formatada. Campos vazios, IDs repetidos e tipos inadequados são tratados pelo parser. Estado de carregamento é anunciado e as ações em andamento são protegidas contra duplicação.

## Arquivos criados e modificados

Criados:

- `crates/autodock-core/src/logs.rs`: contrato, validação, operação e testes de logs.
- `crates/autodock-core/src/memory_capture.rs`: captura sensível limitada e concorrente, sem disco.
- `apps/desktop/src/pages/ContainerLogs.tsx`: interface de consulta.
- `apps/desktop/src/logs-format.ts` e `tests/logs-format.test.mjs`: texto seguro e cópia.
- `apps/desktop/tests/ui/console-logs.spec.ts`: regressões Console/logs.
- `scripts/verify-icons.ps1`: verificação dos recursos e comparação do ícone Windows.
- Este relatório.

Modificados:

- Core: `src/lib.rs`, `doctor.rs`, `console.rs`, `docker.rs`, `monitoring.rs`; formatação aplicada.
- Tauri: `src-tauri/src/lib.rs`, `tauri.conf.json` e recursos derivados em `icons/`.
- Frontend: `pages/Console.tsx`, `Containers.tsx`, `Monitoring.tsx`, `console-format.ts`, `types.ts`, `style.css`, `index.html`.
- Testes: `tests/console-format.test.mjs`, `tests/ui/workspace.spec.ts`.
- `README.md` e `bootstrap-files.json`; manifesto atualizado para distribuir recursos/arquivos novos e também os módulos Console/Docker/Monitoring já existentes.

Nenhuma dependência nova de produção, crate ou plugin. Código do desenvolvedor preservado; não houve reescrita do projeto. Evidências de testes, ícones e smoke local ficam em `.cache/etapa-004-3/`, sem conteúdo dos logs reais.

## Testes e execução

| Verificação | Resultado |
| --- | --- |
| cargo test -p autodock-core | 51 aprovados (26 unitários + 25 integrações) |
| cargo fmt --all -- --check | Aprovado |
| cargo check --workspace | Aprovado |
| npm.cmd --prefix apps/desktop run typecheck | Aprovado |
| npm.cmd --prefix apps/desktop run test | 19 aprovados |
| npm.cmd --prefix apps/desktop run build | Aprovado |
| npm.cmd --prefix apps/desktop run test:ui | 11 cenários aprovados em Edge |
| tauri build --debug --no-bundle | Executável Windows gerado |
| scripts/verify-icons.ps1 -CheckExecutable | 49 PNGs, ICO/ICNS e correspondência do ícone Windows aprovados |
| Smoke na aplicação Tauri/WebView2 real | Lista com 9 containers; consulta de logs concluída e renderizada, sem mock IPC |

Testes novos cobrem ID/tail inválidos, campos de operações não autorizadas, streams simultâneos, vazio, não-zero, falha ao iniciar, timeout com saída parcial, encerramento real de descendente, limite exato, excesso de bytes/linhas, JSON truncado e formas inválidas. Testes UI cobrem as três opções de linhas, seleção, cópia, limpeza, estados de falha, navegação sem duplicação, histórico de 20, layout estreito e acessibilidade dos seletores.

Durante desenvolvimento, os testes UI identificaram rótulos de seletor que incluíam as opções no texto de label; corrigidos separando label e campo. Um seletor de teste foi ajustado para distinguir o Console preservado mas oculto da listagem visível. Suite final aprovada.

O smoke nativo executou consulta real somente leitura. Nenhum texto de log foi escrito no relatório ou no terminal. O resultado foi limpo antes da screenshot `.cache/etapa-004-3/native-logs-cleared.png`. `native-result.json` contém somente metadados de validação. Processo de teste e porta de depuração loopback foram encerrados; configuração permanente de depuração não foi adicionada.

## Limitações e próxima etapa

- Builds/execução macOS e Linux pendentes; recursos foram gerados, mas não certificados nesses sistemas.
- Disponibilidade de logs depende do driver, permissões, contexto Docker e existência do container no momento da consulta. O estado exibido é o da última listagem.
- Um resultado truncado/timeout é parcial. Não há follow, busca, paginação, exportação para arquivo ou cancelamento manual nesta etapa.
- Não há apagamento criptográfico de memória nem controle sobre swap/crash dumps do sistema; a aplicação não persiste logs deliberadamente. A área de transferência pode ser gerenciada pelo sistema após a cópia autorizada.
- Em falha excepcional de encerramento do grupo, o resultado informa o problema; a espera de limpeza é limitada. O caminho de diagnóstico antigo continua usando temporários para suas consultas não relacionadas a logs; os logs nunca passam por ele.
- O ícone tem fundo opaco e texto, conforme o original; a legibilidade em tamanhos mínimos depende desse desenho. Nenhuma identidade substituta foi inventada.

Próximo passo recomendado: validar UI/ícones e processos em macOS/Linux e ampliar regressões de drivers Docker antes de considerar acompanhamento contínuo ou novas operações.

Para iniciar:

```powershell
cd C:\projetos\autodock
npm.cmd --prefix apps/desktop run tauri dev
```

Git ainda não inicializado (status: not a git repository). Sem commit, push, merge, PR ou deploy. Não foram alterados arquivos de outros projetos nem containers existentes.
