# MAC.8 — Auditoria de lifecycle e CI nativa

Auditoria realizada em 2026-11-06 na branch `feature/005-container-lifecycle`. Os cinco arquivos solicitados foram revisados, incluindo testes, contratos e chamadas ao executor. Também foram inspecionados `memory_capture.rs`, manifestos do workspace, configuração/build do Tauri e testes de subprocessos existentes. Não havia `.github/` nem workflows preexistentes. Nenhuma configuração de produção, dependência ou contrato público foi alterado nesta etapa.

O ponto de partida continha alterações locais em `Cargo.lock`, manifest do core, `lib.rs`, lifecycle, journal, lock e teste de processos; `lifecycle_fs.rs` e `docs/container-lifecycle-safety.md` ainda não eram rastreados. Essas alterações foram preservadas. Backups anteriores às edições, patch inicial e logs ficam em `.local-backups/mac8-audit-20261106/`, já ignorado pelo Git. O journal e o lock ficaram byte a byte iguais ao início desta etapa; o executor de produção também, com acréscimo apenas de um teste no seu módulo.

## Achados e correções

| ID | Severidade | Evidência e consequência | Tratamento |
| --- | --- | --- | --- |
| A1 | Alta | `lifecycle_fs.rs`, `Directory::child`: sincronizava o pai apenas quando `mkdirat` criava uma entrada. Após interrupção ou erro de sync, uma nova tentativa encontrava EEXIST e podia aceitar uma entrada sem confirmar sua persistência no pai. A sincronização posterior do arquivo/journal não confirma toda a cadeia de ancestrais. | Sincronizar o pai em toda aquisição bem-sucedida, inclusive EEXIST. O descritor recém-aberto passa a ser possuído por `File` antes do sync falível. Regressão com falha injetada falha com o comportamento anterior e passa com a correção. Não é um teste físico de perda de energia. |
| A2 | Alta, específica de Windows; validação nativa pendente | `lifecycle_fs.rs`, abertura de diretórios, e `lifecycle_lock.rs::lock_path_for`: o nome textual do journal define o lock irmão. Um alias 8.3 ou nome normalizado pelo Win32 pode selecionar o mesmo diretório com outro nome de lock, comprometendo a exclusão cooperativa. A análise se baseia nos contratos de nomes/API do Windows; não foi reproduzida em Windows nesta sessão. | Comparar cada componente solicitado com o nome normalizado obtido do handle por `GetFinalPathNameByHandleW`. Rejeitar diferenças, aceitando apenas variação ASCII de caixa; falhas de consulta bloqueiam acesso. Testes Windows para nomes canônicos, caixa, ponto/espaço final e aliases 8.3. Root de drive/UNC é aberto sem tentativa desnecessária de criação. |
| A3 | Média, disponibilidade/confiabilidade da validação | `lifecycle_lock_process.rs`: handshake por leitura bloqueante sem deadline; helper liberava o primeiro lock após três segundos independentemente do término da verificação. Um runner lento podia falhar indevidamente e um helper travado podia prender o teste. O primeiro helper não tinha cleanup RAII em todo caminho de falha. | Deadline de 15 segundos no handshake, detecção de EOF, guard para kill/wait em unwind e retenção do lock via stdin até encerramento explícito. Regressão verifica timeout e saída precoce. As chamadas `.output()` restantes continuam limitadas globalmente pelo timeout do job, não por um timeout individual. |
| A4 | Baixa, lacuna de cobertura | Não havia injeção determinística de falha de sync de diretório no caminho completo do serviço. | Hook exclusivamente `cfg(test, unix)`, local à thread e restaurado por RAII. Teste prova executor com zero chamadas, preservação dos dois nomes após link/sync falho e bloqueio de nova tentativa. |

A semântica dos aliases e da consulta de nome foi conferida na documentação primária: [nomes Win32](https://learn.microsoft.com/en-us/windows/win32/fileio/naming-a-file) e [GetFinalPathNameByHandleW](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-getfinalpathnamebyhandlew). A comparação conservadora pode rejeitar variações Unicode de caixa e ambientes onde a consulta normalizada não é suportada: o efeito é negar a operação, não executar sem lock válido.

Não foi identificado novo problema crítico que exigisse mudança de contrato público ou reescrita arquitetural dentro do modelo declarado de processos cooperativos e armazenamento privado.

## Invariantes verificados

- `LifecycleService::execute` valida confirmação/ID, adquire transação e lista registros antes de inspecionar. Resolve ID completo e valida prefixo antes de reservar. `transaction.record(...)` retorna com sucesso antes de `executor.execute(...)`.
- A reserva usa criação exclusiva, escrita completa, sync de arquivo, publicação por hard link sem sobrescrita, sync de diretório, retirada do temporário, novo sync e releitura. Falhas anteriores à execução não chamam o executor. Temporários, corrupção e hard links ambíguos bloqueiam o journal.
- `LifecycleFileLock` usa aquisição exclusiva não bloqueante e verifica identidade do arquivo. O arquivo de lock permanece no disco. O guard abrange reserva, comando, inspeção e conclusão; reconciliação concorrente não libera uma operação ativa. Não há ciclo de espera por mutex/lock no caminho do serviço: contenção retorna erro.
- O lock global serializa inclusive containers distintos; isso é uma limitação de disponibilidade deliberada. O executor real tem limites de tempo; implementações customizadas de `LifecycleExecutor` são confiáveis e devem obedecer ao contrato e retornar.
- Timeout, saída truncada, exit code não zero, estado inesperado ou inspeção falha mantêm reserva. Sucesso do CLI não basta para `stateVerified`. `Missing` permite retirada somente pelo contrato de que nenhum comando foi iniciado.
- Reconciliação exige reconhecimento explícito e ID completo, verifica contexto, identidade e estado estável, não repete comandos e conserva pendência se inspeção falhar.
- No Unix, operações relativas a descritores e `O_NOFOLLOW` evitam redirecionamento por troca de caminhos; owner/permissões e hard links são verificados. No Windows, diretórios ficam abertos sem compartilhamento de exclusão, reparse points são rejeitados e o nome final agora também é verificado. Isso não transforma locks consultivos em proteção contra um processo malicioso com o mesmo usuário.
- Falha de sync após retirada de um registro pode produzir erro depois de ele já ter desaparecido da visão atual; a retirada só ocorre após estado verificado ou reconhecimento explícito. Após crash, uma retirada não persistida pode ressuscitar pendência conservadoramente. Não há replay automático.

## Workflow

Criado `.github/workflows/rust-ci.yml`, com jobs nativos independentes em `ubuntu-24.04`, `windows-2022` e `macos-15`, `fail-fast: false` e timeout de 30 minutos por job. Eventos: pull requests, pushes em `main`/`feature/005-container-lifecycle` e acionamento manual. Jobs da mesma referência são cancelados quando substituídos.

Todos executam `cargo check --workspace --locked` e `cargo test -p autodock-core --locked`; somente Ubuntu executa `cargo fmt --all -- --check`. O shell Bash é usado em todos os jobs para scripts uniformes, executando binários nativos de cada runner. Rust 1.99.1 é fixado no ambiente do workflow e foi confirmado no manifesto público oficial. Checkout v6.1.0 e cache v4.3.0 estão fixados por SHA conferido nas tags upstream. Token apenas `contents: read`, sem persistência de credenciais.

O cache armazena índice/arquivos baixados do registry e objetos Git; não guarda `target`, credenciais ou registros de lifecycle. A chave inclui OS, arquitetura, Rust e hash exato do lockfile, sem restore keys abrangentes. Cargo continua responsável por resolução locked e verificação dos checksums. A instalação das bibliotecas Linux necessárias ao workspace Tauri ocorre somente no runner; não altera configurações locais/produção. Referências: [pré-requisitos oficiais do Tauri](https://v2.tauri.app/start/prerequisites/) e [cache do GitHub Actions](https://docs.github.com/en/actions/concepts/workflows-and-actions/dependency-caching).

O workflow não inicia Docker, Compose ou o aplicativo desktop e não publica artefatos. A revisão dos testes confirmou uso de executores simulados para lifecycle. A última etapa verifica se a validação alterou arquivos rastreados.

## Validação efetivamente executada

Ambiente: macOS Apple Silicon, rustc 1.99.1.

- `cargo check --workspace --locked`: aprovado para core, CLI e desktop.
- `cargo fmt --all -- --check`: aprovado.
- `cargo test -p autodock-core --locked`: 104 aprovados, zero falhas/ignorados (72 unitários; 8 doctor; 7 findings; 5 processos, incluindo helpers; 12 scan).
- Regressão A1 com comportamento original temporariamente reproduzido: falhou na segunda aquisição, como esperado; arquivo corrigido restaurado em `finally` e suíte completa aprovada em seguida.
- Regressões de sync no serviço, handshake, exclusão entre processos e morte após reserva: aprovadas no macOS.
- YAML analisado com Ruby/Psych; matriz, eventos, unicidade/condição do fmt e pinagem SHA conferidos. Todos os scripts `run` passaram em `bash -n`. Isso não substitui a execução pelo GitHub Actions.
- `git diff --check`: aprovado; `git status --short` revisado.
- Aviso preexistente de importação `Path` não utilizada em `doctor.rs` permanece fora do escopo.

Nenhum teste ou build foi executado em Windows/Linux nesta sessão. Os testes específicos de Windows não entram na contagem local. O cenário 8.3 depende de o filesystem fornecer um alias; o teste informa quando o recurso está desabilitado. A validação direta do nome por handle permanece obrigatória no teste Windows de nomes normalizados.

## Riscos residuais e recomendações antes do commit

1. Executar a matriz remota após publicação autorizada; revisar especialmente permissões/compartilhamento de handles, aliases Windows e dependências GTK/WebKit no Ubuntu. Não considerar a correção Windows validada antes disso.
2. Windows não fornece nesta implementação flush portátil do diretório; perda de energia pode perder metadados mesmo após sync de arquivo. macOS usa `sync_all`, sem `F_FULLFSYNC`; hardware/filesystem continuam relevantes. Os novos testes injetam erros, não simulam energia física.
3. ACLs Windows devem manter o armazenamento privado; o código não provisiona ACLs. No Unix, paths com ancestrais inseguros são recusados. A sincronização de cada pai existente aumenta IO e pode recusar filesystems que não suportam sync de diretório; esse resultado é fail-closed.
4. Contexto identifica um nome, não um fingerprint imutável do daemon. Renomeação/edição externa ou dois contextos apontando para o mesmo daemon não têm exclusão global garantida. Endurecer isso exigiria decisão de identidade/schema fora desta etapa.
5. A confirmação de reconciliação reconhece um resultado histórico incerto. Após timeout, o serviço não consegue provar que o daemon ou um CLI cuja limpeza falhou deixou de trabalhar; `LifecycleOutput` não transporta um sinal estruturado de limpeza. Não confundir estado estável pontual com prova de término histórico. Uma política mais forte exigiria rever executor/contrato e identidade do daemon antes de implementação.
6. Temporários abandonados/corrupção exigem investigação manual; não adicionar limpeza/replay automático sem revisão. Locks assumem processos cooperativos, mesmo caminho de armazenamento e filesystem local compatível.
7. Revisar separadamente o delta MAC.8 e as alterações de lifecycle que já estavam no workspace. Não incluir `.local-backups`, logs ou arquivos temporários. Nenhum commit, push, merge, rebase, reset ou deploy foi realizado.

## Arquivos alterados nesta etapa

- `crates/autodock-core/src/lifecycle_fs.rs`: correções Unix/Windows, injeção de falhas exclusiva de testes e regressões.
- `crates/autodock-core/src/container_lifecycle.rs`: somente novo teste de falha de sincronização.
- `crates/autodock-core/tests/lifecycle_lock_process.rs`: handshake/cleanup determinísticos e regressão.
- `.github/workflows/rust-ci.yml`: novo workflow nativo.
- `docs/mac8-lifecycle-audit.md`: este relatório.

Os demais arquivos modificados/não rastreados em `git status` são alterações anteriores preservadas.
