// SPDX-License-Identifier: GPL-2.0-or-later
//
// Build the keeberry firmware from the current commit and stage it for the
// native desktop bundle. Produces two Tauri `resource` artifacts the app ships
// and flashes:
//
//   app/src-tauri/resources/keeberry.bin    the raw flash image (objcopy of the ELF)
//   app/src-tauri/resources/firmware.json   { "version": "<firmware Cargo.toml>" }
//
// Both are build artifacts, regenerated here and gitignored — never committed —
// so the app can never ship a stale image or a version stamp that disagrees with
// the binary it carries. `package.json`'s `pretauri:build` runs this before every
// `tauri build`, so a release bundle always carries a freshly-built image.

import { execFileSync } from 'node:child_process';
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const scriptDir = dirname(fileURLToPath(import.meta.url));
// app/scripts -> repo root.
const repoRoot = resolve(scriptDir, '..', '..');

const firmwareDir = join(repoRoot, 'firmware');
const firmwareCargoToml = join(firmwareDir, 'Cargo.toml');
// The firmware workspace pins thumbv7m-none-eabi via firmware/.cargo/config.toml,
// so the release ELF lands at this fixed path regardless of the host triple.
const firmwareElf = join(firmwareDir, 'target', 'thumbv7m-none-eabi', 'release', 'keeberry');

const resourcesDir = join(repoRoot, 'app', 'src-tauri', 'resources');
const binOut = join(resourcesDir, 'keeberry.bin');
const manifestOut = join(resourcesDir, 'firmware.json');

/** Read the `[package]` version from a Cargo.toml without a TOML dependency. */
function readPackageVersion(tomlPath) {
  const lines = readFileSync(tomlPath, 'utf8').split(/\r?\n/);
  let inPackage = false;
  for (const line of lines) {
    const trimmed = line.trim();
    if (trimmed.startsWith('[')) {
      inPackage = trimmed === '[package]';
      continue;
    }
    if (inPackage) {
      const match = trimmed.match(/^version\s*=\s*"([^"]+)"/);
      if (match) {
        return match[1];
      }
    }
  }
  throw new Error(`no [package] version found in ${tomlPath}`);
}

function run(command, args, cwd) {
  execFileSync(command, args, { cwd, stdio: 'inherit' });
}

mkdirSync(resourcesDir, { recursive: true });

console.log('[stage-firmware] building keeberry firmware (release)…');
run('cargo', ['build', '-p', 'keeberry', '--release'], firmwareDir);

console.log('[stage-firmware] objcopy ELF -> keeberry.bin');
try {
  run('rust-objcopy', ['-O', 'binary', firmwareElf, binOut]);
} catch (err) {
  // rust-objcopy ships with the llvm-tools component via cargo-binutils; surface
  // that rather than a bare ENOENT so the fix is obvious.
  throw new Error(
    `rust-objcopy failed (${err.message}). Install it with: ` +
      'rustup component add llvm-tools && cargo install cargo-binutils',
  );
}

const version = readPackageVersion(firmwareCargoToml);
writeFileSync(manifestOut, `${JSON.stringify({ version }, null, 2)}\n`);

console.log(`[stage-firmware] staged firmware v${version} -> ${binOut}`);
