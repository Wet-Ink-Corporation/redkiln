# The shared launcher

Every plugin starts its binary through this launcher (`AGENTS.md`, Plugin rules). A plugin ships two copies, named after its binary: `plugins/<plugin>/libexec/<binary>` (from `launcher`) and `libexec/<binary>.cmd` (from `launcher.cmd`). Builds go in `libexec/<target>/<binary>[.exe]`. Hooks call `${CLAUDE_PLUGIN_ROOT}/libexec/<binary>` by path in exec form; there is never a `plugins/<plugin>/bin/`.

What it does, in order:

1. Runs the newest build among the platform's targets (musl before gnu on Linux, msvc before gnu on Windows, unless the later one is newer).
2. Falls back to a `<binary>` on `PATH` that is not this launcher.
3. With nothing to run: a hook call (`<binary> hook <event> ...`) exits 0, guards included, and `hook SessionStart` prints a `systemMessage` saying the plugin is off (not in Cowork). Any other call exits 127.

A build's exit code is the launcher's, so a guard whose binary runs and fails still blocks. Hooks fail open only when the plugin has no build at all.

Not here yet: anneal's download-and-verify fallback for installs that carry no binaries. Whether it is needed depends on the org-sync and Cowork `libexec/` spikes (phase 0), so it waits for them.

The copies are byte-identical to these files, and CI checks that (`ci.yml`, plugins job). Tests: `sh tools/launcher/test.sh` (Linux, macOS) and `pwsh tools/launcher/test.ps1` (Windows).
