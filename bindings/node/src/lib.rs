//! Native Node-API boundary for the curated `@casoon/runemark` package.
//!
//! The binding accepts plain Node data objects and keeps runemark's Rust types
//! private. This lets each public DTO evolve additively without exposing Rust
//! builders, traits, or enum representation details as a JavaScript contract.

use napi::{Either, Error, Result, Status};
use napi_derive::napi;
use runemark::{
    Badge, ColorMode, Confidence, Console, DetailLevel, DiffBlock, ErrorBlock, FileAction,
    FileChange, Finding, FindingGroup, Location, NextStep, ProgressMode, ProgressSink,
    RenderOptions, Report, ScopeNote, SymbolTheme, TerminalProgress, Tone, Verdict, Vocabulary,
};

#[napi(object)]
pub struct JsConsoleOptions {
    pub color: Option<String>,
    pub symbols: Option<String>,
    pub is_terminal: Option<bool>,
    pub stream: Option<String>,
    pub hyperlinks: Option<bool>,
}

#[napi(object)]
pub struct JsRenderOptions {
    pub color: Option<String>,
    pub symbols: Option<String>,
    pub is_terminal: Option<bool>,
    pub stream: Option<String>,
    pub hyperlinks: Option<bool>,
    pub width: Option<f64>,
}

#[napi(object)]
pub struct JsMetric {
    pub key: String,
    pub value: String,
    /// One of the verdicts `render_verdict` takes. Renders that verdict's
    /// symbol in front of the metric, so a failing one still reads as failing
    /// with colour off — in a pipe or under `NO_COLOR` a tone alone is
    /// nothing, and `errors: 3` looked exactly like `errors: 0`.
    pub verdict: Option<String>,
    pub tone: Option<String>,
    pub trend: Option<String>,
    pub delta: Option<String>,
}

#[napi(object)]
pub struct JsLocation {
    pub kind: String,
    pub value: String,
    pub line: Option<f64>,
    pub column: Option<f64>,
}

#[napi(object)]
pub struct JsBadge {
    pub label: String,
    pub tone: Option<String>,
}

#[napi(object)]
pub struct JsFinding {
    pub message: String,
    pub tone: Option<String>,
    pub location: Option<JsLocation>,
    pub rule_id: Option<String>,
    pub remedy: Option<String>,
    pub confidence: Option<String>,
    pub badge: Option<JsBadge>,
}

#[napi(object)]
pub struct JsFindingGroup {
    pub title: String,
    pub findings: Option<Vec<JsFinding>>,
    pub total_count: Option<f64>,
    pub advisory: Option<bool>,
}

#[napi(object)]
pub struct JsNextStep {
    pub text: String,
    pub command: Option<String>,
}

#[napi(object)]
pub struct JsScopeNote {
    pub heading: String,
    pub items: Vec<String>,
}

/// Tagged union: `kind` selects the variant, `value` is its payload.
#[napi(object)]
pub struct JsVocabulary {
    pub kind: String,
    pub value: Option<Either<String, f64>>,
}

#[napi(object)]
pub struct JsReportInput {
    pub schema_version: Option<f64>,
    pub title: String,
    pub verdict: String,
    pub metrics: Option<Vec<JsMetric>>,
    pub groups: Option<Vec<JsFindingGroup>>,
    pub next_steps: Option<Vec<JsNextStep>>,
    pub scope_notes: Option<Vec<JsScopeNote>>,
    pub vocabulary: Option<Vec<JsVocabulary>>,
    /// A number sets the threshold, `false` disables the summary.
    pub top_issues_threshold: Option<Either<f64, bool>>,
    /// A string replaces the hint after the compact "more findings" line,
    /// `false` drops it.
    pub details_hint: Option<Either<String, bool>>,
    pub detail_level: Option<String>,
    pub max_compact_samples: Option<f64>,
}

#[napi(object)]
pub struct JsErrorBlockInput {
    pub heading: String,
    pub explanation: Option<String>,
    pub remedy: Option<String>,
    pub commands: Option<Vec<String>>,
}

#[napi(object)]
pub struct JsFileChange {
    pub action: String,
    pub path: String,
    pub delta: Option<String>,
}

#[napi(object)]
pub struct JsDiffInput {
    pub changes: Vec<JsFileChange>,
}

#[napi(object)]
pub struct JsProgressOptions {
    pub color: Option<String>,
    pub symbols: Option<String>,
    pub is_terminal: Option<bool>,
    pub mode: Option<String>,
}

/// Native state behind the public JavaScript `RunemarkConsole` facade.
#[napi(js_name = "NativeConsole")]
pub struct NativeConsole {
    console: Console,
}

#[napi]
impl NativeConsole {
    #[napi(constructor)]
    pub fn new(options: Option<JsConsoleOptions>) -> Result<Self> {
        Ok(Self {
            console: console_from_options(options)?,
        })
    }

    #[napi]
    pub fn paint(&self, tone: String, message: String) -> Result<String> {
        Ok(self.console.paint(parse_tone(&tone)?, message))
    }

    #[napi]
    pub fn render_verdict(&self, verdict: String, message: String) -> Result<String> {
        Ok(parse_verdict(&verdict)?.render(self.console, &message))
    }
}

/// Native state behind the public JavaScript `RunemarkReport` facade.
#[napi(js_name = "NativeReport")]
pub struct NativeReport {
    report: Report,
}

#[napi]
impl NativeReport {
    #[napi(constructor)]
    pub fn new(input: JsReportInput) -> Result<Self> {
        Ok(Self {
            report: report_from_input(input)?,
        })
    }

    #[napi]
    pub fn render(&self, options: Option<JsRenderOptions>) -> Result<String> {
        let (console, layout) = render_context(options)?;
        Ok(self.report.render_with_options(console, layout))
    }
}

/// Native state behind the public JavaScript `RunemarkProgress` facade.
#[napi(js_name = "NativeProgress")]
pub struct NativeProgress {
    progress: TerminalProgress,
}

#[napi]
impl NativeProgress {
    #[napi(constructor)]
    pub fn new(options: Option<JsProgressOptions>) -> Result<Self> {
        let options = options.unwrap_or(JsProgressOptions {
            color: None,
            symbols: None,
            is_terminal: None,
            mode: None,
        });
        let is_terminal = options
            .is_terminal
            .unwrap_or_else(|| std::io::IsTerminal::is_terminal(&std::io::stderr()));
        let console = console_from_parts(
            options.color.as_deref(),
            options.symbols.as_deref(),
            Some(is_terminal),
            None,
            None,
        )?;
        let mode = match options.mode.as_deref().unwrap_or("auto") {
            "auto" => ProgressMode::Auto,
            "always" => ProgressMode::Always,
            "never" => ProgressMode::Never,
            value => return Err(invalid_value("progress mode", value)),
        };

        Ok(Self {
            progress: TerminalProgress::stderr(mode, console, is_terminal),
        })
    }

    #[napi]
    pub fn start(&self, total: f64, message: String) -> Result<()> {
        self.progress
            .start(u64::from(to_u32("total", total)?), &message);
        Ok(())
    }

    #[napi]
    pub fn advance(&self, position: f64, message: String) -> Result<()> {
        self.progress
            .advance(u64::from(to_u32("position", position)?), &message);
        Ok(())
    }

    #[napi]
    pub fn notice(&self, tone: String, message: String) -> Result<()> {
        self.progress.notice(parse_tone(&tone)?, &message);
        Ok(())
    }

    #[napi]
    pub fn finish(&self, verdict: String, message: String) -> Result<()> {
        self.progress.finish(parse_verdict(&verdict)?, &message);
        Ok(())
    }
}

#[napi]
pub fn render_error(input: JsErrorBlockInput, options: Option<JsConsoleOptions>) -> Result<String> {
    let mut block = ErrorBlock::new(input.heading);
    if let Some(explanation) = input.explanation {
        block = block.with_explanation(explanation);
    }
    if let Some(remedy) = input.remedy {
        block = block.with_remedy(remedy);
    }
    for command in input.commands.unwrap_or_default() {
        block = block.add_command(command);
    }
    Ok(block.render(console_from_options(options)?))
}

#[napi]
pub fn render_diff(input: JsDiffInput, options: Option<JsConsoleOptions>) -> Result<String> {
    let mut block = DiffBlock::new();
    for change in input.changes {
        let mut file_change = FileChange::new(parse_file_action(&change.action)?, change.path);
        if let Some(delta) = change.delta {
            file_change = file_change.with_delta(delta);
        }
        block = block.add_change(file_change);
    }
    Ok(block.render(console_from_options(options)?))
}

fn console_from_options(options: Option<JsConsoleOptions>) -> Result<Console> {
    let options = options.unwrap_or(JsConsoleOptions {
        color: None,
        symbols: None,
        is_terminal: None,
        stream: None,
        hyperlinks: None,
    });
    console_from_parts(
        options.color.as_deref(),
        options.symbols.as_deref(),
        options.is_terminal,
        options.stream.as_deref(),
        options.hyperlinks,
    )
}

fn render_context(options: Option<JsRenderOptions>) -> Result<(Console, RenderOptions)> {
    let options = options.unwrap_or(JsRenderOptions {
        color: None,
        symbols: None,
        is_terminal: None,
        stream: None,
        hyperlinks: None,
        width: None,
    });
    let console = console_from_parts(
        options.color.as_deref(),
        options.symbols.as_deref(),
        options.is_terminal,
        options.stream.as_deref(),
        options.hyperlinks,
    )?;
    let layout = match options.width {
        Some(width) => RenderOptions::new().with_width(to_u32("width", width)? as usize),
        None => RenderOptions::default(),
    };
    Ok((console, layout))
}

fn console_from_parts(
    color: Option<&str>,
    symbols: Option<&str>,
    is_terminal: Option<bool>,
    stream: Option<&str>,
    hyperlinks: Option<bool>,
) -> Result<Console> {
    let color = match color.unwrap_or("auto") {
        "auto" => ColorMode::Auto,
        "always" => ColorMode::Always,
        "never" => ColorMode::Never,
        value => return Err(invalid_value("color mode", value)),
    };
    // `auto` checks the stream the caller writes to; an explicit `isTerminal` wins.
    let stream_is_terminal = match stream.unwrap_or("stdout") {
        "stdout" => std::io::IsTerminal::is_terminal(&std::io::stdout()),
        "stderr" => std::io::IsTerminal::is_terminal(&std::io::stderr()),
        value => return Err(invalid_value("stream", value)),
    };
    let is_terminal = is_terminal.unwrap_or(stream_is_terminal);
    let mut console = Console::new(color, is_terminal);
    if let Some(hyperlinks) = hyperlinks {
        console = console.with_hyperlinks(hyperlinks);
    }
    match symbols {
        None => Ok(console),
        Some("unicode") => Ok(console.with_theme(SymbolTheme::Unicode)),
        Some("ascii") => Ok(console.with_theme(SymbolTheme::Ascii)),
        Some(value) => Err(invalid_value("symbol theme", value)),
    }
}

fn report_from_input(input: JsReportInput) -> Result<Report> {
    if input.schema_version.unwrap_or(1.0) != 1.0 {
        return Err(Error::new(
            Status::InvalidArg,
            "unsupported ReportInput schemaVersion; expected 1",
        ));
    }

    let detail_level = match input.detail_level.as_deref().unwrap_or("compact") {
        "compact" => DetailLevel::Compact,
        "detailed" => DetailLevel::Detailed,
        value => return Err(invalid_value("detail level", value)),
    };
    let mut report =
        Report::new(input.title, parse_verdict(&input.verdict)?).with_detail_level(detail_level);
    if let Some(max_samples) = input.max_compact_samples {
        report.max_compact_samples = to_u32("maxCompactSamples", max_samples)? as usize;
    }
    for metric in input.metrics.unwrap_or_default() {
        let mut output = runemark::Metric::new(metric.key, metric.value);
        if let Some(verdict) = metric.verdict {
            output = output.with_verdict(parse_verdict(&verdict)?);
        }
        // After the verdict, which supplies a tone only where none was set.
        // An explicit tone wins either way; ordering it here says so.
        if let Some(tone) = metric.tone {
            output = output.with_tone(parse_tone(&tone)?);
        }
        // Validated even without a delta; a trend alone renders nothing.
        let trend = match metric.trend.as_deref() {
            None => None,
            Some("positive") => Some(runemark::Trend::Positive),
            Some("negative") => Some(runemark::Trend::Negative),
            Some("neutral") => Some(runemark::Trend::Neutral),
            Some(value) => return Err(invalid_value("metric trend", value)),
        };
        output.trend = trend;
        output.delta = metric.delta;
        report = report.add_metric(output);
    }
    for group in input.groups.unwrap_or_default() {
        report = report.add_group(group_from_input(group)?);
    }
    for step in input.next_steps.unwrap_or_default() {
        let mut output = NextStep::new(step.text);
        if let Some(command) = step.command {
            output = output.with_command(command);
        }
        report = report.add_next_step(output);
    }
    for note in input.scope_notes.unwrap_or_default() {
        report = report.add_scope_note(ScopeNote::new(note.heading, note.items));
    }
    for vocabulary in input.vocabulary.unwrap_or_default() {
        report = report.add_vocabulary(vocabulary_from_input(vocabulary)?);
    }
    match input.top_issues_threshold {
        None => {}
        Some(Either::A(threshold)) => {
            report.top_issues_threshold = Some(to_u32("topIssuesThreshold", threshold)? as usize);
        }
        Some(Either::B(false)) => report.top_issues_threshold = None,
        Some(Either::B(true)) => {
            return Err(Error::new(
                Status::InvalidArg,
                "invalid topIssuesThreshold: true; expected a number or false",
            ));
        }
    }
    match input.details_hint {
        None => {}
        Some(Either::A(hint)) => report = report.with_details_hint(Some(hint)),
        Some(Either::B(false)) => report = report.with_details_hint(None),
        Some(Either::B(true)) => {
            return Err(Error::new(
                Status::InvalidArg,
                "invalid detailsHint: true; expected a string or false",
            ));
        }
    }
    Ok(report)
}

fn vocabulary_from_input(input: JsVocabulary) -> Result<Vocabulary> {
    let wrong_value = |expected: &str| {
        Error::new(
            Status::InvalidArg,
            format!("vocabulary kind {} needs {expected}", input.kind),
        )
    };
    match (input.kind.as_str(), input.value.as_ref()) {
        ("dry-run", None) => Ok(Vocabulary::DryRun),
        ("dry-run", Some(_)) => Err(wrong_value("no value")),
        ("empty-state", Some(Either::A(text))) => Ok(Vocabulary::EmptyState(text.clone())),
        ("empty-state", _) => Err(wrong_value("a string value")),
        ("baseline", Some(Either::A(text))) => Ok(Vocabulary::Baseline(text.clone())),
        ("baseline", _) => Err(wrong_value("a string value")),
        ("truncated", Some(Either::B(count))) => {
            let count = to_u32("vocabulary value", *count)?;
            Ok(Vocabulary::Truncated(count as usize))
        }
        ("truncated", _) => Err(wrong_value("a number value")),
        (kind, _) => Err(invalid_value("vocabulary kind", kind)),
    }
}

fn group_from_input(input: JsFindingGroup) -> Result<FindingGroup> {
    let mut group = FindingGroup::new(input.title).with_advisory(input.advisory.unwrap_or(false));
    if let Some(total_count) = input.total_count {
        group = group.with_total_count(to_u32("totalCount", total_count)? as usize);
    }
    for finding in input.findings.unwrap_or_default() {
        group = group.add_finding(finding_from_input(finding)?);
    }
    Ok(group)
}

fn finding_from_input(input: JsFinding) -> Result<Finding> {
    let mut finding = Finding::new(
        parse_tone(input.tone.as_deref().unwrap_or("warning"))?,
        input.message,
    );
    if let Some(location) = input.location {
        finding = finding.with_location(location_from_input(location)?);
    }
    if let Some(rule_id) = input.rule_id {
        finding = finding.with_rule_id(rule_id);
    }
    if let Some(remedy) = input.remedy {
        finding = finding.with_remedy(remedy);
    }
    if let Some(confidence) = input.confidence {
        finding = finding.with_confidence(match confidence.as_str() {
            "high" => Confidence::High,
            "medium" => Confidence::Medium,
            "low" => Confidence::Low,
            value => return Err(invalid_value("confidence", value)),
        });
    }
    if let Some(badge) = input.badge {
        finding = finding.with_badge(Badge::new(
            badge.label,
            parse_tone(badge.tone.as_deref().unwrap_or("info"))?,
        ));
    }
    Ok(finding)
}

fn location_from_input(input: JsLocation) -> Result<Location> {
    let line = input
        .line
        .map(|line| to_u32("location.line", line))
        .transpose()?;
    let column = input
        .column
        .map(|column| to_u32("location.column", column))
        .transpose()?;
    if column.is_some() && line.is_none() {
        return Err(Error::new(
            Status::InvalidArg,
            "location.column requires location.line",
        ));
    }
    match input.kind.as_str() {
        "file" => match (line, column) {
            (Some(line), Some(column)) => Ok(Location::file_line_col(
                input.value,
                line as usize,
                column as usize,
            )),
            (Some(line), None) => Ok(Location::file_line(input.value, line as usize)),
            (None, _) => Ok(Location::file(input.value)),
        },
        "url" => Ok(Location::Url(input.value)),
        "selector" => Ok(Location::Selector(input.value)),
        "artifact" => Ok(Location::Artifact(input.value.into())),
        value => Err(invalid_value("location kind", value)),
    }
}

fn parse_tone(value: &str) -> Result<Tone> {
    match value {
        "title" => Ok(Tone::Title),
        "muted" => Ok(Tone::Muted),
        "info" => Ok(Tone::Info),
        "success" => Ok(Tone::Success),
        "warning" => Ok(Tone::Warning),
        "error" => Ok(Tone::Error),
        _ => Err(invalid_value("tone", value)),
    }
}

fn parse_verdict(value: &str) -> Result<Verdict> {
    match value {
        "passed" => Ok(Verdict::Passed),
        "warning" => Ok(Verdict::Warning),
        "failed" => Ok(Verdict::Failed),
        "action-required" => Ok(Verdict::ActionRequired),
        "skipped" => Ok(Verdict::Skipped),
        "info" => Ok(Verdict::Info),
        _ => Err(invalid_value("verdict", value)),
    }
}

fn parse_file_action(value: &str) -> Result<FileAction> {
    match value {
        "added" => Ok(FileAction::Added),
        "modified" => Ok(FileAction::Modified),
        "deleted" => Ok(FileAction::Deleted),
        "renamed" => Ok(FileAction::Renamed),
        _ => Err(invalid_value("file action", value)),
    }
}

/// JavaScript numbers arrive as `f64`; reject anything a `u32` cannot hold
/// exactly instead of letting a cast wrap it.
fn to_u32(field: &str, value: f64) -> Result<u32> {
    if value.fract() == 0.0 && (0.0..=f64::from(u32::MAX)).contains(&value) {
        Ok(value as u32)
    } else {
        Err(Error::new(
            Status::InvalidArg,
            format!(
                "invalid {field}: {value}; expected an integer from 0 to {}",
                u32::MAX
            ),
        ))
    }
}

fn invalid_value(kind: &str, value: &str) -> Error {
    Error::new(Status::InvalidArg, format!("invalid {kind}: {value}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_tone() {
        assert_eq!(parse_tone("title").unwrap(), Tone::Title);
        assert_eq!(parse_tone("muted").unwrap(), Tone::Muted);
        assert_eq!(parse_tone("info").unwrap(), Tone::Info);
        assert_eq!(parse_tone("success").unwrap(), Tone::Success);
        assert_eq!(parse_tone("warning").unwrap(), Tone::Warning);
        assert_eq!(parse_tone("error").unwrap(), Tone::Error);

        let err = parse_tone("invalid").unwrap_err();
        assert_eq!(err.status, Status::InvalidArg);
        assert!(err.to_string().contains("invalid tone: invalid"));
    }

    #[test]
    fn test_parse_verdict() {
        assert_eq!(parse_verdict("passed").unwrap(), Verdict::Passed);
        assert_eq!(parse_verdict("warning").unwrap(), Verdict::Warning);
        assert_eq!(parse_verdict("failed").unwrap(), Verdict::Failed);
        assert_eq!(parse_verdict("action-required").unwrap(), Verdict::ActionRequired);
        assert_eq!(parse_verdict("skipped").unwrap(), Verdict::Skipped);
        assert_eq!(parse_verdict("info").unwrap(), Verdict::Info);

        let err = parse_verdict("unknown").unwrap_err();
        assert_eq!(err.status, Status::InvalidArg);
        assert!(err.to_string().contains("invalid verdict: unknown"));
    }

    #[test]
    fn test_hyperlinks_option_overrides_the_colour_default() {
        let console = |color: &str, hyperlinks| {
            console_from_parts(Some(color), None, Some(false), None, hyperlinks).unwrap()
        };
        assert!(console("always", None).hyperlinks_enabled());
        assert!(!console("never", None).hyperlinks_enabled());
        assert!(console("never", Some(true)).hyperlinks_enabled());
        assert!(!console("always", Some(false)).hyperlinks_enabled());
    }

    #[test]
    fn test_parse_file_action() {
        assert_eq!(parse_file_action("added").unwrap(), FileAction::Added);
        assert_eq!(parse_file_action("modified").unwrap(), FileAction::Modified);
        assert_eq!(parse_file_action("deleted").unwrap(), FileAction::Deleted);
        assert_eq!(parse_file_action("renamed").unwrap(), FileAction::Renamed);

        let err = parse_file_action("copied").unwrap_err();
        assert_eq!(err.status, Status::InvalidArg);
    }

    #[test]
    fn test_location_from_input() {
        let loc = location_from_input(JsLocation {
            kind: "file".into(),
            value: "src/main.rs".into(),
            line: None,
            column: None,
        })
        .unwrap();
        assert_eq!(loc, Location::file("src/main.rs"));

        let loc = location_from_input(JsLocation {
            kind: "file".into(),
            value: "src/main.rs".into(),
            line: Some(10.0),
            column: None,
        })
        .unwrap();
        assert_eq!(loc, Location::file_line("src/main.rs", 10));

        let loc = location_from_input(JsLocation {
            kind: "file".into(),
            value: "src/main.rs".into(),
            line: Some(10.0),
            column: Some(4.0),
        })
        .unwrap();
        assert_eq!(loc, Location::file_line_col("src/main.rs", 10, 4));

        let loc = location_from_input(JsLocation {
            kind: "url".into(),
            value: "https://example.com".into(),
            line: None,
            column: None,
        })
        .unwrap();
        assert_eq!(loc, Location::Url("https://example.com".into()));

        let loc = location_from_input(JsLocation {
            kind: "selector".into(),
            value: "#main".into(),
            line: None,
            column: None,
        })
        .unwrap();
        assert_eq!(loc, Location::Selector("#main".into()));

        let loc = location_from_input(JsLocation {
            kind: "artifact".into(),
            value: "build/out".into(),
            line: None,
            column: None,
        })
        .unwrap();
        assert_eq!(loc, Location::Artifact("build/out".into()));

        let err = location_from_input(JsLocation {
            kind: "invalid".into(),
            value: "test".into(),
            line: None,
            column: None,
        })
        .unwrap_err();
        assert_eq!(err.status, Status::InvalidArg);
    }

    #[test]
    fn test_to_u32_rejects_out_of_range_and_fractions() {
        assert_eq!(to_u32("n", 0.0).unwrap(), 0);
        assert_eq!(to_u32("n", 4_294_967_295.0).unwrap(), u32::MAX);
        for bad in [-1.0, 1.5, 4_294_967_296.0, f64::NAN, f64::INFINITY] {
            let err = to_u32("n", bad).unwrap_err();
            assert_eq!(err.status, Status::InvalidArg);
            assert!(err.to_string().contains("invalid n:"));
        }
    }

    #[test]
    fn test_location_column_requires_line() {
        let err = location_from_input(JsLocation {
            kind: "file".into(),
            value: "src/main.rs".into(),
            line: None,
            column: Some(4.0),
        })
        .unwrap_err();
        assert_eq!(err.status, Status::InvalidArg);
        assert!(err.to_string().contains("column requires location.line"));
    }

    #[test]
    fn test_report_from_input_schema_version_and_options() {
        let err = report_from_input(JsReportInput {
            schema_version: Some(2.0),
            title: "Future".into(),
            verdict: "passed".into(),
            metrics: None,
            groups: None,
            next_steps: None,
            scope_notes: None,
            vocabulary: None,
            top_issues_threshold: None,
            details_hint: None,
            detail_level: None,
            max_compact_samples: None,
        })
        .unwrap_err();
        assert_eq!(err.status, Status::InvalidArg);
        assert!(err.to_string().contains("unsupported ReportInput schemaVersion"));

        let report = report_from_input(JsReportInput {
            schema_version: Some(1.0),
            title: "Valid".into(),
            verdict: "passed".into(),
            metrics: Some(vec![JsMetric {
                key: "Errors".into(),
                value: "0".into(),
                verdict: Some("passed".into()),
                tone: Some("success".into()),
                trend: Some("positive".into()),
                delta: Some("-2".into()),
            }]),
            groups: Some(vec![JsFindingGroup {
                title: "Code".into(),
                findings: Some(vec![JsFinding {
                    message: "All good".into(),
                    tone: Some("info".into()),
                    location: None,
                    rule_id: Some("lint/clean".into()),
                    remedy: None,
                    confidence: Some("high".into()),
                    badge: Some(JsBadge {
                        label: "verified".into(),
                        tone: Some("success".into()),
                    }),
                }]),
                total_count: Some(1.0),
                advisory: Some(false),
            }]),
            next_steps: Some(vec![JsNextStep {
                text: "Ship it".into(),
                command: Some("deploy".into()),
            }]),
            scope_notes: Some(vec![JsScopeNote {
                heading: "Scope".into(),
                items: vec!["src only".into()],
            }]),
            vocabulary: Some(vec![
                JsVocabulary {
                    kind: "dry-run".into(),
                    value: None,
                },
                JsVocabulary {
                    kind: "truncated".into(),
                    value: Some(Either::B(5.0)),
                },
            ]),
            top_issues_threshold: Some(Either::A(7.0)),
            details_hint: Some(Either::A("use --verbose".into())),
            detail_level: Some("detailed".into()),
            max_compact_samples: Some(5.0),
        })
        .unwrap();
        assert_eq!(report.title, "Valid");
        assert_eq!(report.detail_level, DetailLevel::Detailed);
        assert_eq!(report.max_compact_samples, 5);
        assert_eq!(report.scope_notes.len(), 1);
        assert_eq!(
            report.vocabulary_notes,
            vec![Vocabulary::DryRun, Vocabulary::Truncated(5)]
        );
        assert_eq!(report.top_issues_threshold, Some(7));
        assert_eq!(report.details_hint.as_deref(), Some("use --verbose"));
    }

    #[test]
    fn test_details_hint_default_false_and_invalid() {
        let input = |details_hint| JsReportInput {
            schema_version: None,
            title: "T".into(),
            verdict: "passed".into(),
            metrics: None,
            groups: None,
            next_steps: None,
            scope_notes: None,
            vocabulary: None,
            top_issues_threshold: None,
            details_hint,
            detail_level: None,
            max_compact_samples: None,
        };
        let hint = |value| report_from_input(input(value)).unwrap().details_hint;
        assert_eq!(hint(None).as_deref(), Some("use --details to view all"));
        assert_eq!(hint(Some(Either::B(false))), None);
        assert_eq!(
            hint(Some(Either::A("use --verbose".into()))).as_deref(),
            Some("use --verbose")
        );

        let err = report_from_input(input(Some(Either::B(true)))).unwrap_err();
        assert_eq!(err.status, Status::InvalidArg);
        assert!(err.to_string().contains("invalid detailsHint: true"));
    }

    #[test]
    fn test_metric_trend_validated_without_delta() {
        let input = |trend: &str| JsReportInput {
            schema_version: None,
            title: "T".into(),
            verdict: "passed".into(),
            metrics: Some(vec![JsMetric {
                key: "K".into(),
                value: "V".into(),
                verdict: None,
                tone: None,
                trend: Some(trend.into()),
                delta: None,
            }]),
            groups: None,
            next_steps: None,
            scope_notes: None,
            vocabulary: None,
            top_issues_threshold: None,
            details_hint: None,
            detail_level: None,
            max_compact_samples: None,
        };
        assert!(report_from_input(input("positive")).is_ok());
        let err = report_from_input(input("bogus")).unwrap_err();
        assert!(err.to_string().contains("invalid metric trend: bogus"));
    }

    #[test]
    fn test_vocabulary_from_input() {
        let vocabulary = |kind: &str, value: Option<Either<String, f64>>| {
            vocabulary_from_input(JsVocabulary {
                kind: kind.into(),
                value,
            })
        };
        assert_eq!(vocabulary("dry-run", None).unwrap(), Vocabulary::DryRun);
        assert_eq!(
            vocabulary("baseline", Some(Either::A("main".into()))).unwrap(),
            Vocabulary::Baseline("main".into())
        );
        assert_eq!(
            vocabulary("empty-state", Some(Either::A("Nothing found".into()))).unwrap(),
            Vocabulary::EmptyState("Nothing found".into())
        );
        assert_eq!(
            vocabulary("truncated", Some(Either::B(20.0))).unwrap(),
            Vocabulary::Truncated(20)
        );
        assert!(vocabulary("dry-run", Some(Either::A("x".into()))).is_err());
        assert!(vocabulary("baseline", None).is_err());
        assert!(vocabulary("truncated", Some(Either::A("x".into()))).is_err());
        assert!(vocabulary("truncated", Some(Either::B(-1.0))).is_err());
        let err = vocabulary("unknown", None).unwrap_err();
        assert!(err.to_string().contains("invalid vocabulary kind: unknown"));
    }

    #[test]
    fn test_report_from_input_invalid_enums() {
        let err = report_from_input(JsReportInput {
            schema_version: Some(1.0),
            title: "Test".into(),
            verdict: "passed".into(),
            metrics: Some(vec![JsMetric {
                key: "K".into(),
                value: "V".into(),
                verdict: None,
                tone: None,
                trend: Some("invalid_trend".into()),
                delta: Some("+1".into()),
            }]),
            groups: None,
            next_steps: None,
            scope_notes: None,
            vocabulary: None,
            top_issues_threshold: None,
            details_hint: None,
            detail_level: None,
            max_compact_samples: None,
        })
        .unwrap_err();
        assert_eq!(err.status, Status::InvalidArg);

        let err = finding_from_input(JsFinding {
            message: "msg".into(),
            tone: None,
            location: None,
            rule_id: None,
            remedy: None,
            confidence: Some("super_high".into()),
            badge: None,
        })
        .unwrap_err();
        assert_eq!(err.status, Status::InvalidArg);
    }
}
