#!/usr/bin/env node
"use strict";

// Wrapper JS que npm registra en el PATH global.
// Busca y ejecuta el binario nativo de Rust para la plataforma actual.
// El usuario solo escribe "ingenierIA" y funciona — sin configurar PATH manual.

const { execFileSync } = require("child_process");
const path = require("path");
const fs = require("fs");

const isWindows = process.platform === "win32";
const binaryName = isWindows ? "ingenieria.exe" : "ingenieria";

// 1. Buscar el binario que postinstall dejó junto a este script
const localBin = path.join(__dirname, binaryName);
if (fs.existsSync(localBin)) {
  run(localBin);
  return;
}

// 2. Buscar directamente en el paquete de plataforma instalado
const PLATFORM_MAP = {
  "darwin-arm64": "@your-org/ingenieria-darwin-arm64",
  "darwin-x64": "@your-org/ingenieria-darwin-x64",
  "win32-x64": "@your-org/ingenieria-win32-x64",
  "linux-x64": "@your-org/ingenieria-linux-x64",
};

const platformKey = `${process.platform}-${process.arch}`;
const pkg = PLATFORM_MAP[platformKey];

if (pkg) {
  try {
    const pkgDir = path.dirname(require.resolve(`${pkg}/package.json`));
    const pkgBin = path.join(pkgDir, "bin", binaryName);
    if (fs.existsSync(pkgBin)) {
      run(pkgBin);
      return;
    }
  } catch {
    // package not found, fall through
  }
}

console.error(
  `[ingenieria] No se encontro el binario para ${platformKey}.\n` +
    "Intenta: npm rebuild @your-org/ingenieria"
);
process.exit(1);

function run(binaryPath) {
  try {
    execFileSync(binaryPath, process.argv.slice(2), {
      stdio: "inherit",
      env: process.env,
    });
  } catch (err) {
    // execFileSync throws on non-zero exit — forward the exit code
    process.exit(err.status ?? 1);
  }
}
