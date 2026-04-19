#!/bin/bash
# Sincroniza la version de todos los paquetes externos (npm + docs-site)
# con la version del workspace Cargo.toml.
#
# Fuente unica de verdad: [workspace.package] version en Cargo.toml.
#
# Uso: ./scripts/sync-version.sh

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
CARGO_TOML="${ROOT}/Cargo.toml"

# ── Leer version del workspace (fuente unica) ────────────────────────────────
VERSION="$(awk '
  /^\[workspace\.package\]/ { in_ws=1; next }
  /^\[/ && in_ws { in_ws=0 }
  in_ws && /^version[[:space:]]*=/ {
    gsub(/[[:space:]"]/, "", $0)
    split($0, a, "=")
    print a[2]
    exit
  }
' "$CARGO_TOML")"

if [[ -z "$VERSION" ]]; then
  echo "error: no se pudo leer [workspace.package] version de Cargo.toml" >&2
  exit 1
fi

echo "==> Sincronizando a v${VERSION}"

NPM_DIRS=(
  "npm/ingenieria"
  "npm/ingenieria-darwin-arm64"
  "npm/ingenieria-darwin-x64"
  "npm/ingenieria-win32-x64"
  "npm/ingenieria-linux-x64"
  "docs-site"
)

has_jq=0
if command -v jq &>/dev/null; then
  has_jq=1
fi

for dir in "${NPM_DIRS[@]}"; do
  pkg="${ROOT}/${dir}/package.json"
  [[ -f "$pkg" ]] || { echo "skip ${dir} (sin package.json)"; continue; }

  if [[ $has_jq -eq 1 ]]; then
    tmp=$(mktemp)
    jq ".version = \"${VERSION}\"" "$pkg" > "$tmp" && mv "$tmp" "$pkg"
  else
    sed -i.bak "0,/\"version\": \"[^\"]*\"/s//\"version\": \"${VERSION}\"/" "$pkg"
    rm -f "${pkg}.bak"
  fi
  echo "  ${dir}/package.json -> ${VERSION}"
done

# ── optionalDependencies en el paquete meta npm ──────────────────────────────
main_pkg="${ROOT}/npm/ingenieria/package.json"
if [[ -f "$main_pkg" && $has_jq -eq 1 ]]; then
  tmp=$(mktemp)
  jq "
    .optionalDependencies[\"@your-org/ingenieria-darwin-arm64\"] = \"${VERSION}\" |
    .optionalDependencies[\"@your-org/ingenieria-darwin-x64\"]   = \"${VERSION}\" |
    .optionalDependencies[\"@your-org/ingenieria-win32-x64\"]    = \"${VERSION}\" |
    .optionalDependencies[\"@your-org/ingenieria-linux-x64\"]    = \"${VERSION}\"
  " "$main_pkg" > "$tmp" && mv "$tmp" "$main_pkg"
  echo "  npm/ingenieria optionalDependencies -> ${VERSION}"
fi

echo "==> OK"
