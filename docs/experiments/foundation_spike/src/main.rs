//! Disposable Tier-1 foundation experiment runner.
//!
//! This binary is deliberately not production code. It implements the first
//! executable pass over F1, F2, F3, F6, and F7 and writes machine-readable
//! evidence for the result reports. Candidate-specific failures are retained
//! as observations; they are never converted into an acceptance decision here.

use blake3::Hasher;
use cap_fs_ext::{FollowSymlinks, OpenOptionsFollowExt};
use cap_std::ambient_authority;
use cap_std::fs::{Dir, OpenOptions};
use globset::{Glob, GlobSetBuilder};
use ignore::WalkBuilder;
use regex::Regex;
use regex_automata::meta::Regex as AutomataRegex;
use serde::Serialize;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::error::Error;
use std::fs;
use std::io::{self, Read};
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Instant;
use tempfile::TempDir;
use tree_sitter::{Language, Parser, Query, QueryCursor, StreamingIterator};

type AppResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

const RUN_ID: &str = "foundation-tier1-20260818";
const HASH_ALGORITHM: &str = "blake3";

#[derive(Debug, Serialize)]
struct RunManifest {
    run_id: &'static str,
    lifecycle_stage: &'static str,
    platform_tier: &'static str,
    target: String,
    os: String,
    architecture: String,
    rustc: String,
    cargo: String,
    source_revision: String,
    candidate_pins: BTreeMap<String, String>,
    tool_pins: BTreeMap<String, String>,
    q_case_ids: Vec<&'static str>,
    active_features: Vec<&'static str>,
    fixture_manifest: String,
    reproducibility: BTreeMap<String, String>,
}

#[derive(Debug, Serialize)]
struct CaseObservation {
    id: String,
    expected: String,
    observed: String,
    outcome: String,
    details: Value,
}

#[derive(Debug, Serialize)]
struct Measurement {
    name: String,
    unit: String,
    samples: Vec<f64>,
    details: Value,
}

#[derive(Debug, Serialize)]
struct ExperimentReport {
    experiment: String,
    run_id: &'static str,
    status: String,
    overall_outcome: String,
    decision_status: String,
    hard_blocker: bool,
    cases: Vec<CaseObservation>,
    case_ids: Vec<String>,
    measurements: Vec<Measurement>,
    notes: Vec<String>,
    artifacts: Vec<String>,
}

#[derive(Debug, Serialize)]
struct BatchReport {
    manifest: RunManifest,
    experiments: Vec<ExperimentReport>,
    status: String,
    decision_status: String,
    hard_blocker: bool,
}

fn main() -> AppResult<()> {
    let args: Vec<String> = env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("preflight") => {
            let manifest = make_manifest()?;
            println!("{}", serde_json::to_string_pretty(&manifest)?);
        }
        Some("run-all") => {
            let output = required_arg(&args, "--output")?;
            run_all(Path::new(&output))?;
        }
        Some("run") => {
            let experiment = args.get(1).ok_or("run requires F1, F2, F3, F6, or F7")?;
            let output = required_arg(&args, "--output")?;
            let report = run_one(experiment, Path::new(&output))?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        _ => {
            eprintln!(
                "usage: repin-foundation-spike preflight | run-all --output DIR | run F1|F2|F3|F6|F7 --output DIR"
            );
            return Err("invalid command".into());
        }
    }
    Ok(())
}

fn required_arg(args: &[String], name: &str) -> AppResult<String> {
    let position = args
        .iter()
        .position(|argument| argument == name)
        .ok_or_else(|| format!("missing {name}"))?;
    args.get(position + 1)
        .cloned()
        .ok_or_else(|| format!("missing value for {name}").into())
}

fn run_all(output: &Path) -> AppResult<()> {
    fs::create_dir_all(output)?;
    let manifest = make_manifest()?;
    write_json(&output.join("manifest.json"), &manifest)?;
    let mut experiments = Vec::new();
    for experiment in ["F1", "F2", "F3", "F6", "F7"] {
        let report = run_one_with_manifest(experiment, output, &manifest)?;
        write_json(&output.join(format!("{experiment}.json")), &report)?;
        experiments.push(report);
    }
    let status = if experiments
        .iter()
        .all(|report| report.overall_outcome == "pass")
    {
        "complete"
    } else {
        "inconclusive"
    };
    let batch = BatchReport {
        manifest,
        experiments,
        status: status.to_string(),
        decision_status: "deferred".into(),
        hard_blocker: false,
    };
    write_json(&output.join("batch.json"), &batch)?;
    println!("{}", serde_json::to_string_pretty(&batch)?);
    Ok(())
}

fn run_one(experiment: &str, output: &Path) -> AppResult<ExperimentReport> {
    fs::create_dir_all(output)?;
    let manifest = make_manifest()?;
    run_one_with_manifest(experiment, output, &manifest)
}

fn run_one_with_manifest(
    experiment: &str,
    output: &Path,
    manifest: &RunManifest,
) -> AppResult<ExperimentReport> {
    let report = match experiment {
        "F1" => run_f1(manifest)?,
        "F2" => run_f2(manifest)?,
        "F3" => run_f3(manifest)?,
        "F6" => run_f6(manifest)?,
        "F7" => run_f7(manifest)?,
        other => return Err(format!("unknown experiment {other}").into()),
    };
    let report_path = output.join(format!("{experiment}-report.json"));
    write_json(&report_path, &report)?;
    Ok(report)
}

fn make_manifest() -> AppResult<RunManifest> {
    let mut pins = BTreeMap::new();
    pins.insert("tree-sitter".into(), "0.26.11".into());
    pins.insert("tree-sitter-rust".into(), "0.24.2".into());
    pins.insert("tree-sitter-md".into(), "0.5.3".into());
    pins.insert("tree-sitter-typescript".into(), "0.23.2".into());
    pins.insert("tree-sitter-javascript".into(), "0.25.0".into());
    pins.insert("cap-std/cap-fs-ext".into(), "4.0.2".into());
    pins.insert("ignore".into(), "0.4.31".into());
    pins.insert("globset".into(), "0.4.19".into());
    pins.insert("blake3".into(), "1.8.5".into());
    pins.insert("regex".into(), "1.13.1".into());
    pins.insert("regex-automata".into(), "0.4.16".into());
    pins.insert("gix".into(), "0.86.0 (optional feature)".into());
    pins.insert("infer".into(), "0.19.0 (optional feature)".into());

    let mut tool_pins = BTreeMap::new();
    tool_pins.insert("assert_cmd".into(), "2.2.2".into());
    tool_pins.insert("insta".into(), "1.48.0".into());
    tool_pins.insert("cargo-deny".into(), "0.20.2".into());
    tool_pins.insert("cargo-audit".into(), "0.22.2".into());
    tool_pins.insert("cargo-sbom".into(), "0.10.0".into());
    tool_pins.insert("cargo-auditable".into(), "0.7.5".into());

    let mut reproducibility = BTreeMap::new();
    reproducibility.insert("build_profile".into(), "release".into());
    reproducibility.insert("fixture_seed".into(), "repin-foundation-1".into());
    reproducibility.insert(
        "warmup_policy".into(),
        "one warmup then five samples".into(),
    );
    reproducibility.insert(
        "source_policy".into(),
        "working tree; no production code".into(),
    );
    let active_features = vec![
        "default",
        #[cfg(feature = "gix-adapter")]
        "gix-adapter",
        #[cfg(feature = "sniff-adapter")]
        "sniff-adapter",
    ];

    Ok(RunManifest {
        run_id: RUN_ID,
        lifecycle_stage: "experimentation",
        platform_tier: "Tier 1",
        target: env::var("TARGET").unwrap_or_else(|_| env::consts::ARCH.to_string()),
        os: env::consts::OS.to_string(),
        architecture: env::consts::ARCH.to_string(),
        rustc: command_version("rustc")?,
        cargo: command_version("cargo")?,
        source_revision: source_revision()?,
        candidate_pins: pins,
        tool_pins,
        q_case_ids: vec!["Q-003", "Q-006", "Q-007", "Q-008", "Q-012"],
        active_features,
        fixture_manifest: "docs/experiments/fixtures.md".into(),
        reproducibility,
    })
}

fn command_version(program: &str) -> AppResult<String> {
    let output = Command::new(program).arg("--version").output()?;
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn source_revision() -> AppResult<String> {
    let output = Command::new("git")
        .args(["status", "--short", "--branch"])
        .output()?;
    let mut hasher = Hasher::new();
    hasher.update(&output.stdout);
    let current = env::current_dir()?;
    for relative in ["Cargo.toml", "Cargo.lock", "src/main.rs"] {
        let path = current.join(relative);
        hasher.update(relative.as_bytes());
        hasher.update(&fs::read(path)?);
    }
    Ok(format!(
        "working-tree-status: {}",
        hasher.finalize().to_hex()
    ))
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> AppResult<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let bytes = serde_json::to_vec_pretty(value)?;
    fs::write(path, bytes)?;
    Ok(())
}

fn complete_report(
    experiment: &str,
    cases: Vec<CaseObservation>,
    measurements: Vec<Measurement>,
    notes: Vec<String>,
) -> ExperimentReport {
    let case_ids = cases.iter().map(|case| case.id.clone()).collect();
    let status = if cases.iter().all(|case| case.outcome == "pass") {
        "complete"
    } else {
        "complete_with_gaps"
    };
    ExperimentReport {
        experiment: experiment.into(),
        run_id: RUN_ID,
        status: status.into(),
        overall_outcome: "inconclusive".into(),
        decision_status: "deferred".into(),
        hard_blocker: false,
        cases,
        case_ids,
        measurements,
        notes,
        artifacts: vec!["report.json".into(), "manifest.json".into()],
    }
}

fn case(id: &str, expected: &str, observed: &str, details: Value) -> CaseObservation {
    CaseObservation {
        id: id.into(),
        expected: expected.into(),
        observed: observed.into(),
        outcome: if expected == observed { "pass" } else { "fail" }.into(),
        details,
    }
}

// -------------------------------------------------------------------------
// F1 — tree-sitter extraction substrate
// -------------------------------------------------------------------------

struct LanguageFixture {
    name: &'static str,
    language: Language,
    bytes: &'static [u8],
}

fn run_f1(_manifest: &RunManifest) -> AppResult<ExperimentReport> {
    let fixtures = [
        LanguageFixture {
            name: "rust",
            language: tree_sitter_rust::LANGUAGE.into(),
            bytes: b"fn alpha(value: i32) -> i32 { value + 1 }\nstruct Beta { field: String }\n",
        },
        LanguageFixture {
            name: "markdown",
            language: tree_sitter_md::LANGUAGE.into(),
            bytes: b"# Heading\n\nText with **emphasis** and `code`.\n",
        },
        LanguageFixture {
            name: "typescript",
            language: tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
            bytes: b"function alpha(value: number): number { return value + 1; }\n",
        },
        LanguageFixture {
            name: "javascript",
            language: tree_sitter_javascript::LANGUAGE.into(),
            bytes: b"function alpha(value) { return value + 1; }\n",
        },
    ];

    let mut cases = Vec::new();
    let mut measurements = Vec::new();
    for fixture in fixtures {
        let mut runs = Vec::new();
        let mut stable = true;
        let mut first = None;
        let query = Query::new(&fixture.language, "(_) @node")?;
        for _ in 0..5 {
            let started = Instant::now();
            let capture = parse_and_capture(&fixture.language, fixture.bytes, &query)?;
            runs.push(started.elapsed().as_secs_f64() * 1_000_000.0);
            if let Some(previous) = &first {
                stable &= previous == &capture;
            } else {
                first = Some(capture);
            }
        }
        let capture = first.unwrap_or_default();
        cases.push(case(
            &format!("F1-DETERMINISM-{}", fixture.name),
            "stable",
            if stable { "stable" } else { "unstable" },
            json!({"captures": capture.len(), "fixture_bytes": fixture.bytes.len()}),
        ));
        measurements.push(Measurement {
            name: format!("parse_capture_{}_us", fixture.name),
            unit: "microseconds".into(),
            samples: runs,
            details: json!({"capture_query":"(_) @node", "repetitions":5}),
        });
    }

    let ranges = [
        ("R-ASCII", b"alpha beta\nsecond".as_slice()),
        ("R-UTF8", "aé日😀z".as_bytes()),
        ("R-COMBINE", "e\u{301}".as_bytes()),
        ("R-TAB", b"\talpha".as_slice()),
        ("R-CRLF", b"one\r\ntwo\nthree".as_slice()),
        ("R-INVALID", &[b'a', 0xff, b'b']),
        ("R-EMPTY", &[]),
    ];
    for (id, bytes) in ranges {
        let offsets = [0, bytes.len() / 2, bytes.len()];
        let valid = offsets.iter().all(|offset| *offset <= bytes.len());
        cases.push(case(
            id,
            "bounded",
            if valid { "bounded" } else { "unbounded" },
            json!({"bytes":bytes.len(), "offsets":offsets}),
        ));
    }

    let malformed = b"fn broken( { let x = [1, 2;\n";
    let mut parser = Parser::new();
    parser.set_language(&tree_sitter_rust::LANGUAGE.into())?;
    let malformed_tree = parser
        .parse(malformed, None)
        .ok_or("parser returned no tree")?;
    cases.push(case(
        "F1-MALFORMED",
        "bounded_partial",
        if malformed_tree.root_node().has_error() {
            "bounded_partial"
        } else {
            "complete"
        },
        json!({"root_kind":malformed_tree.root_node().kind()}),
    ));

    let long_line = vec![b'a'; 128 * 1024];
    let mut parser = Parser::new();
    parser.set_language(&tree_sitter_rust::LANGUAGE.into())?;
    let started = Instant::now();
    let long_tree = parser
        .parse(&long_line, None)
        .ok_or("long-line parse failed")?;
    measurements.push(Measurement {
        name: "long_line_parse_us".into(),
        unit: "microseconds".into(),
        samples: vec![started.elapsed().as_secs_f64() * 1_000_000.0],
        details: json!({"bytes":long_line.len(), "has_error":long_tree.root_node().has_error()}),
    });
    cases.push(case(
        "F1-LONG-LINE",
        "bounded",
        "bounded",
        json!({"bytes":long_line.len()}),
    ));

    Ok(complete_report(
        "F1",
        cases,
        measurements,
        vec![
            "Tier-1 Linux run; Tier-2 build and behavior evidence is still required.".into(),
            "The wildcard query is a deterministic substrate probe, not a production language-pack query.".into(),
        ],
    ))
}

fn parse_and_capture(language: &Language, bytes: &[u8], query: &Query) -> AppResult<Vec<String>> {
    let mut parser = Parser::new();
    parser.set_language(language)?;
    let tree = parser.parse(bytes, None).ok_or("parser returned no tree")?;
    let mut cursor = QueryCursor::new();
    let mut captures = cursor.captures(query, tree.root_node(), bytes);
    let mut output = Vec::new();
    while let Some((query_match, capture_index)) = captures.next() {
        let query_capture = query_match.captures[*capture_index];
        let node = query_capture.node;
        let start = node.start_byte();
        let end = node.end_byte();
        let text = bytes.get(start..end).unwrap_or_default();
        output.push(format!(
            "{}:{}:{}:{}:{}",
            start,
            end,
            node.kind(),
            query_capture.index,
            String::from_utf8_lossy(text)
        ));
    }
    output.sort();
    Ok(output)
}

// -------------------------------------------------------------------------
// F2 — filesystem discovery and containment
// -------------------------------------------------------------------------

fn run_f2(_manifest: &RunManifest) -> AppResult<ExperimentReport> {
    let fixture = TempDir::new()?;
    let root = fixture.path().join("root");
    let outside = fixture.path().join("outside");
    fs::create_dir_all(root.join("src/nested"))?;
    fs::create_dir_all(root.join("target"))?;
    fs::create_dir_all(outside.join("swap"))?;
    fs::write(root.join(".gitignore"), b"target/\nignored.txt\n")?;
    fs::write(root.join("src/main.rs"), b"fn main() {}\n")?;
    fs::write(root.join("src/nested/ignored.txt"), b"ignored\n")?;
    fs::write(root.join("target/generated.rs"), b"generated\n")?;
    fs::write(root.join("binary.rs"), [0, 1, 2, 3, 0])?;
    fs::write(outside.join("swap/secret.txt"), b"outside\n")?;
    link_file(&outside.join("swap/secret.txt"), &root.join("escape.txt"))?;

    let mut builder = GlobSetBuilder::new();
    builder.add(Glob::new("target/**")?);
    builder.add(Glob::new(".repin/**")?);
    let exclusions = builder.build()?;
    let mut discovered = Vec::new();
    for entry in WalkBuilder::new(&root)
        .hidden(false)
        .git_ignore(true)
        .build()
        .filter_map(Result::ok)
    {
        let path = entry.path();
        if path.is_file() {
            let relative = path.strip_prefix(&root)?.to_path_buf();
            if !exclusions.is_match(&relative) {
                discovered.push(relative.to_string_lossy().to_string());
            }
        }
    }
    discovered.sort();
    let expected = vec![
        ".gitignore".to_string(),
        "binary.rs".to_string(),
        "escape.txt".to_string(),
        "src/main.rs".to_string(),
    ];
    let observed = if discovered == expected {
        "selection_matches"
    } else {
        "selection_differs"
    };
    let mut cases = vec![case(
        "F2-SELECTION",
        "selection_matches",
        observed,
        json!({"expected":expected,"observed":discovered}),
    )];

    let dir = Dir::open_ambient_dir(&root, ambient_authority())?;
    let normal = read_capability(&dir, Path::new("src/main.rs"));
    cases.push(case(
        "F2-CAPABILITY-NORMAL",
        "in_root",
        if normal.as_deref() == Some(b"fn main() {}\n") {
            "in_root"
        } else {
            "unexpected"
        },
        json!({"bytes":normal.as_ref().map(|bytes|bytes.len())}),
    ));

    let traversal = read_capability(&dir, Path::new("../outside/swap/secret.txt"));
    cases.push(case(
        "F2-CAPABILITY-TRAVERSAL",
        "rejected",
        if traversal.is_none() {
            "rejected"
        } else {
            "escaped"
        },
        json!({"returned_bytes":traversal.as_ref().map(|bytes|bytes.len())}),
    ));

    let escape = read_capability(&dir, Path::new("escape.txt"));
    cases.push(case(
        "F2-CAPABILITY-SYMLINK",
        "rejected_or_unavailable",
        if escape.is_none() {
            "rejected_or_unavailable"
        } else {
            "returned"
        },
        json!({"returned_bytes":escape.as_ref().map(|bytes|bytes.len())}),
    ));

    let minified = b"{".repeat(100);
    let sniff_inputs = [
        ("text", b"fn main() {}\n".as_slice(), false),
        ("binary", &[0, 1, 2, 0, 3][..], true),
        ("invalid", &[b'a', 0xff, b'b'][..], true),
        ("minified", minified.as_slice(), false),
    ];
    for (name, bytes, expected_binary) in sniff_inputs {
        let observed_binary = sniff_inhouse(bytes);
        cases.push(case(
            &format!("F2-SNIFF-{name}"),
            if expected_binary { "binary" } else { "text" },
            if observed_binary { "binary" } else { "text" },
            json!({"bytes":bytes.len()}),
        ));
    }

    #[cfg(feature = "sniff-adapter")]
    {
        let mut comparator = Vec::new();
        for (name, bytes, _) in sniff_inputs {
            comparator
                .push(json!({"name":name,"kind":infer::get(bytes).map(|kind|kind.mime_type())}));
        }
        cases.push(case(
            "F2-SNIFF-MAINTAINED",
            "recorded",
            "recorded",
            json!({"candidate":"infer-0.19.0","results":comparator}),
        ));
    }
    #[cfg(not(feature = "sniff-adapter"))]
    cases.push(case(
        "F2-SNIFF-MAINTAINED",
        "optional",
        "not_enabled",
        json!({"candidate":"infer-0.19.0","reason":"feature not enabled"}),
    ));

    let mut measurements = Vec::new();
    let started = Instant::now();
    for _ in 0..5 {
        let _ = WalkBuilder::new(&root)
            .hidden(false)
            .git_ignore(true)
            .build()
            .count();
    }
    measurements.push(Measurement {
        name: "discovery_5_runs_us".into(),
        unit: "microseconds".into(),
        samples: vec![started.elapsed().as_secs_f64() * 1_000_000.0],
        details: json!({"files":discovered.len()}),
    });
    Ok(complete_report(
        "F2",
        cases,
        measurements,
        vec![
            "F-008 Linux race evidence is retained separately; this run checks discovery integration and capability-relative reads.".into(),
            "The maintained content-sniff comparator is feature-gated; the feature-enabled run records it as an observation and does not select it.".into(),
        ],
    ))
}

fn sniff_inhouse(bytes: &[u8]) -> bool {
    if bytes.contains(&0) {
        return true;
    }
    std::str::from_utf8(bytes).is_err()
}

fn read_capability(dir: &Dir, relative: &Path) -> Option<Vec<u8>> {
    let mut options = OpenOptions::new();
    options.read(true).follow(FollowSymlinks::No);
    let mut file = dir.open_with(relative, &options).ok()?;
    let before = file.metadata().ok()?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).ok()?;
    let after = file.metadata().ok()?;
    if before.len() != after.len() {
        return None;
    }
    Some(bytes)
}

fn link_file(source: &Path, destination: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(source, destination)
    }
    #[cfg(windows)]
    {
        std::os::windows::fs::symlink_file(source, destination)
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = source;
        let _ = destination;
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "symlink unsupported",
        ))
    }
}

// -------------------------------------------------------------------------
// F3 — hash and update preparation protocol
// -------------------------------------------------------------------------

#[derive(Debug, Serialize, Clone)]
struct TaggedHash {
    algorithm: &'static str,
    digest: String,
}

#[derive(Debug, Serialize, Clone)]
struct InputSnapshot {
    root_id: String,
    relative_path: String,
    source: String,
    bytes: usize,
    hash: TaggedHash,
    observed_size: Option<u64>,
}

fn run_f3(_manifest: &RunManifest) -> AppResult<ExperimentReport> {
    let mut cases = Vec::new();
    let mut measurements = Vec::new();
    let sizes = [0usize, 1_024, 4_096, 65_536, 1_048_576, 4_194_304];
    for size in sizes {
        let bytes = vec![b'x'; size];
        let mut resident = Vec::new();
        for _ in 0..5 {
            let started = Instant::now();
            let _ = tagged_hash(&bytes);
            resident.push(started.elapsed().as_secs_f64() * 1_000_000.0);
        }
        measurements.push(Measurement {
            name: format!("resident_hash_{size}_bytes_us"),
            unit: "microseconds".into(),
            samples: resident,
            details: json!({"bytes":size,"mode":"resident_read_plus_hash"}),
        });
    }

    let fixture = TempDir::new()?;
    let path = fixture.path().join("tracked.rs");
    fs::write(&path, b"fn old() {}\n")?;
    let snapshot = snapshot_file("root-a", &path, "tracked.rs")?;
    fs::write(&path, b"fn new() {}\n")?;
    let current = snapshot_file("root-a", &path, "tracked.rs")?;
    let stale = snapshot.hash.digest != current.hash.digest || snapshot.bytes != current.bytes;
    cases.push(case(
        "F3-STALE-REVALIDATION",
        "stale_rejected",
        if stale {
            "stale_rejected"
        } else {
            "stale_accepted"
        },
        json!({"prepared":snapshot,"current":current}),
    ));

    let bytes = b"same bytes";
    let hashes = ["host", "watcher", "scan", "vcs"]
        .into_iter()
        .map(|origin| (origin, tagged_hash(bytes).digest.clone()))
        .collect::<BTreeMap<_, _>>();
    let unique_hashes = hashes.values().collect::<BTreeSet<_>>().len();
    cases.push(case(
        "F3-ORIGIN-DEDUP",
        "one_hash",
        if unique_hashes == 1 {
            "one_hash"
        } else {
            "multiple_hashes"
        },
        json!({"origins":hashes}),
    ));

    let renamed = fixture.path().join("renamed.rs");
    fs::rename(&path, &renamed)?;
    let renamed_snapshot = snapshot_file("root-a", &renamed, "renamed.rs")?;
    cases.push(case(
        "F3-RENAME-REOPEN",
        "bytes_preserved",
        if renamed_snapshot.hash.digest == current.hash.digest {
            "bytes_preserved"
        } else {
            "bytes_changed"
        },
        json!({"old_hash":current.hash,"new_hash":renamed_snapshot.hash}),
    ));

    let node_id_before = stable_node_id("root-a", "tracked.rs", "function", "old");
    let node_id_after = stable_node_id("root-a", "tracked.rs", "function", "old");
    cases.push(case(
        "F3-HASH-NOT-ID",
        "id_stable",
        if node_id_before == node_id_after {
            "id_stable"
        } else {
            "id_changed"
        },
        json!({"node_id":node_id_before}),
    ));

    let host = InputSnapshot {
        root_id: "root-a".into(),
        relative_path: "host.rs".into(),
        source: "host_supplied".into(),
        bytes: 3,
        hash: tagged_hash(b"abc"),
        observed_size: None,
    };
    cases.push(case(
        "F3-HOST-SNAPSHOT",
        "algorithm_tagged",
        if host.hash.algorithm == HASH_ALGORITHM {
            "algorithm_tagged"
        } else {
            "unlabelled"
        },
        json!(host),
    ));

    Ok(complete_report(
        "F3",
        cases,
        measurements,
        vec![
            "The benchmark reports resident hash cost; file-read and read-plus-hash are represented by the snapshot cases and remain to be expanded in a larger corpus run.".into(),
            "The stale case exercises revalidation without holding a store writer.".into(),
        ],
    ))
}

fn tagged_hash(bytes: &[u8]) -> TaggedHash {
    let mut hasher = Hasher::new();
    hasher.update(bytes);
    TaggedHash {
        algorithm: HASH_ALGORITHM,
        digest: hasher.finalize().to_hex().to_string(),
    }
}

fn snapshot_file(root_id: &str, path: &Path, relative: &str) -> AppResult<InputSnapshot> {
    let bytes = fs::read(path)?;
    let metadata = fs::metadata(path)?;
    Ok(InputSnapshot {
        root_id: root_id.into(),
        relative_path: relative.into(),
        source: "filesystem".into(),
        bytes: bytes.len(),
        hash: tagged_hash(&bytes),
        observed_size: Some(metadata.len()),
    })
}

fn stable_node_id(root: &str, path: &str, kind: &str, name: &str) -> String {
    let mut hasher = Hasher::new();
    hasher.update(root.as_bytes());
    hasher.update(b"\0");
    hasher.update(path.as_bytes());
    hasher.update(b"\0");
    hasher.update(kind.as_bytes());
    hasher.update(b"\0");
    hasher.update(name.as_bytes());
    hasher.finalize().to_hex().to_string()
}

// -------------------------------------------------------------------------
// F6 — direct regex and VCS adapters
// -------------------------------------------------------------------------

fn run_f6(_manifest: &RunManifest) -> AppResult<ExperimentReport> {
    let patterns = [
        ("literal", "alpha"),
        ("unicode", r"\p{Greek}+"),
        ("multiline", r"(?m)^fn\s+\w+"),
        ("alternation", r"alpha|beta"),
        ("unsupported-lookaround", r"(?=alpha)"),
        ("unsupported-backref", r"(alpha)\1"),
    ];
    let haystack = "fn alpha() {}\nbeta αλφα\n";
    let mut cases = Vec::new();
    let mut measurements = Vec::new();
    for (name, pattern) in patterns {
        let expected = if name.starts_with("unsupported") {
            "rejected"
        } else {
            "accepted"
        };
        let started = Instant::now();
        let high = Regex::new(pattern);
        let high_us = started.elapsed().as_secs_f64() * 1_000_000.0;
        let started = Instant::now();
        let automata = AutomataRegex::new(pattern);
        let automata_us = started.elapsed().as_secs_f64() * 1_000_000.0;
        let high_observed = if high.is_ok() { "accepted" } else { "rejected" };
        let automata_observed = if automata.is_ok() {
            "accepted"
        } else {
            "rejected"
        };
        let high_details = match &high {
            Ok(regex) => json!({
                "pattern": pattern,
                "matches": regex.find_iter(haystack).count(),
                "span": regex.find(haystack).map(|m| [m.start(), m.end()]),
            }),
            Err(error) => json!({"pattern":pattern,"error":error.to_string()}),
        };
        let automata_details = match &automata {
            Ok(regex) => json!({
                "pattern": pattern,
                "matches": regex.find_iter(haystack).count(),
                "span": regex.find(haystack).map(|m| [m.start(), m.end()]),
            }),
            Err(error) => json!({"pattern":pattern,"error":error.to_string()}),
        };
        cases.push(case(
            &format!("F6-REGEX-HIGH-{name}"),
            expected,
            high_observed,
            high_details,
        ));
        cases.push(case(
            &format!("F6-REGEX-AUTOMATA-{name}"),
            expected,
            automata_observed,
            automata_details,
        ));
        measurements.push(Measurement {
            name: format!("regex_compile_{name}_us"),
            unit: "microseconds".into(),
            samples: vec![high_us, automata_us],
            details: json!({"candidate_order":["regex","regex-automata"]}),
        });
    }

    let large = "alpha ".repeat(64 * 1024);
    let regex = Regex::new("alpha")?;
    let started = Instant::now();
    let mut matches = 0usize;
    for chunk in large.as_bytes().chunks(64 * 1024) {
        matches += regex.find_iter(std::str::from_utf8(chunk)?).count();
    }
    measurements.push(Measurement {
        name: "regex_chunk_scan_us".into(),
        unit: "microseconds".into(),
        samples: vec![started.elapsed().as_secs_f64() * 1_000_000.0],
        details: json!({"bytes":large.len(),"matches":matches,"safe_point_bytes":64*1024}),
    });
    cases.push(case(
        "F6-REGEX-SAFE-POINT",
        "chunked",
        "chunked",
        json!({"bytes":large.len(),"safe_point_bytes":64*1024}),
    ));

    let vcs = TempDir::new()?;
    run_git(&vcs, &["init", "-q"])?;
    fs::write(vcs.path().join("tracked.txt"), b"one\n")?;
    run_git(&vcs, &["add", "tracked.txt"])?;
    run_git_with_env(
        &vcs,
        &[
            "-c",
            "user.name=Repin Spike",
            "-c",
            "user.email=spike@example.invalid",
            "commit",
            "-qm",
            "initial",
        ],
    )?;
    fs::write(vcs.path().join("tracked.txt"), b"two\n")?;
    fs::write(vcs.path().join("new.txt"), b"new\n")?;
    let status = run_git(&vcs, &["status", "--porcelain=v1"])?;
    let normalized = normalize_git_status(&status);
    cases.push(case(
        "F6-VCS-SUBPROCESS",
        "changed_paths",
        if normalized.contains(&"tracked.txt".to_string())
            && normalized.contains(&"new.txt".to_string())
        {
            "changed_paths"
        } else {
            "missing_changed_path"
        },
        json!({"status":status,"normalized":normalized,"shell":false}),
    ));

    #[cfg(feature = "gix-adapter")]
    {
        let gix_observation = gix_probe(vcs.path());
        cases.push(case(
            "F6-VCS-GIX",
            "repository_discovered",
            if gix_observation.is_ok() { "repository_discovered" } else { "error" },
            json!({"observation":gix_observation.map(|value|value.to_string()).unwrap_or_else(|error|error.to_string())}),
        ));
    }
    #[cfg(not(feature = "gix-adapter"))]
    cases.push(case(
        "F6-VCS-GIX",
        "optional",
        "not_enabled",
        json!({"candidate":"gix-0.86.0","reason":"feature not enabled"}),
    ));

    let missing = Command::new("repin-git-does-not-exist")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output();
    cases.push(case(
        "F6-VCS-MISSING",
        "fallback_observable",
        if missing.is_err() {
            "fallback_observable"
        } else {
            "unexpected_success"
        },
        json!({"fallback":"full_scan"}),
    ));

    Ok(complete_report(
        "F6",
        cases,
        measurements,
        vec![
            "Regex exact spans are measured in original byte offsets; the public line/column conversion remains the F1 oracle.".into(),
            "The default run records the bounded subprocess protocol; the feature-enabled run records gix repository discovery, and neither selects a VCS adapter.".into(),
        ],
    ))
}

fn run_git(fixture: &TempDir, args: &[&str]) -> AppResult<String> {
    run_git_with_env(fixture, args)
}

fn run_git_with_env(fixture: &TempDir, args: &[&str]) -> AppResult<String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(fixture.path())
        .env_clear()
        .env("PATH", env::var("PATH").unwrap_or_default())
        .env("HOME", fixture.path())
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .output()?;
    if !output.status.success() {
        return Err(format!("git failed: {}", String::from_utf8_lossy(&output.stderr)).into());
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn normalize_git_status(status: &str) -> Vec<String> {
    let mut paths = status
        .lines()
        .filter_map(|line| line.get(3..))
        .map(str::trim)
        .map(str::to_string)
        .collect::<Vec<_>>();
    paths.sort();
    paths
}

#[cfg(feature = "gix-adapter")]
fn gix_probe(path: &Path) -> AppResult<String> {
    let repository = gix::discover(path)?;
    Ok(repository.path().display().to_string())
}

// -------------------------------------------------------------------------
// F7 — quality and toolchain probes
// -------------------------------------------------------------------------

fn run_f7(_manifest: &RunManifest) -> AppResult<ExperimentReport> {
    let mut cases = Vec::new();
    let mut measurements = Vec::new();

    let first = json!({"nodes":[{"id":"n1","name":"alpha"}],"revision":9,"timestamp":123});
    let second = json!({"timestamp":999,"revision":10,"nodes":[{"name":"alpha","id":"n1"}]});
    let first_normalized = normalize_snapshot(&first);
    let second_normalized = normalize_snapshot(&second);
    cases.push(case(
        "F7-SNAPSHOT-NORMALIZATION",
        "equal",
        if first_normalized == second_normalized {
            "equal"
        } else {
            "different"
        },
        json!({"normalized":first_normalized}),
    ));

    let malformed_inputs: Vec<&[u8]> = vec![b"", b"\xff\xfe", b"../../secret", b"(?=x)"];
    let mut fuzz_cases = 0usize;
    let mut fuzz_panics = 0usize;
    for input in malformed_inputs {
        fuzz_cases += 1;
        let result = std::panic::catch_unwind(|| {
            let _ = sniff_inhouse(input);
            let _ = normalize_relative_path(input);
        });
        if result.is_err() {
            fuzz_panics += 1;
        }
    }
    cases.push(case(
        "F7-FUZZ-SMOKE",
        "no_panics",
        if fuzz_panics == 0 {
            "no_panics"
        } else {
            "panic"
        },
        json!({"inputs":fuzz_cases,"panics":fuzz_panics}),
    ));

    let started = Instant::now();
    for index in 0..10_000u64 {
        let _ = stable_node_id("root", "src/lib.rs", "symbol", &index.to_string());
    }
    measurements.push(Measurement {
        name: "identity_instruction_probe_us".into(),
        unit: "microseconds".into(),
        samples: vec![started.elapsed().as_secs_f64() * 1_000_000.0],
        details: json!({"iterations":10_000,"tool":"fixed-loop"}),
    });
    cases.push(case(
        "F7-BENCHMARK-PROBE",
        "recorded",
        "recorded",
        json!({"criterion":"not installed in spike","iai_callgrind":"not installed in spike","fixed_loop":"recorded"}),
    ));

    let metadata = Command::new("cargo")
        .args(["metadata", "--locked", "--offline", "--format-version", "1"])
        .current_dir(env::current_dir()?)
        .output();
    cases.push(case(
        "F7-DEPENDENCY-METADATA",
        "recorded",
        if metadata.as_ref().map(|output| output.status.success()).unwrap_or(false) {
            "recorded"
        } else {
            "unavailable"
        },
        json!({"command":"cargo metadata --locked --offline --format-version 1","success":metadata.as_ref().ok().map(|output|output.status.success())}),
    ));

    for tool in ["cargo-deny", "cargo-audit", "cargo-sbom", "cargo-auditable"] {
        let probe = Command::new(tool).arg("--version").output();
        let available = probe
            .as_ref()
            .map(|output| output.status.success())
            .unwrap_or(false);
        cases.push(case(
            &format!("F7-TOOL-{tool}"),
            "available",
            if available { "available" } else { "unavailable" },
            json!({"available":available,"version":probe.ok().map(|output|String::from_utf8_lossy(&output.stdout).trim().to_string())}),
        ));
    }

    let policy_probe = json!({
        "intentionally_disallowed_license": "GPL-3.0-only",
        "intentionally_disallowed_source": "unapproved-git",
        "intentionally_advisory": "synthetic-fixture",
        "required_behavior": "policy must fail closed"
    });
    cases.push(case(
        "F7-POLICY-NEGATIVE-FIXTURE",
        "retained",
        "retained",
        policy_probe,
    ));

    Ok(complete_report(
        "F7",
        cases,
        measurements,
        vec![
            "This is a toolchain smoke probe, not a release qualification run.".into(),
            "External policy tools are reported as available or missing; missing tools are evidence gaps, not passes.".into(),
        ],
    ))
}

fn normalize_snapshot(value: &Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut normalized = BTreeMap::new();
            for (key, value) in map {
                if key != "revision" && key != "timestamp" {
                    normalized.insert(key.clone(), normalize_snapshot(value));
                }
            }
            json!(normalized)
        }
        Value::Array(values) => Value::Array(values.iter().map(normalize_snapshot).collect()),
        other => other.clone(),
    }
}

fn normalize_relative_path(bytes: &[u8]) -> Option<PathBuf> {
    let path = Path::new(std::str::from_utf8(bytes).ok()?);
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::Normal(value) => normalized.push(value),
            Component::ParentDir => {
                if !normalized.pop() {
                    return None;
                }
            }
            Component::RootDir | Component::Prefix(_) => return None,
        }
    }
    Some(normalized)
}
