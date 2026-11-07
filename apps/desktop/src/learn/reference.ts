// Quick reference (commands) and glossary for the Learn area. Same rules as
// lessons.ts: every text is a { en, pt } pair and the file stays free of
// TypeScript-only runtime syntax so the unit tests can load it directly.
import type { Bi } from "./lessons.ts";

export type RefGroup = "containers" | "images" | "data" | "compose" | "inspect" | "cleanup";

export const REF_GROUPS: { id: RefGroup; title: Bi }[] = [
  { id: "containers", title: { en: "Containers", pt: "Containers" } },
  { id: "images", title: { en: "Images", pt: "Imagens" } },
  { id: "data", title: { en: "Volumes and networks", pt: "Volumes e redes" } },
  { id: "compose", title: { en: "Compose", pt: "Compose" } },
  { id: "inspect", title: { en: "Logs and inspection", pt: "Logs e inspeção" } },
  { id: "cleanup", title: { en: "Cleanup", pt: "Limpeza" } },
];

export type RefCommand = {
  id: string;
  group: RefGroup;
  /** Command as typed in bash; powershell/cmd only when the spelling differs. */
  bash: string;
  powershell?: string;
  cmd?: string;
  description: Bi;
};

export const REFERENCE: RefCommand[] = [
  // Containers
  { id: "run", group: "containers", bash: "docker run -d --name web -p 8080:80 nginx:alpine", description: { en: "Start a container in the background and publish a port", pt: "Iniciar um container em segundo plano e publicar uma porta" } },
  { id: "run-rm", group: "containers", bash: "docker run --rm -it alpine sh", description: { en: "Try something in a throwaway container (deleted on exit)", pt: "Testar algo em um container descartável (apagado ao sair)" } },
  { id: "ps", group: "containers", bash: "docker ps", description: { en: "List running containers", pt: "Listar containers em execução" } },
  { id: "ps-a", group: "containers", bash: "docker ps -a", description: { en: "List all containers, including stopped ones", pt: "Listar todos os containers, inclusive os parados" } },
  { id: "stop", group: "containers", bash: "docker stop web", description: { en: "Stop a container gracefully", pt: "Parar um container com educação" } },
  { id: "start", group: "containers", bash: "docker start web", description: { en: "Start a stopped container again", pt: "Iniciar de novo um container parado" } },
  { id: "restart", group: "containers", bash: "docker restart web", description: { en: "Restart a container", pt: "Reiniciar um container" } },
  { id: "rm", group: "containers", bash: "docker rm web", description: { en: "Remove a stopped container (add -f to force)", pt: "Remover um container parado (use -f para forçar)" } },
  { id: "exec", group: "containers", bash: "docker exec -it web sh", description: { en: "Open a shell inside a running container", pt: "Abrir um shell dentro de um container em execução" } },

  // Images
  { id: "pull", group: "images", bash: "docker pull postgres:16", description: { en: "Download an image", pt: "Baixar uma imagem" } },
  { id: "images", group: "images", bash: "docker images", description: { en: "List the images on this computer", pt: "Listar as imagens deste computador" } },
  { id: "build", group: "images", bash: "docker build -t myapp .", description: { en: "Build an image from the Dockerfile in this folder", pt: "Construir uma imagem a partir do Dockerfile desta pasta" } },
  { id: "tag", group: "images", bash: "docker tag myapp myapp:1.0", description: { en: "Give an image another name or version", pt: "Dar outro nome ou versão a uma imagem" } },
  { id: "rmi", group: "images", bash: "docker rmi myapp:1.0", description: { en: "Remove an image", pt: "Remover uma imagem" } },

  // Volumes and networks
  { id: "volume-create", group: "data", bash: "docker volume create pgdata", description: { en: "Create a named volume", pt: "Criar um volume nomeado" } },
  { id: "volume-ls", group: "data", bash: "docker volume ls", description: { en: "List volumes", pt: "Listar volumes" } },
  {
    id: "bind",
    group: "data",
    bash: "docker run --rm -v \"$PWD\":/app -w /app node:22 npm test",
    powershell: "docker run --rm -v \"${PWD}:/app\" -w /app node:22 npm test",
    cmd: "docker run --rm -v \"%cd%:/app\" -w /app node:22 npm test",
    description: { en: "Mount the current folder inside a container", pt: "Montar a pasta atual dentro de um container" },
  },
  { id: "network-ls", group: "data", bash: "docker network ls", description: { en: "List networks", pt: "Listar redes" } },

  // Compose
  { id: "up", group: "compose", bash: "docker compose up -d --build", description: { en: "Build and start every service in the background", pt: "Construir e iniciar todos os serviços em segundo plano" } },
  { id: "compose-ps", group: "compose", bash: "docker compose ps", description: { en: "Show the services of the project", pt: "Mostrar os serviços do projeto" } },
  { id: "compose-logs", group: "compose", bash: "docker compose logs -f app", description: { en: "Follow the logs of one service", pt: "Acompanhar os logs de um serviço" } },
  { id: "compose-exec", group: "compose", bash: "docker compose exec app sh", description: { en: "Open a shell in a service", pt: "Abrir um shell em um serviço" } },
  { id: "down", group: "compose", bash: "docker compose down", description: { en: "Stop and remove the services (volumes are kept)", pt: "Parar e remover os serviços (os volumes são mantidos)" } },

  // Logs and inspection
  { id: "logs", group: "inspect", bash: "docker logs -f --tail 100 web", description: { en: "Last 100 lines of a container, then follow live", pt: "Últimas 100 linhas de um container e depois acompanhar ao vivo" } },
  { id: "stats", group: "inspect", bash: "docker stats --no-stream", description: { en: "CPU, memory and network of running containers", pt: "CPU, memória e rede dos containers em execução" } },
  { id: "inspect", group: "inspect", bash: "docker inspect web", description: { en: "Full details of a container as JSON", pt: "Detalhes completos de um container em JSON" } },
  {
    id: "exit-code",
    group: "inspect",
    bash: "docker inspect --format '{{.State.ExitCode}} {{.State.OOMKilled}}' web",
    cmd: "docker inspect --format \"{{.State.ExitCode}} {{.State.OOMKilled}}\" web",
    description: { en: "Why did it stop? Exit code and out-of-memory flag", pt: "Por que parou? Código de saída e indicação de falta de memória" },
  },
  { id: "top", group: "inspect", bash: "docker top web", description: { en: "Processes running inside a container", pt: "Processos rodando dentro de um container" } },

  // Cleanup
  { id: "df", group: "cleanup", bash: "docker system df", description: { en: "How much space Docker uses", pt: "Quanto espaço o Docker usa" } },
  { id: "container-prune", group: "cleanup", bash: "docker container prune", description: { en: "Remove every stopped container", pt: "Remover todos os containers parados" } },
  { id: "image-prune", group: "cleanup", bash: "docker image prune", description: { en: "Remove images that no tag points to", pt: "Remover imagens que nenhuma tag aponta" } },
  { id: "builder-prune", group: "cleanup", bash: "docker builder prune", description: { en: "Remove the build cache", pt: "Remover o cache de build" } },
];

export type GlossaryTerm = { id: string; term: Bi; definition: Bi };

export const GLOSSARY: GlossaryTerm[] = [
  { id: "image", term: { en: "Image", pt: "Imagem" }, definition: { en: "A read-only package with an application and everything it needs. Containers are started from images.", pt: "Um pacote somente leitura com uma aplicação e tudo que ela precisa. Os containers são iniciados a partir de imagens." } },
  { id: "container", term: { en: "Container", pt: "Container" }, definition: { en: "A running instance of an image, isolated from the rest of the computer.", pt: "Uma instância em execução de uma imagem, isolada do resto do computador." } },
  { id: "dockerfile", term: { en: "Dockerfile", pt: "Dockerfile" }, definition: { en: "A text file with the steps to build an image.", pt: "Um arquivo de texto com os passos para construir uma imagem." } },
  { id: "layer", term: { en: "Layer", pt: "Camada" }, definition: { en: "One step of an image build. Unchanged layers are reused from cache, which makes builds fast.", pt: "Um passo da construção de uma imagem. Camadas que não mudaram são reaproveitadas do cache, o que acelera os builds." } },
  { id: "tag", term: { en: "Tag", pt: "Tag" }, definition: { en: "The version label after the colon in an image name, like postgres:16.", pt: "O rótulo de versão depois dos dois pontos no nome de uma imagem, como postgres:16." } },
  { id: "registry", term: { en: "Registry", pt: "Registro" }, definition: { en: "A server that stores and distributes images. Docker Hub is the best known.", pt: "Um servidor que guarda e distribui imagens. O Docker Hub é o mais conhecido." } },
  { id: "volume", term: { en: "Volume", pt: "Volume" }, definition: { en: "Storage managed by Docker that outlives containers. The right place for database files.", pt: "Armazenamento gerenciado pelo Docker que sobrevive aos containers. O lugar certo para arquivos de banco de dados." } },
  { id: "bind-mount", term: { en: "Bind mount", pt: "Bind mount" }, definition: { en: "A folder of your computer shared with a container, handy to edit code live.", pt: "Uma pasta do seu computador compartilhada com um container, útil para editar código ao vivo." } },
  { id: "port-publishing", term: { en: "Port publishing", pt: "Publicação de porta" }, definition: { en: "Mapping a host port to a container port (-p host:container) so you can reach the service.", pt: "Mapear uma porta do host para uma porta do container (-p host:container) para você acessar o serviço." } },
  { id: "network", term: { en: "Network", pt: "Rede" }, definition: { en: "A private network that lets containers find each other by name.", pt: "Uma rede privada que permite aos containers se encontrarem pelo nome." } },
  { id: "compose", term: { en: "Compose", pt: "Compose" }, definition: { en: "A tool and file format (docker-compose.yml) to describe and run several services together.", pt: "Uma ferramenta e formato de arquivo (docker-compose.yml) para descrever e rodar vários serviços juntos." } },
  { id: "service", term: { en: "Service", pt: "Serviço" }, definition: { en: "One component of a Compose project, such as app or db. Its name is also its host name on the network.", pt: "Um componente de um projeto Compose, como app ou db. O nome dele também é o nome de host na rede." } },
  { id: "engine", term: { en: "Docker Engine", pt: "Docker Engine" }, definition: { en: "The background service that builds and runs containers. The docker command talks to it.", pt: "O serviço em segundo plano que constrói e roda containers. O comando docker conversa com ele." } },
  { id: "desktop", term: { en: "Docker Desktop", pt: "Docker Desktop" }, definition: { en: "The application that installs and runs the engine on Windows and macOS.", pt: "O aplicativo que instala e roda o engine no Windows e no macOS." } },
  { id: "wsl2", term: { en: "WSL 2", pt: "WSL 2" }, definition: { en: "Windows Subsystem for Linux: the lightweight Linux environment where Docker Desktop runs Linux containers on Windows.", pt: "Subsistema do Windows para Linux: o ambiente Linux leve onde o Docker Desktop roda containers Linux no Windows." } },
  { id: "healthcheck", term: { en: "Healthcheck", pt: "Healthcheck" }, definition: { en: "A command Docker runs regularly to know whether the service inside a container is really working.", pt: "Um comando que o Docker executa de tempos em tempos para saber se o serviço dentro do container está mesmo funcionando." } },
  { id: "env-var", term: { en: "Environment variable", pt: "Variável de ambiente" }, definition: { en: "A named setting given to a container, such as a database address or password.", pt: "Uma configuração com nome entregue ao container, como o endereço ou a senha de um banco." } },
  { id: "exit-code", term: { en: "Exit code", pt: "Código de saída" }, definition: { en: "The number a program returns when it ends. 0 means success; other values mean an error or a kill.", pt: "O número que um programa devolve ao terminar. 0 é sucesso; outros valores indicam erro ou encerramento forçado." } },
  { id: "prune", term: { en: "Prune", pt: "Prune (limpeza)" }, definition: { en: "Removing Docker objects that are no longer used, to free disk space.", pt: "Remover objetos do Docker que não são mais usados, para liberar espaço em disco." } },
];
