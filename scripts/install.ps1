# ingenierIA TUI — Instalador Windows
# Uso: irm https://raw.githubusercontent.com/your-org/ingenieria-tui/main/scripts/install.ps1 | iex
# ──────────────────────────────────────────────────────────────────────────────
#Requires -Version 5.1
$ErrorActionPreference = "Stop"

$Repo      = "your-org/ingenieria-tui"
$InstDir   = "$env:LOCALAPPDATA\ingenieria"
$BinPath   = "$InstDir\ingenieria.exe"
$Target    = "x86_64-pc-windows-msvc"

function Write-Step  { param($msg) Write-Host "  > $msg" -ForegroundColor Cyan }
function Write-Ok    { param($msg) Write-Host "  OK $msg" -ForegroundColor Green }
function Write-Fail  { param($msg) Write-Host "  ERROR $msg" -ForegroundColor Red; exit 1 }

Write-Host ""
Write-Host "  ingenierIA TUI Installer" -ForegroundColor White
Write-Host "  ─────────────────────────────────────────────" -ForegroundColor DarkGray
Write-Host ""

# ── Obtener la última versión ─────────────────────────────────────────────────
Write-Step "Buscando la ultima version..."
try {
    $Release = Invoke-RestMethod "https://api.github.com/repos/$Repo/releases/latest" `
        -Headers @{ "User-Agent" = "ingenieria-installer" }
    $Version = $Release.tag_name
} catch {
    Write-Fail "No se pudo obtener la version. Verifica tu conexion a internet."
}

Write-Host "  ingenierIA TUI $Version para Windows x64`n" -ForegroundColor Gray

# ── Crear directorio de instalación ──────────────────────────────────────────
Write-Step "Instalando en $InstDir..."
New-Item -ItemType Directory -Force -Path $InstDir | Out-Null

# ── Descargar el archivo ──────────────────────────────────────────────────────
$Archive    = "ingenieria-$Target.zip"
$DownloadUrl = "https://github.com/$Repo/releases/download/$Version/$Archive"
$TempZip     = "$env:TEMP\$Archive"

Write-Step "Descargando desde GitHub Releases..."
try {
    Invoke-WebRequest -Uri $DownloadUrl -OutFile $TempZip -UseBasicParsing
} catch {
    Write-Fail "Descarga fallida. URL: $DownloadUrl"
}

# ── Extraer e instalar ────────────────────────────────────────────────────────
Expand-Archive -Path $TempZip -DestinationPath $env:TEMP -Force
Copy-Item "$env:TEMP\ingenieria.exe" $BinPath -Force
Remove-Item $TempZip -Force

Write-Ok "Binario instalado en $BinPath"

# ── Agregar al PATH del usuario ───────────────────────────────────────────────
$UserPath = [Environment]::GetEnvironmentVariable("PATH", "User")
if ($UserPath -notlike "*$InstDir*") {
    Write-Step "Agregando $InstDir al PATH de usuario..."
    [Environment]::SetEnvironmentVariable("PATH", "$UserPath;$InstDir", "User")
    Write-Ok "PATH actualizado (abre una nueva terminal para que tenga efecto)"
} else {
    Write-Ok "$InstDir ya esta en el PATH"
}

# ── Listo ─────────────────────────────────────────────────────────────────────
Write-Host ""
Write-Host "  ingenierIA TUI instalado correctamente." -ForegroundColor Green
Write-Host ""
Write-Host "  Abre una nueva terminal y ejecuta: ingenierIA" -ForegroundColor White
Write-Host "  La primera vez se te pedira configurar tu perfil." -ForegroundColor Gray
Write-Host ""

# Intentar lanzar directamente si ingenieria.exe ya está accesible
if (Test-Path $BinPath) {
    Write-Host "  Iniciando ingenierIA TUI..." -ForegroundColor Cyan
    Start-Process $BinPath -NoNewWindow -Wait
}
