// Weigh-In mod (#114-#116): a band above the prompt with this repository's ledger counts, `/wi`
// commands answered without a Claude turn, the local board pane (pane.tsx), and a toast for the
// decisions anchored at a file an edit just touched.
//
// The ledger has one writer, the weighin CLI: the mod reads `open --repo --json`, `digest
// --json`, `board export --stdout` and `decisions --path <file> --json` from this plugin's own
// launcher, and writes only through `decide`, `defer`, `board apply --surface pane` and `batch`,
// run by argv with no shell. It observes the events it hooks and returns every `next(e)` result
// unchanged. Where no terminal or desktop draws (the VS Code panel, cloud sessions, Cowork,
// `claude -p`) and under WEIGHIN_MOD=0 it registers nothing and runs nothing.
import { atom, read, update } from 'claude-code'
import type { EngineInterface, Register, RenderSurface } from 'claude-code'

import type { WeighAnchored, WeighCounts, WeighView } from '../../types'
import { NO_BINARY, WRITE_TIMED_OUT, captureOff, cliError, firstLine, unfinished, weighin } from './cli'
import type { Run } from './cli'
import { BOARD_PANE, drawPane, isPaneOpen, openBoard, refreshPane } from './pane'

export { BOARD_PANE }

const view = atom({ plugin: 'weigh-mod', key: 'view' } as const, null)
const anchored = atom({ plugin: 'weigh-mod', key: 'anchored' } as const, null)
/** What Sit runs: the skill's own sitting, `/weigh-in`. */
const SIT = 'weigh-in'
/** Surfaces that draw the band. */
const DRAWS: ReadonlySet<RenderSurface> = new Set(['terminal', 'desktop'])
/** As long as a hook gets in hooks.json. */
const REFRESH_TIMEOUT_MS = 10_000
const COMMAND_TIMEOUT_MS = 30_000
/** `open --json` lists `--top` items (12 by default); the band counts every one. */
const ALL = '1000000'
/** The tools whose edits can touch a file a decision is anchored at (#116). */
const EDITS: ReadonlySet<string> = new Set(['Write', 'Edit', 'MultiEdit'])
/** As many decisions per edit as the PostToolUse hook gives the model (`recall::PER_EDIT`). */
const PER_EDIT = 3

const USAGE = [
  'Usage:',
  '  /wi open                     open items in this repository',
  '  /wi board                    the board pane: decide, defer or dismiss grounded cards',
  '  /wi stats                    the ledger\'s health numbers',
  '  /wi decide <id> <option> [note]',
  '  /wi decide <id>: <option> (note: …)',
  '  /wi defer <id> — trigger: …; resolve by: …; default: <X> (note: …)',
].join('\n')

// Off for this load: WEIGHIN_MOD=0, or no surface that draws. A reload runs register and
// session.start again, so module variables are enough here.
let off = true
let commandRegistered = false
let refreshing: Promise<void> | null = null
let lastTurn: string | null = null
// The status entry each surface asked for while its band had no room; the one shown.
const wantsStatus = new Map<RenderSurface, string>()
let shownStatus: string | undefined
// Whether this session was already told the edit toasts cannot read the ledger (once is enough).
let toldAnchoredFailure = false

export { captureOff }
export type { Run }

/** Whether a terminal or the desktop app draws this session. */
async function draws($: EngineInterface): Promise<boolean> {
  return (await $.session.surfaces()).some(s => DRAWS.has(s))
}

/** The counts from the two documents, or why they could not be read. */
export function countsOf(openJson: string, digestJson: string): WeighCounts | null {
  try {
    const items: unknown = JSON.parse(openJson)
    const digest: unknown = JSON.parse(digestJson)
    if (!Array.isArray(items) || typeof digest !== 'object' || digest === null) return null
    const defaults: unknown = (digest as { defaults?: unknown }).defaults
    if (!Array.isArray(defaults)) return null
    const oneWay = items.filter(i => doorOf(i) === 'one-way').length
    const unacked = defaults.filter(d => (d as { default?: { acknowledged?: unknown } } | null)?.default?.acknowledged !== true)
    return { open: items.length, oneWay, defaults: unacked.length }
  } catch {
    return null
  }
}

/** The item's door as triaged, else as raised: `Item::door` in weighin-core. */
function doorOf(item: unknown): unknown {
  const i = item as { door?: unknown; door_hint?: unknown } | null
  return i?.door ?? i?.door_hint
}

/** What a failed run says, for the band: the no-binary line, or the CLI's first error line. */
function failure(run: Run): WeighView {
  if (run.kind !== 'ran') return { kind: 'unavailable', line: unfinished(run) }
  const { exitCode, stderr, stdout } = run.result
  if (exitCode === NO_BINARY) return { kind: 'no-binary', line: captureOff(stderr) }
  const why = firstLine(stderr) || firstLine(stdout) || `exit ${exitCode}`
  return { kind: 'unavailable', line: `Weigh-In: ledger unavailable: ${why.replace(/^weighin:\s*/, '')}` }
}

/** One refresh: both documents, read side by side. */
async function readView($: EngineInterface): Promise<WeighView> {
  const [open, digest] = await Promise.all([
    weighin($, ['open', '--repo', '--json', '--top', ALL], REFRESH_TIMEOUT_MS),
    weighin($, ['digest', '--json'], REFRESH_TIMEOUT_MS),
  ])
  if (open.kind !== 'ran' || open.result.exitCode !== 0) return failure(open)
  if (digest.kind !== 'ran' || digest.result.exitCode !== 0) return failure(digest)
  if (open.result.isStdoutTruncated || digest.result.isStdoutTruncated) {
    return { kind: 'unavailable', line: 'Weigh-In: ledger unavailable: the listing was too long to read' }
  }
  const counts = countsOf(open.result.stdout, digest.result.stdout)
  if (counts === null) return { kind: 'unavailable', line: 'Weigh-In: ledger unavailable: weighin printed what does not parse' }
  return { kind: 'counts', counts }
}

/** Refreshes the band; a refresh already running is the one that answers. */
function refresh($: EngineInterface): Promise<void> {
  if (refreshing) return refreshing
  refreshing = (async () => {
    if (!(await draws($))) return
    const fresh = await readView($)
    await update($, view, () => fresh)
  })().finally(() => {
    refreshing = null
  })
  return refreshing
}

/** `/wi` is listed once a surface that draws is known. */
async function registerCommand($: EngineInterface): Promise<void> {
  if (commandRegistered) return
  commandRegistered = true
  await $.command.register({
    name: 'wi',
    description: 'Weigh-In ledger: open, stats, the board pane, decide or defer an item, with no Claude turn',
    argumentHint: 'open | stats | board | decide <id> <option> [note] | defer <id> — trigger: …; resolve by: …; default: <X>',
  })
}

/** What `/wi <args>` runs: the CLI's arguments, the board pane, or the usage text. */
export function commandArgs(input: string): { args: string[] } | { board: true } | { usage: string } {
  const text = input.trim()
  const [sub = '', ...rest] = text.split(/\s+/)
  const tail = text.slice(sub.length).trim()
  switch (sub) {
    case 'open':
      return rest.length === 0 ? { args: ['open', '--repo'] } : { usage: USAGE }
    case 'stats':
      return rest.length === 0 ? { args: ['stats'] } : { usage: USAGE }
    case 'board':
      return rest.length === 0 ? { board: true } : { usage: USAGE }
    case 'decide':
      return decideArgs(tail)
    case 'defer':
      return deferArgs(tail)
    default:
      return { usage: USAGE }
  }
}

/** Splits `(note: …)` off the end, as the board writes it. */
function splitNote(text: string): { body: string; note?: string } {
  const m = /^(.*?)\s*\(note:\s*(.*)\)\s*$/s.exec(text)
  if (!m) return { body: text }
  const [, body = '', note = ''] = m
  return note.trim() ? { body, note: note.trim() } : { body }
}

/** `<id> <option> [note]`, or the board's `<id>: <option> (note: …)`. */
function decideArgs(tail: string): { args: string[] } | { usage: string } {
  const { body, note: boardNote } = splitNote(tail)
  const m = /^(\S+?):?\s+(\S+)(?:\s+(.+))?$/s.exec(body.trim())
  if (!m) return { usage: USAGE }
  const [, id = '', option = '', plainNote] = m
  const note = boardNote ?? plainNote?.trim()
  // Options as `--flag=value` and the typed positionals after `--`, so text that starts with `-`
  // is read as text, never as a flag.
  return { args: ['decide', ...(note ? [`--note=${note}`] : []), '--', id, option] }
}

/** The board's `<id> — trigger: …; resolve by: …; default: <X> (note: …)`, as the skill maps it. */
function deferArgs(tail: string): { args: string[] } | { usage: string } {
  const { body, note } = splitNote(tail)
  const m = /^(\S+?):?\s+(?:—|–|--?)?\s*trigger:\s*(.+?);\s*resolve by:\s*(.+?);\s*default:\s*(.*)$/s.exec(body.trim())
  if (!m) return { usage: USAGE }
  const [, id = '', trigger = '', by = '', fallback = ''] = m
  return {
    args: [
      'defer', `--flip=${trigger.trim()}`, `--trigger=${trigger.trim()}`, `--by=${by.trim()}`, `--default=${fallback.trim()}`,
      ...(note ? [`--reason=${note}`] : []), '--', id,
    ],
  }
}

/** The command's answer: the CLI's own text, and its exit code when it failed. */
export function answer(run: Run, isWrite = false): { text: string; exitCode?: number } {
  if (run.kind === 'timed-out' && isWrite) return { text: WRITE_TIMED_OUT, exitCode: 1 }
  if (run.kind !== 'ran') return { text: unfinished(run), exitCode: 1 }
  const { exitCode, stdout, stderr } = run.result
  if (exitCode === NO_BINARY) return { text: captureOff(stderr), exitCode }
  if (exitCode === 0) return { text: stdout.trimEnd() || stderr.trimEnd() }
  const text = [stdout.trimEnd(), stderr.trimEnd()].filter(Boolean).join('\n') || `weighin exited ${exitCode}`
  return { text, exitCode: Math.min(255, Math.max(1, Math.trunc(exitCode))) }
}

/** The band's line, fitted to `columns`, or null when it has no room. `button` is the room the
 * buttons after it take. */
export function bandLine(c: WeighCounts, columns: number, button: number): string | null {
  const full = `Weigh-In · ${c.open} open here · ${c.oneWay} one-way · ${c.defaults} defaults`
  const short = `Weigh-In · ${c.open} open · ${c.oneWay} 1-way · ${c.defaults} dflt`
  for (const line of [full, short]) {
    if (line.length + button <= columns) return line
  }
  return null
}

/** The status entry: the full line, where the band has no room for it. */
export function statusLine(c: WeighCounts): string {
  return `Weigh-In · ${c.open} open here · ${c.oneWay} one-way · ${c.defaults} defaults`
}

/** Asks for (or withdraws) the status entry for one surface; shows it while any surface asks. */
function wantStatus($: EngineInterface, surface: RenderSurface, text: string | undefined): void {
  if (text === undefined) wantsStatus.delete(surface)
  else wantsStatus.set(surface, text)
  const next = wantsStatus.values().next().value
  if (next === shownStatus) return
  shownStatus = next
  $.ui.status(next)
}

/** `[ Sit ]` and `[ Board ]`, each with the space before it. */
const BUTTONS_WIDTH = ' [ Sit ]'.length + ' [ Board ]'.length

/** `s` on one line, cut to at most `n` characters, an ellipsis last when cut: `recall::one_line`. */
export function oneLine(s: string, n: number): string {
  const chars = Array.from(s.replace(/[\r\n]/g, ' '))
  if (chars.length <= n) return chars.join('')
  return n < 1 ? '' : `${chars.slice(0, n - 1).join('')}…`
}

/** The band's and the toast's line for one decision, as `recall::Anchored::line` writes it. */
export function decisionLine(d: unknown): string | null {
  const x = d as { id?: unknown; option?: unknown; title?: unknown; question?: unknown; at?: unknown } | null
  if (x === null || typeof x.id !== 'string' || typeof x.option !== 'string') return null
  const choice = typeof x.title === 'string' ? `${x.option} "${oneLine(x.title, 60)}"` : oneLine(x.option, 80)
  const question = typeof x.question === 'string' ? oneLine(x.question, 120) : ''
  const at = typeof x.at === 'string' ? ` (${x.at})` : ''
  return `${x.id} decided ${choice}: ${question}${at}`
}

/** The decisions `decisions --path <file> --json` printed, at most PER_EDIT, as lines. */
export function anchoredLines(stdout: string): string[] | null {
  try {
    const listed: unknown = JSON.parse(stdout)
    if (!Array.isArray(listed)) return null
    return listed.slice(0, PER_EDIT).map(decisionLine).filter((l): l is string => l !== null)
  } catch {
    return null
  }
}

/** Why `decisions --path --json` gave no list, or null when it did; null too for no binary,
 * which the band already reports as capture being off. */
function anchoredFailure(run: Run, lines: string[] | null): string | null {
  if (run.kind !== 'ran') return unfinished(run)
  if (run.result.exitCode === NO_BINARY) return null
  if (run.result.exitCode !== 0) return `Weigh-In: decisions at edited files unavailable: ${cliError(run.result)}`
  if (run.result.isStdoutTruncated || lines === null) return 'Weigh-In: decisions at edited files unavailable: weighin printed what does not parse'
  return null
}

/**
 * After an edit of `file`: one spawn, then a toast and a band line when decisions stand there.
 * An edit that lists none, of any file, clears the band line; a failure is told once a session.
 */
async function showAnchored($: EngineInterface, file: string): Promise<void> {
  const run = await weighin($, ['decisions', '--path', file, '--json'], REFRESH_TIMEOUT_MS)
  const lines = run.kind === 'ran' && run.result.exitCode === 0 && !run.result.isStdoutTruncated ? anchoredLines(run.result.stdout) : null
  if (lines === null || lines.length === 0) {
    await update($, anchored, () => null)
    const why = anchoredFailure(run, lines)
    if (why !== null && !toldAnchoredFailure) {
      toldAnchoredFailure = true
      $.ui.log(why)
    }
    return
  }
  const fresh: WeighAnchored = { file, lines }
  await update($, anchored, () => fresh)
  const head = lines.length === 1 ? 'Weigh-In: a decision stands here' : `Weigh-In: ${lines.length} decisions stand here`
  $.ui.toast([head, ...lines].join('\n'), { timeoutMs: 8000 })
}

/** The band's second row while decisions stand at the file last edited. */
function anchoredRow(a: WeighAnchored): string {
  const [first = ''] = a.lines
  return `Decided here: ${first}${a.lines.length > 1 ? ` (+${a.lines.length - 1} more)` : ''}`
}

/** After a write from the pane: the band's counts, read again. */
const afterWrite = ($: EngineInterface) => refresh($)

/** The band's Board button: the pane, or where it cannot be placed, the board as a notice. */
async function board($: EngineInterface): Promise<void> {
  const opened = await openBoard($)
  if (!opened.isPlaced) $.ui.log(opened.text)
}

export const register: Register = on => {
  off = true
  commandRegistered = false
  refreshing = null
  lastTurn = null
  wantsStatus.clear()
  shownStatus = undefined
  toldAnchoredFailure = false

  on('session.start', async ($, e, next) => {
    const result = await next(e)
    // A reload starts with no status entry asked for: clear one the previous load left up.
    $.ui.status(undefined)
    off = (await $.env.get('WEIGHIN_MOD')) === '0'
    if (off || !(await draws($))) return result
    await registerCommand($)
    await refresh($)
    return result
  })

  // A client that draws attached after the start (the desktop app connects as an SDK host, so
  // session.start saw no surface): register and draw now rather than at the first turn end.
  on('session.attach', async ($, e, next) => {
    const result = await next(e)
    if (!off && DRAWS.has(e.surface)) {
      await registerCommand($)
      $.clock.after(0, () => refresh($))
    }
    return result
  })

  // One refresh per main-loop turn, after the hook has returned, so a slow CLI never holds the
  // end of the turn. Subagents' turns end inside it and are not counted.
  on('turn.complete', async ($, e, next) => {
    const result = await next(e)
    if (off || e.agentId !== undefined || e.turnId === lastTurn) return result
    lastTurn = e.turnId
    $.clock.after(0, async () => {
      await refresh($)
      if (await isPaneOpen($)) await refreshPane($)
    })
    return result
  })

  // #116: after a Write, Edit or MultiEdit that succeeded, the decisions anchored at its file, in a
  // toast and on the band. It reads the call's result and returns it unchanged, and its .catch
  // replays that result: the mod never alters or decides a tool call.
  on('tool.call', async ($, e, next) => {
    const result = await next(e)
    if (off || !EDITS.has(String(e.tool))) return result
    if (result.deny !== undefined || result.isError === true) return result
    const file = (e as unknown as { file_path?: unknown }).file_path
    if (typeof file !== 'string' || file === '') return result
    if (!(await draws($))) return result
    $.clock.after(0, () => showAnchored($, file))
    return result
  }).catch(($, e, next) => next(e))

  on('command.run', { command: 'wi' }, async ($, e) => {
    const parsed = commandArgs(e.args)
    if ('usage' in parsed) return { text: parsed.usage, exitCode: 2 }
    if ('board' in parsed) {
      const opened = await openBoard($)
      return { text: opened.isPlaced ? 'Weigh-In: the board pane is open.' : opened.text }
    }
    const [sub] = parsed.args
    const isWrite = sub === 'decide' || sub === 'defer'
    const run = await weighin($, parsed.args, COMMAND_TIMEOUT_MS)
    const out = answer(run, isWrite)
    // A write that timed out may have landed: read the counts again as after one that did.
    if (isWrite && (out.exitCode === undefined || run.kind === 'timed-out')) await refresh($)
    return out
  })

  on('ui.render', { component: 'Pane', requestId: BOARD_PANE }, async ($, e) => {
    if (e.surface !== 'terminal' && e.surface !== 'desktop') {
      const { Box, Text } = $.ui.resolve(e)
      return <Box><Text dimColor>The Weigh-In board draws in a terminal or the desktop app.</Text></Box>
    }
    return drawPane($, $.ui.resolve(e), () => afterWrite($))
  })

  on('ui.render', { component: 'AbovePrompt' }, async ($, e, next) => {
    if (off || !DRAWS.has(e.surface)) return next(e)
    const v = await read($, view)
    const a = e.props.hasSurvey ? null : await read($, anchored)
    if (v === null || (v.kind === 'counts' && v.counts.open === 0 && v.counts.defaults === 0)) {
      wantStatus($, e.surface, undefined)
      if (a === null) return next(e)
      const { Box, Text } = $.ui.resolve(e)
      return (
        <Box>
          <Text key="anchored" dimColor wrap="truncate-end">{anchoredRow(a)}</Text>
        </Box>
      )
    }
    if (v.kind !== 'counts') {
      wantStatus($, e.surface, e.props.hasSurvey ? v.line : undefined)
      if (e.props.hasSurvey) return next(e)
      const { Box, Text } = $.ui.resolve(e)
      return (
        <Box>
          <Text dimColor wrap="truncate-end">{v.line}</Text>
        </Box>
      )
    }
    const line = e.props.hasSurvey ? null : bandLine(v.counts, e.props.bodyColumns, BUTTONS_WIDTH)
    if (line === null) {
      wantStatus($, e.surface, statusLine(v.counts))
      return next(e)
    }
    wantStatus($, e.surface, undefined)
    const { Box, Button, Text } = $.ui.resolve(e)
    const row = (
      <Box>
        <Text key="counts" wrap="truncate-end">{line}</Text>
        <Text> </Text>
        <Button key="sit" label="Sit" hotkey="s" variant="primary" onPress={() => sit($)} />
        <Text> </Text>
        <Button key="board" label="Board" hotkey="b" onPress={() => board($)} />
      </Box>
    )
    if (a === null) return row
    return (
      <Box flexDirection="column">
        {row}
        <Text key="anchored" dimColor wrap="truncate-end">{anchoredRow(a)}</Text>
      </Box>
    )
  })
}

/** Starts a sitting: runs the skill's `/weigh-in`, as the person typing it would. */
async function sit($: EngineInterface): Promise<void> {
  await $.command.run({ command: SIT })
}
