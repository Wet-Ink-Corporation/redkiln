// CLI-11 and HK-11 with no build: every hook of every plugin must let Claude Code's call
// through when the plugin's copy holds no build at all.
//
//   node tools/launcher/no-build-check.mjs [plugins folder]
//
// For each `hooks.json` command entry it checks that `onFailure` is not "block" (Claude Code
// counts a hook that cannot start as a failure, so "block" would block every call it matches),
// then spawns the entry the way Claude Code does, exec form with no shell, from the source
// tree, which holds no builds:
//   - on Linux and macOS the launcher copy runs and must exit 0;
//   - on Windows the extensionless path resolves only to `<binary>.exe`, which a copy with no
//     build lacks, so the spawn must fail to start (ENOENT). That is the failure Claude Code then
//     reports as a hook error and lets through, since the entry does not block on failure.
// CI runs it in the launcher job on all three platforms (ci.yml).
import { spawnSync } from "node:child_process";
import { mkdtempSync, readdirSync, readFileSync, existsSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

const root = resolve(process.argv[2] ?? "plugins");
const windows = process.platform === "win32";
const cwd = mkdtempSync(join(tmpdir(), "no-build-"));
let failed = false;
let checked = 0;
const fail = (message) => {
  console.error(`FAIL ${message}`);
  failed = true;
};

for (const plugin of readdirSync(root).sort()) {
  const dir = join(root, plugin);
  const manifest = join(dir, "hooks", "hooks.json");
  if (!existsSync(manifest)) continue;
  const libexec = join(dir, "libexec");
  const builds = existsSync(libexec)
    ? readdirSync(libexec, { withFileTypes: true }).filter((e) => e.isDirectory() || e.name.endsWith(".exe"))
    : [];
  if (builds.length > 0) {
    fail(`${plugin}: libexec/ holds builds (${builds.map((e) => e.name).join(", ")}); run this on a source tree`);
    continue;
  }
  const { hooks = {} } = JSON.parse(readFileSync(manifest, "utf8"));
  for (const [event, groups] of Object.entries(hooks)) {
    for (const group of groups) {
      for (const hook of group.hooks ?? []) {
        if (hook.type !== "command" || !Array.isArray(hook.args)) continue;
        const name = `${plugin} ${event} ${hook.args.join(" ")}`;
        checked += 1;
        if (hook.onFailure === "block") {
          fail(`${name}: onFailure "block" would block every call when the binary cannot start (HK-11)`);
        }
        const command = hook.command.replace("${CLAUDE_PLUGIN_ROOT}", dir);
        const input = JSON.stringify({ hook_event_name: event, session_id: "no-build", cwd });
        const run = spawnSync(command, hook.args, {
          input,
          cwd,
          env: { ...process.env, CLAUDE_PLUGIN_ROOT: dir },
          timeout: 30_000,
        });
        if (windows) {
          if (run.error?.code === "ENOENT") {
            console.log(`ok   ${name}: fails to start (ENOENT), which Claude Code lets through`);
          } else {
            fail(`${name}: expected the spawn to fail to start with no build, got ${run.error ?? `exit ${run.status}`}`);
          }
        } else if (run.error || run.status !== 0) {
          fail(`${name}: ${run.error ?? `exit ${run.status}`}\n${run.stderr}`);
        } else {
          console.log(`ok   ${name}: the launcher exits 0 with no build`);
        }
      }
    }
  }
}
if (checked === 0) fail(`no hooks.json command entries under ${root}`);
process.exit(failed ? 1 : 0);
