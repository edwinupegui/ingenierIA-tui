#!/bin/bash
# Prepara los paquetes npm con los binarios compilados.
# Uso: ./scripts/npm-prepare.sh <version>
#
# Espera los binarios en dist/:
#   dist/ingenieria-aarch64-apple-darwin
#   dist/ingenieria-x86_64-apple-darwin
#   dist/ingenieria-x86_64-pc-windows-msvc.exe
#   dist/ingenieria-x86_64-unknown-linux-musl

set -euo pipefail

VERSION="${1:?Uso: $0 <version>}"

echo "==> Preparando paquetes npm v${VERSION}"

# ── Mapeo: directorio npm → binario compilado ────────────────────────────────
declare -A TARGETS=(
  ["ingenieria-darwin-arm64"]="dist/ingenieria-aarch64-apple-darwin"
  ["ingenieria-darwin-x64"]="dist/ingenieria-x86_64-apple-darwin"
  ["ingenieria-win32-x64"]="dist/ingenieria-x86_64-pc-windows-msvc.exe"
  ["ingenieria-linux-x64"]="dist/ingenieria-x86_64-unknown-linux-musl"
)

# ── Copiar binarios a cada paquete de plataforma ─────────────────────────────
for pkg in "${!TARGETS[@]}"; do
  src="${TARGETS[$pkg]}"
  dest="npm/${pkg}/bin"
  mkdir -p "$dest"

  if [[ "$src" == *.exe ]]; then
    cp "$src" "$dest/ingenieria.exe"
  else
    cp "$src" "$dest/ingenieria"
    chmod +x "$dest/ingenieria"
  fi

  echo "  ${pkg}: $(ls -lh ${dest}/ingenieria* | awk '{print $5}')"
done

# ── Actualizar versiones en todos los package.json ───────────────────────────
for dir in npm/ingenieria npm/ingenieria-darwin-arm64 npm/ingenieria-darwin-x64 npm/ingenieria-win32-x64 npm/ingenieria-linux-x64; do
  if command -v jq &>/dev/null; then
    tmp=$(mktemp)
    jq ".version = \"${VERSION}\"" "$dir/package.json" > "$tmp" && mv "$tmp" "$dir/package.json"
  else
    sed -i.bak "s/\"version\": \"[^\"]*\"/\"version\": \"${VERSION}\"/" "$dir/package.json"
    rm -f "$dir/package.json.bak"
  fi
done

# Actualizar optionalDependencies en el paquete principal
if command -v jq &>/dev/null; then
  tmp=$(mktemp)
  jq "
    .optionalDependencies[\"@your-org/ingenieria-darwin-arm64\"] = \"${VERSION}\" |
    .optionalDependencies[\"@your-org/ingenieria-darwin-x64\"] = \"${VERSION}\" |
    .optionalDependencies[\"@your-org/ingenieria-win32-x64\"] = \"${VERSION}\" |
    .optionalDependencies[\"@your-org/ingenieria-linux-x64\"] = \"${VERSION}\"
  " npm/ingenieria/package.json > "$tmp" && mv "$tmp" npm/ingenieria/package.json
fi

# ── Verificar contenido (seguridad) ──────────────────────────────────────────
echo ""
echo "==> Verificacion de seguridad: contenido de cada paquete"
for dir in npm/ingenieria npm/ingenieria-darwin-arm64 npm/ingenieria-darwin-x64 npm/ingenieria-win32-x64 npm/ingenieria-linux-x64; do
  echo ""
  echo "--- ${dir} ---"
  (cd "$dir" && npm pack --dry-run 2>&1 | head -30)
done

echo ""
echo "==> Paquetes listos para publicar con npm publish"
