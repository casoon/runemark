//! Confirmation prompts.
//!
//! ```bash
//! cargo run --example confirm_demo --features select
//! ```
//!
//! Piping the input shows what a script gets: nothing is asked, and nothing is confirmed.
//!
//! ```bash
//! echo y | cargo run --example confirm_demo --features select
//! ```

use runemark::{Answer, Challenge, ColorMode, Confirm, Console, Verdict};

fn main() -> std::io::Result<()> {
    let console = Console::stderr(ColorMode::Auto);

    let answer = Confirm::new("Apply 12 changes?")
        .with_header(Verdict::Warning, "Approve plan 3f2a1c")
        .run(console)?;
    if answer != Answer::Confirmed {
        return report(answer);
    }

    let answer = Challenge::new("Type 3f2a1c to delete 12 notes:", "3f2a1c")
        .with_header(Verdict::Failed, "This cannot be undone")
        .run(console)?;
    report(answer)
}

fn report(answer: Answer) -> std::io::Result<()> {
    match answer {
        Answer::Confirmed => println!("confirmed"),
        Answer::Declined => println!("declined"),
        Answer::Unavailable => {
            eprintln!("Not confirmed: run this command in a terminal to approve.");
        }
    }
    Ok(())
}
