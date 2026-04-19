#!/usr/bin/env node
"use strict";

// Postinstall: copia el binario nativo desde el paquete de plataforma
// hacia bin/ (junto al wrapper) para acceso rapido.

const fs = require("fs");
const path = require("path");

const PLATFORM_MAP = {
  "darwin-arm64": "@your-org/ingenieria-darwin-arm64",
  "darwin-x64": "@your-org/ingenieria-darwin-x64",
  "win32-x64": "@your-org/ingenieria-win32-x64",
  "linux-x64": "@your-org/ingenieria-linux-x64",
};

const platformKey = `${process.platform}-${process.arch}`;
const pkg = PLATFORM_MAP[platformKey];

if (!pkg) {
  console.warn(
    `[ingenieria] Plataforma no soportada: ${platformKey}. ` +
      `El wrapper intentara buscar el binario en runtime.`
  );
  process.exit(0);
}

let pkgDir;
try {
  pkgDir = path.dirname(require.resolve(`${pkg}/package.json`));
} catch {
  console.warn(
    `[ingenieria] Paquete ${pkg} no encontrado. ` +
      `El wrapper intentara buscar el binario en runtime.`
  );
  process.exit(0);
}

const isWindows = process.platform === "win32";
const binaryName = isWindows ? "ingenieria.exe" : "ingenieria";
const src = path.join(pkgDir, "bin", binaryName);
const dest = path.join(__dirname, "..", "bin", binaryName);

if (!fs.existsSync(src)) {
  console.warn(`[ingenieria] Binario no encontrado en ${src}. Se buscara en runtime.`);
  process.exit(0);
}

fs.copyFileSync(src, dest);

if (!isWindows) {
  fs.chmodSync(dest, 0o755);
}

console.log(`[ingenieria] Instalado para ${platformKey}`);
