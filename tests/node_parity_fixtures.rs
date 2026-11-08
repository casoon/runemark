//! Fixtures that tie the Node package to the Rust renderer.
//!
//! Every case builds a report, error block or diff with the Rust API, renders it
//! with a fixed console, and compares the result with a committed JSON file under
//! `tests/fixtures/node-parity/`. Each file also holds the matching Node input
//! object and render options, so `packages/runemark/test/fixtures.test.mjs` can
//! feed the same data through the Node API and require identical bytes.
//!
//! Review a changed fixture like a golden file, then rewrite them with:
//!
//! ```text
//! RUNEMARK_BLESS=1 cargo test --test node_parity_fixtures
//! ```

use std::fmt::Write as _;
use std::fs;
use std::path::PathBuf;

use runemark::{
    Badge, ColorMode, Confidence, Console, DetailLevel, DiffBlock, ErrorBlock, FileAction,
    FileChange, Finding, FindingGroup, Location, Metric, NextStep, RenderOptions, Report,
    ScopeNote, SymbolTheme, Tone, Trend, Verdict, Vocabulary,
};

/// Just enough JSON to write the fixtures without a dependency.
#[derive(Clone)]
enum Json {
    Str(String),
    Num(u64),
    Bool(bool),
    Arr(Vec<Json>),
    Obj(Vec<(&'static str, Json)>),
}

fn s(value: &str) -> Json {
    Json::Str(value.to_string())
}

fn obj(fields: Vec<(&'static str, Json)>) -> Json {
    Json::Obj(fields)
}

fn strs(values: &[&str]) -> Json {
    Json::Arr(values.iter().map(|value| s(value)).collect())
}

impl Json {
    fn write(&self, out: &mut String, depth: usize) {
        match self {
            Json::Str(value) => write_string(out, value),
            Json::Num(value) => write!(out, "{value}").unwrap(),
            Json::Bool(value) => write!(out, "{value}").unwrap(),
            Json::Arr(items) if items.is_empty() => out.push_str("[]"),
            Json::Arr(items) => {
                out.push_str("[\n");
                for (index, item) in items.iter().enumerate() {
                    indent(out, depth + 1);
                    item.write(out, depth + 1);
                    out.push_str(if index + 1 < items.len() { ",\n" } else { "\n" });
                }
                indent(out, depth);
                out.push(']');
            }
            Json::Obj(fields) => {
                out.push_str("{\n");
                for (index, (key, value)) in fields.iter().enumerate() {
                    indent(out, depth + 1);
                    write_string(out, key);
                    out.push_str(": ");
                    value.write(out, depth + 1);
                    out.push_str(if index + 1 < fields.len() {
                        ",\n"
                    } else {
                        "\n"
                    });
                }
                indent(out, depth);
                out.push('}');
            }
        }
    }
}

fn indent(out: &mut String, depth: usize) {
    for _ in 0..depth {
        out.push_str("  ");
    }
}

fn write_string(out: &mut String, value: &str) {
    out.push('"');
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            ch if (ch as u32) < 0x20 || ch as u32 == 0x7f => {
                write!(out, "\\u{:04x}", ch as u32).unwrap();
            }
            ch => out.push(ch),
        }
    }
    out.push('"');
}

/// The render options of one case, expressed once for both languages.
#[derive(Clone, Copy)]
struct Opts {
    colour: bool,
    unicode: bool,
    width: Option<u64>,
    hyperlinks: Option<bool>,
}

const PLAIN: Opts = Opts {
    colour: false,
    unicode: false,
    width: None,
    hyperlinks: None,
};

impl Opts {
    fn console(self) -> Console {
        let mode = if self.colour {
            ColorMode::Always
        } else {
            ColorMode::Never
        };
        let mut console = Console::new(mode, self.colour).with_theme(if self.unicode {
            SymbolTheme::Unicode
        } else {
            SymbolTheme::Ascii
        });
        if let Some(hyperlinks) = self.hyperlinks {
            console = console.with_hyperlinks(hyperlinks);
        }
        console
    }

    fn layout(self) -> RenderOptions {
        match self.width {
            Some(width) => RenderOptions::new().with_width(width as usize),
            None => RenderOptions::new(),
        }
    }

    fn json(self, with_width: bool) -> Json {
        let mut fields = vec![
            ("color", s(if self.colour { "always" } else { "never" })),
            ("symbols", s(if self.unicode { "unicode" } else { "ascii" })),
            ("isTerminal", Json::Bool(self.colour)),
        ];
        if let Some(hyperlinks) = self.hyperlinks {
            fields.push(("hyperlinks", Json::Bool(hyperlinks)));
        }
        if let (true, Some(width)) = (with_width, self.width) {
            fields.push(("width", Json::Num(width)));
        }
        obj(fields)
    }
}

struct Case {
    name: &'static str,
    /// The Node function that renders the input.
    function: &'static str,
    input: Json,
    options: Json,
    expected: String,
}

fn report_case(name: &'static str, input: Json, report: Report, opts: Opts) -> Case {
    Case {
        name,
        function: "renderReport",
        input,
        options: opts.json(true),
        expected: report.render_with_options(opts.console(), opts.layout()),
    }
}

fn base(title: &str, verdict: &str, mut fields: Vec<(&'static str, Json)>) -> Json {
    let mut all = vec![
        ("schemaVersion", Json::Num(1)),
        ("title", s(title)),
        ("verdict", s(verdict)),
    ];
    all.append(&mut fields);
    obj(all)
}

fn group_json(title: &str, mut fields: Vec<(&'static str, Json)>, findings: Vec<Json>) -> Json {
    let mut all = vec![("title", s(title))];
    all.append(&mut fields);
    all.push(("findings", Json::Arr(findings)));
    obj(all)
}

fn file_json(path: &str, line: Option<u64>, column: Option<u64>) -> Json {
    let mut fields = vec![("kind", s("file")), ("value", s(path))];
    if let Some(line) = line {
        fields.push(("line", Json::Num(line)));
    }
    if let Some(column) = column {
        fields.push(("column", Json::Num(column)));
    }
    obj(fields)
}

fn url_json(url: &str) -> Json {
    obj(vec![("kind", s("url")), ("value", s(url))])
}

/// Five findings with a remedy each, the shape the overflow cases share.
fn five_findings() -> (FindingGroup, Vec<Json>) {
    let mut group = FindingGroup::new("G");
    let mut json = Vec::new();
    for index in 1..=5u64 {
        group = group.add_finding(
            Finding::new(Tone::Warning, format!("Finding {index}"))
                .with_rule_id(format!("rule/{index}"))
                .with_remedy(format!("Fix {index}"))
                .with_location(Location::file_line("a.rs", index as usize)),
        );
        json.push(obj(vec![
            ("message", s(&format!("Finding {index}"))),
            ("tone", s("warning")),
            ("ruleId", s(&format!("rule/{index}"))),
            ("remedy", s(&format!("Fix {index}"))),
            ("location", file_json("a.rs", Some(index), None)),
        ]));
    }
    (group, json)
}

fn audit_report() -> (Json, Report) {
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
    let input = base(
        "Security & Quality Audit",
        "warning",
        vec![
            (
                "metrics",
                Json::Arr(vec![
                    obj(vec![
                        ("key", s("Issues")),
                        ("value", s("2")),
                        ("tone", s("warning")),
                    ]),
                    obj(vec![
                        ("key", s("Coverage")),
                        ("value", s("88%")),
                        ("tone", s("success")),
                        ("trend", s("positive")),
                        ("delta", s("+3%")),
                    ]),
                ]),
            ),
            (
                "groups",
                Json::Arr(vec![group_json(
                    "Vulnerabilities",
                    vec![],
                    vec![
                        obj(vec![
                            ("message", s("SQL injection vulnerability")),
                            ("tone", s("error")),
                            ("ruleId", s("security/sql-injection")),
                            ("location", file_json("src/db.rs", Some(42), None)),
                            ("confidence", s("high")),
                            (
                                "badge",
                                obj(vec![("label", s("critical")), ("tone", s("error"))]),
                            ),
                        ]),
                        obj(vec![
                            ("message", s("Hardcoded secret")),
                            ("tone", s("warning")),
                            ("ruleId", s("security/secret-leak")),
                            ("location", file_json("src/auth.rs", Some(10), Some(5))),
                        ]),
                    ],
                )]),
            ),
            (
                "nextSteps",
                Json::Arr(vec![obj(vec![
                    ("text", s("Fix critical vulnerabilities")),
                    ("command", s("cargo audit fix")),
                ])]),
            ),
        ],
    );
    (input, report)
}

fn cases() -> Vec<Case> {
    let mut cases = Vec::new();

    // Compact report with metrics, trend, badge, confidence, locations, next steps.
    let (input, report) = audit_report();
    cases.push(report_case("report-compact", input, report, PLAIN));

    // The same report with Unicode symbols and with ANSI colour.
    let (input, report) = audit_report();
    cases.push(report_case(
        "report-unicode",
        input,
        report,
        Opts {
            unicode: true,
            ..PLAIN
        },
    ));
    let (input, report) = audit_report();
    cases.push(report_case(
        "report-colour",
        input,
        report,
        // Links off: a file link embeds the working directory.
        Opts {
            colour: true,
            unicode: true,
            hyperlinks: Some(false),
            ..PLAIN
        },
    ));

    // Detailed level shows every finding with its remedy.
    let (group, findings) = five_findings();
    let report = Report::new("T", Verdict::Warning)
        .with_detail_level(DetailLevel::Detailed)
        .add_group(group);
    let input = base(
        "T",
        "warning",
        vec![
            ("detailLevel", s("detailed")),
            ("groups", Json::Arr(vec![group_json("G", vec![], findings)])),
        ],
    );
    cases.push(report_case("report-detailed", input, report, PLAIN));

    // Metric verdicts, tones and every trend, with and without a delta.
    let report = Report::new("Audit", Verdict::Failed)
        .add_metric(Metric::new("Errors", "3").with_verdict(Verdict::Failed))
        .add_metric(Metric::new("Warnings", "0").with_verdict(Verdict::Passed))
        .add_metric(Metric::new("Skipped", "2").with_verdict(Verdict::Skipped))
        .add_metric(
            Metric::new("Score", "71")
                .with_verdict(Verdict::Warning)
                .with_trend(Trend::Negative, "-4"),
        )
        .add_metric(Metric::new("Pages", "40").with_trend(Trend::Neutral, "+0"))
        .add_metric(Metric::new("Plain", "x"));
    let metric = |key: &str, value: &str, extra: Vec<(&'static str, Json)>| {
        let mut fields = vec![("key", s(key)), ("value", s(value))];
        fields.extend(extra);
        obj(fields)
    };
    let input = base(
        "Audit",
        "failed",
        vec![(
            "metrics",
            Json::Arr(vec![
                metric("Errors", "3", vec![("verdict", s("failed"))]),
                metric("Warnings", "0", vec![("verdict", s("passed"))]),
                metric("Skipped", "2", vec![("verdict", s("skipped"))]),
                metric(
                    "Score",
                    "71",
                    vec![
                        ("verdict", s("warning")),
                        ("trend", s("negative")),
                        ("delta", s("-4")),
                    ],
                ),
                metric(
                    "Pages",
                    "40",
                    vec![("trend", s("neutral")), ("delta", s("+0"))],
                ),
                metric("Plain", "x", vec![]),
            ]),
        )],
    );
    cases.push(report_case("report-metrics", input, report, PLAIN));

    // Scope notes and every vocabulary kind.
    let report = Report::new("T", Verdict::Warning)
        .add_scope_note(ScopeNote::new(
            "Scope",
            vec!["src only".to_string(), "no history".to_string()],
        ))
        .add_vocabulary(Vocabulary::DryRun)
        .add_vocabulary(Vocabulary::Baseline("main".to_string()))
        .add_vocabulary(Vocabulary::EmptyState("Nothing new".to_string()))
        .add_vocabulary(Vocabulary::Truncated(50));
    let input = base(
        "T",
        "warning",
        vec![
            (
                "scopeNotes",
                Json::Arr(vec![obj(vec![
                    ("heading", s("Scope")),
                    ("items", strs(&["src only", "no history"])),
                ])]),
            ),
            (
                "vocabulary",
                Json::Arr(vec![
                    obj(vec![("kind", s("dry-run"))]),
                    obj(vec![("kind", s("baseline")), ("value", s("main"))]),
                    obj(vec![
                        ("kind", s("empty-state")),
                        ("value", s("Nothing new")),
                    ]),
                    obj(vec![("kind", s("truncated")), ("value", Json::Num(50))]),
                ]),
            ),
        ],
    );
    cases.push(report_case("report-scope-vocabulary", input, report, PLAIN));

    // Compact overflow: default hint, custom hint, no hint.
    for (name, hint) in [
        ("report-overflow-default-hint", None),
        ("report-overflow-custom-hint", Some(Some("use --verbose"))),
        ("report-overflow-no-hint", Some(None)),
    ] {
        let (group, findings) = five_findings();
        let mut report = Report::new("T", Verdict::Warning).add_group(group);
        let mut fields = vec![("groups", Json::Arr(vec![group_json("G", vec![], findings)]))];
        match hint {
            None => {}
            Some(Some(text)) => {
                report = report.with_details_hint(Some(text.to_string()));
                fields.push(("detailsHint", s(text)));
            }
            Some(None) => {
                report = report.with_details_hint(None);
                fields.push(("detailsHint", Json::Bool(false)));
            }
        }
        cases.push(report_case(
            name,
            base("T", "warning", fields),
            report,
            PLAIN,
        ));
    }

    // A larger total than the findings given, an advisory group and a sample bound.
    let (group, findings) = five_findings();
    let group = group.with_total_count(12).with_advisory(true);
    let mut report = Report::new("T", Verdict::Info).add_group(group);
    report.max_compact_samples = 1;
    let input = base(
        "T",
        "info",
        vec![
            ("maxCompactSamples", Json::Num(1)),
            (
                "groups",
                Json::Arr(vec![group_json(
                    "G",
                    vec![
                        ("totalCount", Json::Num(12)),
                        ("advisory", Json::Bool(true)),
                    ],
                    findings,
                )]),
            ),
        ],
    );
    cases.push(report_case("report-total-advisory", input, report, PLAIN));

    // Width: metrics stack and a long message wraps.
    let long_message = vec!["word"; 20].join(" ");
    let report = Report::new("T", Verdict::Passed)
        .add_metric(Metric::new("Alpha", "1"))
        .add_metric(Metric::new("Beta", "2"))
        .add_metric(Metric::new("Gamma", "3"))
        .add_group(
            FindingGroup::new("G").add_finding(
                Finding::new(Tone::Info, long_message.clone())
                    .with_rule_id("style/long")
                    .with_badge(Badge::new("note", Tone::Info)),
            ),
        );
    let input = base(
        "T",
        "passed",
        vec![
            (
                "metrics",
                Json::Arr(vec![
                    obj(vec![("key", s("Alpha")), ("value", s("1"))]),
                    obj(vec![("key", s("Beta")), ("value", s("2"))]),
                    obj(vec![("key", s("Gamma")), ("value", s("3"))]),
                ]),
            ),
            (
                "groups",
                Json::Arr(vec![group_json(
                    "G",
                    vec![],
                    vec![obj(vec![
                        ("message", s(&long_message)),
                        ("tone", s("info")),
                        ("ruleId", s("style/long")),
                        (
                            "badge",
                            obj(vec![("label", s("note")), ("tone", s("info"))]),
                        ),
                    ])],
                )]),
            ),
        ],
    );
    for (name, width) in [("report-width-30", 30), ("report-width-60", 60)] {
        let report = report.clone();
        cases.push(report_case(
            name,
            input.clone(),
            report,
            Opts {
                width: Some(width),
                ..PLAIN
            },
        ));
    }

    // Locations: file, url, selector and artifact with hyperlinks off, and a url
    // with hyperlinks on (a file link would embed the working directory).
    let locations =
        Report::new("Links", Verdict::Passed).add_group(
            FindingGroup::new("Pages")
                .add_finding(
                    Finding::new(Tone::Warning, "Slow page")
                        .with_location(Location::Url("https://example.com/a?x=1&y=2".to_string())),
                )
                .add_finding(
                    Finding::new(Tone::Warning, "Large file")
                        .with_location(Location::file_line_col("src/main.rs", 7, 3)),
                )
                .add_finding(
                    Finding::new(Tone::Info, "Hidden element")
                        .with_location(Location::Selector("main > .card".to_string())),
                )
                .add_finding(
                    Finding::new(Tone::Info, "Written report")
                        .with_location(Location::Artifact("out/report.html".into())),
                ),
        );
    let locations_input = base(
        "Links",
        "passed",
        vec![(
            "groups",
            Json::Arr(vec![group_json(
                "Pages",
                vec![],
                vec![
                    obj(vec![
                        ("message", s("Slow page")),
                        ("tone", s("warning")),
                        ("location", url_json("https://example.com/a?x=1&y=2")),
                    ]),
                    obj(vec![
                        ("message", s("Large file")),
                        ("tone", s("warning")),
                        ("location", file_json("src/main.rs", Some(7), Some(3))),
                    ]),
                    obj(vec![
                        ("message", s("Hidden element")),
                        ("tone", s("info")),
                        (
                            "location",
                            obj(vec![("kind", s("selector")), ("value", s("main > .card"))]),
                        ),
                    ]),
                    obj(vec![
                        ("message", s("Written report")),
                        ("tone", s("info")),
                        (
                            "location",
                            obj(vec![
                                ("kind", s("artifact")),
                                ("value", s("out/report.html")),
                            ]),
                        ),
                    ]),
                ],
            )]),
        )],
    );
    cases.push(report_case(
        "report-locations-links-off",
        locations_input.clone(),
        locations,
        Opts {
            hyperlinks: Some(false),
            ..PLAIN
        },
    ));
    let url_only = Report::new("Links", Verdict::Passed).add_group(
        FindingGroup::new("Pages").add_finding(
            Finding::new(Tone::Warning, "Slow page")
                .with_location(Location::Url("https://example.com/a?x=1&y=2".to_string())),
        ),
    );
    let url_only_input = base(
        "Links",
        "passed",
        vec![(
            "groups",
            Json::Arr(vec![group_json(
                "Pages",
                vec![],
                vec![obj(vec![
                    ("message", s("Slow page")),
                    ("tone", s("warning")),
                    ("location", url_json("https://example.com/a?x=1&y=2")),
                ])],
            )]),
        )],
    );
    cases.push(report_case(
        "report-url-links-on",
        url_only_input,
        url_only,
        Opts {
            hyperlinks: Some(true),
            ..PLAIN
        },
    ));

    // Error blocks.
    let console = PLAIN.console();
    cases.push(Case {
        name: "error-remedy-commands",
        function: "renderError",
        input: obj(vec![
            ("heading", s("Browser executable not found")),
            ("explanation", s("The audit needs a local browser.")),
            ("remedy", s("Install one:")),
            (
                "commands",
                strs(&[
                    "npx playwright install chromium",
                    "npx playwright --version",
                ]),
            ),
        ]),
        options: PLAIN.json(false),
        expected: ErrorBlock::new("Browser executable not found")
            .with_explanation("The audit needs a local browser.")
            .with_remedy("Install one:")
            .add_command("npx playwright install chromium")
            .add_command("npx playwright --version")
            .render(console),
    });
    let unicode = Opts {
        unicode: true,
        ..PLAIN
    };
    cases.push(Case {
        name: "error-heading-unicode",
        function: "renderError",
        input: obj(vec![("heading", s("Config invalid"))]),
        options: unicode.json(false),
        expected: ErrorBlock::new("Config invalid").render(unicode.console()),
    });

    // Diff block: every action, with and without a delta.
    let change = |action: &str, path: &str, delta: Option<&str>| {
        let mut fields = vec![("action", s(action)), ("path", s(path))];
        if let Some(delta) = delta {
            fields.push(("delta", s(delta)));
        }
        obj(fields)
    };
    let diff = DiffBlock::new()
        .add_change(FileChange::new(FileAction::Added, "a.json"))
        .add_change(FileChange::new(FileAction::Modified, "b.ts").with_delta("+18 -4"))
        .add_change(FileChange::new(FileAction::Deleted, "c.ts"))
        .add_change(FileChange::new(FileAction::Renamed, "d.ts -> e.ts"));
    for (name, opts) in [("diff-all-actions", PLAIN), ("diff-unicode", unicode)] {
        cases.push(Case {
            name,
            function: "renderDiff",
            input: obj(vec![(
                "changes",
                Json::Arr(vec![
                    change("added", "a.json", None),
                    change("modified", "b.ts", Some("+18 -4")),
                    change("deleted", "c.ts", None),
                    change("renamed", "d.ts -> e.ts", None),
                ]),
            )]),
            options: opts.json(false),
            expected: diff.clone().render(opts.console()),
        });
    }

    cases
}

fn fixture_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("node-parity")
}

fn fixture_text(case: &Case) -> String {
    let mut out = String::new();
    obj(vec![
        ("function", s(case.function)),
        ("input", case.input.clone()),
        ("options", case.options.clone()),
        ("expected", s(&case.expected)),
    ])
    .write(&mut out, 0);
    out.push('\n');
    out
}

#[test]
fn node_parity_fixtures_match_the_rust_renderer() {
    let dir = fixture_dir();
    let bless = std::env::var_os("RUNEMARK_BLESS").is_some_and(|value| value == "1");
    let cases = cases();

    if bless {
        fs::create_dir_all(&dir).unwrap();
        for entry in fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().is_some_and(|ext| ext == "json") {
                fs::remove_file(path).unwrap();
            }
        }
        for case in &cases {
            fs::write(dir.join(format!("{}.json", case.name)), fixture_text(case)).unwrap();
        }
        return;
    }

    let mut problems = Vec::new();
    for case in &cases {
        let path = dir.join(format!("{}.json", case.name));
        match fs::read_to_string(&path) {
            // A Windows checkout may have converted the line endings.
            Ok(text) if text.replace("\r\n", "\n") == fixture_text(case) => {}
            Ok(_) => problems.push(format!("{} differs from the Rust output", case.name)),
            Err(_) => problems.push(format!("{} is missing", case.name)),
        }
    }
    let known: Vec<String> = cases
        .iter()
        .map(|case| format!("{}.json", case.name))
        .collect();
    if let Ok(entries) = fs::read_dir(&dir) {
        for entry in entries {
            let name = entry.unwrap().file_name().to_string_lossy().into_owned();
            if name.ends_with(".json") && !known.contains(&name) {
                problems.push(format!("{name} belongs to no case"));
            }
        }
    }

    assert!(
        problems.is_empty(),
        "Node parity fixtures under tests/fixtures/node-parity are out of date:\n  {}\n\n\
         If the change is intended, review it and rewrite the fixtures with:\n  \
         RUNEMARK_BLESS=1 cargo test --test node_parity_fixtures\n\
         then run `npm test` in packages/runemark to check Node still matches.",
        problems.join("\n  ")
    );
}
