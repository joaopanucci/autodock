🌐 [English](SECURITY.md) · **Português (Brasil)**

# Política de segurança

## Como relatar uma vulnerabilidade

**Não abra uma issue pública.** Use o [relato privado de vulnerabilidades](https://github.com/joaopanucci/autodock/security/advisories/new) do GitHub. Inclua:

- a versão do AutoDock e o seu sistema operacional;
- os passos para reproduzir e o impacto que você enxerga;
- se possível, uma sugestão de correção.

Respondemos assim que possível e combinamos a divulgação depois da correção.

## Escopo

O AutoDock gera arquivos Docker, roda instaladores de frameworks dentro de containers e abre editores no seu computador. São especialmente relevantes:

- sobrescrita ou escrita fora da pasta escolhida (path traversal, symlinks);
- injeção em Dockerfile, YAML ou linhas de comando a partir de entradas do usuário ou de arquivos do projeto;
- execução de código dos projetos analisados;
- leitura de arquivos sensíveis (`.env`, chaves);
- exposição de serviços fora de `127.0.0.1`.

Os princípios de segurança do projeto estão em [`docs/SECURITY.md`](docs/SECURITY.md).

## Versões suportadas

Apenas a versão mais recente publicada em [Releases](https://github.com/joaopanucci/autodock/releases) recebe correções.

## Sobre os instaladores

Os instaladores **ainda não são assinados** (Apple/Microsoft). Confira sempre o `SHA256SUMS.txt` de cada release e baixe somente do repositório oficial.
