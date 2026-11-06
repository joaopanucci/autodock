## O que muda e por quê

<!-- Explique em poucas linhas. Link para a issue, se houver: Closes #123 -->

## Como foi testado

- [ ] `cargo fmt --all -- --check`
- [ ] `cargo test -p autodock-core -p autodock-cli --locked`
- [ ] `npm --prefix apps/desktop run typecheck` e `npm --prefix apps/desktop test`
- [ ] Testei o fluxo na prática (ex.: gerei o projeto e rodei `docker compose up --build`)

Sistemas em que testei manualmente: <!-- macOS / Windows / Linux -->

## Checklist de segurança

- [ ] Não sobrescreve arquivos existentes nem segue symlinks
- [ ] Valores que entram em Dockerfile/YAML/comandos são validados (lista permitida)
- [ ] Não lê `.env` nem executa scripts de projetos analisados
- [ ] Não depende de suposições de um único sistema operacional
