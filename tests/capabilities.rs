//! Colour, symbols and hyperlinks are independent capabilities of a `Console`.
//! None of these tests needs a real terminal.

use runemark::{ColorMode, Console, Location, SymbolTheme, Verdict};

const URL: &str = "https://example.com";
const OSC8: &str = "\x1b]8;;";

fn console(color: bool, symbols: SymbolTheme, hyperlinks: bool) -> Console {
    let mode = if color {
        ColorMode::Always
    } else {
        ColorMode::Never
    };
    Console::new(mode, false)
        .with_theme(symbols)
        .with_hyperlinks(hyperlinks)
}

#[test]
fn every_capability_combination_renders_independently() {
    for color in [false, true] {
        for symbols in [SymbolTheme::Unicode, SymbolTheme::Ascii] {
            for hyperlinks in [false, true] {
                let console = console(color, symbols, hyperlinks);
                let case = format!("color={color} symbols={symbols:?} hyperlinks={hyperlinks}");

                let verdict = Verdict::Passed.render(console, "ok");
                let symbol = if symbols == SymbolTheme::Ascii {
                    "[OK]"
                } else {
                    "✔"
                };
                assert!(verdict.contains(symbol), "{case}: {verdict:?}");
                assert_eq!(verdict.contains("\x1b["), color, "{case}: {verdict:?}");

                let link = Location::Url(URL.into()).render(console);
                assert_eq!(link.contains(OSC8), hyperlinks, "{case}: {link:?}");
                assert_eq!(link.contains("\x1b[36m"), color, "{case}: {link:?}");
                assert!(link.contains(URL), "{case}: {link:?}");
            }
        }
    }
}

#[test]
fn hyperlinks_without_colour_wrap_unstyled_text() {
    let console = Console::new(ColorMode::Never, false).with_hyperlinks(true);
    assert_eq!(
        Location::Url(URL.into()).render(console),
        format!("{OSC8}{URL}\x1b\\{URL}{OSC8}\x1b\\")
    );
}

#[test]
fn colour_without_hyperlinks_styles_plain_text() {
    let console = Console::new(ColorMode::Always, false).with_hyperlinks(false);
    assert_eq!(
        Location::Url(URL.into()).render(console),
        format!("\x1b[36m{URL}\x1b[0m")
    );
}

#[test]
fn existing_constructors_keep_their_output() {
    let always = Console::new(ColorMode::Always, false);
    assert_eq!(
        Location::Url(URL.into()).render(always),
        format!("{OSC8}{URL}\x1b\\\x1b[36m{URL}\x1b[0m{OSC8}\x1b\\")
    );
    assert_eq!(
        Verdict::Passed.render(always, "ok"),
        "\x1b[1;92m✔\x1b[0m ok"
    );

    let never = Console::new(ColorMode::Never, true);
    assert_eq!(Location::Url(URL.into()).render(never), URL);
    assert_eq!(Verdict::Passed.render(never, "ok"), "[OK] ok");
}
