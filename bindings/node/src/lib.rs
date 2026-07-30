//! Native Node-API boundary for the curated `@casoon/runemark` package.
//!
//! The binding accepts plain Node data objects and keeps Runemark's Rust types
//! private. This lets each public DTO evolve additively without exposing Rust
//! builders, traits, or enum representation details as a JavaScript contract.

use napi::{Error, Result, Status};
use napi_derive::napi;
use runemark::{
    Badge, ColorMode, Confidence, Console, DetailLevel, DiffBlock, ErrorBlock, FileAction,
    FileChange, Finding, FindingGroup, Location, NextStep, ProgressMode, ProgressSink,
    RenderOptions, Report, SymbolTheme, TerminalProgress, Tone, Verdict,
};

#[napi(object)]
pub struct JsConsoleOptions {
    pub color: Option<String>,
    pub symbols: Option<String>,
    pub is_terminal: Option<bool>,
}

#[napi(object)]
pub struct JsRenderOptions {
    pub color: Option<String>,
    pub symbols: Option<String>,
    pub is_terminal: Option<bool>,
    pub width: Option<u32>,
}

#[napi(object)]
pub struct JsMetric {
    pub key: String,
    pub value: String,
    pub tone: Option<String>,
    pub trend: Option<String>,
    pub delta: Option<String>,
}

#[napi(object)]
pub struct JsLocation {
    pub kind: String,
    pub value: String,
    pub line: Option<u32>,
    pub column: Option<u32>,
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
    pub total_count: Option<u32>,
    pub advisory: Option<bool>,
}

#[napi(object)]
pub struct JsNextStep {
    pub text: String,
    pub command: Option<String>,
}

#[napi(object)]
pub struct JsReportInput {
    pub schema_version: Option<u32>,
    pub title: String,
    pub verdict: String,
    pub metrics: Option<Vec<JsMetric>>,
    pub groups: Option<Vec<JsFindingGroup>>,
    pub next_steps: Option<Vec<JsNextStep>>,
    pub detail_level: Option<String>,
    pub max_compact_samples: Option<u32>,
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
        let console = console_from_parts(
            options.color.as_deref(),
            options.symbols.as_deref(),
            options.is_terminal,
        )?;
        let mode = match options.mode.as_deref().unwrap_or("auto") {
            "auto" => ProgressMode::Auto,
            "always" => ProgressMode::Always,
            "never" => ProgressMode::Never,
            value => return Err(invalid_value("progress mode", value)),
        };
        let is_terminal = options
            .is_terminal
            .unwrap_or_else(|| std::io::IsTerminal::is_terminal(&std::io::stderr()));

        Ok(Self {
            progress: TerminalProgress::stderr(mode, console, is_terminal),
        })
    }

    #[napi]
    pub fn start(&self, total: u32, message: String) {
        self.progress.start(u64::from(total), &message);
    }

    #[napi]
    pub fn advance(&self, position: u32, message: String) {
        self.progress.advance(u64::from(position), &message);
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
    });
    console_from_parts(
        options.color.as_deref(),
        options.symbols.as_deref(),
        options.is_terminal,
    )
}

fn render_context(options: Option<JsRenderOptions>) -> Result<(Console, RenderOptions)> {
    let options = options.unwrap_or(JsRenderOptions {
        color: None,
        symbols: None,
        is_terminal: None,
        width: None,
    });
    let console = console_from_parts(
        options.color.as_deref(),
        options.symbols.as_deref(),
        options.is_terminal,
    )?;
    let layout = options
        .width
        .map(|width| RenderOptions::new().with_width(width as usize))
        .unwrap_or_default();
    Ok((console, layout))
}

fn console_from_parts(
    color: Option<&str>,
    symbols: Option<&str>,
    is_terminal: Option<bool>,
) -> Result<Console> {
    let color = match color.unwrap_or("auto") {
        "auto" => ColorMode::Auto,
        "always" => ColorMode::Always,
        "never" => ColorMode::Never,
        value => return Err(invalid_value("color mode", value)),
    };
    let is_terminal =
        is_terminal.unwrap_or_else(|| std::io::IsTerminal::is_terminal(&std::io::stdout()));
    let console = Console::new(color, is_terminal);
    match symbols {
        None => Ok(console),
        Some("unicode") => Ok(console.with_theme(SymbolTheme::Unicode)),
        Some("ascii") => Ok(console.with_theme(SymbolTheme::Ascii)),
        Some(value) => Err(invalid_value("symbol theme", value)),
    }
}

fn report_from_input(input: JsReportInput) -> Result<Report> {
    if input.schema_version.unwrap_or(1) != 1 {
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
        report.max_compact_samples = max_samples as usize;
    }
    for metric in input.metrics.unwrap_or_default() {
        let mut output = runemark::Metric::new(metric.key, metric.value);
        if let Some(tone) = metric.tone {
            output = output.with_tone(parse_tone(&tone)?);
        }
        if let Some(delta) = metric.delta {
            let trend = match metric.trend.as_deref().unwrap_or("neutral") {
                "positive" => runemark::Trend::Positive,
                "negative" => runemark::Trend::Negative,
                "neutral" => runemark::Trend::Neutral,
                value => return Err(invalid_value("metric trend", value)),
            };
            output = output.with_trend(trend, delta);
        }
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
    Ok(report)
}

fn group_from_input(input: JsFindingGroup) -> Result<FindingGroup> {
    let mut group = FindingGroup::new(input.title).with_advisory(input.advisory.unwrap_or(false));
    if let Some(total_count) = input.total_count {
        group = group.with_total_count(total_count as usize);
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
    match input.kind.as_str() {
        "file" => match (input.line, input.column) {
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

fn invalid_value(kind: &str, value: &str) -> Error {
    Error::new(Status::InvalidArg, format!("invalid {kind}: {value}"))
}
