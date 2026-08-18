#!/usr/bin/env bash
set -euo pipefail

tool_root="${1:?usage: install_q_tools.sh TOOL_ROOT}"

cargo install --locked --root "$tool_root" --version 0.20.2 cargo-deny
cargo install --locked --root "$tool_root" --version 0.22.2 cargo-audit
cargo install --locked --root "$tool_root" --version 0.10.0 cargo-sbom
cargo install --locked --root "$tool_root" --version 0.7.5 cargo-auditable
