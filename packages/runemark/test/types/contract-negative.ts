import {
  type ColorMode,
  type ConsoleOptions,
  type DetailLevel,
  type DiffInput,
  type FindingInput,
  type LocationInput,
  type MetricInput,
  type ProgressMode,
  type ProgressOptions,
  type ReportInput,
  RunemarkConsole,
  RunemarkProgress,
  type SymbolTheme,
  type Tone,
  type Verdict,
} from '../..'

// @ts-expect-error Invalid color mode literal
const _invalidColor: ColorMode = 'rainbow'

// @ts-expect-error Invalid symbol theme literal
const _invalidSymbols: SymbolTheme = 'emoji'

// @ts-expect-error Invalid progress mode literal
const _invalidProgressMode: ProgressMode = 'sometimes'

// @ts-expect-error Invalid detail level literal
const _invalidDetailLevel: DetailLevel = 'extreme'

// @ts-expect-error Invalid tone literal
const _invalidTone: Tone = 'critical'

// @ts-expect-error Invalid verdict literal
const _invalidVerdict: Verdict = 'exceptional'

const _invalidFinding: FindingInput = {
  message: 'bad confidence',
  // @ts-expect-error Invalid confidence literal
  confidence: 'ultra_high',
}

const _invalidMetric: MetricInput = {
  key: 'Errors',
  value: '0',
  // @ts-expect-error Invalid metric trend literal
  trend: 'upward',
}

const _invalidLocation: LocationInput = {
  // @ts-expect-error Invalid location kind literal
  kind: 'database',
  value: 'users',
}

const _invalidDiff: DiffInput = {
  // @ts-expect-error Invalid file action literal
  changes: [{ action: 'copied', path: 'src/lib.rs' }],
}

const _invalidSchemaVersion: ReportInput = {
  // @ts-expect-error Unsupported schemaVersion literal (only 1 allowed)
  schemaVersion: 2,
  title: 'Future',
  verdict: 'passed',
}

// @ts-expect-error Missing required field 'verdict'
const _missingVerdict: ReportInput = {
  title: 'Missing verdict',
}

// @ts-expect-error Missing required field 'title'
const _missingTitle: ReportInput = {
  verdict: 'passed',
}

const _unknownProp: ReportInput = {
  title: 'Extra',
  verdict: 'passed',
  // @ts-expect-error Extra undeclared property 'unknownProp' on ReportInput
  unknownProp: 42,
}

// @ts-expect-error Progress constructor does not accept undeclared option 'total'
new RunemarkProgress({ total: 10 })

// @ts-expect-error Console constructor does not accept unknown options
new RunemarkConsole({ unknown: true })
