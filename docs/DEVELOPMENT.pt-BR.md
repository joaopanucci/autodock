🌐 [English](DEVELOPMENT.md) · **Português (Brasil)**

# Guia de desenvolvimento

Como rodar, testar e entender o código do AutoDock. Para a visão geral do produto veja o [README](../README.pt-BR.md); para o fluxo de contribuição veja o [CONTRIBUTING](../CONTRIBUTING.pt-BR.md).

## Pré-requisitos

- **Rust** stable e Cargo (a CI fixa a toolchain nos workflows). No Windows use a toolchain MSVC.
- **Node.js** e npm para o frontend (Node 22.18+; a CI usa 24).
- **Dependências de sistema do Tauri 2:** ferramentas Xcode no macOS; ferramentas C++, Windows SDK e WebView2 no Windows; `libwebkit2gtk-4.1-dev` e afins no Linux. Veja os [pré-requisitos oficiais do Tauri 2](https://v2.tauri.app/start/prerequisites/).
- **Docker** (Engine e `docker compose` v2). A análise funciona sem ele, mas criar projetos de framework e rodar os ambientes gerados exige Docker.

## Organização

```
crates/autodock-core   Núcleo em Rust (sem UI): scanner, gerador, instaladores de framework,
                       IDEs, portas, senhas, consultas Docker, i18n
crates/autodock-cli    Linha de comando (Clap)
apps/desktop           Interface React + Vite; src-tauri é o adaptador Tauri
fixtures               Manifests pequenos e inertes usados nos testes
scripts                install.sh / install.ps1, screenshots.mjs, scripts legados do Windows
docs                   Princípios de segurança, arquitetura e relatórios de etapa
```

O núcleo não pode depender de Tauri. Os comandos Tauri (`apps/desktop/src-tauri/src/lib.rs`) só envolvem funções do núcleo e repassam o idioma de quem chamou.

## Rodar o app

```bash
npm --prefix apps/desktop ci
npm --prefix apps/desktop run tauri dev      # app desktop com hot reload
```

```bash
npm --prefix apps/desktop run build          # TypeScript + bundle web
npm --prefix apps/desktop run tauri build    # instaladores nativos (veja abaixo)
```

O bundle web sozinho não tem os comandos Tauri, então o fluxo completo exige o app desktop.

## Linha de comando

```bash
cargo run -p autodock-cli -- scan ./fixtures/node-basic
cargo run -p autodock-cli -- doctor --json
cargo run -p autodock-cli -- generate ./meu-projeto --stack nodejs --port 3000 --service postgres
cargo run -p autodock-cli -- --lang pt scaffold ./pasta-vazia --framework laravel --name loja
```

`generate` e `scaffold` só mostram o preview, a menos que você passe `--write` / `--run`. Defina `AUTODOCK_HOME` para manter o registro de portas (`ports.json`) longe do seu real enquanto experimenta.

## Testes e verificações

```bash
cargo fmt --all -- --check
cargo check --workspace --locked
cargo clippy -p autodock-core -p autodock-cli --all-targets --locked
cargo test -p autodock-core -p autodock-cli --locked

npm --prefix apps/desktop run typecheck
npm --prefix apps/desktop test                # testes unitários (Node), inclui a paridade de idiomas
npm --prefix apps/desktop run test:ui         # Playwright; AUTODOCK_BROWSER_CHANNEL=chrome|msedge
```

- Os testes Rust que tocam o sistema de arquivos usam diretórios temporários; os de portas nunca assumem uma porta livre fixa.
- Os testes de UI rodam em `pt-BR` por padrão (os mais antigos afirmam textos em português); `tests/ui/i18n.spec.ts` roda em inglês.
- Testes que precisam de Docker não fazem parte da CI: valide os projetos gerados à mão com `docker compose up --build`.

## Instaladores e releases

```bash
npm --prefix apps/desktop run tauri build -- --bundles app,dmg      # macOS
npm --prefix apps/desktop run tauri build -- --bundles nsis         # Windows
npm --prefix apps/desktop run tauri build -- --bundles deb,appimage # Linux
```

Para lançar uma versão:

1. Abra uma issue/PR que suba a versão em `Cargo.toml` (crates do workspace), `apps/desktop/src-tauri/tauri.conf.json`, `apps/desktop/package.json` (+ lock), `Cargo.lock` e os links de download em `README.md` / `README.pt-BR.md`.
2. Depois do merge, envie a tag `vX.Y.Z` na `main`. O workflow **Build installers** gera macOS (universal), Windows e Linux e cria o release (ou anexa a um existente) com um `SHA256SUMS.txt`.
3. Confira os arquivos e os checksums, escreva as notas do release em inglês e publique.

O workflow também assina os instaladores para o atualizador do app e envia os arquivos `.sig` e um `latest.json` (o manifesto de atualização). Publicar o release é o que torna a nova versão visível para os apps instalados, porque o app lê `releases/latest/download/latest.json`.

### Chave de assinatura das atualizações

- O par de chaves é criado uma vez com `npx tauri signer generate -w ~/.tauri/autodock-updater.key` (dentro de `apps/desktop`). A chave **pública** fica em `plugins.updater.pubkey` de `apps/desktop/src-tauri/tauri.conf.json`; a **privada** e a senha são os segredos do repositório `TAURI_SIGNING_PRIVATE_KEY` e `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`. Nunca versione a chave privada.
- **Guarde um backup do arquivo da chave privada e da senha** (um gerenciador de senhas). Se forem perdidos, os apps instalados não aceitam atualizações assinadas por uma chave nova: as pessoas teriam de instalar essa versão à mão uma vez.
- Builds locais também precisam da chave, porque `createUpdaterArtifacts` está ligado: defina `TAURI_SIGNING_PRIVATE_KEY` (o conteúdo do arquivo) e `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` antes do `tauri build`.
- Testar uma atualização sem publicar: gere duas versões com `--config` apontando `plugins.updater.endpoints` para `http://127.0.0.1:<porta>/latest.json` e `dangerousInsecureTransportProtocol: true` (só nos builds de teste), sirva um `latest.json` que cite o instalador mais novo e o `.sig` dele, instale o mais antigo e deixe-o se atualizar.

O workflow **Smoke test installers** instala um release publicado em runners limpos de Windows, macOS e Linux (pelos scripts `scripts/install.ps1` e `scripts/install.sh`), abre o app e salva um screenshot.

## Traduções e screenshots

Veja as seções *Traduções* e *Atualizando os screenshots do README* do [CONTRIBUTING](../CONTRIBUTING.pt-BR.md).

## Tema, ícone e imagens da marca

- **Tema.** O app tem tema claro e escuro (`apps/desktop/src/theme.ts`, seletor na barra superior e em Configurações). A escolha fica no `localStorage` (`autodock.theme`); o padrão segue o sistema operacional. As cores vivem em tokens: `src/theme-tokens.css` define cada cor `--c-*` duas vezes (claro em `:root`, escuro em `:root[data-theme="dark"]`) e os tokens escritos à mão (`--bg`, `--panel`, `--ink`, `--line`…) ficam no fim de `src/style.css`. Para mudar uma cor, edite nos **dois** lugares; não escreva hex novo nas regras.
- **Ícone.** A fonte é `apps/desktop/src-tauri/icons/app-icon.svg`. Renderize em PNG de 1024 px (`app-icon.png`, cantos transparentes) e rode `npx tauri icon src-tauri/icons/app-icon.png` em `apps/desktop` para regerar todos os tamanhos.
- **Instalador do Windows.** `bundle.windows.nsis` no `tauri.conf.json` define a instalação por usuário, a página de licença e a arte em `src-tauri/installer/` (`sidebar.bmp` 164×314 e `header.bmp` 150×57, 24 bits). O `installer/hooks.nsh` renova o cache de ícones do Windows após instalar.
- **Imagens da marca.** `node scripts/brand-assets.mjs` regera o fundo do DMG do macOS e o `docs/social-preview.png`. Rode antes o `scripts/screenshots.mjs` (precisa de `cargo build -p autodock-cli` e do servidor de desenvolvimento); use `AUTODOCK_THEME=dark` para o conjunto escuro. Os dois scripts funcionam no Windows, macOS e Linux (defina `AUTODOCK_BROWSER_CHANNEL` se não tiver Edge ou Chrome).

## Motor de upgrade de Java

`crates/autodock-core/src/java.rs` lê um projeto Maven (nível do Java, Spring Boot, APIs removidas do JDK); `java_upgrade.rs` planeja e executa o upgrade de uma cópia dentro de `maven:3.9-eclipse-temurin-25`. A ordem dos passos (`hops_for`) é o que mantém cada build intermediário funcionando: Spring Boot 2.7 primeiro (ainda compila no nível antigo), Java 17 (o Spring Boot 3 exige), Spring Boot 3.5 (`javax` → `jakarta`), Spring Boot 4.0 e Java 25.

- O plugin do OpenRewrite e as bibliotecas de receitas são fixados (`REWRITE_PLUGIN`, `RECIPE_LIBRARIES`) junto com a imagem e as versões-alvo (`TARGET_JAVA`, `TARGET_BOOT`). Para subir de versão, troque tudo junto e rode os projetos de teste abaixo de ponta a ponta; um nome de receita que não existe faz a execução falhar (`failOnInvalidActiveRecipes`).
- Usamos `rewrite:runNoFork` de propósito: o `rewrite:run` compila antes, e um build legado não compila no JDK novo. Num projeto multimódulo o `runNoFork` não resolve um módulo que depende de um irmão, então o script roda antes a fase `compile` com `-Dmaven.main.skip=true` (veja o comentário em `script`).
- Cada passo é envolvido por `snap` e `restore` no script: uma receita que falha ou uma compilação que falha devolve a cópia ao estado anterior (`ROLLBACK <passo>` na saída), os passos seguintes não rodam e os testes rodam no último estado que funcionou (são pulados quando nenhum passo funcionou). O relatório tem `rolledBack` e `reached`.
- `PINNING_BOMS` (em `java.rs`) lista BOMs que decidem sozinhos a versão do Spring Boot (hoje só `tech.jhipster:jhipster-dependencies`). Para um projeto assim `hops_pinned` planeja só passos de Java, limitados ao Java 17 enquanto o Spring Boot for anterior ao 3.5. Foi um projeto JHipster Online de verdade (Spring Boot 2.7.3) que mostrou isso: a receita do Spring Boot 3.5 aplicou mas deixou um pom inválido.
- Os testes unitários usam um executor falso (sem Docker). Para conferir um upgrade de verdade, monte projetos pequenos: um Spring Boot 1.5 em Java 1.7 (JPA, um controller REST, um teste JUnit 4 que usa `javax.xml.bind`), uma biblioteca Java 1.6 simples com JUnit 4, um projeto multimódulo e um que usa `sun.misc.BASE64Encoder` (deve parar no passo do Java 17 e dizer qual). Depois rode `autodock java upgrade <pasta> --yes` e confira que o original ficou intacto e que a cópia compila e passa nos testes.

## Bootstrap legado do Windows

`bootstrap-autodock.ps1` e os testes PowerShell em `tests/` são anteriores à matriz de CI. Só valem para um checkout completo no Windows, gravam seus relatórios em `.autodock-bootstrap/` e não instalam nada:

```powershell
pwsh -NoProfile -File .\bootstrap-autodock.ps1
pwsh -NoProfile -File tests/Diagnostics.Tests.ps1
pwsh -NoProfile -File tests/Bootstrap.Tests.ps1
```
