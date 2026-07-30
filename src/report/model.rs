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
/// Runemark deliberately does not inspect the terminal size itself. Applications
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
#[non_exhaustive]
pub enum Trend {
    Positive,
    Negative,
    Neutral,
}

/// A status badge tag for findings, KPIs, or changes (e.g. `[QUICK WIN]`, `[GAINED]`).
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
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

    pub fn render(&self, console: crate::color::Console) -> String {
        console.paint(self.tone, format!("[{}]", self.label))
    }
}

/// A key-value metric item with optional trend and threshold coloring.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
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
#[non_exhaustive]
pub struct Finding {
    pub message: String,
    pub tone: Tone,
    pub location: Option<crate::location::Location>,
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

    pub(super) fn rendered_count(&self) -> usize {
        self.total_count.max(self.findings.len())
    }
}

/// A scope or completeness note (e.g. analysis scope warnings, unavailable history).
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
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
#[non_exhaustive]
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
}
