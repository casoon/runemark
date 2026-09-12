use runemark::{
    Badge, ColorMode, Confidence, Console, DetailLevel, DiffBlock, ErrorBlock, FileAction,
    FileChange, Finding, FindingGroup, Location, Metric, NextStep, Report, Tone, Trend, Verdict,
};

#[test]
fn golden_console_verdicts_plain() {
    let console = Console::new(ColorMode::Never, false);
    assert_eq!(
        Verdict::Passed.render(console, "Build passed"),
        "[OK] Build passed"
    );
    assert_eq!(
        Verdict::Warning.render(console, "Check warnings"),
        "[WARN] Check warnings"
    );
    assert_eq!(
        Verdict::Failed.render(console, "Compilation failed"),
        "[FAIL] Compilation failed"
    );
    assert_eq!(
        Verdict::ActionRequired.render(console, "Action required"),
        "[ACT] Action required"
    );
    assert_eq!(
        Verdict::Skipped.render(console, "Tests skipped"),
        "[SKIP] Tests skipped"
    );
    assert_eq!(
        Verdict::Info.render(console, "Info note"),
        "[INFO] Info note"
    );
}

#[test]
fn golden_error_block_plain() {
    let console = Console::new(ColorMode::Never, false);
    let error = ErrorBlock::new("Package build failed")
        .with_explanation("A required build tool was not found in PATH.")
        .with_remedy("Install the required tool using your package manager:")
        .add_command("brew install rust")
        .add_command("rustup update");

    let expected = "\
[FAIL] Package build failed
  A required build tool was not found in PATH.

  Remedy: Install the required tool using your package manager:

  $ brew install rust
  $ rustup update
";
    assert_eq!(error.render(console), expected);
}

#[test]
fn golden_diff_block_plain() {
    let console = Console::new(ColorMode::Never, false);
    let diff = DiffBlock::new()
        .add_change(FileChange::new(FileAction::Added, "src/new_feature.rs"))
        .add_change(FileChange::new(FileAction::Modified, "src/lib.rs").with_delta("+12 -4 lines"))
        .add_change(FileChange::new(FileAction::Deleted, "legacy/old_mod.rs"))
        .add_change(FileChange::new(
            FileAction::Renamed,
            "src/utils.rs -> src/helpers.rs",
        ));

    let expected = "  + CREATE src/new_feature.rs\n  ~ UPDATE src/lib.rs (+12 -4 lines)\n  - DELETE legacy/old_mod.rs\n  -> RENAME src/utils.rs -> src/helpers.rs\n";
    assert_eq!(diff.render(console), expected);
}

#[test]
fn golden_report_compact_plain() {
    let console = Console::new(ColorMode::Never, false);
    let report = Report::new("Security & Quality Audit", Verdict::Warning)
        .add_metric(Metric::new("Issues", "2").with_tone(Tone::Warning))
        .add_metric(
            Metric::new("Coverage", "88%")
                .with_tone(Tone::Success)
                .with_trend(Trend::Positive, "+3%"),
        )
        .add_group(
            FindingGroup::new("Vulnerabilities")
                .add_finding(
                    Finding::new(Tone::Error, "SQL injection vulnerability")
                        .with_rule_id("security/sql-injection")
                        .with_location(Location::file_line("src/db.rs", 42))
                        .with_confidence(Confidence::High)
                        .with_badge(Badge::new("critical", Tone::Error)),
                )
                .add_finding(
                    Finding::new(Tone::Warning, "Hardcoded secret")
                        .with_rule_id("security/secret-leak")
                        .with_location(Location::file_line_col("src/auth.rs", 10, 5)),
                ),
        )
        .add_next_step(
            NextStep::new("Fix critical vulnerabilities").with_command("cargo audit fix"),
        );

    let rendered = report.render(console);

    let expected = "\
[WARN] Security & Quality Audit

  Issues: 2   Coverage: 88% (+3%)

* Vulnerabilities (2)
  - [critical] SQL injection vulnerability [security/sql-injection] (high confidence) at src/db.rs:42
  - Hardcoded secret [security/secret-leak] at src/auth.rs:10:5

Next steps:
  - Fix critical vulnerabilities
    $ cargo audit fix
";
    assert_eq!(rendered, expected);
}

#[test]
fn golden_report_detailed_plain() {
    let console = Console::new(ColorMode::Never, false);
    let report = Report::new("Full Report", Verdict::Passed)
        .with_detail_level(DetailLevel::Detailed)
        .add_metric(Metric::new("Passing", "100%"))
        .add_group(
            FindingGroup::new("Completed Checks")
                .with_advisory(true)
                .add_finding(Finding::new(Tone::Success, "All tests passed")),
        );

    let rendered = report.render(console);
    let expected = "\
[OK] Full Report

  Passing: 100%

* Completed Checks (1) [advisory]
  - All tests passed

";
    assert_eq!(rendered, expected);
}

#[test]
fn location_file_paths_unix_and_windows_formatting() {
    let unix_loc = Location::file_line_col("src/lib.rs", 10, 4);
    assert_eq!(unix_loc.to_plain_string(), "src/lib.rs:10:4");

    let special_loc = Location::file("path with spaces/and#special.rs");
    assert_eq!(
        special_loc.to_plain_string(),
        "path with spaces/and#special.rs"
    );

    let url_loc = Location::Url("https://example.com/path".into());
    assert_eq!(url_loc.to_plain_string(), "https://example.com/path");

    let selector_loc = Location::Selector(".btn-primary".into());
    assert_eq!(selector_loc.to_plain_string(), "`.btn-primary`");

    let artifact_loc = Location::Artifact("dist/bundle.js".into());
    assert_eq!(artifact_loc.to_plain_string(), "dist/bundle.js");
}

#[test]
fn public_renderers_neutralize_terminal_control_characters() {
    let console = Console::new(ColorMode::Never, false);
    let outputs = [
        Verdict::Warning.render(console, "Verdict\x1b[31m\rspoof"),
        ErrorBlock::new("Error\x07")
            .with_remedy("Retry\rnow")
            .render(console),
        DiffBlock::new()
            .add_change(FileChange::new(FileAction::Added, "unsafe\x1b[2J.rs"))
            .render(console),
        Report::new("Report\x1b[31m", Verdict::Info)
            .add_next_step(NextStep::new("Continue\rhidden"))
            .render(console),
    ];

    for output in outputs {
        assert!(
            !output
                .chars()
                .any(|ch| matches!(ch, '\x1b' | '\x07' | '\r'))
        );
    }
}
