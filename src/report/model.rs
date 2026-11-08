//! Decision-oriented report data model: metrics, findings, groups, and next steps.

use crate::color::Tone;
use crate::verdict::Verdict;

/// Render detail mode for reports.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum DetailLevel {
    /// Compact summary with aggregated counts and bounded finding samples.
    #[default]
    Compact,
    /// Full detail view for every finding provided by the report data, with
    /// locations, rule IDs, and remedies.
    Detailed,
}

/// Host-provided layout constraints for rendering a report.
///
/// runemark deliberately does not inspect the terminal size itself. Applications
/// can pass a known width for an interactive terminal or a deterministic width
/// for tests and redirected output.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct RenderOptions {
    width: Option<usize>,
}

impl RenderOptions {
    /// Creates unconstrained rendering options.
    pub const fn new() -> Self {
        Self { width: None }
    }

    /// Uses the available terminal width for adaptive report layout.
    ///
    /// A width of zero is ignored because no usable terminal layout can be
    /// derived from it.
    pub fn with_width(mut self, width: usize) -> Self {
        self.width = (width > 0).then_some(width);
        self
    }

    /// Returns the configured terminal width, if any.
    pub const fn width(self) -> Option<usize> {
        self.width
    }
}

/// Confidence level of an audit finding or diagnostic check.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Confidence {
    /// The check is very likely correct.
    High,
    /// The check is probably correct.
    Medium,
    /// The check may be a false positive.
    Low,
}

impl Confidence {
    /// Returns the text rendered in parentheses after a finding, such as `high confidence`.
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
#[non_exhaustive]
pub enum Trend {
    /// The delta is an improvement.
    Positive,
    /// The delta is a regression.
    Negative,
    /// The delta is neither an improvement nor a regression.
    Neutral,
}

/// A status badge tag for findings, KPIs, or changes (e.g. `[QUICK WIN]`, `[GAINED]`).
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Badge {
    /// The text between the brackets, rendered as given (`QUICK WIN` renders `[QUICK WIN]`).
    pub label: String,
    /// The tone the badge is painted in.
    pub tone: Tone,
}

impl Badge {
    /// Creates a badge with a custom label and tone.
    pub fn new(label: impl Into<String>, tone: Tone) -> Self {
        Self {
            label: label.into(),
            tone,
        }
    }

    /// A `QUICK WIN` badge in the success tone.
    pub fn quick_win() -> Self {
        Self::new("QUICK WIN", Tone::Success)
    }

    /// A `GAINED` badge in the success tone.
    pub fn gained() -> Self {
        Self::new("GAINED", Tone::Success)
    }

    /// A `LOST` badge in the error tone.
    pub fn lost() -> Self {
        Self::new("LOST", Tone::Error)
    }

    /// Writes the badge directly to a writer.
    pub fn write_to(
        &self,
        console: crate::color::Console,
        writer: &mut (impl std::io::Write + ?Sized),
    ) -> std::io::Result<()> {
        console.write_paint(self.tone, format!("[{}]", self.label), writer)
    }

    /// Renders the badge as a String.
    pub fn render(&self, console: crate::color::Console) -> String {
        crate::internal::collect_to_string(|buf| self.write_to(console, buf))
    }
}

/// A key-value metric item with optional trend and threshold coloring.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Metric {
    /// The metric's name.
    pub key: String,
    /// The metric's value, rendered as given.
    pub value: String,
    /// Explicit tone; when unset, the verdict's tone applies if there is one.
    pub tone: Option<Tone>,
    /// Trend direction; colours the delta and renders nothing without one.
    pub trend: Option<Trend>,
    /// Change since the last run, such as `+14.2%`.
    pub delta: Option<String>,
    /// Outcome marker: renders the verdict's symbol in front of the metric.
    pub verdict: Option<crate::Verdict>,
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
            verdict: None,
        }
    }

    /// Sets the semantic tone for this metric.
    pub fn with_tone(mut self, tone: Tone) -> Self {
        self.tone = Some(tone);
        self
    }

    /// Marks this metric as an outcome, rendering its verdict's symbol.
    ///
    /// A tone alone disappears with colour: piped, or under `NO_COLOR`, a
    /// failing metric reads exactly like a passing one. The symbol survives
    /// both, which is what makes a metric usable as a status rather than only
    /// as a number.
    ///
    /// The verdict also supplies the tone where none was set, so the two
    /// cannot drift apart; an explicit [`Metric::with_tone`] still wins,
    /// whichever order they are called in.
    pub fn with_verdict(mut self, verdict: crate::Verdict) -> Self {
        self.verdict = Some(verdict);
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
#[non_exhaustive]
pub struct Finding {
    /// The finding text.
    pub message: String,
    /// The tone the message is painted in.
    pub tone: Tone,
    /// Where the finding was observed.
    pub location: Option<crate::location::Location>,
    /// Identifier of the rule that produced the finding, rendered as `[rule-id]`.
    pub rule_id: Option<String>,
    /// How to fix the finding; only rendered in [`DetailLevel::Detailed`].
    pub remedy: Option<String>,
    /// How sure the check is, rendered in parentheses.
    pub confidence: Option<Confidence>,
    /// A badge rendered in front of the message.
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
    pub fn with_location(mut self, location: crate::location::Location) -> Self {
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
#[non_exhaustive]
pub struct FindingGroup {
    /// The group heading.
    pub title: String,
    /// The findings carried by the report; possibly a bounded sample of `total_count`.
    pub findings: Vec<Finding>,
    /// The number of findings the group stands for; raised to `findings.len()` when lower.
    pub total_count: usize,
    /// Marks the group as advisory; the heading gets an `[advisory]` suffix.
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

    pub(super) fn rendered_count(&self) -> usize {
        self.total_count.max(self.findings.len())
    }
}

/// A scope or completeness note (e.g. analysis scope warnings, unavailable history).
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct ScopeNote {
    /// The label before the colon.
    pub heading: String,
    /// The items, rendered comma-separated after the heading.
    pub items: Vec<String>,
}

impl ScopeNote {
    /// Creates a scope note from a heading and its items.
    pub fn new(heading: impl Into<String>, items: Vec<String>) -> Self {
        Self {
            heading: heading.into(),
            items,
        }
    }
}

/// A next-step action recommendation at the end of a report.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct NextStep {
    /// What to do next.
    pub text: String,
    /// A copyable command, rendered as `$ command`.
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
#[non_exhaustive]
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
#[non_exhaustive]
pub struct Report {
    /// The headline shown next to the verdict symbol.
    pub title: String,
    /// The overall outcome.
    pub verdict: Verdict,
    /// Summary metrics, rendered in one row (stacked when a narrow width requires it).
    pub metrics: Vec<Metric>,
    /// Finding groups, in order. Empty groups are not rendered.
    pub groups: Vec<FindingGroup>,
    /// Scope or completeness notes shown under the verdict line.
    pub scope_notes: Vec<ScopeNote>,
    /// Recommended actions shown at the end.
    pub next_steps: Vec<NextStep>,
    /// Dry-run, empty-state, baseline and truncation notes shown under the verdict line.
    pub vocabulary_notes: Vec<Vocabulary>,
    /// Compact or detailed rendering.
    pub detail_level: DetailLevel,
    /// How many findings per group the compact form shows (default 3). There is
    /// no builder; assign the field.
    pub max_compact_samples: usize,
    /// Finding count from which a "Top issue rules" summary is shown (default
    /// `Some(20)`); `None` turns it off. There is no builder; assign the field.
    pub top_issues_threshold: Option<usize>,
    /// Text in parentheses after a compact-mode overflow message; `None` omits it.
    pub details_hint: Option<String>,
}

impl Report {
    /// Creates a new report with a title and verdict.
    ///
    /// Compact reports show three sample findings per group, a "Top issue rules"
    /// summary from 20 findings upwards, and the default details hint.
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
            details_hint: Some("use --details to view all".to_string()),
        }
    }

    /// Sets the detail level mode.
    pub fn with_detail_level(mut self, level: DetailLevel) -> Self {
        self.detail_level = level;
        self
    }

    /// Sets the hint shown in parentheses after a compact-mode overflow message.
    ///
    /// The default is `use --details to view all`. `None` drops the parenthetical.
    pub fn with_details_hint(mut self, hint: Option<String>) -> Self {
        self.details_hint = hint;
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
}
