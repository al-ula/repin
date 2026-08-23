set shell := ["bash", "-eu", "-o", "pipefail", "-c"]

# Optional provenance: plain Cargo builds remain valid without Git metadata.
git_commit := `git rev-parse --verify HEAD 2>/dev/null || true`
host_target := `rustc -vV | sed -n 's/^host: //p'`
release_target := env_var_or_default("CARGO_BUILD_TARGET", host_target)
target_dir := env_var_or_default("CARGO_TARGET_DIR", "target")

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
    REPIN_GIT_COMMIT={{git_commit}} cargo build --workspace --release --target "{{release_target}}"

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
    binary_path="{{target_dir}}/{{release_target}}/release/repin"
    if [ ! -f "${binary_path}" ]; then
        echo "Release binary not found for target {{release_target}}: ${binary_path}" >&2
        exit 1
    fi
    staging_dir="$(mktemp -d)"
    trap 'rm -rf "${staging_dir}"' EXIT
    cp "${binary_path}" "${staging_dir}/repin"
    cp -r docs/usage "${staging_dir}/docs"
    tar -czf "repin-${tag}-{{release_target}}.tar.gz" -C "${staging_dir}" repin docs
