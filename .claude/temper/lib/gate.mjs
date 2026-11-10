// The gate: fmt (auto-fix), the escape-hatch scan of changed files, the
// member-manifest check, clippy -D warnings, tests and rustdoc -D warnings.
// When .claude/temper-baseline.json exists, clippy and rustdoc ratchet against
// it instead (baseline.mjs): only new code and count increases fail.
// Results are cached per tree fingerprint so repeated Stop/SubagentStop checks
// cost nothing.
import fs from 'node:fs';
import path from 'node:path';
import {
  run, readText, writeAtomic, fingerprint, gateProgressPath, pidAlive, rustChanges, changedFiles, showAt, crateOf, relTo, globMatch, norm,
} from './project.mjs';
import { scanSource, newHits, classifyPath, formatHit, isBuildScript, NEW_CODE_ONLY } from './scan.mjs';
import { parseToml } from './toml.mjs';
import { memberProblems, membersFromToml, ownLintsMember, splitProblems } from './manifest.mjs';
import { collectFindings, mergeCollections, isComplete, writeCachedFindings, ruleMap } from './findings.mjs';
import { readBaseline, baselineExists, ratchet, renderRatchet, debtLine, changedLines, renamedSince } from './baseline.mjs';

const has = (cmd, args) => run(cmd, args, { timeoutMs: 20000 }).code === 0;

export function defaultSteps(cfg) {
  const feat = cfg.gate.allFeatures ? ['--all-features'] : [];
  const steps = [
    { name: 'clippy', cmd: ['cargo', 'clippy', '--workspace', '--all-targets', ...feat, '--message-format=short', '--', '-D', 'warnings'] },
  ];
  if (has('cargo', ['nextest', '--version'])) {
    steps.push({ name: 'test', cmd: ['cargo', 'nextest', 'run', '--workspace', ...feat, '--no-fail-fast', '--no-tests=pass'] });
    steps.push({ name: 'doctest', cmd: ['cargo', 'test', '--doc', '--workspace', ...feat] });
  } else {
    steps.push({ name: 'test', cmd: ['cargo', 'test', '--workspace', ...feat, '--no-fail-fast'] });
  }
  if (cfg.gate.doc) {
    steps.push({ name: 'doc', cmd: ['cargo', 'doc', '--workspace', '--no-deps', ...feat], env: { RUSTDOCFLAGS: '-D warnings' } });
  }
  return steps;
}

/**
 * Steps when `.claude/temper-baseline.json` exists: clippy and rustdoc run in
 * ratchet mode (collect every finding, fail only on new code and increases);
 * tests and doctests are unchanged. Still one clippy run. The doc step ratchets
 * everything collected by then, the rustdoc findings included (the same
 * comparison CI's `baseline --check` makes).
 */
export function ratchetSteps(cfg) {
  const steps = defaultSteps(cfg).map((s) => {
    if (s.name === 'clippy') {
      const sources = cfg.gate.scan === false ? ['clippy'] : ['clippy', 'scan'];
      return { name: 'clippy', ratchet: sources, scope: sources, cmd: ['temper', 'baseline', '--check', `(${sources.join(', ')})`] };
    }
    if (s.name === 'doc') return { name: 'doc', ratchet: ['rustdoc'], scope: null, cmd: ['temper', 'baseline', '--check', '(rustdoc)'] };
    return s;
  });
  const last = steps.filter((s) => s.ratchet).at(-1);
  if (last) last.final = true;
  return steps;
}

function summarize(name, out) {
  const lines = out.split(/\r?\n/);
  let keep;
  if (name === 'clippy' || name === 'doc') {
    keep = lines.filter((l) => /(^|: )(error|warning)(\[|:)/.test(l) && !/could not compile|build failed|generated \d+ warning|aborting due to/.test(l));
  } else {
    keep = lines.filter((l) => /FAILED|panicked at|^---- .* ----$|^failures:|assertion|^\s+left:|^\s+right:|error\[E\d+\]|^error:|test result: FAILED|FAIL \[/.test(l));
  }
  if (!keep.length) keep = lines.filter((l) => l.trim()).slice(-15);
  const capped = keep.slice(0, 30);
  if (keep.length > capped.length) capped.push(`... ${keep.length - capped.length} more lines in the log`);
  return capped.join('\n');
}

/**
 * fmt's failure summary. When cargo cannot load the workspace (a `members` glob that matches no
 * crate, a manifest that does not parse), `cargo fmt` fails before formatting anything and its
 * output ends in rustfmt's help text, so the generic tail says nothing useful. Text only: the
 * step fails either way.
 */
export function summarizeFmt(out, { check = false } = {}) {
  if (!/`cargo metadata` exited with an error|failed to load manifest for workspace member/.test(out)) {
    return check ? 'needs formatting: the implementer runs `cargo fmt --all` (or the gate without --check)' : summarize('fmt', out);
  }
  const ws = out.match(/workspace at `(.+?)[\\/]Cargo\.toml`/)?.[1];
  // Only workspace paths are shortened and given forward slashes; other backticked tokens stay as cargo wrote them.
  const rel = (p) => (ws && p.startsWith(ws) && p.length > ws.length ? p.slice(ws.length + 1).replaceAll('\\', '/') : p);
  const keep = out.split(/\r?\n/)
    .map((l) => l.replace(/^`cargo metadata` exited with an error: /, ''))
    .filter((l) => /^error: |^\s*--> /.test(l))
    .map((l) => l.trim().replace(/`([^`]+)`/g, (_, p) => `\`${rel(p)}\``));
  const lines = ['cargo cannot load the workspace, so nothing was formatted:', ...keep.slice(0, 10)];
  const member = out.match(/failed to load manifest for workspace member `([^`]+)`/)?.[1];
  if (member && /[*?]/.test(member)) {
    const glob = rel(member);
    lines.push(`no crate matches the workspace member \`${glob}\`: create one with \`cargo new --lib ${glob.replace(/[^/]*[*?][^/]*/g, '<name>')}\`, or change \`members\` in Cargo.toml`);
  }
  return lines.join('\n');
}

/** Scan changed .rs files against their base-revision content: new hard hits fail the gate. */
export function scanChanged(proj, base) {
  const out = { hard: [], ask: [], soft: [] };
  const comparable = Boolean(proj.gitRoot && base);
  const files = rustChanges(proj, base).filter((f) => f.endsWith('.rs'));
  // A moved or renamed file (staged or not) is compared with its content at the old path.
  const renames = files.length ? renamedSince(proj, base) : new Map();
  for (const rel of files) {
    const abs = path.join(proj.gitRoot ?? proj.root, rel);
    const after = readText(abs);
    if (after === null) continue;
    const krate = crateOf(abs);
    const crateRel = krate ? relTo(krate.dir, abs) : rel;
    const kind = krate ? classifyPath(crateRel, krate.hasLib) : 'prod';
    const build = isBuildScript(crateRel);
    const before = showAt(proj, base, renames.get(norm(rel)) ?? rel) ?? '';
    const exempt = (h) => (proj.config.scanner.exempt ?? []).some((e) => e.rule === h.id && (e.paths ?? []).some((p) => globMatch(p, rel)));
    const fresh = newHits(scanSource(before, { kind, build }).hits, scanSource(after, { kind, build }).hits).filter((h) => !exempt(h));
    for (const h of fresh) {
      // Without git or a base, nothing tells new code from old: a new-code-only rule (decisions.md 3.2) only advises.
      const sev = !comparable && NEW_CODE_ONLY.has(h.key) && h.severity === 'hard' ? 'soft' : h.severity;
      out[sev]?.push({ ...h, severity: sev, file: rel });
    }
  }
  return out;
}

const ADVISORY_MAX = 12;

/**
 * Advisories (soft scan hits) under MUST rules, as `ID file:line message`, at
 * most ADVISORY_MAX and a count of the rest. Several MUST rules have only an
 * advisory scan behind them (ASY-04, ASY-07, ARC-09, ARC-11, ...), so the gate
 * lists the ones a change adds for the reviewer to judge; they never fail it.
 */
export function mustAdvisories(soft, map = ruleMap()) {
  const must = soft.filter((h) => map.rules[h.id]?.level === 'MUST').map((h) => `${h.id} ${h.file}:${h.line} ${h.msg}`);
  if (must.length <= ADVISORY_MAX) return must;
  return [...must.slice(0, ADVISORY_MAX), `... and ${must.length - ADVISORY_MAX} more (temper scan lists them all)`];
}

/**
 * Member manifests against their base-revision text (MOD-09, ARC-01, PERF-09,
 * TST-09), and each member's charter (ARC-08).
 * Only problems a manifest did not have at the base fail (the ratchet), so a
 * legacy member does not turn the gate red; a new, untracked or changed member
 * (or a changed root manifest) is checked in full. Without a base, every
 * problem counts, except the checks added in 1.x (mocking crates, charters),
 * which then only advise.
 * @returns {{ ok: boolean, summary: string }}
 */
export function checkManifestPaths(proj, base, rootPath, manifestPaths) {
  const rootNow = parseToml(readText(rootPath));
  const changed = proj.gitRoot && base ? new Set(changedFiles(proj, base).map(norm)) : null;
  const rootRel = relTo(proj.root, rootPath);
  const rootBaseText = changed ? showAt(proj, base, rootRel) : null;
  const rootBase = rootBaseText === null ? null : parseToml(rootBaseText);
  const fresh = [];
  const legacy = [];
  // A manifest at its base path, or at the path it was moved from (a crate moved or renamed with its directory).
  let renames = null;
  const atBase = (rel) => {
    const text = showAt(proj, base, rel);
    if (text !== null) return { rel, text };
    renames ??= renamedSince(proj, base);
    const old = renames.get(norm(rel));
    const was = old ? showAt(proj, base, old) : null;
    return was === null ? null : { rel: old, text: was };
  };
  for (const mp of manifestPaths) {
    const rel = relTo(proj.root, mp);
    const now = parseToml(readText(mp));
    if (!now || !rootNow) { fresh.push(`${rel}: not readable as TOML by temper`); continue; }
    const self = path.resolve(mp) === path.resolve(rootPath);
    const opts = { ownLints: ownLintsMember(proj.config, proj.root, mp) };
    const after = memberProblems(now, { path: rootPath, manifest: rootNow, self }, opts);
    let before = [];
    if (changed && !changed.has(rel) && !changed.has(rootRel)) before = after; // untouched since the base
    else if (changed && rootBase) {
      const prior = atBase(rel);
      const was = prior ? parseToml(prior.text) : null;
      if (was) before = memberProblems(was, { path: rootPath, manifest: rootBase, self }, opts);
    }
    const split = splitProblems(before, after);
    // A check added in 1.x (`newOnly`) has nothing to compare with when there is no base: advise only.
    if (!changed) {
      split.legacy.push(...split.fresh.filter((p) => p.newOnly));
      split.fresh = split.fresh.filter((p) => !p.newOnly);
    }
    const c = charterProblem(proj, base, mp, changed && atBase);
    if (c) split[c.fresh ? 'fresh' : 'legacy'].push(c);
    for (const p of split.fresh) fresh.push(`${p.id} ${rel}: ${p.msg}.\n    fix: ${p.fix}`);
    for (const p of split.legacy) legacy.push(`${p.id} ${rel}`);
  }
  const old = legacy.length ? `${legacy.length} older problem(s) not introduced since the base (advisory): ${legacy.join(', ')}` : '';
  return {
    ok: fresh.length === 0,
    summary: fresh.length
      ? fresh.join('\n') + (old ? `\n(${old})` : '')
      : `${manifestPaths.length} member manifest(s): no new lint-inheritance, profile, mocking-crate or charter problems${old ? `; ${old}` : ''}`,
  };
}

/**
 * ARC-08: a member without `AGENTS.md` beside its manifest. It fails only when
 * the crate is new since the base, or its charter existed at the base; a crate
 * that already lacked one (also after a move), or any crate when there is no
 * base (`atBase` null), is advisory.
 * @returns {{ id: string, msg: string, fix: string, fresh: boolean } | null}
 */
function charterProblem(proj, base, manifestPath, atBase) {
  const dir = path.dirname(path.resolve(manifestPath));
  if (fs.existsSync(path.join(dir, 'AGENTS.md'))) return null;
  const was = atBase ? atBase(relTo(proj.root, manifestPath)) : null;
  const legacy = !atBase || (was !== null && showAt(proj, base, path.posix.join(path.posix.dirname(was.rel), 'AGENTS.md')) === null);
  return {
    id: 'ARC-08', fresh: !legacy,
    msg: 'the crate has no charter (`AGENTS.md` beside its Cargo.toml)',
    fix: 'add `AGENTS.md` (purpose, layer, allowed dependencies, public surface, invariants, error types, test strategy, do-not list, decisions log: the template in architecture.md, ARC-08) and a `CLAUDE.md` containing `@AGENTS.md`. Filling in its layer and allowed dependencies is part of the crate-boundary decision (H-16, ARC-07).',
  };
}

/** The manifests step without cargo: members from the root's `members` globs. */
export function checkManifestsToml(proj, base) {
  const members = membersFromToml(proj.root);
  if (members === null) return { ok: true, checked: false, summary: `not checked: no readable Cargo.toml at ${proj.root}` };
  return { ...checkManifestPaths(proj, base, path.join(proj.root, 'Cargo.toml'), members), checked: true };
}

/**
 * The manifests step: members from `cargo metadata --no-deps` (fast, no
 * resolution), falling back to the TOML walk when cargo cannot be run.
 * Stable Cargo warns about neither problem, so a new crate without
 * `[lints] workspace = true` would compile with every escape hatch allowed.
 * @returns {{ ok: boolean, summary: string }}
 */
export function checkManifests(proj, base) {
  const r = run('cargo', ['metadata', '--no-deps', '--format-version', '1'], { cwd: proj.root, env: { CARGO_TERM_COLOR: 'never' }, timeoutMs: 120000 });
  if (r.error) return checkManifestsToml(proj, base);
  if (r.code !== 0) {
    const why = r.stderr.trim().split(/\r?\n/).filter(Boolean).slice(0, 6).join('\n');
    return { ok: false, summary: `cargo metadata failed:\n${why}` };
  }
  let meta;
  try { meta = JSON.parse(r.stdout); } catch { return { ok: false, summary: 'cargo metadata printed output that is not JSON' }; }
  const members = new Set(meta.workspace_members ?? []);
  const paths = (meta.packages ?? []).filter((p) => !members.size || members.has(p.id)).map((p) => p.manifest_path);
  return checkManifestPaths(proj, base, path.join(meta.workspace_root, 'Cargo.toml'), paths);
}

/**
 * Run the gate. Returns { ok, fp, cached, steps: [{name, ok, ms, summary, log}] }.
 * @param {{ base?: string, noCache?: boolean, quiet?: boolean }} opts
 */
export function runGate(proj, opts = {}) {
  const cwd = proj.root;
  const cfg = proj.config;
  const steps = [];
  const logs = path.join(proj.stateDir, 'logs');
  fs.mkdirSync(logs, { recursive: true });
  const env = { CARGO_TERM_COLOR: 'never' };
  // The gate's start, for the progress file's elapsed time: the toolchain probe and fmt count too.
  const began = Date.now();

  // 0. Is the toolchain usable here at all? An unusable toolchain is an
  // environment problem, not a red gate: report it instead of blocking.
  const probe = run('cargo', ['--version'], { cwd, env, timeoutMs: 60000 });
  if (probe.code !== 0) {
    const why = (probe.error?.message ?? `${probe.stderr}${probe.stdout}`).trim().split(/\r?\n/).slice(0, 3).join(' ');
    const toolchain = { name: 'toolchain', ok: false, ms: 0, summary: `cargo is not usable in this environment: ${why}` };
    // The manifest check needs only TOML, so it still runs; a new violation is a red gate.
    const t = Date.now();
    const m = checkManifestsToml(proj, opts.base);
    const manifests = { name: 'manifests', ok: m.ok, ms: Date.now() - t, checked: m.checked, summary: `${m.summary} (read without cargo)` };
    const at = new Date().toISOString();
    if (!m.ok) return { ok: false, unavailable: false, fp: null, at, cached: false, steps: [toolchain, manifests] };
    return { ok: null, unavailable: true, fp: null, at, cached: false, steps: [toolchain, manifests] };
  }

  // 1. fmt is mechanical: fix, don't argue. In check mode (reviewers) verify only.
  const t0 = Date.now();
  const fmtArgs = opts.check ? ['fmt', '--all', '--', '--check'] : ['fmt', '--all'];
  const fmt = run('cargo', fmtArgs, { cwd, env, timeoutMs: 120000 });
  const fmtSummary = summarizeFmt(fmt.stderr + fmt.stdout, { check: opts.check });
  steps.push({ name: 'fmt', ok: fmt.code === 0, ms: Date.now() - t0, summary: fmt.code === 0 ? (opts.check ? 'clean' : 'applied') : fmtSummary });
  if (fmt.code !== 0) return finish(proj, null, steps, false);

  // 2. cache by fingerprint (after formatting)
  const fp = fingerprint(proj, opts.base);
  const cachePath = path.join(proj.stateDir, 'gate', `${fp}.json`);
  if (!opts.noCache) {
    const cached = readText(cachePath);
    if (cached) {
      try { return { ...JSON.parse(cached), cached: true }; } catch { /* rerun */ }
    }
  }

  // While it runs, the gate keeps a progress file for this fingerprint (gate-progress-<fp>.json)
  // so readers such as `status --json` can show the step it is on; it goes when the gate ends.
  // It is written from here, after formatting, because the fingerprint is known only then; its
  // start is the gate's own, so the elapsed time covers the toolchain probe and fmt too.
  const prog = progress(proj, fp, steps, began);
  try {
    // 3. escape-hatch scan of changed files (cheap; catches code written around the edit hooks)
    if (cfg.gate.scan) {
      prog.start('scan');
      const t = Date.now();
      const s = scanChanged(proj, opts.base);
      const ok = s.hard.length === 0;
      steps.push({
        name: 'scan', ok, ms: Date.now() - t,
        summary: ok ? `${s.soft.length} advisory, ${s.ask.length} exception request(s)` : s.hard.slice(0, 12).map((h) => formatHit(h, h.file)).join('\n'),
        exceptions: s.ask.map((h) => `${h.file}:${h.line} ${h.exception ?? h.id}`),
        // Without git or a base every hit looks new, so only the count is given.
        advisories: proj.gitRoot && opts.base ? mustAdvisories(s.soft) : [],
      });
      if (!ok) return finish(proj, fp, steps, true, cachePath);
    }

    // 4. member manifests (cargo metadata --no-deps: fast, no resolution), before clippy
    {
      prog.start('manifests');
      const t = Date.now();
      const m = checkManifests(proj, opts.base);
      steps.push({ name: 'manifests', ok: m.ok, ms: Date.now() - t, summary: m.summary });
      if (!m.ok) return finish(proj, fp, steps, true, cachePath);
    }

    // 5. cargo steps, fail fast. With a baseline, clippy and rustdoc ratchet
    // against it; a gate.steps override in temper.json always wins. Choosing the
    // default steps probes for nextest: that runs between steps, under none of them.
    prog.between();
    const ratcheting = !cfg.gate.steps && baselineExists(proj.root);
    const rs = { collection: null, baseline: null, changed: undefined };
    for (const step of cfg.gate.steps ?? (ratcheting ? ratchetSteps(cfg) : defaultSteps(cfg))) {
      prog.start(step.name);
      if (step.ratchet) {
        const s = ratchetStep(proj, step, rs, { base: opts.base, prefix: path.join(logs, fp), fp });
        steps.push(s);
        if (!s.ok) break;
        continue;
      }
      const t = Date.now();
      const [cmd, ...args] = step.cmd;
      const r = run(cmd, args, { cwd, env: { ...env, ...(step.env ?? {}) }, timeoutMs: cfg.gate.stepTimeoutSec * 1000 });
      const out = `${r.stdout}\n${r.stderr}`;
      const log = path.join(logs, `${fp}-${step.name}.log`);
      try { fs.writeFileSync(log, `$ ${step.cmd.join(' ')}\n${out}`); } catch { /* ignore */ }
      const ok = r.code === 0;
      steps.push({ name: step.name, ok, ms: Date.now() - t, summary: ok ? tail(step.name, out) : (r.error ? `could not run: ${r.error.message}` : summarize(step.name, out)), log });
      if (!ok) break;
    }
    return finish(proj, fp, steps, true, cachePath);
  } finally {
    prog.done();
  }
}

/**
 * One ratchet step: collect this step's sources, merge them into the gate's
 * collection, and ratchet the step's lints against the baseline. The complete
 * collection is cached for `temper report` and the reviewer.
 */
function ratchetStep(proj, step, rs, { base, prefix, fp }) {
  const t = Date.now();
  if (!rs.baseline) {
    try { rs.baseline = readBaseline(proj.root); } catch (e) {
      return { name: step.name, ok: false, ms: Date.now() - t, ratchet: true, summary: `${e.message}. Restore it from git; changing it is a human decision (H-12).` };
    }
  }
  const part = collectFindings(proj, { cfg: proj.config, sources: step.ratchet, logPrefix: prefix, reuse: rs.collection });
  rs.collection = rs.collection ? mergeCollections(rs.collection, part) : part;
  if (rs.changed === undefined) {
    rs.notes = new Set();
    try { rs.changed = changedLines(proj, base); } catch (e) {
      // An implicit base that is not a commit here (a TEMPER_BASE from another repository): diff against HEAD.
      try { rs.changed = changedLines(proj, 'HEAD'); rs.first = `${e.message}; compared with HEAD instead`; } catch { rs.changed = null; rs.first = `${e.message}; only the totals were compared`; }
    }
  }
  const res = ratchet(rs.collection, rs.baseline, { proj, changed: rs.changed, base, sources: step.scope ?? undefined, checkRaised: false });
  // The last ratchet step also compares the baseline with the one at the session base (a flag, like --check).
  const all = ratchet(rs.collection, rs.baseline, { proj, changed: rs.changed, base, checkRaised: Boolean(step.final) });
  const raised = all.raised.length
    ? [`BASELINE RAISED since ${(all.base ?? '').slice(0, 12)}: ${all.raised.length} count(s) higher in .claude/temper-baseline.json (accepted debt grew; a human decision, H-12):`,
      ...all.raised.slice(0, 30).map((x) => `  ${x.target ? `[${x.target}] ` : ''}${x.crate} ${x.lint}: ${x.from} -> ${x.to}`)]
    : [];
  const notes = [rs.first, ...res.notes, res.mode === 'totals' ? 'no base revision: only the totals were compared' : null]
    .filter((n) => n && !rs.notes.has(n));
  for (const n of notes) rs.notes.add(n);
  res.notes = notes;
  if (isComplete(rs.collection, proj.config)) writeCachedFindings(proj, fp, { ...rs.collection, ratchet: all }, { base });
  const log = Object.values(part.logs ?? {})[0];
  return {
    name: step.name, ok: res.ok, ms: Date.now() - t, ratchet: true,
    summary: [res.ok ? [`no new findings and no counts above the baseline (${step.scope ? step.scope.join(', ') : 'all sources'})`, ...notes.map((n) => `note: ${n}`)].join('\n') : renderRatchet(res), ...raised].join('\n'),
    ...(all.raised.length ? { raised: all.raised } : {}),
    debt: debtLine(all),
    counts: { new_code: res.new_code.length, increases: res.increases.length, fixed: all.fixed_count, legacy: all.legacy_total, baseline: all.baseline_total, errors: res.errors.length },
    ...(log ? { log } : {}),
  };
}

const STALE_MARGIN_SEC = 300;

/**
 * The progress file of a gate running for `fp`: { fp, pid, startedAt, steps: [{ name, ok, ms }],
 * current: { name, startedAt } | null }. `startedAt` is the gate's start (`began`, ms), before its
 * toolchain probe and fmt. Each write replaces it whole (writeAtomic), from the
 * gate's own `steps` list; any failure to write is ignored, since progress is only a view.
 */
function progress(proj, fp, steps, began) {
  const file = gateProgressPath(proj, fp);
  const startedAt = new Date(began).toISOString();
  let current = null;
  const write = () => {
    try {
      writeAtomic(file, JSON.stringify({ fp, pid: process.pid, startedAt, steps: steps.map((s) => ({ name: s.name, ok: s.ok, ms: s.ms })), current }));
    } catch { /* progress is best effort */ }
  };
  write();
  return {
    start(name) { current = { name, startedAt: new Date().toISOString() }; write(); },
    // The last step is done and the next not started: the steps done, with no step current.
    between() { current = null; write(); },
    // Another gate on the same fingerprint (a second worktree at the same tree, `gate --no-cache`
    // beside Stop) may have written the file since: leave it to that gate.
    done() {
      try { if (JSON.parse(fs.readFileSync(file, 'utf8')).pid === process.pid) fs.rmSync(file, { force: true }); } catch { /* gone, or not ours */ }
    },
  };
}

/**
 * The progress of a gate running now for `fp`, or null. A file whose process is gone, or whose
 * current step (or start, between steps) is older than the step timeout plus a margin, is stale: a hook killed
 * mid-gate leaves it behind, and it says nothing about the tree.
 */
export function gateProgress(proj, fp, now = Date.now()) {
  const t = readText(gateProgressPath(proj, fp));
  if (!t) return null;
  let p;
  try { p = JSON.parse(t); } catch { return null; }
  if (!p || p.fp !== fp || !pidAlive(p.pid)) return null;
  const since = Date.parse(p.current?.startedAt ?? p.startedAt);
  // A ratchet step runs `cargo metadata` before clippy, so allow a margin over the step timeout;
  // the pid is the main signal, the age only guards against a reused pid.
  if (!Number.isFinite(since) || now - since > (proj.config.gate.stepTimeoutSec + STALE_MARGIN_SEC) * 1000) return null;
  return { startedAt: p.startedAt, steps: Array.isArray(p.steps) ? p.steps : [], current: p.current ?? null };
}

function tail(name, out) {
  if (name === 'test' || name === 'doctest') {
    const res = out.split(/\r?\n/).filter((l) => /test result:|Summary \[|tests? run:/.test(l));
    return res.slice(-3).join(' | ') || 'passed';
  }
  return 'clean';
}

function finish(proj, fp, steps, cacheable, cachePath) {
  const ok = steps.every((s) => s.ok);
  const result = { ok, fp, at: new Date().toISOString(), steps };
  // Cache only if the tree did not change while we were running.
  if (cacheable && fp && cachePath) {
    try { writeAtomic(cachePath, JSON.stringify(result, null, 2)); } catch { /* ignore */ }
  }
  return { ...result, cached: false };
}

export function cachedGate(proj, fp) {
  const t = readText(path.join(proj.stateDir, 'gate', `${fp}.json`));
  if (!t) return null;
  try { return JSON.parse(t); } catch { return null; }
}

export function renderGate(res) {
  if (res.unavailable) {
    const m = res.steps.find((s) => s.name === 'manifests');
    const only = m?.checked ? `Only the member manifests were checked (without cargo): ${m.summary}. Nothing else was checked` : 'Nothing was checked, not even the member manifests';
    return `temper gate: UNAVAILABLE (${res.steps[0].summary}). ${only}; say so in "Not verified".`;
  }
  const head = res.steps.map((s) => `${s.name} ${s.ok ? 'ok' : 'FAILED'}`).join(' | ');
  const fails = res.steps.filter((s) => !s.ok).map((s) => `--- ${s.name} ---\n${s.summary}${s.log ? `\n(full log: ${s.log})` : ''}`).join('\n');
  const passes = res.steps.filter((s) => s.ok && (s.name.includes('test') || s.ratchet)).map((s) => `${s.name}: ${s.summary}`).join('\n');
  const exc = res.steps.flatMap((s) => s.exceptions ?? []);
  const adv = res.steps.flatMap((s) => s.advisories ?? []);
  const debt = res.steps.filter((s) => s.debt).at(-1)?.debt;
  return [
    `temper gate: ${res.ok ? 'GREEN' : 'RED'} (fingerprint ${res.fp ?? 'n/a'}${res.cached ? ', cached' : ''})`,
    head, fails, passes,
    exc.length ? `lint exceptions in the diff (need human approval, H-05): ${exc.join('; ')}` : '',
    adv.length ? `advisories this change adds under MUST rules (each is fixed or justified, or the reviewer reports it): ${adv.join('; ')}` : '',
    res.ok && debt ? `ratchet against the baseline: ${debt}` : '',
  ].filter(Boolean).join('\n');
}
