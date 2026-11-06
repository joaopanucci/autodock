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

O workflow **Smoke test installers** instala um release publicado em runners limpos de Windows, macOS e Linux (pelos scripts `scripts/install.ps1` e `scripts/install.sh`), abre o app e salva um screenshot.

## Traduções e screenshots

Veja as seções *Traduções* e *Atualizando os screenshots do README* do [CONTRIBUTING](../CONTRIBUTING.pt-BR.md).

## Bootstrap legado do Windows

`bootstrap-autodock.ps1` e os testes PowerShell em `tests/` são anteriores à matriz de CI. Só valem para um checkout completo no Windows, gravam seus relatórios em `.autodock-bootstrap/` e não instalam nada:

```powershell
pwsh -NoProfile -File .\bootstrap-autodock.ps1
pwsh -NoProfile -File tests/Diagnostics.Tests.ps1
pwsh -NoProfile -File tests/Bootstrap.Tests.ps1
```
