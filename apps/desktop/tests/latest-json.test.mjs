import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { execFileSync, spawnSync } from "node:child_process";
import { buildManifest } from "../../../scripts/make-latest-json.mjs";

const SIG = "dW50cnVzdGVkIGNvbW1lbnQ6IHNpZ25hdHVyZSBmcm9tIHRhdXJpIHNlY3JldCBrZXkKUlVTVkJEbUVkNGZ0dCtTRmRiZmtuSHBDUGZ6TGQrY2cvM2hoQkgvdk5TUXNzQnpTenVYY25q";
const NOW = new Date("2026-11-08T12:34:56.789Z");
const FILES = [
  "AutoDock_0.4.0_x64-setup.exe",
  "AutoDock_0.4.0_x64-setup.exe.sig",
  "AutoDock.app.tar.gz",
  "AutoDock.app.tar.gz.sig",
  "AutoDock_0.4.0_universal.dmg",
  "AutoDock_0.4.0_amd64.AppImage",
  "AutoDock_0.4.0_amd64.AppImage.sig",
  "AutoDock_0.4.0_amd64.deb",
  "AutoDock_0.4.0_amd64.deb.sig",
];
const build = (names = FILES, tag = "v0.4.0") =>
  buildManifest({ names, readSignature: () => `${SIG}\n`, tag, repo: "joaopanucci/autodock", now: NOW });

test("the manifest has the version, the date and one entry per update target", () => {
  const manifest = build();
  assert.equal(manifest.version, "0.4.0");
  assert.equal(manifest.pub_date, "2026-11-08T12:34:56Z");
  assert.match(manifest.notes, /AutoDock 0\.4\.0 is available\..*releases\/tag\/v0\.4\.0/);
  assert.deepEqual(Object.keys(manifest.platforms).sort(), [
    "darwin-aarch64",
    "darwin-x86_64",
    "linux-x86_64",
    "linux-x86_64-appimage",
    "linux-x86_64-deb",
    "windows-x86_64",
  ]);
  const base = "https://github.com/joaopanucci/autodock/releases/download/v0.4.0/";
  assert.equal(manifest.platforms["windows-x86_64"].url, `${base}AutoDock_0.4.0_x64-setup.exe`);
  assert.equal(manifest.platforms["darwin-aarch64"].url, `${base}AutoDock.app.tar.gz`);
  assert.equal(manifest.platforms["darwin-x86_64"].url, manifest.platforms["darwin-aarch64"].url, "universal build");
  assert.equal(manifest.platforms["linux-x86_64"].url, `${base}AutoDock_0.4.0_amd64.AppImage`);
  assert.equal(manifest.platforms["linux-x86_64-deb"].url, `${base}AutoDock_0.4.0_amd64.deb`);
  for (const platform of Object.values(manifest.platforms)) assert.equal(platform.signature, SIG, "trimmed");
});

test("the deb is optional but the other installers and every signature are required", () => {
  const withoutDeb = build(FILES.filter((n) => !n.includes(".deb")));
  assert.ok(!("linux-x86_64-deb" in withoutDeb.platforms));
  assert.ok("linux-x86_64-appimage" in withoutDeb.platforms);

  assert.throws(() => build(FILES.filter((n) => n !== "AutoDock_0.4.0_x64-setup.exe.sig")), /missing from the release: AutoDock_0\.4\.0_x64-setup\.exe\.sig/);
  assert.throws(() => build(FILES.filter((n) => !n.startsWith("AutoDock.app"))), /macOS app archive/);
  assert.throws(() => build(FILES.filter((n) => !n.includes("AppImage"))), /Linux AppImage/);
  // a deb without a signature is left out rather than offered unsigned
  const unsignedDeb = build(FILES.filter((n) => n !== "AutoDock_0.4.0_amd64.deb.sig"));
  assert.ok(!("linux-x86_64-deb" in unsignedDeb.platforms));
});

test("bad tags, repositories and signatures are refused", () => {
  for (const tag of ["0.4.0", "v0.4", "v0.4.0.1", "v../1.0.0", "", "latest"]) {
    assert.throws(() => build(FILES, tag), /not a release tag/, tag);
  }
  assert.equal(build(FILES, "v0.5.0-beta.1").version, "0.5.0-beta.1");
  assert.throws(() => buildManifest({ names: FILES, readSignature: () => "x", tag: "v0.4.0", repo: "a/b", now: NOW }), /does not look like a signature/);
  assert.throws(() => buildManifest({ names: FILES, readSignature: () => SIG, tag: "v0.4.0", repo: "not a repo", now: NOW }), /owner\/repo/);
  assert.throws(() => build([...FILES, "Other_setup-setup.exe"]), /more than one file/);
});

test("file names are escaped in the download URL", () => {
  const names = FILES.map((n) => n.replace("AutoDock_0.4.0_x64", "Auto Dock_0.4.0_x64"));
  assert.match(build(names).platforms["windows-x86_64"].url, /Auto%20Dock_0\.4\.0_x64-setup\.exe$/);
});

test("the script writes latest.json next to the installers and fails loudly when something is missing", () => {
  const script = path.join(import.meta.dirname, "../../../scripts/make-latest-json.mjs");
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "latest-json-"));
  try {
    for (const name of FILES) fs.writeFileSync(path.join(dir, name), name.endsWith(".sig") ? SIG : "x");
    const out = execFileSync(process.execPath, [script, dir, "v0.4.0", "joaopanucci/autodock"], { encoding: "utf8" });
    assert.match(out, /latest\.json for 0\.4\.0: .*windows-x86_64/);
    const written = JSON.parse(fs.readFileSync(path.join(dir, "latest.json"), "utf8"));
    assert.equal(written.version, "0.4.0");

    fs.rmSync(path.join(dir, "AutoDock.app.tar.gz.sig"));
    const failed = spawnSync(process.execPath, [script, dir, "v0.4.0", "joaopanucci/autodock"], { encoding: "utf8" });
    assert.equal(failed.status, 1);
    assert.match(failed.stderr, /missing from the release: AutoDock\.app\.tar\.gz\.sig/);
    assert.equal(spawnSync(process.execPath, [script], { encoding: "utf8" }).status, 2);
  } finally {
    fs.rmSync(dir, { recursive: true, force: true });
  }
});
