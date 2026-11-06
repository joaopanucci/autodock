# Container lifecycle: persistência e recuperação

Implementação na branch `feature/005-container-lifecycle`, validada em macOS Apple Silicon em 2026-11-06. Nenhum comando Docker foi executado durante o desenvolvimento. A alteração local preexistente que canonicaliza o diretório temporário dos testes do journal foi preservada. Backups e logs estão em `.local-backups/container-lifecycle-20261106/`, ignorado pelo Git.

## Fluxo e contratos

`LifecycleService` recebe diretório de persistência e contexto Docker explícitos. `pending()` enumera operações após reinicialização; nunca repete comandos. `execute_with_docker()` e `reconcile_with_docker()` usam o executor real; `execute()` e `reconcile()` aceitam `LifecycleExecutor` para simulações. Os pontos de entrada anteriores permanecem disponíveis e escolhem o contexto atual uma vez, passando `--context` explicitamente a todos os comandos seguintes.

1. Validar confirmação, ação, contexto e ID solicitado.
2. Adquirir `LifecycleFileLock` exclusivo, não bloqueante, externo ao diretório de registros.
3. Validar todos os registros; corrupção, duplicatas e entradas inesperadas bloqueiam o journal inteiro.
4. Inspecionar somente leitura, resolver ID completo de 64 caracteres e confirmar sua correspondência com o prefixo solicitado.
5. Reservar a identidade `(contexto, ID completo normalizado)` em disco.
6. Executar apenas `start`, `stop --time 10` ou `restart --time 10`, usando o ID completo.
7. Inspecionar novamente identidade e estado. Apenas saída íntegra, exit code zero e estado esperado autorizam conclusão normal.
8. Retirar a reserva após resultado verificado. `Missing` também permite retirada porque seu contrato garante que nenhum comando foi iniciado. Todos os demais resultados não verificados conservam a reserva.

O lock cobre validação, reserva, execução, inspeção e retirada. Uma reserva isolada não distingue executor interrompido de executor ainda ativo; por isso o lock é mantido durante a operação. Isso serializa inclusive containers e contextos distintos dentro do mesmo journal, preferindo previsibilidade a paralelismo. Um journal ocupado retorna erro; não há espera ou repetição automática. O descarte faz unlock explícito, evitando retenção transitória por descritores herdados por fork de outra thread.

`outcomeConfirmed` mantém seu contrato de confirmação do comando CLI. `stateVerified` confirma adicionalmente identidade e estado; sucesso do CLI sozinho não libera uma operação incerta.

## Publicação e segurança dos arquivos

O registro é serializado integralmente em `<operationId>.tmp`, criado exclusivamente, gravado e sincronizado. A publicação cria um hard link exclusivo `<operationId>.json`; não usa `rename` com possibilidade de sobrescrita. Em seguida sincroniza o diretório, remove o temporário, sincroniza novamente e relê o registro antes de autorizar execução. Colisões de identificador falham sem substituir registros. Filesystems sem hard links falham antes de qualquer mutação Docker.

Falhas de escrita, sincronização ou publicação conservam evidências em vez de limpar arquivos automaticamente. Uma interrupção pode deixar somente temporário, ambos os nomes ou um registro definitivo. Os dois primeiros casos bloqueiam o journal; o último é uma pendência reconciliável. Não há comando de limpeza automática de temporários/corrupção: é necessária investigação manual com o executor parado e preservação dos arquivos.

A leitura limita cada registro a 4096 bytes, rejeita schema/campos desconhecidos, nomes inconsistentes, arquivos não regulares, symlinks, reparse points, hard links inesperados e identidades duplicadas. A enumeração e abertura não dependem somente de uma prechecagem de metadados.

## Reconciliação

Exige confirmação explícita, contexto correspondente e ID completo persistido, aceitando variação de caixa. Inspeciona identidade exata e um estado estável (`running`, `exited`, `created`, `paused` ou `dead`). Falha de inspeção, identidade divergente ou estado transitório mantém a pendência. Não executa a ação original e não afirma conhecer seu resultado histórico. Uma observação de estado é pontual; outros clientes Docker podem alterá-lo posteriormente.

## Plataformas e limites

| Plataforma | Acesso e proteção | Sincronização e diretório padrão |
| --- | --- | --- |
| macOS | `openat`, `mkdirat`, `linkat`, `unlinkat`, `O_NOFOLLOW`; diretórios ancorados em descritores; novos diretórios 0700 e arquivos 0600 | `sync_all` de arquivo e diretório; `~/Library/Application Support/AutoDock/lifecycle/pending` |
| Linux | Mesmo acesso relativo a descritores; `O_NONBLOCK` evita bloqueio ao encontrar FIFO; proprietário/permissões de arquivos e ancestrais verificados | `sync_all` de arquivo e diretório; `$XDG_DATA_HOME/autodock/lifecycle/pending` ou `~/.local/share/autodock/lifecycle/pending` |
| Windows | Handles de todos os diretórios abertos sem compartilhamento de exclusão, rejeitando reparse points; arquivos abertos sem seguir reparse points; identidade de lock por volume/File ID e contagem de links por handle | Arquivo sincronizado antes da publicação; sem garantia portátil de flush de diretório; `%LOCALAPPDATA%/AutoDock/lifecycle/pending` |

Somente macOS foi executado nesta sessão; Linux e Windows ainda exigem CI nativo. O Windows depende de ACLs privadas herdadas do diretório de dados do usuário; não provisionamos ACLs. No Unix, diretório do journal e seu pai devem pertencer ao usuário e não permitir escrita por grupo/outros; ancestrais inseguros também são rejeitados, com exceção de diretórios sticky confiáveis como `/tmp`. Caminhos fornecidos não são canonicalizados silenciosamente para aceitar symlinks; apenas bases conhecidas do sistema/usuário e temporários de testes são resolvidos antes da construção do caminho.

Locks são consultivos. Processos maliciosos com a mesma identidade do usuário, administradores, substituição externa do arquivo de lock, alteração do armazenamento ou edição concorrente de configurações/endpoints Docker estão fora da garantia de exclusão cooperativa. O contexto persistido identifica o nome do contexto, não um fingerprint imutável do daemon. Não compartilhar journals entre máquinas, diretórios sincronizados ou filesystems sem garantias equivalentes de lock/link/sync. `sync_all` não garante sobrevivência a falha física de hardware; no macOS não acrescentamos `F_FULLFSYNC`, e no Windows não prometemos durabilidade de metadados após perda de energia.

## Validação

- `cargo check --workspace --locked`: aprovado (core, CLI e desktop).
- `cargo fmt --all -- --check`: aprovado.
- `cargo test -p autodock-core --locked`: 101 testes aprovados (70 unitários, 31 de integração, incluindo dois helpers de subprocesso); nenhum ignorado.
- `git diff --check`: aprovado.
- Aviso preexistente: importação `Path` não utilizada em `doctor.rs` no macOS, mantida fora do escopo.

Os testes de lifecycle usam executores simulados. Cobrem reinicialização do journal, sobrescrita, escrita parcial, falha de publicação, temporários abandonados, interrupção entre publicação e limpeza, threads concorrentes, lock entre processos, morte abrupta após reserva, reconciliação em outro processo, IDs equivalentes/contextos distintos, inspeção falha/divergente, symlink/hardlink/FIFO, permissões e substituição de diretório. Os testes anteriores do registry em memória foram adaptados para o serviço persistente, com cenários correlatos agrupados em matrizes.

A dependência direta Unix `libc` reutiliza a versão já presente no lockfile e permite chamadas relativas a descritores sem adicionar pacotes novos ao grafo. O schema persistido permanece na versão 1.

Próxima etapa: executar os testes em runners nativos Windows/Linux e conectar a UI desktop a uma instância com caminho/contexto explícitos, exibindo `pending()` e solicitando reconhecimento antes de reconciliar. Manter temporários/corrupção bloqueados até um procedimento de recuperação manual revisado.
