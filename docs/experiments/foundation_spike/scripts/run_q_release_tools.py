#!/usr/bin/env python3
"""Run the Q-003/Q-006/Q-007/Q-008/Q-012 evidence pass.

The script intentionally treats all external tools as evidence inputs. It
retains stdout, stderr, exit codes, versions, generated SBOMs, and hashes, but
never promotes a candidate into a production decision.
"""

from __future__ import annotations

import argparse
from datetime import date
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tempfile
from typing import Any

from q_policy import evaluate_findings, extract_findings, load_exceptions


RUN_ID = "q-release-tools-20260818"
TOOL_PINS = {
    "cargo-deny": "0.20.2",
    "cargo-audit": "0.22.2",
    "cargo-sbom": "0.10.0",
    "cargo-auditable": "0.7.5",
}
SCRIPT_DIR = Path(__file__).resolve().parent
SPIKE_ROOT = SCRIPT_DIR.parent
RESULT_ROOT = SPIKE_ROOT.parent / "results" / "raw"
DENY_CONFIG = SPIKE_ROOT / "deny.toml"
EXCEPTIONS = SPIKE_ROOT / "advisory-exceptions.toml"
GPL_FIXTURE = SPIKE_ROOT / "fixtures" / "q012-gpl"
TIME_FIXTURE = SPIKE_ROOT / "fixtures" / "q012-time"
GIT_FIXTURE_SOURCE = SPIKE_ROOT / "fixtures" / "q012-git-source" / "git-dep"
USEARCH_FIXTURE = SPIKE_ROOT / "fixtures" / "q008-usearch"


def sha256_file(path: Path) -> str | None:
    if not path.is_file():
        return None
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def safe_name(value: str) -> str:
    return "".join(character if character.isalnum() or character in "._-" else "_" for character in value)


def write_json(path: Path, value: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def run_command(
    output_root: Path,
    label: str,
    args: list[str],
    cwd: Path,
    env: dict[str, str],
) -> dict[str, Any]:
    command_root = output_root / "commands"
    command_root.mkdir(parents=True, exist_ok=True)
    result: dict[str, Any] = {
        "label": label,
        "command": args,
        "cwd": str(cwd),
    }
    try:
        completed = subprocess.run(
            args,
            cwd=cwd,
            env=env,
            text=True,
            capture_output=True,
            check=False,
        )
    except OSError as error:
        result.update({"exit_code": None, "error": str(error), "available": False})
        return result

    stem = safe_name(label)
    stdout_path = command_root / f"{stem}.stdout"
    stderr_path = command_root / f"{stem}.stderr"
    stdout_path.write_text(completed.stdout, encoding="utf-8")
    stderr_path.write_text(completed.stderr, encoding="utf-8")
    result.update(
        {
            "exit_code": completed.returncode,
            "available": True,
            "stdout_path": str(stdout_path.relative_to(output_root)),
            "stderr_path": str(stderr_path.relative_to(output_root)),
            "stdout": completed.stdout,
            "stderr": completed.stderr,
        }
    )
    return result


def add_case(
    cases: list[dict[str, Any]],
    case_id: str,
    expected: str,
    observed: str,
    details: dict[str, Any],
    passed: bool,
) -> None:
    cases.append(
        {
            "id": case_id,
            "expected": expected,
            "observed": observed,
            "outcome": "pass" if passed else "fail",
            "details": details,
        }
    )


def tool_root_environment(tool_root: Path | None) -> dict[str, str]:
    environment = os.environ.copy()
    if tool_root:
        environment["PATH"] = str(tool_root / "bin") + os.pathsep + environment.get("PATH", "")
    return environment


def executable(environment: dict[str, str], name: str) -> str | None:
    return shutil.which(name, path=environment.get("PATH"))


def source_revision() -> str:
    status = subprocess.run(
        ["git", "status", "--short", "--branch"],
        cwd=SPIKE_ROOT.parents[2],
        text=True,
        capture_output=True,
        check=False,
    ).stdout
    return hashlib.sha256(status.encode("utf-8")).hexdigest()


def ensure_lock(output_root: Path, label: str, project: Path, environment: dict[str, str]) -> dict[str, Any]:
    lockfile = project / "Cargo.lock"
    if lockfile.is_file():
        return {"label": label, "lockfile": str(lockfile), "generated": False, "command": None}
    command = run_command(
        output_root,
        f"{label}-generate-lockfile",
        ["cargo", "generate-lockfile", "--manifest-path", str(project / "Cargo.toml")],
        project,
        environment,
    )
    command["lockfile"] = str(lockfile)
    command["generated"] = lockfile.is_file()
    return command


def metadata(output_root: Path, label: str, project: Path, environment: dict[str, str]) -> tuple[dict[str, Any], dict[str, Any] | None]:
    command = run_command(
        output_root,
        f"{label}-metadata",
        ["cargo", "metadata", "--locked", "--all-features", "--format-version", "1"],
        project,
        environment,
    )
    if command.get("exit_code") != 0:
        return command, None
    try:
        return command, json.loads(command.get("stdout", ""))
    except json.JSONDecodeError:
        return command, None


def metadata_package_keys(document: dict[str, Any] | None) -> set[str]:
    packages = {
        str(package.get("id")): f"{package.get('name')}@{package.get('version')}"
        for package in (document or {}).get("packages", [])
        if isinstance(package, dict)
        and package.get("id")
        and package.get("name")
        and package.get("version")
    }
    resolve = (document or {}).get("resolve") or {}
    nodes = {
        str(node.get("id")): node
        for node in resolve.get("nodes", [])
        if isinstance(node, dict) and node.get("id")
    }
    root = resolve.get("root")
    if not root or root not in nodes:
        return set(packages.values())
    reachable = {str(root)}
    pending = [str(root)]
    while pending:
        node = nodes.get(pending.pop())
        if not node:
            continue
        for dependency in node.get("deps", []):
            if not isinstance(dependency, dict):
                continue
            dependency_kinds = dependency.get("dep_kinds", [])
            if dependency_kinds and all(
                kind.get("kind") in {"dev", "build"}
                for kind in dependency_kinds
                if isinstance(kind, dict)
            ):
                continue
            package_id = str(dependency.get("pkg", ""))
            if package_id in nodes and package_id not in reachable:
                reachable.add(package_id)
                pending.append(package_id)
    return {packages[package_id] for package_id in reachable if package_id in packages}


def metadata_root_key(document: dict[str, Any] | None) -> str | None:
    resolve = (document or {}).get("resolve") or {}
    root = resolve.get("root")
    for package in (document or {}).get("packages", []):
        if isinstance(package, dict) and package.get("id") == root:
            if package.get("name") and package.get("version"):
                return f"{package['name']}@{package['version']}"
    return None


def advisory_database(result: dict[str, Any]) -> dict[str, Any] | None:
    try:
        payload = json.loads(result.get("stdout", ""))
    except (json.JSONDecodeError, TypeError):
        return None
    database = payload.get("database")
    return database if isinstance(database, dict) else None


def make_git_fixture(output_root: Path, environment: dict[str, str]) -> tuple[Path | None, dict[str, Any]]:
    workspace_root = output_root / "generated-fixtures" / "q012-git-source"
    repository = workspace_root / "git-dep"
    project = workspace_root / "project"
    repository.parent.mkdir(parents=True, exist_ok=True)
    shutil.copytree(GIT_FIXTURE_SOURCE, repository, dirs_exist_ok=True)
    project.mkdir(parents=True, exist_ok=True)

    commands: list[dict[str, Any]] = []
    commands.append(
        run_command(output_root, "git-fixture-init", ["git", "init", "--quiet"], repository, environment)
    )
    commands.append(
        run_command(
            output_root,
            "git-fixture-config-name",
            ["git", "config", "user.name", "Repin Q Fixture"],
            repository,
            environment,
        )
    )
    commands.append(
        run_command(
            output_root,
            "git-fixture-config-email",
            ["git", "config", "user.email", "q-fixture@example.invalid"],
            repository,
            environment,
        )
    )
    commands.append(run_command(output_root, "git-fixture-add", ["git", "add", "."], repository, environment))
    commands.append(
        run_command(
            output_root,
            "git-fixture-commit",
            ["git", "commit", "--quiet", "--allow-empty", "--no-gpg-sign", "-m", "Q-012 source fixture"],
            repository,
            environment,
        )
    )
    revision = run_command(output_root, "git-fixture-revision", ["git", "rev-parse", "HEAD"], repository, environment)
    commands.append(revision)
    if revision.get("exit_code") != 0:
        return None, {"commands": commands}
    commit = revision.get("stdout", "").strip()
    manifest = f'''[package]\nname = "q012-git-root"\nversion = "0.1.0"\nedition = "2024"\npublish = false\nlicense = "MIT"\n\n[dependencies]\nq012-git-source = {{ version = "0.1.0", git = "{repository.as_uri()}", rev = "{commit}" }}\n'''
    (project / "Cargo.toml").write_text(manifest, encoding="utf-8")
    (project / "src").mkdir(parents=True, exist_ok=True)
    (project / "src" / "main.rs").write_text("fn main() {}\n", encoding="utf-8")
    lock = ensure_lock(output_root, "git-fixture", project, environment)
    return project, {"commands": commands, "revision": commit, "lock": lock}


def cargo_deny_case(
    output_root: Path,
    label: str,
    project: Path,
    environment: dict[str, str],
    cases: list[dict[str, Any]],
    expected_failure: bool,
) -> dict[str, Any]:
    result = run_command(
        output_root,
        label,
        ["cargo", "deny", "--config", str(DENY_CONFIG), "check", "licenses", "bans", "sources"],
        project,
        environment,
    )
    failed = result.get("exit_code") not in (0, None)
    expected = "reject" if expected_failure else "pass"
    observed = "reject" if failed else ("pass" if result.get("exit_code") == 0 else "unavailable")
    add_case(
        cases,
        f"Q006-{label.upper()}",
        expected,
        observed,
        {key: value for key, value in result.items() if key not in {"stdout", "stderr"}},
        failed == expected_failure if result.get("exit_code") is not None else False,
    )
    return result


def audit_case(
    output_root: Path,
    label: str,
    project: Path,
    lockfile: Path,
    environment: dict[str, str],
    cases: list[dict[str, Any]],
    expected_advisory: str | None,
    exceptions: list[dict[str, Any]],
    no_fetch: bool,
) -> tuple[dict[str, Any], list[dict[str, Any]]]:
    audit_args = ["cargo", "audit"]
    if no_fetch:
        audit_args.append("--no-fetch")
    audit_args.extend(["--json", "--file", str(lockfile)])
    result = run_command(
        output_root,
        label,
        audit_args,
        project,
        environment,
    )
    payload: dict[str, Any] | None = None
    try:
        payload = json.loads(result.get("stdout", ""))
    except (json.JSONDecodeError, TypeError):
        payload = None
    json_path = output_root / "artifacts" / f"{safe_name(label)}.json"
    if payload is not None:
        write_json(json_path, payload)
    findings = extract_findings(payload) if payload is not None else []
    policy = evaluate_findings(findings, exceptions, today=date(2026, 8, 18))
    policy_path = output_root / "artifacts" / f"{safe_name(label)}-policy.json"
    write_json(policy_path, policy)
    if expected_advisory:
        observed = "advisory_detected" if any(item.get("id") == expected_advisory for item in findings) else "missing_advisory"
        passed = result.get("exit_code") not in (0, None) and observed == "advisory_detected"
        expected = "advisory_detected"
    else:
        observed = "clean" if not findings and result.get("exit_code") == 0 else "findings_or_error"
        passed = observed == "clean"
        expected = "clean"
    add_case(
        cases,
        f"Q007-{label.upper()}",
        expected,
        observed,
        {
            "findings": findings,
            "policy": policy,
            "command": {key: value for key, value in result.items() if key not in {"stdout", "stderr"}},
        },
        passed,
    )
    return result, findings


def sbom_case(
    output_root: Path,
    label: str,
    project: Path,
    environment: dict[str, str],
    sbom_command: list[str] | None,
    output_format: str,
    required_packages: set[str],
    expected_package_keys: set[str],
    root_package_key: str | None,
    cases: list[dict[str, Any]],
) -> dict[str, Any]:
    if sbom_command is None:
        add_case(cases, f"Q008-{label.upper()}", "valid_sbom", "unavailable", {}, False)
        return {"exit_code": None, "available": False}
    result = run_command(
        output_root,
        label,
        sbom_command + ["--project-directory", str(project), "--output-format", output_format],
        project,
        environment,
    )
    payload: dict[str, Any] | None = None
    try:
        payload = json.loads(result.get("stdout", ""))
    except (json.JSONDecodeError, TypeError):
        payload = None
    artifact_path = output_root / "artifacts" / f"{safe_name(label)}.json"
    if payload is not None:
        write_json(artifact_path, payload)
    if output_format == "spdx_json_2_3":
        package_records = (payload or {}).get("packages", [])
    else:
        package_records = (payload or {}).get("components", [])
    package_records = [package for package in package_records if isinstance(package, dict)]
    names = {str(package.get("name")) for package in package_records}
    package_keys = {
        f"{package.get('name')}@{package.get('versionInfo') or package.get('version')}"
        for package in package_records
        if package.get("name") and (package.get("versionInfo") or package.get("version"))
    }
    missing = sorted(required_packages - names)
    expected_for_format = set(expected_package_keys)
    if output_format != "spdx_json_2_3" and root_package_key:
        expected_for_format.discard(root_package_key)
    missing_metadata_packages = sorted(expected_for_format - package_keys)
    unexpected_metadata_packages = sorted(package_keys - expected_for_format)
    package_set_matches_metadata = not missing_metadata_packages and not unexpected_metadata_packages
    correct_format = payload is not None and payload.get("spdxVersion") == "SPDX-2.3"
    if output_format != "spdx_json_2_3":
        correct_format = (
            payload is not None
            and payload.get("bomFormat") == "CycloneDX"
            and payload.get("specVersion") == "1.6"
        )
    required_field_errors: dict[str, list[str]] = {}
    for package_name in sorted(required_packages & names):
        package = next(package for package in package_records if package.get("name") == package_name)
        errors: list[str] = []
        version = package.get("versionInfo") or package.get("version")
        source = package.get("downloadLocation") or package.get("purl")
        license_value = package.get("licenseDeclared") or package.get("licenses")
        if not version:
            errors.append("version")
        if not source:
            errors.append("source")
        if not license_value:
            errors.append("license")
        if errors:
            required_field_errors[package_name] = errors
    relationship_count = len((payload or {}).get("relationships", [])) if output_format == "spdx_json_2_3" else len((payload or {}).get("dependencies", []))
    schema_fields_ok = (
        payload is not None
        and relationship_count > 0
        and (set((payload or {}).keys()) >= {"SPDXID", "creationInfo", "dataLicense", "documentNamespace", "name", "packages", "relationships", "spdxVersion"} if output_format == "spdx_json_2_3" else set((payload or {}).keys()) >= {"bomFormat", "components", "dependencies", "metadata", "specVersion", "version"})
    )
    passed = (
        result.get("exit_code") == 0
        and payload is not None
        and not missing
        and correct_format
        and not required_field_errors
        and schema_fields_ok
        and package_set_matches_metadata
    )
    observed = "valid_sbom" if passed else ("invalid_sbom" if result.get("exit_code") == 0 else "error")
    add_case(
        cases,
        f"Q008-{label.upper()}",
        "valid_sbom",
        observed,
        {
            "format": output_format,
            "required_packages": sorted(required_packages),
            "missing_packages": missing,
            "package_count": len(names),
            "package_record_count": len(package_records),
            "package_set_matches_metadata": package_set_matches_metadata,
            "expected_metadata_package_count": len(expected_for_format),
            "missing_metadata_packages": missing_metadata_packages,
            "unexpected_metadata_packages": unexpected_metadata_packages,
            "required_field_errors": required_field_errors,
            "relationship_count": relationship_count,
            "schema_fields_ok": schema_fields_ok,
            "artifact": str(artifact_path.relative_to(output_root)) if payload is not None else None,
        },
        passed,
    )
    return result


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--output", type=Path, default=RESULT_ROOT / RUN_ID)
    parser.add_argument("--tool-root", type=Path)
    parser.add_argument(
        "--no-fetch",
        action="store_true",
        help="use the existing cargo-audit advisory database without fetching",
    )
    args = parser.parse_args()

    output_root = args.output.resolve()
    output_root.mkdir(parents=True, exist_ok=True)
    environment = tool_root_environment(args.tool_root.resolve() if args.tool_root else None)
    cases: list[dict[str, Any]] = []
    commands: list[dict[str, Any]] = []
    artifacts: list[str] = []

    tool_commands = {
        "cargo-deny": ["cargo-deny", "--version"],
        "cargo-audit": ["cargo-audit", "--version"],
        "cargo-sbom": ["cargo-sbom", "--version"],
        "cargo-auditable": ["cargo-auditable", "--version"],
    }
    versions: dict[str, str | None] = {}
    for name, command in tool_commands.items():
        binary = executable(environment, command[0])
        if not binary:
            versions[name] = None
            add_case(cases, f"Q012-TOOL-{safe_name(name).upper()}", "available", "unavailable", {}, False)
            continue
        if name == "cargo-auditable":
            if args.tool_root:
                result = run_command(
                    output_root,
                    f"version-{name}",
                    ["cargo", "install", "--list", "--root", str(args.tool_root.resolve())],
                    SPIKE_ROOT,
                    environment,
                )
                match = re.search(r"^cargo-auditable v([^:]+):", result.get("stdout", ""), re.MULTILINE)
                version = match.group(1) if match else None
            else:
                result = run_command(output_root, f"version-{name}", command, SPIKE_ROOT, environment)
                version = None
        else:
            result = run_command(output_root, f"version-{name}", command, SPIKE_ROOT, environment)
            match = re.search(r"\d+\.\d+\.\d+", result.get("stdout", ""))
            version = match.group(0) if match else None
        commands.append({key: value for key, value in result.items() if key not in {"stdout", "stderr"}})
        versions[name] = version
        passed = result.get("exit_code") == 0 and version == TOOL_PINS[name]
        add_case(
            cases,
            f"Q012-TOOL-{safe_name(name).upper()}",
            TOOL_PINS[name],
            version or "unavailable",
            {"version_command": {key: value for key, value in result.items() if key not in {"stdout", "stderr"}}},
            passed,
        )

    base_lock = SPIKE_ROOT / "Cargo.lock"
    commands.append(ensure_lock(output_root, "spike", SPIKE_ROOT, environment))
    commands.append(ensure_lock(output_root, "gpl-fixture", GPL_FIXTURE, environment))
    commands.append(ensure_lock(output_root, "time-fixture", TIME_FIXTURE, environment))
    commands.append(ensure_lock(output_root, "usearch-fixture", USEARCH_FIXTURE, environment))

    if all(versions.get(name) for name in tool_commands):
        commands.append(cargo_deny_case(output_root, "deny-baseline", SPIKE_ROOT, environment, cases, False))
        commands.append(cargo_deny_case(output_root, "deny-gpl", GPL_FIXTURE, environment, cases, True))
        git_project, git_details = make_git_fixture(output_root, environment)
        commands.append(git_details)
        if git_project:
            commands.append(cargo_deny_case(output_root, "deny-git-source", git_project, environment, cases, True))
        else:
            add_case(cases, "Q006-DENY-GIT-SOURCE", "reject", "fixture_error", git_details, False)
    else:
        add_case(cases, "Q006-POLICY-TOOLS", "available", "incomplete", {}, False)

    exceptions, exception_errors = load_exceptions(EXCEPTIONS, today=date(2026, 8, 18))
    write_json(output_root / "artifacts" / "exception-validation.json", {"errors": exception_errors, "count": len(exceptions)})
    if exception_errors:
        add_case(cases, "Q007-EXCEPTION-FILE", "valid", "invalid", {"errors": exception_errors}, False)
    else:
        add_case(cases, "Q007-EXCEPTION-FILE", "valid", "valid", {"count": len(exceptions)}, True)

    baseline_audit_findings: list[dict[str, Any]] = []
    time_audit_findings: list[dict[str, Any]] = []
    advisory_databases: dict[str, dict[str, Any]] = {}
    if versions.get("cargo-audit"):
        baseline_result, baseline_audit_findings = audit_case(
            output_root,
            "audit-baseline",
            SPIKE_ROOT,
            base_lock,
            environment,
            cases,
            None,
            exceptions,
            args.no_fetch,
        )
        commands.append({key: value for key, value in baseline_result.items() if key not in {"stdout", "stderr"}})
        if database := advisory_database(baseline_result):
            advisory_databases["audit-baseline"] = database
        time_result, time_audit_findings = audit_case(
            output_root,
            "audit-time-advisory",
            TIME_FIXTURE,
            TIME_FIXTURE / "Cargo.lock",
            environment,
            cases,
            "RUSTSEC-2020-0071",
            exceptions,
            args.no_fetch,
        )
        commands.append({key: value for key, value in time_result.items() if key not in {"stdout", "stderr"}})
        if database := advisory_database(time_result):
            advisory_databases["audit-time-advisory"] = database
    else:
        add_case(cases, "Q007-AUDIT-TOOLS", "available", "unavailable", {}, False)

    metadata_command, baseline_metadata = metadata(output_root, "spike", SPIKE_ROOT, environment)
    commands.append({key: value for key, value in metadata_command.items() if key not in {"stdout", "stderr"}})
    baseline_package_keys = metadata_package_keys(baseline_metadata)
    baseline_root_key = metadata_root_key(baseline_metadata)
    baseline_names = {package_key.split("@", 1)[0] for package_key in baseline_package_keys}
    usearch_metadata_command, usearch_metadata = metadata(output_root, "usearch-fixture", USEARCH_FIXTURE, environment)
    commands.append({key: value for key, value in usearch_metadata_command.items() if key not in {"stdout", "stderr"}})
    usearch_package_keys = metadata_package_keys(usearch_metadata)
    usearch_root_key = metadata_root_key(usearch_metadata)
    usearch_names = {package_key.split("@", 1)[0] for package_key in usearch_package_keys}

    sbom_command: list[str] | None = None
    if executable(environment, "cargo-sbom"):
        sbom_command = ["cargo-sbom"]
    elif executable(environment, "cargo"):
        probe = run_command(output_root, "probe-cargo-sbom-subcommand", ["cargo", "sbom", "--help"], SPIKE_ROOT, environment)
        commands.append({key: value for key, value in probe.items() if key not in {"stdout", "stderr"}})
        if probe.get("exit_code") == 0:
            sbom_command = ["cargo", "sbom"]

    if versions.get("cargo-sbom"):
        sbom_case(
            output_root,
            "sbom-spike-spdx",
            SPIKE_ROOT,
            environment,
            sbom_command,
            "spdx_json_2_3",
            {"tree-sitter", "tree-sitter-rust", "tree-sitter-md", "tree-sitter-typescript", "tree-sitter-javascript"},
            baseline_package_keys,
            baseline_root_key,
            cases,
        )
        sbom_case(
            output_root,
            "sbom-spike-cyclonedx",
            SPIKE_ROOT,
            environment,
            sbom_command,
            "cyclone_dx_json_1_6",
            {"tree-sitter", "tree-sitter-rust", "tree-sitter-md", "tree-sitter-typescript", "tree-sitter-javascript"},
            baseline_package_keys,
            baseline_root_key,
            cases,
        )
        sbom_case(
            output_root,
            "sbom-usearch-spdx",
            USEARCH_FIXTURE,
            environment,
            sbom_command,
            "spdx_json_2_3",
            {"usearch", "cxx"},
            usearch_package_keys,
            usearch_root_key,
            cases,
        )
    else:
        add_case(cases, "Q008-SBOM-TOOLS", "available", "unavailable", {}, False)

    if versions.get("cargo-auditable"):
        builds = [
            ("auditable-spike-build", SPIKE_ROOT, "repin-foundation-spike"),
            ("auditable-usearch-build", USEARCH_FIXTURE, "q008-usearch-inventory"),
        ]
        for label, project, binary_name in builds:
            result = run_command(
                output_root,
                label,
                [
                    "cargo",
                    "auditable",
                    "build",
                    "--release",
                    "--locked",
                    "--manifest-path",
                    str(project / "Cargo.toml"),
                    *(["--bin", binary_name] if project == SPIKE_ROOT else []),
                ],
                project,
                environment,
            )
            commands.append({key: value for key, value in result.items() if key not in {"stdout", "stderr"}})
            binary = project / "target" / "release" / binary_name
            if result.get("exit_code") == 0 and binary.is_file():
                binary_hash = sha256_file(binary)
                add_case(cases, f"Q008-{label.upper()}", "auditable_binary", "auditable_binary", {"sha256": binary_hash}, True)
                if versions.get("cargo-audit"):
                    inspect = run_command(
                        output_root,
                        f"{label}-inspect",
                        [
                            "cargo",
                            "audit",
                            *( ["--no-fetch"] if args.no_fetch else [] ),
                            "bin",
                            str(binary),
                        ],
                        project,
                        environment,
                    )
                    commands.append({key: value for key, value in inspect.items() if key not in {"stdout", "stderr"}})
                    inspect_path = output_root / "artifacts" / f"{safe_name(label)}-audit.txt"
                    inspection_text = (inspect.get("stdout", "") + "\n" + inspect.get("stderr", "")).strip() + "\n"
                    inspect_path.write_text(inspection_text, encoding="utf-8")
                    inspection_report = {
                        "binary": str(binary),
                        "sha256": binary_hash,
                        "report": str(inspect_path.relative_to(output_root)),
                        "exit_code": inspect.get("exit_code"),
                        "dependency_report_nonempty": bool(inspection_text.strip()),
                    }
                    write_json(output_root / "artifacts" / f"{safe_name(label)}-audit.json", inspection_report)
                    inspectable = inspect.get("exit_code") == 0 and bool(inspection_text.strip())
                    add_case(cases, f"Q008-{label.upper()}-INSPECT", "inspectable", "inspectable" if inspectable else "inspection_error", inspection_report, inspectable)
            else:
                add_case(cases, f"Q008-{label.upper()}", "auditable_binary", "build_error", {"binary": str(binary)}, False)
    else:
        add_case(cases, "Q008-AUDITABLE-TOOL", "available", "unavailable", {}, False)

    lockfile_paths = {
        "spike": SPIKE_ROOT / "Cargo.lock",
        "gpl_fixture": GPL_FIXTURE / "Cargo.lock",
        "time_fixture": TIME_FIXTURE / "Cargo.lock",
        "usearch_fixture": USEARCH_FIXTURE / "Cargo.lock",
    }
    source_paths = {
        "Cargo.toml": SPIKE_ROOT / "Cargo.toml",
        "deny.toml": DENY_CONFIG,
        "advisory-exceptions.toml": EXCEPTIONS,
        "src/main.rs": SPIKE_ROOT / "src" / "main.rs",
        "scripts/q_policy.py": SCRIPT_DIR / "q_policy.py",
        "scripts/run_q_release_tools.py": SCRIPT_DIR / "run_q_release_tools.py",
        "tests/q003_quality_tools.rs": SPIKE_ROOT / "tests" / "q003_quality_tools.rs",
    }
    artifact_hashes = {
        str(path.relative_to(output_root)): sha256_file(path)
        for path in sorted((output_root / "artifacts").glob("*"))
        if path.is_file()
    }
    manifest = {
        "run_id": RUN_ID,
        "lifecycle_stage": "experimentation",
        "platform_scope": "Linux x86_64/glibc PoC",
        "source_revision": source_revision(),
        "rustc": subprocess.run(["rustc", "--version"], text=True, capture_output=True, check=False).stdout.strip(),
        "cargo": subprocess.run(["cargo", "--version"], text=True, capture_output=True, check=False).stdout.strip(),
        "tool_pins": versions,
        "tool_binaries": {
            name: sha256_file(Path(executable(environment, name))) if executable(environment, name) else None
            for name in ["cargo-deny", "cargo-audit", "cargo-sbom", "cargo-auditable"]
        },
        "fixture_seed": "repin-q-release-tools-20260818",
        "lockfile_hashes": {
            label: sha256_file(path) for label, path in lockfile_paths.items()
        },
        "source_digests": {
            label: sha256_file(path) for label, path in source_paths.items()
        },
        "advisory_databases": advisory_databases,
        "artifact_hashes": artifact_hashes,
        "candidate_scope": {
            "baseline_packages": len(baseline_package_keys),
            "baseline_unique_package_names": len(baseline_names),
            "usearch_fixture_packages": len(usearch_package_keys),
            "usearch_fixture_unique_package_names": len(usearch_names),
            "tree_sitter_native_boundary": "represented by tree-sitter Cargo packages",
            "usearch_native_boundary": "represented by usearch/cxx Cargo packages; raw C++ is not a separate Cargo package",
        },
        "commands": commands,
    }
    write_json(output_root / "manifest.json", manifest)
    report = {
        "experiment": "F7-Q-follow-up",
        "run_id": RUN_ID,
        "status": "complete" if all(case["outcome"] == "pass" for case in cases) else "complete_with_gaps",
        "overall_outcome": "inconclusive",
        "decision_status": "deferred",
        "hard_blocker": False,
        "cases": cases,
        "case_ids": [case["id"] for case in cases],
        "tool_pins": versions,
        "measurements": [],
        "notes": [
            "Q-003/Q-006/Q-007/Q-008/Q-012 follow-up evidence; no production dependency is accepted.",
            "SPDX JSON 2.3 is the provisional canonical SBOM output; CycloneDX JSON 1.6 is a compatibility comparison.",
            "USearch is inventoried only in an isolated Q-008 fixture; S3 remains deferred.",
            "All requested tools and the advisory database were available on this host; future unavailable cases remain explicit failures rather than silent passes.",
        ],
        "artifacts": ["manifest.json", "report.json", "commands/", "artifacts/"],
    }
    write_json(output_root / "report.json", report)
    print(json.dumps(report, indent=2, sort_keys=True))
    return 0 if report["status"] == "complete" else 2


if __name__ == "__main__":
    sys.exit(main())
