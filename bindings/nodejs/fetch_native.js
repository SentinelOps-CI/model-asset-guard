#!/usr/bin/env node
"use strict";

/**
 * Thin wrapper: invoke repo fetch helper into bindings native dirs.
 * Honesty: does not embed or publish prebuilts; requires gh CLI.
 */

const { spawnSync } = require("child_process");
const fs = require("fs");
const path = require("path");

const root = path.resolve(__dirname, "..", "..");
const isWin = process.platform === "win32";
const script = isWin
  ? path.join(root, "scripts", "fetch_guardd_native.ps1")
  : path.join(root, "scripts", "fetch_guardd_native.sh");

if (!fs.existsSync(script)) {
  console.error(
    "Error: fetch helper not found at",
    script,
    "\nRun from a full git checkout of model-asset-guard (npm does not ship prebuilts)."
  );
  process.exit(2);
}

/** Map GNU-style flags to PowerShell param names for the .ps1 helper. */
function toPowershellArgs(argv) {
  const out = [];
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    switch (a) {
      case "--tag":
        out.push("-Tag", argv[++i] ?? "");
        break;
      case "--run-id":
        out.push("-RunId", argv[++i] ?? "");
        break;
      case "--from-ci":
        out.push("-FromCi");
        break;
      case "--from-release":
        out.push("-FromRelease");
        break;
      case "-h":
      case "--help":
        out.push("-Help");
        break;
      case "ubuntu-latest":
      case "macos-latest":
      case "windows-latest":
        out.push("-Os", a);
        break;
      default:
        // Bare run id (back-compat) or already-PowerShell-shaped args
        if (/^\d+$/.test(a)) {
          out.push("-RunId", a);
        } else {
          out.push(a);
        }
        break;
    }
  }
  return out;
}

const passthrough = process.argv.slice(2);
let cmd;
let args;
if (isWin) {
  cmd = "powershell";
  args = [
    "-NoProfile",
    "-ExecutionPolicy",
    "Bypass",
    "-File",
    script,
    ...toPowershellArgs(passthrough),
  ];
} else {
  cmd = "bash";
  args = [script, ...passthrough];
}

const result = spawnSync(cmd, args, { stdio: "inherit", cwd: root });
if (result.error) {
  console.error("Error: failed to spawn", cmd, result.error.message);
  process.exit(2);
}
process.exit(result.status === null ? 1 : result.status);
