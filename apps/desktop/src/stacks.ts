import type { TranslationKey } from "./i18n";
import type { ServiceId, StackId } from "./types";

// Technologies and services the assistants can set up. Shared by the single-app
// wizard and the multi-app screen.
export const STACKS: {
  id: StackId;
  title: string;
  hint: TranslationKey;
  port: number;
  versions: string[];
  versionLabel: TranslationKey;
}[] = [
  { id: "nodejs", title: "Node.js", hint: "wizard.stack.nodejs.hint", port: 3000, versions: ["22", "24", "20"], versionLabel: "wizard.stack.nodejs.version" },
  { id: "spring-boot", title: "Java / Spring Boot", hint: "wizard.stack.spring.hint", port: 8080, versions: ["21", "17"], versionLabel: "wizard.stack.spring.version" },
  { id: "python", title: "Python", hint: "wizard.stack.python.hint", port: 8000, versions: ["3.12", "3.13", "3.11"], versionLabel: "wizard.stack.python.version" },
  { id: "laravel", title: "PHP / Laravel", hint: "wizard.stack.laravel.hint", port: 8000, versions: ["8.4", "8.5", "8.3", "8.2"], versionLabel: "wizard.stack.laravel.version" },
  { id: "go", title: "Go", hint: "wizard.stack.go.hint", port: 8080, versions: ["1.26", "1.25", "1.24"], versionLabel: "wizard.stack.go.version" },
  { id: "dotnet", title: ".NET / ASP.NET Core", hint: "wizard.stack.dotnet.hint", port: 8080, versions: ["10.0", "9.0", "8.0"], versionLabel: "wizard.stack.dotnet.version" },
  { id: "rails", title: "Ruby on Rails", hint: "wizard.stack.rails.hint", port: 3000, versions: ["3.4", "3.3", "4.0"], versionLabel: "wizard.stack.rails.version" },
];

export const SERVICES: { id: ServiceId; title: string; hint: TranslationKey; group: TranslationKey }[] = [
  { id: "postgres", title: "PostgreSQL", hint: "wizard.svc.relational", group: "wizard.group.db" },
  { id: "mysql", title: "MySQL", hint: "wizard.svc.relational", group: "wizard.group.db" },
  { id: "mongodb", title: "MongoDB", hint: "wizard.svc.document", group: "wizard.group.db" },
  { id: "redis", title: "Redis", hint: "wizard.svc.cache", group: "wizard.group.cache" },
  { id: "rabbitmq", title: "RabbitMQ", hint: "wizard.svc.queue", group: "wizard.group.cache" },
  { id: "adminer", title: "Adminer", hint: "wizard.svc.adminer", group: "wizard.group.dev" },
  { id: "mailhog", title: "MailHog", hint: "wizard.svc.mailhog", group: "wizard.group.dev" },
];
