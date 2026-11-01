# Cross-platform

Ryan works in Git Bash on Windows; CI runs Linux, Windows and macOS; users meet the plugins in the Windows and macOS desktop apps, cloud containers and Cowork. Every repository has a Windows chapter in its history. This one collects them.

## Guardrails

### XP-1 LF everywhere, pinned first in `.gitattributes`

`* text=auto eol=lf` is the first row (git applies the last matching row, so later rows refine it); `*.cmd` is CRLF; fixtures whose digest covers their own bytes are `-text`; fonts and other binaries are named `binary` rather than trusted to detection. A CI check that no tracked text file contains CR backs it up.

- **Why.** System-level `core.autocrlf=true` on the operator's machine smudged 270 working files, and 1,480 of 1,757 in a run-owned worktree, so "no checkout on this box was byte-faithful". CRLF in a packaged workflow script made the Workflow harness reject it ("control characters that would be hidden in the approval dialog").
- **Evidence.** redkiln-rs ADR-0164, ADR-0151, `.gitattributes`; redkiln-old `.gitattributes`; redkiln `.gitattributes`.

### XP-2 Rust calls the `.exe`, never a `.cmd`

Rust code invokes binaries directly. Since Rust 1.77.2 `Command` refuses batch-file arguments it cannot escape safely (CVE-2024-24576), and exec-form hooks cannot spawn a `.cmd` at all (`EINVAL`). Treat `InvalidInput` as a typed error and ban `raw_arg`.

- **Evidence.** weigh-in B-16; prior-art/platform.md §6.

### XP-3 Compare paths after resolving them, and never hand git a verbatim path

`fs::canonicalize` returns `\\?\D:\…`, which slash-flipped becomes `//?/D:/…`, a URL to git. Cargo reports verbatim paths; `%TEMP%` on hosted runners is the 8.3 form `C:\Users\RUNNER~1` while git reports long paths; macOS `TMPDIR` sits under the `/var` to `/private/var` symlink. Compare real paths whenever a lexical path leaves the root, strip the verbatim prefix in one place, and point `TMP`/`TEMP` at `RUNNER_TEMP` in CI.

- **Evidence.** redkiln-rs F20 defect 2, `@ef747da`; temper `@a485408`, `@2d8e9a4`, `@8be0683`.

### XP-4 Deadlines are enforced in-process, and generously on Windows

`timeout` on PATH can be `System32\TIMEOUT.EXE`. Windows runners ran about 5x slower than Linux in one suite and about 2x in another; "catch an infinite loop" bounds are 30 s, not 5. Poll for a recorded deadline instead of sleeping a fixed time, and remember whole-second clocks: a gate opened at second S expires only from S+2.0.

- **Evidence.** weigh-in `@ab89c71`, `@ae3ccb0`; temper #155; redkiln-rs `@aab766c`.

### XP-5 Windows reuses PIDs fast and holds files open

Markers named by PID collide (two sequential children with the same PID left one marker); hard-link to the first free name instead. A detached child holds directories open (`EBUSY` in cleanup); `mkdir` on a just-deleted directory throws `EPERM`, not `EEXIST` (11 of 3,600 contended acquisitions). Retry those codes symmetrically on acquire and release; keep a tree that will not delete and never reuse it. Append-mode handles cannot truncate on Windows.

- **Evidence.** redkiln-rs `@4a36f19`, Halt 6; temper `@797dc55`, `@f878324`; redkiln-old `CHANGELOG.md:160-180` (#212).

### XP-6 Never trust a binary you did not just build

`cargo test` and `cargo clippy` do not rebuild the CLI binary in `target/`, and `fs::copy` on Windows keeps the source mtime, so cargo may skip a rebuild. Rebuild before any check that runs the binary. Rebuild every binary you keep after a change, because two builds can reopen one store under different journal modes.

- **Evidence.** redkiln-rs Halt 1, `@c51826b` ("a stale binary is how this programme reached two false conclusions"); weigh-in B-05.

## Patterns worth copying

- **A `.cmd` launcher that survives Git for Windows**: compare file ages with `xcopy /D /L` and match the file name, not the localised summary, and name `System32\find.exe` by path so Git's Unix `find` is never picked (weigh-in `plugin/libexec/weighin.cmd`).
- **Baseline counts per host triple**: `cfg(windows)` code changes clippy counts, so a Linux-recorded baseline was always red on Windows. Keep counts per triple, and a host with no section stays red with a message naming the fix (temper wi-2771a4, #141).
- **Hard-link to publish atomically without clobbering**, where rename would overwrite (redkiln-rs `@4a36f19`).
- **Write executables in tests through a child and a rename**: if a sibling test forks while an executable's write descriptor is open, exec fails with `ETXTBSY`, and renaming keeps the same inode. Have a child `install -m 0755` into a staging directory and rename into place (assay #176, `@08bc61c`).

## Gotchas

- **Without Git for Windows, shell-form hooks run under PowerShell**, where `sh` is missing, and capture was silently off; exec form avoids it (weigh-in D-0015).
- **PowerShell 5's `Set-Content -Encoding UTF8` writes a BOM** that broke config parsing (weigh-in CHANGELOG 0.9.0).
- **Hooks start `node` or the binary with no shell**, so `.cmd` shims from version managers fail and PATH must be set in the user environment, not `$PROFILE` (temper `docs/troubleshooting.md:185-193`).
- **The Windows desktop app's skill shell cannot see the plugin folder**; the cause is unknown and B-26's copy-to-git-dir works around it (weigh-in B-26).
- **`\` must be refused in session ids used as file names** (weigh-in B-28).
- **`Path::join` inserts `\` on Windows**, which broke a POSIX `export PATH=` line (anneal `@209ebd3`).
- **Process spawn on Windows cost about 210 ms per fixture repository**, and vitest ran serially on 2-vCPU runners unnoticed (89-minute Windows leg) (redkiln-old `CHANGELOG.md:715-823`).
- **Node's default `maxBuffer` (1 MiB) failed a successful noisy build**, and a timeout killed the shell, not the process beneath it (redkiln-old native-repo seed, A4-09). Cap output deliberately and kill the group.

## Prior art

- Git Bash rewrites arguments that look like POSIX paths (`-L/regex/`, `ref:path`); `MSYS_NO_PATHCONV=1` disables it. **Adopt** in the sh launcher under MSYS, with `<ref>:<path>` cases in the launcher's Windows tests. [Stopping path conversion for Git Bash](https://docs.cloudbees.com/docs/cloudbees-cd-kb/latest/cloudbees-cd-kb/kbec-00420-stopping-path-conversion-for-git-bash)
- Rust adds `\\?\` to absolute paths automatically, but git needs `core.longpaths=true`. **Adopt** `-c core.longpaths=true` in the git wrapper on Windows. [rust-lang/rust #89174](https://github.com/rust-lang/rust/pull/89174)
- Windows Job Objects with `KILL_ON_JOB_CLOSE` kill a whole tree; tokio's `kill_on_drop` kills only the direct child. **Adopt** for every supervised child. [tokio Command](https://docs.rs/tokio/latest/tokio/process/struct.Command.html)
- Line-ending normalisation: `* text=auto eol=lf` with CRLF exceptions and one `git add --renormalize`. **Agree; adopt** a CRLF exception for `*.ps1` if launcher tests need it. [Git and normalization of line endings](https://dev.to/kevinshu/git-and-normalization-of-line-endings-228j)
