# Installing Weigh-In

For using Weigh-In. Building it from source, for development, is in the
[README](../README.md#install). `Wet-Ink-Corporation/weigh-in` is public (B-33), so the routes
below that clone it need no access; the release repository `weigh-in-plugin` stays internal.

## Through your organization (recommended)

Each release is pushed to `Wet-Ink-Corporation/weigh-in-plugin`, a release-only repository
holding the built plugin with its Linux, macOS and Windows binaries (B-19). An organization that
syncs it gets every release in Cowork and in Claude Code with no step of anyone's. Once, as an
owner:

1. Create `Wet-Ink-Corporation/weigh-in-plugin`, private, with no files.
2. Make an SSH key pair (`ssh-keygen -t ed25519 -N "" -f weigh-in-plugin`). Add the public key to
   that repository under Settings > Deploy keys, with write access, and the private key to
   `Wet-Ink-Corporation/weigh-in` under Settings > Secrets and variables > Actions as
   `WEIGH_IN_PLUGIN_DEPLOY_KEY`. The release refuses to publish without it.
3. On claude.ai, Organization settings > Plugins & skills: sync from GitHub
   `Wet-Ink-Corporation/weigh-in-plugin` (not `weigh-in`, whose `main` is the unreleased tree),
   leave **Sync automatically** on, and set `weigh` to installed by default or required.
4. Remove other installs, so `/weigh-in` appears once: an uploaded zip under Customize >
   Plugins, and in Claude Code `/plugin marketplace remove weigh-in` if you added the
   `#marketplace` branch (below).

The first release after step 2 fills the repository; until then there is nothing to sync. For
`Wet-Ink-Corporation` steps 1 and 2 are done, and the repository holds the latest release.

## Claude Code without the organization sync

Each release publishes a `marketplace` branch: one commit holding the plugin with its Linux,
macOS (Apple silicon) and Windows binaries (D-0009). No Rust toolchain is needed.

1. Let git reach the private repository without prompting. With the GitHub CLI:

   ```sh
   gh auth login
   gh auth setup-git
   ```

   Background updates run git with no prompt, through a credential helper (`gh`, the macOS
   Keychain, Git Credential Manager) or an SSH key in `ssh-agent`. A `GITHUB_TOKEN` in the
   environment is not enough on its own.
2. In Claude Code:

   ```
   /plugin marketplace add Wet-Ink-Corporation/weigh-in#marketplace
   /plugin install weigh@weigh-in
   /plugin install weigh-mod@weigh-in
   ```

   `weigh-mod` is the ledger band, the board pane and `/wi` (below), Claude Code only. It is a
   separate plugin because a desktop chat turns off any plugin that ships a mod (B-35).

3. Turn on auto-update: `/plugin` → **Marketplaces** → `weigh-in` → **Enable auto-update**.
   Claude Code leaves it off for every marketplace that is not an official one or added from
   claude.ai, so without this step releases arrive only when you update by hand.
4. Start a new session. Type `/weigh-in` (Claude Code also lists it as `/weigh:weigh-in`). In a
   client repository, run `/weigh-in init client` first, which keeps the board local.

Hooks need no shell: Claude Code starts `libexec/weighin` directly (exec form), which on Windows
resolves to `libexec/weighin.exe`, so capture works with or without Git for Windows (B-16). The
sitting is different: `/weigh-in` runs `weighin` through `sh`, so on Windows it needs
[Git for Windows](https://git-scm.com/downloads/win) (Git Bash). Without it, Claude Code uses its
PowerShell tool and the sitting's commands fail with `sh` not found; if Git is installed but not
found, set `CLAUDE_CODE_GIT_BASH_PATH` (Claude Code's
[setup guide](https://code.claude.com/docs/en/setup#set-up-on-windows)).

### Update

With auto-update on (step 3), Claude Code checks the marketplace in the background and re-clones
it when it has new commits, so a release reaches you without a step of your own. To update now,
or with auto-update off:

```
/plugin marketplace update weigh-in
```

Keep one install. A local build (`/plugin marketplace add <path to a checkout>`) or a plugin
uploaded to your claude.ai account beside this one runs a second copy, and two versions should
not share a ledger (B-05). If `/weigh-in` shows twice, remove one.

### Uninstall

```
/plugin uninstall weigh-mod@weigh-in
/plugin uninstall weigh@weigh-in
/plugin marketplace remove weigh-in
```

The plugin's files are under `~/.claude/plugins/cache/weigh-in/`. Uninstalling leaves every
ledger and every `.kb/` atom where it is; delete a ledger's folder (below) yourself if you want
it gone.

## The band, the board pane and edit toasts (Claude Code)

The `weigh-mod` plugin is a mod: a hooks module (`hooks/mod/register.tsx`) that shows this
repository's ledger above the prompt, `Weigh-In · 3 open here · 1 one-way · 2 defaults`, with a
**Sit** button that runs `/weigh-in` and a **Board** button that opens the board pane. The band is hidden when nothing is open and no default
waits for review, and where the row is too narrow it moves to the status line under the prompt.
It refreshes when the session starts and once after each turn, from `weighin open --repo --json`
and `weighin digest --json`. It also adds `/wi`, answered without a Claude turn:

    /wi open                                       open items in this repository
    /wi board                                      the board pane (below)
    /wi stats                                      the ledger's health numbers
    /wi decide <id> <option> [note]                also <id>: <option> (note: …), as the board copies it
    /wi defer <id> — trigger: …; resolve by: …; default: <X> (note: …)

`decide` and `defer` take the same arguments as the board's "Copy as prompt" strings, with
`/wi` in place of `/weigh-in`, and print what `weighin` printed. When the plugin has no binary
for your platform, the band and `/wi` say "capture is off", as SessionStart does (B-11).

**The board pane** (`/wi board`, or **Board** on the band) is the local board (D-0017): it runs
on your machine and never publishes. It reads `weighin board export --stdout` and has three tabs.
*Needs you* holds the cards a sitting has grounded, by rank, each in full: the question, why it
matters, the door and its note, the blast radius, resolve-by, the evidence as `path:line` text you
can select, the excerpt, the options with what each holds if and costs if wrong, and the
recommendation. A one-way door lists its options first and pre-selects none, not even the
default of a deferral. Type an optional note,
then press an option to decide, **Defer…** for a trigger, a resolve-by and a default, or
**Dismiss…** with a reason. Items no sitting has grounded are listed read-only: settle them in a
sitting. *Decided for you* lists the defaults taken without you, each with **Reopen** and **Ack**,
and *Settled* the decisions already made. Each press writes one answer through `weighin board
apply --surface pane` (an acknowledgement through `weighin batch`), shows what `weighin` printed,
refused answers included, and reads the board again; a press while an answer is still being
written is ignored, and a write that timed out says it may have been applied. Under board mode `off` (or with no
`.kb/_ledger/config.json`) the pane shows `weighin`'s one line and nothing else. Where the pane
cannot be placed, `/wi board` answers with the board as text, and **Board** writes it to the
transcript; a pane left waiting undrawn is not read again until it is placed.

**Edit toasts.** After a `Write`, `Edit` or `MultiEdit` that succeeded, the mod runs `weighin
decisions --path <file> --json` once and, when decisions stand at that file, shows at most three
of them in a toast and on a second band line, newest first, in the line format the model reads.
The model's come from the `PostToolUse` hook, which ranks them by the line the edit touched, so
the two may pick different three when more stand at a file. An edit that lists none clears the
band line. If `weighin` cannot list them, the transcript says so once a session. It never changes
the tool call or its result, and runs nothing after a failed or refused one.

It needs Claude Code 2.1.287 or later in a terminal, or the desktop app 2.1.286 or later. Older
versions ignore the module and run the hooks and the skill as before. It draws only in a terminal
and in the desktop app: in the VS Code panel, cloud sessions, Cowork and `claude -p` it registers
nothing and runs nothing.

To turn it off and keep the rest of the plugin, set `WEIGHIN_MOD=0` in the environment Claude
Code starts with, or in the `env` block of `~/.claude/settings.json`:

    { "env": { "WEIGHIN_MOD": "0" } }

`disableAllHooks` and `--safe-mode` turn it off too, with every other hook.

## Cowork

Cowork takes plugins from your claude.ai account. From 1.1.0 its skill downloads the Linux
binary of its own release on first use in a chat and checks its SHA-256 (B-33), so it needs no
connected checkout. 1.0.0 and earlier were packaged while the repository was internal and cannot
download, so they use a binary in a connected folder, as does any release whose download fails
(D-0012):

1. Keep a checkout of the `marketplace` branch:

   ```sh
   git clone --branch marketplace --single-branch https://github.com/Wet-Ink-Corporation/weigh-in.git weigh-in-marketplace
   ```

   After each release:

   ```sh
   git -C weigh-in-marketplace fetch origin marketplace
   git -C weigh-in-marketplace reset --hard origin/marketplace
   ```

   The branch is replaced each release, so `git pull` would refuse to merge the unrelated
   commit; the reset takes it. Keep nothing of your own in this checkout.
2. Connect that folder to Cowork once. The skill finds
   `plugin/libexec/x86_64-unknown-linux-musl/weighin` in it.
3. With the organization sync (above), the plugin is already there. Without it, upload
   `weigh-in-plugin-<version>.zip` from the
   [release](https://github.com/Wet-Ink-Corporation/weigh-in/releases) under Customize >
   Plugins, and again for each release.

The plugin you upload also appears in Claude Code. If you installed from the marketplace branch
too, you will see `/weigh-in` twice; remove one.

To uninstall, remove the plugin under Customize > Plugins and disconnect the folder.

## Where the ledger is

First match wins:

1. `--store <file>` on the command line.
2. `$WEIGHIN_STORE`.
3. `store` in `.kb/_ledger/config.json`, set with `weighin init --ledger <path>`; a relative path
   is taken from the repository root.
4. In a git repository: `<git common dir>/weighin/ledger.sqlite3`, usually
   `.git/weighin/ledger.sqlite3`. All worktrees share it, and it is never committed.
5. Outside git (a Cowork folder): `<folder>/.weighin/ledger.sqlite3`.

A ledger is created by the first item raised; a session that raises nothing leaves none behind.
Decisions are also written as atoms under `.kb/`, which you commit; the ledger holds the
history and the open items. If a synced folder (OneDrive, Dropbox) is used from more than one
computer, point `store` outside the synced tree.

## `hook-errors.log`

Hooks never fail a session. When one fails, it appends a line to `hook-errors.log` in the
ledger's folder (step 4 above: `.git/weighin/hook-errors.log`) and exits quietly:

```
<time, RFC 3339 UTC> <hook>: <error>: <cause>: …
```

The hook is `SessionStart`, `Stop`, `SubagentStop` or `PostToolUse`; the error is followed by
each of its causes, outermost first. The next session start reports the lines added since the last one,
once:

> Weigh-In: 2 hook error(s) since the last session, so capture may be off. Last: … (full log: …)

Markers raised while a hook was failing were not captured; `/weigh-in` sweeps the conversation
and catches what is still in it. `hook-errors.seen` beside the log only records how far it has
been reported. The log is never cleared by Weigh-In; truncate it yourself when you have read it.

## "Capture is off"

> Weigh-In: capture is off: this plugin has no weighin binary for Linux-aarch64; build one with
> scripts/build-plugin-bins.sh or install a release that ships it.

The launcher found no binary for this computer, so no hook records anything. In order:

1. "This copy has no binaries"? The plugin is the development tree: installed from `main`, a
   local checkout without a build, or a claude.ai organization sync of the `weigh-in`
   repository, whose default branch is `main` (B-18). Install from the `marketplace` branch
   (above), or let the organization sync `weigh-in-plugin` instead, which holds the releases
   (B-19); `main` holds no binaries.
2. A platform the release does not ship (Intel macOS, Linux on ARM, Windows on ARM)? Build one:
   `cargo install --path crates/weighin` from a checkout puts `weighin` on `PATH`, which the
   launcher falls back to.
3. Binary present but not running? The launcher sets the executable bit when it is missing;
   check the file under `~/.claude/plugins/cache/weigh-in/weigh/<version>/libexec/<target>/`
   and that nothing (antivirus, a `noexec` mount) blocks it.

In Cowork this message never appears: hooks there record nothing, and the sitting sweeps the
conversation instead (B-04).

## Windows: `ENOENT … plugin/libexec/weighin`

> Stop hook error: … ENOENT: no such file or directory, uv_spawn 'D:\repos\weigh-in\plugin/libexec/weighin'

On Windows, Claude Code starts `libexec/weighin` with no shell, and that path resolves to
`libexec/weighin.exe` (B-16). When that file is missing, every hook fails this way instead of
saying capture is off. Released packages ship it. A local checkout gets it only from the build
scripts of 0.5.0 or later, so a checkout built before 0.5.0 has the per-target build but not this
copy. Check with `dir <checkout>\plugin\libexec`. Rebuild in the checkout (not the repository
you were working in), then start a new session:

```powershell
cd <checkout>
pwsh scripts/build-plugin-bins.ps1
```

Or install from the `marketplace` branch (above), which ships `weighin.exe`, and remove the local
install.

A copy synced by claude.ai (its path has `…/rpm/plugin_…/` in it) with only `weighin` and
`weighin.cmd` in `libexec` came from the `weigh-in` repository's `main`, not from a release
(B-18): there `/weigh-in` says "this copy has no binaries", and every hook fails this way. Only
`weigh-in-plugin` should be synced; a copy from it has `libexec/weighin.exe` and
`libexec/x86_64-pc-windows-msvc/weighin.exe`.
