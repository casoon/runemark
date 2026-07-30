use runemark::{
    ColorMode, Console, DetailLevel, DiffBlock, ErrorBlock, FileAction, FileChange, Finding,
    FindingGroup, Location, Metric, NextStep, Report, ScopeNote, Verdict,
};

fn main() {
    let console = Console::stdout(ColorMode::Auto);

    println!("=== 1. Verdict & Status ===");
    println!(
        "{}",
        Verdict::Passed.render(console, "Configuration validated cleanly")
    );
    println!(
        "{}",
        Verdict::Warning.render(console, "3 non-critical findings detected")
    );
    println!(
        "{}",
        Verdict::Failed.render(console, "Build failed: 2 errors")
    );
    println!();

    println!("=== 2. Error / Prerequisite Block ===");
    let err_block = ErrorBlock::new("Required toolchain is unavailable")
        .with_explanation("The configured version is not installed.")
        .with_remedy("Install the required toolchain:")
        .add_command("toolchain install stable");
    print!("{}", err_block.render(console));

    println!("=== 3. Decision-Oriented Report (Compact) ===");
    let report = Report::new("Site audit v1.2.0", Verdict::Warning)
        .add_scope_note(ScopeNote::new(
            "Scope warnings",
            vec![
                "Git history partial".to_string(),
                "1 file skipped".to_string(),
            ],
        ))
        .add_metric(Metric::new("Errors", "0"))
        .add_metric(Metric::new("Warnings", "4").with_trend(runemark::Trend::Negative, "+2"))
        .add_metric(Metric::new("Score", "94/100").with_trend(runemark::Trend::Positive, "+5%"))
        .add_group(
            FindingGroup::new("Accessibility Violations")
                .add_finding(
                    Finding::new(runemark::Tone::Warning, "Image missing alt attribute")
                        .with_rule_id("a11y/img-alt")
                        .with_badge(runemark::Badge::quick_win())
                        .with_confidence(runemark::Confidence::High)
                        .with_location(Location::file_line_col("src/pages/index.astro", 42, 10))
                        .with_remedy("Add alt=\"...\" description to <img> tag"),
                )
                .add_finding(
                    Finding::new(runemark::Tone::Warning, "Low contrast ratio on hero button")
                        .with_rule_id("a11y/contrast")
                        .with_location(Location::file_line("src/components/Hero.astro", 18)),
                ),
        )
        .add_next_step(
            NextStep::new("Run auto-fixer for formatting issues").with_command("audit --fix"),
        );

    print!("{}", report.render(console));

    println!("=== 4. Detailed Report View ===");
    let detailed_report = report.with_detail_level(DetailLevel::Detailed);
    print!("{}", detailed_report.render(console));

    println!("=== 5. Generator Diff View ===");
    let diff = DiffBlock::new()
        .add_change(
            FileChange::new(FileAction::Added, "src/components/Footer.astro").with_delta("+1.2 kB"),
        )
        .add_change(FileChange::new(
            FileAction::Modified,
            "src/layouts/Layout.astro",
        ))
        .add_change(FileChange::new(
            FileAction::Deleted,
            "src/legacy/Footer.jsx",
        ));
    print!("{}", diff.render(console));
}
