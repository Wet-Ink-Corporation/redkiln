// The one way the mod reaches the ledger: this plugin's own weighin launcher, run by argv with no
// shell, with the skill's `W` globals. Answers go to the CLI on stdin, never through a file.
import type { EngineInterface, ProcessRunResult } from 'claude-code'

/** The launcher's exit when it finds no binary for this platform. */
export const NO_BINARY = 127

/**
 * One run of the launcher, as the band, the pane and the commands read it: `timed-out` when the
 * child was still running at the timeout and was killed, which may be after it wrote.
 */
export type Run =
  | { kind: 'ran'; result: ProcessRunResult }
  | { kind: 'no-start'; why: string }
  | { kind: 'timed-out'; why: string }

/** The launcher for this platform: the Windows build directly, else the sh launcher (B-16). */
async function launcher($: EngineInterface): Promise<string> {
  const libexec = `${$.plugin.root}/libexec`
  if ((await $.env.get('OS')) !== 'Windows_NT') return `${libexec}/weighin`
  try {
    const exe = await $.fs.stat(`${libexec}/weighin.exe`)
    if (exe.kind === 'file') return `${libexec}/weighin.exe`
  } catch {
    // No build at libexec/weighin.exe: the cmd launcher finds one under libexec/<target>/.
  }
  return `${libexec}/weighin.cmd`
}

/**
 * Runs weighin with the skill's `W` globals: this session's key and its project directory.
 * `stdin`, when given, is written to the child's standard input and closed.
 */
export async function weighin($: EngineInterface, args: readonly string[], timeoutMs: number, stdin?: string): Promise<Run> {
  let started = 0
  try {
    const argv = [await launcher($), '--session', await $.session.id(), '--dir', await $.session.root(), ...args]
    const init = stdin === undefined ? { timeoutMs } : { timeoutMs, stdin }
    started = Date.now()
    return { kind: 'ran', result: await $.process.run(argv, init) }
  } catch (err) {
    const why = firstLine(err instanceof Error ? err.message : String(err))
    // `$.process.run` rejects both when the child cannot start and when it outlives the timeout.
    const isTimeout = /timed? ?out|timeout/i.test(why) || (started > 0 && Date.now() - started >= timeoutMs)
    return { kind: isTimeout ? 'timed-out' : 'no-start', why }
  }
}

/** What a run that did not finish says: it did not start, or it timed out. */
export function unfinished(run: Exclude<Run, { kind: 'ran' }>): string {
  return run.kind === 'timed-out' ? `Weigh-In: weighin timed out: ${run.why}` : `Weigh-In: weighin did not start: ${run.why}`
}

/** What a write that timed out says: it may have landed before the child was killed. */
export const WRITE_TIMED_OUT = 'Weigh-In: weighin timed out; the answer may have been applied'

export function firstLine(text: string): string {
  return text.trim().split('\n')[0]?.trim() ?? ''
}

/** B-11's line: the launcher's own reason, without its `weighin:` prefix. */
export function captureOff(stderr: string): string {
  const why = firstLine(stderr).replace(/^weighin:\s*/, '')
  return `Weigh-In: capture is off: ${why || 'no weighin binary for this platform.'}`
}

/** The CLI's first error line, without its `weighin:` prefix. */
export function cliError(result: ProcessRunResult): string {
  const why = firstLine(result.stderr) || firstLine(result.stdout) || `exit ${result.exitCode}`
  return why.replace(/^weighin:\s*/, '')
}
