//! The scene behind the start page's hero: a progress run slow enough to
//! watch, then the report it ends in.
//!
//! Paced on purpose. `progress_demo` advances four times in a row and is over
//! before a recording has a frame to show it, which is right for an example
//! someone reads and wrong for one someone watches. Lines stay inside 64
//! columns so the recording is legible on a phone.

use runemark::{
    ColorMode, Console, Finding, FindingGroup, Metric, NextStep, ProgressMode, ProgressSink,
    Report, TerminalProgress, Tone, Verdict,
};
use std::io::IsTerminal;
use std::thread::sleep;
use std::time::Duration;

/// Long enough for a frame to land on every step, short enough for a hero.
const STEP: Duration = Duration::from_millis(420);

fn main() {
    let console = Console::stdout(ColorMode::Auto);
    let progress = TerminalProgress::stderr(
        ProgressMode::Auto,
        Console::stderr(ColorMode::Auto),
        std::io::stderr().is_terminal(),
    );

    let pages = [
        "https://example.com/",
        "https://example.com/pricing",
        "https://example.com/about",
        "https://example.com/contact",
    ];

    progress.start(pages.len() as u64, "Auditing pages");
    for (done, page) in pages.iter().enumerate() {
        sleep(STEP);
        progress.advance(done as u64 + 1, page);
    }
    sleep(STEP);
    progress.finish(Verdict::Warning, "4 pages audited");
    sleep(STEP);

    let report = Report::new("Site audit", Verdict::Warning)
        .add_metric(Metric::new("Errors", "0"))
        .add_metric(Metric::new("Warnings", "2"))
        .add_metric(Metric::new("Score", "94/100"))
        .add_group(
            FindingGroup::new("Accessibility")
                .add_finding(
                    Finding::new(Tone::Warning, "Image missing alt attribute")
                        .with_rule_id("a11y/img-alt"),
                )
                .add_finding(
                    Finding::new(Tone::Warning, "Low contrast on hero button")
                        .with_rule_id("a11y/contrast"),
                ),
        )
        .add_next_step(NextStep::new("Run the auto-fixer").with_command("audit --fix"));

    print!("{}", report.render(console));
}
