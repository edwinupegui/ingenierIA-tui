# ingenierIA TUI — Instalador local Windows (repo privado)
# ──────────────────────────────────────────────────────────────────────────────
# Uso:  .\install-local.ps1 ingenieria-x86_64-pc-windows-msvc.zip
#       .\install-local.ps1  (auto-detecta el .zip en el directorio actual)
# ──────────────────────────────────────────────────────────────────────────────
#Requires -Version 5.1
$ErrorActionPreference = "Stop"

$BinaryName = "ingenieria.exe"
$InstDir    = "$env:LOCALAPPDATA\ingenieria"

function Write-Step  { param($msg) Write-Host "  > $msg" -ForegroundColor Cyan }
function Write-Ok    { param($msg) Write-Host "  OK $msg" -ForegroundColor Green }
function Write-Fail  { param($msg) Write-Host "  ERROR $msg" -ForegroundColor Red; exit 1 }

# ── Resolver archivo ─────────────────────────────────────────────────────────
if ($args.Count -gt 0) {
    $Archive = $args[0]
} else {
    $Archive = Get-ChildItem -Filter "ingenieria-*.zip" -ErrorAction SilentlyContinue | Select-Object -First 1
    if (-not $Archive) {
        Write-Fail "No se encontro ningun archivo ingenieria-*.zip`n`n  Uso:`n    .\install-local.ps1 ingenieria-x86_64-pc-windows-msvc.zip`n`n  O coloca el archivo .zip en este directorio y ejecuta:`n    .\install-local.ps1"
    }
    $Archive = $Archive.Name
}

if (-not (Test-Path $Archive)) {
    Write-Fail "Archivo no encontrado: $Archive"
}

Write-Host ""
Write-Host "  ingenierIA TUI - Instalador local" -ForegroundColor White
Write-Host "  Archivo: $Archive" -ForegroundColor DarkGray
Write-Host ""

# ── Crear directorio de instalacion ──────────────────────────────────────────
Write-Step "Instalando en $InstDir..."
New-Item -ItemType Directory -Force -Path $InstDir | Out-Null

# ── Extraer e instalar ───────────────────────────────────────────────────────
$TempDir = Join-Path $env:TEMP "ingenieria-install"
if (Test-Path $TempDir) { Remove-Item $TempDir -Recurse -Force }
New-Item -ItemType Directory -Force -Path $TempDir | Out-Null

Write-Step "Extrayendo..."
Expand-Archive -Path $Archive -DestinationPath $TempDir -Force

$BinSource = Get-ChildItem -Path $TempDir -Filter $BinaryName -Recurse | Select-Object -First 1
if (-not $BinSource) {
    Remove-Item $TempDir -Recurse -Force
    Write-Fail "No se encontro $BinaryName dentro del archivo."
}

Copy-Item $BinSource.FullName (Join-Path $InstDir $BinaryName) -Force
Remove-Item $TempDir -Recurse -Force

Write-Ok "Binario instalado en $InstDir\$BinaryName"

# ── Agregar al PATH del usuario ──────────────────────────────────────────────
$UserPath = [Environment]::GetEnvironmentVariable("PATH", "User")
if ($UserPath -notlike "*$InstDir*") {
    Write-Step "Agregando $InstDir al PATH de usuario..."
    [Environment]::SetEnvironmentVariable("PATH", "$UserPath;$InstDir", "User")
    Write-Ok "PATH actualizado (abre una nueva terminal para que tenga efecto)"
} else {
    Write-Ok "$InstDir ya esta en el PATH"
}

# ── Desbloquear binario (Windows SmartScreen) ────────────────────────────────
$BinPath = Join-Path $InstDir $BinaryName
Write-Step "Desbloqueando binario (SmartScreen)..."
Unblock-File -Path $BinPath -ErrorAction SilentlyContinue
Write-Ok "Binario desbloqueado"

# ── Listo ─────────────────────────────────────────────────────────────────────
Write-Host ""
Write-Host "  ingenierIA TUI instalado correctamente." -ForegroundColor Green
Write-Host ""
Write-Host "  Abre una nueva terminal y ejecuta: ingenierIA" -ForegroundColor White
Write-Host "  La primera vez se te pedira configurar tu perfil." -ForegroundColor Gray
Write-Host ""
