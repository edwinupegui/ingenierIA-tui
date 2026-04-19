#!/usr/bin/env sh
# ingenierIA TUI — Instalador Mac & Linux
# Uso:  curl -fsSL https://raw.githubusercontent.com/your-org/ingenieria-tui/main/scripts/install.sh | sh
# ──────────────────────────────────────────────────────────────────────────────
set -e

REPO="your-org/ingenieria-tui"
BINARY_NAME="ingenieria"
INSTALL_DIR="${INSTALL_DIR:-/usr/local/bin}"

# ── Colores ───────────────────────────────────────────────────────────────────
BOLD='\033[1m'
GREEN='\033[0;32m'
RED='\033[0;31m'
DIM='\033[2m'
RESET='\033[0m'

print_step()  { printf "  ${BOLD}▸${RESET} %s\n" "$1"; }
print_ok()    { printf "  ${GREEN}✔${RESET} %s\n" "$1"; }
print_err()   { printf "  ${RED}✖${RESET} %s\n" "$1" >&2; }

# ── Detectar plataforma ───────────────────────────────────────────────────────
OS=$(uname -s | tr '[:upper:]' '[:lower:]')
ARCH=$(uname -m)

case "${OS}-${ARCH}" in
  darwin-arm64)   TARGET="aarch64-apple-darwin"      ;;
  darwin-x86_64)  TARGET="x86_64-apple-darwin"       ;;
  linux-x86_64)   TARGET="x86_64-unknown-linux-musl" ;;
  linux-aarch64)  TARGET="aarch64-unknown-linux-musl" ;;
  *)
    print_err "Plataforma no soportada: ${OS}-${ARCH}"
    print_err "Contáctanos en #dev-tools para soporte."
    exit 1
    ;;
esac

# ── Verificar dependencias ────────────────────────────────────────────────────
if ! command -v curl >/dev/null 2>&1; then
  print_err "curl no está instalado. Por favor instálalo y vuelve a intentar."
  exit 1
fi

# ── Obtener la última versión ─────────────────────────────────────────────────
print_step "Buscando la última versión..."
VERSION=$(curl -sf "https://api.github.com/repos/${REPO}/releases/latest" \
  | grep '"tag_name"' | cut -d'"' -f4)

if [ -z "$VERSION" ]; then
  print_err "No se pudo obtener la versión. Verifica tu conexión a internet."
  exit 1
fi

printf "\n  ${BOLD}ingenierIA TUI${RESET} ${GREEN}${VERSION}${RESET} para ${DIM}${TARGET}${RESET}\n\n"

# ── Descargar binario ─────────────────────────────────────────────────────────
ARCHIVE="ingenieria-${TARGET}.tar.gz"
URL="https://github.com/${REPO}/releases/download/${VERSION}/${ARCHIVE}"
TMP_DIR=$(mktemp -d)

print_step "Descargando desde GitHub Releases..."
curl -fsSL --progress-bar "${URL}" -o "${TMP_DIR}/${ARCHIVE}" || {
  print_err "Descarga fallida. URL: ${URL}"
  rm -rf "${TMP_DIR}"
  exit 1
}

# ── Instalar ──────────────────────────────────────────────────────────────────
print_step "Instalando en ${INSTALL_DIR}..."
tar -xzf "${TMP_DIR}/${ARCHIVE}" -C "${TMP_DIR}"
chmod +x "${TMP_DIR}/${BINARY_NAME}"

# Intentar copiar a /usr/local/bin; si falla, pedir sudo
if cp "${TMP_DIR}/${BINARY_NAME}" "${INSTALL_DIR}/${BINARY_NAME}" 2>/dev/null; then
  :
elif command -v sudo >/dev/null 2>&1; then
  sudo cp "${TMP_DIR}/${BINARY_NAME}" "${INSTALL_DIR}/${BINARY_NAME}"
else
  print_err "No tienes permisos para escribir en ${INSTALL_DIR}."
  print_err "Intenta: INSTALL_DIR=~/.local/bin ${0}"
  rm -rf "${TMP_DIR}"
  exit 1
fi

rm -rf "${TMP_DIR}"

print_ok "Instalado en ${INSTALL_DIR}/${BINARY_NAME}"

# ── Verificar que esté en el PATH ─────────────────────────────────────────────
if ! command -v "${BINARY_NAME}" >/dev/null 2>&1; then
  printf "\n  ${DIM}Nota: ${INSTALL_DIR} no está en tu PATH.${RESET}\n"
  printf "  Agrega esta línea a tu ~/.zshrc o ~/.bashrc:\n"
  printf "\n    export PATH=\"${INSTALL_DIR}:\$PATH\"\n\n"
fi

# ── Lanzar ingenierIA TUI ───────────────────────────────────────────────────────
printf "\n  ${GREEN}✔${RESET} ${BOLD}¡Listo!${RESET}\n\n"
printf "  Ejecuta ${BOLD}ingenierIA${RESET} desde cualquier directorio.\n"
printf "  La primera vez se te pedirá configurar tu perfil.\n\n"
exec "${INSTALL_DIR}/${BINARY_NAME}"
