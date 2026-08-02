import { existsSync, rmSync } from "node:fs";
import { resolve } from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("..", import.meta.url));
const result = spawnSync(
  "wasm-pack",
  [
    "build",
    "crates/cassiopeia-wasm",
    "--target",
    "web",
    "--release",
    "--out-dir",
    "../../dist/wasm",
    "--out-name",
    "cassiopeia_wasm",
    "--no-pack",
    "--no-opt",
  ],
  {
    cwd: root,
    encoding: "utf8",
    shell: process.platform === "win32",
    stdio: "inherit",
  },
);

if (result.error) throw result.error;
if (result.status !== 0) process.exit(result.status ?? 1);

const generatedIgnore = resolve(root, "dist/wasm/.gitignore");
if (existsSync(generatedIgnore)) rmSync(generatedIgnore);
