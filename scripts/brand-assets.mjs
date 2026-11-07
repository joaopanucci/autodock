// Regenerates the images that carry the brand outside the app:
//   - apps/desktop/src-tauri/dmg/background.png (+ its .src.html)  macOS installer window
//   - docs/social-preview.png                                       GitHub social preview
// Run after changing the icon or the look of the app, and after scripts/screenshots.mjs
// (the social preview shows docs/screenshots/en/06-assistente-revisao.png).
//   node scripts/brand-assets.mjs
// Needs Playwright (apps/desktop devDependency) and Edge or Chrome; override with
// AUTODOCK_BROWSER_CHANNEL.
import { createRequire } from "node:module";
import { readFileSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import path from "node:path";

const require = createRequire(new URL("../apps/desktop/package.json", import.meta.url));
const { chromium } = require("@playwright/test");
const root = fileURLToPath(new URL("..", import.meta.url));
const at = (...parts) => path.join(root, ...parts);

const icon = readFileSync(at("apps/desktop/src-tauri/icons/app-icon.svg"), "utf8").replace(/ width="1024" height="1024"/, "");
const fontFile = at("apps/desktop/node_modules/@fontsource-variable/inter/files/inter-latin-wght-normal.woff2");
const face = `@font-face{font-family:"InterX";font-weight:100 900;src:url(data:font/woff2;base64,${readFileSync(fontFile).toString("base64")}) format("woff2")}`;
const shot = readFileSync(at("docs/screenshots/en/06-assistente-revisao.png")).toString("base64");

const dmg = `<html><body style="margin:0"><style>${face}</style>
<div style="width:660px;height:400px;position:relative;overflow:hidden;font-family:'InterX',-apple-system,'Segoe UI',sans-serif;background:#ececea;color:#1f2328;-webkit-font-smoothing:antialiased">
  <div style="position:absolute;left:0;right:0;top:34px;text-align:center">
    <div style="font-size:22px;font-weight:650;letter-spacing:-.03em">Arraste o autodock para Aplicativos</div>
    <div style="font-size:13px;color:#565b63;margin-top:7px">para instalar. É só isso.</div>
  </div>
  <svg style="position:absolute;left:255px;top:168px" width="150" height="64" viewBox="0 0 150 64" fill="none">
    <path d="M6 32 H118" stroke="#1f2328" stroke-width="3" stroke-linecap="round" stroke-dasharray="1 11"/>
    <path d="M108 18 L128 32 L108 46" stroke="#1f2328" stroke-width="3.5" stroke-linecap="round" stroke-linejoin="round"/>
  </svg>
  <div style="position:absolute;left:0;right:0;bottom:22px;text-align:center;font-size:11.5px;color:#767b83">Requer Docker instalado · Open source (Apache-2.0)</div>
</div></body></html>`;
writeFileSync(at("apps/desktop/src-tauri/dmg/background.src.html"), dmg);

const chips = ["Node.js", "Next.js", "Spring Boot", "Python", "Laravel", "Go", ".NET", "Rails"]
  .map((c) => `<span style="border:1px solid #b4b4ae;padding:7px 14px;border-radius:2px;font-size:19px;color:#2e333a">${c}</span>`)
  .join("");
const social = `<html><body style="margin:0"><style>${face}</style>
<div style="width:1280px;height:640px;position:relative;overflow:hidden;background:#ececea;color:#1f2328;font-family:'InterX',sans-serif;-webkit-font-smoothing:antialiased">
  <div style="position:absolute;left:72px;top:64px;width:92px;height:92px">${icon.replace("<svg ", '<svg width="92" height="92" ')}</div>
  <div style="position:absolute;left:68px;top:196px;font-size:104px;font-weight:650;letter-spacing:-.055em;line-height:1">autodock</div>
  <div style="position:absolute;left:72px;top:330px;width:520px;font-size:34px;font-weight:300;line-height:1.28;letter-spacing:-.015em">Put any project in Docker in a few clicks.</div>
  <div style="position:absolute;left:72px;top:446px;width:540px;display:flex;flex-wrap:wrap;gap:10px">${chips}</div>
  <div style="position:absolute;left:72px;bottom:52px;font-size:20px;color:#767b83;letter-spacing:.01em">Open source · macOS · Windows · Linux</div>
  <div style="position:absolute;left:668px;top:66px;width:760px;border:1px solid #b4b4ae;background:#f3f3f1;box-shadow:0 30px 60px -30px rgba(0,0,0,.35)">
    <img src="data:image/png;base64,${shot}" style="display:block;width:760px" />
  </div>
</div></body></html>`;

const browser = await chromium.launch({ channel: process.env.AUTODOCK_BROWSER_CHANNEL ?? (process.platform === "win32" ? "msedge" : "chrome") });
async function render(html, width, height, scale, file) {
  const page = await browser.newPage({ viewport: { width, height }, deviceScaleFactor: scale });
  await page.setContent(html);
  await page.evaluate(() => document.fonts.ready);
  await page.waitForTimeout(400);
  await page.screenshot({ path: file, clip: { x: 0, y: 0, width, height } });
  await page.close();
}
await render(dmg, 660, 400, 2, at("apps/desktop/src-tauri/dmg/background.png"));
await render(social, 1280, 640, 1, at("docs/social-preview.png"));
await browser.close();
console.log("brand assets rendered");
