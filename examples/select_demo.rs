//! Interactive selection.
//!
//! ```bash
//! cargo run --example select_demo --features select
//! ```
//!
//! Piping it shows the non-interactive path instead:
//!
//! ```bash
//! cargo run --example select_demo --features select | cat
//! ```

use std::io::IsTerminal;

use runemark::{ColorMode, Console, Group, Hint, Item, Menu, Outcome, SelectMode};

fn main() -> std::io::Result<()> {
    let console = Console::stderr(ColorMode::Auto);

    let menu = Menu::new()
        .with_heading("casoon.dev")
        .with_note("pnpm")
        .add_group(
            Group::new("Development")
                .add_item(Item::new("dev", "dev").with_description("Start the site"))
                .add_item(
                    Item::new("dev:landings", "dev:landings")
                        .with_description("Start the landing pages"),
                ),
        )
        .add_group(
            Group::new("Build")
                .add_item(Item::new("build", "build").with_description("Production build")),
        )
        .add_hint(Hint::new('U', "Updates"))
        .add_hint(Hint::new('C', "Clean"));

    let outcome = menu.run(console, SelectMode::Auto, std::io::stderr().is_terminal())?;

    match outcome {
        Outcome::Selected(id) => println!("selected: {id}"),
        Outcome::Hotkey(key) => println!("hotkey: {key}"),
        Outcome::Cancelled => println!("cancelled"),
        // No terminal: show the list once and leave.
        Outcome::Unavailable => print!("{}", menu.render(Console::stdout(ColorMode::Auto))),
    }

    Ok(())
}
