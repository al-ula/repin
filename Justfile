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
    REPIN_GIT_COMMIT={{git_commit}} cargo run -p repin -- version --json

docs:
    mdbook build docs/code
    mdbook test docs/code
    mdbook build docs/usage
    mdbook test docs/usage
    mkdir -p book
    cp docs/index.html book/index.html

# Package the release binary and raw usage documentation into a versioned tarball.
dist: release
    #!/usr/bin/env bash
    set -euo pipefail
    tag="${GITHUB_REF_NAME:-$(git describe --tags --always)}"
    staging_dir="$(mktemp -d)"
    trap 'rm -rf "${staging_dir}"' EXIT
    cp target/release/repin "${staging_dir}/repin"
    cp -r docs/usage "${staging_dir}/docs"
    tar -czf "repin-${tag}-x86_64-unknown-linux-gnu.tar.gz" -C "${staging_dir}" repin docs
