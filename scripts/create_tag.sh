#!/usr/bin/env bash

set -euo pipefail

repo_root="$(git rev-parse --show-toplevel)"
cd "$repo_root"

package_version="$({
    cargo metadata --no-deps --format-version 1
} | python3 -c '
import json
import sys

packages = [
    package
    for package in json.load(sys.stdin)["packages"]
    if package["name"] == "repin-cli"
]
if len(packages) != 1:
    raise SystemExit(f"expected one repin-cli package, found {len(packages)}")
print(packages[0]["version"])
')"
tag="v${package_version}"

if git show-ref --tags --verify --quiet "refs/tags/${tag}"; then
    printf 'tag already exists: %s\n' "$tag" >&2
    exit 1
fi

git tag --annotate "$tag" --message "Release $tag"
printf 'created tag: %s\n' "$tag"
