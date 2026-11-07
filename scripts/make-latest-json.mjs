// Builds the `latest.json` the in-app updater reads (Tauri updater format) from the signed
// installers of a release. Run by the "Build installers" workflow when a tag is pushed:
//
//   node scripts/make-latest-json.mjs <folder with the installers and .sig files> <tag> <owner/repo>
//
// It writes <folder>/latest.json and fails, listing what is missing, when an installer or its
// signature is not there: a release without update files must not go out unnoticed.
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const VERSION = /^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$/;
const REPO = /^[A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+$/;

/** What each update target needs: the installer the updater downloads and where it is found. */
const TARGETS = [
  { keys: ["windows-x86_64"], find: (n) => n.endsWith("-setup.exe"), required: true, label: "Windows installer (*-setup.exe)" },
  { keys: ["darwin-aarch64", "darwin-x86_64"], find: (n) => n.endsWith(".app.tar.gz"), required: true, label: "macOS app archive (*.app.tar.gz)" },
  { keys: ["linux-x86_64-appimage", "linux-x86_64"], find: (n) => n.endsWith(".AppImage"), required: true, label: "Linux AppImage (*.AppImage)" },
  { keys: ["linux-x86_64-deb"], find: (n) => n.endsWith(".deb"), required: false, label: "Linux package (*.deb)" },
];

/**
 * @param {{ names: string[], readSignature: (name: string) => string, tag: string, repo: string, now?: Date }} input
 * `names` are the file names of the release; the signature of `X` is the file `X.sig`.
 */
export function buildManifest({ names, readSignature, tag, repo, now = new Date() }) {
  const version = tag.replace(/^v/, "");
  if (!tag.startsWith("v") || !VERSION.test(version)) throw new Error(`not a release tag: ${JSON.stringify(tag)}`);
  if (!REPO.test(repo)) throw new Error(`not an owner/repo: ${JSON.stringify(repo)}`);

  const platforms = {};
  const missing = [];
  for (const target of TARGETS) {
    const installers = names.filter((n) => !n.endsWith(".sig") && target.find(n));
    if (installers.length > 1) throw new Error(`more than one file for ${target.label}: ${installers.join(", ")}`);
    const installer = installers[0];
    if (!installer || !names.includes(`${installer}.sig`)) {
      if (target.required) missing.push(installer ? `${installer}.sig` : target.label);
      continue;
    }
    const signature = readSignature(`${installer}.sig`).trim();
    if (!/^[A-Za-z0-9+/=]{40,}$/.test(signature)) throw new Error(`${installer}.sig does not look like a signature`);
    const url = `https://github.com/${repo}/releases/download/${tag}/${encodeURIComponent(installer)}`;
    for (const key of target.keys) platforms[key] = { signature, url };
  }
  if (missing.length) throw new Error(`missing from the release: ${missing.join("; ")}`);

  return {
    version,
    notes: `AutoDock ${version} is available. What's new: https://github.com/${repo}/releases/tag/${tag}`,
    pub_date: now.toISOString().replace(/\.\d{3}Z$/, "Z"),
    platforms,
  };
}

function main(argv) {
  const [folder, tag, repo] = argv;
  if (!folder || !tag || !repo) {
    console.error("usage: node scripts/make-latest-json.mjs <folder> <tag> <owner/repo>");
    return 2;
  }
  const names = fs.readdirSync(folder);
  const manifest = buildManifest({
    names,
    readSignature: (name) => fs.readFileSync(path.join(folder, name), "utf8"),
    tag,
    repo,
  });
  fs.writeFileSync(path.join(folder, "latest.json"), `${JSON.stringify(manifest, null, 2)}\n`);
  console.log(`latest.json for ${manifest.version}: ${Object.keys(manifest.platforms).join(", ")}`);
  return 0;
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try {
    process.exitCode = main(process.argv.slice(2));
  } catch (error) {
    console.error(`Error: ${error.message}`);
    process.exitCode = 1;
  }
}
