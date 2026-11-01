// Weigh-In mod: the values its drawing reads from `$.state` (the plugin's contract, #114-#116).
// They are read from `weighin open --repo --json`, `weighin digest --json`, `weighin board export
// --stdout` and `weighin decisions --path <file> --json`; the mod never opens the ledger itself.

/** The ledger's counts for this repository, as the band shows them. */
export type WeighCounts = {
  /** Items open in this repository (`open --repo`), deferred ones included. */
  open: number
  /** Of those, the ones whose door is one-way (as triaged, else as raised). */
  oneWay: number
  /** Defaults taken without the owner and not yet acknowledged (`digest`). */
  defaults: number
}

/**
 * What the band draws: the counts; `no-binary` when the launcher found no weighin build for
 * this platform (B-11's "capture is off"); `unavailable` when weighin ran and failed, or printed
 * what does not parse. `null` before the first refresh: the band then draws nothing.
 */
export type WeighView =
  | { kind: 'counts'; counts: WeighCounts }
  | { kind: 'no-binary'; line: string }
  | { kind: 'unavailable'; line: string }

/** One option on a card, as the board state document carries it (`weighin_core::card`). */
export type WeighOption = {
  id: string
  title: string
  summary?: string
  holds_if?: string
  if_wrong?: string
  /** The MUST it fails: shown struck through, never chosen. */
  fails_must?: string
  evasion?: string
}

/** One card of `board export --stdout` (`BoardItem`): field names are the board page's. */
export type WeighCard = {
  id: string
  rank: number
  question: string
  why: string
  door: string
  door_note?: string
  blast: string
  authority: string
  resolve_by: string
  blocks?: string[]
  timing: string
  source: { path: string; quote: string }
  evidence: { src: string; text: string }[]
  excerpt?: { path: string; start: number; lines: string[] }
  options: WeighOption[]
  recommendation: { option: string; flip?: string; tradeoff?: string; why_wrong?: string }
  /** `recommended`, or `options-first` (a one-way door, or a recommendation not earned). */
  present: string
  card_rev?: number
  decision?: { option: string; decided_at: string; provenance: string; matched_recommendation: boolean; ledger_note?: string }
}

/** A default taken without the owner (`BoardDefault`). */
export type WeighDefault = { id: string; question: string; taken: string; alt: string; reason: string; cause: string }

/** An earlier decision (`BoardSettled`). */
export type WeighSettled = { id: string; question: string; answer: string; decided_at: string; provenance: string }

/** The board state document (`BoardState`, schema 1), the board exchange format. */
export type WeighBoardState = {
  schema: number
  scope: { repo: string; session: string; source: string; generated_at: string; board_mode: string }
  items: WeighCard[]
  decided: WeighCard[]
  defaults: WeighDefault[]
  defaults_total: number
  settled: WeighSettled[]
}

/** An open item no sitting has grounded, from `open --repo --json`: read-only in the pane. */
export type WeighRawItem = { id: string; question: string; door: string; at: string | null }

/**
 * What the board pane draws: the board and the raw items; `refused` when `board export
 * --stdout` refused (board mode `off`, no config), with its one line; `unavailable` when weighin
 * did not start, failed otherwise or printed what does not parse.
 */
export type WeighPane =
  | { kind: 'board'; board: WeighBoardState; raw: WeighRawItem[] }
  | { kind: 'refused'; line: string }
  | { kind: 'unavailable'; line: string }

/** The pane's tabs: Needs you, Decided for you, Settled. */
export type WeighTab = 'needs' | 'defaults' | 'settled'

/** The form open under a card or a default, when one is. */
export type WeighForm = { id: string; kind: 'defer' | 'dismiss' }

/** What the last action in the pane printed: the CLI's receipt, or its refusal. */
export type WeighReceipt = { isRefused: boolean; text: string }

/** The decisions anchored at the file an edit just touched (#116), as the band shows them. */
export type WeighAnchored = { file: string; lines: string[] }

declare module 'claude-code' {
  interface PluginState {
    'weigh-mod': {
      view: WeighView | null
      pane: WeighPane | null
      tab: WeighTab
      /** The card shown in full under Needs you; the first when null or gone. */
      selected: string | null
      form: WeighForm | null
      /** What the pane's fields hold, by field key (`note:<id>`, `trigger:<id>`, ...). */
      drafts: Record<string, string>
      receipt: WeighReceipt | null
      /** The item whose answer is being written, while one is: other writes wait for it. */
      busy: string | null
      anchored: WeighAnchored | null
    }
  }
}
