#!/usr/bin/env sh
# ingenierIA TUI — Instalador local (repo privado)
# ──────────────────────────────────────────────────────────────────────────────
# Uso:  sh install-local.sh ingenieria-aarch64-apple-darwin.tar.gz
#       sh install-local.sh  (auto-detecta el .tar.gz en el directorio actual)
# ──────────────────────────────────────────────────────────────────────────────
set -e

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

# ── Resolver archivo ─────────────────────────────────────────────────────────
if [ -n "$1" ]; then
  ARCHIVE="$1"
else
  # Auto-detectar: buscar un .tar.gz de ingenieria en el directorio actual
  ARCHIVE=$(ls ingenieria-*.tar.gz 2>/dev/null | head -1)
  if [ -z "$ARCHIVE" ]; then
    print_err "No se encontro ningun archivo ingenieria-*.tar.gz"
    printf "\n  ${BOLD}Uso:${RESET}\n"
    printf "    sh install-local.sh ingenieria-aarch64-apple-darwin.tar.gz\n\n"
    printf "  O coloca el archivo .tar.gz en este directorio y ejecuta:\n"
    printf "    sh install-local.sh\n\n"
    exit 1
  fi
fi

if [ ! -f "$ARCHIVE" ]; then
  print_err "Archivo no encontrado: $ARCHIVE"
  exit 1
fi

printf "\n  ${BOLD}ingenierIA TUI${RESET} — Instalador local\n"
printf "  Archivo: ${DIM}%s${RESET}\n\n" "$ARCHIVE"

# ── Extraer ───────────────────────────────────────────────────────────────────
TMP_DIR=$(mktemp -d)
print_step "Extrayendo..."
tar -xzf "$ARCHIVE" -C "$TMP_DIR"

# Buscar el binario (puede estar en raiz o subdirectorio)
BINARY_PATH=$(find "$TMP_DIR" -name "$BINARY_NAME" -type f | head -1)
if [ -z "$BINARY_PATH" ]; then
  print_err "No se encontro el binario '$BINARY_NAME' dentro del archivo."
  rm -rf "$TMP_DIR"
  exit 1
fi

chmod +x "$BINARY_PATH"

# ── Instalar ──────────────────────────────────────────────────────────────────
print_step "Instalando en ${INSTALL_DIR}..."

if cp "$BINARY_PATH" "${INSTALL_DIR}/${BINARY_NAME}" 2>/dev/null; then
  :
elif command -v sudo >/dev/null 2>&1; then
  sudo cp "$BINARY_PATH" "${INSTALL_DIR}/${BINARY_NAME}"
else
  print_err "No tienes permisos para escribir en ${INSTALL_DIR}."
  printf "  Intenta: ${BOLD}INSTALL_DIR=~/.local/bin sh install-local.sh %s${RESET}\n" "$ARCHIVE"
  rm -rf "$TMP_DIR"
  exit 1
fi

rm -rf "$TMP_DIR"

# ── macOS: quitar cuarentena de Gatekeeper ────────────────────────────────────
OS=$(uname -s | tr '[:upper:]' '[:lower:]')
if [ "$OS" = "darwin" ]; then
  print_step "Removiendo cuarentena de macOS (Gatekeeper)..."
  xattr -d com.apple.quarantine "${INSTALL_DIR}/${BINARY_NAME}" 2>/dev/null || true
  print_ok "Cuarentena removida"
fi

print_ok "Instalado en ${INSTALL_DIR}/${BINARY_NAME}"

# ── Verificar PATH ───────────────────────────────────────────────────────────
if ! command -v "$BINARY_NAME" >/dev/null 2>&1; then
  printf "\n  ${DIM}Nota: ${INSTALL_DIR} no esta en tu PATH.${RESET}\n"
  printf "  Agrega esta linea a tu ~/.zshrc o ~/.bashrc:\n"
  printf "\n    export PATH=\"${INSTALL_DIR}:\$PATH\"\n\n"
fi

# ── Listo ─────────────────────────────────────────────────────────────────────
printf "\n  ${GREEN}✔${RESET} ${BOLD}Listo!${RESET}\n\n"
printf "  Ejecuta ${BOLD}ingenierIA${RESET} desde cualquier directorio.\n"
printf "  La primera vez se te pedira configurar tu perfil.\n\n"
