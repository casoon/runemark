import {
  type BadgeInput,
  type ColorMode,
  type ConsoleOptions,
  type DetailLevel,
  type DiffInput,
  type ErrorBlockInput,
  type FindingGroupInput,
  type FindingInput,
  type LocationInput,
  type MetricInput,
  type NextStepInput,
  type ProgressMode,
  type ProgressOptions,
  type RenderOptions,
  type ReportInput,
  RunemarkConsole,
  RunemarkProgress,
  RunemarkReport,
  type SymbolTheme,
  Tone,
  type Tone as ToneType,
  Verdict,
  type Verdict as VerdictType,
  type Writable,
  createProgress,
  renderDiff,
  renderError,
  renderReport,
  renderStatus,
} from '../..'

// 1. Types & Enums
const colorMode: ColorMode = 'auto'
const symbolTheme: SymbolTheme = 'unicode'
const progressMode: ProgressMode = 'always'
const detailLevel: DetailLevel = 'compact'
const tone: ToneType = Tone.Info
const verdict: VerdictType = Verdict.Passed

// 2. ConsoleOptions & RenderOptions
const consoleOpts: ConsoleOptions = {
  color: colorMode,
  symbols: symbolTheme,
  isTerminal: true,
}

const renderOpts: RenderOptions = {
  ...consoleOpts,
  width: 80,
}

// 3. RunemarkConsole
const console = new RunemarkConsole(consoleOpts)
const painted: string = console.render(tone, 'message')
const verdictRendered: string = console.renderVerdict(verdict, 'all good')
const titleStr: string = console.title('Title')
const mutedStr: string = console.muted('Muted')
const infoStr: string = console.info('Info')
const successStr: string = console.success('Success')
const warningStr: string = console.warning('Warning')
const errorStr: string = console.error('Error')

const mockStream: Writable = {
  write: (_text: string) => true,
}
console.write(tone, 'msg', mockStream)
console.writeInfo('info', mockStream)
console.writeSuccess('success', mockStream)
console.writeWarning('warning', mockStream)
console.writeError('error', mockStream)

// 4. ReportInput and RunemarkReport
const location: LocationInput = {
  kind: 'file',
  value: 'src/lib.rs',
  line: 42,
  column: 10,
}

const badge: BadgeInput = {
  label: 'auto-fix',
  tone: Tone.Success,
}

const finding: FindingInput = {
  message: 'Unexpected token',
  tone: Tone.Warning,
  location,
  ruleId: 'syntax/token',
  remedy: 'Remove token',
  confidence: 'high',
  badge,
}

const group: FindingGroupInput = {
  title: 'Parser findings',
  findings: [finding],
  totalCount: 1,
  advisory: false,
}

const metric: MetricInput = {
  key: 'Coverage',
  value: '95%',
  tone: Tone.Success,
  trend: 'positive',
  delta: '+2%',
}

const nextStep: NextStepInput = {
  text: 'Run formatter',
  command: 'cargo fmt',
}

const reportInput: ReportInput = {
  schemaVersion: 1,
  title: 'Audit report',
  verdict: Verdict.Warning,
  metrics: [metric],
  groups: [group],
  nextSteps: [nextStep],
  detailLevel,
  maxCompactSamples: 3,
}

const report = new RunemarkReport(reportInput)
const renderedReport: string = report.render(renderOpts)

// 5. Progress
const progressOpts: ProgressOptions = {
  mode: progressMode,
  isTerminal: false,
}
const progress = new RunemarkProgress(progressOpts)
progress.start(10, 'Processing')
progress.advance(5, 'Halfway')
progress.notice(Tone.Info, 'Still working')
progress.finish(Verdict.Passed, 'Done')

const progressCreated = createProgress(progressOpts)
progressCreated.start(1, 'Task')

// 6. Stateless helpers
const statusOut: string = renderStatus(verdict, 'Ready', consoleOpts)
const reportOut: string = renderReport(reportInput, renderOpts)

const errorInput: ErrorBlockInput = {
  heading: 'Build failed',
  explanation: 'Missing dependency',
  remedy: 'Run install',
  commands: ['npm install'],
}
const errorOut: string = renderError(errorInput, consoleOpts)

const diffInput: DiffInput = {
  changes: [
    { action: 'added', path: 'new.txt' },
    { action: 'modified', path: 'main.rs', delta: '+1 -1' },
    { action: 'deleted', path: 'old.txt' },
    { action: 'renamed', path: 'src/renamed.rs' },
  ],
}
const diffOut: string = renderDiff(diffInput, consoleOpts)

void [
  painted,
  verdictRendered,
  titleStr,
  mutedStr,
  infoStr,
  successStr,
  warningStr,
  errorStr,
  renderedReport,
  statusOut,
  reportOut,
  errorOut,
  diffOut,
]
