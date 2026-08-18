//! Disposable F-008 root-capability open experiment.
//!
//! This is an experiment harness, not production filesystem code. It compares
//! a canonicalize-then-open baseline with a root-relative cap-std open under
//! deterministic component replacement and symlink attacks.

use blake3::Hasher;
use cap_fs_ext::{FollowSymlinks, MetadataExt, OpenOptionsFollowExt};
use cap_std::ambient_authority;
use cap_std::fs::{Dir, OpenOptions};
use std::env;
use std::fs;
use std::io::{self, Read};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, SyncSender};
use std::thread;

const ITERATIONS: usize = 100;
const MAX_BYTES: usize = 1024;
const IN_ROOT: &[u8] = b"IN_ROOT\n";
const OUTSIDE: &[u8] = b"OUTSIDE_ROOT\n";

static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Copy)]
enum Protocol {
    Baseline,
    Capability,
}

impl Protocol {
    fn name(self) -> &'static str {
        match self {
            Self::Baseline => "canonicalize_then_open",
            Self::Capability => "root_capability_open",
        }
    }
}

#[derive(Clone, Copy)]
enum Case {
    Normal,
    Traversal,
    Absolute,
    Escape,
    ComponentSwap,
    FinalSwap,
}

impl Case {
    fn name(self) -> &'static str {
        match self {
            Self::Normal => "P-NORMAL",
            Self::Traversal => "P-TRAVERSAL",
            Self::Absolute => "P-ABSOLUTE",
            Self::Escape => "P-ESCAPE",
            Self::ComponentSwap => "P-SWAP-COMPONENT",
            Self::FinalSwap => "P-SWAP-FINAL",
        }
    }

    fn path(self, fixture: &Fixture) -> PathBuf {
        match self {
            Self::Normal | Self::ComponentSwap => PathBuf::from("swap/dir/target.txt"),
            Self::Escape => PathBuf::from("escape.txt"),
            Self::FinalSwap => PathBuf::from("final.txt"),
            Self::Traversal => PathBuf::from("swap/dir/../dir/target.txt"),
            Self::Absolute => fixture.root.join("swap/dir/target.txt"),
        }
    }

    fn needs_attack(self) -> bool {
        matches!(self, Self::ComponentSwap | Self::FinalSwap)
    }
}

#[derive(Debug, Clone, Copy)]
enum Outcome {
    AcceptedInRoot,
    Rejected,
    UnstableSnapshot,
    OutsideRoot,
    UnexpectedError,
    Unsupported,
}

impl Outcome {
    fn name(self) -> &'static str {
        match self {
            Self::AcceptedInRoot => "accepted_in_root",
            Self::Rejected => "rejected",
            Self::UnstableSnapshot => "unstable_snapshot",
            Self::OutsideRoot => "outside_root",
            Self::UnexpectedError => "unexpected_error",
            Self::Unsupported => "unsupported",
        }
    }
}

#[derive(Debug)]
struct Observation {
    outcome: Outcome,
    error_kind: Option<String>,
    bytes_label: Option<&'static str>,
    hash: Option<String>,
}

impl Observation {
    fn accepted(bytes: &[u8]) -> Self {
        let bytes_label = if bytes == IN_ROOT {
            Some("IN_ROOT")
        } else if bytes == OUTSIDE {
            Some("OUTSIDE_ROOT")
        } else {
            None
        };
        let outcome = if bytes == OUTSIDE {
            Outcome::OutsideRoot
        } else if bytes == IN_ROOT {
            Outcome::AcceptedInRoot
        } else {
            Outcome::UnexpectedError
        };
        Self {
            outcome,
            error_kind: None,
            bytes_label,
            hash: Some(hash_bytes(bytes)),
        }
    }

    fn error(outcome: Outcome, error: &io::Error) -> Self {
        Self {
            outcome,
            error_kind: Some(format!("{:?}", error.kind())),
            bytes_label: None,
            hash: None,
        }
    }

    fn unsupported(error: &io::Error) -> Self {
        Self::error(Outcome::Unsupported, error)
    }
}

struct Fixture {
    base: PathBuf,
    root: PathBuf,
    outside: PathBuf,
}

impl Fixture {
    fn new() -> io::Result<Self> {
        let id = NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed);
        let base = env::temp_dir().join(format!("repin-f008-{}-{}", std::process::id(), id));
        fs::create_dir(&base)?;
        let fixture = Self {
            root: base.join("root"),
            outside: base.join("outside"),
            base,
        };
        fixture.reset()?;
        Ok(fixture)
    }

    fn reset(&self) -> io::Result<()> {
        if self.root.exists() {
            fs::remove_dir_all(&self.root)?;
        }
        if self.outside.exists() {
            fs::remove_dir_all(&self.outside)?;
        }

        fs::create_dir_all(self.root.join("swap/dir"))?;
        fs::create_dir_all(self.outside.join("swap/dir"))?;
        fs::write(self.root.join("swap/dir/target.txt"), IN_ROOT)?;
        fs::write(self.outside.join("swap/dir/target.txt"), OUTSIDE)?;
        fs::write(self.root.join("final.txt"), IN_ROOT)?;
        fs::write(self.outside.join("outside-final.txt"), OUTSIDE)?;
        link_file(
            &self.outside.join("outside-final.txt"),
            &self.root.join("escape.txt"),
        )?;
        Ok(())
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.base);
    }
}

fn main() -> io::Result<()> {
    println!(
        "{{\"type\":\"run\",\"experiment\":\"F-008\",\"platform\":{:?},\"iterations\":{},\"max_bytes\":{}}}",
        env::consts::OS,
        ITERATIONS,
        MAX_BYTES
    );

    for protocol in [Protocol::Baseline, Protocol::Capability] {
        for case in [
            Case::Normal,
            Case::Traversal,
            Case::Absolute,
            Case::Escape,
            Case::ComponentSwap,
            Case::FinalSwap,
        ] {
            for iteration in 0..ITERATIONS {
                let fixture = Fixture::new()?;
                let observation = match run_case(&fixture, protocol, case) {
                    Ok(observation) => observation,
                    Err(error) if error.kind() == io::ErrorKind::Unsupported => {
                        Observation::unsupported(&error)
                    }
                    Err(error) => Observation::error(Outcome::UnexpectedError, &error),
                };
                println!(
                    "{{\"type\":\"case\",\"protocol\":{:?},\"case\":{:?},\"iteration\":{},\"outcome\":{:?},\"error_kind\":{},\"bytes\":{},\"hash\":{}}}",
                    protocol.name(),
                    case.name(),
                    iteration,
                    observation.outcome.name(),
                    json_string(observation.error_kind.as_deref()),
                    json_string(observation.bytes_label),
                    json_string(observation.hash.as_deref()),
                );
            }
        }
    }
    Ok(())
}

fn run_case(fixture: &Fixture, protocol: Protocol, case: Case) -> io::Result<Observation> {
    fixture.reset()?;
    let path = case.path(fixture);
    match protocol {
        Protocol::Baseline => run_baseline(fixture, case, &path),
        Protocol::Capability => run_capability(fixture, case, &path),
    }
}

fn run_baseline(fixture: &Fixture, case: Case, path: &Path) -> io::Result<Observation> {
    let relative = match normalize_relative(path) {
        Ok(relative) => relative,
        Err(error) => return Ok(Observation::error(Outcome::Rejected, &error)),
    };
    let absolute = fixture.root.join(relative);
    let canonical = match fs::canonicalize(absolute) {
        Ok(path) => path,
        Err(error) => {
            return Ok(Observation::error(Outcome::Rejected, &error));
        }
    };
    if !canonical.starts_with(&fixture.root) {
        return Ok(Observation::error(
            Outcome::Rejected,
            &io::Error::new(io::ErrorKind::PermissionDenied, "outside root"),
        ));
    }

    let attack = if case.needs_attack() {
        Some(attack_for(fixture, case))
    } else {
        None
    };
    let bytes = gated_read(attack, || fs::read(canonical))?;
    Ok(Observation::accepted(&bytes))
}

fn run_capability(fixture: &Fixture, case: Case, path: &Path) -> io::Result<Observation> {
    let relative = match normalize_relative(path) {
        Ok(relative) => relative,
        Err(error) => return Ok(Observation::error(Outcome::Rejected, &error)),
    };
    let root = Dir::open_ambient_dir(&fixture.root, ambient_authority())?;
    let pre = match root.symlink_metadata(&relative) {
        Ok(metadata) => metadata,
        Err(error) => return Ok(Observation::error(Outcome::Rejected, &error)),
    };
    if !pre.is_file() {
        return Ok(Observation::error(
            Outcome::Rejected,
            &io::Error::new(io::ErrorKind::InvalidData, "not a regular file"),
        ));
    }

    let attack = if case.needs_attack() {
        Some(attack_for(fixture, case))
    } else {
        None
    };
    let mut options = OpenOptions::new();
    options.read(true).follow(FollowSymlinks::No);
    let bytes = gated_read(attack, || {
        let mut file = root.open_with(&relative, &options)?;
        let mut bytes = Vec::new();
        file.by_ref()
            .take((MAX_BYTES + 1) as u64)
            .read_to_end(&mut bytes)?;
        let post = file.metadata()?;
        if bytes.len() > MAX_BYTES
            || pre.len() != post.len()
            || pre.dev() != post.dev()
            || pre.ino() != post.ino()
        {
            return Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "unstable snapshot",
            ));
        }
        Ok(bytes)
    });

    match bytes {
        Ok(bytes) => Ok(Observation::accepted(&bytes)),
        Err(error) if error.kind() == io::ErrorKind::Interrupted => {
            Ok(Observation::error(Outcome::UnstableSnapshot, &error))
        }
        Err(error) if is_rejection_error(&error) => {
            Ok(Observation::error(Outcome::Rejected, &error))
        }
        Err(error) => Ok(Observation::error(Outcome::UnexpectedError, &error)),
    }
}

fn is_rejection_error(error: &io::Error) -> bool {
    matches!(
        error.kind(),
        io::ErrorKind::PermissionDenied | io::ErrorKind::NotFound | io::ErrorKind::InvalidInput
    ) || format!("{:?}", error.kind()) == "FilesystemLoop"
}

fn normalize_relative(path: &Path) -> io::Result<PathBuf> {
    if path.is_absolute() {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "absolute path"));
    }
    let mut relative = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(value) => relative.push(value),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "path escapes root",
                ));
            }
        }
    }
    if relative.as_os_str().is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "empty relative path",
        ));
    }
    Ok(relative)
}

fn attack_for(fixture: &Fixture, case: Case) -> impl FnOnce() -> io::Result<()> + Send + 'static {
    let root = fixture.root.clone();
    let outside = fixture.outside.clone();
    move || match case {
        Case::ComponentSwap => {
            let original = root.join("swap");
            let parked = root.join("swap-original");
            fs::rename(original, parked)?;
            link_dir(&outside.join("swap"), &root.join("swap"))
        }
        Case::FinalSwap => {
            let original = root.join("final.txt");
            let parked = root.join("final-original.txt");
            fs::rename(original, parked)?;
            link_file(&outside.join("outside-final.txt"), &root.join("final.txt"))
        }
        _ => Ok(()),
    }
}

fn gated_read<A, F>(attack: Option<A>, read: F) -> io::Result<Vec<u8>>
where
    A: FnOnce() -> io::Result<()> + Send + 'static,
    F: FnOnce() -> io::Result<Vec<u8>>,
{
    let Some(attack) = attack else {
        return read();
    };
    let (ready_tx, ready_rx): (SyncSender<()>, Receiver<()>) = mpsc::sync_channel(0);
    let (done_tx, done_rx) = mpsc::sync_channel(0);
    let attacker = thread::spawn(move || {
        ready_rx.recv().expect("attack gate");
        let result = attack().map_err(|error| format!("{:?}: {}", error.kind(), error));
        done_tx.send(result).expect("attack result");
    });
    ready_tx.send(()).expect("attack ready");
    let attack_result = done_rx.recv().expect("attack result");
    let read_result = if let Err(message) = attack_result {
        Err(io::Error::new(io::ErrorKind::Unsupported, message))
    } else {
        read()
    };
    attacker.join().expect("attack thread");
    read_result
}

fn hash_bytes(bytes: &[u8]) -> String {
    let mut hasher = Hasher::new();
    hasher.update(bytes);
    hasher.finalize().to_hex().to_string()
}

fn json_string(value: Option<&str>) -> String {
    match value {
        Some(value) => format!("{:?}", value),
        None => "null".to_owned(),
    }
}

#[cfg(unix)]
fn link_dir(target: &Path, link: &Path) -> io::Result<()> {
    std::os::unix::fs::symlink(target, link)
}

#[cfg(windows)]
fn link_dir(target: &Path, link: &Path) -> io::Result<()> {
    std::os::windows::fs::symlink_dir(target, link)
}

#[cfg(unix)]
fn link_file(target: &Path, link: &Path) -> io::Result<()> {
    std::os::unix::fs::symlink(target, link)
}

#[cfg(windows)]
fn link_file(target: &Path, link: &Path) -> io::Result<()> {
    std::os::windows::fs::symlink_file(target, link)
}
