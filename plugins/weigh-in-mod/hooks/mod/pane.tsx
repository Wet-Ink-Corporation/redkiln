// Weigh-In mod: the local board pane (#115, D-0017), a third client of the board exchange format.
//
// It reads `weighin board export --stdout` (the board state document, never a file and never
// published) and `weighin open --repo --json` (for the items no sitting has grounded, which it
// shows read-only), and writes one Answer at a time through `weighin board apply --surface pane`
// on stdin. A default is acknowledged through `weighin batch` on stdin: the board exchange format
// has no `ack` answer. Under board mode `off` the export refuses and the pane shows its one line.
import { atom, read, update } from 'claude-code'
import type { Elements, EngineInterface } from 'claude-code'

import type {
  WeighBoardState, WeighCard, WeighDefault, WeighForm, WeighOption, WeighPane, WeighRawItem, WeighReceipt, WeighTab,
} from '../../types'
import { NO_BINARY, WRITE_TIMED_OUT, captureOff, cliError, unfinished, weighin } from './cli'
import type { Run } from './cli'

/** The pane's id: one per session, opened by the band's Board button and `/wi board`. */
export const BOARD_PANE = 'weigh-in-board'
const TITLE = 'Weigh-In board'
const READ_TIMEOUT_MS = 10_000
const WRITE_TIMEOUT_MS = 30_000
/** `open --json` lists `--top` items (12 by default); the pane lists every raw one. */
const ALL = '1000000'

export const pane = atom({ plugin: 'weigh-mod', key: 'pane' } as const, null)
export const tab = atom({ plugin: 'weigh-mod', key: 'tab' } as const, 'needs')
const selected = atom({ plugin: 'weigh-mod', key: 'selected' } as const, null)
const form = atom({ plugin: 'weigh-mod', key: 'form' } as const, null)
const drafts = atom({ plugin: 'weigh-mod', key: 'drafts' } as const, {})
const receipt = atom({ plugin: 'weigh-mod', key: 'receipt' } as const, null)
const busy = atom({ plugin: 'weigh-mod', key: 'busy' } as const, null)

/** The reasons a dismissal takes (`board::validate_answer`). */
export const DISMISS_REASONS = ['not-useful', 'already-decided', 'recoverable', 'duplicate', 'moot'] as const

const TABS: readonly WeighTab[] = ['needs', 'defaults', 'settled']

/** An answer document as the board page writes it (`weighin_core::board::Answer`). */
export type Answer = {
  item_id: string
  action: 'choose' | 'defer' | 'dismiss' | 'reopen'
  option?: string
  note?: string
  reason?: string
  defer?: { trigger: string; resolve_by: string; default: string }
  card_rev?: number
}

/** The pane's state from the two documents, or why they could not be read. */
export function paneOf(exported: Run, open: Run): WeighPane {
  if (exported.kind !== 'ran') return { kind: 'unavailable', line: unfinished(exported) }
  if (exported.result.exitCode === NO_BINARY) return { kind: 'unavailable', line: captureOff(exported.result.stderr) }
  // Board mode `off`, no config, a broken one: the export's own one line.
  if (exported.result.exitCode !== 0) return { kind: 'refused', line: cliError(exported.result) }
  if (open.kind !== 'ran') return { kind: 'unavailable', line: unfinished(open) }
  if (open.result.exitCode !== 0) return { kind: 'unavailable', line: `Weigh-In: ledger unavailable: ${cliError(open.result)}` }
  if (exported.result.isStdoutTruncated || open.result.isStdoutTruncated) {
    return { kind: 'unavailable', line: 'Weigh-In: ledger unavailable: the board was too long to read' }
  }
  const board = boardOf(exported.result.stdout)
  const raw = rawOf(open.result.stdout)
  if (board === null || raw === null) return { kind: 'unavailable', line: 'Weigh-In: ledger unavailable: weighin printed what does not parse' }
  return { kind: 'board', board, raw }
}

function boardOf(stdout: string): WeighBoardState | null {
  try {
    const doc: unknown = JSON.parse(stdout)
    if (typeof doc !== 'object' || doc === null) return null
    const d = doc as Partial<WeighBoardState>
    if (d.schema !== 1 || !Array.isArray(d.items)) return null
    return {
      schema: 1,
      scope: d.scope ?? { repo: '', session: '', source: '', generated_at: '', board_mode: '' },
      items: d.items,
      decided: Array.isArray(d.decided) ? d.decided : [],
      defaults: Array.isArray(d.defaults) ? d.defaults : [],
      defaults_total: typeof d.defaults_total === 'number' ? d.defaults_total : 0,
      settled: Array.isArray(d.settled) ? d.settled : [],
    }
  } catch {
    return null
  }
}

/** The open items with no card: the ones no sitting has grounded. */
function rawOf(stdout: string): WeighRawItem[] | null {
  try {
    const items: unknown = JSON.parse(stdout)
    if (!Array.isArray(items)) return null
    const raw: WeighRawItem[] = []
    for (const item of items) {
      const i = item as { id?: unknown; question?: unknown; door?: unknown; door_hint?: unknown; at?: unknown; source_path?: unknown; card?: unknown } | null
      if (i === null || typeof i.id !== 'string' || (i.card !== null && i.card !== undefined)) continue
      const door = i.door ?? i.door_hint
      const at = i.at ?? i.source_path
      raw.push({
        id: i.id,
        question: typeof i.question === 'string' ? i.question : '',
        door: typeof door === 'string' ? door : 'unsure',
        at: typeof at === 'string' ? at : null,
      })
    }
    return raw
  } catch {
    return null
  }
}

/** Reads both documents side by side and stores what the pane draws. */
export async function refreshPane($: EngineInterface): Promise<void> {
  const [exported, open] = await Promise.all([
    weighin($, ['board', 'export', '--stdout'], READ_TIMEOUT_MS),
    weighin($, ['open', '--repo', '--json', '--top', ALL], READ_TIMEOUT_MS),
  ])
  const fresh = paneOf(exported, open)
  await update($, pane, () => fresh)
}

/** Whether the pane is open and drawn now. One waiting undrawn reads the board when it opens. */
export async function isPaneOpen($: EngineInterface): Promise<boolean> {
  return (await $.ui.panes()).some(p => p.id === BOARD_PANE && p.isPlaced)
}

/**
 * Opens the pane on fresh data. Where it cannot be placed, the text the band's Board button logs
 * and `/wi board` answers instead: the board as lines, with the `/wi` commands that answer it.
 */
export async function openBoard($: EngineInterface): Promise<{ isPlaced: true } | { isPlaced: false; text: string }> {
  await refreshPane($)
  const opened = await $.ui.open({ id: BOARD_PANE, title: TITLE, focus: true })
  if (opened.isPlaced) return { isPlaced: true }
  return { isPlaced: false, text: boardText(await read($, pane), opened.reason) }
}

/** The board as lines, for where the pane cannot be placed. */
export function boardText(p: WeighPane | null, reason: string): string {
  const head = `Weigh-In: the board pane cannot open here (${reason}).`
  if (p === null) return head
  if (p.kind !== 'board') return `${head}\n${p.line}`
  const { board, raw } = p
  const lines = [head, `Needs you (${board.items.length}):`]
  if (board.items.length === 0) lines.push('  nothing')
  for (const c of board.items) {
    lines.push(`  ${c.id} [${c.door}] ${c.question}`)
    const options = c.options.map(o => `${o.id} ${o.title}${o.fails_must ? ' (fails a MUST)' : ''}`).join(' · ')
    const rec = recommended(c) ? ` · recommended ${c.recommendation.option}` : ''
    lines.push(`    ${options}${rec}`)
  }
  if (board.defaults.length > 0) {
    lines.push(`Decided for you (${board.defaults_total}):`)
    for (const d of board.defaults) lines.push(`  ${d.id} ${d.question}: took ${d.taken}`)
  }
  if (raw.length > 0) {
    lines.push(`Not grounded yet, so read-only: settle in a sitting (/weigh-in) (${raw.length}):`)
    for (const r of raw) lines.push(`  ${r.id} [${r.door}] ${r.question}`)
  }
  lines.push('Answer with /wi decide <id> <option> [note] or /wi defer <id> — trigger: …; resolve by: …; default: <X>.')
  return lines.join('\n')
}

/** Whether the card pre-selects its recommendation: earned, and not a one-way door. */
function recommended(c: WeighCard): boolean {
  return c.present === 'recommended' && c.door !== 'one-way'
}

/** The note a field holds, when it holds one. */
function noted(note: string | undefined): { note: string } | Record<string, never> {
  const n = note?.trim()
  return n ? { note: n } : {}
}

function rev(c: WeighCard): { card_rev: number } | Record<string, never> {
  return c.card_rev === undefined ? {} : { card_rev: c.card_rev }
}

/** The Answer a press on option `option` of card `c` writes. */
export function chooseAnswer(c: WeighCard, option: string, note?: string): Answer {
  return { item_id: c.id, action: 'choose', option, ...rev(c), ...noted(note) }
}

/** The Answer the defer form writes, or, with nothing sent, which of its parts are missing. */
export function deferAnswer(c: WeighCard, trigger: string, resolveBy: string, fallback: string, note?: string): Answer | string {
  const missing = [!trigger.trim() && 'a trigger', !resolveBy.trim() && 'a resolve-by', !fallback.trim() && 'a default'].filter((m): m is string => m !== false)
  if (missing.length > 0) {
    const last = missing.pop() ?? ''
    return `To defer, give ${missing.length > 0 ? `${missing.join(', ')} and ${last}` : last}.`
  }
  return { item_id: c.id, action: 'defer', defer: { trigger: trigger.trim(), resolve_by: resolveBy.trim(), default: fallback }, ...rev(c), ...noted(note) }
}

export function dismissAnswer(id: string, reason: string, note?: string): Answer {
  return { item_id: id, action: 'dismiss', reason, ...noted(note) }
}

export function reopenAnswer(id: string, note?: string): Answer {
  return { item_id: id, action: 'reopen', ...noted(note) }
}

/** The receipt of one write: the CLI's ✓ line, or its refusal as it printed it. */
export function receiptOf(run: Run): WeighReceipt {
  // A write killed at its timeout may have landed: say so, and the board read after it shows which.
  if (run.kind === 'timed-out') return { isRefused: true, text: WRITE_TIMED_OUT }
  if (run.kind !== 'ran') return { isRefused: true, text: unfinished(run) }
  const { exitCode, stdout, stderr } = run.result
  if (exitCode === NO_BINARY) return { isRefused: true, text: captureOff(stderr) }
  const lines = stdout.split('\n').map(l => l.trim()).filter(l => l !== '' && !l.startsWith('applied:'))
  if (exitCode === 0) return { isRefused: false, text: lines.join('\n') || 'applied' }
  const refused = lines.filter(l => l.startsWith('✗'))
  return { isRefused: true, text: refused.length > 0 ? refused.join('\n') : cliError(run.result) }
}

/**
 * Takes the pane's one write for `id`, or answers false while another is being written: a second
 * press before the first settles is ignored, not sent to be refused as already answered.
 */
async function claim($: EngineInterface, id: string): Promise<boolean> {
  let isMine = false
  await update($, busy, b => {
    isMine = b === null
    return b ?? id
  })
  return isMine
}

/** Runs one write under the claim, and lets the claim go once the board is read again. */
async function writing($: EngineInterface, id: string, write: () => Promise<void>): Promise<void> {
  if (!(await claim($, id))) return
  try {
    await write()
  } finally {
    await update($, busy, () => null)
  }
}

/** Writes one Answer through `board apply --surface pane`, then refreshes the pane. */
async function apply($: EngineInterface, answer: Answer, after: () => Promise<void>): Promise<void> {
  await writing($, answer.item_id, async () => {
    const run = await weighin($, ['board', 'apply', '--surface', 'pane'], WRITE_TIMEOUT_MS, JSON.stringify(answer))
    await settle($, answer.item_id, receiptOf(run), after)
  })
}

/** Acknowledges a default: `weighin batch` on stdin, as a sitting's digest does. */
async function ack($: EngineInterface, id: string, after: () => Promise<void>): Promise<void> {
  await writing($, id, async () => {
    const run = await weighin($, ['batch'], WRITE_TIMEOUT_MS, JSON.stringify([{ op: 'ack', id }]))
    // `batch` exits 1 when it refuses an operation, its ✗ receipt on stdout: receiptOf shows it.
    await settle($, id, receiptOf(run), after)
  })
}

/** Shows the receipt, drops a settled item's fields and form, and reads the board again. */
async function settle($: EngineInterface, id: string, r: WeighReceipt, after: () => Promise<void>): Promise<void> {
  await update($, receipt, () => r)
  if (!r.isRefused) {
    await update($, drafts, d => Object.fromEntries(Object.entries(d).filter(([k]) => !k.endsWith(`:${id}`))))
    await update($, form, f => (f?.id === id ? null : f))
  }
  await refreshPane($)
  await after()
}

/** What field `key` holds now: read when a handler runs, never from the drawing it closed over. */
async function held($: EngineInterface, key: string): Promise<string | undefined> {
  return (await read($, drafts))[key]
}

async function draft($: EngineInterface, key: string, value: string): Promise<void> {
  await update($, drafts, d => ({ ...d, [key]: value }))
}

async function showTab($: EngineInterface, t: WeighTab): Promise<void> {
  await update($, tab, () => t)
  await update($, receipt, () => null)
}

async function toggleForm($: EngineInterface, id: string, kind: WeighForm['kind']): Promise<void> {
  await update($, form, f => (f?.id === id && f.kind === kind ? null : { id, kind }))
}

type Els = Elements['terminal'] | Elements['desktop']

/** What the pane draws on a surface with fields: the tabs, the last receipt and the tab's body. */
export async function drawPane($: EngineInterface, els: Els, after: () => Promise<void>) {
  const { Box, Button, Text } = els
  const p = await read($, pane)
  if (p === null) return <Box><Text dimColor>Reading the board…</Text></Box>
  if (p.kind !== 'board') {
    return (
      <Box flexDirection="column">
        <Text key="refused">{p.line}</Text>
      </Box>
    )
  }
  const current = await read($, tab)
  const r = await read($, receipt)
  const counts: Record<WeighTab, string> = {
    needs: `Needs you (${p.board.items.length})`,
    defaults: `Decided for you (${p.board.defaults_total})`,
    settled: `Settled (${p.board.decided.length + p.board.settled.length})`,
  }
  const writingFor = await read($, busy)
  const ctx: Ctx = { els, after, drafts: await read($, drafts), form: await read($, form), isBusy: writingFor !== null }
  const body =
    current === 'defaults' ? defaultsTab($, ctx, p.board) : current === 'settled' ? settledTab(ctx, p.board) : needsTab($, ctx, p.board, p.raw, await read($, selected))
  return (
    <Box flexDirection="column">
      <Box>
        {TABS.map(t => (
          <Button key={`tab-${t}`} label={counts[t]} variant={t === current ? 'primary' : undefined} onPress={() => showTab($, t)} />
        ))}
      </Box>
      {writingFor !== null && <Text key="busy" dimColor>{`Writing the answer for ${writingFor}…`}</Text>}
      {writingFor === null && r !== null && <Text key="receipt" color={r.isRefused ? 'error' : 'success'}>{r.text}</Text>}
      {body}
    </Box>
  )
}

type Ctx = {
  els: Els
  after: () => Promise<void>
  drafts: Record<string, string>
  form: WeighForm | null
  /** While a write is being settled: the write buttons are drawn dim, and a press is ignored. */
  isBusy: boolean
}

function needsTab($: EngineInterface, ctx: Ctx, board: WeighBoardState, raw: readonly WeighRawItem[], picked: string | null) {
  const { Box, Button, Text } = ctx.els
  const shown = board.items.find(c => c.id === picked) ?? board.items[0]
  return (
    <Box flexDirection="column">
      {board.items.length === 0 && <Text key="empty" dimColor>Nothing needs you.</Text>}
      {board.items.length > 1 &&
        board.items.map(c => (
          <Button key={`pick:${c.id}`} plain dimColor={c.id !== shown?.id} onPress={() => update($, selected, () => c.id)}>
            {`${c.rank}. ${c.question} `}
            <Text dimColor>{`${c.id} · ${c.door}`}</Text>
          </Button>
        ))}
      {shown !== undefined && card($, ctx, shown)}
      {raw.length > 0 && (
        <Box flexDirection="column" marginTop={1}>
          <Text key="raw-head" bold>Not grounded yet: settle in a sitting (/weigh-in). Read-only here.</Text>
          {raw.map(i => (
            <Text key={`raw:${i.id}`} dimColor>{`${i.id} · ${i.door} · ${i.question}${i.at ? ` (${i.at})` : ''}`}</Text>
          ))}
        </Box>
      )}
    </Box>
  )
}

/** One card in full: options first, with no option pre-selected, on a one-way door. */
function card($: EngineInterface, ctx: Ctx, c: WeighCard) {
  const { Box, Text, Code } = ctx.els
  const optionsFirst = !recommended(c)
  const options = (
    <Box key={`options:${c.id}`} flexDirection="column">
      <Text bold>Options</Text>
      {c.options.map(o => option(ctx, o))}
    </Box>
  )
  const rec = recommendation(ctx, c)
  return (
    <Box flexDirection="column" marginTop={1}>
      <Text key={`q:${c.id}`} bold>{c.question}</Text>
      <Text dimColor>{`${c.id} · rank ${c.rank}`}</Text>
      {c.why !== '' && <Text>{c.why}</Text>}
      <Text>{`Door: ${c.door}${c.door_note ? ` · ${c.door_note}` : ''}`}</Text>
      <Text>{`Blast: ${c.blast} · Resolve by: ${c.resolve_by}`}</Text>
      {c.blocks !== undefined && c.blocks.length > 0 && <Text>{`Blocks: ${c.blocks.join(', ')}`}</Text>}
      {c.timing !== '' && <Text>{`Timing: ${c.timing}`}</Text>}
      {c.evidence.length > 0 && <Text bold>Evidence</Text>}
      {c.evidence.map((ev, i) => (
        <Text key={`ev:${c.id}:${i}`}>
          {ev.src}
          <Text dimColor>{`  ${ev.text}`}</Text>
        </Text>
      ))}
      {c.excerpt !== undefined && <Text dimColor>{c.excerpt.path}</Text>}
      {c.excerpt !== undefined && <Code source={c.excerpt.lines.join('\n')} path={c.excerpt.path} startLine={c.excerpt.start} />}
      {optionsFirst ? options : rec}
      {optionsFirst ? rec : options}
      {actions($, ctx, c)}
    </Box>
  )
}

function option(ctx: Ctx, o: WeighOption) {
  const { Box, Text } = ctx.els
  const failed = o.fails_must !== undefined
  return (
    <Box key={`option:${o.id}`} flexDirection="column">
      <Text strikethrough={failed}>{`${o.id}  ${o.title}${o.summary ? ` · ${o.summary}` : ''}`}</Text>
      {o.holds_if ? <Text dimColor>{`   Holds if: ${o.holds_if}`}</Text> : null}
      {o.if_wrong ? <Text dimColor>{`   If wrong: ${o.if_wrong}`}</Text> : null}
      {failed ? <Text color="error">{`   Fails a MUST: ${o.fails_must}`}</Text> : null}
      {o.evasion ? <Text color="warning">{`   Evasion: ${o.evasion}`}</Text> : null}
    </Box>
  )
}

function recommendation(ctx: Ctx, c: WeighCard) {
  const { Box, Text } = ctx.els
  const r = c.recommendation
  return (
    <Box key={`rec:${c.id}`} flexDirection="column">
      <Text bold>{`Recommendation: ${r.option}`}</Text>
      {r.tradeoff ? <Text>{r.tradeoff}</Text> : null}
      {r.flip ? <Text dimColor>{`Unless: ${r.flip}`}</Text> : null}
      {r.why_wrong ? <Text dimColor>{`Might be wrong: ${r.why_wrong}`}</Text> : null}
    </Box>
  )
}

/** The note field, a button per option that can be chosen, and the defer and dismiss forms. */
function actions($: EngineInterface, ctx: Ctx, c: WeighCard) {
  const { els, after } = ctx
  const { Box, Button, Input, Select } = els
  const pre = recommended(c) ? c.recommendation.option : undefined
  const eligible = c.options.filter(o => o.fails_must === undefined)
  const open = ctx.form?.id === c.id ? ctx.form.kind : null
  // As on the card, no default is pre-selected where no option is (a one-way door).
  const firstDefault = pre !== undefined && eligible.some(o => o.id === pre) ? pre : undefined
  const note = () => held($, `note:${c.id}`)
  const shownDefault = ctx.drafts[`default:${c.id}`] ?? firstDefault
  const fallback = async () => (await held($, `default:${c.id}`)) ?? firstDefault ?? ''
  const reason = async () => (await held($, `reason:${c.id}`)) ?? DISMISS_REASONS[0]
  return (
    <Box key={`actions:${c.id}`} flexDirection="column" marginTop={1}>
      <Input key={`note:${c.id}`} label="Note: " placeholder="optional, kept with the answer" value={ctx.drafts[`note:${c.id}`] ?? ''}
        onInput={v => void draft($, `note:${c.id}`, v)} onSubmit={v => void draft($, `note:${c.id}`, v)} />
      <Box>
        {eligible.map(o => (
          <Button key={`choose:${c.id}:${o.id}`} label={`${o.id}: ${o.title}`} variant={o.id === pre ? 'primary' : undefined} dimColor={ctx.isBusy}
            {...(o.id === pre ? { autoFocus: true as const } : {})}
            onPress={async () => apply($, chooseAnswer(c, o.id, await note()), after)} />
        ))}
        {eligible.length > 0 && (
          <Button key={`defer:${c.id}`} label="Defer…" variant={open === 'defer' ? 'secondary' : undefined} onPress={() => toggleForm($, c.id, 'defer')} />
        )}
        <Button key={`dismiss:${c.id}`} label="Dismiss…" variant={open === 'dismiss' ? 'secondary' : undefined} onPress={() => toggleForm($, c.id, 'dismiss')} />
      </Box>
      {open === 'defer' && eligible.length > 0 && (
        <Box key={`defer-form:${c.id}`} flexDirection="column">
          <Input key={`trigger:${c.id}`} label="Trigger (the fact that flips it): " value={ctx.drafts[`trigger:${c.id}`] ?? ''}
            onInput={v => void draft($, `trigger:${c.id}`, v)} onSubmit={v => void draft($, `trigger:${c.id}`, v)} />
          <Input key={`by:${c.id}`} label="Resolve by: " placeholder="a date, or before-merge:<branch>" value={ctx.drafts[`by:${c.id}`] ?? ''}
            onInput={v => void draft($, `by:${c.id}`, v)} onSubmit={v => void draft($, `by:${c.id}`, v)} />
          <Select key={`default:${c.id}`} label="Default if nobody acts: " {...(shownDefault === undefined ? {} : { value: shownDefault })}
            options={eligible.map(o => ({ value: o.id, label: `${o.id}: ${o.title}` }))}
            onSelect={v => void draft($, `default:${c.id}`, v)} />
          <Button key={`defer-send:${c.id}`} label="Defer" dimColor={ctx.isBusy} onPress={async () => {
            const a = deferAnswer(c, (await held($, `trigger:${c.id}`)) ?? '', (await held($, `by:${c.id}`)) ?? '', await fallback(), await note())
            return typeof a === 'string' ? update($, receipt, () => ({ isRefused: true, text: a })) : apply($, a, after)
          }} />
        </Box>
      )}
      {open === 'dismiss' && (
        <Box key={`dismiss-form:${c.id}`} flexDirection="column">
          <Select key={`reason:${c.id}`} label="Reason: " value={ctx.drafts[`reason:${c.id}`] ?? DISMISS_REASONS[0]} options={DISMISS_REASONS.map(r => ({ value: r }))}
            onSelect={v => void draft($, `reason:${c.id}`, v)} />
          <Button key={`dismiss-send:${c.id}`} label="Dismiss" dimColor={ctx.isBusy} onPress={async () => apply($, dismissAnswer(c.id, await reason(), await note()), after)} />
        </Box>
      )}
    </Box>
  )
}

function defaultsTab($: EngineInterface, ctx: Ctx, board: WeighBoardState) {
  const { Box, Text } = ctx.els
  const more = board.defaults_total - board.defaults.length
  return (
    <Box flexDirection="column">
      {board.defaults.length === 0 && <Text key="empty" dimColor>No default waits for you.</Text>}
      {board.defaults.map(d => dflt($, ctx, d))}
      {more > 0 && <Text key="more" dimColor>{`${more} more: weighin digest lists them.`}</Text>}
    </Box>
  )
}

/** A default taken without the owner: reopen it as a question, or acknowledge it. */
function dflt($: EngineInterface, ctx: Ctx, d: WeighDefault) {
  const { els, after } = ctx
  const { Box, Button, Input, Text } = els
  const note = () => held($, `note:${d.id}`)
  return (
    <Box key={`dflt:${d.id}`} flexDirection="column" marginTop={1}>
      <Text bold>{d.question}</Text>
      <Text dimColor>{`${d.id} · ${d.cause}`}</Text>
      <Text>{`Taken: ${d.taken}`}</Text>
      {d.alt !== '' && <Text dimColor>{`Instead of: ${d.alt}`}</Text>}
      <Text dimColor>{`Why: ${d.reason}`}</Text>
      <Input key={`note:${d.id}`} label="Note: " placeholder="optional, kept with a reopen" value={ctx.drafts[`note:${d.id}`] ?? ''}
        onInput={v => void draft($, `note:${d.id}`, v)} onSubmit={v => void draft($, `note:${d.id}`, v)} />
      <Box>
        <Button key={`reopen:${d.id}`} label="Reopen" dimColor={ctx.isBusy} onPress={async () => apply($, reopenAnswer(d.id, await note()), after)} />
        <Button key={`ack:${d.id}`} label="Ack" dimColor={ctx.isBusy} onPress={() => ack($, d.id, after)} />
      </Box>
    </Box>
  )
}

function settledTab(ctx: Ctx, board: WeighBoardState) {
  const { Box, Text } = ctx.els
  const empty = board.decided.length === 0 && board.settled.length === 0
  return (
    <Box flexDirection="column">
      {empty && <Text key="empty" dimColor>Nothing settled yet.</Text>}
      {board.decided.map(c => {
        const chosen = c.options.find(o => o.id === c.decision?.option)
        const answer = c.decision ? `${c.decision.option}${chosen ? ` ${chosen.title}` : ''}` : '?'
        return (
          <Text key={`settled:${c.id}`}>
            {`${c.question} → ${answer} `}
            <Text dimColor>{`${c.id} · ${c.decision?.provenance ?? ''} · ${c.decision?.decided_at ?? ''}`}</Text>
          </Text>
        )
      })}
      {board.settled.map(s => (
        <Text key={`settled:${s.id}`}>
          {`${s.question} → ${s.answer} `}
          <Text dimColor>{`${s.id} · ${s.provenance} · ${s.decided_at}`}</Text>
        </Text>
      ))}
    </Box>
  )
}
