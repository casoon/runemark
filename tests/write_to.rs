//! `write_to` streams the same bytes that `render` returns.

use runemark::{Badge, ColorMode, Console, DiffBlock, FileAction, FileChange, Location};
use std::io::Write;

fn consoles() -> [Console; 2] {
    [
        Console::new(ColorMode::Never, false),
        Console::new(ColorMode::Always, false),
    ]
}

#[test]
fn diff_block_write_to_matches_render() {
    let diff = DiffBlock::new()
        .add_change(FileChange::new(FileAction::Added, "a.rs").with_delta("+1 kB"))
        .add_change(FileChange::new(FileAction::Renamed, "b.rs"));
    for console in consoles() {
        let mut buf = Vec::new();
        diff.write_to(console, &mut buf).unwrap();
        assert_eq!(String::from_utf8(buf).unwrap(), diff.render(console));
    }
}

#[test]
fn location_write_to_matches_render() {
    let locations = [
        Location::file_line_col("src/lib.rs", 3, 4),
        Location::Url("https://example.com".into()),
        Location::Selector("main > p".into()),
        Location::Artifact("dist/out.js".into()),
    ];
    for console in consoles() {
        for location in &locations {
            let mut buf = Vec::new();
            location.write_to(console, &mut buf).unwrap();
            assert_eq!(String::from_utf8(buf).unwrap(), location.render(console));
        }
    }
}

#[test]
fn badge_write_to_matches_render_through_a_dyn_writer() {
    let badge = Badge::quick_win();
    for console in consoles() {
        let mut buf = Vec::new();
        let writer: &mut dyn Write = &mut buf;
        badge.write_to(console, writer).unwrap();
        assert_eq!(String::from_utf8(buf).unwrap(), badge.render(console));
    }
}
