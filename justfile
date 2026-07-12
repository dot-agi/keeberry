# SPDX-License-Identifier: GPL-2.0-or-later
#
# Convenience targets for the keeberry monorepo. The firmware staging and the
# Tauri build live in app/package.json (so they work without `just`); these
# recipes are thin wrappers that document the canonical entry points.

# List available recipes.
default:
    @just --list

# Scaffold a new firmware feature + its configurator wiring end-to-end (cargo xtask, see
# .planning/sdk-llm-friendly.md §4). KIND is toggle | config | keycode; the scaffolder
# allocates the next-free ids, stamps the files, wires every `// @scaffold:` anchor, and
# prints the TODO(behavior) checklist + the validate command.
new-feature NAME KIND="toggle":
    cargo xtask new-feature {{NAME}} --kind {{KIND}}

# Build the firmware and stage keeberry.bin + firmware.json for the native bundle.
stage-firmware:
    cd app && npm run stage:firmware

# Build the native desktop app (pretauri:build stages the firmware first).
tauri-build:
    cd app && npm run tauri:build

# Run the web app test/lint/format gates.
check:
    cd app && npm run lint && npm run test && npm run format:check
