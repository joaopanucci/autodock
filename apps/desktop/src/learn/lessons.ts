// Lesson content for the Learn area. Every text is a { en, pt } pair so a lesson
// cannot be added in only one language (TypeScript fails) and tests check that
// both sides are filled in. Links go to official documentation only.

export type Bi = { en: string; pt: string };

export type AppPage = "setup" | "containers" | "monitoring" | "console";

export type Block =
  | { kind: "p"; text: Bi }
  | { kind: "tip"; text: Bi }
  | { kind: "warn"; text: Bi }
  | { kind: "code"; code: string; caption?: Bi };

export type Lesson = {
  id: string;
  level: "beginner" | "intermediate";
  minutes: number;
  title: Bi;
  summary: Bi;
  blocks: Block[];
  /** Screen of AutoDock where the reader can see the concept for real. */
  tryIn?: { page: AppPage; label: Bi };
  docs: { url: string; label: Bi }[];
};

export const LESSONS: Lesson[] = [
  {
    id: "what-is-docker",
    level: "beginner",
    minutes: 5,
    title: { en: "What Docker is and why use it", pt: "O que é o Docker e por que usar" },
    summary: {
      en: "Images, containers and the idea behind \"it works on my machine\".",
      pt: "Imagens, containers e a ideia por trás do \"na minha máquina funciona\".",
    },
    blocks: [
      {
        kind: "p",
        text: {
          en: "Docker packages an application together with everything it needs (runtime, libraries, configuration) so it runs the same way on any computer.",
          pt: "O Docker empacota uma aplicação junto com tudo que ela precisa (runtime, bibliotecas, configuração) para que rode igual em qualquer computador.",
        },
      },
      {
        kind: "p",
        text: {
          en: "An image is the read-only recipe. A container is a running instance of an image, like a process with its own isolated file system and network. You can start many containers from the same image.",
          pt: "Uma imagem é a receita somente leitura. Um container é uma instância em execução de uma imagem, como um processo com sistema de arquivos e rede isolados. Dá para iniciar vários containers da mesma imagem.",
        },
      },
      {
        kind: "tip",
        text: {
          en: "A container is not a virtual machine: it shares the host kernel, so it starts in seconds and uses far less memory.",
          pt: "Um container não é uma máquina virtual: ele compartilha o kernel do host, por isso inicia em segundos e usa bem menos memória.",
        },
      },
      { kind: "code", code: "docker --version\ndocker run hello-world", caption: { en: "Check the install and run your first container", pt: "Confira a instalação e rode seu primeiro container" } },
    ],
    tryIn: { page: "console", label: { en: "Open the Console", pt: "Abrir o Console" } },
    docs: [{ url: "https://docs.docker.com/get-started/docker-overview/", label: { en: "Docker overview", pt: "Visão geral do Docker" } }],
  },
  {
    id: "images-containers",
    level: "beginner",
    minutes: 8,
    title: { en: "Working with images and containers", pt: "Trabalhando com imagens e containers" },
    summary: {
      en: "Pull, run, list, stop and remove: the everyday commands.",
      pt: "Baixar, rodar, listar, parar e remover: os comandos do dia a dia.",
    },
    blocks: [
      {
        kind: "p",
        text: {
          en: "Most of your time with Docker is spent on a handful of commands. The ones below create, inspect and clean up containers.",
          pt: "Na maior parte do tempo você usa poucos comandos. Os abaixo criam, inspecionam e limpam containers.",
        },
      },
      { kind: "code", code: "docker pull nginx:alpine\ndocker run -d --name web nginx:alpine\ndocker ps\ndocker logs web\ndocker stop web\ndocker rm web", caption: { en: "From download to cleanup", pt: "Do download até a limpeza" } },
      {
        kind: "p",
        text: {
          en: "-d runs the container in the background, --name gives it a readable name, and docker ps lists the running ones (add -a to include stopped containers).",
          pt: "-d roda o container em segundo plano, --name dá um nome legível e docker ps lista os que estão rodando (use -a para incluir os parados).",
        },
      },
      {
        kind: "warn",
        text: {
          en: "Removing a container deletes its writable layer. Data you want to keep belongs in a volume (see the Volumes lesson).",
          pt: "Remover um container apaga a camada gravável dele. Dados que você quer manter ficam em um volume (veja a lição de Volumes).",
        },
      },
    ],
    tryIn: { page: "containers", label: { en: "See your containers", pt: "Ver seus containers" } },
    docs: [{ url: "https://docs.docker.com/reference/cli/docker/", label: { en: "Docker CLI reference", pt: "Referência da CLI do Docker" } }],
  },
  {
    id: "ports",
    level: "beginner",
    minutes: 7,
    title: { en: "How ports work", pt: "Como funcionam as portas" },
    summary: {
      en: "Publishing a container port to your computer, and why 127.0.0.1 matters.",
      pt: "Publicar a porta de um container no seu computador, e por que o 127.0.0.1 importa.",
    },
    blocks: [
      {
        kind: "p",
        text: {
          en: "A container has its own network, so a service listening on port 80 inside it is not reachable from your computer by default. Publishing maps a host port to a container port, written host:container.",
          pt: "Um container tem a própria rede, então um serviço escutando na porta 80 dentro dele não é acessível do seu computador por padrão. Publicar mapeia uma porta do host para uma porta do container, escrito host:container.",
        },
      },
      { kind: "code", code: "docker run -d -p 8080:80 nginx:alpine\n# open http://localhost:8080", caption: { en: "Host 8080 reaches container 80", pt: "A porta 8080 do host chega na 80 do container" } },
      {
        kind: "p",
        text: {
          en: "Two containers cannot publish the same host port. If you see \"port is already allocated\", pick another host port: that is exactly what AutoDock's port registry avoids for you.",
          pt: "Dois containers não podem publicar a mesma porta do host. Se aparecer \"port is already allocated\", escolha outra porta do host: é exatamente isso que o registro de portas do AutoDock evita para você.",
        },
      },
      { kind: "code", code: "docker run -d -p 127.0.0.1:5432:5432 postgres:16", caption: { en: "Reachable only from your own computer", pt: "Acessível só do seu próprio computador" } },
      {
        kind: "warn",
        text: {
          en: "-p 5432:5432 listens on all network interfaces, so other devices on your network may reach it. For databases and tools, bind to 127.0.0.1, as AutoDock does.",
          pt: "-p 5432:5432 escuta em todas as interfaces de rede, então outros dispositivos da sua rede podem alcançar. Para bancos e ferramentas, use 127.0.0.1, como o AutoDock faz.",
        },
      },
    ],
    tryIn: { page: "setup", label: { en: "See ports in the Docker assistant", pt: "Ver portas no assistente Docker" } },
    docs: [{ url: "https://docs.docker.com/engine/network/port-publishing/", label: { en: "Port publishing and mapping", pt: "Publicação e mapeamento de portas" } }],
  },
  {
    id: "volumes",
    level: "beginner",
    minutes: 6,
    title: { en: "Volumes: keeping your data", pt: "Volumes: guardando seus dados" },
    summary: {
      en: "Why databases need volumes and how bind mounts differ.",
      pt: "Por que bancos de dados precisam de volumes e no que os bind mounts diferem.",
    },
    blocks: [
      {
        kind: "p",
        text: {
          en: "Everything a container writes disappears with it. A volume is storage managed by Docker that outlives containers, so it is the right place for database files.",
          pt: "Tudo que um container grava some com ele. Um volume é um armazenamento gerenciado pelo Docker que sobrevive aos containers, por isso é o lugar certo para arquivos de banco de dados.",
        },
      },
      { kind: "code", code: "docker volume create pgdata\ndocker run -d -v pgdata:/var/lib/postgresql/data postgres:16", caption: { en: "Named volume", pt: "Volume nomeado" } },
      {
        kind: "p",
        text: {
          en: "A bind mount maps a folder of your computer into the container, which is handy to edit code live. A volume is for data Docker should own; a bind mount is for files you also touch by hand.",
          pt: "Um bind mount mapeia uma pasta do seu computador para dentro do container, útil para editar código ao vivo. Um volume é para dados que o Docker deve gerenciar; um bind mount é para arquivos que você também mexe à mão.",
        },
      },
      { kind: "code", code: "docker run -v \"$PWD\":/app -w /app node:22 npm test", caption: { en: "Bind mount of the current folder", pt: "Bind mount da pasta atual" } },
    ],
    docs: [{ url: "https://docs.docker.com/engine/storage/volumes/", label: { en: "Volumes", pt: "Volumes" } }],
  },
  {
    id: "compose",
    level: "intermediate",
    minutes: 10,
    title: { en: "Docker Compose: several services at once", pt: "Docker Compose: vários serviços de uma vez" },
    summary: {
      en: "Describe your app and its database in one file and start it with one command.",
      pt: "Descreva seu app e o banco em um arquivo e suba tudo com um comando.",
    },
    blocks: [
      {
        kind: "p",
        text: {
          en: "Compose describes services, ports, volumes and environment in a docker-compose.yml. It is what the Docker assistant generates for your project.",
          pt: "O Compose descreve serviços, portas, volumes e ambiente em um docker-compose.yml. É o que o assistente Docker gera para o seu projeto.",
        },
      },
      {
        kind: "code",
        code: "services:\n  app:\n    build: .\n    ports:\n      - \"127.0.0.1:3000:3000\"\n    env_file: .env.docker\n    depends_on: [db]\n  db:\n    image: postgres:16\n    env_file: .env.docker\n    volumes:\n      - dbdata:/var/lib/postgresql/data\nvolumes:\n  dbdata:",
        caption: { en: "An app with a database", pt: "Um app com banco de dados" },
      },
      { kind: "code", code: "docker compose up --build\ndocker compose logs -f app\ndocker compose down", caption: { en: "Start, follow the logs, stop", pt: "Subir, acompanhar os logs, parar" } },
      {
        kind: "tip",
        text: {
          en: "Keep passwords out of the compose file: put them in an env file (not committed to git) and reference it with env_file, like AutoDock does.",
          pt: "Não deixe senhas no arquivo compose: coloque em um arquivo env (fora do git) e referencie com env_file, como o AutoDock faz.",
        },
      },
    ],
    tryIn: { page: "setup", label: { en: "Generate a compose file", pt: "Gerar um arquivo compose" } },
    docs: [{ url: "https://docs.docker.com/compose/gettingstarted/", label: { en: "Get started with Compose", pt: "Primeiros passos com Compose" } }],
  },
  {
    id: "monitoring",
    level: "intermediate",
    minutes: 5,
    title: { en: "Reading CPU, memory and network", pt: "Lendo CPU, memória e rede" },
    summary: {
      en: "What the numbers mean and when to worry.",
      pt: "O que os números significam e quando se preocupar.",
    },
    blocks: [
      {
        kind: "p",
        text: {
          en: "CPU is shown per container and can exceed 100% when a container uses more than one core (200% means two full cores). Memory is compared with the container limit.",
          pt: "A CPU aparece por container e pode passar de 100% quando ele usa mais de um núcleo (200% são dois núcleos cheios). A memória é comparada com o limite do container.",
        },
      },
      { kind: "code", code: "docker stats --no-stream", caption: { en: "The same numbers in the terminal", pt: "Os mesmos números no terminal" } },
      {
        kind: "tip",
        text: {
          en: "A memory line that keeps climbing and never comes down usually points to a leak. Network totals only grow; the per-second chart shows the actual traffic.",
          pt: "Uma linha de memória que só sobe e nunca desce costuma indicar vazamento. Os totais de rede só crescem; o gráfico por segundo mostra o tráfego real.",
        },
      },
    ],
    tryIn: { page: "monitoring", label: { en: "Open Monitoring", pt: "Abrir o Monitoramento" } },
    docs: [{ url: "https://docs.docker.com/reference/cli/docker/container/stats/", label: { en: "docker stats", pt: "docker stats" } }],
  },
];
