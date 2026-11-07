// Lesson content for the Learn area. Every text is a { en, pt } pair so a lesson
// cannot be added in only one language (TypeScript fails) and tests check that
// both sides are filled in. Links go to official documentation only.
//
// Keep this file free of TypeScript-only runtime syntax (enums, parameter
// properties): the unit tests load it directly with Node's type stripping.

export type Bi = { en: string; pt: string };

/** Screens of AutoDock where the reader can see a concept for real. */
export type AppPage = "setup" | "containers" | "monitoring" | "console" | "environment";

export type Shell = "bash" | "powershell" | "cmd";

export type Block =
  | { kind: "p"; text: Bi }
  | { kind: "tip"; text: Bi }
  | { kind: "warn"; text: Bi }
  | { kind: "code"; code: string; caption?: Bi }
  /** A command whose spelling depends on the terminal; missing variants reuse bash. */
  | { kind: "shell"; caption?: Bi; bash: string; powershell?: string; cmd?: string }
  /** Ordered steps, shown numbered. */
  | { kind: "steps"; items: Bi[] }
  /** Plain bullet list. */
  | { kind: "list"; items: Bi[] };

export type Track = "start" | "build" | "operate";

export type QuizQuestion = {
  question: Bi;
  options: Bi[];
  /** Index of the correct option. */
  answer: number;
  why: Bi;
};

export type Lesson = {
  id: string;
  track: Track;
  level: "beginner" | "intermediate";
  minutes: number;
  title: Bi;
  summary: Bi;
  blocks: Block[];
  /** Short self-check shown at the end of the lesson. */
  quiz: QuizQuestion[];
  /** Screen of AutoDock where the reader can see the concept for real. */
  tryIn?: { page: AppPage; label: Bi };
  docs: { url: string; label: Bi }[];
};

export const TRACKS: { id: Track; title: Bi; description: Bi }[] = [
  {
    id: "start",
    title: { en: "First steps", pt: "Primeiros passos" },
    description: {
      en: "What Docker is, how to run it on your computer and the everyday commands.",
      pt: "O que é o Docker, como rodá-lo no seu computador e os comandos do dia a dia.",
    },
  },
  {
    id: "build",
    title: { en: "Building your own", pt: "Construindo o seu" },
    description: {
      en: "Package your app, configure it safely and run it with its database.",
      pt: "Empacote seu app, configure com segurança e rode junto com o banco de dados.",
    },
  },
  {
    id: "operate",
    title: { en: "Running and fixing", pt: "Operando e resolvendo" },
    description: {
      en: "Watch what is happening, find the cause of problems and keep your disk clean.",
      pt: "Acompanhe o que está acontecendo, ache a causa dos problemas e mantenha o disco limpo.",
    },
  },
];

export const LESSONS: Lesson[] = [
  // ───────────────────────── First steps ─────────────────────────
  {
    id: "what-is-docker",
    track: "start",
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
      {
        kind: "code",
        code: "docker --version\ndocker run hello-world",
        caption: { en: "Check the install and run your first container", pt: "Confira a instalação e rode seu primeiro container" },
      },
    ],
    quiz: [
      {
        question: { en: "What is the difference between an image and a container?", pt: "Qual é a diferença entre uma imagem e um container?" },
        options: [
          { en: "An image is a read-only recipe; a container is a running instance of it.", pt: "Imagem é uma receita somente leitura; container é uma instância dela em execução." },
          { en: "They are two names for the same thing.", pt: "São dois nomes para a mesma coisa." },
          { en: "A container is stored on disk and an image runs in memory.", pt: "O container fica no disco e a imagem roda na memória." },
        ],
        answer: 0,
        why: {
          en: "You build or pull an image once and can start as many containers from it as you like.",
          pt: "Você constrói ou baixa a imagem uma vez e pode iniciar quantos containers quiser a partir dela.",
        },
      },
      {
        question: { en: "Why does a container start faster than a virtual machine?", pt: "Por que um container inicia mais rápido que uma máquina virtual?" },
        options: [
          { en: "It shares the host kernel instead of booting its own operating system.", pt: "Ele compartilha o kernel do host em vez de iniciar um sistema operacional próprio." },
          { en: "It skips the network.", pt: "Ele dispensa a rede." },
          { en: "It runs only on SSD disks.", pt: "Ele só roda em discos SSD." },
        ],
        answer: 0,
        why: {
          en: "No second operating system has to boot, so a container is closer to a normal process.",
          pt: "Não há um segundo sistema operacional para iniciar, então o container fica mais próximo de um processo comum.",
        },
      },
    ],
    tryIn: { page: "console", label: { en: "Open the Console", pt: "Abrir o Console" } },
    docs: [{ url: "https://docs.docker.com/get-started/docker-overview/", label: { en: "Docker overview", pt: "Visão geral do Docker" } }],
  },
  {
    id: "docker-windows",
    track: "start",
    level: "beginner",
    minutes: 8,
    title: { en: "Docker on Windows", pt: "Docker no Windows" },
    summary: {
      en: "Docker Desktop, WSL 2, paths and the small differences that trip people up.",
      pt: "Docker Desktop, WSL 2, caminhos e as pequenas diferenças que costumam atrapalhar.",
    },
    blocks: [
      {
        kind: "p",
        text: {
          en: "On Windows, Docker Desktop runs your containers inside a lightweight Linux environment (WSL 2 by default). So the containers are Linux containers, even though you type the commands in Windows.",
          pt: "No Windows, o Docker Desktop roda seus containers dentro de um ambiente Linux leve (WSL 2, por padrão). Ou seja, os containers são Linux, mesmo que você digite os comandos no Windows.",
        },
      },
      {
        kind: "steps",
        items: [
          { en: "Install Docker Desktop and keep the WSL 2 option enabled.", pt: "Instale o Docker Desktop e deixe a opção WSL 2 ativada." },
          { en: "Open Docker Desktop and wait until it reports that the engine is running.", pt: "Abra o Docker Desktop e espere ele informar que o engine está rodando." },
          { en: "In PowerShell run docker version: both Client and Server must appear.", pt: "No PowerShell rode docker version: Client e Server precisam aparecer." },
        ],
      },
      { kind: "code", code: "docker version\ndocker info", caption: { en: "If Server is missing, Docker Desktop is not running yet", pt: "Se o Server não aparecer, o Docker Desktop ainda não está rodando" } },
      {
        kind: "p",
        text: {
          en: "The same command is written differently in each terminal. Use the tabs to pick yours; AutoDock remembers the choice.",
          pt: "O mesmo comando se escreve diferente em cada terminal. Use as abas para escolher o seu; o AutoDock lembra da escolha.",
        },
      },
      {
        kind: "shell",
        caption: { en: "Mount the current folder inside a container", pt: "Montar a pasta atual dentro de um container" },
        bash: "docker run --rm -v \"$PWD\":/app -w /app node:22 node --version",
        powershell: "docker run --rm -v \"${PWD}:/app\" -w /app node:22 node --version",
        cmd: "docker run --rm -v \"%cd%:/app\" -w /app node:22 node --version",
      },
      {
        kind: "warn",
        text: {
          en: "Git Bash rewrites arguments that start with / (so /app becomes a Windows path). Set MSYS_NO_PATHCONV=1 before the command, or write //app.",
          pt: "O Git Bash reescreve argumentos que começam com / (o /app vira um caminho do Windows). Defina MSYS_NO_PATHCONV=1 antes do comando ou escreva //app.",
        },
      },
      {
        kind: "warn",
        text: {
          en: "Scripts saved with Windows line endings (CRLF) fail inside Linux containers with errors like \"bad interpreter\" or \"no such file or directory\". Keep shell scripts in LF.",
          pt: "Scripts salvos com final de linha do Windows (CRLF) falham dentro de containers Linux com erros como \"bad interpreter\" ou \"no such file or directory\". Mantenha scripts de shell em LF.",
        },
      },
      { kind: "code", code: "* text=auto\n*.sh text eol=lf", caption: { en: ".gitattributes that keeps shell scripts in LF", pt: ".gitattributes que mantém os scripts de shell em LF" } },
      {
        kind: "tip",
        text: {
          en: "Projects with many small files (node_modules) are much faster when they live inside the WSL 2 file system than in a Windows folder mounted into the container.",
          pt: "Projetos com muitos arquivos pequenos (node_modules) ficam bem mais rápidos quando estão dentro do sistema de arquivos do WSL 2 do que em uma pasta do Windows montada no container.",
        },
      },
      {
        kind: "p",
        text: {
          en: "A published port is reachable from Windows at localhost. From inside a container, a program running on your Windows host is reachable at host.docker.internal, because localhost there means the container itself.",
          pt: "Uma porta publicada fica acessível no Windows em localhost. De dentro de um container, um programa rodando no seu Windows é acessível em host.docker.internal, porque ali localhost é o próprio container.",
        },
      },
    ],
    quiz: [
      {
        question: { en: "Which kind of containers does Docker Desktop on Windows run by default?", pt: "Que tipo de container o Docker Desktop roda por padrão no Windows?" },
        options: [
          { en: "Linux containers, inside WSL 2.", pt: "Containers Linux, dentro do WSL 2." },
          { en: "Native Windows executables only.", pt: "Somente executáveis nativos do Windows." },
          { en: "macOS containers.", pt: "Containers macOS." },
        ],
        answer: 0,
        why: {
          en: "That is why paths inside the container look like /app and scripts need LF line endings.",
          pt: "Por isso os caminhos dentro do container são como /app e os scripts precisam de final de linha LF.",
        },
      },
      {
        question: { en: "Inside a container, how do you reach a service running on your Windows computer?", pt: "De dentro de um container, como acessar um serviço rodando no seu computador Windows?" },
        options: [
          { en: "host.docker.internal", pt: "host.docker.internal" },
          { en: "localhost", pt: "localhost" },
          { en: "It is not possible.", pt: "Não é possível." },
        ],
        answer: 0,
        why: {
          en: "Inside a container, localhost points to the container itself, not to your computer.",
          pt: "Dentro de um container, localhost aponta para o próprio container, não para o seu computador.",
        },
      },
    ],
    tryIn: { page: "environment", label: { en: "Check my environment", pt: "Verificar meu ambiente" } },
    docs: [
      { url: "https://docs.docker.com/desktop/setup/install/windows-install/", label: { en: "Install Docker Desktop on Windows", pt: "Instalar o Docker Desktop no Windows" } },
      { url: "https://docs.docker.com/desktop/features/wsl/", label: { en: "Docker Desktop and WSL 2", pt: "Docker Desktop e WSL 2" } },
    ],
  },
  {
    id: "images-containers",
    track: "start",
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
        kind: "p",
        text: {
          en: "An image name looks like repository:tag. The tag picks a version (nginx:alpine, postgres:16). Without a tag Docker uses latest, which can change under you: pin a version for anything that matters.",
          pt: "O nome de uma imagem é repositório:tag. A tag escolhe a versão (nginx:alpine, postgres:16). Sem tag o Docker usa latest, que pode mudar sem aviso: fixe uma versão em tudo que importa.",
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
    quiz: [
      {
        question: { en: "What does the -d flag do in docker run -d?", pt: "O que a opção -d faz em docker run -d?" },
        options: [
          { en: "Runs the container in the background.", pt: "Roda o container em segundo plano." },
          { en: "Deletes the container when it stops.", pt: "Apaga o container quando ele para." },
          { en: "Downloads the image again.", pt: "Baixa a imagem de novo." },
        ],
        answer: 0,
        why: {
          en: "-d means detached. To delete the container on exit you would use --rm.",
          pt: "-d significa destacado (detached). Para apagar o container ao sair você usaria --rm.",
        },
      },
      {
        question: { en: "Which command also lists containers that already stopped?", pt: "Qual comando lista também os containers que já pararam?" },
        options: [
          { en: "docker ps -a", pt: "docker ps -a" },
          { en: "docker ps", pt: "docker ps" },
          { en: "docker images", pt: "docker images" },
        ],
        answer: 0,
        why: {
          en: "docker ps alone shows only running containers; -a adds the stopped ones.",
          pt: "docker ps sozinho mostra só os que estão rodando; o -a inclui os parados.",
        },
      },
    ],
    tryIn: { page: "containers", label: { en: "See your containers", pt: "Ver seus containers" } },
    docs: [{ url: "https://docs.docker.com/reference/cli/docker/", label: { en: "Docker CLI reference", pt: "Referência da CLI do Docker" } }],
  },
  {
    id: "ports",
    track: "start",
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
      {
        kind: "tip",
        text: {
          en: "EXPOSE in a Dockerfile only documents a port. Nothing is reachable from your computer until you publish it with -p or with ports: in Compose.",
          pt: "O EXPOSE em um Dockerfile só documenta a porta. Nada fica acessível no seu computador até você publicar com -p ou com ports: no Compose.",
        },
      },
    ],
    quiz: [
      {
        question: { en: "In -p 8080:80, which number is the container port?", pt: "Em -p 8080:80, qual número é a porta do container?" },
        options: [
          { en: "80", pt: "80" },
          { en: "8080", pt: "8080" },
          { en: "Both are.", pt: "Os dois são." },
        ],
        answer: 0,
        why: {
          en: "The format is host:container, so your browser opens 8080 and Docker forwards it to 80 inside.",
          pt: "O formato é host:container, então o navegador abre 8080 e o Docker encaminha para a 80 lá dentro.",
        },
      },
      {
        question: { en: "Why bind a database to 127.0.0.1:5432 instead of 5432?", pt: "Por que publicar um banco em 127.0.0.1:5432 em vez de 5432?" },
        options: [
          { en: "So only your own computer can reach it.", pt: "Para que só o seu computador consiga acessar." },
          { en: "It makes the database faster.", pt: "Deixa o banco mais rápido." },
          { en: "Docker requires it.", pt: "O Docker exige." },
        ],
        answer: 0,
        why: {
          en: "Without 127.0.0.1 the port listens on every network interface and other devices may connect.",
          pt: "Sem o 127.0.0.1 a porta escuta em todas as interfaces e outros dispositivos podem se conectar.",
        },
      },
    ],
    tryIn: { page: "setup", label: { en: "See ports in the Docker assistant", pt: "Ver portas no assistente Docker" } },
    docs: [{ url: "https://docs.docker.com/engine/network/port-publishing/", label: { en: "Port publishing and mapping", pt: "Publicação e mapeamento de portas" } }],
  },
  {
    id: "volumes",
    track: "start",
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
      {
        kind: "shell",
        caption: { en: "Bind mount of the current folder", pt: "Bind mount da pasta atual" },
        bash: "docker run --rm -v \"$PWD\":/app -w /app node:22 npm test",
        powershell: "docker run --rm -v \"${PWD}:/app\" -w /app node:22 npm test",
        cmd: "docker run --rm -v \"%cd%:/app\" -w /app node:22 npm test",
      },
      {
        kind: "warn",
        text: {
          en: "docker compose down -v also deletes the named volumes of the project, including the database files. Use plain docker compose down to keep your data.",
          pt: "docker compose down -v também apaga os volumes nomeados do projeto, incluindo os arquivos do banco. Use só docker compose down para manter seus dados.",
        },
      },
    ],
    quiz: [
      {
        question: { en: "Where should a database keep its files so they survive docker rm?", pt: "Onde um banco de dados deve guardar os arquivos para sobreviverem ao docker rm?" },
        options: [
          { en: "In a volume.", pt: "Em um volume." },
          { en: "In the container's writable layer.", pt: "Na camada gravável do container." },
          { en: "In the image.", pt: "Na imagem." },
        ],
        answer: 0,
        why: {
          en: "The writable layer is deleted with the container; a volume outlives it.",
          pt: "A camada gravável é apagada junto com o container; o volume continua existindo.",
        },
      },
      {
        question: { en: "When is a bind mount the better choice?", pt: "Quando um bind mount é a melhor escolha?" },
        options: [
          { en: "To edit your source code on the computer and see it inside the container.", pt: "Para editar o código no seu computador e ver dentro do container." },
          { en: "To store production database files.", pt: "Para guardar os arquivos do banco em produção." },
          { en: "To make the image smaller.", pt: "Para deixar a imagem menor." },
        ],
        answer: 0,
        why: {
          en: "A bind mount shares a real folder of yours; Docker-managed volumes are better for data you do not touch by hand.",
          pt: "O bind mount compartilha uma pasta real sua; volumes gerenciados pelo Docker são melhores para dados em que você não mexe à mão.",
        },
      },
    ],
    docs: [{ url: "https://docs.docker.com/engine/storage/volumes/", label: { en: "Volumes", pt: "Volumes" } }],
  },

  // ───────────────────────── Building your own ─────────────────────────
  {
    id: "dockerfile",
    track: "build",
    level: "intermediate",
    minutes: 10,
    title: { en: "Writing a Dockerfile", pt: "Escrevendo um Dockerfile" },
    summary: {
      en: "Turn your project into an image, layer by layer, and keep builds fast.",
      pt: "Transforme seu projeto em uma imagem, camada por camada, e mantenha os builds rápidos.",
    },
    blocks: [
      {
        kind: "p",
        text: {
          en: "A Dockerfile is the recipe for an image. Each instruction creates a layer, and Docker reuses a layer from cache when nothing before it changed.",
          pt: "Um Dockerfile é a receita de uma imagem. Cada instrução cria uma camada, e o Docker reaproveita a camada do cache quando nada antes dela mudou.",
        },
      },
      {
        kind: "code",
        code: "FROM node:22-alpine\nWORKDIR /app\nCOPY package*.json ./\nRUN npm ci\nCOPY . .\nEXPOSE 3000\nCMD [\"node\", \"server.js\"]",
        caption: { en: "A Node.js application", pt: "Uma aplicação Node.js" },
      },
      {
        kind: "list",
        items: [
          { en: "FROM picks the base image to start from.", pt: "FROM escolhe a imagem base de onde partir." },
          { en: "WORKDIR sets the folder where the next commands run.", pt: "WORKDIR define a pasta onde os próximos comandos rodam." },
          { en: "COPY brings files from your project into the image; RUN executes a command while building.", pt: "COPY traz arquivos do seu projeto para a imagem; RUN executa um comando durante o build." },
          { en: "CMD is the command the container runs when it starts.", pt: "CMD é o comando que o container executa ao iniciar." },
        ],
      },
      {
        kind: "tip",
        text: {
          en: "Copy the dependency files first and install, then copy the rest. Editing your code then reuses the cached dependency layer and rebuilds in seconds.",
          pt: "Copie primeiro os arquivos de dependências e instale; depois copie o resto. Assim, ao editar o código, a camada de dependências vem do cache e o build leva segundos.",
        },
      },
      { kind: "code", code: "docker build -t myapp .\ndocker run --rm -p 3000:3000 myapp", caption: { en: "Build the image and run it", pt: "Construir a imagem e rodá-la" } },
      { kind: "code", code: "node_modules\n.git\n.env\n*.log", caption: { en: ".dockerignore keeps heavy and private files out of the build", pt: "O .dockerignore deixa arquivos pesados e privados fora do build" } },
      {
        kind: "warn",
        text: {
          en: "Anything copied into an image can be read by whoever gets the image. Never COPY a .env file or private keys into it.",
          pt: "Tudo que é copiado para uma imagem pode ser lido por quem receber a imagem. Nunca use COPY com arquivo .env ou chaves privadas.",
        },
      },
    ],
    quiz: [
      {
        question: { en: "Why copy package.json before the rest of the code?", pt: "Por que copiar o package.json antes do resto do código?" },
        options: [
          { en: "So the dependency layer stays cached when only your code changes.", pt: "Para a camada de dependências continuar no cache quando só o código muda." },
          { en: "Because Docker requires alphabetical order.", pt: "Porque o Docker exige ordem alfabética." },
          { en: "To make the container run faster.", pt: "Para o container rodar mais rápido." },
        ],
        answer: 0,
        why: {
          en: "A layer is rebuilt only when it or an earlier layer changes, so stable steps go first.",
          pt: "Uma camada só é refeita quando ela ou uma anterior muda, então os passos estáveis vêm primeiro.",
        },
      },
      {
        question: { en: "What does a .dockerignore file do?", pt: "O que faz o arquivo .dockerignore?" },
        options: [
          { en: "Excludes files and folders from what is sent to the build.", pt: "Exclui arquivos e pastas do que é enviado ao build." },
          { en: "Stops a container from starting.", pt: "Impede um container de iniciar." },
          { en: "Hides the image from docker images.", pt: "Esconde a imagem do docker images." },
        ],
        answer: 0,
        why: {
          en: "It works like .gitignore: it keeps node_modules, .git and secrets out of the image.",
          pt: "Funciona como o .gitignore: deixa node_modules, .git e segredos fora da imagem.",
        },
      },
    ],
    tryIn: { page: "setup", label: { en: "Generate a Dockerfile", pt: "Gerar um Dockerfile" } },
    docs: [
      { url: "https://docs.docker.com/get-started/docker-concepts/building-images/writing-a-dockerfile/", label: { en: "Writing a Dockerfile", pt: "Escrevendo um Dockerfile" } },
      { url: "https://docs.docker.com/reference/dockerfile/", label: { en: "Dockerfile reference", pt: "Referência do Dockerfile" } },
    ],
  },
  {
    id: "env-secrets",
    track: "build",
    level: "intermediate",
    minutes: 6,
    title: { en: "Environment variables and secrets", pt: "Variáveis de ambiente e segredos" },
    summary: {
      en: "Configure a container without baking passwords into images or git.",
      pt: "Configure um container sem deixar senhas dentro de imagens ou do git.",
    },
    blocks: [
      {
        kind: "p",
        text: {
          en: "Environment variables are how you give a container its settings: database address, mode, passwords. The same image then works in development and in production.",
          pt: "Variáveis de ambiente são o jeito de dar configurações a um container: endereço do banco, modo, senhas. Assim a mesma imagem serve em desenvolvimento e em produção.",
        },
      },
      {
        kind: "code",
        code: "docker run -d -e POSTGRES_PASSWORD=change-me --name db postgres:16\ndocker run -d --env-file .env --name api myapp",
        caption: { en: "Pass variables to a container", pt: "Passar variáveis para um container" },
      },
      { kind: "code", code: "POSTGRES_USER=app\nPOSTGRES_PASSWORD=a-long-random-value\nDATABASE_URL=postgres://app:a-long-random-value@db:5432/app", caption: { en: "An env file (.env): one KEY=value per line", pt: "Um arquivo env (.env): uma linha CHAVE=valor" } },
      {
        kind: "warn",
        text: {
          en: "Do not commit the .env file. Commit a .env.example with the names only. A password written with ENV in a Dockerfile stays inside the image history for anyone to read.",
          pt: "Não versione o arquivo .env. Versione um .env.example só com os nomes. Uma senha escrita com ENV em um Dockerfile fica no histórico da imagem para qualquer um ler.",
        },
      },
      {
        kind: "p",
        text: {
          en: "AutoDock follows this pattern: it generates strong passwords into .env.docker, references it with env_file in the compose file and lists it in .gitignore.",
          pt: "O AutoDock segue esse padrão: gera senhas fortes em .env.docker, referencia no compose com env_file e coloca o arquivo no .gitignore.",
        },
      },
      {
        kind: "tip",
        text: {
          en: "Anyone who can run docker inspect on a container can see its variables. Treat the Docker socket like administrator access.",
          pt: "Quem consegue rodar docker inspect em um container enxerga as variáveis dele. Trate o acesso ao Docker como acesso de administrador.",
        },
      },
    ],
    quiz: [
      {
        question: { en: "Where should a database password live?", pt: "Onde a senha de um banco deve ficar?" },
        options: [
          { en: "In an env file that is not committed to git.", pt: "Em um arquivo env fora do git." },
          { en: "In an ENV line of the Dockerfile.", pt: "Em uma linha ENV do Dockerfile." },
          { en: "In the README.", pt: "No README." },
        ],
        answer: 0,
        why: {
          en: "Files and image layers are copied around; an uncommitted env file stays on your machine.",
          pt: "Arquivos e camadas de imagem são copiados por aí; um arquivo env fora do git fica só na sua máquina.",
        },
      },
      {
        question: { en: "What is a .env.example for?", pt: "Para que serve um .env.example?" },
        options: [
          { en: "To show which variables exist, without real values.", pt: "Mostrar quais variáveis existem, sem os valores reais." },
          { en: "To store the production secrets safely.", pt: "Guardar os segredos de produção com segurança." },
          { en: "To speed up the build.", pt: "Acelerar o build." },
        ],
        answer: 0,
        why: {
          en: "Whoever clones the project copies it to .env and fills in their own values.",
          pt: "Quem clonar o projeto copia para .env e preenche os próprios valores.",
        },
      },
    ],
    tryIn: { page: "setup", label: { en: "See the generated env files", pt: "Ver os arquivos env gerados" } },
    docs: [{ url: "https://docs.docker.com/compose/how-tos/environment-variables/", label: { en: "Environment variables in Compose", pt: "Variáveis de ambiente no Compose" } }],
  },
  {
    id: "compose",
    track: "build",
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
      {
        kind: "warn",
        text: {
          en: "depends_on only controls the start order. It does not wait for the database to be ready to accept connections: use a healthcheck or let the app retry.",
          pt: "depends_on só controla a ordem de início. Ele não espera o banco estar pronto para aceitar conexões: use um healthcheck ou faça o app tentar de novo.",
        },
      },
    ],
    quiz: [
      {
        question: { en: "Which command starts every service of docker-compose.yml in the background?", pt: "Qual comando sobe todos os serviços do docker-compose.yml em segundo plano?" },
        options: [
          { en: "docker compose up -d", pt: "docker compose up -d" },
          { en: "docker compose down", pt: "docker compose down" },
          { en: "docker compose ls", pt: "docker compose ls" },
        ],
        answer: 0,
        why: {
          en: "up creates and starts the services; -d detaches. down stops and removes them.",
          pt: "up cria e inicia os serviços; -d deixa em segundo plano. down para e remove.",
        },
      },
      {
        question: { en: "Does depends_on guarantee the database is ready to accept connections?", pt: "O depends_on garante que o banco está pronto para aceitar conexões?" },
        options: [
          { en: "No, only that it started first.", pt: "Não, só que ele iniciou antes." },
          { en: "Yes, always.", pt: "Sim, sempre." },
          { en: "Only for Postgres.", pt: "Só para o Postgres." },
        ],
        answer: 0,
        why: {
          en: "Started does not mean ready. Combine it with a healthcheck or retries in the app.",
          pt: "Iniciado não significa pronto. Combine com um healthcheck ou tentativas no app.",
        },
      },
    ],
    tryIn: { page: "setup", label: { en: "Generate a compose file", pt: "Gerar um arquivo compose" } },
    docs: [{ url: "https://docs.docker.com/compose/gettingstarted/", label: { en: "Get started with Compose", pt: "Primeiros passos com Compose" } }],
  },
  {
    id: "networks",
    track: "build",
    level: "intermediate",
    minutes: 7,
    title: { en: "Networks: how containers talk", pt: "Redes: como os containers conversam" },
    summary: {
      en: "Service names, why localhost fails inside a container and how to reach the host.",
      pt: "Nomes de serviço, por que localhost falha dentro de um container e como acessar o host.",
    },
    blocks: [
      {
        kind: "p",
        text: {
          en: "Compose puts the services of a project on one private network. Inside it, a container reaches another by the service name: the app connects to db:5432, not to an IP address.",
          pt: "O Compose coloca os serviços de um projeto em uma rede privada. Dentro dela, um container acessa outro pelo nome do serviço: o app conecta em db:5432, não em um IP.",
        },
      },
      { kind: "code", code: "DATABASE_URL=postgres://app:secret@db:5432/app", caption: { en: "db is the service name in docker-compose.yml", pt: "db é o nome do serviço no docker-compose.yml" } },
      {
        kind: "warn",
        text: {
          en: "Inside a container, localhost is the container itself. An app that connects to localhost:5432 will not find a database that runs in another container.",
          pt: "Dentro de um container, localhost é o próprio container. Um app que conecta em localhost:5432 não encontra um banco que roda em outro container.",
        },
      },
      {
        kind: "p",
        text: {
          en: "The published port (127.0.0.1:5432 on your computer) is for you and your tools outside Docker. Containers on the same network do not need it.",
          pt: "A porta publicada (127.0.0.1:5432 no seu computador) é para você e suas ferramentas fora do Docker. Containers na mesma rede não precisam dela.",
        },
      },
      { kind: "code", code: "docker network ls\ndocker network inspect <network-name>", caption: { en: "List networks and see who is connected", pt: "Listar redes e ver quem está conectado" } },
      {
        kind: "tip",
        text: {
          en: "To reach a program on your own computer from a container use host.docker.internal. It works in Docker Desktop on Windows and macOS; on Linux add --add-host=host.docker.internal:host-gateway.",
          pt: "Para acessar um programa do seu computador a partir de um container use host.docker.internal. Funciona no Docker Desktop do Windows e do macOS; no Linux adicione --add-host=host.docker.internal:host-gateway.",
        },
      },
    ],
    quiz: [
      {
        question: { en: "The app container must connect to the database container. What host name does it use?", pt: "O container do app precisa conectar ao container do banco. Qual nome de host ele usa?" },
        options: [
          { en: "The service name, for example db.", pt: "O nome do serviço, por exemplo db." },
          { en: "localhost", pt: "localhost" },
          { en: "The IP of your Wi-Fi.", pt: "O IP do seu Wi-Fi." },
        ],
        answer: 0,
        why: {
          en: "Compose registers each service name as a host name on the project network.",
          pt: "O Compose registra o nome de cada serviço como nome de host na rede do projeto.",
        },
      },
      {
        question: { en: "What does localhost mean inside a container?", pt: "O que significa localhost dentro de um container?" },
        options: [
          { en: "The container itself.", pt: "O próprio container." },
          { en: "Your computer.", pt: "O seu computador." },
          { en: "The internet.", pt: "A internet." },
        ],
        answer: 0,
        why: {
          en: "Each container has its own network, so localhost never leaves it.",
          pt: "Cada container tem a própria rede, então localhost nunca sai dele.",
        },
      },
    ],
    docs: [
      { url: "https://docs.docker.com/compose/how-tos/networking/", label: { en: "Networking in Compose", pt: "Redes no Compose" } },
      { url: "https://docs.docker.com/engine/network/", label: { en: "Networking overview", pt: "Visão geral de redes" } },
    ],
  },

  // ───────────────────────── Running and fixing ─────────────────────────
  {
    id: "monitoring",
    track: "operate",
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
    quiz: [
      {
        question: { en: "A container shows 200% CPU. What does it mean?", pt: "Um container mostra 200% de CPU. O que significa?" },
        options: [
          { en: "It uses about two CPU cores fully.", pt: "Ele usa cerca de dois núcleos de CPU por inteiro." },
          { en: "Something is broken.", pt: "Algo está quebrado." },
          { en: "The reading is wrong.", pt: "A leitura está errada." },
        ],
        answer: 0,
        why: {
          en: "100% is one full core, so a busy multi-threaded container can go above it.",
          pt: "100% é um núcleo inteiro, então um container multithread ocupado pode passar disso.",
        },
      },
      {
        question: { en: "Memory that only goes up and never comes back down usually suggests…", pt: "Memória que só sobe e nunca volta a descer costuma indicar…" },
        options: [
          { en: "a memory leak.", pt: "um vazamento de memória." },
          { en: "a healthy container.", pt: "um container saudável." },
          { en: "a network problem.", pt: "um problema de rede." },
        ],
        answer: 0,
        why: {
          en: "Healthy services rise and fall with load. A steady climb is worth investigating.",
          pt: "Serviços saudáveis sobem e descem com a carga. Uma subida constante merece investigação.",
        },
      },
    ],
    tryIn: { page: "monitoring", label: { en: "Open Monitoring", pt: "Abrir o Monitoramento" } },
    docs: [{ url: "https://docs.docker.com/reference/cli/docker/container/stats/", label: { en: "docker stats", pt: "docker stats" } }],
  },
  {
    id: "debugging",
    track: "operate",
    level: "intermediate",
    minutes: 8,
    title: { en: "Logs and debugging", pt: "Logs e depuração" },
    summary: {
      en: "Read the logs, get inside a container and understand why it stopped.",
      pt: "Leia os logs, entre em um container e entenda por que ele parou.",
    },
    blocks: [
      {
        kind: "p",
        text: {
          en: "When something is wrong, start with the logs: they show what the application printed. Then look at the state of the container.",
          pt: "Quando algo dá errado, comece pelos logs: eles mostram o que a aplicação imprimiu. Depois olhe o estado do container.",
        },
      },
      { kind: "code", code: "docker logs --tail 100 web\ndocker logs -f web", caption: { en: "Last 100 lines, then follow live (Ctrl+C leaves)", pt: "Últimas 100 linhas e depois acompanhar ao vivo (Ctrl+C sai)" } },
      { kind: "code", code: "docker exec -it web sh", caption: { en: "Open a shell inside a running container", pt: "Abrir um shell dentro de um container em execução" } },
      {
        kind: "shell",
        caption: { en: "Why did it stop? Exit code and out-of-memory flag", pt: "Por que parou? Código de saída e indicação de falta de memória" },
        bash: "docker inspect --format '{{.State.ExitCode}} {{.State.OOMKilled}}' web",
        powershell: "docker inspect --format '{{.State.ExitCode}} {{.State.OOMKilled}}' web",
        cmd: "docker inspect --format \"{{.State.ExitCode}} {{.State.OOMKilled}}\" web",
      },
      {
        kind: "list",
        items: [
          { en: "Exit code 0: the program finished on its own, which is normal for a one-off task.", pt: "Código 0: o programa terminou sozinho, normal para uma tarefa pontual." },
          { en: "Exit code 1 (or another small number): the application failed; the logs say why.", pt: "Código 1 (ou outro número pequeno): a aplicação falhou; os logs dizem o motivo." },
          { en: "Exit code 137: the container was killed, often for using too much memory (OOMKilled true).", pt: "Código 137: o container foi morto, muitas vezes por usar memória demais (OOMKilled true)." },
          { en: "Exit code 143: it received a normal stop request (docker stop).", pt: "Código 143: ele recebeu um pedido normal de parada (docker stop)." },
        ],
      },
      {
        kind: "tip",
        text: {
          en: "A container that starts and exits immediately usually has no long-running foreground process, or it crashed on startup. docker logs shows the last words.",
          pt: "Um container que inicia e sai na hora costuma não ter um processo de primeiro plano de longa duração, ou quebrou ao iniciar. O docker logs mostra as últimas palavras.",
        },
      },
      {
        kind: "p",
        text: {
          en: "To restart automatically after a crash or after the computer boots, run with --restart unless-stopped (or restart: unless-stopped in Compose).",
          pt: "Para reiniciar sozinho após uma falha ou quando o computador ligar, rode com --restart unless-stopped (ou restart: unless-stopped no Compose).",
        },
      },
    ],
    quiz: [
      {
        question: { en: "A container exited with code 137. What is the most likely cause?", pt: "Um container saiu com código 137. Qual a causa mais provável?" },
        options: [
          { en: "It was killed, often because it ran out of memory.", pt: "Ele foi morto, muitas vezes por falta de memória." },
          { en: "It finished its work successfully.", pt: "Ele terminou o trabalho com sucesso." },
          { en: "The image does not exist.", pt: "A imagem não existe." },
        ],
        answer: 0,
        why: {
          en: "137 is 128 + 9 (SIGKILL). Check OOMKilled in docker inspect.",
          pt: "137 é 128 + 9 (SIGKILL). Confira o OOMKilled no docker inspect.",
        },
      },
      {
        question: { en: "Which command opens a shell inside a running container?", pt: "Qual comando abre um shell dentro de um container em execução?" },
        options: [
          { en: "docker exec -it web sh", pt: "docker exec -it web sh" },
          { en: "docker run web sh", pt: "docker run web sh" },
          { en: "docker logs web sh", pt: "docker logs web sh" },
        ],
        answer: 0,
        why: {
          en: "exec runs a command in an existing container; run would create a new one.",
          pt: "exec roda um comando em um container existente; run criaria um novo.",
        },
      },
    ],
    tryIn: { page: "console", label: { en: "Open the Console (Container logs)", pt: "Abrir o Console (Logs de containers)" } },
    docs: [
      { url: "https://docs.docker.com/reference/cli/docker/container/logs/", label: { en: "docker logs", pt: "docker logs" } },
      { url: "https://docs.docker.com/reference/cli/docker/container/exec/", label: { en: "docker exec", pt: "docker exec" } },
      { url: "https://docs.docker.com/engine/containers/start-containers-automatically/", label: { en: "Start containers automatically", pt: "Iniciar containers automaticamente" } },
    ],
  },
  {
    id: "cleanup",
    track: "operate",
    level: "intermediate",
    minutes: 5,
    title: { en: "Cleaning up disk space", pt: "Limpando o espaço em disco" },
    summary: {
      en: "Where Docker's disk usage comes from and how to free it without losing data.",
      pt: "De onde vem o uso de disco do Docker e como liberá-lo sem perder dados.",
    },
    blocks: [
      {
        kind: "p",
        text: {
          en: "Images, stopped containers, volumes and build cache pile up over time. Look at what uses space before you delete anything.",
          pt: "Imagens, containers parados, volumes e cache de build se acumulam com o tempo. Veja o que ocupa espaço antes de apagar qualquer coisa.",
        },
      },
      { kind: "code", code: "docker system df", caption: { en: "Space used by images, containers, volumes and cache", pt: "Espaço usado por imagens, containers, volumes e cache" } },
      { kind: "code", code: "docker container prune\ndocker image prune\ndocker builder prune", caption: { en: "Remove stopped containers, unused images and build cache", pt: "Remover containers parados, imagens sem uso e cache de build" } },
      {
        kind: "warn",
        text: {
          en: "Pruning deletes things for good. docker volume prune and docker system prune --volumes can remove database data you still need. Read the confirmation prompt and check your volumes first.",
          pt: "Limpar apaga de vez. docker volume prune e docker system prune --volumes podem remover dados de banco de que você ainda precisa. Leia a confirmação e confira seus volumes antes.",
        },
      },
      {
        kind: "tip",
        text: {
          en: "On Docker Desktop with WSL 2 the virtual disk may stay large after cleaning. Docker Desktop can reclaim it in Settings, under Resources.",
          pt: "No Docker Desktop com WSL 2 o disco virtual pode continuar grande depois da limpeza. O Docker Desktop consegue recuperar o espaço em Settings, em Resources.",
        },
      },
    ],
    quiz: [
      {
        question: { en: "What should you do before running docker volume prune?", pt: "O que fazer antes de rodar docker volume prune?" },
        options: [
          { en: "Check which volumes exist and whether any holds data you need.", pt: "Conferir quais volumes existem e se algum guarda dados de que você precisa." },
          { en: "Nothing, it is always safe.", pt: "Nada, é sempre seguro." },
          { en: "Restart the computer.", pt: "Reiniciar o computador." },
        ],
        answer: 0,
        why: {
          en: "A volume without a running container may still hold your database.",
          pt: "Um volume sem container rodando ainda pode guardar o seu banco de dados.",
        },
      },
      {
        question: { en: "Which command shows how much space Docker uses?", pt: "Qual comando mostra quanto espaço o Docker usa?" },
        options: [
          { en: "docker system df", pt: "docker system df" },
          { en: "docker stats", pt: "docker stats" },
          { en: "docker top", pt: "docker top" },
        ],
        answer: 0,
        why: {
          en: "df reports disk usage by type, and shows how much of it can be reclaimed.",
          pt: "O df mostra o uso de disco por tipo e quanto dele pode ser recuperado.",
        },
      },
    ],
    docs: [{ url: "https://docs.docker.com/engine/manage-resources/pruning/", label: { en: "Prune unused Docker objects", pt: "Limpar objetos Docker sem uso" } }],
  },
  {
    id: "troubleshooting",
    track: "operate",
    level: "intermediate",
    minutes: 9,
    title: { en: "Fixing common errors", pt: "Resolvendo erros comuns" },
    summary: {
      en: "The messages you will meet most often and what to do about each one.",
      pt: "As mensagens que você mais vai encontrar e o que fazer em cada uma.",
    },
    blocks: [
      {
        kind: "p",
        text: {
          en: "Read the whole message, the useful part is usually at the end. These are the cases that cover most problems.",
          pt: "Leia a mensagem inteira, a parte útil costuma estar no final. Estes casos cobrem a maioria dos problemas.",
        },
      },
      {
        kind: "warn",
        text: {
          en: "\"Cannot connect to the Docker daemon\" or \"error during connect ... pipe ... docker\": the engine is not running. Open Docker Desktop (or start the Docker service) and wait until it is ready.",
          pt: "\"Cannot connect to the Docker daemon\" ou \"error during connect ... pipe ... docker\": o engine não está rodando. Abra o Docker Desktop (ou inicie o serviço do Docker) e espere ficar pronto.",
        },
      },
      {
        kind: "p",
        text: {
          en: "\"port is already allocated\" or \"address already in use\": another program already uses that host port. Find it, or choose another port.",
          pt: "\"port is already allocated\" ou \"address already in use\": outro programa já usa essa porta do host. Descubra qual, ou escolha outra porta.",
        },
      },
      {
        kind: "shell",
        caption: { en: "Who is using port 8080?", pt: "Quem está usando a porta 8080?" },
        bash: "lsof -i :8080",
        powershell: "Get-NetTCPConnection -LocalPort 8080 | Select-Object LocalAddress, State, OwningProcess",
        cmd: "netstat -ano | findstr :8080",
      },
      {
        kind: "p",
        text: {
          en: "\"pull access denied\" or \"manifest unknown\": the image name or tag is wrong, or it is private and you are not logged in. Check the spelling on the registry page.",
          pt: "\"pull access denied\" ou \"manifest unknown\": o nome ou a tag da imagem está errado, ou ela é privada e você não fez login. Confira a grafia na página do registro.",
        },
      },
      {
        kind: "p",
        text: {
          en: "\"no space left on device\": Docker's disk is full. Run docker system df and clean what you do not need (see the cleanup lesson).",
          pt: "\"no space left on device\": o disco do Docker encheu. Rode docker system df e limpe o que não precisa (veja a lição de limpeza).",
        },
      },
      {
        kind: "p",
        text: {
          en: "\"exec format error\" or \"no such file or directory\" on a script: the image was built for another CPU architecture, or the script has Windows (CRLF) line endings.",
          pt: "\"exec format error\" ou \"no such file or directory\" em um script: a imagem foi feita para outra arquitetura de CPU, ou o script está com final de linha do Windows (CRLF).",
        },
      },
      {
        kind: "tip",
        text: {
          en: "AutoDock's Environment screen checks Docker, Compose and the Engine separately, so you can tell \"Docker is not installed\" from \"Docker is installed but stopped\".",
          pt: "A tela Ambiente do AutoDock verifica Docker, Compose e Engine separadamente, então dá para distinguir \"Docker não instalado\" de \"Docker instalado, mas parado\".",
        },
      },
    ],
    quiz: [
      {
        question: { en: "You see \"Cannot connect to the Docker daemon\". What do you check first?", pt: "Você vê \"Cannot connect to the Docker daemon\". O que conferir primeiro?" },
        options: [
          { en: "Whether Docker Desktop / the Docker engine is running.", pt: "Se o Docker Desktop / o engine do Docker está rodando." },
          { en: "Whether the image name is spelled correctly.", pt: "Se o nome da imagem está escrito certo." },
          { en: "Whether the port is free.", pt: "Se a porta está livre." },
        ],
        answer: 0,
        why: {
          en: "The command line talks to the engine; if the engine is stopped, every command fails the same way.",
          pt: "A linha de comando conversa com o engine; se ele está parado, todo comando falha do mesmo jeito.",
        },
      },
      {
        question: { en: "\"port is already allocated\" means…", pt: "\"port is already allocated\" significa…" },
        options: [
          { en: "another program or container already uses that host port.", pt: "outro programa ou container já usa essa porta do host." },
          { en: "the container port is wrong.", pt: "a porta do container está errada." },
          { en: "Docker has no network.", pt: "o Docker está sem rede." },
        ],
        answer: 0,
        why: {
          en: "Pick a free host port, or stop whatever holds it.",
          pt: "Escolha uma porta livre do host, ou pare o que está usando.",
        },
      },
    ],
    tryIn: { page: "environment", label: { en: "Check my environment", pt: "Verificar meu ambiente" } },
    docs: [{ url: "https://docs.docker.com/desktop/troubleshoot-and-support/troubleshoot/", label: { en: "Troubleshoot Docker Desktop", pt: "Solução de problemas do Docker Desktop" } }],
  },
];
