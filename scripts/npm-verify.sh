#!/bin/bash
# Verifica que los paquetes npm NO contengan source code ni secretos.
# Ejecutar ANTES de publicar manualmente.
# Uso: ./scripts/npm-verify.sh

set -euo pipefail

RED='\033[0;31m'
GREEN='\033[0;32m'
NC='\033[0m'

FAIL=0

DANGEROUS_PATTERNS='\.rs$|\.ts$|\.tsx$|\.map$|\.toml$|\.lock$|\.env$|\.pem$|\.key$|credentials|src/|target/|\.github/'

echo "==> Verificando paquetes npm..."
echo ""

for dir in npm/ingenieria npm/ingenieria-darwin-arm64 npm/ingenieria-darwin-x64 npm/ingenieria-win32-x64 npm/ingenieria-linux-x64; do
  if [ ! -d "$dir" ]; then
    echo -e "${RED}SKIP${NC}: $dir no existe"
    continue
  fi

  echo "--- $dir ---"
  packed=$(cd "$dir" && npm pack --dry-run 2>&1)

  # Mostrar contenido
  echo "$packed" | grep -E '^\d|Tarball|npm notice' | head -20

  # Buscar archivos peligrosos
  leaks=$(echo "$packed" | grep -iE "$DANGEROUS_PATTERNS" || true)
  if [ -n "$leaks" ]; then
    echo -e "${RED}PELIGRO: archivos sospechosos detectados:${NC}"
    echo "$leaks"
    FAIL=1
  else
    echo -e "${GREEN}OK${NC}: sin source code ni secretos"
  fi
  echo ""
done

if [ $FAIL -ne 0 ]; then
  echo -e "${RED}==> FALLO: hay archivos peligrosos en los paquetes. NO publicar.${NC}"
  exit 1
fi

echo -e "${GREEN}==> TODO OK: seguro para publicar${NC}"
