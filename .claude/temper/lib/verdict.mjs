// Review verdicts: parsing, validation and storage keyed by tree fingerprint.
import { createHash } from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { readText, writeAtomic } from './project.mjs';

export const DIMENSIONS = [
  'correctness', 'escape_hatches', 'ownership', 'errors', 'api', 'architecture',
  'tests', 'performance', 'concurrency', 'observability', 'craft', 'docs',
];
const SEVERITIES = ['blocker', 'major', 'minor', 'nit'];

/**
 * The dimensions every verdict must rate 0-4 rather than "n/a": they apply to
 * any Rust change, however small.
 */
export const ALWAYS_SCORED = ['correctness', 'escape_hatches', 'craft'];

/** A path with "/" separators and no leading "./". */
const normPath = (f) => String(f).trim().replace(/\\/g, '/').replace(/^(?:\.\/)+/, '');

/** An absolute path once normalised: "/x", "//?/C:/x", "//server/x" or "C:/x". */
const isAbs = (f) => f.startsWith('/') || /^[A-Za-z]:\//.test(f);

/**
 * The changed files (normalised) that a files_read entry (normalised) covers: the
 * same repo-relative path, or for an absolute entry the longest changed path its
 * trailing components equal. A bare suffix such as "lib.rs" or "src/lib.rs" for
 * crates/core/src/lib.rs covers nothing, and the absolute path of
 * crates/core/src/lib.rs does not cover a root package's src/lib.rs as well.
 */
function coveredBy(read, changed) {
  if (changed.includes(read)) return [read];
  if (!isAbs(read)) return [];
  const tails = changed.filter((f) => f !== '' && read.endsWith(`/${f}`)).sort((a, b) => b.length - a.length);
  return tails.slice(0, 1);
}

/** Extract the last fenced json block containing "temper_verdict". */
export function extractVerdict(text) {
  if (!text) return null;
  const blocks = [...text.matchAll(/```(?:json)?\s*\n([\s\S]*?)```/g)].map((m) => m[1]).filter((b) => b.includes('temper_verdict'));
  const raw = blocks.at(-1);
  if (!raw) return null;
  try { return JSON.parse(raw); } catch (e) { return { __parseError: String(e.message) }; }
}

/**
 * Validate a verdict. Returns a list of problems (empty = valid).
 * @param {object} v
 * @param {{ changedRust?: string[], gate?: {ok:boolean}|null }} ctx
 */
export const validateVerdict = (v, ctx = {}) => verdictProblems(v, ctx).map((x) => x.message);

/**
 * The problems validateVerdict reports, each as { kind, message }: `kind` is a short, stable
 * name for telemetry (the message is prose the reviewer reads and may change).
 */
export function verdictProblems(v, ctx = {}) {
  const p = [];
  const add = (kind, message) => p.push({ kind, message });
  if (!v) return [{ kind: 'no-block', message: 'no verdict block found: end the final report with a ```json block containing "temper_verdict": 1' }];
  if (v.__parseError) return [{ kind: 'json-parse', message: `verdict JSON does not parse: ${v.__parseError}` }];
  if (v.temper_verdict !== 1) add('schema-version', '"temper_verdict" must be 1');
  if (!['approved', 'changes-requested'].includes(v.verdict)) add('verdict-value', '"verdict" must be "approved" or "changes-requested"');
  if (typeof v.fingerprint !== 'string' || !/^[0-9a-f]{16}$/.test(v.fingerprint)) add('fingerprint', '"fingerprint" must be the 16-hex fingerprint given in your review ticket');
  if (!Array.isArray(v.findings)) add('findings-array', '"findings" must be an array (empty if none)');
  const findings = Array.isArray(v.findings) ? v.findings : [];
  findings.forEach((f, i) => {
    if (!f || typeof f !== 'object') { add('finding-object', `findings[${i}] must be an object`); return; }
    const rule = String(f.rule ?? '');
    if (!/^[A-Z]{1,4}-\d{2}$/.test(rule) && !(rule === 'none' && f.severity === 'nit')) add('finding-rule', `findings[${i}].rule must be a rule ID like OWN-08 ("none" is allowed only for a nit no rule covers)`);
    if (!SEVERITIES.includes(f.severity)) add('finding-severity', `findings[${i}].severity must be one of ${SEVERITIES.join('/')}`);
    if (!f.file) add('finding-file', `findings[${i}].file is required`);
    if (!f.evidence) add('finding-evidence', `findings[${i}].evidence is required (quote the code)`);
    if (!f.fix) add('finding-fix', `findings[${i}].fix is required`);
  });
  const scores = v.scores && typeof v.scores === 'object' ? v.scores : null;
  if (!scores) add('scores-missing', '"scores" object is required');
  else {
    for (const d of DIMENSIONS) {
      const s = scores[d];
      if (s === undefined) add('score-missing', `scores.${d} is missing (0-4 or "n/a")`);
      else if (s !== 'n/a' && !(Number.isInteger(s) && s >= 0 && s <= 4)) add('score-range', `scores.${d} must be 0-4 or "n/a"`);
    }
    const na = ALWAYS_SCORED.filter((d) => scores[d] === 'n/a');
    if (na.length) add('score-na', `scores must rate ${ALWAYS_SCORED.slice(0, -1).join(', ')} and ${ALWAYS_SCORED.at(-1)} 0-4: they apply to any change ("n/a" only where another dimension does not apply); "n/a": ${na.join(', ')}`);
  }
  const ev = v.evidence ?? {};
  if (!Array.isArray(ev.files_read) || ev.files_read.length === 0) add('files-read', 'evidence.files_read must list the files you actually read');
  if (!Array.isArray(ev.commands_run) || ev.commands_run.length === 0) add('commands-run', 'evidence.commands_run must list the commands you ran (at least the gate)');
  if (ctx.changedRust?.length && Array.isArray(ev.files_read)) {
    const changed = ctx.changedRust.map(normPath);
    const seen = new Set(ev.files_read.flatMap((r) => coveredBy(normPath(r), changed)));
    const missed = ctx.changedRust.filter((f) => f.endsWith('.rs') && !seen.has(normPath(f)));
    if (missed.length) add('files-unreviewed', `changed files not reviewed: ${missed.slice(0, 10).join(', ')}`);
  }
  if (v.verdict === 'approved') {
    const bad = findings.filter((f) => f.severity === 'blocker' || f.severity === 'major');
    if (bad.length) add('approved-with-findings', `"approved" is inconsistent with ${bad.length} blocker/major finding(s): use "changes-requested"`);
    if (scores) {
      const low = DIMENSIONS.filter((d) => Number.isInteger(scores[d]) && scores[d] < 3);
      if (low.length) add('approved-low-score', `"approved" requires every applicable dimension >= 3; low: ${low.join(', ')}`);
    }
    if (ctx.gate && ctx.gate.ok === false) add('approved-red-gate', '"approved" is impossible: the gate is red for this fingerprint');
    const g = v.gate ?? {};
    if (Object.values(g).some((x) => x !== 'pass' && x !== 'n/a')) add('approved-gate-entry', '"approved" requires every gate entry to be "pass"');
  }
  return p;
}

const serious = (f) => f?.severity === 'blocker' || f?.severity === 'major';

/** The dimensions `v` scores below 3. */
const lowScores = (v) => DIMENSIONS.filter((d) => Number.isInteger(v.scores?.[d]) && v.scores[d] < 3);

/** The gate entries of `v` that are neither "pass" nor "n/a". */
const failedGate = (v) => Object.entries(v.gate ?? {}).filter(([, x]) => x !== 'pass' && x !== 'n/a').map(([k]) => k);

/** The distinct problem kinds of a verdictProblems list, in order, for a telemetry row. */
export const problemKinds = (problems) => [...new Set(problems.map((x) => x.kind))];

/** The verdictProblems kinds that keep a verdict from being an approval of its tree. */
const OVERTURN_KINDS = new Set(['approved-with-findings', 'approved-low-score', 'approved-gate-entry']);

/**
 * Whether `v` gives a reason to overturn an approval of its tree: the problems it would have as an
 * approval, a blocker or major finding, a score below 3, or a gate entry that is not "pass" or "n/a".
 */
const overturns = (v) => verdictProblems({ ...v, verdict: 'approved' }).some((x) => OVERTURN_KINDS.has(x.kind));

/** `s` on one line, so a multi-line field cannot add lines to a hook's message. */
const oneLine = (s) => String(s ?? '').replace(/\s*[\r\n\u2028\u2029\u0085]+\s*/g, ' ');

/**
 * Record `v` as the verdict for its fingerprint, the one Stop reads. A
 * changes-requested verdict that gives no reason to overturn (no blocker or
 * major finding, no score below 3, no failing gate entry) does not replace a
 * non-stale approval of the same tree (#136): the approval stays, and `v` is
 * appended to verdicts/<fp>.history.jsonl, which is pruned with the verdict.
 * The returned record then carries `kept: 'approved'`.
 */
export function recordVerdict(proj, v, meta = {}) {
  const rec = { ...v, recorded_at: new Date().toISOString(), ...meta };
  const dir = path.join(proj.stateDir, 'verdicts');
  const prev = verdictFor(proj, v.fingerprint);
  if (prev?.verdict === 'approved' && !prev.stale && v.verdict === 'changes-requested' && !overturns(v)) {
    const kept = { ...rec, kept: 'approved' };
    fs.mkdirSync(dir, { recursive: true });
    fs.appendFileSync(path.join(dir, `${v.fingerprint}.history.jsonl`), `${JSON.stringify(kept)}\n`);
    return kept;
  }
  writeAtomic(path.join(dir, `${v.fingerprint}.json`), JSON.stringify(rec, null, 2));
  return rec;
}

/** The verdicts kept as history beside an approval of `fp` and recorded after `since` (an ISO time), oldest first. */
export function historyFor(proj, fp, since = '') {
  const t = readText(path.join(proj.stateDir, 'verdicts', `${fp}.history.jsonl`)) ?? '';
  return t.split('\n').flatMap((l) => { try { return l ? [JSON.parse(l)] : []; } catch { return []; } })
    .filter((h) => String(h.recorded_at ?? '') > since);
}

/** `x` with its object keys sorted, at every depth. */
const canonical = (x) => (Array.isArray(x) ? x.map(canonical)
  : x && typeof x === 'object' ? Object.fromEntries(Object.keys(x).sort().map((k) => [k, canonical(x[k])])) : x);

/** A short hash of verdict `v`'s canonical JSON: the same report read twice hashes the same. */
export const verdictHash = (v) => createHash('sha256').update(JSON.stringify(canonical(v))).digest('hex').slice(0, 16);

export function verdictFor(proj, fp) {
  const t = readText(path.join(proj.stateDir, 'verdicts', `${fp}.json`));
  if (!t) return null;
  try { return JSON.parse(t); } catch { return null; }
}

export function summarizeFindings(v, max = 10) {
  const bad = (v.findings ?? []).filter(serious);
  const rest = (v.findings ?? []).length - bad.length;
  return bad.slice(0, max).map((f) => `- [${f.severity}] ${oneLine(f.rule)} ${oneLine(f.file)}${f.line ? `:${oneLine(f.line)}` : ''}: ${oneLine(f.fix)}`).join('\n')
    + (rest > 0 ? `\n(${rest} minor/nit finding(s) in the full verdict)` : '');
}

/**
 * The reviewer's SubagentStart lines for a tree that already has verdict `v` (#136): the earlier
 * verdict, when it was recorded, its blocker and major findings, low scores and failing gate
 * entries, so a repeat review re-checks them, and an approval of the unchanged tree is overturned
 * only for one of those reasons. `kept` is the number of later verdicts kept as history beside it.
 */
export function earlierReview(v, kept = 0) {
  const list = summarizeFindings(v).trim();
  const head = `- An earlier review of this same tree is recorded: ${v.verdict} at ${oneLine(v.recorded_at ?? 'an unknown time')}`;
  const lines = [(v.findings ?? []).some(serious) ? `${head}.\n${list}` : `${head}${overturns(v) ? '.' : ', with no blocker or major finding.'}${list ? `\n${list}` : ''}`];
  const low = lowScores(v);
  if (low.length) lines.push(`  Scores below 3: ${low.join(', ')}.`);
  const gate = failedGate(v);
  if (gate.length) lines.push(`  Gate entries not "pass": ${gate.map(oneLine).join(', ')}.`);
  if (v.verdict === 'approved' && kept > 0) lines.push(`  ${kept} later changes-requested verdict(s) gave no reason to overturn it and are kept as history beside it.`);
  lines.push('  Re-check each earlier finding, and say in your summary which still hold and which no longer do, with evidence.');
  if (v.verdict === 'approved') lines.push('  Overturn that approval only with a blocker or major finding, a score below 3 or a gate entry that is not "pass", quoting the evidence: a changes-requested verdict without one is kept as history and does not replace the approval.');
  return lines.join('\n');
}
