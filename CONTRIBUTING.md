# Contribuindo com o AutoDock

Obrigado por querer ajudar! O AutoDock é open source ([Apache-2.0](LICENSE)) e toda contribuição conta: código, testes, documentação, relatos de bugs e testes em outros sistemas.

## Formas de ajudar

- 🐛 **Relate um problema** ou 💡 **sugira uma ideia** abrindo uma [issue](https://github.com/joaopanucci/autodock/issues/new/choose).
- 🧪 **Teste no Windows e no Linux** e conte como foi (o app completo foi testado principalmente no macOS).
- 🔧 **Envie um pull request** (veja o fluxo abaixo).
- 📝 **Melhore a documentação** ou as traduções.

## Fluxo de contribuição

1. Faça um **fork** e crie uma branch a partir da `main` (`feat/minha-ideia`, `fix/meu-bug`).
2. Faça mudanças **pequenas e focadas**, com testes.
3. Rode as verificações abaixo.
4. Abra um **pull request** para a `main` descrevendo *o quê* e *por quê*. A CI roda em Linux, Windows e macOS.
5. A pessoa mantenedora revisa. Só quem mantém o repositório faz o merge.

> Mensagens de commit seguem o estilo [Conventional Commits](https://www.conventionalcommits.org/pt-br/) (`feat:`, `fix:`, `docs:`, `test:`, `ci:`).

## Rodando localmente

Pré-requisitos e comandos completos estão no [guia de desenvolvimento](docs/DEVELOPMENT.md). Resumo:

```bash
npm --prefix apps/desktop ci
npm --prefix apps/desktop run tauri dev      # abre o app
```

### Verificações antes de abrir o PR

```bash
cargo fmt --all -- --check
cargo check --workspace --locked
cargo test -p autodock-core -p autodock-cli --locked
npm --prefix apps/desktop run typecheck
npm --prefix apps/desktop test
npm --prefix apps/desktop run test:ui        # precisa de um navegador Chromium/Chrome/Edge
```

Se algo não puder ser testado no seu sistema, **diga isso no PR** em vez de afirmar que funciona.

## Princípios que todo PR deve respeitar

O AutoDock altera arquivos do usuário, então a confiança vem primeiro (veja também [`docs/SECURITY.md`](docs/SECURITY.md)):

- **O `autodock-core` não depende de Tauri.** Toda a lógica fica no core; a interface só chama comandos.
- **Preview antes de gravar.** Nada é escrito sem confirmação, e o conteúdo é gerado no backend, nunca enviado pela interface.
- **Nunca sobrescrever** arquivos existentes e nunca seguir symlinks ao gravar.
- **Entradas por lista permitida.** Valores que entram em Dockerfile, YAML ou linha de comando (portas, versões, nomes, scripts) precisam ser validados. Sem montar comandos por texto e sem shell no host.
- **Nunca ler `.env`** nem executar scripts dos projetos analisados.
- **Portas de serviços auxiliares só em `127.0.0.1`.**
- **Multiplataforma.** Evite suposições de macOS/Linux em testes (nomes de arquivo, permissões, caminhos `\\?\` no Windows).

## Onde mexer

| Quero… | Arquivo |
|---|---|
| Adicionar um serviço (ex.: MariaDB) ou ajustar um modelo de Dockerfile/compose | `crates/autodock-core/src/generate.rs` |
| Adicionar um instalador oficial de framework | `crates/autodock-core/src/scaffold.rs` |
| Adicionar uma IDE | `crates/autodock-core/src/ide.rs` |
| Mudar uma tela | `apps/desktop/src/pages/` |

Cada mudança no core deve vir com testes. Para validar de verdade, gere o projeto e suba com `docker compose up --build` (a CLI ajuda: `autodock generate … --write`).

## Atualizando os screenshots do README

```bash
cargo build -p autodock-cli
npm --prefix apps/desktop run dev &
node scripts/screenshots.mjs
```

## Código de conduta

Ao participar, você concorda em seguir o [Código de Conduta](CODE_OF_CONDUCT.md).

## Licença

Ao contribuir, você concorda que sua contribuição será licenciada sob a [Apache-2.0](LICENSE).
