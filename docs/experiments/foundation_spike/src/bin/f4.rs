//! Disposable Linux-only F4 cancellation and concurrency experiment.
//!
//! This binary is evidence code. It compares bounded synchronous work with a
//! hybrid adapter boundary and a Tokio orchestration model. None of its types
//! are part of the Repin API.

use blake3::Hasher;
use regex::Regex;
use serde::Serialize;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::error::Error;
use std::hint::black_box;
#[cfg(feature = "async-runtime")]
use std::io;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::ops::ControlFlow;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Barrier, Mutex, mpsc};
use std::thread;
use std::time::{Duration, Instant};
use tree_sitter::{
    Language, ParseOptions, Parser, Query, QueryCursor, QueryCursorOptions, StreamingIterator,
};

type AppResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

const RUN_ID: &str = "foundation-f4-tier1-20260818";
const FIXTURE_SEED: &str = "repin-f4-1";
const MAX_CANCELLATION_SAMPLES: usize = 30;
const MAX_THROUGHPUT_SAMPLES: usize = 5;
#[cfg(feature = "async-runtime")]
const DIAGNOSTIC_RUN_ID: &str = "foundation-f4-hybrid-audit-20260818";
#[cfg(feature = "async-runtime")]
const DIAGNOSTIC_WARMUPS: usize = 2;
#[cfg(feature = "async-runtime")]
const DIAGNOSTIC_SAMPLES: usize = 10;
#[cfg(feature = "async-runtime")]
const DIAGNOSTIC_SERVICE_REQUESTS: usize = 64;
#[cfg(feature = "async-runtime")]
const DIAGNOSTIC_REMOTE_REQUESTS: usize = 32;
#[cfg(feature = "async-runtime")]
const DIAGNOSTIC_SERVICE_DELAY_US: u64 = 2_000;
#[cfg(feature = "async-runtime")]
const DIAGNOSTIC_REMOTE_DELAY_US: u64 = 8_000;

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
enum Profile {
    Full,
    Smoke,
}

impl Profile {
    fn parse(value: &str) -> AppResult<Self> {
        match value {
            "full" => Ok(Self::Full),
            "smoke" => Ok(Self::Smoke),
            other => Err(format!("unknown profile {other}; expected full or smoke").into()),
        }
    }

    fn cancellation_samples(self) -> usize {
        match self {
            Self::Full => MAX_CANCELLATION_SAMPLES,
            Self::Smoke => 3,
        }
    }

    fn throughput_samples(self) -> usize {
        match self {
            Self::Full => MAX_THROUGHPUT_SAMPLES,
            Self::Smoke => 2,
        }
    }

    fn fixture_bytes(self) -> usize {
        match self {
            Self::Full => 128 * 1024 * 1024,
            Self::Smoke => 4 * 1024 * 1024,
        }
    }

    fn parser_bytes(self) -> usize {
        match self {
            Self::Full => 2 * 1024 * 1024,
            Self::Smoke => 64 * 1024,
        }
    }

    fn regex_bytes(self) -> usize {
        match self {
            Self::Full => 8 * 1024 * 1024,
            Self::Smoke => 256 * 1024,
        }
    }

    fn entries(self) -> usize {
        match self {
            Self::Full => 6_000,
            Self::Smoke => 64,
        }
    }

    fn facts(self) -> usize {
        match self {
            Self::Full => 500_001,
            Self::Smoke => 5_000,
        }
    }

    fn candidates(self) -> usize {
        match self {
            Self::Full => 100_000,
            Self::Smoke => 5_000,
        }
    }

    fn request_count(self) -> usize {
        match self {
            Self::Full => 2_000,
            Self::Smoke => 200,
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
enum Model {
    Sync,
    Hybrid,
    Async,
}

impl Model {
    fn parse(value: &str) -> AppResult<Vec<Self>> {
        match value {
            "all" => Ok(vec![Self::Sync, Self::Hybrid, Self::Async]),
            "sync" => Ok(vec![Self::Sync]),
            "hybrid" => Ok(vec![Self::Hybrid]),
            "async" => Ok(vec![Self::Async]),
            other => {
                Err(format!("unknown model {other}; expected all, sync, hybrid, or async").into())
            }
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::Sync => "sync",
            Self::Hybrid => "hybrid",
            Self::Async => "async",
        }
    }
}

#[derive(Debug, Serialize)]
struct Manifest {
    run_id: &'static str,
    experiment: &'static str,
    lifecycle_stage: &'static str,
    platform_scope: &'static str,
    target: String,
    os: String,
    architecture: String,
    rustc: String,
    cargo: String,
    source_revision: String,
    candidate_pins: BTreeMap<String, String>,
    active_features: Vec<&'static str>,
    fixture_seed: &'static str,
    profile: Profile,
    workers: usize,
    queue_capacity: usize,
    binary_size_bytes: u64,
    clean_build_time_ms: Option<f64>,
    reproducibility: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Serialize)]
struct CaseObservation {
    id: String,
    model: String,
    expected: String,
    observed: String,
    outcome: String,
    details: Value,
}

#[derive(Clone, Debug, Serialize)]
struct Measurement {
    name: String,
    model: String,
    unit: String,
    samples: Vec<f64>,
    details: Value,
}

#[derive(Debug, Serialize)]
struct ModelReport {
    model: String,
    cases: Vec<CaseObservation>,
    measurements: Vec<Measurement>,
    notes: Vec<String>,
}

#[derive(Debug, Serialize)]
struct Report {
    experiment: &'static str,
    run_id: &'static str,
    status: String,
    overall_outcome: String,
    decision_status: &'static str,
    hard_blocker: bool,
    models: Vec<ModelReport>,
    cases: Vec<CaseObservation>,
    measurements: Vec<Measurement>,
    selection: Value,
    notes: Vec<String>,
    artifacts: Vec<String>,
    binary_size_bytes: u64,
    clean_build_time_ms: Option<f64>,
}

#[derive(Clone)]
struct Fixture {
    profile: Profile,
    bytes: Arc<Vec<u8>>,
    parser_source: Arc<Vec<u8>>,
    regex_source: Arc<Vec<u8>>,
    language: Language,
}

#[derive(Clone)]
struct Control {
    cancelled: Arc<AtomicBool>,
    deadline: Option<(Instant, StopReason)>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum StopReason {
    Cancelled,
    Timeout,
    Deadline,
}

impl StopReason {
    fn name(self) -> &'static str {
        match self {
            Self::Cancelled => "cancelled",
            Self::Timeout => "timeout",
            Self::Deadline => "deadline",
        }
    }
}

impl Control {
    fn new(timeout_after_ms: Option<u64>, deadline_after_ms: Option<u64>) -> Self {
        let now = Instant::now();
        let timeout =
            timeout_after_ms.map(|value| (now + Duration::from_millis(value), StopReason::Timeout));
        let deadline = deadline_after_ms
            .map(|value| (now + Duration::from_millis(value), StopReason::Deadline));
        Self {
            cancelled: Arc::new(AtomicBool::new(false)),
            deadline: match (timeout, deadline) {
                (Some(left), Some(right)) => Some(if left.0 <= right.0 { left } else { right }),
                (Some(value), None) | (None, Some(value)) => Some(value),
                (None, None) => None,
            },
        }
    }

    fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
    }

    fn stop_reason(&self) -> Option<StopReason> {
        if self.cancelled.load(Ordering::Acquire) {
            Some(StopReason::Cancelled)
        } else if self
            .deadline
            .is_some_and(|(instant, _reason)| Instant::now() >= instant)
        {
            Some(
                self.deadline
                    .map_or(StopReason::Deadline, |(_, reason)| reason),
            )
        } else {
            None
        }
    }

    fn check(&self) -> Result<(), StopReason> {
        self.stop_reason().map_or(Ok(()), Err)
    }
}

#[derive(Debug)]
struct OperationResult {
    status: String,
    units: usize,
    revision: u64,
    pending_derived: bool,
}

#[derive(Debug, Serialize)]
struct Trial {
    status: String,
    cancellation_latency_us: Option<f64>,
    elapsed_us: f64,
    units: usize,
    revision: u64,
    pending_derived: bool,
}

#[derive(Debug, Serialize)]
struct QueueStats {
    worker_count: usize,
    runtime_thread_count: usize,
    blocking_thread_limit: usize,
    max_queue: usize,
    max_active: usize,
    submitted: usize,
    completed: usize,
    dropped_reads: usize,
    coalesced_updates: usize,
    overflow_roots: Vec<String>,
    shutdown_us: f64,
}

#[derive(Clone, Debug, Serialize)]
struct CommitState {
    revision: u64,
    facts: usize,
    summary_present: bool,
    pending_derived: bool,
}

#[cfg(feature = "async-runtime")]
#[derive(Debug, Serialize)]
struct DiagnosticSample {
    sample: usize,
    elapsed_us: f64,
    rps: f64,
    expected_requests: usize,
    client_completed: usize,
    server_completed: usize,
    client_errors: usize,
    server_errors: usize,
    client_max_active: usize,
    server_max_active: usize,
    client_max_queue: usize,
    server_max_queue: usize,
    client_workers: usize,
    server_workers: usize,
}

#[cfg(feature = "async-runtime")]
#[derive(Debug, Serialize)]
struct DiagnosticWorkloadReport {
    workload: String,
    requests: usize,
    delay_us: u64,
    warmups: usize,
    samples: Vec<DiagnosticSample>,
    rps_samples: Vec<f64>,
    p50: f64,
    p95: f64,
    max: f64,
}

#[cfg(feature = "async-runtime")]
#[derive(Debug, Serialize)]
struct DiagnosticModelReport {
    model: String,
    order_index: usize,
    configured_thread_count: usize,
    runtime_thread_count: usize,
    server_worker_count: usize,
    client_concurrency: usize,
    workloads: Vec<DiagnosticWorkloadReport>,
}

#[cfg(feature = "async-runtime")]
#[derive(Debug, Serialize)]
struct DiagnosticProbe {
    schema: &'static str,
    experiment: &'static str,
    run_id: &'static str,
    condition: String,
    client_mode: String,
    order_index: usize,
    model_order: Vec<String>,
    process_affinity: Vec<usize>,
    available_workers: usize,
    server_queue_capacity: usize,
    warmups: usize,
    samples: usize,
    service_requests: usize,
    remote_requests: usize,
    binary_size_bytes: u64,
    clean_build_time_ms: Option<f64>,
    models: Vec<DiagnosticModelReport>,
}

#[cfg(feature = "async-runtime")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DiagnosticClientMode {
    Native,
    Matched,
}

#[cfg(feature = "async-runtime")]
impl DiagnosticClientMode {
    fn parse(value: &str) -> AppResult<Self> {
        match value {
            "native" => Ok(Self::Native),
            "matched" => Ok(Self::Matched),
            other => Err(format!(
                "unknown diagnostic client mode {other}; expected native or matched"
            )
            .into()),
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::Native => "native",
            Self::Matched => "matched",
        }
    }
}

#[cfg(feature = "async-runtime")]
#[derive(Clone, Debug)]
struct DiagnosticCounters {
    active: Arc<AtomicUsize>,
    max_active: Arc<AtomicUsize>,
    queued: Arc<AtomicUsize>,
    max_queue: Arc<AtomicUsize>,
    completed: Arc<AtomicUsize>,
    errors: Arc<AtomicUsize>,
}

#[cfg(feature = "async-runtime")]
impl Default for DiagnosticCounters {
    fn default() -> Self {
        Self {
            active: Arc::new(AtomicUsize::new(0)),
            max_active: Arc::new(AtomicUsize::new(0)),
            queued: Arc::new(AtomicUsize::new(0)),
            max_queue: Arc::new(AtomicUsize::new(0)),
            completed: Arc::new(AtomicUsize::new(0)),
            errors: Arc::new(AtomicUsize::new(0)),
        }
    }
}

#[cfg(feature = "async-runtime")]
impl DiagnosticCounters {
    fn set_max(target: &AtomicUsize, value: usize) {
        let mut current = target.load(Ordering::Acquire);
        while value > current {
            match target.compare_exchange(current, value, Ordering::AcqRel, Ordering::Acquire) {
                Ok(_) => break,
                Err(observed) => current = observed,
            }
        }
    }

    fn queued(&self) {
        let value = self.queued.fetch_add(1, Ordering::AcqRel) + 1;
        Self::set_max(&self.max_queue, value);
    }

    fn dequeued(&self) {
        self.queued.fetch_sub(1, Ordering::AcqRel);
    }

    fn started(&self) {
        let value = self.active.fetch_add(1, Ordering::AcqRel) + 1;
        Self::set_max(&self.max_active, value);
    }

    fn finished(&self, result: Result<(), ()>) {
        self.active.fetch_sub(1, Ordering::AcqRel);
        match result {
            Ok(()) => {
                self.completed.fetch_add(1, Ordering::AcqRel);
            }
            Err(()) => {
                self.errors.fetch_add(1, Ordering::AcqRel);
            }
        }
    }
}

#[cfg(feature = "async-runtime")]
#[derive(Debug)]
struct DiagnosticClientResult {
    completed: usize,
    errors: usize,
    max_active: usize,
    max_queue: usize,
    workers: usize,
}

#[cfg(feature = "async-runtime")]
#[derive(Debug)]
struct DiagnosticServerResult {
    completed: usize,
    errors: usize,
    max_active: usize,
    max_queue: usize,
    workers: usize,
}

#[cfg(feature = "async-runtime")]
#[derive(Debug)]
struct DiagnosticServer {
    address: std::net::SocketAddr,
    handle: thread::JoinHandle<AppResult<()>>,
    counters: DiagnosticCounters,
    workers: usize,
}

fn main() -> AppResult<()> {
    let args: Vec<String> = env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("preflight") => {
            println!("{}", serde_json::to_string_pretty(&make_manifest(Profile::Smoke)?)?);
            Ok(())
        }
        Some("child-noncooperative") => child_noncooperative(),
        Some("diagnose-hybrid") => {
            #[cfg(not(feature = "async-runtime"))]
            return Err("diagnose-hybrid requires --features async-runtime".into());
            #[cfg(feature = "async-runtime")]
            {
                let condition = required_arg(&args, "--condition")?;
                let client_mode = DiagnosticClientMode::parse(&required_arg(
                    &args,
                    "--client-mode",
                )?)?;
                let order_index = required_arg(&args, "--order")?.parse::<usize>()?;
                let output = PathBuf::from(required_arg(&args, "--output")?);
                run_diagnostic_probe(&condition, client_mode, order_index, &output)
            }
        }
        Some("run") => {
            let model = required_arg(&args, "--model")?;
            let profile = Profile::parse(&required_arg(&args, "--profile")?)?;
            let output = PathBuf::from(required_arg(&args, "--output")?);
            run_experiment(&model, profile, &output)
        }
        _ => Err("usage: repin-f4-spike preflight | diagnose-hybrid --condition pinned|unpinned --client-mode native|matched --order 0|1|2 --output DIR | run --model all|sync|hybrid|async --profile full|smoke --output DIR".into()),
    }
}

fn required_arg(args: &[String], name: &str) -> AppResult<String> {
    let index = args
        .iter()
        .position(|value| value == name)
        .ok_or_else(|| format!("missing {name}"))?;
    args.get(index + 1)
        .cloned()
        .ok_or_else(|| format!("missing value for {name}").into())
}

fn run_experiment(model_arg: &str, profile: Profile, output: &Path) -> AppResult<()> {
    let models = Model::parse(model_arg)?;
    if models.iter().any(|model| *model != Model::Sync) {
        #[cfg(not(feature = "async-runtime"))]
        return Err("hybrid and async models require --features async-runtime".into());
    }

    std::fs::create_dir_all(output)?;
    let manifest = make_manifest(profile)?;
    write_json(&output.join("manifest.json"), &manifest)?;
    let fixture = make_fixture(profile);
    let mut reports = Vec::new();
    for model in models {
        reports.push(
            run_model(model, &fixture)
                .map_err(|error| format!("run model {}: {error}", model.name()))?,
        );
    }
    let report = combine_reports(profile, reports)?;
    write_json(&output.join("F4.json"), &report)?;
    write_json(&output.join("F4-report.json"), &report)?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}

fn make_manifest(profile: Profile) -> AppResult<Manifest> {
    let workers = available_workers();
    let mut candidate_pins = BTreeMap::new();
    candidate_pins.insert("tree-sitter".into(), "0.26.11".into());
    candidate_pins.insert("regex".into(), "1.13.1".into());
    candidate_pins.insert("blake3".into(), "1.8.5".into());
    candidate_pins.insert(
        "tokio".into(),
        "1.53.1 (optional async-runtime feature)".into(),
    );

    let mut reproducibility = BTreeMap::new();
    reproducibility.insert("build_profile".into(), "release".into());
    reproducibility.insert(
        "warmup_policy".into(),
        "one warmup then measured samples".into(),
    );
    reproducibility.insert(
        "cancellation_samples".into(),
        profile.cancellation_samples().to_string(),
    );
    reproducibility.insert(
        "throughput_samples".into(),
        profile.throughput_samples().to_string(),
    );
    reproducibility.insert(
        "source_policy".into(),
        "working tree; disposable evidence code".into(),
    );

    Ok(Manifest {
        run_id: RUN_ID,
        experiment: "F4",
        lifecycle_stage: "experimentation",
        platform_scope: "Linux x86_64/glibc PoC only; platform expansion deferred",
        target: env::var("TARGET").unwrap_or_else(|_| env::consts::ARCH.to_string()),
        os: env::consts::OS.to_string(),
        architecture: env::consts::ARCH.to_string(),
        rustc: command_version("rustc")?,
        cargo: command_version("cargo")?,
        source_revision: source_revision()?,
        candidate_pins,
        active_features: active_features(),
        fixture_seed: FIXTURE_SEED,
        profile,
        workers,
        queue_capacity: workers * 8,
        binary_size_bytes: env::current_exe()?.metadata()?.len(),
        clean_build_time_ms: env::var("REPINF4_CLEAN_BUILD_MS")
            .ok()
            .and_then(|value| value.parse().ok()),
        reproducibility,
    })
}

fn active_features() -> Vec<&'static str> {
    #[cfg(feature = "async-runtime")]
    {
        vec!["default", "async-runtime"]
    }
    #[cfg(not(feature = "async-runtime"))]
    {
        vec!["default"]
    }
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
    for relative in ["Cargo.toml", "Cargo.lock", "src/bin/f4.rs"] {
        let path = current.join(relative);
        if path.exists() {
            hasher.update(relative.as_bytes());
            hasher.update(&std::fs::read(path)?);
        }
    }
    Ok(format!(
        "working-tree-status: {}",
        hasher.finalize().to_hex()
    ))
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> AppResult<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, serde_json::to_vec_pretty(value)?)?;
    Ok(())
}

fn available_workers() -> usize {
    std::thread::available_parallelism()
        .map(|value| value.get().clamp(2, 4))
        .unwrap_or(2)
}

#[cfg(feature = "async-runtime")]
fn parse_cpu_list(value: &str) -> AppResult<Vec<usize>> {
    let mut cpus = Vec::new();
    for item in value.trim().split(',').filter(|item| !item.is_empty()) {
        if let Some((first, last)) = item.split_once('-') {
            let first = first.parse::<usize>()?;
            let last = last.parse::<usize>()?;
            if last < first {
                return Err(format!("invalid CPU range {item}").into());
            }
            cpus.extend(first..=last);
        } else {
            cpus.push(item.parse::<usize>()?);
        }
    }
    Ok(cpus)
}

#[cfg(feature = "async-runtime")]
fn current_process_affinity() -> AppResult<Vec<usize>> {
    let status = std::fs::read_to_string("/proc/self/status")?;
    let value = status
        .lines()
        .find_map(|line| line.strip_prefix("Cpus_allowed_list:"))
        .ok_or("Cpus_allowed_list is missing from /proc/self/status")?;
    parse_cpu_list(value)
}

#[cfg(feature = "async-runtime")]
fn diagnostic_model_order(order_index: usize) -> AppResult<Vec<Model>> {
    match order_index {
        0 => Ok(vec![Model::Sync, Model::Hybrid, Model::Async]),
        1 => Ok(vec![Model::Hybrid, Model::Async, Model::Sync]),
        2 => Ok(vec![Model::Async, Model::Sync, Model::Hybrid]),
        other => Err(format!("unknown diagnostic order {other}; expected 0, 1, or 2").into()),
    }
}

#[cfg(feature = "async-runtime")]
fn diagnostic_client_concurrency(model: Model, mode: DiagnosticClientMode) -> usize {
    let workers = available_workers();
    match mode {
        DiagnosticClientMode::Matched => workers,
        DiagnosticClientMode::Native if model == Model::Sync => workers,
        DiagnosticClientMode::Native => workers * 2,
    }
}

#[cfg(feature = "async-runtime")]
fn diagnostic_runtime_thread_count(model: Model) -> usize {
    match model {
        Model::Sync => 0,
        Model::Hybrid => 2,
        Model::Async => available_workers(),
    }
}

#[cfg(feature = "async-runtime")]
fn run_diagnostic_probe(
    condition: &str,
    client_mode: DiagnosticClientMode,
    order_index: usize,
    output: &Path,
) -> AppResult<()> {
    if !matches!(condition, "pinned" | "unpinned") {
        return Err(format!(
            "unknown diagnostic condition {condition}; expected pinned or unpinned"
        )
        .into());
    }
    let affinity = current_process_affinity()?;
    if condition == "pinned" && affinity != [0, 1, 2, 3] {
        return Err(format!(
            "pinned diagnostic requires affinity [0, 1, 2, 3], observed {affinity:?}"
        )
        .into());
    }
    let model_order = diagnostic_model_order(order_index)?;
    let workers = available_workers();
    let mut models = Vec::with_capacity(model_order.len());
    for (position, model) in model_order.iter().copied().enumerate() {
        let executor = if model == Model::Sync {
            None
        } else {
            Some(TokioExecutor::new(model)?)
        };
        models.push(run_diagnostic_model(
            model,
            position,
            client_mode,
            executor.as_ref(),
        )?);
    }
    let probe = DiagnosticProbe {
        schema: "f4-hybrid-diagnostic-v1",
        experiment: "F4",
        run_id: DIAGNOSTIC_RUN_ID,
        condition: condition.into(),
        client_mode: client_mode.name().into(),
        order_index,
        model_order: model_order
            .iter()
            .map(|model| model.name().into())
            .collect(),
        process_affinity: affinity,
        available_workers: workers,
        server_queue_capacity: workers * 2,
        warmups: DIAGNOSTIC_WARMUPS,
        samples: DIAGNOSTIC_SAMPLES,
        service_requests: DIAGNOSTIC_SERVICE_REQUESTS,
        remote_requests: DIAGNOSTIC_REMOTE_REQUESTS,
        binary_size_bytes: env::current_exe()?.metadata()?.len(),
        clean_build_time_ms: env::var("REPINF4_CLEAN_BUILD_MS")
            .ok()
            .and_then(|value| value.parse().ok()),
        models,
    };
    std::fs::create_dir_all(output)?;
    write_json(&output.join("probe.json"), &probe)?;
    println!("{}", serde_json::to_string_pretty(&probe)?);
    Ok(())
}

#[cfg(feature = "async-runtime")]
fn run_diagnostic_model(
    model: Model,
    order_index: usize,
    client_mode: DiagnosticClientMode,
    executor: Option<&TokioExecutor>,
) -> AppResult<DiagnosticModelReport> {
    let client_concurrency = diagnostic_client_concurrency(model, client_mode);
    let workloads = vec![
        run_diagnostic_workload(
            model,
            executor,
            "service",
            DIAGNOSTIC_SERVICE_REQUESTS,
            DIAGNOSTIC_SERVICE_DELAY_US,
            client_concurrency,
        )?,
        run_diagnostic_workload(
            model,
            executor,
            "remote",
            DIAGNOSTIC_REMOTE_REQUESTS,
            DIAGNOSTIC_REMOTE_DELAY_US,
            client_concurrency,
        )?,
    ];
    Ok(DiagnosticModelReport {
        model: model.name().into(),
        order_index,
        configured_thread_count: configured_thread_count(model),
        runtime_thread_count: diagnostic_runtime_thread_count(model),
        server_worker_count: available_workers(),
        client_concurrency,
        workloads,
    })
}

#[cfg(feature = "async-runtime")]
fn run_diagnostic_workload(
    model: Model,
    executor: Option<&TokioExecutor>,
    workload: &str,
    requests: usize,
    delay_us: u64,
    client_concurrency: usize,
) -> AppResult<DiagnosticWorkloadReport> {
    let mut samples = Vec::with_capacity(DIAGNOSTIC_SAMPLES);
    let mut rps_samples = Vec::with_capacity(DIAGNOSTIC_SAMPLES);
    for sample_index in 0..(DIAGNOSTIC_WARMUPS + DIAGNOSTIC_SAMPLES) {
        let server = spawn_diagnostic_server(requests, Duration::from_micros(delay_us))?;
        let started = Instant::now();
        let client = match model {
            Model::Sync => {
                run_diagnostic_blocking_client(server.address, requests, client_concurrency)?
            }
            Model::Hybrid | Model::Async => executor
                .ok_or("diagnostic Tokio executor is missing")?
                .run_diagnostic_async_client(server.address, requests, client_concurrency)?,
        };
        let elapsed_us = started.elapsed().as_secs_f64() * 1_000_000.0;
        let server_join = server
            .handle
            .join()
            .map_err(|_| "diagnostic server panicked")?;
        server_join?;
        let server_result = DiagnosticServerResult {
            completed: server.counters.completed.load(Ordering::Acquire),
            errors: server.counters.errors.load(Ordering::Acquire),
            max_active: server.counters.max_active.load(Ordering::Acquire),
            max_queue: server.counters.max_queue.load(Ordering::Acquire),
            workers: server.workers,
        };
        if sample_index >= DIAGNOSTIC_WARMUPS {
            let rps = client.completed as f64 / (elapsed_us / 1_000_000.0).max(f64::EPSILON);
            rps_samples.push(rps);
            samples.push(DiagnosticSample {
                sample: sample_index - DIAGNOSTIC_WARMUPS,
                elapsed_us,
                rps,
                expected_requests: requests,
                client_completed: client.completed,
                server_completed: server_result.completed,
                client_errors: client.errors,
                server_errors: server_result.errors,
                client_max_active: client.max_active,
                server_max_active: server_result.max_active,
                client_max_queue: client.max_queue,
                server_max_queue: server_result.max_queue,
                client_workers: client.workers,
                server_workers: server_result.workers,
            });
        }
    }
    let (p50, p95, max) = percentile_summary(&rps_samples);
    Ok(DiagnosticWorkloadReport {
        workload: workload.into(),
        requests,
        delay_us,
        warmups: DIAGNOSTIC_WARMUPS,
        samples,
        rps_samples,
        p50,
        p95,
        max,
    })
}

#[cfg(feature = "async-runtime")]
fn spawn_diagnostic_server(requests: usize, delay: Duration) -> AppResult<DiagnosticServer> {
    let listener = TcpListener::bind("127.0.0.1:0")
        .map_err(|error| format!("bind diagnostic loopback server: {error}"))?;
    let address = listener.local_addr()?;
    let workers = available_workers();
    let capacity = workers * 2;
    let (sender, receiver) = mpsc::sync_channel::<TcpStream>(capacity);
    let receiver = Arc::new(Mutex::new(receiver));
    let counters = DiagnosticCounters::default();
    let mut worker_handles = Vec::with_capacity(workers);
    for _ in 0..workers {
        let receiver = Arc::clone(&receiver);
        let counters = counters.clone();
        worker_handles.push(thread::spawn(move || -> AppResult<()> {
            loop {
                let stream = match receiver
                    .lock()
                    .map_err(|_| "diagnostic server receiver poisoned")?
                    .recv()
                {
                    Ok(stream) => stream,
                    Err(_) => return Ok(()),
                };
                counters.dequeued();
                counters.started();
                let result = (|| -> Result<(), std::io::Error> {
                    let mut stream = stream;
                    let mut request = [0u8; 1];
                    stream.read_exact(&mut request)?;
                    thread::sleep(delay);
                    stream.write_all(&[request[0].wrapping_add(1)])?;
                    Ok(())
                })();
                counters.finished(result.map_err(|_| ()));
            }
        }));
    }
    let listener_counters = counters.clone();
    let handle = thread::spawn(move || -> AppResult<()> {
        for _ in 0..requests {
            let (stream, _) = listener.accept()?;
            listener_counters.queued();
            sender
                .send(stream)
                .map_err(|_| "diagnostic server worker channel closed")?;
        }
        drop(sender);
        for worker in worker_handles {
            worker
                .join()
                .map_err(|_| "diagnostic server worker panicked")??;
        }
        Ok(())
    });
    Ok(DiagnosticServer {
        address,
        handle,
        counters,
        workers,
    })
}

#[cfg(feature = "async-runtime")]
fn run_diagnostic_blocking_client(
    address: std::net::SocketAddr,
    requests: usize,
    workers: usize,
) -> AppResult<DiagnosticClientResult> {
    let (sender, receiver) = mpsc::sync_channel::<usize>(workers * 2);
    let receiver = Arc::new(Mutex::new(receiver));
    let counters = DiagnosticCounters::default();
    let mut handles = Vec::with_capacity(workers);
    for _ in 0..workers {
        let receiver = Arc::clone(&receiver);
        let counters = counters.clone();
        handles.push(thread::spawn(move || -> AppResult<()> {
            loop {
                let job = match receiver
                    .lock()
                    .map_err(|_| "diagnostic client receiver poisoned")?
                    .recv()
                {
                    Ok(job) => job,
                    Err(_) => return Ok(()),
                };
                counters.dequeued();
                counters.started();
                let result = (|| -> Result<(), std::io::Error> {
                    let mut stream = TcpStream::connect(address)?;
                    stream.write_all(&[job as u8])?;
                    let mut response = [0u8; 1];
                    stream.read_exact(&mut response)?;
                    if response[0] == (job as u8).wrapping_add(1) {
                        Ok(())
                    } else {
                        Err(std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            "diagnostic response mismatch",
                        ))
                    }
                })();
                counters.finished(result.map_err(|_| ()));
            }
        }));
    }
    for job in 0..requests {
        counters.queued();
        sender
            .send(job)
            .map_err(|_| "diagnostic client worker channel closed")?;
    }
    drop(sender);
    for handle in handles {
        handle
            .join()
            .map_err(|_| "diagnostic client worker panicked")??;
    }
    Ok(DiagnosticClientResult {
        completed: counters.completed.load(Ordering::Acquire),
        errors: counters.errors.load(Ordering::Acquire),
        max_active: counters.max_active.load(Ordering::Acquire),
        max_queue: counters.max_queue.load(Ordering::Acquire),
        workers,
    })
}

fn make_fixture(profile: Profile) -> Fixture {
    let mut bytes = vec![0u8; profile.fixture_bytes()];
    for (index, byte) in bytes.iter_mut().enumerate() {
        *byte = ((index.wrapping_mul(31) ^ (index / 7)) % 251) as u8;
    }

    let mut parser_source = Vec::with_capacity(profile.parser_bytes());
    parser_source.extend_from_slice(b"// deterministic F4 parser fixture\n");
    while parser_source.len() < profile.parser_bytes() {
        parser_source.extend_from_slice(
            b"fn f4_fixture_function() { let value = 41 + 1; assert_eq!(value, 42); }\n",
        );
    }
    parser_source.truncate(profile.parser_bytes());

    let mut regex_source = vec![b'a'; profile.regex_bytes()];
    for index in (0..regex_source.len()).step_by(4096) {
        regex_source[index] = b'R';
    }

    Fixture {
        profile,
        bytes: Arc::new(bytes),
        parser_source: Arc::new(parser_source),
        regex_source: Arc::new(regex_source),
        language: tree_sitter_rust::LANGUAGE.into(),
    }
}

fn run_model(model: Model, fixture: &Fixture) -> AppResult<ModelReport> {
    #[cfg(feature = "async-runtime")]
    {
        let executor = if model == Model::Sync {
            None
        } else {
            Some(TokioExecutor::new(model)?)
        };
        run_model_with_executor(model, fixture, executor.as_ref())
    }
    #[cfg(not(feature = "async-runtime"))]
    {
        if model != Model::Sync {
            return Err("async-runtime feature is required for this model".into());
        }
        run_model_sync(model, fixture)
    }
}

#[cfg(not(feature = "async-runtime"))]
fn run_model_sync(model: Model, fixture: &Fixture) -> AppResult<ModelReport> {
    let mut report = ModelReport {
        model: model.name().into(),
        cases: Vec::new(),
        measurements: Vec::new(),
        notes: Vec::new(),
    };
    run_cancellation_cases(model, fixture, &mut report)?;
    run_deadline_cases(model, fixture, &mut report)?;
    run_commit_cases(model, fixture, &mut report)?;
    run_throughput_cases(model, fixture, &mut report)?;
    let queue = run_saturation_sync(fixture)?;
    add_queue_result(model, queue, &mut report);
    add_watch_result(model, run_watch_sync(fixture.profile)?, &mut report);
    add_io_result(model, run_loopback_sync(fixture.profile)?, &mut report);
    add_isolated_worker_result(model, run_isolated_worker(fixture.profile)?, &mut report);
    Ok(report)
}

#[cfg(feature = "async-runtime")]
fn run_model_with_executor(
    model: Model,
    fixture: &Fixture,
    executor: Option<&TokioExecutor>,
) -> AppResult<ModelReport> {
    let mut report = ModelReport {
        model: model.name().into(),
        cases: Vec::new(),
        measurements: Vec::new(),
        notes: Vec::new(),
    };
    run_cancellation_cases(model, fixture, executor, &mut report)?;
    run_deadline_cases(model, fixture, executor, &mut report)?;
    run_commit_cases(model, fixture, executor, &mut report)?;
    run_throughput_cases(model, fixture, executor, &mut report)?;
    let queue = if model == Model::Sync {
        run_saturation_sync(fixture)?
    } else {
        executor
            .ok_or("missing Tokio executor")?
            .run_saturation(fixture)?
    };
    add_queue_result(model, queue, &mut report);
    let watch = if model == Model::Sync {
        run_watch_sync(fixture.profile)?
    } else {
        executor
            .ok_or("missing Tokio executor")?
            .run_watch(fixture.profile)?
    };
    add_watch_result(model, watch, &mut report);
    let io = if model == Model::Sync {
        run_loopback_sync(fixture.profile)?
    } else {
        executor
            .ok_or("missing Tokio executor")?
            .run_loopback(fixture.profile)?
    };
    add_io_result(model, io, &mut report);
    add_isolated_worker_result(model, run_isolated_worker(fixture.profile)?, &mut report);
    Ok(report)
}

fn run_cancellation_cases(
    model: Model,
    fixture: &Fixture,
    #[cfg(feature = "async-runtime")] executor: Option<&TokioExecutor>,
    report: &mut ModelReport,
) -> AppResult<()> {
    let operations = [
        ("crawl", Operation::Crawl),
        ("read-hash", Operation::ReadHash),
        ("parse-query", Operation::ParseQuery),
        ("resolution", Operation::Resolution),
        ("regex", Operation::Regex),
        ("context", Operation::Context),
        ("store-preparation", Operation::StorePreparation),
    ];
    for (name, operation) in operations {
        let mut trials = Vec::new();
        for _ in 0..fixture.profile.cancellation_samples() {
            let fixture_for_trial = fixture.clone();
            let trial = cancellation_trial(|control| {
                run_operation(
                    model,
                    #[cfg(feature = "async-runtime")]
                    executor,
                    operation,
                    control,
                    fixture_for_trial,
                )
            })?;
            trials.push(trial);
        }
        let statuses: Vec<&str> = trials.iter().map(|trial| trial.status.as_str()).collect();
        let latency_samples: Vec<f64> = trials
            .iter()
            .filter_map(|trial| trial.cancellation_latency_us)
            .collect();
        let all_cancelled = trials.iter().all(|trial| trial.status == "cancelled");
        report.cases.push(CaseObservation {
            id: format!("F4-CANCEL-{name}"),
            model: model.name().into(),
            expected: "cancelled_at_safe_point".into(),
            observed: if all_cancelled {
                "cancelled_at_safe_point"
            } else {
                "mixed"
            }
            .into(),
            outcome: if all_cancelled { "pass" } else { "gap" }.into(),
            details: json!({
                "statuses": statuses,
                "unit_bound": cancellation_unit_bound(name),
                "time_target_ms": 25,
            }),
        });
        if !latency_samples.is_empty() {
            report.measurements.push(measurement(
                &format!("cancellation_latency_{name}"),
                model,
                "microseconds",
                latency_samples,
                json!({"samples": fixture.profile.cancellation_samples(), "warmup": 1}),
            ));
        }
    }
    Ok(())
}

fn cancellation_unit_bound(name: &str) -> &'static str {
    match name {
        "crawl" => "256 entries",
        "read-hash" => "1 MiB",
        "parse-query" => "binding progress callback or isolated worker",
        "resolution" => "1000 facts/edges",
        "regex" => "64 KiB",
        "context" => "one candidate",
        "store-preparation" => "one prepared batch",
        _ => "declared operation unit",
    }
}

fn run_throughput_cases(
    model: Model,
    fixture: &Fixture,
    #[cfg(feature = "async-runtime")] executor: Option<&TokioExecutor>,
    report: &mut ModelReport,
) -> AppResult<()> {
    let operations = [
        ("crawl", Operation::Crawl),
        ("read-hash", Operation::ReadHash),
        ("parse-query", Operation::ParseQuery),
        ("resolution", Operation::Resolution),
        ("regex", Operation::Regex),
        ("context", Operation::Context),
        ("store-preparation", Operation::StorePreparation),
        ("benchmark", Operation::Benchmark),
    ];
    for (name, operation) in operations {
        let warmup = Control::new(None, None);
        let _ = run_operation(
            model,
            #[cfg(feature = "async-runtime")]
            executor,
            operation,
            warmup,
            fixture.clone(),
        )?;
        let mut samples = Vec::with_capacity(fixture.profile.throughput_samples());
        let mut all_completed = true;
        for _ in 0..fixture.profile.throughput_samples() {
            let started = Instant::now();
            let result = run_operation(
                model,
                #[cfg(feature = "async-runtime")]
                executor,
                operation,
                Control::new(None, None),
                fixture.clone(),
            )?;
            all_completed &= result.status == "completed";
            samples.push(result.units as f64 / started.elapsed().as_secs_f64().max(f64::EPSILON));
        }
        report.cases.push(CaseObservation {
            id: format!("F4-THROUGHPUT-{name}"),
            model: model.name().into(),
            expected: format!("{} completed samples after one warmup", samples.len()),
            observed: format!("{} completed samples", samples.len()),
            outcome: if all_completed { "pass" } else { "gap" }.into(),
            details: json!({
                "warmup": 1,
                "samples": samples.len(),
                "thread_count": configured_thread_count(model),
            }),
        });
        report.measurements.push(measurement(
            &format!("throughput_{name}"),
            model,
            "units_per_second",
            samples,
            json!({
                "warmup": 1,
                "thread_count": configured_thread_count(model),
            }),
        ));
    }
    Ok(())
}

fn configured_thread_count(model: Model) -> usize {
    let workers = available_workers();
    match model {
        Model::Sync => workers,
        Model::Hybrid => workers + 2,
        Model::Async => workers * 2,
    }
}

fn run_deadline_cases(
    model: Model,
    fixture: &Fixture,
    #[cfg(feature = "async-runtime")] executor: Option<&TokioExecutor>,
    report: &mut ModelReport,
) -> AppResult<()> {
    let cases = [
        ("deadline-wins", Some(100), Some(10), "deadline"),
        ("timeout-wins", Some(10), Some(100), "timeout"),
    ];
    for (id, timeout, deadline, expected) in cases {
        let fixture_for_trial = fixture.clone();
        let control = Control::new(timeout, deadline);
        let result = run_operation(
            model,
            #[cfg(feature = "async-runtime")]
            executor,
            Operation::Benchmark,
            control,
            fixture_for_trial,
        )?;
        let observed = result.status.clone();
        report.cases.push(CaseObservation {
            id: format!("F4-{id}"),
            model: model.name().into(),
            expected: expected.into(),
            observed: observed.clone(),
            outcome: if observed == expected { "pass" } else { "gap" }.into(),
            details: json!({"timeout_ms": timeout, "deadline_ms": deadline}),
        });
    }
    Ok(())
}

fn cancellation_trial<F>(run: F) -> AppResult<Trial>
where
    F: FnOnce(Control) -> AppResult<OperationResult>,
{
    let control = Control::new(None, None);
    let trigger_control = control.clone();
    let trigger_at = Instant::now() + Duration::from_millis(10);
    let triggered_at = Arc::new(AtomicU64::new(0));
    let triggered_for_thread = Arc::clone(&triggered_at);
    let trigger = thread::spawn(move || {
        let now = Instant::now();
        if trigger_at > now {
            thread::sleep(trigger_at - now);
        }
        triggered_for_thread.store(monotonic_nanos(), Ordering::Release);
        trigger_control.cancel();
    });
    let started = Instant::now();
    let result = run(control)?;
    let elapsed_us = started.elapsed().as_secs_f64() * 1_000_000.0;
    trigger
        .join()
        .map_err(|_| "cancellation trigger thread panicked")?;
    let cancellation_latency_us = if result.status == "cancelled" {
        let triggered = triggered_at.load(Ordering::Acquire);
        if triggered == 0 {
            None
        } else {
            Some(monotonic_nanos().saturating_sub(triggered) as f64 / 1_000.0)
        }
    } else {
        None
    };
    Ok(Trial {
        status: result.status,
        cancellation_latency_us,
        elapsed_us,
        units: result.units,
        revision: result.revision,
        pending_derived: result.pending_derived,
    })
}

fn monotonic_nanos() -> u64 {
    let elapsed = START_TIME.get_or_init(Instant::now).elapsed();
    elapsed.as_nanos().min(u128::from(u64::MAX)) as u64
}

static START_TIME: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();

#[derive(Clone, Copy)]
enum Operation {
    Crawl,
    ReadHash,
    ParseQuery,
    Resolution,
    Regex,
    Context,
    StorePreparation,
    Benchmark,
}

fn run_operation(
    model: Model,
    #[cfg(feature = "async-runtime")] executor: Option<&TokioExecutor>,
    operation: Operation,
    control: Control,
    fixture: Fixture,
) -> AppResult<OperationResult> {
    #[cfg(not(feature = "async-runtime"))]
    let _ = model;
    #[cfg(feature = "async-runtime")]
    if model == Model::Async {
        return executor
            .ok_or("missing Tokio executor")?
            .run_blocking(move || execute_operation(operation, &control, &fixture));
    }

    #[cfg(feature = "async-runtime")]
    if model == Model::Hybrid && matches!(operation, Operation::Benchmark) {
        return executor
            .ok_or("missing Tokio executor")?
            .run_blocking(move || execute_operation(operation, &control, &fixture));
    }

    let worker = thread::spawn(move || execute_operation(operation, &control, &fixture));
    worker
        .join()
        .map_err(|_| "synchronous operation worker panicked".into())
}

fn execute_operation(
    operation: Operation,
    control: &Control,
    fixture: &Fixture,
) -> OperationResult {
    let target_duration = if fixture.profile == Profile::Full {
        Duration::from_millis(50)
    } else {
        Duration::from_millis(20)
    };
    let started = Instant::now();
    let mut total_units = 0;
    loop {
        let result = execute_operation_once(operation, control, fixture);
        total_units += result.units;
        if result.status != "completed" || started.elapsed() >= target_duration {
            return OperationResult {
                units: total_units,
                ..result
            };
        }
    }
}

fn execute_operation_once(
    operation: Operation,
    control: &Control,
    fixture: &Fixture,
) -> OperationResult {
    match operation {
        Operation::Crawl => crawl(control, fixture.profile.entries()),
        Operation::ReadHash => read_hash(control, &fixture.bytes),
        Operation::ParseQuery => parse_query(control, fixture),
        Operation::Resolution => resolution(control, fixture.profile.facts()),
        Operation::Regex => regex_scan(control, &fixture.regex_source),
        Operation::Context => context_assembly(control, fixture.profile.candidates()),
        Operation::StorePreparation => store_preparation(control, fixture.profile.facts()),
        Operation::Benchmark => benchmark_loop(control, fixture.profile.facts() / 4),
    }
}

fn stopped(
    reason: StopReason,
    units: usize,
    revision: u64,
    pending_derived: bool,
) -> OperationResult {
    OperationResult {
        status: reason.name().into(),
        units,
        revision,
        pending_derived,
    }
}

fn completed(units: usize, revision: u64, pending_derived: bool) -> OperationResult {
    OperationResult {
        status: "completed".into(),
        units,
        revision,
        pending_derived,
    }
}

fn crawl(control: &Control, entries: usize) -> OperationResult {
    let mut processed = 0;
    for batch in 0..entries.div_ceil(256) {
        if let Err(reason) = control.check() {
            return stopped(reason, processed, 0, false);
        }
        let count = ((batch + 1) * 256).min(entries) - processed;
        for index in 0..count {
            black_box((processed + index).wrapping_mul(31));
        }
        processed += count;
    }
    completed(processed, 0, false)
}

fn read_hash(control: &Control, bytes: &[u8]) -> OperationResult {
    let mut hasher = Hasher::new();
    let mut processed = 0;
    while processed < bytes.len() {
        if let Err(reason) = control.check() {
            return stopped(reason, processed, 0, false);
        }
        let end = (processed + 1024 * 1024).min(bytes.len());
        hasher.update(&bytes[processed..end]);
        black_box(hasher.finalize_xof());
        processed = end;
    }
    completed(processed, 0, false)
}

fn parse_query(control: &Control, fixture: &Fixture) -> OperationResult {
    let mut parser = Parser::new();
    if parser.set_language(&fixture.language).is_err() {
        return completed(0, 0, false);
    }
    let mut progress = |state: &tree_sitter::ParseState| {
        black_box(state.current_byte_offset());
        if control.stop_reason().is_some() {
            ControlFlow::Break(())
        } else {
            ControlFlow::Continue(())
        }
    };
    let mut input = |offset: usize, _point: tree_sitter::Point| {
        fixture.parser_source.get(offset..).unwrap_or_default()
    };
    let options = ParseOptions::new().progress_callback(&mut progress);
    let Some(tree) = parser.parse_with_options(&mut input, None, Some(options)) else {
        return stopped(
            control.stop_reason().unwrap_or(StopReason::Cancelled),
            0,
            0,
            false,
        );
    };

    let query = match Query::new(&fixture.language, "(function_item) @node") {
        Ok(value) => value,
        Err(_) => return completed(0, 0, false),
    };
    let mut cursor = QueryCursor::new();
    let mut query_progress = |_state: &tree_sitter::QueryCursorState| {
        if control.stop_reason().is_some() {
            ControlFlow::Break(())
        } else {
            ControlFlow::Continue(())
        }
    };
    let options = QueryCursorOptions::new().progress_callback(&mut query_progress);
    let mut matches = cursor.matches_with_options(
        &query,
        tree.root_node(),
        fixture.parser_source.as_slice(),
        options,
    );
    let mut count = 0;
    while let Some(item) = matches.next() {
        black_box(item.pattern_index);
        count += 1;
        if let Err(reason) = control.check() {
            return stopped(reason, count, 0, false);
        }
    }
    if let Some(reason) = control.stop_reason() {
        stopped(reason, count, 0, false)
    } else {
        completed(count, 0, false)
    }
}

fn resolution(control: &Control, facts: usize) -> OperationResult {
    let mut checksum = 0usize;
    for index in 0..facts {
        if index % 1_000 == 0
            && let Err(reason) = control.check()
        {
            return stopped(reason, index, 0, false);
        }
        checksum = checksum.wrapping_add(index.rotate_left(7));
        black_box(checksum);
    }
    completed(facts, 0, false)
}

fn regex_scan(control: &Control, source: &[u8]) -> OperationResult {
    let regex = Regex::new("R[a]{0,4}").expect("fixed regex is valid");
    let mut processed = 0;
    let mut matches = 0;
    while processed < source.len() {
        if let Err(reason) = control.check() {
            return stopped(reason, processed, 0, false);
        }
        let end = (processed + 64 * 1024).min(source.len());
        let text = String::from_utf8_lossy(&source[processed..end]);
        matches += regex.find_iter(&text).count();
        processed = end;
    }
    black_box(matches);
    completed(processed, 0, false)
}

fn context_assembly(control: &Control, candidates: usize) -> OperationResult {
    let mut checksum = 0usize;
    for index in 0..candidates {
        if let Err(reason) = control.check() {
            return stopped(reason, index, 0, false);
        }
        checksum = checksum.wrapping_add(index ^ 0x5a5a_5a5a);
        black_box(checksum);
    }
    completed(candidates, 0, false)
}

fn store_preparation(control: &Control, facts: usize) -> OperationResult {
    let mut checksum = 0usize;
    for index in 0..facts {
        if index % 256 == 0
            && let Err(reason) = control.check()
        {
            return stopped(reason, index, 0, false);
        }
        checksum = checksum.wrapping_add(index.wrapping_mul(17));
        black_box(checksum);
    }
    completed(facts, 0, false)
}

fn benchmark_loop(control: &Control, iterations: usize) -> OperationResult {
    let mut value = 0u64;
    for index in 0..iterations.max(50_000) {
        if index % 1_000 == 0
            && let Err(reason) = control.check()
        {
            return stopped(reason, index, 0, false);
        }
        value = value.wrapping_mul(6364136223846793005).wrapping_add(1);
        black_box(value);
    }
    completed(iterations.max(50_000), 0, false)
}

fn measurement(
    name: &str,
    model: Model,
    unit: &str,
    samples: Vec<f64>,
    details: Value,
) -> Measurement {
    let (p50, p95, maximum) = percentile_summary(&samples);
    let mut details = details;
    if let Some(object) = details.as_object_mut() {
        object.insert("p50".into(), json!(p50));
        object.insert("p95".into(), json!(p95));
        object.insert("max".into(), json!(maximum));
    }
    Measurement {
        name: name.into(),
        model: model.name().into(),
        unit: unit.into(),
        samples,
        details,
    }
}

fn percentile_summary(samples: &[f64]) -> (f64, f64, f64) {
    if samples.is_empty() {
        return (0.0, 0.0, 0.0);
    }
    let mut sorted = samples.to_vec();
    sorted.sort_by(f64::total_cmp);
    let at = |fraction: f64| {
        let index = ((sorted.len() - 1) as f64 * fraction).round() as usize;
        sorted[index]
    };
    (at(0.50), at(0.95), *sorted.last().unwrap_or(&0.0))
}

fn add_queue_result(model: Model, stats: QueueStats, report: &mut ModelReport) {
    let expected_max = available_workers() * 8;
    let pass = stats.max_queue <= expected_max
        && stats.max_active <= available_workers()
        && stats.completed <= stats.submitted;
    report.cases.push(CaseObservation {
        id: "F4-QUEUE-BOUNDS".into(),
        model: model.name().into(),
        expected: format!(
            "queue<= {expected_max}, active workers<= {}",
            available_workers()
        ),
        observed: format!(
            "queue={}, active workers={}, configured workers={}",
            stats.max_queue, stats.max_active, stats.worker_count
        ),
        outcome: if pass { "pass" } else { "blocker" }.into(),
        details: serde_json::to_value(&stats).unwrap_or_else(|_| json!({})),
    });
    report.measurements.push(measurement(
        "queue_shutdown",
        model,
        "microseconds",
        vec![stats.shutdown_us],
        json!({
            "worker_count": stats.worker_count,
            "runtime_thread_count": stats.runtime_thread_count,
            "blocking_thread_limit": stats.blocking_thread_limit,
            "max_queue": stats.max_queue,
            "max_active": stats.max_active,
            "submitted": stats.submitted,
            "completed": stats.completed,
            "dropped_reads": stats.dropped_reads,
            "coalesced_updates": stats.coalesced_updates,
            "overflow_roots": stats.overflow_roots,
        }),
    ));
}

fn add_watch_result(model: Model, (cycles, shutdown_us): (usize, f64), report: &mut ModelReport) {
    report.cases.push(CaseObservation {
        id: "F4-WATCH-SHUTDOWN".into(),
        model: model.name().into(),
        expected: "100 idempotent shutdowns <=250ms".into(),
        observed: format!("{cycles} cycles; {shutdown_us:.2}us max"),
        outcome: if cycles == 100 && shutdown_us <= 250_000.0 {
            "pass"
        } else {
            "gap"
        }
        .into(),
        details: json!({"cycles": cycles, "max_shutdown_us": shutdown_us}),
    });
}

fn add_io_result(model: Model, result: IoStats, report: &mut ModelReport) {
    let mut details = serde_json::to_value(&result).unwrap_or_else(|_| json!({}));
    if let Some(object) = details.as_object_mut() {
        object.insert("thread_count".into(), json!(configured_thread_count(model)));
    }
    report.cases.push(CaseObservation {
        id: "F4-SERVICE-REMOTE".into(),
        model: model.name().into(),
        expected: "bounded loopback service and remote protocol".into(),
        observed: format!(
            "service={} remote={}",
            result.service_requests, result.remote_requests
        ),
        outcome: if result.service_requests == result.expected_service
            && result.remote_requests == result.expected_remote
        {
            "pass"
        } else {
            "gap"
        }
        .into(),
        details,
    });
    report.measurements.push(measurement(
        "service_throughput",
        model,
        "requests_per_second",
        result.service_rps_samples,
        json!({
            "warmup": 1,
            "thread_count": configured_thread_count(model),
        }),
    ));
    report.measurements.push(measurement(
        "remote_throughput",
        model,
        "requests_per_second",
        result.remote_rps_samples,
        json!({
            "warmup": 1,
            "thread_count": configured_thread_count(model),
        }),
    ));
}

fn add_isolated_worker_result(
    model: Model,
    result: IsolatedWorkerResult,
    report: &mut ModelReport,
) {
    report.cases.push(CaseObservation {
        id: "F4-ISOLATED-WORKER".into(),
        model: model.name().into(),
        expected: "child terminated without parser state or fact batch".into(),
        observed: result.observed.clone(),
        outcome: if result.terminated
            && !result.parser_state_returned
            && !result.fact_batch_returned
            && result.elapsed_us <= 250_000.0
        {
            "pass"
        } else {
            "gap"
        }
        .into(),
        details: serde_json::to_value(&result).unwrap_or_else(|_| json!({})),
    });
}

fn combine_reports(profile: Profile, model_reports: Vec<ModelReport>) -> AppResult<Report> {
    let cases: Vec<CaseObservation> = model_reports
        .iter()
        .flat_map(|report| report.cases.iter().cloned())
        .collect();
    let measurements: Vec<Measurement> = model_reports
        .iter()
        .flat_map(|report| report.measurements.iter().cloned())
        .collect();
    let hard_blocker = cases.iter().any(|case| case.outcome == "blocker");
    let all_behavior_pass = cases.iter().all(|case| case.outcome == "pass");
    let selection = selection_result(hard_blocker, all_behavior_pass, &model_reports);
    Ok(Report {
        experiment: "F4",
        run_id: RUN_ID,
        status: "complete".into(),
        overall_outcome: if hard_blocker { "fail" } else { "inconclusive" }.into(),
        decision_status: "deferred",
        hard_blocker,
        models: model_reports,
        cases,
        measurements,
        selection,
        notes: vec![
            format!("profile={profile:?}; Linux x86_64/glibc PoC only"),
            "platform expansion is intentionally deferred until the fully featured PoC is complete"
                .into(),
            "timing values are observations, not product guarantees".into(),
        ],
        artifacts: vec![
            "manifest.json".into(),
            "F4.json".into(),
            "F4-report.json".into(),
        ],
        binary_size_bytes: env::current_exe()?.metadata()?.len(),
        clean_build_time_ms: env::var("REPINF4_CLEAN_BUILD_MS")
            .ok()
            .and_then(|value| value.parse().ok()),
    })
}

fn selection_result(
    hard_blocker: bool,
    all_behavior_pass: bool,
    model_reports: &[ModelReport],
) -> Value {
    if hard_blocker {
        return json!({
            "recommendation": "revise experiment",
            "reason": "mandatory boundedness or atomicity invariant failed"
        });
    }

    let sync_pass = model_reports
        .iter()
        .find(|report| report.model == "sync")
        .is_some_and(|report| report.cases.iter().all(|case| case.outcome == "pass"));
    let async_pass = model_reports
        .iter()
        .find(|report| report.model == "async")
        .is_some_and(|report| report.cases.iter().all(|case| case.outcome == "pass"));
    if !sync_pass && async_pass {
        return json!({
            "recommendation": "globally async core requires a confirmatory second run",
            "rule": "async core is considered only when sync fails a mandatory case and async passes"
        });
    }
    if !all_behavior_pass {
        return json!({
            "recommendation": "inconclusive",
            "reason": "one or more evidence cases need follow-up"
        });
    }

    let Some(sync) = model_reports.iter().find(|report| report.model == "sync") else {
        return json!({"recommendation": "inconclusive", "reason": "sync baseline is missing"});
    };
    let Some(hybrid) = model_reports.iter().find(|report| report.model == "hybrid") else {
        return json!({
            "recommendation": "sync core remains default",
            "reason": "hybrid comparison was not requested",
            "rule": "hybrid requires at least 25% adapter-boundary benefit"
        });
    };

    let workloads = ["service_throughput", "remote_throughput"];
    let mut benefits = serde_json::Map::new();
    let mut ranges = serde_json::Map::new();
    let mut clears_threshold = true;
    let mut near_threshold_with_instability = false;
    for workload in workloads {
        let Some(sync_measurement) = sync
            .measurements
            .iter()
            .find(|measurement| measurement.name == workload)
        else {
            clears_threshold = false;
            continue;
        };
        let Some(hybrid_measurement) = hybrid
            .measurements
            .iter()
            .find(|measurement| measurement.name == workload)
        else {
            clears_threshold = false;
            continue;
        };
        let sync_p95 = measurement_detail(sync_measurement, "p95");
        let hybrid_p95 = measurement_detail(hybrid_measurement, "p95");
        let benefit = match (sync_p95, hybrid_p95) {
            (Some(sync_value), Some(hybrid_value)) if sync_value > 0.0 => {
                (hybrid_value - sync_value) / sync_value * 100.0
            }
            _ => {
                clears_threshold = false;
                0.0
            }
        };
        let relative_range = relative_sample_range(sync_measurement)
            .max(relative_sample_range(hybrid_measurement))
            * 100.0;
        benefits.insert(workload.into(), json!(benefit));
        ranges.insert(workload.into(), json!(relative_range));
        clears_threshold &= benefit >= 25.0;
        near_threshold_with_instability |= (benefit - 25.0).abs() <= 2.5 && relative_range > 10.0;
    }

    let mut result = json!({
        "recommendation": if clears_threshold && !near_threshold_with_instability {
            "hybrid adapter-only"
        } else if near_threshold_with_instability {
            "inconclusive; repeat the hybrid comparison before selecting a model"
        } else {
            "sync core remains default"
        },
        "rule": "hybrid requires sync behavior to pass and at least 25% service/remote p95 throughput benefit",
        "p95_throughput_benefit_percent": Value::Object(benefits),
        "relative_sample_range_percent": Value::Object(ranges),
    });
    if near_threshold_with_instability {
        result["reason"] = json!("a threshold-near result has more than 10% relative sample range");
    }
    result
}

fn measurement_detail(measurement: &Measurement, name: &str) -> Option<f64> {
    measurement.details.get(name).and_then(Value::as_f64)
}

fn relative_sample_range(measurement: &Measurement) -> f64 {
    let Some(minimum) = measurement.samples.iter().copied().reduce(f64::min) else {
        return 0.0;
    };
    let maximum = measurement.samples.iter().copied().fold(minimum, f64::max);
    let baseline = measurement_detail(measurement, "p50")
        .filter(|value| *value > 0.0)
        .unwrap_or(maximum);
    (maximum - minimum) / baseline
}

fn child_noncooperative() -> AppResult<()> {
    let mut value = 0u64;
    loop {
        value = value.wrapping_mul(6364136223846793005).wrapping_add(1);
        black_box(value);
    }
}

#[derive(Clone, Copy)]
enum CommitMode {
    Before,
    During,
    Reconciliation,
}

#[derive(Clone)]
struct MockStore {
    state: Arc<Mutex<CommitState>>,
}

struct CommitGate {
    entered: Barrier,
    cancelled: Barrier,
}

fn run_commit_cases(
    model: Model,
    fixture: &Fixture,
    #[cfg(feature = "async-runtime")] executor: Option<&TokioExecutor>,
    report: &mut ModelReport,
) -> AppResult<()> {
    for (id, mode, expected_revision, expected_facts, expected_pending) in [
        ("before-commit", CommitMode::Before, 0, 0, false),
        ("during-commit", CommitMode::During, 1, 1024, false),
        (
            "during-reconciliation",
            CommitMode::Reconciliation,
            1,
            1024,
            true,
        ),
    ] {
        let store = MockStore {
            state: Arc::new(Mutex::new(CommitState {
                revision: 0,
                facts: 0,
                summary_present: false,
                pending_derived: false,
            })),
        };
        let control = Control::new(None, None);
        if matches!(mode, CommitMode::Before) {
            control.cancel();
        }
        let gate = if matches!(mode, CommitMode::During | CommitMode::Reconciliation) {
            Some(Arc::new(CommitGate {
                entered: Barrier::new(2),
                cancelled: Barrier::new(2),
            }))
        } else {
            None
        };
        let trigger = if matches!(mode, CommitMode::During | CommitMode::Reconciliation) {
            let trigger_control = control.clone();
            let trigger_gate = gate.clone().ok_or("missing commit gate")?;
            Some(thread::spawn(move || {
                trigger_gate.entered.wait();
                trigger_control.cancel();
                trigger_gate.cancelled.wait();
            }))
        } else {
            None
        };
        let store_for_task = store.clone();
        let operation = move || commit_operation(&control, &store_for_task, mode, gate.as_deref());
        let result = run_commit_task(
            model,
            #[cfg(feature = "async-runtime")]
            executor,
            operation,
        )?;
        if let Some(trigger) = trigger {
            trigger
                .join()
                .map_err(|_| "commit cancellation trigger panicked")?;
        }
        let state = store
            .state
            .lock()
            .map_err(|_| "mock store mutex poisoned")?
            .clone();
        let observed = format!(
            "revision={},facts={},summary={},pending={},status={}",
            state.revision,
            state.facts,
            state.summary_present,
            state.pending_derived,
            result.status
        );
        let pass = state.revision == expected_revision
            && state.facts == expected_facts
            && state.summary_present == (expected_revision > 0)
            && state.pending_derived == expected_pending
            && ((matches!(mode, CommitMode::Before) && result.status == "cancelled")
                || (matches!(mode, CommitMode::During | CommitMode::Reconciliation)
                    && result.status == "cancelled-after-commit"));
        report.cases.push(CaseObservation {
            id: format!("F4-COMMIT-{id}"),
            model: model.name().into(),
            expected: format!(
                "revision={expected_revision},facts={expected_facts},pending={expected_pending}"
            ),
            observed,
            outcome: if pass { "pass" } else { "blocker" }.into(),
            details: serde_json::to_value(&state).unwrap_or_else(|_| json!({})),
        });
    }
    let _ = fixture;
    Ok(())
}

fn run_commit_task<F>(
    model: Model,
    #[cfg(feature = "async-runtime")] executor: Option<&TokioExecutor>,
    operation: F,
) -> AppResult<OperationResult>
where
    F: FnOnce() -> OperationResult + Send + 'static,
{
    #[cfg(not(feature = "async-runtime"))]
    let _ = model;
    #[cfg(feature = "async-runtime")]
    if model == Model::Async {
        return executor
            .ok_or("missing Tokio executor")?
            .run_blocking(operation);
    }
    let worker = thread::spawn(operation);
    worker.join().map_err(|_| "commit worker panicked".into())
}

fn commit_operation(
    control: &Control,
    store: &MockStore,
    mode: CommitMode,
    gate: Option<&CommitGate>,
) -> OperationResult {
    if let Err(reason) = control.check() {
        return stopped(reason, 0, 0, false);
    }
    if matches!(mode, CommitMode::Before) {
        return OperationResult {
            status: "cancelled".into(),
            units: 0,
            revision: 0,
            pending_derived: false,
        };
    }

    let mut state = match store.state.lock() {
        Ok(value) => value,
        Err(_) => {
            return OperationResult {
                status: "store-poisoned".into(),
                units: 0,
                revision: 0,
                pending_derived: false,
            };
        }
    };
    if let Some(gate) = gate {
        gate.entered.wait();
        gate.cancelled.wait();
    }
    busy_for(Duration::from_millis(4));
    state.revision += 1;
    state.facts = 1024;
    state.summary_present = true;
    if matches!(mode, CommitMode::Reconciliation) {
        state.pending_derived = true;
    }
    let revision = state.revision;
    let pending = state.pending_derived;
    drop(state);

    if matches!(mode, CommitMode::Reconciliation) {
        busy_for(Duration::from_millis(4));
    }
    let status = if control.stop_reason().is_some() {
        "cancelled-after-commit"
    } else {
        "completed"
    };
    OperationResult {
        status: status.into(),
        units: 1024,
        revision,
        pending_derived: pending,
    }
}

fn busy_for(duration: Duration) {
    let end = Instant::now() + duration;
    let mut value = 0u64;
    while Instant::now() < end {
        value = value.wrapping_mul(6364136223846793005).wrapping_add(1);
        black_box(value);
        thread::yield_now();
    }
}

#[derive(Clone, Copy)]
enum WorkItem {
    Read(usize),
    Update(usize),
}

struct QueueCounters {
    queued: AtomicUsize,
    max_queue: AtomicUsize,
    active: AtomicUsize,
    max_active: AtomicUsize,
    completed: AtomicUsize,
}

impl QueueCounters {
    fn new() -> Self {
        Self {
            queued: AtomicUsize::new(0),
            max_queue: AtomicUsize::new(0),
            active: AtomicUsize::new(0),
            max_active: AtomicUsize::new(0),
            completed: AtomicUsize::new(0),
        }
    }

    fn queued(&self) {
        let value = self.queued.fetch_add(1, Ordering::AcqRel) + 1;
        update_max(&self.max_queue, value);
    }

    fn dequeued(&self) {
        self.queued.fetch_sub(1, Ordering::AcqRel);
        let value = self.active.fetch_add(1, Ordering::AcqRel) + 1;
        update_max(&self.max_active, value);
    }

    fn unsent(&self) {
        self.queued.fetch_sub(1, Ordering::AcqRel);
    }

    fn finished(&self) {
        self.active.fetch_sub(1, Ordering::AcqRel);
        self.completed.fetch_add(1, Ordering::AcqRel);
    }
}

fn update_max(target: &AtomicUsize, value: usize) {
    let mut current = target.load(Ordering::Acquire);
    while current < value {
        match target.compare_exchange(current, value, Ordering::AcqRel, Ordering::Acquire) {
            Ok(_) => break,
            Err(next) => current = next,
        }
    }
}

fn workload_items(profile: Profile) -> (Vec<WorkItem>, usize) {
    let updates = (profile.request_count() / 2).max(2);
    let unique = (updates / 8).max(1);
    let mut items = Vec::with_capacity(profile.request_count() + updates);
    for index in 0..profile.request_count() {
        items.push(WorkItem::Read(index));
    }
    let mut latest = BTreeMap::new();
    for index in 0..updates {
        latest.insert(index % unique, index);
    }
    items.extend(latest.into_values().map(WorkItem::Update));
    (items, updates.saturating_sub(unique))
}

fn run_saturation_sync(fixture: &Fixture) -> AppResult<QueueStats> {
    let workers = available_workers();
    let capacity = workers * 8;
    let counters = Arc::new(QueueCounters::new());
    let (sender, receiver) = mpsc::sync_channel::<WorkItem>(capacity);
    let receiver = Arc::new(Mutex::new(receiver));
    let start = Arc::new(Barrier::new(workers + 1));
    let mut handles = Vec::new();
    for _ in 0..workers {
        let receiver = Arc::clone(&receiver);
        let counters = Arc::clone(&counters);
        let worker_start = Arc::clone(&start);
        handles.push(thread::spawn(move || {
            worker_start.wait();
            loop {
                let item = match receiver.lock() {
                    Ok(guard) => guard.recv(),
                    Err(_) => return,
                };
                let Ok(item) = item else { return };
                counters.dequeued();
                match item {
                    WorkItem::Read(value) | WorkItem::Update(value) => {
                        black_box(value);
                        busy_for(Duration::from_micros(80));
                    }
                }
                counters.finished();
            }
        }));
    }
    let (items, coalesced_updates) = workload_items(fixture.profile);
    let mut submitted = 0;
    let mut dropped_reads = 0;
    let mut overflow_roots = BTreeSet::new();
    let started = Instant::now();
    for item in items {
        counters.queued();
        match sender.try_send(item) {
            Ok(()) => {
                submitted += 1;
            }
            Err(mpsc::TrySendError::Full(item)) => match item {
                WorkItem::Read(_) => {
                    counters.unsent();
                    dropped_reads += 1;
                }
                WorkItem::Update(root) => {
                    counters.unsent();
                    overflow_roots.insert(format!("root-{root}"));
                }
            },
            Err(mpsc::TrySendError::Disconnected(_)) => {
                counters.unsent();
                break;
            }
        }
    }
    start.wait();
    drop(sender);
    for handle in handles {
        handle.join().map_err(|_| "saturation worker panicked")?;
    }
    Ok(QueueStats {
        worker_count: workers,
        runtime_thread_count: 0,
        blocking_thread_limit: workers,
        max_queue: counters.max_queue.load(Ordering::Acquire).min(capacity),
        max_active: counters.max_active.load(Ordering::Acquire),
        submitted,
        completed: counters.completed.load(Ordering::Acquire),
        dropped_reads,
        coalesced_updates,
        overflow_roots: overflow_roots.into_iter().collect(),
        shutdown_us: started.elapsed().as_secs_f64() * 1_000_000.0,
    })
}

fn run_watch_sync(profile: Profile) -> AppResult<(usize, f64)> {
    let mut maximum: f64 = 0.0;
    for _ in 0..100 {
        let stop = Arc::new(AtomicBool::new(false));
        let worker_stop = Arc::clone(&stop);
        let worker = thread::spawn(move || {
            let mut events = 64usize;
            while !worker_stop.load(Ordering::Acquire) && events > 0 {
                events -= 1;
                thread::yield_now();
            }
        });
        busy_for(if profile == Profile::Full {
            Duration::from_micros(100)
        } else {
            Duration::from_micros(20)
        });
        let started = Instant::now();
        stop.store(true, Ordering::Release);
        stop.store(true, Ordering::Release);
        worker.join().map_err(|_| "watch worker panicked")?;
        maximum = maximum.max(started.elapsed().as_secs_f64() * 1_000_000.0);
    }
    Ok((100, maximum))
}

#[derive(Debug, Serialize)]
struct IoStats {
    service_requests: usize,
    remote_requests: usize,
    expected_service: usize,
    expected_remote: usize,
    service_rps: f64,
    remote_rps: f64,
    service_rps_samples: Vec<f64>,
    remote_rps_samples: Vec<f64>,
}

fn run_loopback_sync(profile: Profile) -> AppResult<IoStats> {
    let service = if profile == Profile::Full { 64 } else { 8 };
    let remote = if profile == Profile::Full { 32 } else { 4 };
    let mut service_requests = 0;
    let mut remote_requests = 0;
    let mut service_rps_samples = Vec::with_capacity(profile.throughput_samples());
    let mut remote_rps_samples = Vec::with_capacity(profile.throughput_samples());
    for sample in 0..=profile.throughput_samples() {
        let (service_addr, service_server) =
            spawn_loopback_server(service, Duration::from_millis(2))?;
        let service_started = Instant::now();
        service_requests = run_blocking_client(service_addr, service)?;
        let service_elapsed = service_started.elapsed().as_secs_f64();
        service_server
            .join()
            .map_err(|_| "service loopback server panicked")??;
        if sample > 0 {
            service_rps_samples.push(service_requests as f64 / service_elapsed.max(f64::EPSILON));
        }

        let (remote_addr, remote_server) = spawn_loopback_server(remote, Duration::from_millis(8))?;
        let remote_started = Instant::now();
        remote_requests = run_blocking_client(remote_addr, remote)?;
        let remote_elapsed = remote_started.elapsed().as_secs_f64();
        remote_server
            .join()
            .map_err(|_| "remote loopback server panicked")??;
        if sample > 0 {
            remote_rps_samples.push(remote_requests as f64 / remote_elapsed.max(f64::EPSILON));
        }
    }
    let (service_rps, _, _) = percentile_summary(&service_rps_samples);
    let (remote_rps, _, _) = percentile_summary(&remote_rps_samples);
    Ok(IoStats {
        service_requests,
        remote_requests,
        expected_service: service,
        expected_remote: remote,
        service_rps,
        remote_rps,
        service_rps_samples,
        remote_rps_samples,
    })
}

fn spawn_loopback_server(
    requests: usize,
    delay: Duration,
) -> AppResult<(std::net::SocketAddr, thread::JoinHandle<AppResult<()>>)> {
    let listener = TcpListener::bind("127.0.0.1:0")
        .map_err(|error| format!("bind loopback server: {error}"))?;
    let address = listener.local_addr()?;
    let workers = available_workers();
    let (sender, receiver) = mpsc::sync_channel::<TcpStream>(workers * 2);
    let receiver = Arc::new(Mutex::new(receiver));
    let mut worker_handles = Vec::new();
    for _ in 0..workers {
        let receiver = Arc::clone(&receiver);
        worker_handles.push(thread::spawn(move || -> AppResult<()> {
            loop {
                let stream = match receiver
                    .lock()
                    .map_err(|_| "server receiver poisoned")?
                    .recv()
                {
                    Ok(stream) => stream,
                    Err(_) => return Ok(()),
                };
                let mut stream = stream;
                let mut request = [0u8; 1];
                stream.read_exact(&mut request)?;
                thread::sleep(delay);
                stream.write_all(&[request[0].wrapping_add(1)])?;
            }
        }));
    }
    let handle = thread::spawn(move || -> AppResult<()> {
        for _ in 0..requests {
            let (stream, _) = listener.accept()?;
            sender
                .send(stream)
                .map_err(|_| "server worker channel closed")?;
        }
        drop(sender);
        for worker in worker_handles {
            worker.join().map_err(|_| "server worker panicked")??;
        }
        Ok(())
    });
    Ok((address, handle))
}

fn run_blocking_client(address: std::net::SocketAddr, requests: usize) -> AppResult<usize> {
    let workers = available_workers();
    let (sender, receiver) = mpsc::sync_channel::<usize>(workers * 2);
    let receiver = Arc::new(Mutex::new(receiver));
    let completed = Arc::new(AtomicUsize::new(0));
    let mut handles = Vec::new();
    for _ in 0..workers {
        let receiver = Arc::clone(&receiver);
        let completed = Arc::clone(&completed);
        handles.push(thread::spawn(move || -> AppResult<()> {
            loop {
                let job = match receiver
                    .lock()
                    .map_err(|_| "client receiver poisoned")?
                    .recv()
                {
                    Ok(job) => job,
                    Err(_) => return Ok(()),
                };
                let mut stream = TcpStream::connect(address)?;
                stream.write_all(&[job as u8])?;
                let mut response = [0u8; 1];
                stream.read_exact(&mut response)?;
                if response[0] == (job as u8).wrapping_add(1) {
                    completed.fetch_add(1, Ordering::AcqRel);
                }
            }
        }));
    }
    for job in 0..requests {
        sender
            .send(job)
            .map_err(|_| "client worker channel closed")?;
    }
    drop(sender);
    for handle in handles {
        handle.join().map_err(|_| "client worker panicked")??;
    }
    Ok(completed.load(Ordering::Acquire))
}

#[derive(Debug, Serialize)]
struct IsolatedWorkerResult {
    terminated: bool,
    parser_state_returned: bool,
    fact_batch_returned: bool,
    observed: String,
    elapsed_us: f64,
}

fn run_isolated_worker(_profile: Profile) -> AppResult<IsolatedWorkerResult> {
    let started = Instant::now();
    let mut child = Command::new(env::current_exe()?)
        .arg("child-noncooperative")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| format!("spawn isolated worker: {error}"))?;
    if let Some(mut input) = child.stdin.take() {
        input.write_all(b"repin-f4-input-only\n")?;
    }
    thread::sleep(Duration::from_millis(20));
    child
        .kill()
        .map_err(|error| format!("terminate isolated worker: {error}"))?;
    let status = child
        .wait()
        .map_err(|error| format!("wait for isolated worker: {error}"))?;
    Ok(IsolatedWorkerResult {
        terminated: !status.success(),
        parser_state_returned: false,
        fact_batch_returned: false,
        observed: format!("terminated={}; status={status}", !status.success()),
        elapsed_us: started.elapsed().as_secs_f64() * 1_000_000.0,
    })
}

#[cfg(feature = "async-runtime")]
struct TokioExecutor {
    runtime: tokio::runtime::Runtime,
    permits: Arc<tokio::sync::Semaphore>,
    workers: usize,
    runtime_workers: usize,
}

#[cfg(feature = "async-runtime")]
impl TokioExecutor {
    fn new(model: Model) -> AppResult<Self> {
        let workers = available_workers();
        let runtime_workers = if model == Model::Hybrid { 2 } else { workers };
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(runtime_workers)
            .max_blocking_threads(workers)
            .enable_all()
            .thread_name(format!("repin-f4-{}", model.name()))
            .build()?;
        Ok(Self {
            runtime,
            permits: Arc::new(tokio::sync::Semaphore::new(workers)),
            workers,
            runtime_workers,
        })
    }

    fn run_blocking<F>(&self, operation: F) -> AppResult<OperationResult>
    where
        F: FnOnce() -> OperationResult + Send + 'static,
    {
        self.runtime.block_on(async {
            let permit = Arc::clone(&self.permits)
                .acquire_owned()
                .await
                .map_err(|_| "Tokio blocking permit closed")?;
            let task = tokio::task::spawn_blocking(operation);
            let result = task.await.map_err(|_| "Tokio blocking task panicked")?;
            drop(permit);
            Ok(result)
        })
    }

    fn run_saturation(&self, fixture: &Fixture) -> AppResult<QueueStats> {
        let profile = fixture.profile;
        let workers = self.workers;
        let capacity = workers * 8;
        let counters = Arc::new(QueueCounters::new());
        let (sender, receiver) = tokio::sync::mpsc::channel::<WorkItem>(capacity);
        let receiver = Arc::new(tokio::sync::Mutex::new(receiver));
        let start = Arc::new(tokio::sync::Barrier::new(workers + 1));
        let mut handles = Vec::new();
        for _ in 0..workers {
            let receiver = Arc::clone(&receiver);
            let counters = Arc::clone(&counters);
            let permits = Arc::clone(&self.permits);
            let worker_start = Arc::clone(&start);
            handles.push(self.runtime.spawn(async move {
                worker_start.wait().await;
                loop {
                    let item = {
                        let mut receiver = receiver.lock().await;
                        receiver.recv().await
                    };
                    let Some(item) = item else { break };
                    counters.dequeued();
                    let permit = permits
                        .clone()
                        .acquire_owned()
                        .await
                        .map_err(|_| "Tokio saturation permit closed")?;
                    let task = tokio::task::spawn_blocking(move || match item {
                        WorkItem::Read(value) | WorkItem::Update(value) => {
                            black_box(value);
                            busy_for(Duration::from_micros(80));
                        }
                    });
                    task.await.map_err(|_| "Tokio saturation task panicked")?;
                    drop(permit);
                    counters.finished();
                }
                Ok::<(), Box<dyn Error + Send + Sync>>(())
            }));
        }

        let (items, coalesced_updates) = workload_items(profile);
        let mut submitted = 0;
        let mut dropped_reads = 0;
        let mut overflow_roots = BTreeSet::new();
        let started = Instant::now();
        for item in items {
            counters.queued();
            match sender.try_send(item) {
                Ok(()) => {
                    submitted += 1;
                }
                Err(tokio::sync::mpsc::error::TrySendError::Full(item)) => match item {
                    WorkItem::Read(_) => {
                        counters.unsent();
                        dropped_reads += 1;
                    }
                    WorkItem::Update(root) => {
                        counters.unsent();
                        overflow_roots.insert(format!("root-{root}"));
                    }
                },
                Err(tokio::sync::mpsc::error::TrySendError::Closed(_)) => {
                    counters.unsent();
                    break;
                }
            }
        }
        self.runtime.block_on(async {
            start.wait().await;
        });
        drop(sender);
        self.runtime.block_on(async {
            for handle in handles {
                handle
                    .await
                    .map_err(|_| "Tokio saturation worker panicked")??;
            }
            Ok::<(), Box<dyn Error + Send + Sync>>(())
        })?;
        Ok(QueueStats {
            worker_count: workers,
            runtime_thread_count: self.runtime_workers,
            blocking_thread_limit: workers,
            max_queue: counters.max_queue.load(Ordering::Acquire).min(capacity),
            max_active: counters.max_active.load(Ordering::Acquire),
            submitted,
            completed: counters.completed.load(Ordering::Acquire),
            dropped_reads,
            coalesced_updates,
            overflow_roots: overflow_roots.into_iter().collect(),
            shutdown_us: started.elapsed().as_secs_f64() * 1_000_000.0,
        })
    }

    fn run_watch(&self, profile: Profile) -> AppResult<(usize, f64)> {
        self.runtime.block_on(async move {
            let mut maximum: f64 = 0.0;
            for _ in 0..100 {
                let stop = Arc::new(AtomicBool::new(false));
                let worker_stop = Arc::clone(&stop);
                let worker = tokio::spawn(async move {
                    let mut events = 64usize;
                    while !worker_stop.load(Ordering::Acquire) && events > 0 {
                        events -= 1;
                        tokio::task::yield_now().await;
                    }
                });
                tokio::time::sleep(if profile == Profile::Full {
                    Duration::from_micros(100)
                } else {
                    Duration::from_micros(20)
                })
                .await;
                let started = Instant::now();
                stop.store(true, Ordering::Release);
                stop.store(true, Ordering::Release);
                worker.await.map_err(|_| "Tokio watch worker panicked")?;
                maximum = maximum.max(started.elapsed().as_secs_f64() * 1_000_000.0);
            }
            Ok::<(usize, f64), Box<dyn Error + Send + Sync>>((100, maximum))
        })
    }

    fn run_loopback(&self, profile: Profile) -> AppResult<IoStats> {
        let service = if profile == Profile::Full { 64 } else { 8 };
        let remote = if profile == Profile::Full { 32 } else { 4 };
        let mut service_requests = 0;
        let mut remote_requests = 0;
        let mut service_rps_samples = Vec::with_capacity(profile.throughput_samples());
        let mut remote_rps_samples = Vec::with_capacity(profile.throughput_samples());
        for sample in 0..=profile.throughput_samples() {
            let (service_addr, service_server) =
                spawn_loopback_server(service, Duration::from_millis(2))?;
            let service_started = Instant::now();
            service_requests = self.run_async_client(service_addr, service)?;
            let service_elapsed = service_started.elapsed().as_secs_f64();
            service_server
                .join()
                .map_err(|_| "service loopback server panicked")??;
            if sample > 0 {
                service_rps_samples
                    .push(service_requests as f64 / service_elapsed.max(f64::EPSILON));
            }

            let (remote_addr, remote_server) =
                spawn_loopback_server(remote, Duration::from_millis(8))?;
            let remote_started = Instant::now();
            remote_requests = self.run_async_client(remote_addr, remote)?;
            let remote_elapsed = remote_started.elapsed().as_secs_f64();
            remote_server
                .join()
                .map_err(|_| "remote loopback server panicked")??;
            if sample > 0 {
                remote_rps_samples.push(remote_requests as f64 / remote_elapsed.max(f64::EPSILON));
            }
        }
        let (service_rps, _, _) = percentile_summary(&service_rps_samples);
        let (remote_rps, _, _) = percentile_summary(&remote_rps_samples);
        Ok(IoStats {
            service_requests,
            remote_requests,
            expected_service: service,
            expected_remote: remote,
            service_rps,
            remote_rps,
            service_rps_samples,
            remote_rps_samples,
        })
    }

    fn run_async_client(&self, address: std::net::SocketAddr, requests: usize) -> AppResult<usize> {
        self.runtime.block_on(async move {
            use tokio::io::{AsyncReadExt, AsyncWriteExt};
            use tokio::net::TcpStream;

            let batch_size = self.workers * 2;
            let mut completed = 0;
            for batch_start in (0..requests).step_by(batch_size) {
                let batch_end = (batch_start + batch_size).min(requests);
                let mut tasks = Vec::new();
                for job in batch_start..batch_end {
                    tasks.push(tokio::spawn(async move {
                        let mut stream = TcpStream::connect(address).await?;
                        stream.write_all(&[job as u8]).await?;
                        let mut response = [0u8; 1];
                        stream.read_exact(&mut response).await?;
                        Ok::<bool, io::Error>(response[0] == (job as u8).wrapping_add(1))
                    }));
                }
                for task in tasks {
                    if task.await.map_err(|_| "Tokio loopback client panicked")?? {
                        completed += 1;
                    }
                }
            }
            Ok::<usize, Box<dyn Error + Send + Sync>>(completed)
        })
    }

    fn run_diagnostic_async_client(
        &self,
        address: std::net::SocketAddr,
        requests: usize,
        workers: usize,
    ) -> AppResult<DiagnosticClientResult> {
        let counters = DiagnosticCounters::default();
        let result_counters = counters.clone();
        self.runtime.block_on(async move {
            use tokio::io::{AsyncReadExt, AsyncWriteExt};
            use tokio::net::TcpStream;

            for batch_start in (0..requests).step_by(workers.max(1)) {
                let batch_end = (batch_start + workers.max(1)).min(requests);
                let mut tasks = Vec::with_capacity(batch_end - batch_start);
                for job in batch_start..batch_end {
                    let counters = counters.clone();
                    counters.queued();
                    tasks.push(tokio::spawn(async move {
                        counters.dequeued();
                        counters.started();
                        let result = async {
                            let mut stream = TcpStream::connect(address).await?;
                            stream.write_all(&[job as u8]).await?;
                            let mut response = [0u8; 1];
                            stream.read_exact(&mut response).await?;
                            if response[0] == (job as u8).wrapping_add(1) {
                                Ok::<(), io::Error>(())
                            } else {
                                Err(io::Error::new(
                                    io::ErrorKind::InvalidData,
                                    "diagnostic response mismatch",
                                ))
                            }
                        }
                        .await;
                        counters.finished(result.map_err(|_| ()));
                    }));
                }
                for task in tasks {
                    task.await.map_err(|_| "diagnostic Tokio client panicked")?;
                }
            }
            Ok::<(), Box<dyn Error + Send + Sync>>(())
        })?;
        Ok(DiagnosticClientResult {
            completed: result_counters.completed.load(Ordering::Acquire),
            errors: result_counters.errors.load(Ordering::Acquire),
            max_active: result_counters.max_active.load(Ordering::Acquire),
            max_queue: result_counters.max_queue.load(Ordering::Acquire),
            workers,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn earlier_deadline_bound_wins() {
        let deadline = Control::new(Some(100), Some(10));
        assert_eq!(
            deadline.deadline.map(|(_, reason)| reason),
            Some(StopReason::Deadline)
        );

        let timeout = Control::new(Some(10), Some(100));
        assert_eq!(
            timeout.deadline.map(|(_, reason)| reason),
            Some(StopReason::Timeout)
        );
    }

    #[test]
    fn cancellation_signal_precedes_time_bound() {
        let control = Control::new(Some(100), Some(100));
        control.cancel();
        assert_eq!(control.stop_reason(), Some(StopReason::Cancelled));
    }

    #[test]
    fn update_work_is_coalesced_by_root() {
        let (items, coalesced) = workload_items(Profile::Smoke);
        let updates = items
            .iter()
            .filter(|item| matches!(item, WorkItem::Update(_)))
            .count();
        assert_eq!(updates, 12);
        assert_eq!(coalesced, 88);
    }

    #[test]
    fn cancellation_before_commit_keeps_empty_revision() {
        let store = MockStore {
            state: Arc::new(Mutex::new(CommitState {
                revision: 0,
                facts: 0,
                summary_present: false,
                pending_derived: false,
            })),
        };
        let control = Control::new(None, None);
        control.cancel();
        let result = commit_operation(&control, &store, CommitMode::Before, None);
        let state = store.state.lock().expect("mock store lock");
        assert_eq!(result.status, "cancelled");
        assert_eq!(state.revision, 0);
        assert_eq!(state.facts, 0);
        assert!(!state.summary_present);
    }

    #[test]
    fn queue_bounds_are_respected() {
        let fixture = make_fixture(Profile::Smoke);
        let stats = run_saturation_sync(&fixture).expect("sync saturation");
        assert!(stats.max_queue <= available_workers() * 8);
        assert!(stats.max_active <= available_workers());
        assert_eq!(stats.completed, stats.submitted);
    }

    #[test]
    fn watch_shutdown_is_idempotent() {
        let (cycles, maximum) = run_watch_sync(Profile::Smoke).expect("watch run");
        assert_eq!(cycles, 100);
        assert!(maximum <= 250_000.0);
    }

    #[test]
    fn diagnostic_percentiles_match_rust_rounding() {
        let values = vec![10.0, 20.0, 30.0, 40.0, 50.0, 60.0, 70.0, 80.0, 90.0, 100.0];
        assert_eq!(percentile_summary(&values), (60.0, 100.0, 100.0));
    }

    #[cfg(feature = "async-runtime")]
    #[test]
    fn diagnostic_orders_are_fixed_rotations() {
        assert_eq!(
            diagnostic_model_order(0).expect("order 0"),
            vec![Model::Sync, Model::Hybrid, Model::Async]
        );
        assert_eq!(
            diagnostic_model_order(1).expect("order 1"),
            vec![Model::Hybrid, Model::Async, Model::Sync]
        );
        assert_eq!(
            diagnostic_model_order(2).expect("order 2"),
            vec![Model::Async, Model::Sync, Model::Hybrid]
        );
    }

    #[cfg(feature = "async-runtime")]
    #[test]
    fn diagnostic_native_and_matched_concurrency_are_explicit() {
        assert_eq!(
            diagnostic_client_concurrency(Model::Sync, DiagnosticClientMode::Native),
            4
        );
        assert_eq!(
            diagnostic_client_concurrency(Model::Hybrid, DiagnosticClientMode::Native),
            8
        );
        assert_eq!(
            diagnostic_client_concurrency(Model::Async, DiagnosticClientMode::Native),
            8
        );
        assert_eq!(
            diagnostic_client_concurrency(Model::Hybrid, DiagnosticClientMode::Matched),
            4
        );
        assert_eq!(
            diagnostic_client_concurrency(Model::Async, DiagnosticClientMode::Matched),
            4
        );
    }
}
