; ingenierIA TUI — NSIS Installer Script
; ──────────────────────────────────────────────────────────────────────────────
; Genera un .exe instalador para Windows con:
;   - Instalación en Program Files
;   - Agrega al PATH del sistema
;   - Desinstalador incluido
;   - Entrada en "Agregar o quitar programas"
; ──────────────────────────────────────────────────────────────────────────────

!include "MUI2.nsh"

; ── Metadata ─────────────────────────────────────────────────────────────────
!define PRODUCT_NAME    "ingenierIA"
!ifndef PRODUCT_VERSION
  !error "PRODUCT_VERSION no definida. Usa: makensis -DPRODUCT_VERSION=x.y.z ingenieria.nsi"
!endif
!define PRODUCT_PUBLISHER "your-org"
!define PRODUCT_EXE     "ingenieria.exe"
!define PRODUCT_DIR     "ingenierIA"

Name "${PRODUCT_NAME} ${PRODUCT_VERSION}"
OutFile "ingenieria-${PRODUCT_VERSION}-windows-x64-setup.exe"
InstallDir "$PROGRAMFILES64\${PRODUCT_DIR}"
InstallDirRegKey HKLM "Software\${PRODUCT_NAME}" "InstallDir"
RequestExecutionLevel admin

; ── UI ───────────────────────────────────────────────────────────────────────
!define MUI_ICON "ingenieria.ico"
!define MUI_ABORTWARNING
!define MUI_WELCOMEPAGE_TITLE "Instalar ${PRODUCT_NAME}"
!define MUI_WELCOMEPAGE_TEXT "Este asistente instalará ${PRODUCT_NAME} ${PRODUCT_VERSION} en tu computador.$\r$\n$\r$\ningenierIA es una herramienta de terminal para interactuar con ingenierIA MCP Server.$\r$\n$\r$\nHaz clic en Siguiente para continuar."
!define MUI_FINISHPAGE_TITLE "Instalación completada"
!define MUI_FINISHPAGE_TEXT "${PRODUCT_NAME} se instaló correctamente.$\r$\n$\r$\nAbre una terminal (cmd o PowerShell) y ejecuta:$\r$\n$\r$\n  ingenierIA$\r$\n$\r$\nLa primera vez se te pedirá configurar tu perfil."

; ── Páginas ──────────────────────────────────────────────────────────────────
!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH

!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES

!insertmacro MUI_LANGUAGE "Spanish"

; ── Sección de instalación ───────────────────────────────────────────────────
Section "Instalar"
  SetOutPath "$INSTDIR"

  ; Copiar el binario
  File "${PRODUCT_EXE}"

  ; Guardar ruta de instalación en registro
  WriteRegStr HKLM "Software\${PRODUCT_NAME}" "InstallDir" "$INSTDIR"

  ; Crear desinstalador
  WriteUninstaller "$INSTDIR\uninstall.exe"

  ; Entrada en Agregar/Quitar programas
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\${PRODUCT_NAME}" \
    "DisplayName" "${PRODUCT_NAME}"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\${PRODUCT_NAME}" \
    "DisplayVersion" "${PRODUCT_VERSION}"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\${PRODUCT_NAME}" \
    "Publisher" "${PRODUCT_PUBLISHER}"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\${PRODUCT_NAME}" \
    "UninstallString" "$\"$INSTDIR\uninstall.exe$\""
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\${PRODUCT_NAME}" \
    "InstallLocation" "$INSTDIR"
  WriteRegDWORD HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\${PRODUCT_NAME}" \
    "NoModify" 1
  WriteRegDWORD HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\${PRODUCT_NAME}" \
    "NoRepair" 1

  ; Agregar al PATH del sistema via PowerShell (compatible con NSIS 3.x)
  nsExec::ExecToLog 'powershell -ExecutionPolicy Bypass -Command "\
    $$p = [Environment]::GetEnvironmentVariable(\"Path\", \"Machine\"); \
    if ($$p -notlike \"*$INSTDIR*\") { \
      [Environment]::SetEnvironmentVariable(\"Path\", \"$$p;$INSTDIR\", \"Machine\") \
    }"'
SectionEnd

; ── Sección de desinstalación ────────────────────────────────────────────────
Section "Uninstall"
  ; Quitar archivos
  Delete "$INSTDIR\${PRODUCT_EXE}"
  Delete "$INSTDIR\uninstall.exe"
  RMDir "$INSTDIR"

  ; Quitar del PATH del sistema via PowerShell
  nsExec::ExecToLog 'powershell -ExecutionPolicy Bypass -Command "\
    $$p = [Environment]::GetEnvironmentVariable(\"Path\", \"Machine\"); \
    $$p = ($$p.Split(\";\") | Where-Object { $$_ -ne \"$INSTDIR\" }) -join \";\"; \
    [Environment]::SetEnvironmentVariable(\"Path\", $$p, \"Machine\")"'

  ; Limpiar registro
  DeleteRegKey HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\${PRODUCT_NAME}"
  DeleteRegKey HKLM "Software\${PRODUCT_NAME}"
SectionEnd
