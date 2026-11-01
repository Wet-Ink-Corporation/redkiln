// Weigh-In mod (#114-#116), under `claude plugin test plugin-mod`. The test's own hooks answer
// `$.process.run` (the weighin launcher), `$.session.*`, `$.env.get`, `$.ui.open` and the tool
// beneath `tool.call`, so no process runs: each case hands the mod what `open --repo --json`,
// `digest --json`, `board export --stdout` and `decisions --json` print and reads what it draws.
import { expect, mock, test } from 'claude-code/testing'
import type { On, RenderSurface } from 'claude-code'

const SURFACES = ['terminal', 'desktop'] as const

const BAND = {
  plugin: 'weigh-mod',
  component: 'AbovePrompt',
  props: { hasSurvey: false, isWorking: false, maxRows: 10, bodyColumns: 120, scroll: { offset: 0, bodyRows: 10 }, view: {} },
} as const

const item = (id: string, door: string | null, hint = 'unsure') => ({ id, door, door_hint: hint, status: 'raised' })
const dflt = (id: string, acknowledged = false) => ({ id, status: 'defaulted', default: { option: 'A', acknowledged } })

const OPEN3 = JSON.stringify([item('wi-000001', 'one-way'), item('wi-000002', null, 'one-way'), item('wi-000003', 'two-way', 'one-way')])
const DIGEST = (defaults: unknown[] = []) => JSON.stringify({ defaults, untriaged_from_other_sessions: [] })

type Out = { exitCode?: number; stdout?: string; stderr?: string }
type Answer = (argv: string[], stdin?: string) => Out | Error

const DOOR_NOTE = 'a schema other services read'
/** The board state document `board export --stdout` prints, as `weighin_core::board` writes it. */
const card1 = {
  id: 'wi-aaaaa1', rank: 1, question: 'Which cache backs the session store?', why: 'Every request reads it.',
  door: 'one-way', door_note: DOOR_NOTE, blast: 'system', authority: 'authority', resolve_by: '2026-11-01',
  timing: 'Decide now: the schema ships Friday.', source: { path: 'src/a.rs:10', quote: 'Which cache?' },
  evidence: [{ src: 'src/a.rs:10', text: 'the cache is built here' }, { src: 'docs/design.md:4', text: 'p99 under 5 ms' }],
  excerpt: { path: 'src/a.rs', start: 9, lines: ['let cache = Cache::new();', 'cache.warm();'] },
  options: [
    { id: 'A', title: 'Redis', summary: 'a shared cache', holds_if: 'we run more than one node', if_wrong: 'an extra service to run' },
    { id: 'B', title: 'In-process LRU', summary: '', holds_if: 'one node is enough', if_wrong: 'a cold start per deploy' },
    { id: 'C', title: 'No cache', summary: '', holds_if: '', if_wrong: '', fails_must: 'p99 under 5 ms' },
  ],
  criteria: [], recommendation: { option: 'A', flip: 'one node is enough', tradeoff: 'You trade a service for shared state.', why_wrong: 'traffic may stay low' },
  present: 'options-first', card_rev: 2,
}
const card2 = {
  ...card1, id: 'wi-bbbbb2', rank: 2, question: 'Tabs or spaces in the generated config?', why: 'Diffs.', door: 'two-way',
  door_note: undefined, blast: 'file', present: 'recommended', card_rev: 5, evidence: [], excerpt: undefined,
  options: [{ id: 'A', title: 'Spaces', holds_if: 'the linter wants them' }, { id: 'B', title: 'Tabs' }],
  recommendation: { option: 'A', tradeoff: 'Consistency over habit.' },
}
const decided = { ...card2, id: 'wi-ccccc3', rank: 0, question: 'Which port?', decision: { option: 'B', decided_at: '2026-10-18T09:00:00Z', provenance: 'user-directed', matched_recommendation: false, applied_at: '2026-10-18T09:00:00Z', ledger_note: '' } }
const dflt4 = { id: 'wi-ddddd4', question: 'Retry the upload?', taken: 'Retry three times', alt: 'Fail fast', reason: 'two-way door the agent acted on', cause: 'two-way' }
const settled5 = { id: 'kb-0007', question: 'Which licence?', answer: 'MIT OR Apache-2.0', decided_at: '2026-09-01', provenance: 'user-approved' }
const scope = { repo: 'w', session: 'session test-session', source: 'main', generated_at: '2026-10-19T10:00:00Z', board_mode: 'local' }
const BOARD = (over: Record<string, unknown> = {}) =>
  JSON.stringify({ schema: 1, scope, items: [card1, card2], decided: [decided], defaults: [dflt4], defaults_total: 1, settled: [settled5], ...over })
/** `open --repo --json`: the two grounded items and one raw item no sitting has grounded. */
const OPEN_RAW = JSON.stringify([
  { ...item('wi-aaaaa1', 'one-way'), card: { why: '' } },
  { ...item('wi-bbbbb2', 'two-way'), card: { why: '' } },
  { ...item('wi-eeeee5', null, 'one-way'), question: 'Should the API version in the path?', at: 'src/api.rs:3', card: null },
])
const OFF_LINE = "this repo's board mode is `off`: the board shows nothing here (`weighin init --board local` changes the mode)"

const PANE = {
  plugin: 'weigh-mod',
  component: 'Pane',
  requestId: 'weigh-in-board',
  props: { title: 'Weigh-In board', isFocused: true, bodyColumns: 100, placement: 'dock', scroll: { offset: 0, bodyRows: 60 }, view: {} },
} as const

/** The world beneath the mod: the CLI's answers, the session, the environment; records each run. */
type World = {
  answer?: Answer; open?: string; digest?: string; surfaces?: RenderSurface[]; env?: Record<string, string>
  board?: Out; decisions?: string; isPlaced?: boolean; tool?: () => unknown
  /** Holds a run before it answers: a slow CLI. */
  wait?: (argv: string[]) => Promise<void> | undefined
}

function world(on: On, opts: World = {}) {
  const runs: string[][] = []
  const stdins: (string | undefined)[] = []
  const status: (string | undefined)[] = []
  const submits: unknown[] = []
  const commands: string[] = []
  const opens: unknown[] = []
  const toasts: string[] = []
  const logs: string[] = []
  const tools: unknown[] = []
  const answer: Answer = opts.answer ?? (argv => {
    if (argv.includes('export')) return opts.board ?? { stdout: BOARD() }
    if (argv.includes('decisions')) return { stdout: opts.decisions ?? '[]' }
    if (argv.includes('open') && argv.includes('--json')) return { stdout: opts.open ?? OPEN3 }
    if (argv.includes('digest')) return { stdout: opts.digest ?? DIGEST([dflt('wi-000009'), dflt('wi-00000a'), dflt('wi-00000b', true)]) }
    return { stdout: '' }
  })
  on('process.run', async (_$, e) => {
    const argv = [...e.argv]
    runs.push(argv)
    stdins.push(e.init?.stdin)
    await opts.wait?.(argv)
    const out = answer(argv, e.init?.stdin)
    if (out instanceof Error) return { deny: out.message } as any
    return { value: { exitCode: out.exitCode ?? 0, stdout: out.stdout ?? '', stderr: out.stderr ?? '', isStdoutTruncated: false, isStderrTruncated: false } }
  })
  on('session.surfaces', async () => ({ value: opts.surfaces ?? ['terminal'] }) as any)
  on('session.id', async () => ({ value: 'test-session' }) as any)
  on('session.root', async () => ({ value: '/w' }) as any)
  on('ui.status', async (_$, e: any) => { status.push(e.text); return { value: undefined } as any })
  on('command.run', { command: 'weigh-in' }, async (_$, e: any) => { submits.push(e); return { text: '' } })
  on('command.register', async (_$, e: any) => { commands.push(e.name); return { value: undefined } as any })
  on('session.start', async (_$, e) => ({ cwd: e.cwd }))
  on('turn.complete', async () => ({ text: '' }))
  on('session.attach', async (_$, e) => ({ clientId: e.clientId }) as any)
  // The engine draws nothing in the band: an empty Box stands for that here.
  on('ui.render', { component: 'AbovePrompt' }, async () => h('Box', {}) as any)
  const open: { id: string }[] = []
  on('ui.open', async (_$, e: any) => {
    opens.push(e)
    if (!open.some(p => p.id === e.id)) open.push({ id: e.id })
    const placed = opts.isPlaced ?? true
    return { value: placed ? { isPlaced: true } : { isPlaced: false, reason: 'no surface attached places panes' } } as any
  })
  on('ui.panes', async () => ({ value: open.map(p => ({ ...p, title: p.id, isShown: true, isFocused: false, isPlaced: opts.isPlaced ?? true })) }) as any)
  on('ui.toast', async (_$, e: any) => { toasts.push(e.text); return { value: undefined } as any })
  on('ui.log', async (_$, e: any) => { logs.push(e.text); return { value: undefined } as any })
  // The tool beneath every plugin: it answers each call as the engine would.
  on('tool.call', async (_$, e: any) => {
    tools.push(e)
    return (opts.tool?.() ?? { result: { type: 'update', filePath: e.file_path }, text: 'The file was updated.' }) as any
  })
  mock.env(on, opts.env ?? {})
  return { runs, stdins, status, submits, commands, opens, toasts, logs, tools }
}

const start = ($: any, surface: RenderSurface | null = 'terminal') => $.session.start({ cwd: '/w', surface, isInteractive: true })

/** A main-loop turn ends; the mod refreshes once its hook has returned, so the clock moves on by 0 ms. */
async function turnEnd($: any, clock: { advance: (ms: number) => Promise<void> }, turnId = 't1', agentId?: string) {
  await $.turn.complete({ answer: '', durationMs: 1, isAborted: false, turnId, reason: 'answer', ...(agentId ? { agentId } : {}) })
  await clock.advance(0)
}

async function band($: any, surface: RenderSurface, props: Record<string, unknown> = {}) {
  const ui = await $.ui.mount({ ...BAND, surface, props: { ...BAND.props, ...props } })
  const texts = (await ui.findAll({ type: 'Text' })).map((t: { text: string }) => t.text).join('')
  const sit = await ui.find({ key: 'sit' })
  return { ui, texts, sit }
}

async function bandText($: any, surface: RenderSurface, props: Record<string, unknown> = {}): Promise<string> {
  const { ui, texts } = await band($, surface, props)
  await ui.unmount()
  return texts
}

const wi = ($: any, args: string) => $.command.run({ command: 'wi', args })

for (const surface of SURFACES) {
  test(`band on ${surface}: N open, M one-way, K defaults, and Sit`, async ($, on) => {
    const w = world(on, { surfaces: [surface] })
    await start($, surface)
    const { ui, texts, sit } = await band($, surface)
    expect(texts).toBe('Weigh-In · 3 open here · 2 one-way · 2 defaults  ')
    expect(sit?.text).toContain('Sit')
    expect((await ui.find({ key: 'board' }))?.text).toContain('Board')
    await ui.press({ key: 'sit' })
    expect(w.submits).toHaveLength(1)
    expect(w.submits[0]).toMatchObject({ command: 'weigh-in', args: '' })
    await ui.unmount()
    // One refresh: both documents, with the skill's `W` globals, through this plugin's launcher.
    expect(w.runs).toHaveLength(2)
    for (const run of w.runs) {
      expect(run[0]).toMatch(/\/libexec\/weighin$/)
      expect(run.slice(1, 5)).toEqual(['--session', 'test-session', '--dir', '/w'])
    }
    expect(w.runs.map(r => r.slice(5, 8))).toEqual([['open', '--repo', '--json'], ['digest', '--json']])
    expect(w.runs[0]?.slice(8, 9)).toEqual(['--top'])
    expect(w.commands).toEqual(['wi'])
    // The start clears a status entry a previous load of the module left up, and asks for none.
    expect(w.status).toEqual([undefined])
  })

  test(`band on ${surface}: hidden at 0`, async ($, on) => {
    const w = world(on, { surfaces: [surface], open: '[]', digest: DIGEST([dflt('wi-00000b', true)]) })
    await start($, surface)
    const { ui, texts, sit } = await band($, surface)
    expect(texts).toBe('')
    expect(sit).toBeUndefined()
    await ui.unmount()
    expect(w.status).toEqual([undefined])
  })

  test(`band on ${surface}: defaults alone still show`, async ($, on) => {
    world(on, { surfaces: [surface], open: '[]', digest: DIGEST([dflt('wi-000009')]) })
    await start($, surface)
    expect(await bandText($, surface)).toBe('Weigh-In · 0 open here · 0 one-way · 1 defaults  ')
  })

  test(`band on ${surface}: narrow, the short line; too narrow, the status entry instead`, async ($, on) => {
    const w = world(on, { surfaces: [surface] })
    await start($, surface)
    expect(await bandText($, surface, { bodyColumns: 60 })).toBe('Weigh-In · 3 open · 2 1-way · 2 dflt  ')
    expect(w.status).toEqual([undefined])
    expect(await bandText($, surface, { bodyColumns: 30 })).toBe('')
    expect(w.status).toEqual([undefined, 'Weigh-In · 3 open here · 2 one-way · 2 defaults'])
    expect(await bandText($, surface)).toBe('Weigh-In · 3 open here · 2 one-way · 2 defaults  ')
    expect(w.status).toEqual([undefined, 'Weigh-In · 3 open here · 2 one-way · 2 defaults', undefined])
  })

  test(`band on ${surface}: a survey holds the band, so the counts go to the status entry`, async ($, on) => {
    const w = world(on, { surfaces: [surface] })
    await start($, surface)
    expect(await bandText($, surface, { hasSurvey: true })).toBe('')
    expect(w.status).toEqual([undefined, 'Weigh-In · 3 open here · 2 one-way · 2 defaults'])
  })

  test(`band on ${surface}: a failing weighin draws one line with its error`, async ($, on) => {
    world(on, { surfaces: [surface], answer: () => ({ exitCode: 1, stderr: "weighin: loading this folder's config: bad json\n" }) })
    await start($, surface)
    expect(await bandText($, surface)).toBe("Weigh-In: ledger unavailable: loading this folder's config: bad json")
  })

  test(`band on ${surface}: a spawn that cannot start draws one line`, async ($, on) => {
    world(on, { surfaces: [surface], answer: () => new Error('spawn EACCES') })
    await start($, surface)
    expect(await bandText($, surface)).toContain('Weigh-In: weighin did not start')
  })

  test(`band on ${surface}: output that does not parse draws one line`, async ($, on) => {
    world(on, { surfaces: [surface], open: '{ not json' })
    await start($, surface)
    expect(await bandText($, surface)).toBe('Weigh-In: ledger unavailable: weighin printed what does not parse')
  })

  test(`band on ${surface}: no binary says capture is off, as B-11 does`, async ($, on) => {
    world(on, {
      surfaces: [surface],
      answer: () => ({ exitCode: 127, stderr: 'weighin: this copy has no binaries: it is the development tree. (/p/libexec)\n' }),
    })
    await start($, surface)
    expect(await bandText($, surface)).toBe('Weigh-In: capture is off: this copy has no binaries: it is the development tree. (/p/libexec)')
  })
}

test('a turn ending refreshes once; the same turn again, and a subagent turn, do not', async ($, on) => {
  const clock = mock.clock(on)
  let open = OPEN3
  const w = world(on, { answer: argv => (argv.includes('digest') ? { stdout: DIGEST() } : { stdout: open }) })
  await start($)
  expect(w.runs).toHaveLength(2)
  expect(await bandText($, 'terminal')).toContain('3 open here')
  open = JSON.stringify([item('wi-000001', 'one-way')])
  await turnEnd($, clock, 't1')
  expect(w.runs).toHaveLength(4)
  expect(await bandText($, 'terminal')).toContain('1 open here · 1 one-way · 0 defaults')
  await turnEnd($, clock, 't1')
  await turnEnd($, clock, 's1', 'agent-1')
  expect(w.runs).toHaveLength(4)
  await turnEnd($, clock, 't2')
  expect(w.runs).toHaveLength(6)
})

test('WEIGHIN_MOD=0: nothing registered, run or drawn', async ($, on) => {
  const clock = mock.clock(on)
  const w = world(on, { env: { WEIGHIN_MOD: '0' } })
  await start($)
  await turnEnd($, clock)
  expect(w.runs).toHaveLength(0)
  // Only the clear of a status entry a previous load may have left up.
  expect(w.status).toEqual([undefined])
  expect(w.commands).toEqual([])
  expect(await bandText($, 'terminal')).toBe('')
})

for (const surfaces of [[], ['vscode'], ['mobile']] as RenderSurface[][]) {
  test(`where only [${surfaces.join(', ')}] draws: nothing registered or run`, async ($, on) => {
    const clock = mock.clock(on)
    const w = world(on, { surfaces })
    await start($, surfaces[0] ?? null)
    await turnEnd($, clock)
    expect(w.runs).toHaveLength(0)
    expect(w.commands).toEqual([])
  })
}

test('a desktop client attaching after the start registers /wi and draws the band', async ($, on) => {
  const clock = mock.clock(on)
  let surfaces: RenderSurface[] = []
  const runs: string[][] = []
  const commands: string[] = []
  on('process.run', async (_$, e) => {
    runs.push([...e.argv])
    const stdout = e.argv.includes('digest') ? DIGEST() : OPEN3
    return { value: { exitCode: 0, stdout, stderr: '', isStdoutTruncated: false, isStderrTruncated: false } }
  })
  on('session.surfaces', async () => ({ value: surfaces }) as any)
  on('session.id', async () => ({ value: 'test-session' }) as any)
  on('session.root', async () => ({ value: '/w' }) as any)
  on('command.register', async (_$, e: any) => { commands.push(e.name); return { value: undefined } as any })
  on('session.start', async (_$, e) => ({ cwd: e.cwd }))
  on('session.attach', async (_$, e) => ({ clientId: e.clientId }) as any)
  on('ui.render', { component: 'AbovePrompt' }, async () => h('Box', {}) as any)
  mock.env(on, {})
  await start($, null)
  expect(runs).toHaveLength(0)
  surfaces = ['desktop']
  await $.session.attach({ surface: 'desktop', clientId: 'desktop:default' })
  await clock.advance(0)
  expect(commands).toEqual(['wi'])
  expect(runs).toHaveLength(2)
  expect(await bandText($, 'desktop')).toContain('3 open here')
})

test('on Windows the launcher is libexec/weighin.exe, else libexec/weighin.cmd', async ($, on) => {
  let hasExe = true
  const w = world(on, { env: { OS: 'Windows_NT' } })
  on('fs.stat', async (_$, e: any) => {
    if (hasExe && String(e.path).endsWith('/libexec/weighin.exe')) return { value: { kind: 'file', size: 1, mtimeMs: 0, isLink: false } } as any
    throw new Error('ENOENT')
  })
  await start($)
  expect(w.runs[0]?.[0]).toMatch(/\/libexec\/weighin\.exe$/)
  hasExe = false
  await wi($, 'stats')
  expect(w.runs.at(-1)?.[0]).toMatch(/\/libexec\/weighin\.cmd$/)
})

// `/wi`: each command runs the CLI by argv, with no shell and no Claude turn, and answers with
// the CLI's own text; a failure answers with its error text and exit code.
const COMMANDS: [string, string, string[]][] = [
  ['open', 'open', ['open', '--repo']],
  ['stats', 'stats', ['stats']],
  ['decide, plain', 'decide wi-562c91 A', ['decide', '--', 'wi-562c91', 'A']],
  ['decide with a note', 'decide wi-562c91 B keep it short; "quoted" $(not run)', ['decide', '--note=keep it short; "quoted" $(not run)', '--', 'wi-562c91', 'B']],
  ['decide as the board copies it', 'decide wi-562c91: A (note: short ttl (for now))', ['decide', '--note=short ttl (for now)', '--', 'wi-562c91', 'A']],
  // Typed text that starts with `-` stays text: the positionals follow `--`, the note is `--note=`.
  ['decide with dashes', 'decide -x -B --all of it', ['decide', '--note=--all of it', '--', '-x', '-B']],
  [
    'defer as the board copies it',
    'defer wi-a99e1d — trigger: load test; resolve by: 2026-11-01; default: B',
    ['defer', '--flip=load test', '--trigger=load test', '--by=2026-11-01', '--default=B', '--', 'wi-a99e1d'],
  ],
  [
    'defer with a note',
    'defer wi-a99e1d — trigger: the vendor answers; resolve by: before-merge:main; default: A (note: ask Sam)',
    ['defer', '--flip=the vendor answers', '--trigger=the vendor answers', '--by=before-merge:main', '--default=A', '--reason=ask Sam', '--', 'wi-a99e1d'],
  ],
]

for (const [what, args, argv] of COMMANDS) {
  test(`/wi ${what}: runs weighin ${argv[0]} by argv and prints what it printed`, async ($, on) => {
    const w = world(on, { answer: a => (a.includes('--json') ? (a.includes('digest') ? { stdout: DIGEST() } : { stdout: '[]' }) : { stdout: `printed by ${a[5]}\n` }) })
    await start($)
    const before = w.runs.length
    const out = await wi($, args)
    expect(out).toEqual({ text: `printed by ${argv[0]}` })
    const run = w.runs[before]
    expect(run?.[0]).toMatch(/\/libexec\/weighin$/)
    expect(run?.slice(1)).toEqual(['--session', 'test-session', '--dir', '/w', ...argv])
    // A decision or deferral changes the counts: the band refreshes once after it.
    const refreshed = argv[0] === 'decide' || argv[0] === 'defer' ? 2 : 0
    expect(w.runs.length - before).toBe(1 + refreshed)
  })
}

test('/wi decide that fails returns the error text and the exit code', async ($, on) => {
  const w = world(on, {
    answer: a => (a.includes('decide') ? { exitCode: 1, stderr: 'weighin: no single item matches `wi-zzz` (try `weighin open --all`)\n' } : { stdout: a.includes('digest') ? DIGEST() : '[]' }),
  })
  await start($)
  const before = w.runs.length
  expect(await wi($, 'decide wi-zzz A')).toEqual({ text: 'weighin: no single item matches `wi-zzz` (try `weighin open --all`)', exitCode: 1 })
  expect(w.runs.length - before).toBe(1)
})

test('/wi with no binary says capture is off, with the launcher\'s exit code', async ($, on) => {
  world(on, { answer: () => ({ exitCode: 127, stderr: 'weighin: no binary for Linux-riscv64 under /p/libexec; build one\n' }) })
  await start($)
  expect(await wi($, 'stats')).toEqual({ text: 'Weigh-In: capture is off: no binary for Linux-riscv64 under /p/libexec; build one', exitCode: 127 })
})

test('/wi whose spawn cannot start says so', async ($, on) => {
  let fail = false
  world(on, { answer: a => (fail ? new Error('spawn ENOENT') : { stdout: a.includes('digest') ? DIGEST() : '[]' }) })
  await start($)
  fail = true
  const out = await wi($, 'open')
  expect(out.text).toMatch(/^Weigh-In: weighin did not start: .*spawn ENOENT$/)
  expect(out.exitCode).toBe(1)
})

for (const args of ['', 'help', 'open --all', 'decide wi-1', 'defer wi-1 tomorrow', 'board now']) {
  test(`/wi ${args || '(nothing)'}: the usage, and nothing run`, async ($, on) => {
    const w = world(on)
    await start($)
    const before = w.runs.length
    const out = await wi($, args)
    expect(out.text).toContain('Usage:')
    expect(out.exitCode).toBe(2)
    expect(w.runs.length).toBe(before)
  })
}

// The board pane (#115): `/wi board` and the band's Board button open it on a fresh export; each
// action writes one Answer to `board apply --surface pane` on stdin, then reads the board again.

const APPLY = ['board', 'apply', '--surface', 'pane']
const tail = (run: string[] | undefined) => run?.slice(5)
const texts = async (ui: any) => (await ui.findAll({ type: 'Text' })).map((t: { text: string }) => t.text)

async function openPane($: any, surface: RenderSurface) {
  const out = await wi($, 'board')
  const ui = await $.ui.mount({ ...PANE, surface })
  return { out, ui }
}

/** The apply runs, with what each wrote on stdin, parsed. */
function applied(w: { runs: string[][]; stdins: (string | undefined)[] }) {
  return w.runs.flatMap((r, i) => (tail(r)?.join(' ') === APPLY.join(' ') ? [JSON.parse(w.stdins[i] ?? 'null')] : []))
}

for (const surface of SURFACES) {
  test(`pane on ${surface}: Needs you shows the first card in full, options first on a one-way door`, async ($, on) => {
    const w = world(on, { surfaces: [surface], open: OPEN_RAW })
    await start($, surface)
    const { out, ui } = await openPane($, surface)
    expect(out).toEqual({ text: 'Weigh-In: the board pane is open.' })
    expect(w.opens).toEqual([{ id: 'weigh-in-board', title: 'Weigh-In board', focus: true }])
    expect(w.runs.map(tail).filter(r => r?.[0] === 'board')).toEqual([['board', 'export', '--stdout']])
    expect((await ui.find({ key: 'tab-needs' }))?.props).toMatchObject({ label: 'Needs you (2)', variant: 'primary' })
    expect((await ui.find({ key: 'tab-defaults' }))?.props.label).toBe('Decided for you (1)')
    expect((await ui.find({ key: 'tab-settled' }))?.props.label).toBe('Settled (2)')
    const t = await texts(ui)
    for (const line of [
      'Which cache backs the session store?', 'Every request reads it.', `Door: one-way · ${DOOR_NOTE}`,
      'Blast: system · Resolve by: 2026-11-01', 'Timing: Decide now: the schema ships Friday.', 'src/a.rs:10  the cache is built here',
      'docs/design.md:4  p99 under 5 ms', 'A  Redis · a shared cache', '   Holds if: we run more than one node', '   If wrong: an extra service to run',
      'Recommendation: A', 'Unless: one node is enough', '   Fails a MUST: p99 under 5 ms',
    ]) expect(t).toContain(line)
    expect(t.indexOf('Options')).toBeLessThan(t.indexOf('Recommendation: A'))
    expect((await ui.find({ type: 'Code' }))?.props).toMatchObject({ source: 'let cache = Cache::new();\ncache.warm();', startLine: 9, path: 'src/a.rs' })
    // No option pre-selected on a one-way door, and none to choose that fails a MUST.
    for (const o of ['A', 'B']) {
      const b = await ui.find({ key: `choose:wi-aaaaa1:${o}` })
      expect(b?.props.variant).toBeUndefined()
      expect(b?.props.autoFocus).toBeUndefined()
    }
    expect(await ui.find({ key: 'choose:wi-aaaaa1:C' })).toBeUndefined()
    // The raw item is listed, read-only: settle it in a sitting.
    expect(t).toContain('Not grounded yet: settle in a sitting (/weigh-in). Read-only here.')
    expect(t).toContain('wi-eeeee5 · one-way · Should the API version in the path? (src/api.rs:3)')
    const buttons = await ui.findAll({ type: 'Button' })
    expect(buttons.filter((b: { key?: string }) => b.key?.includes('wi-eeeee5'))).toEqual([])
    await ui.unmount()
  })

  test(`pane on ${surface}: a two-way card with an earned pick pre-selects it and leads with it`, async ($, on) => {
    world(on, { surfaces: [surface], open: OPEN_RAW })
    await start($, surface)
    const { ui } = await openPane($, surface)
    await ui.press({ key: 'pick:wi-bbbbb2' })
    const t = await texts(ui)
    expect(t).toContain('Tabs or spaces in the generated config?')
    expect(t.indexOf('Recommendation: A')).toBeLessThan(t.indexOf('Options'))
    expect((await ui.find({ key: 'choose:wi-bbbbb2:A' }))?.props).toMatchObject({ variant: 'primary', autoFocus: true })
    expect((await ui.find({ key: 'choose:wi-bbbbb2:B' }))?.props.variant).toBeUndefined()
    await ui.unmount()
  })

  test(`pane on ${surface}: Decided for you lists the defaults with Reopen and Ack`, async ($, on) => {
    world(on, { surfaces: [surface], open: OPEN_RAW })
    await start($, surface)
    const { ui } = await openPane($, surface)
    await ui.press({ key: 'tab-defaults' })
    const t = await texts(ui)
    for (const line of ['Retry the upload?', 'wi-ddddd4 · two-way', 'Taken: Retry three times', 'Instead of: Fail fast', 'Why: two-way door the agent acted on']) {
      expect(t).toContain(line)
    }
    expect(await ui.find({ key: 'reopen:wi-ddddd4' })).toBeDefined()
    expect(await ui.find({ key: 'ack:wi-ddddd4' })).toBeDefined()
    expect(await ui.find({ key: 'choose:wi-aaaaa1:A' })).toBeUndefined()
    await ui.unmount()
  })

  test(`pane on ${surface}: Settled lists decided cards and earlier decisions, read-only`, async ($, on) => {
    world(on, { surfaces: [surface], open: OPEN_RAW })
    await start($, surface)
    const { ui } = await openPane($, surface)
    await ui.press({ key: 'tab-settled' })
    const t = await texts(ui)
    expect(t).toContain('Which port? → B Tabs wi-ccccc3 · user-directed · 2026-10-18T09:00:00Z')
    expect(t).toContain('Which licence? → MIT OR Apache-2.0 kb-0007 · user-approved · 2026-09-01')
    expect((await ui.findAll({ type: 'Button' })).map((b: { key?: string }) => b.key)).toEqual(['tab-needs', 'tab-defaults', 'tab-settled'])
    await ui.unmount()
  })
}

const OK = (line: string) => ({ stdout: `✓ ${line}\napplied: wi-x\n` })

/** A world whose `board apply` and `batch` answer `write`; reads answer as `world` does. */
function writing(on: On, write: (argv: string[]) => Out, opts: World = {}) {
  return world(on, {
    open: OPEN_RAW,
    ...opts,
    answer: argv => {
      if (argv.includes('apply') || argv.includes('batch')) return write(argv)
      if (argv.includes('export')) return opts.board ?? { stdout: BOARD() }
      if (argv.includes('digest')) return { stdout: DIGEST() }
      return { stdout: OPEN_RAW }
    },
  })
}

test('pane: an option button writes a choose Answer with its note and card_rev, then reads the board again', async ($, on) => {
  const w = writing(on, () => OK('decided wi-aaaaa1 → B (override)'))
  await start($)
  const { ui } = await openPane($, 'terminal')
  await ui.input({ key: 'note:wi-aaaaa1', text: 'one node for now', kind: 'change' })
  const before = w.runs.length
  await ui.press({ key: 'choose:wi-aaaaa1:B' })
  const runs = w.runs.slice(before).map(tail)
  expect(runs[0]).toEqual(APPLY)
  expect(w.runs[before]?.slice(1, 5)).toEqual(['--session', 'test-session', '--dir', '/w'])
  expect(w.stdins[before]).toBe('{"item_id":"wi-aaaaa1","action":"choose","option":"B","card_rev":2,"note":"one node for now"}')
  // Then the pane's two documents, and the band's.
  expect(runs.slice(1).map(r => r?.slice(0, 2).join(' ')).sort()).toEqual(['board export', 'digest --json', 'open --repo', 'open --repo'])
  expect(await texts(ui)).toContain('✓ decided wi-aaaaa1 → B (override)')
  expect((await ui.find({ type: 'Text', text: '✓ decided wi-aaaaa1' }))?.props.color).toBe('success')
  await ui.unmount()
})

test('pane: the defer form writes a defer Answer with its trigger, resolve-by and default', async ($, on) => {
  const w = writing(on, () => OK('deferred wi-aaaaa1'))
  await start($)
  const { ui } = await openPane($, 'terminal')
  await ui.press({ key: 'defer:wi-aaaaa1' })
  // A one-way door: no default is pre-selected, as no option is.
  const pick = await ui.find({ key: 'default:wi-aaaaa1' })
  expect(pick?.props).toMatchObject({ options: [{ value: 'A', label: 'A: Redis' }, { value: 'B', label: 'B: In-process LRU' }] })
  expect(pick?.props.value).toBeUndefined()
  await ui.input({ key: 'trigger:wi-aaaaa1', text: 'the load test reports', kind: 'change' })
  await ui.input({ key: 'by:wi-aaaaa1', text: '2026-11-01' })
  await ui.select({ key: 'default:wi-aaaaa1', value: 'B' })
  await ui.press({ key: 'defer-send:wi-aaaaa1' })
  expect(applied(w)).toEqual([{ item_id: 'wi-aaaaa1', action: 'defer', defer: { trigger: 'the load test reports', resolve_by: '2026-11-01', default: 'B' }, card_rev: 2 }])
  expect(w.stdins.find(s => s?.includes('"defer"'))).toBe('{"item_id":"wi-aaaaa1","action":"defer","defer":{"trigger":"the load test reports","resolve_by":"2026-11-01","default":"B"},"card_rev":2}')
  // A settled item's fields and form are dropped.
  expect(await ui.find({ key: 'trigger:wi-aaaaa1' })).toBeUndefined()
  await ui.unmount()
})

test('pane: a deferral with a part missing says so and writes nothing', async ($, on) => {
  const w = writing(on, () => OK('deferred'))
  await start($)
  const { ui } = await openPane($, 'terminal')
  await ui.press({ key: 'defer:wi-aaaaa1' })
  await ui.input({ key: 'trigger:wi-aaaaa1', text: 'the load test reports', kind: 'change' })
  await ui.press({ key: 'defer-send:wi-aaaaa1' })
  expect(applied(w)).toEqual([])
  // On a one-way door the default is not pre-selected, so it is missing too.
  expect(await texts(ui)).toContain('To defer, give a resolve-by and a default.')
  await ui.select({ key: 'default:wi-aaaaa1', value: 'A' })
  await ui.press({ key: 'defer-send:wi-aaaaa1' })
  expect(await texts(ui)).toContain('To defer, give a resolve-by.')
  expect(applied(w)).toEqual([])
  await ui.unmount()
})

test('pane: dismiss writes a dismiss Answer with the reason picked', async ($, on) => {
  const w = writing(on, () => OK('dismissed wi-aaaaa1'))
  await start($)
  const { ui } = await openPane($, 'terminal')
  await ui.press({ key: 'dismiss:wi-aaaaa1' })
  expect((await ui.find({ key: 'reason:wi-aaaaa1' }))?.props).toMatchObject({ value: 'not-useful' })
  await ui.select({ key: 'reason:wi-aaaaa1', value: 'moot' })
  await ui.press({ key: 'dismiss-send:wi-aaaaa1' })
  expect(w.stdins.filter(s => s !== undefined)).toEqual(['{"item_id":"wi-aaaaa1","action":"dismiss","reason":"moot"}'])
  await ui.unmount()
})

test('pane: Reopen on a default writes a reopen Answer with its note', async ($, on) => {
  const w = writing(on, () => OK('reopened wi-ddddd4'))
  await start($)
  const { ui } = await openPane($, 'terminal')
  await ui.press({ key: 'tab-defaults' })
  await ui.input({ key: 'note:wi-ddddd4', text: 'fail fast instead', kind: 'change' })
  await ui.press({ key: 'reopen:wi-ddddd4' })
  expect(w.stdins.filter(s => s !== undefined)).toEqual(['{"item_id":"wi-ddddd4","action":"reopen","note":"fail fast instead"}'])
  expect(w.runs.filter(r => r.includes('apply')).map(tail)).toEqual([APPLY])
  await ui.unmount()
})

test('pane: Ack on a default acknowledges it through batch on stdin (the exchange format has no ack answer)', async ($, on) => {
  const w = writing(on, () => ({ stdout: '✓ acknowledged default on wi-ddddd4\n' }))
  await start($)
  const { ui } = await openPane($, 'terminal')
  await ui.press({ key: 'tab-defaults' })
  const before = w.runs.length
  await ui.press({ key: 'ack:wi-ddddd4' })
  expect(tail(w.runs[before])).toEqual(['batch'])
  expect(w.stdins[before]).toBe('[{"op":"ack","id":"wi-ddddd4"}]')
  expect(await texts(ui)).toContain('✓ acknowledged default on wi-ddddd4')
  await ui.unmount()
})

test('pane: a refused apply shows the CLI\'s own refusal, and the board is read again', async ($, on) => {
  const w = writing(on, () => ({ exitCode: 1, stdout: '✗ wi-aaaaa1: wi-aaaaa1 is not open (status decided)\napplied: none\n', stderr: 'weighin: 1 answer(s) from the pane refused\n' }))
  await start($)
  const { ui } = await openPane($, 'terminal')
  const before = w.runs.length
  await ui.press({ key: 'choose:wi-aaaaa1:A' })
  expect(await texts(ui)).toContain('✗ wi-aaaaa1: wi-aaaaa1 is not open (status decided)')
  expect((await ui.find({ type: 'Text', text: '✗ wi-aaaaa1' }))?.props.color).toBe('error')
  expect(w.runs.slice(before).map(tail).some(r => r?.[1] === 'export')).toBe(true)
  await ui.unmount()
})

test('pane: a stale card\'s refusal passes through as the CLI printed it', async ($, on) => {
  const stale = '✗ wi-aaaaa1: wi-aaaaa1 was re-grounded after this answer was given (card Some(3), answer Some(2)); ask again'
  writing(on, () => ({ exitCode: 1, stdout: `${stale}\napplied: none\n`, stderr: 'weighin: 1 answer(s) from the pane refused\n' }))
  await start($)
  const { ui } = await openPane($, 'terminal')
  await ui.press({ key: 'choose:wi-aaaaa1:A' })
  expect(await texts(ui)).toContain(stale)
  expect((await ui.find({ type: 'Text', text: stale }))?.props.color).toBe('error')
  await ui.unmount()
})

test('pane: a refusal with no receipt (board mode off since) shows the CLI\'s error line', async ($, on) => {
  writing(on, () => ({ exitCode: 1, stderr: `weighin: ${OFF_LINE}\n` }))
  await start($)
  const { ui } = await openPane($, 'terminal')
  await ui.press({ key: 'choose:wi-aaaaa1:A' })
  expect(await texts(ui)).toContain(OFF_LINE)
  expect((await ui.find({ type: 'Text', text: OFF_LINE }))?.props.color).toBe('error')
  await ui.unmount()
})

test('pane: board mode off shows the export\'s one line and nothing else', async ($, on) => {
  world(on, { open: OPEN_RAW, board: { exitCode: 1, stderr: `weighin: ${OFF_LINE}\n` } })
  await start($)
  const { ui } = await openPane($, 'terminal')
  expect(await texts(ui)).toEqual([OFF_LINE])
  expect(await ui.findAll({ type: 'Button' })).toEqual([])
  await ui.unmount()
})

test('pane: an empty board says nothing needs you', async ($, on) => {
  world(on, { open: '[]', board: { stdout: BOARD({ items: [], decided: [], defaults: [], defaults_total: 0, settled: [] }) } })
  await start($)
  const { ui } = await openPane($, 'terminal')
  expect((await ui.find({ key: 'tab-needs' }))?.props.label).toBe('Needs you (0)')
  expect(await texts(ui)).toEqual(['Nothing needs you.'])
  await ui.press({ key: 'tab-defaults' })
  expect(await texts(ui)).toEqual(['No default waits for you.'])
  await ui.unmount()
})

test('pane: where it cannot be placed, /wi board answers with the board as text', async ($, on) => {
  const w = world(on, { open: OPEN_RAW, isPlaced: false })
  await start($)
  const out = await wi($, 'board')
  expect(out.text).toStartWith('Weigh-In: the board pane cannot open here (no surface attached places panes).')
  for (const line of [
    'Needs you (2):', '  wi-aaaaa1 [one-way] Which cache backs the session store?', '    A Redis · B In-process LRU · C No cache (fails a MUST)',
    '    A Spaces · B Tabs · recommended A', 'Decided for you (1):', '  wi-ddddd4 Retry the upload?: took Retry three times',
    '  wi-eeeee5 [one-way] Should the API version in the path?',
  ]) expect(out.text.split('\n')).toContain(line)
  expect(out.exitCode).toBeUndefined()
  expect(w.opens).toHaveLength(1)
})

for (const surface of SURFACES) {
  test(`band on ${surface}: Board opens the pane; where it cannot be placed, the board goes to the transcript`, async ($, on) => {
    let placed = true
    const w = world(on, { surfaces: [surface], open: OPEN_RAW, get isPlaced() { return placed } })
    await start($, surface)
    const { ui } = await band($, surface)
    await ui.press({ key: 'board' })
    expect(w.opens).toEqual([{ id: 'weigh-in-board', title: 'Weigh-In board', focus: true }])
    expect(w.logs).toEqual([])
    placed = false
    await ui.press({ key: 'board' })
    expect(w.logs).toHaveLength(1)
    expect(w.logs[0]).toContain('Weigh-In: the board pane cannot open here')
    expect(w.logs[0]).toContain('wi-aaaaa1 [one-way]')
    await ui.unmount()
  })
}

test('/wi board under board mode off still opens the pane, which shows the one line', async ($, on) => {
  world(on, { board: { exitCode: 1, stderr: `weighin: ${OFF_LINE}\n` }, isPlaced: false })
  await start($)
  expect((await wi($, 'board')).text).toBe(`Weigh-In: the board pane cannot open here (no surface attached places panes).\n${OFF_LINE}`)
})

test('a turn ending reads the board again only while the pane is open', async ($, on) => {
  const clock = mock.clock(on)
  const w = world(on, { open: OPEN_RAW })
  await start($)
  await turnEnd($, clock, 't1')
  expect(w.runs.filter(r => r.includes('export'))).toHaveLength(0)
  await wi($, 'board')
  expect(w.runs.filter(r => r.includes('export'))).toHaveLength(1)
  await turnEnd($, clock, 't2')
  expect(w.runs.filter(r => r.includes('export'))).toHaveLength(2)
})

// #116: after a Write, Edit or MultiEdit that succeeded, the decisions anchored at its file, in a
// toast and on the band, from one `decisions --path <file> --json`; the call's result unchanged.

const anchoredAt = (n: number) =>
  JSON.stringify(Array.from({ length: n }, (_, i) => ({
    id: `wi-00000${i}`, option: 'A', title: i === 1 ? null : 'Redis', question: `Question ${i}?`, at: `src/a.rs:${10 + i}`, line: 10 + i, via: 'pane', decided_at: '2026-10-18T09:00:00Z',
  })))

const edit = ($: any, tool = 'Edit', extra: Record<string, unknown> = {}) =>
  $.tool.call({ tool, tool_use_id: 'tu-1', file_path: '/w/src/a.rs', old_string: 'a', new_string: 'b', ...extra })

const decisionsRuns = (w: { runs: string[][] }) => w.runs.filter(r => r.includes('decisions')).map(tail)

for (const tool of ['Write', 'Edit', 'MultiEdit']) {
  test(`a ${tool} of a file with decisions anchored there shows a toast and a band line`, async ($, on) => {
    const clock = mock.clock(on)
    const w = world(on, { decisions: anchoredAt(1) })
    await start($)
    const ran = await edit($, tool)
    expect(ran).toMatchObject({ result: { type: 'update', filePath: '/w/src/a.rs' }, text: 'The file was updated.' })
    await clock.advance(0)
    expect(decisionsRuns(w)).toEqual([['decisions', '--path', '/w/src/a.rs', '--json']])
    expect(w.toasts).toEqual(['Weigh-In: a decision stands here\nwi-000000 decided A "Redis": Question 0? (src/a.rs:10)'])
    const { ui, texts: t } = await band($, 'terminal')
    expect(t).toContain('Decided here: wi-000000 decided A "Redis": Question 0? (src/a.rs:10)')
    await ui.unmount()
  })
}

test('an edit lists at most three decisions, as the model sees, from one spawn', async ($, on) => {
  const clock = mock.clock(on)
  const w = world(on, { decisions: anchoredAt(5) })
  await start($)
  await edit($)
  await clock.advance(0)
  expect(decisionsRuns(w)).toHaveLength(1)
  expect(w.toasts).toEqual([
    'Weigh-In: 3 decisions stand here\nwi-000000 decided A "Redis": Question 0? (src/a.rs:10)\nwi-000001 decided A: Question 1? (src/a.rs:11)\nwi-000002 decided A "Redis": Question 2? (src/a.rs:12)',
  ])
  expect(await bandText($, 'terminal')).toContain('Decided here: wi-000000 decided A "Redis": Question 0? (src/a.rs:10) (+2 more)')
})

test('an edit with no decision anchored at its file shows no toast, from one spawn', async ($, on) => {
  const clock = mock.clock(on)
  const w = world(on, { decisions: '[]' })
  await start($)
  await edit($)
  await clock.advance(0)
  expect(decisionsRuns(w)).toHaveLength(1)
  expect(w.toasts).toEqual([])
  expect(await bandText($, 'terminal')).not.toContain('Decided here')
})

test('a failed edit runs nothing and shows nothing, and its result is returned unchanged', async ($, on) => {
  const clock = mock.clock(on)
  const w = world(on, { decisions: anchoredAt(1) })
  // A settings hook refusing the call: the plugins' tool.call hooks see it errored, as in a session.
  on('classic.PreToolUse', async () => ({ deny: 'String to replace not found in file.' }))
  await start($)
  const ran = await edit($)
  await clock.advance(0)
  expect(ran).toMatchObject({ isError: true, text: expect.stringContaining('String to replace not found in file.') })
  expect(w.tools).toEqual([])
  expect(decisionsRuns(w)).toEqual([])
  expect(w.toasts).toEqual([])
})

test('a refused edit runs nothing and shows nothing, and its refusal is returned unchanged', async ($, on) => {
  const clock = mock.clock(on)
  const w = world(on, { decisions: anchoredAt(1), tool: () => ({ deny: 'not allowed here' }) })
  await start($)
  const ran = await edit($)
  await clock.advance(0)
  expect(ran).toEqual({ deny: 'not allowed here' })
  expect(decisionsRuns(w)).toEqual([])
  expect(w.toasts).toEqual([])
})

test('a tool that edits nothing runs nothing', async ($, on) => {
  const clock = mock.clock(on)
  const w = world(on, { decisions: anchoredAt(1), tool: () => ({ result: { stdout: '' }, text: '' }) })
  await start($)
  await $.tool.call({ tool: 'Read', tool_use_id: 'tu-2', file_path: '/w/src/a.rs' } as any)
  await clock.advance(0)
  expect(decisionsRuns(w)).toEqual([])
  expect(w.tools).toHaveLength(1)
})

test('an edit under WEIGHIN_MOD=0 runs nothing', async ($, on) => {
  const clock = mock.clock(on)
  const w = world(on, { decisions: anchoredAt(1), env: { WEIGHIN_MOD: '0' } })
  await start($)
  await edit($)
  await clock.advance(0)
  expect(decisionsRuns(w)).toEqual([])
  expect(w.toasts).toEqual([])
})

test('an edit where no surface draws runs nothing', async ($, on) => {
  const clock = mock.clock(on)
  const w = world(on, { decisions: anchoredAt(1), surfaces: [] })
  await start($, null)
  await edit($)
  await clock.advance(0)
  expect(decisionsRuns(w)).toEqual([])
  expect(w.toasts).toEqual([])
})

// Review fixes (B-32).

test('a pane opened but not placed is not read again at each turn end', async ($, on) => {
  const clock = mock.clock(on)
  const w = world(on, { open: OPEN_RAW, isPlaced: false })
  await start($)
  await wi($, 'board')
  expect(w.runs.filter(r => r.includes('export'))).toHaveLength(1)
  await turnEnd($, clock, 't1')
  await turnEnd($, clock, 't2')
  expect(w.runs.filter(r => r.includes('export'))).toHaveLength(1)
})

test('an edit that lists no decision clears the band line, whichever file it touched', async ($, on) => {
  const clock = mock.clock(on)
  let listed = anchoredAt(1)
  const w = world(on, { answer: argv => (argv.includes('decisions') ? { stdout: listed } : argv.includes('digest') ? { stdout: DIGEST() } : { stdout: OPEN3 }) })
  await start($)
  await edit($)
  await clock.advance(0)
  expect(await bandText($, 'terminal')).toContain('Decided here:')
  listed = '[]'
  await edit($, 'Write', { file_path: '/w/src/other.rs', content: 'x' })
  await clock.advance(0)
  expect(decisionsRuns(w).map(r => r?.[2])).toEqual(['/w/src/a.rs', '/w/src/other.rs'])
  expect(await bandText($, 'terminal')).not.toContain('Decided here:')
})

for (const [what, out, logged] of [
  ['fails', { exitCode: 1, stderr: 'weighin: reading the ledger: disk I/O error\n' }, 'Weigh-In: decisions at edited files unavailable: reading the ledger: disk I/O error'],
  ['prints what does not parse', { stdout: '{ nope' }, 'Weigh-In: decisions at edited files unavailable: weighin printed what does not parse'],
  ['cannot start', new Error('spawn EACCES'), 'Weigh-In: weighin did not start'],
  ['has no binary', { exitCode: 127, stderr: 'weighin: no binary for Linux-riscv64\n' }, null],
] as const) {
  test(`when decisions --path ${what}, the session is told once${logged === null ? ' (no binary: not at all, the band says capture is off)' : ''}`, async ($, on) => {
    const clock = mock.clock(on)
    const w = world(on, { answer: argv => (argv.includes('decisions') ? out : argv.includes('digest') ? { stdout: DIGEST() } : { stdout: OPEN3 }) })
    await start($)
    await edit($)
    await clock.advance(0)
    await edit($)
    await clock.advance(0)
    expect(decisionsRuns(w)).toHaveLength(2)
    expect(w.toasts).toEqual([])
    if (logged === null) expect(w.logs).toEqual([])
    else {
      expect(w.logs).toHaveLength(1)
      expect(w.logs[0]).toContain(logged)
    }
  })
}

test('a decision line is clipped as recall::Anchored::line clips it: title 60, question 120', async ($, on) => {
  const clock = mock.clock(on)
  const title = 'T'.repeat(70)
  const question = `${'Q'.repeat(60)}\n${'q'.repeat(69)}`
  world(on, { decisions: JSON.stringify([{ id: 'wi-000001', option: 'A', title, question, at: 'src/a.rs:3', line: 3, via: 'chat', decided_at: '' }]) })
  await start($)
  await edit($)
  await clock.advance(0)
  const { ui } = await band($, 'terminal')
  const row = (await ui.find({ type: 'Text', text: 'Decided here:' }))?.text
  expect(row).toBe(`Decided here: wi-000001 decided A "${'T'.repeat(59)}…": ${'Q'.repeat(60)} ${'q'.repeat(58)}… (src/a.rs:3)`)
  await ui.unmount()
})

test('pane: a second press while an answer is being written is ignored, not sent to be refused', async ($, on) => {
  const clock = mock.clock(on)
  const w = writing(on, () => OK('decided wi-aaaaa1 → A'), { wait: argv => (argv.includes('apply') ? clock.sleep(50) : undefined) })
  await start($)
  const { ui } = await openPane($, 'terminal')
  const first = $.ui.press({ plugin: 'weigh-mod', key: 'choose:wi-aaaaa1:A' })
  await clock.settle()
  const second = $.ui.press({ plugin: 'weigh-mod', key: 'choose:wi-aaaaa1:B' })
  await clock.settle()
  await clock.advance(50)
  await Promise.all([first, second])
  expect(applied(w)).toEqual([{ item_id: 'wi-aaaaa1', action: 'choose', option: 'A', card_rev: 2 }])
  expect(await texts(ui)).toContain('✓ decided wi-aaaaa1 → A')
  // Once settled, a press writes again.
  const third = $.ui.press({ plugin: 'weigh-mod', key: 'choose:wi-aaaaa1:B' })
  await clock.settle()
  await clock.advance(50)
  await third
  expect(applied(w)).toHaveLength(2)
  await ui.unmount()
})

test('pane: a write that timed out says it may have been applied, and the board is read again', async ($, on) => {
  const w = writing(on, () => new Error('timed out after 30000 ms') as any)
  await start($)
  const { ui } = await openPane($, 'terminal')
  const before = w.runs.length
  await ui.press({ key: 'choose:wi-aaaaa1:A' })
  expect(await texts(ui)).toContain('Weigh-In: weighin timed out; the answer may have been applied')
  expect(w.runs.slice(before).map(tail).some(r => r?.[1] === 'export')).toBe(true)
  await ui.unmount()
})

test('/wi decide that timed out says it may have been applied, and the band is read again', async ($, on) => {
  const w = world(on, { answer: a => (a.includes('decide') ? new Error('process timed out') : { stdout: a.includes('digest') ? DIGEST() : '[]' }) })
  await start($)
  const before = w.runs.length
  expect(await wi($, 'decide wi-1 A')).toEqual({ text: 'Weigh-In: weighin timed out; the answer may have been applied', exitCode: 1 })
  expect(w.runs.length - before).toBe(3)
})

test('pane: the defer form pre-selects the default where the card pre-selects its pick', async ($, on) => {
  world(on, { open: OPEN_RAW })
  await start($)
  const { ui } = await openPane($, 'terminal')
  await ui.press({ key: 'pick:wi-bbbbb2' })
  await ui.press({ key: 'defer:wi-bbbbb2' })
  expect((await ui.find({ key: 'default:wi-bbbbb2' }))?.props.value).toBe('A')
  await ui.unmount()
})

test('pane: a card whose every option fails a MUST has no Defer… and no option to choose', async ($, on) => {
  const blocked = { ...card1, options: card1.options.map(o => ({ ...o, fails_must: 'p99 under 5 ms' })) }
  world(on, { open: OPEN_RAW, board: { stdout: BOARD({ items: [blocked] }) } })
  await start($)
  const { ui } = await openPane($, 'terminal')
  const keys = (await ui.findAll({ type: 'Button' })).map((b: { key?: string }) => b.key)
  expect(keys).toEqual(['tab-needs', 'tab-defaults', 'tab-settled', 'dismiss:wi-aaaaa1'])
  await ui.unmount()
})
