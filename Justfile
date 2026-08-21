set shell := ["bash", "-eu", "-o", "pipefail", "-c"]

# Optional provenance: plain Cargo builds remain valid without Git metadata.
git_commit := `git rev-parse --verify HEAD 2>/dev/null || true`

default: build

build:
    REPIN_GIT_COMMIT={{git_commit}} cargo build --workspace

fmt:
    cargo fmt --all -- --check

check:
    REPIN_GIT_COMMIT={{git_commit}} cargo check --workspace

clippy:
    REPIN_GIT_COMMIT={{git_commit}} cargo clippy --workspace --all-targets --all-features -- -D warnings

test:
    REPIN_GIT_COMMIT={{git_commit}} cargo test --workspace --all-targets --all-features

doc-test:
    REPIN_GIT_COMMIT={{git_commit}} cargo test --workspace --doc

release:
    REPIN_GIT_COMMIT={{git_commit}} cargo build --workspace --release

version:
    REPIN_GIT_COMMIT={{git_commit}} cargo run -p repin-cli -- version --json

# Package the release binary into a versioned tarball.
dist: release
    #= package release artifacts
    tar -czf "repin-${GITHUB_REF_NAME:-$(git describe --tags --always)}-x86_64-unknown-linux-gnu.tar.gz" -C target/release repin
