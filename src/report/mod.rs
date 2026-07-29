//! Decision-oriented report presentation models and renderers.

use crate::color::{Console, Tone};
use crate::location::Location;
use crate::verdict::Verdict;
use std::collections::BTreeMap;
use std::io::Write;

/// Render detail mode for reports.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum DetailLevel {
    /// Compact summary with aggregated counts and bounded finding samples.
    #[default]
    Compact,
    /// Full detail view with locations, rule IDs, and remedies.
    Detailed,
}

/// Confidence level of an audit finding or diagnostic check.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Confidence {
    High,
    Medium,
    Low,
}

impl Confidence {
    pub fn label(self) -> &'static str {
        match self {
            Self::High => "high confidence",
            Self::Medium => "medium confidence",
            Self::Low => "low confidence",
        }
    }
}

/// Trend direction for metrics and performance deltas.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Trend {
    Positive,
    Negative,
    Neutral,
}

/// A status badge tag for findings, KPIs, or changes (e.g. `[QUICK WIN]`, `[GAINED]`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Badge {
    pub label: String,
    pub tone: Tone,
}

impl Badge {
    pub fn new(label: impl Into<String>, tone: Tone) -> Self {
        Self {
            label: label.into(),
            tone,
        }
    }

    pub fn quick_win() -> Self {
        Self::new("QUICK WIN", Tone::Success)
    }

    pub fn gained() -> Self {
        Self::new("GAINED", Tone::Success)
    }

    pub fn lost() -> Self {
        Self::new("LOST", Tone::Error)
    }

    pub fn render(&self, console: Console) -> String {
        console.paint(self.tone, format!("[{}]", self.label))
    }
}

/// A key-value metric item with optional trend and threshold coloring.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Metric {
    pub key: String,
    pub value: String,
    pub tone: Option<Tone>,
    pub trend: Option<Trend>,
    pub delta: Option<String>,
}

impl Metric {
    /// Creates a new metric pair.
    pub fn new(key: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            value: value.into(),
            tone: None,
            trend: None,
            delta: None,
        }
    }

    /// Sets the semantic tone for this metric.
    pub fn with_tone(mut self, tone: Tone) -> Self {
        self.tone = Some(tone);
        self
    }

    /// Attaches a trend direction and delta string (e.g. `+14.2%`).
    pub fn with_trend(mut self, trend: Trend, delta: impl Into<String>) -> Self {
        self.trend = Some(trend);
        self.delta = Some(delta.into());
        self
    }
}

/// An individual finding entry within a report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    pub message: String,
    pub tone: Tone,
    pub location: Option<Location>,
    pub rule_id: Option<String>,
    pub remedy: Option<String>,
    pub confidence: Option<Confidence>,
    pub badge: Option<Badge>,
}

impl Finding {
    /// Creates a basic finding.
    pub fn new(tone: Tone, message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            tone,
            location: None,
            rule_id: None,
            remedy: None,
            confidence: None,
            badge: None,
        }
    }

    /// Attaches a location to this finding.
    pub fn with_location(mut self, location: Location) -> Self {
        self.location = Some(location);
        self
    }

    /// Attaches a rule identifier.
    pub fn with_rule_id(mut self, rule_id: impl Into<String>) -> Self {
        self.rule_id = Some(rule_id.into());
        self
    }

    /// Attaches a remedy hint.
    pub fn with_remedy(mut self, remedy: impl Into<String>) -> Self {
        self.remedy = Some(remedy.into());
        self
    }

    /// Attaches a confidence level rating.
    pub fn with_confidence(mut self, confidence: Confidence) -> Self {
        self.confidence = Some(confidence);
        self
    }

    /// Attaches a status badge.
    pub fn with_badge(mut self, badge: Badge) -> Self {
        self.badge = Some(badge);
        self
    }
}

/// A group of related findings (e.g. by rule or category).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FindingGroup {
    pub title: String,
    pub findings: Vec<Finding>,
    pub total_count: usize,
    pub advisory: bool,
}

impl FindingGroup {
    /// Creates a finding group.
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            findings: Vec::new(),
            total_count: 0,
            advisory: false,
        }
    }

    /// Marks this finding group as advisory-only.
    pub fn with_advisory(mut self, advisory: bool) -> Self {
        self.advisory = advisory;
        self
    }

    /// Records the total count when this group only contains a bounded sample.
    pub fn with_total_count(mut self, total_count: usize) -> Self {
        self.total_count = total_count.max(self.findings.len());
        self
    }

    /// Adds a finding to this group.
    pub fn add_finding(mut self, finding: Finding) -> Self {
        self.findings.push(finding);
        self.total_count = self.total_count.max(self.findings.len());
        self
    }

    fn rendered_count(&self) -> usize {
        self.total_count.max(self.findings.len())
    }
}

/// A scope or completeness note (e.g. analysis scope warnings, unavailable history).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScopeNote {
    pub heading: String,
    pub items: Vec<String>,
}

impl ScopeNote {
    pub fn new(heading: impl Into<String>, items: Vec<String>) -> Self {
        Self {
            heading: heading.into(),
            items,
        }
    }
}

/// A next-step action recommendation at the end of a report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NextStep {
    pub text: String,
    pub command: Option<String>,
}

impl NextStep {
    /// Creates a next step recommendation.
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            command: None,
        }
    }

    /// Attaches a copyable command.
    pub fn with_command(mut self, command: impl Into<String>) -> Self {
        self.command = Some(command.into());
        self
    }
}

/// Common shared result status vocabulary (empty-state, dry-run, baseline, truncated).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Vocabulary {
    /// Dry run indicator.
    DryRun,
    /// Clean empty state.
    EmptyState(String),
    /// Baseline reference comparison.
    Baseline(String),
    /// Truncated result notice.
    Truncated(usize),
}

/// A complete decision-oriented report model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    pub title: String,
    pub verdict: Verdict,
    pub metrics: Vec<Metric>,
    pub groups: Vec<FindingGroup>,
    pub scope_notes: Vec<ScopeNote>,
    pub next_steps: Vec<NextStep>,
    pub vocabulary_notes: Vec<Vocabulary>,
    pub detail_level: DetailLevel,
    pub max_compact_samples: usize,
    pub top_issues_threshold: Option<usize>,
}

impl Report {
    /// Creates a new report with a title and verdict.
    pub fn new(title: impl Into<String>, verdict: Verdict) -> Self {
        Self {
            title: title.into(),
            verdict,
            metrics: Vec::new(),
            groups: Vec::new(),
            scope_notes: Vec::new(),
            next_steps: Vec::new(),
            vocabulary_notes: Vec::new(),
            detail_level: DetailLevel::Compact,
            max_compact_samples: 3,
            top_issues_threshold: Some(20),
        }
    }

    /// Sets the detail level mode.
    pub fn with_detail_level(mut self, level: DetailLevel) -> Self {
        self.detail_level = level;
        self
    }

    /// Adds a metric item to the summary grid.
    pub fn add_metric(mut self, metric: Metric) -> Self {
        self.metrics.push(metric);
        self
    }

    /// Adds a group of findings.
    pub fn add_group(mut self, group: FindingGroup) -> Self {
        self.groups.push(group);
        self
    }

    /// Adds a scope note with heading and items.
    pub fn add_scope_note(mut self, note: ScopeNote) -> Self {
        self.scope_notes.push(note);
        self
    }

    /// Adds a vocabulary note (dry-run, baseline, empty state).
    pub fn add_vocabulary(mut self, vocab: Vocabulary) -> Self {
        self.vocabulary_notes.push(vocab);
        self
    }

    /// Adds a next-step recommendation.
    pub fn add_next_step(mut self, step: NextStep) -> Self {
        self.next_steps.push(step);
        self
    }

    /// Writes the report formatted directly to a writer.
    pub fn write_to(
        &self,
        console: Console,
        writer: &mut (impl Write + ?Sized),
    ) -> std::io::Result<()> {
        // 1. Header & Verdict Line
        self.verdict.write_to(console, &self.title, writer)?;
        writeln!(writer)?;

        // Vocabulary badges
        let mut note_items = Vec::new();
        for vocab in &self.vocabulary_notes {
            match vocab {
                Vocabulary::DryRun => note_items.push("[DRY-RUN]".to_string()),
                Vocabulary::EmptyState(msg) => note_items.push(msg.clone()),
                Vocabulary::Baseline(base) => note_items.push(format!("Baseline: {base}")),
                Vocabulary::Truncated(n) => note_items.push(format!("Truncated at {n}")),
            }
        }

        if !note_items.is_empty() {
            write!(writer, "  ")?;
            console.write_paint(Tone::Muted, note_items.join(" • "), writer)?;
            writeln!(writer)?;
        }

        // Scope notes
        for note in &self.scope_notes {
            write!(writer, "  ")?;
            console.write_paint(Tone::Warning, format!("{}: ", note.heading), writer)?;
            console.write_paint(Tone::Muted, note.items.join(", "), writer)?;
            writeln!(writer)?;
        }
        writeln!(writer)?;

        // 2. Summary Metrics Grid
        if !self.metrics.is_empty() {
            write!(writer, "  ")?;
            for (idx, m) in self.metrics.iter().enumerate() {
                if idx > 0 {
                    write!(writer, "   ")?;
                }
                console.write_paint(Tone::Muted, format!("{}: ", m.key), writer)?;
                let val_tone = m.tone.unwrap_or(Tone::Info);
                console.write_paint(val_tone, &m.value, writer)?;
                if let Some(ref d) = m.delta {
                    write!(writer, " ")?;
                    let trend_tone = match m.trend {
                        Some(Trend::Positive) => Tone::Success,
                        Some(Trend::Negative) => Tone::Error,
                        _ => Tone::Muted,
                    };
                    console.write_paint(trend_tone, format!("({d})"), writer)?;
                }
            }
            writeln!(writer)?;
            writeln!(writer)?;
        }

        // 3. Optional Top Issues Aggregation Summary
        let total_finding_count: usize = self.groups.iter().map(FindingGroup::rendered_count).sum();
        if let Some(thresh) = self.top_issues_threshold {
            if total_finding_count >= thresh {
                let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
                for g in &self.groups {
                    for f in &g.findings {
                        if let Some(ref r) = f.rule_id {
                            *counts.entry(r.as_str()).or_insert(0) += 1;
                        }
                    }
                }
                if !counts.is_empty() {
                    write!(writer, "  ")?;
                    console.write_paint(Tone::Title, "Top issue rules:", writer)?;
                    writeln!(writer)?;
                    let mut sorted: Vec<(&str, usize)> = counts.into_iter().collect();
                    sorted.sort_by_key(|(rule, count)| (std::cmp::Reverse(*count), *rule));
                    for (rule, count) in sorted.into_iter().take(5) {
                        write!(writer, "    {:>4}x  ", count)?;
                        console.write_paint(Tone::Muted, rule, writer)?;
                        writeln!(writer)?;
                    }
                    writeln!(writer)?;
                }
            }
        }

        // 4. Finding Groups
        for group in &self.groups {
            let total_count = group.rendered_count();
            if total_count == 0 {
                continue;
            }

            let group_marker = match console.symbol_theme() {
                crate::color::SymbolTheme::Unicode => "●",
                crate::color::SymbolTheme::Ascii => "*",
            };
            write!(writer, "{group_marker} ")?;
            console.write_paint(Tone::Title, &group.title, writer)?;
            write!(writer, " ")?;
            console.write_paint(Tone::Muted, format!("({total_count})"), writer)?;
            if group.advisory {
                write!(writer, " ")?;
                console.write_paint(Tone::Info, "[advisory]", writer)?;
            }
            writeln!(writer)?;

            let sample_limit = match self.detail_level {
                DetailLevel::Compact => self.max_compact_samples,
                DetailLevel::Detailed => usize::MAX,
            };

            let shown_findings = group.findings.iter().take(sample_limit);
            for f in shown_findings {
                write!(writer, "  - ")?;
                if let Some(ref b) = f.badge {
                    write!(writer, "{} ", b.render(console))?;
                }
                console.write_paint(f.tone, &f.message, writer)?;

                if let Some(ref r) = f.rule_id {
                    write!(writer, " ")?;
                    console.write_paint(Tone::Muted, format!("[{r}]"), writer)?;
                }

                if let Some(ref conf) = f.confidence {
                    write!(writer, " ")?;
                    console.write_paint(Tone::Muted, format!("({})", conf.label()), writer)?;
                }

                if let Some(ref loc) = f.location {
                    write!(writer, " at {}", loc.render(console))?;
                }
                writeln!(writer)?;

                if self.detail_level == DetailLevel::Detailed {
                    if let Some(ref rem) = f.remedy {
                        write!(writer, "    ")?;
                        console.write_paint(Tone::Muted, format!("Remedy: {rem}"), writer)?;
                        writeln!(writer)?;
                    }
                }
            }

            if self.detail_level == DetailLevel::Compact
                && total_count > group.findings.len().min(sample_limit)
            {
                let remaining = total_count - group.findings.len().min(sample_limit);
                write!(writer, "  ")?;
                console.write_paint(
                    Tone::Muted,
                    format!("... {remaining} more finding(s) (use --details to view all)"),
                    writer,
                )?;
                writeln!(writer)?;
            }

            writeln!(writer)?;
        }

        // 5. Next Steps
        if !self.next_steps.is_empty() {
            console.write_paint(Tone::Title, "Next steps:", writer)?;
            writeln!(writer)?;
            let bullet = match console.symbol_theme() {
                crate::color::SymbolTheme::Unicode => "•",
                crate::color::SymbolTheme::Ascii => "-",
            };
            for step in &self.next_steps {
                writeln!(writer, "  {bullet} {}", step.text)?;
                if let Some(ref cmd) = step.command {
                    write!(writer, "    ")?;
                    console.write_paint(Tone::Info, format!("$ {cmd}"), writer)?;
                    writeln!(writer)?;
                }
            }
        }

        Ok(())
    }

    /// Renders the report as a formatted String.
    pub fn render(&self, console: Console) -> String {
        let mut buf = Vec::new();
        let _ = self.write_to(console, &mut buf);
        String::from_utf8(buf).unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color::ColorMode;

    #[test]
    fn report_compact_rendering() {
        let report = Report::new("Astro Post Audit", Verdict::Warning)
            .add_metric(Metric::new("Errors", "0"))
            .add_metric(Metric::new("Warnings", "2").with_tone(Tone::Warning))
            .add_group(
                FindingGroup::new("Performance Warnings")
                    .add_finding(Finding::new(Tone::Warning, "Large image uncompressed")),
            );

        let console = Console::new(ColorMode::Never, false);
        let output = report.render(console);

        assert!(output.contains("[WARN] Astro Post Audit"));
        assert!(output.contains("Errors: 0"));
        assert!(output.contains("Warnings: 2"));
        assert!(output.contains("* Performance Warnings (1)"));
        assert!(output.contains("- Large image uncompressed"));
    }

    #[test]
    fn report_writes_to_a_trait_object_writer() {
        let report = Report::new("Audit", Verdict::Passed);
        let console = Console::new(ColorMode::Never, false);
        let mut output = Vec::new();
        let writer: &mut dyn Write = &mut output;

        report.write_to(console, writer).unwrap();

        assert_eq!(String::from_utf8(output).unwrap(), "[OK] Audit\n\n");
    }

    #[test]
    fn compact_report_counts_all_omitted_findings() {
        let report = Report::new("Audit", Verdict::Warning).add_group(
            FindingGroup::new("Warnings")
                .with_total_count(5)
                .add_finding(Finding::new(Tone::Warning, "One representative warning")),
        );
        let console = Console::new(ColorMode::Never, false);

        let output = report.render(console);

        assert!(output.contains("* Warnings (5)"));
        assert!(output.contains("... 4 more finding(s) (use --details to view all)"));
    }

    #[test]
    fn compact_report_renders_aggregate_only_groups() {
        let report = Report::new("Audit", Verdict::Warning)
            .add_group(FindingGroup::new("Warnings").with_total_count(2));
        let console = Console::new(ColorMode::Never, false);

        let output = report.render(console);

        assert!(output.contains("* Warnings (2)"));
        assert!(output.contains("... 2 more finding(s) (use --details to view all)"));
    }

    #[test]
    fn top_issue_order_is_deterministic_for_equal_counts() {
        let report = Report::new("Audit", Verdict::Warning)
            .add_group(
                FindingGroup::new("Warnings")
                    .add_finding(Finding::new(Tone::Warning, "Second").with_rule_id("z-rule"))
                    .add_finding(Finding::new(Tone::Warning, "First").with_rule_id("a-rule")),
            )
            .add_group(FindingGroup::new("Other").with_total_count(18));
        let console = Console::new(ColorMode::Never, false);

        let output = report.render(console);

        assert!(output.find("a-rule").unwrap() < output.find("z-rule").unwrap());
    }
}
