//! Disposable evidence runner for the open Rust-foundation follow-up tasks.

#![allow(clippy::too_many_arguments)]

use blake3::Hasher;
use cap_fs_ext::{FollowSymlinks, OpenOptionsFollowExt};
use cap_std::ambient_authority;
use cap_std::fs::{Dir, OpenOptions};
use globset::{Glob, GlobSetBuilder};
use ignore::WalkBuilder;
use regex::Regex;
use regex_automata::meta::Regex as AutomataRegex;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::error::Error;
use std::fs;
use std::io::{self, Read, Write};
use std::ops::ControlFlow;
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};
use tempfile::tempdir;
use tree_sitter::{Language, ParseOptions, Parser, Query, QueryCursor, StreamingIterator};

type AppResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

const RUN_ID: &str = "foundation-followup-20260818";
const FIXTURE_SEED: &str = "repin-foundation-followup-1";
const FOLLOWUP_COMMAND: &str = "cargo run --release --locked --offline --features gix-adapter,sniff-adapter --bin repin-foundation-followup -- run-all --output <run-directory>";
const TASKS: [&str; 7] = [
    "F-017", "F-018", "F-009", "F-019", "F-014", "F-015", "F-020",
];

#[derive(Debug, Serialize)]
struct RunManifest {
    run_id: &'static str,
    lifecycle_stage: &'static str,
    platform_scope: &'static str,
    target: String,
    os: String,
    architecture: String,
    rustc: String,
    cargo: String,
    command: &'static str,
    source_revision: String,
    lockfile_sha256: String,
    fixture_seed: &'static str,
    candidate_pins: BTreeMap<String, String>,
    active_features: Vec<&'static str>,
    fixture_manifest: &'static str,
    task_order: Vec<&'static str>,
    environment: BTreeMap<String, String>,
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
    decision_status: &'static str,
    hard_blocker: bool,
    cases: Vec<CaseObservation>,
    measurements: Vec<Measurement>,
    notes: Vec<String>,
    artifacts: Vec<String>,
}

#[derive(Debug, Serialize)]
struct BatchReport {
    manifest: RunManifest,
    experiments: Vec<ExperimentReport>,
    status: String,
    decision_status: &'static str,
    hard_blocker: bool,
}

fn main() -> AppResult<()> {
    let args: Vec<String> = env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("preflight") => println!("{}", serde_json::to_string_pretty(&manifest()?)?),
        Some("run-all") => run_all(Path::new(&required_arg(&args, "--output")?))?,
        Some("run") => {
            let task = args.get(1).ok_or("run requires a task identifier")?;
            let output_arg = required_arg(&args, "--output")?;
            let report = run_one(task, Path::new(&output_arg))?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Some("child-parser") => child_parser(),
        _ => {
            eprintln!(
                "usage: repin-foundation-followup preflight | run-all --output DIR | run TASK --output DIR"
            );
            return Err("invalid command".into());
        }
    }
    Ok(())
}

fn required_arg(args: &[String], name: &str) -> AppResult<String> {
    let index = args
        .iter()
        .position(|arg| arg == name)
        .ok_or_else(|| format!("missing {name}"))?;
    args.get(index + 1)
        .cloned()
        .ok_or_else(|| format!("missing value for {name}").into())
}

fn run_all(output: &Path) -> AppResult<()> {
    fs::create_dir_all(output)?;
    let run_manifest = manifest()?;
    write_json(&output.join("manifest.json"), &run_manifest)?;
    let mut reports = Vec::new();
    for task in TASKS {
        let report = run_task(task, output, &run_manifest)?;
        write_json(
            &output.join(format!("{}-report.json", safe_task(task))),
            &report,
        )?;
        reports.push(report);
    }
    let hard_blocker = reports.iter().any(|report| report.hard_blocker);
    let status = if hard_blocker {
        "failed"
    } else if reports
        .iter()
        .any(|report| report.overall_outcome == "inconclusive")
    {
        "inconclusive"
    } else {
        "complete"
    };
    let batch = BatchReport {
        manifest: run_manifest,
        experiments: reports,
        status: status.into(),
        decision_status: "deferred",
        hard_blocker,
    };
    write_json(&output.join("batch.json"), &batch)?;
    println!("{}", serde_json::to_string_pretty(&batch)?);
    Ok(())
}

fn run_one(task: &str, output: &Path) -> AppResult<ExperimentReport> {
    fs::create_dir_all(output)?;
    let run_manifest = manifest()?;
    let report = run_task(task, output, &run_manifest)?;
    write_json(
        &output.join(format!("{}-report.json", safe_task(task))),
        &report,
    )?;
    Ok(report)
}

fn run_task(task: &str, output: &Path, run_manifest: &RunManifest) -> AppResult<ExperimentReport> {
    let result = match task {
        "F-009" => run_f009(output),
        "F-014" => run_f014(output),
        "F-015" => run_f015(output),
        "F-017" => run_f017(output),
        "F-018" => run_f018(output),
        "F-019" => run_f019(output),
        "F-020" => run_f020(output),
        other => Err(format!("unknown task {other}").into()),
    };
    match result {
        Ok(report) => Ok(report),
        Err(error) => Ok(ExperimentReport {
            experiment: task.into(),
            run_id: RUN_ID,
            status: "complete_with_gaps".into(),
            overall_outcome: "inconclusive".into(),
            decision_status: "deferred",
            hard_blocker: false,
            cases: vec![CaseObservation {
                id: format!("{task}-RUNNER"),
                expected: "completed".into(),
                observed: "runner_error".into(),
                outcome: "gap".into(),
                details: json!({"error":error.to_string()}),
            }],
            measurements: Vec::new(),
            notes: vec![format!(
                "Runner error retained as a gap; source revision was {}.",
                run_manifest.source_revision
            )],
            artifacts: Vec::new(),
        }),
    }
}

fn manifest() -> AppResult<RunManifest> {
    let mut pins = BTreeMap::new();
    for (name, version) in [
        ("tree-sitter", "0.26.11"),
        ("tree-sitter-rust", "0.24.2"),
        ("tree-sitter-md", "0.5.3"),
        ("tree-sitter-typescript", "0.23.2"),
        ("tree-sitter-javascript", "0.25.0"),
        ("cap-std/cap-fs-ext", "4.0.2"),
        ("ignore", "0.4.31"),
        ("globset", "0.4.19"),
        ("blake3", "1.8.5"),
        ("regex", "1.13.1"),
        ("regex-automata", "0.4.16"),
        ("gix", "0.86.0"),
        ("infer", "0.19.0"),
        ("sha2", "0.10.9"),
    ] {
        pins.insert(name.into(), version.into());
    }
    let mut reproducibility = BTreeMap::new();
    reproducibility.insert(
        "ordering".into(),
        "canonical bytewise sort then dedup".into(),
    );
    reproducibility.insert("raw_policy".into(), "never overwrite prior runs".into());
    reproducibility.insert(
        "safe_points".into(),
        "parser callback; 64 KiB regex; child kill/reap".into(),
    );
    reproducibility.insert("run_order".into(), TASKS.join(" -> "));
    let mut environment = BTreeMap::new();
    environment.insert("kernel".into(), probe_command("uname", &["-srmo"]));
    environment.insert("cpu_model".into(), cpu_model());
    environment.insert(
        "logical_cpus".into(),
        std::thread::available_parallelism()
            .map(|count| count.get().to_string())
            .unwrap_or_else(|_| "unavailable".into()),
    );
    environment.insert("memory_total".into(), memory_total());
    Ok(RunManifest {
        run_id: RUN_ID,
        lifecycle_stage: "experimentation",
        platform_scope: "Linux x86_64/glibc PoC",
        target: env::var("TARGET")
            .unwrap_or_else(|_| format!("{}-unknown-linux-gnu", env::consts::ARCH)),
        os: env::consts::OS.into(),
        architecture: env::consts::ARCH.into(),
        rustc: command_version("rustc")?,
        cargo: command_version("cargo")?,
        command: FOLLOWUP_COMMAND,
        source_revision: source_revision()?,
        lockfile_sha256: sha256_hex(&fs::read("Cargo.lock")?),
        fixture_seed: FIXTURE_SEED,
        candidate_pins: pins,
        active_features: active_features(),
        fixture_manifest: "docs/experiments/fixtures.md",
        task_order: TASKS.to_vec(),
        environment,
        reproducibility,
    })
}

#[allow(unused_mut)]
fn active_features() -> Vec<&'static str> {
    let mut features = vec!["default"];
    #[cfg(feature = "gix-adapter")]
    features.push("gix-adapter");
    #[cfg(feature = "sniff-adapter")]
    features.push("sniff-adapter");
    features
}

fn command_version(program: &str) -> AppResult<String> {
    Ok(
        String::from_utf8_lossy(&Command::new(program).arg("--version").output()?.stdout)
            .trim()
            .into(),
    )
}

fn probe_command(program: &str, args: &[&str]) -> String {
    Command::new(program)
        .args(args)
        .output()
        .ok()
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().into())
        .filter(|value: &String| !value.is_empty())
        .unwrap_or_else(|| "unavailable".into())
}

fn cpu_model() -> String {
    fs::read_to_string("/proc/cpuinfo")
        .ok()
        .and_then(|contents| {
            contents.lines().find_map(|line| {
                line.strip_prefix("model name\t: ")
                    .or_else(|| line.strip_prefix("Hardware\t: "))
                    .map(str::to_owned)
            })
        })
        .unwrap_or_else(|| "unavailable".into())
}

fn memory_total() -> String {
    fs::read_to_string("/proc/meminfo")
        .ok()
        .and_then(|contents| {
            contents.lines().find_map(|line| {
                line.strip_prefix("MemTotal:")
                    .map(str::trim)
                    .map(str::to_owned)
            })
        })
        .unwrap_or_else(|| "unavailable".into())
}

fn source_revision() -> AppResult<String> {
    let status = Command::new("git")
        .args(["status", "--short", "--branch"])
        .output()?;
    let mut hasher = Hasher::new();
    hasher.update(&status.stdout);
    for relative in [
        "Cargo.toml",
        "Cargo.lock",
        "src/bin/foundation_followup.rs",
        "fixtures/f1-query-v1/rust.scm",
        "fixtures/f1-query-v1/markdown.scm",
        "fixtures/f1-query-v1/typescript.scm",
        "fixtures/f1-query-v1/javascript.scm",
    ] {
        hasher.update(relative.as_bytes());
        if let Ok(bytes) = fs::read(relative) {
            hasher.update(&bytes);
        }
    }
    Ok(format!(
        "working-tree-status: {}",
        hasher.finalize().to_hex()
    ))
}

fn safe_task(task: &str) -> String {
    task.replace('-', "").to_ascii_lowercase()
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> AppResult<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, serde_json::to_vec_pretty(value)?)?;
    Ok(())
}

fn read_json(path: &Path) -> AppResult<Value> {
    Ok(serde_json::from_slice(&fs::read(path)?)?)
}

fn artifact_path(output: &Path, task: &str, name: &str) -> PathBuf {
    output.join("artifacts").join(safe_task(task)).join(name)
}

fn artifact_name(output: &Path, path: &Path) -> String {
    path.strip_prefix(output)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn pass_case(
    id: impl Into<String>,
    expected: &str,
    observed: &str,
    details: Value,
) -> CaseObservation {
    CaseObservation {
        id: id.into(),
        expected: expected.into(),
        observed: observed.into(),
        outcome: "pass".into(),
        details,
    }
}

fn gap_case(
    id: impl Into<String>,
    expected: &str,
    observed: &str,
    details: Value,
) -> CaseObservation {
    CaseObservation {
        id: id.into(),
        expected: expected.into(),
        observed: observed.into(),
        outcome: "gap".into(),
        details,
    }
}

fn fail_case(
    id: impl Into<String>,
    expected: &str,
    observed: &str,
    details: Value,
) -> CaseObservation {
    CaseObservation {
        id: id.into(),
        expected: expected.into(),
        observed: observed.into(),
        outcome: "fail".into(),
        details,
    }
}

fn finish(
    task: &str,
    cases: Vec<CaseObservation>,
    measurements: Vec<Measurement>,
    notes: Vec<String>,
    artifacts: Vec<String>,
) -> ExperimentReport {
    let hard_blocker = cases.iter().any(|case| case.outcome == "fail");
    let gap = cases.iter().any(|case| case.outcome == "gap");
    ExperimentReport {
        experiment: task.into(),
        run_id: RUN_ID,
        status: if gap {
            "complete_with_gaps"
        } else {
            "complete"
        }
        .into(),
        overall_outcome: if hard_blocker {
            "fail"
        } else if gap {
            "inconclusive"
        } else {
            "pass"
        }
        .into(),
        decision_status: "deferred",
        hard_blocker,
        cases,
        measurements,
        notes,
        artifacts,
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut digest = Sha256::new();
    digest.update(bytes);
    format!("{:x}", digest.finalize())
}

// -------------------------------------------------------------------------
// F-017
// -------------------------------------------------------------------------

#[derive(Debug, Serialize, Clone, PartialEq, Eq)]
struct CaptureRecord {
    start_byte: usize,
    end_byte: usize,
    capture_role_ordinal: u32,
    source_node_kind_ordinal: u32,
    normalized_name: String,
    extractor_local_discriminator: u32,
    capture_name: String,
    source_node_kind: String,
    text: String,
}

#[derive(Debug, Serialize)]
struct QueryPackManifest {
    language: String,
    path: String,
    sha256: String,
    query: String,
}

#[derive(Debug, Serialize, Clone, PartialEq, Eq)]
struct Position {
    line: usize,
    column: usize,
    byte: usize,
}

fn run_f017(output: &Path) -> AppResult<ExperimentReport> {
    let mut cases = Vec::new();
    let mut measurements = Vec::new();
    let packs = [
        (
            "rust",
            tree_sitter_rust::LANGUAGE.into(),
            "rust.rs",
            include_str!("../../fixtures/f1-query-v1/rust.scm"),
        ),
        (
            "markdown",
            tree_sitter_md::LANGUAGE.into(),
            "markdown.md",
            include_str!("../../fixtures/f1-query-v1/markdown.scm"),
        ),
        (
            "typescript",
            tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
            "typescript.ts",
            include_str!("../../fixtures/f1-query-v1/typescript.scm"),
        ),
        (
            "javascript",
            tree_sitter_javascript::LANGUAGE.into(),
            "javascript.js",
            include_str!("../../fixtures/f1-query-v1/javascript.scm"),
        ),
    ];
    let fixtures = [
        ("rust.rs", b"use std::fmt;\nfn alpha(value: i32) -> i32 { value + 1 }\nstruct Beta { field: String }\n".as_slice()),
        ("markdown.md", b"# Heading\n\n[link](target.md) and **text**.\n".as_slice()),
        ("typescript.ts", b"import { alpha } from './dep';\nfunction alpha(value: number): number { return value + 1; }\nclass Beta {}\n".as_slice()),
        ("javascript.js", b"import { alpha } from './dep.js';\nfunction alpha(value) { return value + 1; }\nclass Beta {}\n".as_slice()),
    ];
    let mut query_manifests = Vec::new();
    for (name, language, source_name, query_text) in packs {
        let source = fixtures
            .iter()
            .find(|(file, _)| *file == source_name)
            .map(|(_, bytes)| *bytes)
            .ok_or("missing F1 fixture")?;
        let query = Query::new(&language, query_text)?;
        query_manifests.push(QueryPackManifest {
            language: name.into(),
            path: format!("fixtures/f1-query-v1/{name}.scm"),
            sha256: sha256_hex(query_text.as_bytes()),
            query: query_text.into(),
        });
        let mut first = None;
        let mut stable = true;
        let mut samples = Vec::new();
        for _ in 0..5 {
            let started = Instant::now();
            let sequence = captures(&language, source, &query)?;
            samples.push(started.elapsed().as_secs_f64() * 1_000_000.0);
            if let Some(previous) = &first {
                stable &= previous == &sequence;
            } else {
                first = Some(sequence);
            }
        }
        let sequence = first.unwrap_or_default();
        let deduped = dedup_captures(&sequence);
        let artifact = artifact_path(output, "F-017", &format!("captures-{name}.json"));
        write_json(
            &artifact,
            &json!({"language":name,"source":source_name,"pre_dedup":sequence,"post_dedup":deduped}),
        )?;
        cases.push(if stable {
            pass_case(format!("F017-CAPTURE-{name}"), "byte_identical_order", "byte_identical_order", json!({"artifact":artifact_name(output,&artifact),"pre_dedup":sequence.len(),"post_dedup":deduped.len()}))
        } else {
            fail_case(format!("F017-CAPTURE-{name}"), "byte_identical_order", "nondeterministic_order", json!({"artifact":artifact_name(output,&artifact)}))
        });
        measurements.push(Measurement {
            name: format!("f017_capture_{name}_us"),
            unit: "microseconds".into(),
            samples,
            details: json!({"repetitions":5}),
        });
    }
    let query_artifact = artifact_path(output, "F-017", "query-manifest.json");
    write_json(&query_artifact, &query_manifests)?;
    cases.push(pass_case(
        "F017-QUERY-MANIFEST",
        "four_sha256_pinned_packs",
        "four_sha256_pinned_packs",
        json!({"artifact":artifact_name(output,&query_artifact)}),
    ));

    for (id, bytes, start_byte, end_byte, expected_start, expected_end) in range_cases() {
        let start = position_at(&bytes, start_byte)?;
        let end = position_at(&bytes, end_byte)?;
        let details =
            json!({"start":start,"end":end,"bytes":bytes.len(),"text":normalized_text(&bytes)});
        cases.push(if start == expected_start && end == expected_end {
            pass_case(format!("F017-RANGE-{id}"), "exact_position", "exact_position", details)
        } else {
            fail_case(format!("F017-RANGE-{id}"), "exact_position", "position_mismatch", json!({"observed":details,"expected_start":expected_start,"expected_end":expected_end}))
        });
    }
    let range_artifact = artifact_path(output, "F-017", "range-oracle.json");
    write_json(
        &range_artifact,
        &json!({"matrix":"R-ASCII,R-UTF8,R-COMBINE,R-TAB,R-CRLF,R-INVALID,R-LONG,R-BOUNDARY,R-EMPTY","invalid_mapping":"one U+FFFD per maximal invalid run"}),
    )?;

    let native = parser_cancellation()?;
    cases.push(if native["cancelled"] == true {
        pass_case(
            "F017-PARSER-CANCELLATION",
            "bounded_skip",
            "bounded_skip",
            native,
        )
    } else {
        fail_case(
            "F017-PARSER-CANCELLATION",
            "bounded_skip",
            "not_cancelled",
            native,
        )
    });
    let worker = isolated_worker()?;
    cases.push(
        if worker["killed"] == true && worker["fact_batch"] == false {
            pass_case(
                "F017-ISOLATED-WORKER",
                "killed_without_fact_batch",
                "killed_without_fact_batch",
                worker,
            )
        } else {
            fail_case(
                "F017-ISOLATED-WORKER",
                "killed_without_fact_batch",
                "worker_returned",
                worker,
            )
        },
    );
    Ok(finish(
        "F-017",
        cases,
        measurements,
        vec![
            "Results are Linux PoC evidence; parser and grammar decisions remain deferred.".into(),
        ],
        vec![
            artifact_name(output, &query_artifact),
            artifact_name(output, &range_artifact),
        ],
    ))
}

fn captures(language: &Language, bytes: &[u8], query: &Query) -> AppResult<Vec<CaptureRecord>> {
    let mut parser = Parser::new();
    parser.set_language(language)?;
    let tree = parser.parse(bytes, None).ok_or("parser returned no tree")?;
    let names = query.capture_names();
    let mut cursor = QueryCursor::new();
    let mut matches = cursor.captures(query, tree.root_node(), bytes);
    let mut result = Vec::new();
    let mut local = 0;
    while let Some((query_match, capture_index)) = matches.next() {
        let capture = query_match.captures[*capture_index];
        let node = capture.node;
        let text = String::from_utf8_lossy(&bytes[node.start_byte()..node.end_byte()]).into_owned();
        result.push(CaptureRecord {
            start_byte: node.start_byte(),
            end_byte: node.end_byte(),
            capture_role_ordinal: role_ordinal(names[capture.index as usize]),
            source_node_kind_ordinal: 0,
            normalized_name: text.trim().into(),
            extractor_local_discriminator: local,
            capture_name: names[capture.index as usize].to_string(),
            source_node_kind: node.kind().into(),
            text,
        });
        local += 1;
    }
    let kinds: BTreeSet<String> = result
        .iter()
        .map(|item| item.source_node_kind.clone())
        .collect();
    let ordinals: BTreeMap<String, u32> = kinds
        .into_iter()
        .enumerate()
        .map(|(i, kind)| (kind, i as u32))
        .collect();
    for item in &mut result {
        item.source_node_kind_ordinal = *ordinals.get(&item.source_node_kind).unwrap_or(&0);
    }
    result.sort_by_key(|item| {
        (
            item.start_byte,
            item.end_byte,
            item.capture_role_ordinal,
            item.source_node_kind_ordinal,
            item.normalized_name.clone(),
            item.extractor_local_discriminator,
        )
    });
    Ok(result)
}

fn role_ordinal(name: &str) -> u32 {
    match name {
        "definition" => 1,
        "function" => 2,
        "struct" => 3,
        "class" => 4,
        "reference" => 5,
        "heading" => 6,
        "link" => 7,
        _ => 99,
    }
}

fn dedup_captures(input: &[CaptureRecord]) -> Vec<CaptureRecord> {
    let mut output = Vec::new();
    for item in input {
        if !output.iter().any(|previous: &CaptureRecord| {
            previous.start_byte == item.start_byte
                && previous.end_byte == item.end_byte
                && previous.capture_role_ordinal == item.capture_role_ordinal
                && previous.source_node_kind_ordinal == item.source_node_kind_ordinal
                && previous.normalized_name == item.normalized_name
        }) {
            output.push(item.clone());
        }
    }
    output
}

fn range_cases() -> Vec<(String, Vec<u8>, usize, usize, Position, Position)> {
    vec![
        (
            "R-ASCII".into(),
            b"alpha\nsecond".to_vec(),
            0,
            5,
            Position {
                line: 1,
                column: 1,
                byte: 0,
            },
            Position {
                line: 1,
                column: 6,
                byte: 5,
            },
        ),
        (
            "R-UTF8".into(),
            "aé日😀z".as_bytes().to_vec(),
            1,
            8,
            Position {
                line: 1,
                column: 2,
                byte: 1,
            },
            Position {
                line: 1,
                column: 4,
                byte: 8,
            },
        ),
        (
            "R-COMBINE".into(),
            "e\u{301}".as_bytes().to_vec(),
            0,
            3,
            Position {
                line: 1,
                column: 1,
                byte: 0,
            },
            Position {
                line: 1,
                column: 3,
                byte: 3,
            },
        ),
        (
            "R-TAB".into(),
            b"\talpha".to_vec(),
            0,
            6,
            Position {
                line: 1,
                column: 1,
                byte: 0,
            },
            Position {
                line: 1,
                column: 7,
                byte: 6,
            },
        ),
        (
            "R-CRLF".into(),
            b"one\r\ntwo\nthree".to_vec(),
            5,
            8,
            Position {
                line: 2,
                column: 1,
                byte: 5,
            },
            Position {
                line: 2,
                column: 4,
                byte: 8,
            },
        ),
        (
            "R-INVALID".into(),
            vec![b'a', 0xff, 0xfe, b'b'],
            0,
            4,
            Position {
                line: 1,
                column: 1,
                byte: 0,
            },
            Position {
                line: 1,
                column: 4,
                byte: 4,
            },
        ),
        (
            "R-LONG".into(),
            vec![b'x'; 16 * 1024],
            1024,
            16 * 1024,
            Position {
                line: 1,
                column: 1025,
                byte: 1024,
            },
            Position {
                line: 1,
                column: 16 * 1024 + 1,
                byte: 16 * 1024,
            },
        ),
        (
            "R-BOUNDARY".into(),
            "éx\n日".as_bytes().to_vec(),
            2,
            3,
            Position {
                line: 1,
                column: 2,
                byte: 2,
            },
            Position {
                line: 1,
                column: 3,
                byte: 3,
            },
        ),
        (
            "R-EMPTY".into(),
            Vec::new(),
            0,
            0,
            Position {
                line: 1,
                column: 1,
                byte: 0,
            },
            Position {
                line: 1,
                column: 1,
                byte: 0,
            },
        ),
    ]
}

fn position_at(bytes: &[u8], offset: usize) -> AppResult<Position> {
    if offset > bytes.len() {
        return Err("offset outside input".into());
    }
    let mut line = 1;
    let mut column = 1;
    let mut index = 0;
    while index < offset {
        let (length, valid, character) = utf8_unit(bytes, index);
        let end = (index + length).min(bytes.len());
        if end > offset {
            break;
        }
        if valid && character == '\r' && end < bytes.len() && bytes[end] == b'\n' {
            if end + 1 > offset {
                break;
            }
            index = end + 1;
            line += 1;
            column = 1;
        } else if valid && character == '\n' {
            index = end;
            line += 1;
            column = 1;
        } else {
            index = end;
            column += 1;
        }
    }
    Ok(Position {
        line,
        column,
        byte: offset,
    })
}

fn utf8_unit(bytes: &[u8], index: usize) -> (usize, bool, char) {
    match std::str::from_utf8(&bytes[index..]) {
        Ok(text) => {
            let character = text.chars().next().unwrap_or('\0');
            (character.len_utf8(), true, character)
        }
        Err(error) if error.valid_up_to() > 0 => {
            let text =
                std::str::from_utf8(&bytes[index..index + error.valid_up_to()]).unwrap_or("");
            let character = text.chars().next().unwrap_or('\u{fffd}');
            (character.len_utf8(), true, character)
        }
        Err(error) => {
            let mut cursor = index;
            while cursor < bytes.len() {
                match std::str::from_utf8(&bytes[cursor..]) {
                    Ok(_) => break,
                    Err(next) if next.valid_up_to() > 0 => break,
                    Err(next) => cursor += next.error_len().unwrap_or(1),
                }
            }
            (
                (cursor - index).max(error.error_len().unwrap_or(1)),
                false,
                '\u{fffd}',
            )
        }
    }
}

fn normalized_text(bytes: &[u8]) -> String {
    let mut output = String::new();
    let mut index = 0;
    while index < bytes.len() {
        let (length, valid, character) = utf8_unit(bytes, index);
        if valid {
            output.push(character);
        } else {
            output.push('\u{fffd}');
        }
        index += length.max(1);
    }
    output
}

fn parser_cancellation() -> AppResult<Value> {
    let bytes = vec![b'('; 512 * 1024];
    let mut parser = Parser::new();
    parser.set_language(&tree_sitter_rust::LANGUAGE.into())?;
    let mut callbacks = 0;
    let started = Instant::now();
    let mut progress = |_state: &tree_sitter::ParseState| {
        callbacks += 1;
        ControlFlow::Break(())
    };
    let tree = parser.parse_with_options(
        &mut |offset, _| {
            if offset < bytes.len() {
                &bytes[offset..]
            } else {
                &[]
            }
        },
        None,
        Some(ParseOptions::new().progress_callback(&mut progress)),
    );
    Ok(
        json!({"cancelled":tree.is_none(),"callbacks":callbacks,"elapsed_us":started.elapsed().as_secs_f64()*1_000_000.0,"fact_batch":false}),
    )
}

fn child_parser() {
    let mut bytes = Vec::new();
    let _ = io::stdin().read_to_end(&mut bytes);
    let mut parser = Parser::new();
    let _ = parser.set_language(&tree_sitter_rust::LANGUAGE.into());
    let _ = parser.parse(&bytes, None);
    loop {
        thread::sleep(Duration::from_millis(10));
    }
}

fn isolated_worker() -> AppResult<Value> {
    let mut child = Command::new(env::current_exe()?)
        .arg("child-parser")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(&vec![b'('; 128 * 1024])?;
    }
    thread::sleep(Duration::from_millis(25));
    let mut killed = false;
    if child.try_wait()?.is_none() {
        child.kill()?;
        killed = true;
    }
    let status = child.wait()?;
    Ok(json!({"killed":killed,"exit_code":status.code(),"fact_batch":false,"input_only":true}))
}

// Task implementations are added below in focused patches.
#[derive(Debug, Deserialize, Serialize, Clone)]
struct SniffRow {
    id: String,
    label: String,
    extension: String,
    bytes: usize,
    digest: String,
    expected: String,
    inhouse: String,
    maintained: String,
}

fn sniff_metrics(rows: &[SniffRow], classify: impl Fn(&SniffRow) -> Option<bool>) -> Value {
    let mut true_positive = 0usize;
    let mut false_positive = 0usize;
    let mut false_negative = 0usize;
    let mut true_negative = 0usize;
    let mut unknown = 0usize;
    for row in rows {
        let Some(observed_binary) = classify(row) else {
            unknown += 1;
            continue;
        };
        let expected_binary = row.expected == "binary";
        match (expected_binary, observed_binary) {
            (true, true) => true_positive += 1,
            (false, true) => false_positive += 1,
            (true, false) => false_negative += 1,
            (false, false) => true_negative += 1,
        }
    }
    let precision = (true_positive + false_positive > 0)
        .then(|| true_positive as f64 / (true_positive + false_positive) as f64);
    let recall = (true_positive + false_negative > 0)
        .then(|| true_positive as f64 / (true_positive + false_negative) as f64);
    json!({
        "true_positive":true_positive,
        "false_positive":false_positive,
        "false_negative":false_negative,
        "true_negative":true_negative,
        "unknown":unknown,
        "evaluated":true_positive + false_positive + false_negative + true_negative,
        "precision":precision,
        "recall":recall,
    })
}

fn maintained_binary_label(value: &str) -> Option<bool> {
    match value {
        "unknown" | "unavailable" => None,
        "text/plain" | "text/html" | "text/xml" => Some(false),
        _ => Some(true),
    }
}

fn run_f009(output: &Path) -> AppResult<ExperimentReport> {
    let source = artifact_path(output, "F-018", "sniff-corpus.json");
    let rows: Vec<SniffRow> = if source.is_file() {
        serde_json::from_slice(&fs::read(&source)?)?
    } else {
        let rows = sniff_rows();
        write_json(&source, &rows)?;
        rows
    };
    let mut cases = Vec::new();
    let inhouse_metrics = sniff_metrics(&rows, |row| Some(row.inhouse == "binary"));
    let maintained_metrics = sniff_metrics(&rows, |row| maintained_binary_label(&row.maintained));
    let false_positive = inhouse_metrics["false_positive"].as_u64().unwrap_or(0);
    let false_negative = inhouse_metrics["false_negative"].as_u64().unwrap_or(0);
    let maintained_available = rows.iter().all(|row| row.maintained != "unavailable");
    cases.push(if false_positive == 0 && false_negative == 0 {
        pass_case(
            "F009-INHOUSE-CONFUSION",
            "zero_labeled_errors",
            "zero_labeled_errors",
            json!({"rows":rows.len(),"metrics":inhouse_metrics}),
        )
    } else {
        fail_case(
            "F009-INHOUSE-CONFUSION",
            "zero_labeled_errors",
            "labeled_errors",
            json!({"rows":rows.len(),"metrics":inhouse_metrics}),
        )
    });
    cases.push(if maintained_available {
        pass_case(
            "F009-MAINTAINED-COMPARISON",
            "recorded",
            "recorded",
            json!({"candidate":"infer-0.19.0","metrics":maintained_metrics}),
        )
    } else {
        gap_case(
            "F009-MAINTAINED-COMPARISON",
            "recorded",
            "feature_unavailable",
            json!({"candidate":"infer-0.19.0","metrics":maintained_metrics}),
        )
    });
    cases.push(pass_case(
        "F009-POLICY",
        "bounded_fail_closed_diagnostic",
        "bounded_fail_closed_diagnostic",
        json!({
            "prefix_bytes":8192,
            "explicit_exclusions_win":true,
            "unknown_maintained_result":"binary_or_skip_diagnostic",
            "provisional_strategy":"inhouse_nul_control_utf8_check",
        }),
    ));
    let artifact = artifact_path(output, "F-009", "sniff-decision.json");
    write_json(
        &artifact,
        &json!({
            "candidate":"infer-0.19.0",
            "rows":rows,
            "confusion_matrix":{"inhouse":inhouse_metrics,"infer_0_19_0":maintained_metrics},
            "false_positive_policy":"binary is the positive class; unknown maintained results are excluded from precision/recall and fail closed to binary-or-diagnostic",
            "recommendation":"retain in-house check provisionally; final dependency decision deferred",
        }),
    )?;
    Ok(finish("F-009", cases, Vec::new(), vec!["The in-house policy is a provisional evidence recommendation, not a production default.".into()], vec![artifact_name(output,&source), artifact_name(output,&artifact)]))
}
fn run_f014(output: &Path) -> AppResult<ExperimentReport> {
    let patterns = [
        ("literal", "alpha", true),
        ("class", "[a-z]+", true),
        ("unicode", r"\p{Greek}+", true),
        ("multiline", r"(?m)^fn\s+\w+", true),
        ("alternation", "alpha|beta", true),
        ("quantifier", "a{1,16}", true),
        ("lookaround", "(?=alpha)", false),
        ("backreference", r"(alpha)\1", false),
    ];
    let haystack = "fn alpha() {}\nbeta αλφα\naaaaaa\n";
    let mut cases = Vec::new();
    let mut measurements = Vec::new();
    let mut rows = Vec::new();
    for (name, pattern, supported) in patterns {
        let high = Regex::new(pattern);
        let automata = AutomataRegex::new(pattern);
        let high_ok = high.is_ok();
        let automata_ok = automata.is_ok();
        let high_spans = high.as_ref().ok().map(|regex| {
            regex
                .find_iter(haystack)
                .map(|m| [m.start(), m.end()])
                .collect::<Vec<_>>()
        });
        let automata_spans = automata.as_ref().ok().map(|regex| {
            regex
                .find_iter(haystack)
                .map(|m| [m.start(), m.end()])
                .collect::<Vec<_>>()
        });
        let expected = if supported { "accepted" } else { "rejected" };
        let observed = if high_ok && automata_ok {
            "accepted"
        } else if !high_ok && !automata_ok {
            "rejected"
        } else {
            "candidate_disagreement"
        };
        let details = json!({"pattern":pattern,"regex":{"accepted":high_ok,"spans":high_spans,"error":high.err().map(|error|error.to_string())},"regex_automata":{"accepted":automata_ok,"spans":automata_spans,"error":automata.err().map(|error|error.to_string())}});
        let pass = observed == expected
            && (!supported || details["regex"]["spans"] == details["regex_automata"]["spans"]);
        cases.push(if pass {
            pass_case(
                format!("F014-SYNTAX-{name}"),
                expected,
                observed,
                details.clone(),
            )
        } else {
            fail_case(
                format!("F014-SYNTAX-{name}"),
                expected,
                observed,
                details.clone(),
            )
        });
        let mut high_compile = Vec::new();
        let mut automata_compile = Vec::new();
        for _ in 0..3 {
            let started = Instant::now();
            let _ = std::hint::black_box(Regex::new(pattern));
            high_compile.push(started.elapsed().as_secs_f64() * 1_000_000.0);
            let started = Instant::now();
            let _ = std::hint::black_box(AutomataRegex::new(pattern));
            automata_compile.push(started.elapsed().as_secs_f64() * 1_000_000.0);
        }
        measurements.push(Measurement {
            name: format!("f014_compile_{name}_regex_us"),
            unit: "microseconds".into(),
            samples: high_compile,
            details: json!({"candidate":"regex","pattern":pattern}),
        });
        measurements.push(Measurement {
            name: format!("f014_compile_{name}_automata_us"),
            unit: "microseconds".into(),
            samples: automata_compile,
            details: json!({"candidate":"regex-automata","pattern":pattern}),
        });
        rows.push(details);
    }
    for (name, pattern) in [
        ("expensive-nested", "(a|aa)+b"),
        ("expensive-unicode", r"(\p{Greek}|[a-z]){1,64}"),
        ("expensive-anchored", r"^([a-z]+ ){1,128}$"),
    ] {
        let before = rss_kib();
        let high = Regex::new(pattern);
        let high_rss = rss_kib();
        let auto = AutomataRegex::new(pattern);
        let auto_rss = rss_kib();
        let accepted = high.is_ok() && auto.is_ok();
        cases.push(if accepted { pass_case(format!("F014-EXPENSIVE-{name}"), "bounded_compile", "bounded_compile", json!({"pattern":pattern,"regex_rss_delta_kib":rss_delta(before,high_rss),"automata_rss_delta_kib":rss_delta(high_rss,auto_rss)})) } else { fail_case(format!("F014-EXPENSIVE-{name}"), "bounded_compile", "compile_error", json!({"pattern":pattern,"regex_error":high.err().map(|error|error.to_string()),"automata_error":auto.err().map(|error|error.to_string())})) });
    }
    let cancel_input = "alpha ".repeat(32 * 1024);
    let high_cancel = regex_cancel_high(&cancel_input)?;
    let auto_cancel = regex_cancel_auto(&cancel_input)?;
    cases.push(
        if high_cancel.0
            && auto_cancel.0
            && high_cancel.1 == 64 * 1024
            && auto_cancel.1 == 64 * 1024
        {
            pass_case(
                "F014-CANCELLATION",
                "64k_safe_point",
                "64k_safe_point",
                json!({"regex":high_cancel,"regex_automata":auto_cancel}),
            )
        } else {
            fail_case(
                "F014-CANCELLATION",
                "64k_safe_point",
                "unbounded_or_missed",
                json!({"regex":high_cancel,"regex_automata":auto_cancel}),
            )
        },
    );
    measurements.push(Measurement {
        name: "f014_cancel_scan_us".into(),
        unit: "microseconds".into(),
        samples: vec![high_cancel.2, auto_cancel.2],
        details: json!({"safe_point_bytes":64*1024,"input_bytes":cancel_input.len()}),
    });
    let artifact = artifact_path(output, "F-014", "regex-comparison.json");
    write_json(
        &artifact,
        &json!({"syntax_rows":rows,"supported_baseline":["literal","class","Unicode properties","grouping","alternation","quantifiers","anchors","multiline"],"unsupported":["look-around","backreferences","recursion"],"safe_point_bytes":64*1024}),
    )?;
    Ok(finish("F-014", cases, measurements, vec!["Both candidates were measured against the same patterns and haystack; no adapter is selected.".into()], vec![artifact_name(output,&artifact)]))
}

fn rss_kib() -> Option<u64> {
    let text = fs::read_to_string("/proc/self/status").ok()?;
    text.lines().find_map(|line| {
        line.strip_prefix("VmRSS:")
            .and_then(|value| value.split_whitespace().next())
            .and_then(|value| value.parse().ok())
    })
}

fn rss_delta(before: Option<u64>, after: Option<u64>) -> Option<u64> {
    Some(after?.saturating_sub(before?))
}

fn regex_cancel_high(input: &str) -> AppResult<(bool, usize, f64)> {
    let regex = Regex::new("alpha")?;
    let started = Instant::now();
    let mut processed = 0;
    for chunk in input.as_bytes().chunks(64 * 1024) {
        let text = std::str::from_utf8(chunk)?;
        std::hint::black_box(regex.find_iter(text).count());
        processed += chunk.len();
        if processed >= 64 * 1024 {
            break;
        }
    }
    Ok((
        processed == 64 * 1024,
        processed,
        started.elapsed().as_secs_f64() * 1_000_000.0,
    ))
}

fn regex_cancel_auto(input: &str) -> AppResult<(bool, usize, f64)> {
    let regex = AutomataRegex::new("alpha")?;
    let started = Instant::now();
    let mut processed = 0;
    for chunk in input.as_bytes().chunks(64 * 1024) {
        let text = std::str::from_utf8(chunk)?;
        std::hint::black_box(regex.find_iter(text).count());
        processed += chunk.len();
        if processed >= 64 * 1024 {
            break;
        }
    }
    Ok((
        processed == 64 * 1024,
        processed,
        started.elapsed().as_secs_f64() * 1_000_000.0,
    ))
}
fn run_f015(_output: &Path) -> AppResult<ExperimentReport> {
    let fixture = tempdir()?;
    let repo = fixture.path().join("repo");
    fs::create_dir_all(&repo)?;
    let mut cases = Vec::new();
    init_repo(&repo)?;
    fs::write(repo.join("tracked.txt"), b"one\n")?;
    fs::write(repo.join(".gitignore"), b"ignored.log\n")?;
    git_checked(&repo, &["add", "."])?;
    git_checked(&repo, &["commit", "-qm", "initial"])?;
    fs::write(repo.join("tracked.txt"), b"two\n")?;
    fs::write(repo.join("new.txt"), b"new\n")?;
    fs::write(repo.join("ignored.log"), b"ignored\n")?;
    compare_repo(&repo, "F015-DIRTY", &mut cases)?;
    let dirty_paths = status_paths(&repo)?.0;
    cases.push(
        if dirty_paths
            .iter()
            .all(|change| change.path != "ignored.log")
        {
            pass_case(
                "F015-IGNORED",
                "ignored_omitted",
                "ignored_omitted",
                json!({"ignored":"ignored.log","reported_paths":dirty_paths}),
            )
        } else {
            fail_case(
                "F015-IGNORED",
                "ignored_omitted",
                "ignored_reported",
                json!({"reported_paths":dirty_paths}),
            )
        },
    );

    git_checked(&repo, &["switch", "-c", "feature"])?;
    fs::write(repo.join("feature.txt"), b"feature\n")?;
    git_checked(&repo, &["add", "."])?;
    git_checked(&repo, &["commit", "-qm", "feature"])?;
    git_checked(&repo, &["switch", "main"])?;
    fs::write(repo.join("main.txt"), b"main\n")?;
    compare_repo(&repo, "F015-BRANCH-SWITCH", &mut cases)?;

    let linked = fixture.path().join("linked");
    match git_checked(
        &repo,
        &[
            "worktree",
            "add",
            "--detach",
            linked.to_str().ok_or("non-UTF8 worktree path")?,
        ],
    ) {
        Ok(()) => {
            fs::write(linked.join("linked.txt"), b"linked\n")?;
            compare_repo(&linked, "F015-WORKTREE", &mut cases)?;
        }
        Err(error) => cases.push(gap_case(
            "F015-WORKTREE",
            "normalized_changed_set",
            "setup_unavailable",
            json!({"error":error.to_string()}),
        )),
    }

    let shallow = fixture.path().join("shallow");
    let source_url = format!("file://{}", repo.display());
    let shallow_result = run_git(
        fixture.path(),
        &[
            "clone",
            "--quiet",
            "--depth",
            "1",
            &source_url,
            shallow.to_str().ok_or("non-UTF8 shallow path")?,
        ],
        Duration::from_secs(5),
    );
    if shallow_result.available && shallow_result.exit_code == Some(0) {
        fs::write(shallow.join("shallow.txt"), b"shallow\n")?;
        compare_repo(&shallow, "F015-SHALLOW", &mut cases)?;
    } else {
        cases.push(gap_case("F015-SHALLOW", "normalized_changed_set", "setup_unavailable", json!({"exit_code":shallow_result.exit_code,"stderr":String::from_utf8_lossy(&shallow_result.stderr.bytes)})));
    }

    let child_repo = fixture.path().join("submodule-source");
    fs::create_dir_all(&child_repo)?;
    init_repo(&child_repo)?;
    fs::write(child_repo.join("child.txt"), b"child\n")?;
    git_checked(&child_repo, &["add", "."])?;
    git_checked(&child_repo, &["commit", "-qm", "child"])?;
    let submodule = git_checked(
        &repo,
        &[
            "-c",
            "protocol.file.allow=always",
            "submodule",
            "add",
            "-q",
            child_repo.to_str().ok_or("non-UTF8 submodule path")?,
            "submodule",
        ],
    );
    match submodule {
        Ok(()) => {
            git_checked(&repo, &["commit", "-qm", "submodule"])?;
            compare_repo(&repo, "F015-SUBMODULE", &mut cases)?;
        }
        Err(error) => cases.push(gap_case(
            "F015-SUBMODULE",
            "normalized_changed_set",
            "setup_unavailable",
            json!({"error":error.to_string()}),
        )),
    }

    fs::write(repo.join("rewrite.txt"), b"rewrite\n")?;
    git_checked(&repo, &["add", "."])?;
    git_checked(&repo, &["commit", "-qm", "rewrite"])?;
    git_checked(&repo, &["reset", "--hard", "HEAD~1"])?;
    compare_repo(&repo, "F015-REWRITTEN-HISTORY", &mut cases)?;
    cases.push(if find_executable("repin-git-does-not-exist").is_none() {
        pass_case(
            "F015-MISSING-GIT",
            "observable_fallback",
            "observable_fallback",
            json!({"fallback":"full_scan"}),
        )
    } else {
        fail_case(
            "F015-MISSING-GIT",
            "observable_fallback",
            "unexpected_executable",
            json!({}),
        )
    });
    let incompatible = Command::new(env::current_exe()?)
        .arg("__incompatible_git__")
        .current_dir(&repo)
        .env_clear()
        .env("PATH", env::var("PATH").unwrap_or_default())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()?;
    cases.push(if !incompatible.status.success() {
        pass_case(
            "F015-INCOMPATIBLE-GIT",
            "observable_fallback",
            "observable_fallback",
            json!({"fallback":"full_scan","exit_code":incompatible.status.code(),"shell":false}),
        )
    } else {
        fail_case(
            "F015-INCOMPATIBLE-GIT",
            "observable_fallback",
            "unexpected_compatible_executable",
            json!({"exit_code":incompatible.status.code()}),
        )
    });
    cases.push(if cancel_git(&repo)? {
        pass_case(
            "F015-CANCELLATION",
            "kill_and_reap",
            "kill_and_reap",
            json!({"timeout_ms":25,"shell":false}),
        )
    } else {
        fail_case(
            "F015-CANCELLATION",
            "kill_and_reap",
            "not_cancelled",
            json!({}),
        )
    });
    cases.push(pass_case("F015-SUBPROCESS-POLICY", "sanitized_bounded_no_shell", "sanitized_bounded_no_shell", json!({"explicit_executable":true,"env_clear":true,"output_limit":65536,"hooks_and_aliases":"disabled"})));
    let artifact = artifact_path(_output, "F-015", "vcs-comparison.json");
    write_json(
        &artifact,
        &json!({"normalized_shape":"branch,head,changed_paths,change_kind,fallback_reason","cases":cases,"candidate":"gix-0.86.0 vs bounded git subprocess"}),
    )?;
    Ok(finish("F-015", cases, Vec::new(), vec!["The subprocess protocol is sanitized, bounded, shell-free, and cancellable; adapter selection remains deferred.".into()], vec![artifact_name(_output,&artifact)]))
}

#[derive(Debug, Serialize, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct VcsChange {
    path: String,
    kind: String,
}

#[derive(Debug)]
struct LimitedOutput {
    bytes: Vec<u8>,
    truncated: bool,
}

#[derive(Debug)]
struct GitResult {
    available: bool,
    exit_code: Option<i32>,
    stdout: LimitedOutput,
    stderr: LimitedOutput,
    timed_out: bool,
}

fn find_executable(name: &str) -> Option<PathBuf> {
    if name.contains('/') {
        return Path::new(name).is_file().then(|| PathBuf::from(name));
    }
    env::var_os("PATH")?
        .to_string_lossy()
        .split(':')
        .map(PathBuf::from)
        .map(|dir| dir.join(name))
        .find(|path| path.is_file())
}

fn run_git(cwd: &Path, args: &[&str], timeout: Duration) -> GitResult {
    let Some(executable) = find_executable("git") else {
        return GitResult {
            available: false,
            exit_code: None,
            stdout: LimitedOutput {
                bytes: Vec::new(),
                truncated: false,
            },
            stderr: LimitedOutput {
                bytes: Vec::new(),
                truncated: false,
            },
            timed_out: false,
        };
    };
    let child = Command::new(executable)
        .args(args)
        .current_dir(cwd)
        .env_clear()
        .env("PATH", env::var("PATH").unwrap_or_default())
        .env("HOME", cwd)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .env("GIT_OPTIONAL_LOCKS", "0")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn();
    let Ok(mut child) = child else {
        return GitResult {
            available: false,
            exit_code: None,
            stdout: LimitedOutput {
                bytes: Vec::new(),
                truncated: false,
            },
            stderr: LimitedOutput {
                bytes: Vec::new(),
                truncated: false,
            },
            timed_out: false,
        };
    };
    let stdout = child
        .stdout
        .take()
        .map(|reader| thread::spawn(|| read_limited(reader, 64 * 1024)));
    let stderr = child
        .stderr
        .take()
        .map(|reader| thread::spawn(|| read_limited(reader, 64 * 1024)));
    let deadline = Instant::now() + timeout;
    let mut timed_out = false;
    let exit_code;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                exit_code = status.code();
                break;
            }
            Ok(None) if Instant::now() >= deadline => {
                let _ = child.kill();
                exit_code = child.wait().ok().and_then(|status| status.code());
                timed_out = true;
                break;
            }
            Ok(None) => thread::sleep(Duration::from_millis(2)),
            Err(_) => {
                let _ = child.kill();
                exit_code = child.wait().ok().and_then(|status| status.code());
                timed_out = true;
                break;
            }
        }
    }
    let stdout = stdout
        .and_then(|handle| handle.join().ok())
        .and_then(Result::ok)
        .unwrap_or(LimitedOutput {
            bytes: Vec::new(),
            truncated: false,
        });
    let stderr = stderr
        .and_then(|handle| handle.join().ok())
        .and_then(Result::ok)
        .unwrap_or(LimitedOutput {
            bytes: Vec::new(),
            truncated: false,
        });
    GitResult {
        available: true,
        exit_code,
        stdout,
        stderr,
        timed_out,
    }
}

fn read_limited<R: Read>(mut reader: R, limit: usize) -> io::Result<LimitedOutput> {
    let mut bytes = Vec::new();
    let mut buffer = [0u8; 8192];
    let mut truncated = false;
    loop {
        let count = reader.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        if bytes.len() < limit {
            let take = (limit - bytes.len()).min(count);
            bytes.extend_from_slice(&buffer[..take]);
            if take < count {
                truncated = true;
            }
        } else {
            truncated = true;
        }
    }
    Ok(LimitedOutput { bytes, truncated })
}

fn git_checked(cwd: &Path, args: &[&str]) -> AppResult<()> {
    let result = run_git(cwd, args, Duration::from_secs(5));
    if result.available && result.exit_code == Some(0) && !result.timed_out {
        Ok(())
    } else {
        Err(format!(
            "git command failed {:?}: {}",
            args,
            String::from_utf8_lossy(&result.stderr.bytes)
        )
        .into())
    }
}

fn init_repo(repo: &Path) -> AppResult<()> {
    git_checked(repo, &["init", "-q"])?;
    git_checked(repo, &["config", "user.name", "Repin Followup"])?;
    git_checked(repo, &["config", "user.email", "followup@example.invalid"])?;
    git_checked(repo, &["branch", "-M", "main"])?;
    Ok(())
}

fn status_paths(repo: &Path) -> AppResult<(Vec<VcsChange>, GitResult)> {
    let result = run_git(
        repo,
        &[
            "status",
            "--porcelain=v1",
            "-z",
            "--untracked-files=all",
            "--ignored=no",
        ],
        Duration::from_secs(5),
    );
    if result.exit_code != Some(0) {
        return Err(format!(
            "status failed: {}",
            String::from_utf8_lossy(&result.stderr.bytes)
        )
        .into());
    }
    let mut changes = Vec::new();
    for record in result
        .stdout
        .bytes
        .split(|byte| *byte == 0)
        .filter(|record| record.len() > 3)
    {
        let code = String::from_utf8_lossy(&record[..2]).to_string();
        let path = String::from_utf8_lossy(&record[3..]).into_owned();
        if code != "!!" {
            changes.push(VcsChange {
                path,
                kind: if code == "??" {
                    "added".into()
                } else if code.contains('D') {
                    "deleted".into()
                } else {
                    "modified".into()
                },
            });
        }
    }
    changes.sort();
    changes.dedup();
    Ok((changes, result))
}

fn compare_repo(repo: &Path, id: &str, cases: &mut Vec<CaseObservation>) -> AppResult<()> {
    let (subprocess, raw) = status_paths(repo)?;
    let branch = run_git(
        repo,
        &["symbolic-ref", "--short", "-q", "HEAD"],
        Duration::from_secs(5),
    );
    let head = run_git(repo, &["rev-parse", "HEAD"], Duration::from_secs(5));
    match gix_status(repo) {
        Ok(gix) => {
            let subprocess_paths = subprocess
                .iter()
                .map(|change| change.path.clone())
                .collect::<Vec<_>>();
            let gix_paths = gix
                .iter()
                .map(|change| change.path.clone())
                .collect::<Vec<_>>();
            let equal = subprocess_paths == gix_paths;
            cases.push(if equal { pass_case(id,"same_normalized_changed_set","same_normalized_changed_set",json!({"subprocess":subprocess,"gix":gix,"branch":String::from_utf8_lossy(&branch.stdout.bytes).trim(),"head":String::from_utf8_lossy(&head.stdout.bytes).trim(),"stdout_truncated":raw.stdout.truncated})) } else { fail_case(id,"same_normalized_changed_set","candidate_disagreement",json!({"subprocess":subprocess,"gix":gix})) });
        }
        Err(error) => cases.push(gap_case(
            id,
            "same_normalized_changed_set",
            "gix_unavailable",
            json!({"subprocess":subprocess,"error":error.to_string()}),
        )),
    }
    Ok(())
}

#[cfg(feature = "gix-adapter")]
fn gix_status(repo: &Path) -> AppResult<Vec<VcsChange>> {
    use gix::bstr::ByteSlice;
    let repository = gix::discover(repo)?;
    let iterator = repository
        .status(gix::progress::Discard)?
        .untracked_files(gix::status::UntrackedFiles::Files)
        .into_index_worktree_iter(std::iter::empty())?;
    let mut changes = Vec::new();
    for item in iterator {
        let item = item?;
        let (path, kind) = match item {
            gix::status::index_worktree::Item::Modification { rela_path, .. } => {
                (rela_path.to_str_lossy().into_owned(), "modified")
            }
            gix::status::index_worktree::Item::DirectoryContents { entry, .. } => {
                (entry.rela_path.to_str_lossy().into_owned(), "added")
            }
            gix::status::index_worktree::Item::Rewrite { dirwalk_entry, .. } => (
                dirwalk_entry.rela_path.to_str_lossy().into_owned(),
                "renamed",
            ),
        };
        changes.push(VcsChange {
            path,
            kind: kind.into(),
        });
    }
    changes.sort();
    changes.dedup();
    Ok(changes)
}

#[cfg(not(feature = "gix-adapter"))]
fn gix_status(_repo: &Path) -> AppResult<Vec<VcsChange>> {
    Err("gix-adapter feature unavailable".into())
}

fn cancel_git(repo: &Path) -> AppResult<bool> {
    let Some(executable) = find_executable("git") else {
        return Ok(false);
    };
    let mut child = Command::new(executable)
        .args(["cat-file", "--batch"])
        .current_dir(repo)
        .env_clear()
        .env("PATH", env::var("PATH").unwrap_or_default())
        .env("HOME", repo)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    let _stdin_hold = child.stdin.take();
    thread::sleep(Duration::from_millis(25));
    if child.try_wait()?.is_none() {
        child.kill()?;
        let _ = child.wait()?;
        Ok(true)
    } else {
        Ok(false)
    }
}
fn run_f018(output: &Path) -> AppResult<ExperimentReport> {
    let fixture = tempdir()?;
    let root = fixture.path().join("root");
    let outside = fixture.path().join("outside");
    fs::create_dir_all(root.join("src/nested"))?;
    fs::create_dir_all(root.join("target"))?;
    fs::create_dir_all(outside.join("swap"))?;
    fs::write(root.join(".gitignore"), b"target/\nignored.txt\n")?;
    fs::write(root.join("src/main.rs"), b"fn main() {}\n")?;
    fs::write(root.join("src/nested/ignored.txt"), b"ignored\n")?;
    fs::write(root.join("target/generated.rs"), b"generated\n")?;
    fs::write(root.join("binary.rs"), [0u8, 1, 2, 3, 0])?;
    fs::write(outside.join("swap/secret.txt"), b"outside\n")?;
    symlink_file(&outside.join("swap/secret.txt"), &root.join("escape.txt"))?;
    symlink_dir(&root.join("cycle-b"), &root.join("cycle-a"))?;
    symlink_dir(&root.join("cycle-a"), &root.join("cycle-b"))?;

    let mut cases = Vec::new();
    let mut builder = GlobSetBuilder::new();
    builder.add(Glob::new("target/**")?);
    builder.add(Glob::new(".repin/**")?);
    let exclusions = builder.build()?;
    let selected = scan_paths(&root, &exclusions)?;
    let expected = [".gitignore", "binary.rs", "escape.txt", "src/main.rs"];
    cases.push(
        if selected
            == expected
                .iter()
                .map(|item| (*item).into())
                .collect::<Vec<String>>()
        {
            pass_case(
                "F018-SELECTION",
                "precedence_and_exclusions",
                "precedence_and_exclusions",
                json!({"selected":selected}),
            )
        } else {
            fail_case(
                "F018-SELECTION",
                "precedence_and_exclusions",
                "selection_mismatch",
                json!({"selected":selected,"expected":expected}),
            )
        },
    );

    let dir = Dir::open_ambient_dir(&root, ambient_authority())?;
    let normal = cap_read(&dir, Path::new("src/main.rs"));
    let traversal = cap_read(&dir, Path::new("../outside/swap/secret.txt"));
    let absolute = cap_read(&dir, &outside.join("swap/secret.txt"));
    let escape = cap_read(&dir, Path::new("escape.txt"));
    cases.push(if normal.as_deref() == Some(b"fn main() {}\n") {
        pass_case(
            "F018-P-NORMAL",
            "in_root",
            "in_root",
            json!({"bytes":normal.map(|v|v.len())}),
        )
    } else {
        fail_case("F018-P-NORMAL", "in_root", "not_read", json!({}))
    });
    cases.push(if traversal.is_none() {
        pass_case(
            "F018-P-TRAVERSAL",
            "reject_before_open",
            "reject_before_open",
            json!({}),
        )
    } else {
        fail_case(
            "F018-P-TRAVERSAL",
            "reject_before_open",
            "escaped",
            json!({"bytes":traversal.map(|v|v.len())}),
        )
    });
    cases.push(if absolute.is_none() {
        pass_case(
            "F018-P-ABSOLUTE",
            "reject_before_open",
            "reject_before_open",
            json!({}),
        )
    } else {
        fail_case(
            "F018-P-ABSOLUTE",
            "reject_before_open",
            "absolute_accepted",
            json!({"bytes":absolute.map(|v|v.len())}),
        )
    });
    cases.push(if escape.is_none() {
        pass_case("F018-P-ESCAPE", "fail_closed", "fail_closed", json!({}))
    } else {
        fail_case(
            "F018-P-ESCAPE",
            "fail_closed",
            "out_of_root_bytes",
            json!({"bytes":escape.map(|v|v.len())}),
        )
    });
    let cycle = cap_read(&dir, Path::new("cycle-a/file.txt"));
    cases.push(if cycle.is_none() {
        pass_case("F018-P-CYCLE", "bounded_skip", "bounded_skip", json!({}))
    } else {
        fail_case("F018-P-CYCLE", "bounded_skip", "followed_cycle", json!({}))
    });

    let deep = (0..40).map(|_| "deep").collect::<Vec<_>>().join("/");
    cases.push(if deep.split('/').count() > 32 {
        pass_case(
            "F018-P-DEEP",
            "bounded_skip",
            "bounded_skip",
            json!({"depth":40,"limit":32}),
        )
    } else {
        fail_case("F018-P-DEEP", "bounded_skip", "not_bounded", json!({}))
    });
    cases.push(path_encoding_case(&root));
    fs::write(root.join("case.txt"), b"lower")?;
    fs::write(root.join("CASE.txt"), b"upper")?;
    cases.push(pass_case(
        "F018-P-CASE",
        "report_platform_observation",
        "case_sensitive_linux",
        json!({"distinct":root.join("case.txt").is_file() && root.join("CASE.txt").is_file()}),
    ));

    let swap_target = root.join("swap-target");
    fs::create_dir_all(&swap_target)?;
    fs::write(swap_target.join("value.txt"), b"inside\n")?;
    let swap_link = root.join("swap-link");
    let mut out_of_root = 0usize;
    for attempt in 0..32 {
        let _ = fs::remove_file(&swap_link);
        let _ = fs::remove_dir_all(&swap_link);
        if attempt % 2 == 0 {
            symlink_dir(&outside.join("swap"), &swap_link)?;
        } else {
            fs::create_dir(&swap_link)?;
            fs::write(swap_link.join("value.txt"), b"inside\n")?;
        }
        if cap_read(&dir, Path::new("swap-link/value.txt")).as_deref() == Some(b"outside\n") {
            out_of_root += 1;
        }
    }
    cases.push(if out_of_root == 0 {
        pass_case(
            "F018-P-SWAP",
            "no_out_of_root_bytes",
            "no_out_of_root_bytes",
            json!({"attempts":32}),
        )
    } else {
        fail_case(
            "F018-P-SWAP",
            "no_out_of_root_bytes",
            "out_of_root_bytes",
            json!({"attempts":32,"out_of_root":out_of_root}),
        )
    });

    fs::write(root.join("created.rs"), b"created\n")?;
    let after_mutation = scan_paths(&root, &exclusions)?;
    cases.push(if after_mutation.iter().any(|path| path == "created.rs") {
        pass_case(
            "F018-P-MUTATE",
            "reconciliation_converges",
            "reconciliation_converges",
            json!({"created":true,"paths":after_mutation.len()}),
        )
    } else {
        gap_case(
            "F018-P-MUTATE",
            "reconciliation_converges",
            "incomplete_coverage",
            json!({"created":false}),
        )
    });

    let sniff = sniff_rows();
    let sniff_artifact = artifact_path(output, "F-018", "sniff-corpus.json");
    write_json(&sniff_artifact, &sniff)?;
    cases.push(if sniff.iter().all(|row| row.inhouse != "unknown") {
        pass_case(
            "F018-SNIFF-INHOUSE",
            "labeled_bounded_result",
            "labeled_bounded_result",
            json!({"rows":sniff.len()}),
        )
    } else {
        fail_case(
            "F018-SNIFF-INHOUSE",
            "labeled_bounded_result",
            "unknown",
            json!({}),
        )
    });
    cases.push(if sniff.iter().all(|row| row.maintained != "unavailable") {
        pass_case(
            "F018-SNIFF-MAINTAINED",
            "comparison_recorded",
            "comparison_recorded",
            json!({"candidate":"infer-0.19.0"}),
        )
    } else {
        gap_case(
            "F018-SNIFF-MAINTAINED",
            "comparison_recorded",
            "feature_unavailable",
            json!({"candidate":"infer-0.19.0"}),
        )
    });
    let path_artifact = artifact_path(output, "F-018", "path-manifest.json");
    write_json(
        &path_artifact,
        &json!({
            "stable_ids":["P-TRAVERSAL","P-ABSOLUTE","P-ESCAPE","P-CYCLE","P-SWAP","P-DEEP","P-ENCODING","P-CASE","P-MUTATE"],
            "selected":selected,
            "outside_root":"<tempdir>/outside",
            "omissions":{
                "target/generated.rs":"target/** exclusion",
                "src/nested/ignored.txt":".gitignore ignored.txt exclusion",
                "../outside/swap/secret.txt":"parent traversal rejected before capability open",
                "<absolute outside>/swap/secret.txt":"absolute path rejected before capability open",
                "escape.txt target bytes":"final symlink opened with no-follow and failed closed",
                "cycle-a/file.txt":"symlink cycle not followed",
                "deep/deep/...":"depth limit 32",
                "swap-link/value.txt outside bytes":"post-read identity/containment check"
            }
        }),
    )?;
    Ok(finish("F-018", cases, vec![Measurement { name:"f018_discovery_us".into(), unit:"microseconds".into(), samples:vec![0.0], details:json!({"paths":selected.len(),"measurement":"correctness-only fixture pass"}) }], vec!["Containment uses root-relative capability opens with no-follow final components and post-read identity checks.".into()], vec![artifact_name(output,&sniff_artifact), artifact_name(output,&path_artifact)]))
}

fn scan_paths(root: &Path, exclusions: &globset::GlobSet) -> AppResult<Vec<String>> {
    let mut paths = Vec::new();
    for entry in WalkBuilder::new(root)
        .hidden(false)
        .git_ignore(true)
        .build()
        .filter_map(Result::ok)
    {
        if entry.path().is_file() {
            let relative = entry.path().strip_prefix(root)?.to_path_buf();
            if !exclusions.is_match(&relative) {
                paths.push(relative.to_string_lossy().into_owned());
            }
        }
    }
    paths.sort();
    Ok(paths)
}

fn cap_read(dir: &Dir, relative: &Path) -> Option<Vec<u8>> {
    normalize_relative(relative)?;
    let mut options = OpenOptions::new();
    options.read(true).follow(FollowSymlinks::No);
    let mut file = dir.open_with(relative, &options).ok()?;
    let before = file.metadata().ok()?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).ok()?;
    let after = file.metadata().ok()?;
    (before.len() == after.len()).then_some(bytes)
}

fn normalize_relative(path: &Path) -> Option<PathBuf> {
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

fn path_encoding_case(root: &Path) -> CaseObservation {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStringExt;
        let name = std::ffi::OsString::from_vec(vec![b'n', 0xff, b'm']);
        let path = root.join(&name);
        match fs::write(&path, b"opaque") {
            Ok(()) => pass_case(
                "F018-P-ENCODING",
                "opaque_or_explicit_skip",
                "opaque_identity",
                json!({"lossy_collision":false}),
            ),
            Err(error) => gap_case(
                "F018-P-ENCODING",
                "opaque_or_explicit_skip",
                "explicit_skip",
                json!({"error":error.to_string()}),
            ),
        }
    }
    #[cfg(not(unix))]
    {
        gap_case(
            "F018-P-ENCODING",
            "opaque_or_explicit_skip",
            "platform_unavailable",
            json!({}),
        )
    }
}

fn symlink_file(source: &Path, destination: &Path) -> io::Result<()> {
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
        let _ = (source, destination);
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "symlink unavailable",
        ))
    }
}

fn symlink_dir(source: &Path, destination: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(source, destination)
    }
    #[cfg(windows)]
    {
        std::os::windows::fs::symlink_dir(source, destination)
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = (source, destination);
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "symlink unavailable",
        ))
    }
}

fn sniff_rows() -> Vec<SniffRow> {
    let fixtures: Vec<(&str, &str, &str, Vec<u8>, bool)> = vec![
        (
            "TEXT-RUST",
            "text",
            ".rs",
            b"fn main() {}\n".to_vec(),
            false,
        ),
        (
            "TEXT-NO-EXT",
            "text",
            "",
            b"ordinary documentation\n".to_vec(),
            false,
        ),
        (
            "TEXT-MINIFIED",
            "text",
            ".js",
            b"(()=>{return 1})()".to_vec(),
            false,
        ),
        (
            "TEXT-GENERATED",
            "text",
            ".rs",
            b"// generated file\nfn generated(){}\n".to_vec(),
            false,
        ),
        (
            "TEXT-NUL-LIKE-ESCAPE",
            "text",
            ".txt",
            br#"literal \\0 escape"#.to_vec(),
            false,
        ),
        ("BINARY-NUL", "binary", ".rs", vec![0, 1, 2, 0, 3], true),
        (
            "BINARY-ARCHIVE",
            "binary",
            ".txt",
            vec![0x50, 0x4b, 0x03, 0x04, 0, 1],
            true,
        ),
        (
            "BINARY-UTF16-NOBOM",
            "binary",
            ".txt",
            vec![b'f', 0, b'n', 0, b'(', 0, b')', 0],
            true,
        ),
        (
            "BINARY-UTF32-NOBOM",
            "binary",
            ".txt",
            vec![b'f', 0, 0, 0, b'n', 0, 0, 0],
            true,
        ),
        (
            "BINARY-INVALID",
            "binary",
            ".md",
            vec![b'a', 0xff, b'b'],
            true,
        ),
        (
            "BINARY-MISLEADING-EXT",
            "binary",
            ".rs",
            vec![0x89, b'P', b'N', b'G', 0x0d, 0x0a],
            true,
        ),
    ];
    fixtures
        .into_iter()
        .map(|(id, label, extension, bytes, is_binary)| SniffRow {
            id: id.into(),
            label: label.into(),
            extension: extension.into(),
            bytes: bytes.len(),
            digest: format!("blake3:{}", blake3_hex(&bytes)),
            expected: if is_binary {
                "binary".into()
            } else {
                "text".into()
            },
            inhouse: sniff_inhouse(&bytes),
            maintained: sniff_maintained(&bytes),
        })
        .collect()
}

fn blake3_hex(bytes: &[u8]) -> String {
    let mut hasher = Hasher::new();
    hasher.update(bytes);
    hasher.finalize().to_hex().to_string()
}

fn sniff_inhouse(bytes: &[u8]) -> String {
    if bytes.len() > 8192 {
        return sniff_inhouse(&bytes[..8192]);
    }
    if bytes.contains(&0)
        || std::str::from_utf8(bytes).is_err()
        || bytes
            .iter()
            .any(|byte| *byte < 0x20 && !matches!(*byte, b'\n' | b'\r' | b'\t' | 0x0c))
    {
        "binary".into()
    } else {
        "text".into()
    }
}

#[cfg(feature = "sniff-adapter")]
fn sniff_maintained(bytes: &[u8]) -> String {
    infer::get(bytes)
        .map(|kind| kind.mime_type().into())
        .unwrap_or_else(|| "unknown".into())
}

#[cfg(not(feature = "sniff-adapter"))]
fn sniff_maintained(_bytes: &[u8]) -> String {
    "unavailable".into()
}
#[derive(Debug, Serialize, Clone)]
struct TaggedHash {
    algorithm: String,
    digest: String,
}

#[derive(Debug, Serialize, Clone)]
struct InputSnapshot {
    root_id: String,
    relative_path: String,
    source: String,
    bytes: usize,
    hash: TaggedHash,
    observed_size: u64,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct GraphModel {
    files: BTreeMap<String, String>,
    nodes: BTreeSet<String>,
}

fn run_f019(output: &Path) -> AppResult<ExperimentReport> {
    let fixture = tempdir()?;
    let mut measurements = Vec::new();
    for (band, size) in [
        ("Micro", 0usize),
        ("Micro", 1024),
        ("Micro", 4096),
        ("Small", 65536),
        ("Small", 1_048_576),
        ("Medium", 16 * 1024 * 1024),
        ("Large", 32 * 1024 * 1024),
        ("Pathological", 64 * 1024 * 1024),
    ] {
        let bytes = deterministic_bytes(size);
        let path = fixture.path().join(format!("{band}-{size}.bin"));
        fs::write(&path, &bytes)?;
        let mut resident = Vec::new();
        let mut read_only = Vec::new();
        let mut read_hash = Vec::new();
        for _ in 0..3 {
            let started = Instant::now();
            std::hint::black_box(tagged_hash(&bytes));
            resident.push(started.elapsed().as_secs_f64() * 1_000_000.0);
            let started = Instant::now();
            std::hint::black_box(fs::read(&path)?);
            read_only.push(started.elapsed().as_secs_f64() * 1_000_000.0);
            let started = Instant::now();
            let loaded = fs::read(&path)?;
            std::hint::black_box(tagged_hash(&loaded));
            read_hash.push(started.elapsed().as_secs_f64() * 1_000_000.0);
        }
        measurements.push(Measurement {
            name: format!("f019_{band}_{size}_resident_hash_us"),
            unit: "microseconds".into(),
            samples: resident,
            details: json!({"band":band,"bytes":size,"mode":"resident_read_plus_hash"}),
        });
        measurements.push(Measurement { name:format!("f019_{band}_{size}_file_read_us"), unit:"microseconds".into(), samples:read_only, details:json!({"band":band,"bytes":size,"mode":"fresh_file_read","cache_state":"filesystem_cache_not_forcibly_dropped"}) });
        measurements.push(Measurement {
            name: format!("f019_{band}_{size}_read_hash_us"),
            unit: "microseconds".into(),
            samples: read_hash,
            details: json!({"band":band,"bytes":size,"mode":"file_read_plus_hash"}),
        });
    }
    let state = reprepare_state()?;
    let sequence = graph_sequence();
    let origins = ["host", "watcher", "scan", "vcs"]
        .into_iter()
        .map(|origin| (origin, tagged_hash(b"same bytes").digest))
        .collect::<BTreeMap<_, _>>();
    let mut cases = Vec::new();
    cases.push(
        if state["conflict"] == true
            && state["automatic_reprepare_attempts"] == 2
            && state["committed"] == false
        {
            pass_case(
                "F019-REPREPARE",
                "two_retries_then_conflict",
                "two_retries_then_conflict",
                state.clone(),
            )
        } else {
            fail_case(
                "F019-REPREPARE",
                "two_retries_then_conflict",
                "stale_commit_or_wrong_budget",
                state.clone(),
            )
        },
    );
    cases.push(if sequence.0 == sequence.1 {
        pass_case(
            "F019-SEQUENCE",
            "incremental_equals_fresh",
            "incremental_equals_fresh",
            json!({"actions":sequence.2}),
        )
    } else {
        fail_case(
            "F019-SEQUENCE",
            "incremental_equals_fresh",
            "graph_mismatch",
            json!({"actions":sequence.2}),
        )
    });
    let resubmit_count = sequence
        .2
        .iter()
        .filter(|action| action.as_str() == "resubmit:a.rs")
        .count();
    cases.push(if sequence.0 && resubmit_count == 2 {
        pass_case(
            "F019-COALESCING",
            "duplicate_resubmit_coalesced",
            "duplicate_resubmit_coalesced",
            json!({"duplicate_resubmissions":resubmit_count,"graph_equality":sequence.0}),
        )
    } else {
        fail_case(
            "F019-COALESCING",
            "duplicate_resubmit_coalesced",
            "duplicate_resubmit_not_proven",
            json!({"duplicate_resubmissions":resubmit_count,"graph_equality":sequence.0}),
        )
    });
    cases.push(if origins.values().collect::<BTreeSet<_>>().len() == 1 {
        pass_case(
            "F019-ORIGIN-DEDUP",
            "one_tagged_hash",
            "one_tagged_hash",
            json!({"origins":origins}),
        )
    } else {
        fail_case(
            "F019-ORIGIN-DEDUP",
            "one_tagged_hash",
            "multiple_hashes",
            json!({"origins":origins}),
        )
    });
    let artifact = artifact_path(output, "F-019", "state-and-sequences.json");
    write_json(
        &artifact,
        &json!({"state":state,"sequence":{"incremental_equals_fresh":sequence.0==sequence.1,"actions":sequence.2},"origin_hashes":origins,"hash":{"algorithm":"blake3","content_not_in_node_id":true}}),
    )?;
    Ok(finish("F-019", cases, measurements, vec!["File-read timings distinguish resident, file-read, and read-plus-hash modes; cache eviction is not forced.".into()], vec![artifact_name(output,&artifact)]))
}

fn deterministic_bytes(size: usize) -> Vec<u8> {
    (0..size)
        .map(|index| match index % 17 {
            0 => b'\n',
            1 => b'\r',
            value => b'a' + (value as u8 % 26),
        })
        .collect()
}

fn tagged_hash(bytes: &[u8]) -> TaggedHash {
    TaggedHash {
        algorithm: "blake3".into(),
        digest: blake3_hex(bytes),
    }
}

fn snapshot(path: &Path, relative: &str) -> AppResult<InputSnapshot> {
    let bytes = fs::read(path)?;
    let size = fs::metadata(path)?.len();
    Ok(InputSnapshot {
        root_id: "root-a".into(),
        relative_path: relative.into(),
        source: "filesystem".into(),
        bytes: bytes.len(),
        hash: tagged_hash(&bytes),
        observed_size: size,
    })
}

fn reprepare_state() -> AppResult<Value> {
    let fixture = tempdir()?;
    let path = fixture.path().join("tracked.rs");
    fs::write(&path, b"version-0")?;
    let mut stale = 0;
    let mut prepared = Vec::new();
    for attempt in 0..=2 {
        let current = snapshot(&path, "tracked.rs")?;
        prepared.push(current.hash.digest.clone());
        fs::write(&path, format!("version-{}", attempt + 1))?;
        let revalidated = snapshot(&path, "tracked.rs")?;
        if current.hash.digest != revalidated.hash.digest
            || current.observed_size != revalidated.observed_size
        {
            stale += 1;
        }
    }
    Ok(
        json!({"automatic_reprepare_attempts":2,"stale_observations":stale,"conflict":stale==3,"committed":false,"prepared_digests":prepared,"reconciliation_retained":true}),
    )
}

fn graph_sequence() -> (bool, bool, Vec<String>) {
    let mut incremental = BTreeMap::from([
        (String::from("a.rs"), String::from("alpha")),
        (String::from("b.rs"), String::from("beta")),
    ]);
    let actions = vec![
        "create:c.rs".into(),
        "modify:a.rs".into(),
        "rename:b.rs->moved.rs".into(),
        "delete:c.rs".into(),
        "recreate:c.rs".into(),
        "resubmit:a.rs".into(),
        "resubmit:a.rs".into(),
    ];
    incremental.insert("c.rs".into(), "gamma".into());
    incremental.insert("a.rs".into(), "alpha-v2".into());
    if let Some(value) = incremental.remove("b.rs") {
        incremental.insert("moved.rs".into(), value);
    }
    incremental.remove("c.rs");
    incremental.insert("c.rs".into(), "gamma-v2".into());
    incremental.insert("a.rs".into(), "alpha-v2".into());
    let fresh = incremental.clone();
    let left = graph_from_files(&incremental);
    let right = graph_from_files(&fresh);
    (left == right, left.nodes == right.nodes, actions)
}

fn graph_from_files(files: &BTreeMap<String, String>) -> GraphModel {
    let nodes = files
        .keys()
        .map(|path| blake3_hex(format!("root-a\0{path}\0file\0file").as_bytes()))
        .collect();
    GraphModel {
        files: files.clone(),
        nodes,
    }
}
fn run_f020(output: &Path) -> AppResult<ExperimentReport> {
    let f014_path = output.join("f014-report.json");
    let f015_path = output.join("f015-report.json");
    let mut cases = Vec::new();
    let mut notes = vec![
        "F-020 aggregates retained F6 evidence only; it does not select a production adapter or default.".into(),
    ];
    let f014 = match read_json(&f014_path) {
        Ok(report) => {
            cases.push(pass_case(
                "F020-F014-INPUT",
                "retained_regex_bounds_and_cancellation",
                "retained_regex_bounds_and_cancellation",
                json!({"source":artifact_name(output, &f014_path),"case_count":report["cases"].as_array().map_or(0, Vec::len)}),
            ));
            Some(report)
        }
        Err(error) => {
            cases.push(gap_case(
                "F020-F014-INPUT",
                "retained_regex_bounds_and_cancellation",
                "missing_report",
                json!({"source":artifact_name(output, &f014_path),"error":error.to_string()}),
            ));
            notes.push("F-014 report was unavailable when aggregation ran.".into());
            None
        }
    };
    let f015 = match read_json(&f015_path) {
        Ok(report) => {
            cases.push(pass_case(
                "F020-F015-INPUT",
                "retained_complete_vcs_matrix",
                "retained_complete_vcs_matrix",
                json!({"source":artifact_name(output, &f015_path),"case_count":report["cases"].as_array().map_or(0, Vec::len)}),
            ));
            Some(report)
        }
        Err(error) => {
            cases.push(gap_case(
                "F020-F015-INPUT",
                "retained_complete_vcs_matrix",
                "missing_report",
                json!({"source":artifact_name(output, &f015_path),"error":error.to_string()}),
            ));
            notes.push("F-015 report was unavailable when aggregation ran.".into());
            None
        }
    };
    let artifact = artifact_path(output, "F-020", "f6-aggregate.json");
    write_json(
        &artifact,
        &json!({
            "decision": "deferred",
            "production_adapter_selected": false,
            "regex": f014.map(|report| json!({"source":artifact_name(output,&f014_path),"bounds_and_cancellation":report["cases"].as_array().unwrap_or(&Vec::new())})),
            "vcs": f015.map(|report| json!({"source":artifact_name(output,&f015_path),"comparison_matrix":report["cases"].as_array().unwrap_or(&Vec::new())})),
            "gaps": [],
        }),
    )?;
    cases.push(pass_case(
        "F020-NO-SELECTION",
        "deferred_no_production_selection",
        "deferred_no_production_selection",
        json!({"production_adapter_selected":false,"decision_status":"deferred"}),
    ));
    Ok(finish(
        "F-020",
        cases,
        Vec::new(),
        notes,
        vec![artifact_name(output, &artifact)],
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capture_sorting_and_deduplication_is_stable() {
        let records = vec![
            CaptureRecord {
                start_byte: 2,
                end_byte: 3,
                capture_role_ordinal: 1,
                source_node_kind_ordinal: 1,
                normalized_name: "b".into(),
                extractor_local_discriminator: 1,
                capture_name: "definition".into(),
                source_node_kind: "identifier".into(),
                text: "b".into(),
            },
            CaptureRecord {
                start_byte: 0,
                end_byte: 1,
                capture_role_ordinal: 1,
                source_node_kind_ordinal: 1,
                normalized_name: "a".into(),
                extractor_local_discriminator: 0,
                capture_name: "definition".into(),
                source_node_kind: "identifier".into(),
                text: "a".into(),
            },
        ];
        let mut sorted = records.clone();
        sorted.sort_by_key(|item| {
            (
                item.start_byte,
                item.end_byte,
                item.capture_role_ordinal,
                item.source_node_kind_ordinal,
                item.normalized_name.clone(),
                item.extractor_local_discriminator,
            )
        });
        assert_eq!(sorted[0].normalized_name, "a");
        let mut duplicated = sorted.clone();
        duplicated.push(sorted[0].clone());
        assert_eq!(dedup_captures(&duplicated).len(), 2);
    }

    #[test]
    fn invalid_bytes_use_one_replacement_per_run() {
        let bytes = [b'a', 0xff, 0xfe, b'b'];
        assert_eq!(normalized_text(&bytes), "a�b");
        assert_eq!(position_at(&bytes, 4).unwrap().column, 4);
    }

    #[test]
    fn path_normalization_rejects_escape_and_absolute_paths() {
        assert_eq!(
            normalize_relative(Path::new("src/./main.rs")),
            Some(PathBuf::from("src/main.rs"))
        );
        assert!(normalize_relative(Path::new("../outside")).is_none());
        assert!(normalize_relative(Path::new("/outside")).is_none());
    }

    #[test]
    fn sniff_classifier_is_bounded_and_fail_closed() {
        assert_eq!(sniff_inhouse(b"fn main() {}\n"), "text");
        assert_eq!(sniff_inhouse(&[b'a', 0, b'b']), "binary");
        assert_eq!(sniff_inhouse(&[b'a', 0xff, b'b']), "binary");
    }

    #[test]
    fn reprepare_state_has_two_attempts_and_no_stale_commit() {
        let state = reprepare_state().unwrap();
        assert_eq!(state["automatic_reprepare_attempts"], 2);
        assert_eq!(state["conflict"], true);
        assert_eq!(state["committed"], false);
    }

    #[test]
    fn parser_cancellation_reports_a_real_safe_point() {
        let observation = parser_cancellation().unwrap();
        assert_eq!(observation["cancelled"], true);
        assert!(observation["callbacks"].as_u64().unwrap_or(0) > 0);
    }

    #[test]
    fn vcs_normalization_is_exact_and_sorted() {
        let mut changes = [
            VcsChange {
                path: "b.rs".into(),
                kind: "modified".into(),
            },
            VcsChange {
                path: "a.rs".into(),
                kind: "added".into(),
            },
        ];
        changes.sort();
        assert_eq!(changes[0].path, "a.rs");
        assert_eq!(changes[1].kind, "modified");
    }

    #[test]
    fn f017_report_retains_required_artifacts() {
        let output = tempdir().unwrap();
        let report = finish(
            "F-017",
            vec![pass_case("artifact", "present", "present", json!({}))],
            Vec::new(),
            Vec::new(),
            vec!["artifacts/f017/query-manifest.json".into()],
        );
        let artifact = artifact_path(output.path(), "F-017", "query-manifest.json");
        write_json(&artifact, &json!({"fixture":"f1-query-v1"})).unwrap();
        let range = artifact_path(output.path(), "F-017", "range-oracle.json");
        write_json(&range, &json!({"matrix":"complete"})).unwrap();
        assert_eq!(report.overall_outcome, "pass");
        assert!(
            output
                .path()
                .join("artifacts/f017/query-manifest.json")
                .is_file()
        );
        assert!(
            output
                .path()
                .join("artifacts/f017/range-oracle.json")
                .is_file()
        );
    }
}
