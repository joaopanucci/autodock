#!/usr/bin/env bash
# AutoDock installer for macOS.
#
#   curl -fsSL https://raw.githubusercontent.com/joaopanucci/autodock/main/scripts/install.sh | bash
#
# It downloads the universal DMG of the latest release, VERIFIES its SHA-256 against
# the release's SHA256SUMS.txt, and copies AutoDock.app to /Applications.
#
# Why this avoids the "Apple could not verify..." warning: macOS only shows it for
# files carrying the com.apple.quarantine attribute, which browsers add on download.
# curl does not add it. Nothing here weakens macOS: the app is still ad-hoc signed and
# unnotarized, and you can read this whole script before running it.
#
# Options (environment variables):
#   AUTODOCK_VERSION  release tag to install, e.g. v0.3.0 (default: latest)
#   AUTODOCK_DEST     folder to install into (default: /Applications)
#   AUTODOCK_NO_OPEN  set to anything to skip opening the app afterwards
set -euo pipefail

REPO="joaopanucci/autodock"
VERSION="${AUTODOCK_VERSION:-latest}"
DEST="${AUTODOCK_DEST:-/Applications}"

say() { # say "texto em português" "english text"
  case "${LC_ALL:-${LC_MESSAGES:-${LANG:-}}}" in pt*) printf '%s\n' "$1" ;; *) printf '%s\n' "$2" ;; esac
}
die() {
  say "Erro: $1" "Error: $2" >&2
  exit 1
}

[ "$(uname -s)" = "Darwin" ] || die \
  "este script é para macOS. No Windows use install.ps1; no Linux baixe o .deb ou o .AppImage da página de releases." \
  "this script is for macOS. On Windows use install.ps1; on Linux download the .deb or .AppImage from the releases page."
for tool in curl shasum hdiutil ditto; do
  command -v "$tool" >/dev/null 2>&1 || die "ferramenta ausente: $tool" "missing tool: $tool"
done

if [ "$VERSION" = "latest" ]; then
  url="$(curl -fsSLI -o /dev/null -w '%{url_effective}' "https://github.com/$REPO/releases/latest")" \
    || die "não foi possível descobrir a última versão." "could not find the latest version."
  tag="${url##*/}"
else
  tag="$VERSION"
fi
# Only plain version tags: nothing else may end up inside the download URL.
[[ "${tag}" =~ ^v[0-9]+\.[0-9]+\.[0-9]+$ ]] || die "versão inválida: ${tag}" "invalid version: ${tag}"

asset="AutoDock_${tag#v}_universal.dmg"
base="https://github.com/$REPO/releases/download/${tag}"

tmp="$(mktemp -d)"
mount="$tmp/volume"
cleanup() {
  # `mount` prints resolved paths (/private/var/...), so do not grep for ours: just
  # try to detach, forcing it if the first attempt is refused.
  if [ -d "$mount" ]; then
    hdiutil detach "$mount" -quiet >/dev/null 2>&1 || hdiutil detach "$mount" -force -quiet >/dev/null 2>&1 || true
  fi
  rm -rf "$tmp" 2>/dev/null || true
}
trap cleanup EXIT

say "Baixando AutoDock ${tag}..." "Downloading AutoDock ${tag}..."
curl -fL --progress-bar -o "$tmp/${asset}" "$base/${asset}" || die "falha ao baixar ${asset}." "failed to download ${asset}."
curl -fsSL -o "$tmp/SHA256SUMS.txt" "$base/SHA256SUMS.txt" || die "falha ao baixar o SHA256SUMS.txt." "failed to download SHA256SUMS.txt."

say "Conferindo o checksum..." "Verifying the checksum..."
expected="$(awk -v file="${asset}" '$2 == file { print $1 }' "$tmp/SHA256SUMS.txt")"
actual="$(shasum -a 256 "$tmp/${asset}" | awk '{ print $1 }')"
[ -n "$expected" ] || die "${asset} não consta no SHA256SUMS.txt." "${asset} is not listed in SHA256SUMS.txt."
[ "$expected" = "$actual" ] || die \
  "checksum diferente do esperado. Nada foi instalado." \
  "checksum does not match. Nothing was installed."

mkdir -p "$mount"
if ! hdiutil attach -nobrowse -readonly -noautoopen -mountpoint "$mount" "$tmp/${asset}" >"$tmp/hdiutil.log" 2>&1; then
  # Show why: a bare "could not open" gives nothing to act on.
  tail -n 8 "$tmp/hdiutil.log" | sed 's/^/  hdiutil: /' >&2
  die "não foi possível abrir o DMG." "could not open the DMG."
fi
[ -d "$mount/AutoDock.app" ] || die "AutoDock.app não está no DMG." "AutoDock.app is not in the DMG."

if pgrep -x autodock-desktop >/dev/null 2>&1; then
  die "feche o AutoDock e rode de novo." "close AutoDock and run this again."
fi

if ! mkdir -p "${DEST}" 2>/dev/null || [ ! -w "${DEST}" ]; then
  DEST="$HOME/Applications"
  say "Sem permissão em /Applications; instalando em ${DEST}." "No write access to /Applications; installing into ${DEST}."
  mkdir -p "${DEST}"
fi

target="${DEST}/AutoDock.app"
[ ! -e "${target}" ] || rm -rf "${target}"
ditto "$mount/AutoDock.app" "${target}"
# Defensive: a copy made from a curl download has no quarantine mark, but make sure.
xattr -dr com.apple.quarantine "${target}" 2>/dev/null || true

say "AutoDock ${tag} instalado em ${target}" "AutoDock ${tag} installed at ${target}"
if [ -z "${AUTODOCK_NO_OPEN:-}" ]; then
  open "${target}"
fi
