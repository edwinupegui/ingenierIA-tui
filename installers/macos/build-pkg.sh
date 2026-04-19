#!/usr/bin/env sh
# ingenierIA TUI — Generador de .pkg para macOS
# ──────────────────────────────────────────────────────────────────────────────
# Uso:  sh build-pkg.sh <ruta-al-binario-ingenieria> [version]
# Ejemplo:  sh build-pkg.sh ./ingenieria 0.1.0
# Si no se pasa versión, se lee automáticamente de Cargo.toml
# Produce:  ingenieria-<version>-macos-installer.pkg
# ──────────────────────────────────────────────────────────────────────────────
set -e

BINARY="${1:?Uso: sh build-pkg.sh <binario-ingenieria> [version]}"
CARGO_TOML="$(cd "$(dirname "$0")/../.." && pwd)/Cargo.toml"
DEFAULT_VERSION=$(grep '^version' "$CARGO_TOML" | head -1 | sed 's/.*"\(.*\)".*/\1/')
VERSION="${2:-$DEFAULT_VERSION}"
IDENTIFIER="com.your-org.ingenieria"
INSTALL_LOCATION="/usr/local/bin"
PKG_NAME="ingenieria-${VERSION}-macos-installer.pkg"

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
WORK_DIR=$(mktemp -d)

echo "  ▸ Preparando payload..."
mkdir -p "${WORK_DIR}/payload${INSTALL_LOCATION}"
cp "$BINARY" "${WORK_DIR}/payload${INSTALL_LOCATION}/ingenieria"
chmod +x "${WORK_DIR}/payload${INSTALL_LOCATION}/ingenieria"

# ── Scripts de post-instalación ──────────────────────────────────────────────
mkdir -p "${WORK_DIR}/scripts"
cat > "${WORK_DIR}/scripts/postinstall" << 'POSTINSTALL'
#!/bin/sh
# Quitar cuarentena de Gatekeeper
xattr -d com.apple.quarantine /usr/local/bin/ingenieria 2>/dev/null || true
echo ""
echo "  ✔ ingenierIA TUI instalado correctamente."
echo ""
echo "  Abre una terminal y ejecuta: ingenierIA"
echo "  La primera vez se te pedirá configurar tu perfil."
echo ""
POSTINSTALL
chmod +x "${WORK_DIR}/scripts/postinstall"

# ── Generar componente .pkg ──────────────────────────────────────────────────
echo "  ▸ Generando paquete..."
pkgbuild \
  --root "${WORK_DIR}/payload" \
  --scripts "${WORK_DIR}/scripts" \
  --identifier "$IDENTIFIER" \
  --version "$VERSION" \
  --install-location "/" \
  "${WORK_DIR}/ingenieria-component.pkg"

# ── Generar distribution XML (para UI de instalación bonita) ─────────────────
cat > "${WORK_DIR}/distribution.xml" << DIST
<?xml version="1.0" encoding="utf-8"?>
<installer-gui-script minSpecVersion="2">
    <title>ingenierIA TUI ${VERSION}</title>
    <welcome language="es" mime-type="text/plain"><![CDATA[
Bienvenido al instalador de ingenierIA TUI.

ingenierIA es una herramienta de terminal para interactuar con ingenierIA MCP Server.

Se instalará el comando "ingenierIA" en /usr/local/bin.
    ]]></welcome>
    <conclusion language="es" mime-type="text/plain"><![CDATA[
¡ingenierIA TUI se instaló correctamente!

Abre una terminal y ejecuta:

  ingenierIA

La primera vez se te pedirá configurar tu perfil.
    ]]></conclusion>
    <options customize="never" require-scripts="false"/>
    <choices-outline>
        <line choice="default"/>
    </choices-outline>
    <choice id="default" title="ingenierIA TUI">
        <pkg-ref id="${IDENTIFIER}"/>
    </choice>
    <pkg-ref id="${IDENTIFIER}" version="${VERSION}" onConclusion="none">ingenieria-component.pkg</pkg-ref>
</installer-gui-script>
DIST

# ── Generar producto final .pkg ──────────────────────────────────────────────
productbuild \
  --distribution "${WORK_DIR}/distribution.xml" \
  --package-path "${WORK_DIR}" \
  "$PKG_NAME"

rm -rf "$WORK_DIR"

echo "  ✔ Instalador generado: ${PKG_NAME}"
echo ""
echo "  Tu compañero solo necesita hacer doble clic en el .pkg"
