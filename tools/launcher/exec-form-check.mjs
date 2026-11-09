// M1-E: on Windows an exec-form hook (`${CLAUDE_PLUGIN_ROOT}/libexec/weighin`, no shell) must run
// with no Git Bash on PATH. Claude Code spawns hooks through Node, whose spawn resolves an
// extensionless path to `<path>.exe` (never `.cmd`, B-16), so the packaged plugin's root
// `libexec/weighin.exe` is what runs. This spawns it the same way, with PATH cut to System32.
//
//   node tools/launcher/exec-form-check.mjs <packaged plugin folder>
//
// CI runs it on Windows after `cargo xtask build` and `cargo xtask package` (ci.yml, rust-windows).
import { spawnSync } from "node:child_process";
import { mkdtempSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const plugin = process.argv[2];
if (!plugin) {
  console.error("usage: node exec-form-check.mjs <packaged plugin folder>");
  process.exit(2);
}
const cwd = mkdtempSync(join(tmpdir(), "exec-form-"));
const system32 = join(process.env.SystemRoot ?? "C:\\Windows", "System32");
const env = {
  PATH: system32,
  SystemRoot: process.env.SystemRoot,
  TEMP: process.env.TEMP,
  TMP: process.env.TMP,
  USERPROFILE: process.env.USERPROFILE,
};
let failed = false;
for (const event of ["SessionStart", "Stop"]) {
  const input = JSON.stringify({ hook_event_name: event, session_id: "exec-form", cwd, source: "startup" });
  const run = spawnSync(join(plugin, "libexec", "weighin"), ["hook", event], { input, env, cwd, timeout: 30_000 });
  if (run.error || run.status !== 0) {
    console.error(`FAIL ${event}: ${run.error ?? `exit ${run.status}`}\n${run.stderr}`);
    failed = true;
  } else {
    console.log(`ok   ${event} ran through libexec/weighin with no Git Bash`);
  }
}
process.exit(failed ? 1 : 0);
