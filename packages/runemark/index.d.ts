export type ColorMode = 'auto' | 'always' | 'never'
export type SymbolTheme = 'unicode' | 'ascii'
export type Tone = 'title' | 'muted' | 'info' | 'success' | 'warning' | 'error'
export type Verdict = 'passed' | 'warning' | 'failed' | 'action-required' | 'skipped' | 'info'
export type DetailLevel = 'compact' | 'detailed'
export type ProgressMode = 'auto' | 'always' | 'never'

export interface ConsoleOptions {
  color?: ColorMode
  symbols?: SymbolTheme
  isTerminal?: boolean
}

export interface RenderOptions extends ConsoleOptions {
  width?: number
}

export interface LocationInput {
  kind: 'file' | 'url' | 'selector' | 'artifact'
  value: string
  line?: number
  column?: number
}

export interface BadgeInput {
  label: string
  tone?: Tone
}

export interface FindingInput {
  message: string
  tone?: Tone
  location?: LocationInput
  ruleId?: string
  remedy?: string
  confidence?: 'high' | 'medium' | 'low'
  badge?: BadgeInput
}

export interface FindingGroupInput {
  title: string
  findings?: FindingInput[]
  totalCount?: number
  advisory?: boolean
}

export interface MetricInput {
  key: string
  value: string
  tone?: Tone
  trend?: 'positive' | 'negative' | 'neutral'
  delta?: string
}

export interface NextStepInput {
  text: string
  command?: string
}

/** Versioned plain-object contract consumed by the native binding. */
export interface ReportInput {
  schemaVersion?: 1
  title: string
  verdict: Verdict
  metrics?: MetricInput[]
  groups?: FindingGroupInput[]
  nextSteps?: NextStepInput[]
  detailLevel?: DetailLevel
  maxCompactSamples?: number
}

export interface ErrorBlockInput {
  heading: string
  explanation?: string
  remedy?: string
  commands?: string[]
}

export interface DiffInput {
  changes: Array<{
    action: 'added' | 'modified' | 'deleted' | 'renamed'
    path: string
    delta?: string
  }>
}

export interface ProgressOptions extends ConsoleOptions {
  mode?: ProgressMode
}

export interface Writable {
  write(text: string): unknown
}

export class RunemarkConsole {
  constructor(options?: ConsoleOptions)
  render(tone: Tone, message: string): string
  renderVerdict(verdict: Verdict, message: string): string
  title(message: string): string
  muted(message: string): string
  info(message: string): string
  success(message: string): string
  warning(message: string): string
  error(message: string): string
  write(tone: Tone, message: string, stream?: Writable): void
  writeInfo(message: string, stream?: Writable): void
  writeSuccess(message: string, stream?: Writable): void
  writeWarning(message: string, stream?: Writable): void
  writeError(message: string, stream?: Writable): void
}

export class RunemarkReport {
  constructor(input: ReportInput)
  render(options?: RenderOptions): string
}

export class RunemarkProgress {
  constructor(options?: ProgressOptions)
  start(total: number, message: string): void
  advance(position: number, message: string): void
  notice(tone: Tone, message: string): void
  finish(verdict: Verdict, message: string): void
}

export const Verdict: Readonly<{
  Passed: 'passed'
  Warning: 'warning'
  Failed: 'failed'
  ActionRequired: 'action-required'
  Skipped: 'skipped'
  Info: 'info'
}>

export const Tone: Readonly<{
  Title: 'title'
  Muted: 'muted'
  Info: 'info'
  Success: 'success'
  Warning: 'warning'
  Error: 'error'
}>

export function renderStatus(verdict: Verdict, message: string, options?: ConsoleOptions): string
export function renderReport(input: ReportInput, options?: RenderOptions): string
export function renderError(input: ErrorBlockInput, options?: ConsoleOptions): string
export function renderDiff(input: DiffInput, options?: ConsoleOptions): string
export function createProgress(options?: ProgressOptions): RunemarkProgress
