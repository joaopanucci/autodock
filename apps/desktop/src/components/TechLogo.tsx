import django from "simple-icons/icons/django.svg?raw";
import dotnet from "simple-icons/icons/dotnet.svg?raw";
import go from "simple-icons/icons/go.svg?raw";
import laravel from "simple-icons/icons/laravel.svg?raw";
import mongodb from "simple-icons/icons/mongodb.svg?raw";
import mysql from "simple-icons/icons/mysql.svg?raw";
import nestjs from "simple-icons/icons/nestjs.svg?raw";
import nextjs from "simple-icons/icons/nextdotjs.svg?raw";
import nodejs from "simple-icons/icons/nodedotjs.svg?raw";
import postgresql from "simple-icons/icons/postgresql.svg?raw";
import python from "simple-icons/icons/python.svg?raw";
import rabbitmq from "simple-icons/icons/rabbitmq.svg?raw";
import react from "simple-icons/icons/react.svg?raw";
import redis from "simple-icons/icons/redis.svg?raw";
import rails from "simple-icons/icons/rubyonrails.svg?raw";
import springboot from "simple-icons/icons/springboot.svg?raw";
import vue from "simple-icons/icons/vuedotjs.svg?raw";

// Brand marks come from the simple-icons package (CC0). Only the files used here
// are bundled. They are drawn in the current text color so they follow the light
// and dark themes, and they only identify the technology they belong to.
const pathOf = (svg: string) => /<path d="([^"]+)"/.exec(svg)?.[1] ?? "";

const ICONS: Record<string, string> = {
  nodejs: pathOf(nodejs),
  "spring-boot": pathOf(springboot),
  python: pathOf(python),
  laravel: pathOf(laravel),
  go: pathOf(go),
  dotnet: pathOf(dotnet),
  rails: pathOf(rails),
  "vite-react": pathOf(react),
  "vite-vue": pathOf(vue),
  nextjs: pathOf(nextjs),
  nestjs: pathOf(nestjs),
  django: pathOf(django),
  postgres: pathOf(postgresql),
  mysql: pathOf(mysql),
  mongodb: pathOf(mongodb),
  redis: pathOf(redis),
  rabbitmq: pathOf(rabbitmq),
};

export function hasLogo(id: string) {
  return Boolean(ICONS[id]);
}

export function TechLogo({ id, size = 26 }: { id: string; size?: number }) {
  const path = ICONS[id];
  if (!path) return null;
  return (
    <svg
      className="tech-logo"
      viewBox="0 0 24 24"
      width={size}
      height={size}
      fill="currentColor"
      aria-hidden="true"
      focusable="false"
    >
      <path d={path} />
    </svg>
  );
}
