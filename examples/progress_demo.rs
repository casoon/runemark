use runemark::{ColorMode, Console, ProgressMode, ProgressSink, TerminalProgress, Verdict};
use std::io::IsTerminal;

fn main() {
    let progress = TerminalProgress::stderr(
        ProgressMode::Auto,
        Console::stderr(ColorMode::Auto),
        std::io::stderr().is_terminal(),
    );

    progress.start(3, "Auditing pages");
    progress.advance(1, "https://example.com/");
    progress.advance(2, "https://example.com/about");
    progress.advance(3, "https://example.com/contact");
    progress.finish(Verdict::Passed, "Audit complete");
}
