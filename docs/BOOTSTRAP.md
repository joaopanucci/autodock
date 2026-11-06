# Investigação dos scripts recebidos

Ambos os scripts de C:\projetos foram lidos e comparados antes da implementação, sem executá-los ou alterá-los. A segunda versão muda principalmente VersionOf e a classificação de falhas. O scaffold comum serviu de referência inicial.

## Falhas identificadas

1. O parâmetro `$Args` colide com a variável automática do PowerShell. Uma reprodução isolada no PowerShell 7.6.9 confirmou argumentos vazios para node e docker compose. Node sem --version pode aguardar entrada, compatível com o timeout observado. Não há evidência para atribuir todos os resultados antigos a uma causa única.
2. A primeira versão não limita o tempo nem valida explicitamente o exit code em VersionOf.
3. A segunda chama WaitForExit antes de drenar os pipes, permitindo bloqueio por buffer cheio; ReadToEnd posterior também pode esperar filhos com handles abertos.
4. npm.cmd não é um executável nativo para ProcessStartInfo/UseShellExecute=false. npm.ps1 também não deve ser iniciado como PE.
5. Get-Command podia resolver aliases/scripts; PATH não basta para Rust/MSBuild.
6. As verificações interrompiam tudo antes da criação do workspace e dos relatórios.
7. Destino existente era rejeitado, impossibilitando repetição. Checks de instalação/build não tinham prazo.
8. Existência de vswhere ou de chave genérica EdgeUpdate não prova presença de C++/WebView2. A fixture Spring não tinha evidência Spring, e faltava LICENSE completa.

## Consolidação

O bootstrap prepara uma distribuição completa, usando os próprios fontes como templates e bootstrap-files.json como lista explícita. Não mantém outra implementação Rust embutida em here-strings.

- `$Arguments` e ArgumentList preservam parâmetros e espaços.
- Resolução Application; busca adicional de Rust e MSBuild.
- npm executado por node + npm-cli.js, sem shell interpolado.
- Streams concorrentes, timeout total e Job Object Windows.
- Estados separados, executável resolvido, streams, exit code, duração e detalhe.
- Estrutura antes dos checks; relatórios únicos JSON/Markdown mesmo com pendências.
- Apenas arquivos ausentes são copiados; divergências são preservadas e reportadas.
- Sem instalação global, remoção, Git, branches, worktrees ou alterações Docker.

Node/npm/Git/Docker/Compose/Engine responderam com os argumentos corrigidos. MSBuild 18.9.1.35102 foi localizado fora do PATH. Rust e Cargo não foram resolvidos nas buscas realizadas. As falhas anteriores não foram tratadas como prova de instalação ausente.

O teste de captura Unicode usa saída UTF-8 explícita: versões ASCII não expõem diferenças de codepage de ferramentas legadas. Testes também cobrem volume acima do buffer, filho real encerrado por timeout, argumentos com espaços e falhas de inicialização. A repetição do bootstrap preserva edições do usuário e gera novos relatórios.
