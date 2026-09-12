//! Rendering of the report model to plain and ANSI-styled terminal output.

use super::model::{DetailLevel, Finding, FindingGroup, RenderOptions, Report, Trend, Vocabulary};
use crate::color::{Console, SymbolTheme, Tone};
use crate::internal::{write_toned_line, write_toned_suffix};
use std::collections::BTreeMap;
use std::io::Write;
use unicode_width::UnicodeWidthStr;

impl Report {
    /// Writes the report formatted directly to a writer.
    pub fn write_to(
        &self,
        console: Console,
        writer: &mut (impl Write + ?Sized),
    ) -> std::io::Result<()> {
        self.write_with_options(console, RenderOptions::default(), writer)
    }

    /// Writes the report with host-provided layout constraints.
    pub fn write_with_options(
        &self,
        console: Console,
        options: RenderOptions,
        writer: &mut (impl Write + ?Sized),
    ) -> std::io::Result<()> {
        self.write_header(console, writer)?;
        self.write_metrics(console, options, writer)?;
        self.write_top_issues(console, writer)?;
        self.write_groups(console, options, writer)?;
        self.write_next_steps(console, writer)?;
        Ok(())
    }

    /// Renders the report as a formatted String.
    pub fn render(&self, console: Console) -> String {
        crate::internal::collect_to_string(|buf| self.write_to(console, buf))
    }

    /// Renders the report with host-provided layout constraints.
    pub fn render_with_options(&self, console: Console, options: RenderOptions) -> String {
        crate::internal::collect_to_string(|buf| self.write_with_options(console, options, buf))
    }

    /// Verdict line, vocabulary badges, and scope notes.
    fn write_header(
        &self,
        console: Console,
        writer: &mut (impl Write + ?Sized),
    ) -> std::io::Result<()> {
        self.verdict.write_to(console, &self.title, writer)?;
        writeln!(writer)?;

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
            write_toned_line("  ", console, Tone::Muted, note_items.join(" • "), writer)?;
        }

        for note in &self.scope_notes {
            write!(writer, "  ")?;
            console.write_paint(Tone::Warning, format!("{}: ", note.heading), writer)?;
            console.write_paint(Tone::Muted, note.items.join(", "), writer)?;
            writeln!(writer)?;
        }
        writeln!(writer)
    }

    /// Summary metrics grid.
    fn write_metrics(
        &self,
        console: Console,
        options: RenderOptions,
        writer: &mut (impl Write + ?Sized),
    ) -> std::io::Result<()> {
        if self.metrics.is_empty() {
            return Ok(());
        }

        let inline_width = 2
            + self.metrics.iter().map(metric_display_width).sum::<usize>()
            + 3 * self.metrics.len().saturating_sub(1);
        let stack_metrics = options.width().is_some_and(|width| inline_width > width);

        if stack_metrics {
            for metric in &self.metrics {
                write!(writer, "  ")?;
                write_metric(metric, console, writer)?;
                writeln!(writer)?;
            }
        } else {
            write!(writer, "  ")?;
            for (idx, metric) in self.metrics.iter().enumerate() {
                if idx > 0 {
                    write!(writer, "   ")?;
                }
                write_metric(metric, console, writer)?;
            }
            writeln!(writer)?;
        }
        writeln!(writer)
    }

    /// Aggregated "top issue rules" summary, shown once the finding count crosses
    /// `top_issues_threshold`.
    fn write_top_issues(
        &self,
        console: Console,
        writer: &mut (impl Write + ?Sized),
    ) -> std::io::Result<()> {
        let total_finding_count: usize = self.groups.iter().map(FindingGroup::rendered_count).sum();
        let Some(thresh) = self.top_issues_threshold else {
            return Ok(());
        };
        if total_finding_count < thresh {
            return Ok(());
        }

        let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
        for g in &self.groups {
            for f in &g.findings {
                if let Some(ref r) = f.rule_id {
                    *counts.entry(r.as_str()).or_insert(0) += 1;
                }
            }
        }
        if counts.is_empty() {
            return Ok(());
        }

        write_toned_line("  ", console, Tone::Title, "Top issue rules:", writer)?;
        let mut sorted: Vec<(&str, usize)> = counts.into_iter().collect();
        sorted.sort_by_key(|(rule, count)| (std::cmp::Reverse(*count), *rule));
        for (rule, count) in sorted.into_iter().take(5) {
            write_toned_line(
                &format!("    {count:>4}x  "),
                console,
                Tone::Muted,
                rule,
                writer,
            )?;
        }
        writeln!(writer)
    }

    /// Finding groups, each with a bounded compact sample or every finding
    /// provided by the report data in detailed mode.
    fn write_groups(
        &self,
        console: Console,
        options: RenderOptions,
        writer: &mut (impl Write + ?Sized),
    ) -> std::io::Result<()> {
        for group in &self.groups {
            if group.rendered_count() > 0 {
                self.write_group(group, console, options, writer)?;
            }
        }
        Ok(())
    }

    /// A single finding group: header line, bounded finding sample, and an
    /// omitted-count notice when the compact sample doesn't cover every finding.
    fn write_group(
        &self,
        group: &FindingGroup,
        console: Console,
        options: RenderOptions,
        writer: &mut (impl Write + ?Sized),
    ) -> std::io::Result<()> {
        let total_count = group.rendered_count();

        let group_marker = match console.symbol_theme() {
            SymbolTheme::Unicode => "●",
            SymbolTheme::Ascii => "*",
        };
        write!(writer, "{group_marker} ")?;
        console.write_paint(Tone::Title, &group.title, writer)?;
        write!(writer, " ")?;
        console.write_paint(Tone::Muted, format!("({total_count})"), writer)?;
        if group.advisory {
            write_toned_suffix(console, Tone::Info, "[advisory]", writer)?;
        }
        writeln!(writer)?;

        let sample_limit = match self.detail_level {
            DetailLevel::Compact => self.max_compact_samples,
            DetailLevel::Detailed => usize::MAX,
        };

        for f in group.findings.iter().take(sample_limit) {
            write_finding(f, console, self.detail_level, options.width(), writer)?;
        }

        let rendered_finding_count = group.findings.len().min(sample_limit);
        if total_count > rendered_finding_count {
            let remaining = total_count - rendered_finding_count;
            let message = match self.detail_level {
                DetailLevel::Compact => {
                    format!("... {remaining} more finding(s) (use --details to view all)")
                }
                DetailLevel::Detailed => {
                    format!("... {remaining} finding(s) not included in the report data")
                }
            };
            write_toned_line("  ", console, Tone::Muted, message, writer)?;
        }

        writeln!(writer)
    }

    /// Next-step recommendations, each with an optional copyable command.
    fn write_next_steps(
        &self,
        console: Console,
        writer: &mut (impl Write + ?Sized),
    ) -> std::io::Result<()> {
        if self.next_steps.is_empty() {
            return Ok(());
        }

        console.write_paint(Tone::Title, "Next steps:", writer)?;
        writeln!(writer)?;
        let bullet = match console.symbol_theme() {
            SymbolTheme::Unicode => "•",
            SymbolTheme::Ascii => "-",
        };
        for step in &self.next_steps {
            writeln!(
                writer,
                "  {bullet} {}",
                crate::color::sanitize_visible_text(&step.text)
            )?;
            if let Some(ref cmd) = step.command {
                write_toned_line("    ", console, Tone::Info, format!("$ {cmd}"), writer)?;
            }
        }
        Ok(())
    }
}

/// Hanging indent for wrapped finding lines.
const CONTINUATION: &str = "    ";

/// A single finding line: message, optional badge/rule/confidence/location,
/// and (in detailed mode) an indented remedy line.
fn write_finding(
    f: &Finding,
    console: Console,
    detail_level: DetailLevel,
    width: Option<usize>,
    writer: &mut (impl Write + ?Sized),
) -> std::io::Result<()> {
    write!(writer, "  - ")?;
    let mut message_start_width = display_width("  - ");
    if let Some(ref b) = f.badge {
        write!(writer, "{} ", b.render(console))?;
        message_start_width += display_width(&format!("[{}] ", b.label));
    }

    let mut line_width = if let Some(width) = width {
        write_wrapped_toned_text(
            console,
            f.tone,
            &f.message,
            width,
            message_start_width,
            CONTINUATION,
            writer,
        )?
    } else {
        console.write_paint(f.tone, &f.message, writer)?;
        message_start_width + display_width(&f.message)
    };

    if let Some(ref r) = f.rule_id {
        let rule = format!("[{r}]");
        line_width = start_suffix(line_width, display_width(&rule), width, writer)?;
        console.write_paint(Tone::Muted, rule, writer)?;
    }

    if let Some(ref conf) = f.confidence {
        let confidence = format!("({})", conf.label());
        line_width = start_suffix(line_width, display_width(&confidence), width, writer)?;
        console.write_paint(Tone::Muted, confidence, writer)?;
    }

    if let Some(ref loc) = f.location {
        let location_width = display_width("at ") + display_width(&loc.to_plain_string());
        start_suffix(line_width, location_width, width, writer)?;
        write!(writer, "at {}", loc.render(console))?;
    }
    writeln!(writer)?;

    if detail_level == DetailLevel::Detailed {
        if let Some(ref rem) = f.remedy {
            write_toned_line(
                "    ",
                console,
                Tone::Muted,
                format!("Remedy: {rem}"),
                writer,
            )?;
        }
    }

    Ok(())
}

fn write_metric(
    metric: &super::model::Metric,
    console: Console,
    writer: &mut (impl Write + ?Sized),
) -> std::io::Result<()> {
    console.write_paint(Tone::Muted, format!("{}: ", metric.key), writer)?;
    console.write_paint(metric.tone.unwrap_or(Tone::Info), &metric.value, writer)?;
    if let Some(ref delta) = metric.delta {
        write!(writer, " ")?;
        let trend_tone = match metric.trend {
            Some(Trend::Positive) => Tone::Success,
            Some(Trend::Negative) => Tone::Error,
            _ => Tone::Muted,
        };
        console.write_paint(trend_tone, format!("({delta})"), writer)?;
    }
    Ok(())
}

fn metric_display_width(metric: &super::model::Metric) -> usize {
    let delta_width = metric
        .delta
        .as_deref()
        .map(|delta| display_width(delta) + 3)
        .unwrap_or_default();
    display_width(&metric.key) + 2 + display_width(&metric.value) + delta_width
}

/// Starts a same-line suffix `suffix_width` columns wide: a space, or, when a width is
/// known and the suffix would not fit, a line break with the hanging indent.
/// Returns the line width after the suffix.
fn start_suffix(
    line_width: usize,
    suffix_width: usize,
    width: Option<usize>,
    writer: &mut (impl Write + ?Sized),
) -> std::io::Result<usize> {
    let continuation_width = display_width(CONTINUATION);
    match width {
        Some(width) if width > continuation_width && line_width + 1 + suffix_width > width => {
            writeln!(writer)?;
            write!(writer, "{CONTINUATION}")?;
            Ok(continuation_width + suffix_width)
        }
        _ => {
            write!(writer, " ")?;
            Ok(line_width + 1 + suffix_width)
        }
    }
}

/// Writes `text` word by word, wrapping at `width` with the `continuation` indent.
/// Returns the width of the last line written.
fn write_wrapped_toned_text(
    console: Console,
    tone: Tone,
    text: &str,
    width: usize,
    first_line_width: usize,
    continuation: &str,
    writer: &mut (impl Write + ?Sized),
) -> std::io::Result<usize> {
    let continuation_width = display_width(continuation);
    if width <= continuation_width || first_line_width >= width {
        console.write_paint(tone, text, writer)?;
        return Ok(first_line_width + display_width(text));
    }

    let mut line_width = first_line_width;
    let mut first_word = true;
    for word in text.split_whitespace() {
        let word_width = display_width(word);
        let separator_width = usize::from(!first_word);
        if !first_word && line_width + separator_width + word_width > width {
            writeln!(writer)?;
            write!(writer, "{continuation}")?;
            line_width = continuation_width;
            first_word = true;
        }
        if !first_word {
            write!(writer, " ")?;
            line_width += 1;
        }
        console.write_paint(tone, word, writer)?;
        line_width += word_width;
        first_word = false;
    }
    Ok(line_width)
}

fn display_width(text: &str) -> usize {
    UnicodeWidthStr::width(text)
}

#[cfg(test)]
mod tests {
    use super::super::model::{Finding, FindingGroup, Metric, RenderOptions, Report};
    use crate::color::{ColorMode, Console, Tone};
    use crate::verdict::Verdict;
    use std::io::Write;

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
    fn detailed_report_marks_findings_missing_from_the_report_data() {
        let report = Report::new("Audit", Verdict::Warning)
            .with_detail_level(super::super::model::DetailLevel::Detailed)
            .add_group(
                FindingGroup::new("Warnings")
                    .with_total_count(2)
                    .add_finding(Finding::new(Tone::Warning, "Representative warning")),
            );
        let console = Console::new(ColorMode::Never, false);

        let output = report.render(console);

        assert!(output.contains("- Representative warning"));
        assert!(output.contains("... 1 finding(s) not included in the report data"));
    }

    #[test]
    fn narrow_layout_stacks_metrics() {
        let report = Report::new("Audit", Verdict::Warning)
            .add_metric(Metric::new("Errors", "0"))
            .add_metric(Metric::new("Warnings", "12"));
        let console = Console::new(ColorMode::Never, false);

        let output = report.render_with_options(console, RenderOptions::new().with_width(20));

        assert!(output.contains("  Errors: 0\n  Warnings: 12\n"));
    }

    #[test]
    fn narrow_layout_wraps_finding_messages_with_a_hanging_indent() {
        let report = Report::new("Audit", Verdict::Warning).add_group(
            FindingGroup::new("Warnings")
                .add_finding(Finding::new(Tone::Warning, "This message needs wrapping")),
        );
        let console = Console::new(ColorMode::Never, false);

        let output = report.render_with_options(console, RenderOptions::new().with_width(20));

        assert!(output.contains("  - This message\n    needs wrapping"));
    }

    #[test]
    fn narrow_layout_wraps_finding_metadata_with_a_hanging_indent() {
        let report = Report::new("Audit", Verdict::Warning).add_group(
            FindingGroup::new("Images").add_finding(
                Finding::new(Tone::Warning, "Image missing alt")
                    .with_rule_id("a11y/img-alt")
                    .with_confidence(crate::Confidence::High)
                    .with_location(crate::Location::file_line_col(
                        "src/pages/index.astro",
                        42,
                        10,
                    )),
            ),
        );
        let console = Console::new(ColorMode::Never, false);

        let output = report.render_with_options(console, RenderOptions::new().with_width(40));

        assert!(output.contains(
            "  - Image missing alt [a11y/img-alt]\n    (high confidence)\n    at src/pages/index.astro:42:10\n"
        ));
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

    #[test]
    fn max_compact_samples_boundary_testing() {
        let make_report = |limit: usize| {
            let mut group = FindingGroup::new("Issues");
            for i in 0..5 {
                group = group.add_finding(Finding::new(Tone::Warning, format!("Issue {i}")));
            }
            let mut r = Report::new("Audit", Verdict::Warning).add_group(group);
            r.max_compact_samples = limit;
            r
        };

        let console = Console::new(ColorMode::Never, false);

        // Below count (4 of 5)
        let out_4 = make_report(4).render(console);
        assert!(out_4.contains("... 1 more finding(s) (use --details to view all)"));
        assert!(out_4.contains("- Issue 3"));
        assert!(!out_4.contains("- Issue 4"));

        // Exactly at count (5 of 5)
        let out_5 = make_report(5).render(console);
        assert!(!out_5.contains("more finding(s)"));
        assert!(out_5.contains("- Issue 4"));

        // Above count (6 of 5)
        let out_6 = make_report(6).render(console);
        assert!(!out_6.contains("more finding(s)"));
        assert!(out_6.contains("- Issue 4"));
    }

    #[test]
    fn top_issues_threshold_boundary_testing() {
        let make_report = |count: usize| {
            let mut group = FindingGroup::new("Issues");
            for i in 0..count {
                group = group.add_finding(
                    Finding::new(Tone::Warning, format!("Issue {i}")).with_rule_id("common-rule"),
                );
            }
            Report::new("Audit", Verdict::Warning).add_group(group)
        };

        let console = Console::new(ColorMode::Never, false);

        // 19 findings (below threshold 20) -> no top issues
        let out_19 = make_report(19).render(console);
        assert!(!out_19.contains("Top issue rules:"));

        // 20 findings (at threshold 20) -> top issues shown
        let out_20 = make_report(20).render(console);
        assert!(out_20.contains("Top issue rules:"));
        assert!(out_20.contains("common-rule"));

        // 21 findings (above threshold 20) -> top issues shown
        let out_21 = make_report(21).render(console);
        assert!(out_21.contains("Top issue rules:"));
    }

    #[test]
    fn unicode_width_and_overlong_word_handling() {
        let console = Console::new(ColorMode::Never, false);

        // CJK characters take 2 columns each
        let report = Report::new("Audit", Verdict::Info).add_group(
            FindingGroup::new("CJK").add_finding(Finding::new(Tone::Info, "你好世界测试")),
        );
        let out = report.render_with_options(console, RenderOptions::new().with_width(10));
        assert!(out.contains("你好世界测试") || out.contains("你好"));

        // Single overlong word exceeds available width without panic or infinite loop
        let long_word = "SupercalifragilisticexpialidociousAndEvenLongerThanTerminalWidth";
        let report_long = Report::new("Audit", Verdict::Warning).add_group(
            FindingGroup::new("Overlong").add_finding(Finding::new(Tone::Warning, long_word)),
        );
        let out_long =
            report_long.render_with_options(console, RenderOptions::new().with_width(20));
        assert!(out_long.contains(long_word));
    }
}
