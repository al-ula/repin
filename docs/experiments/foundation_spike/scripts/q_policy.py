"""Q-007 advisory policy evaluation helpers.

The release tools remain evidence inputs. This module owns only the explicit
policy gate used to classify their findings; it does not accept a dependency
or suppress a tool report.
"""

from __future__ import annotations

from datetime import date, timedelta
from pathlib import Path
import tomllib
from typing import Any, Iterable

BLOCKING_KINDS = frozenset({"vulnerability", "unsound"})
WARNING_KINDS = frozenset({"unmaintained", "notice"})
REQUIRED_EXCEPTION_FIELDS = (
    "id",
    "package",
    "version",
    "owner",
    "rationale",
    "remediation_issue",
    "compensating_control",
    "created",
    "expires",
)


def parse_date(value: Any, field: str) -> date:
    if not isinstance(value, str):
        raise ValueError(f"{field} must be an ISO date")
    try:
        return date.fromisoformat(value)
    except ValueError as error:
        raise ValueError(f"{field} must be an ISO date") from error


def validate_exception(entry: dict[str, Any], today: date | None = None) -> list[str]:
    """Return all validation errors for one temporary advisory exception."""

    today = today or date.today()
    errors: list[str] = []
    for field in REQUIRED_EXCEPTION_FIELDS:
        value = entry.get(field)
        if not isinstance(value, str) or not value.strip():
            errors.append(f"missing {field}")

    advisory_id = entry.get("id", "")
    package = entry.get("package", "")
    version = entry.get("version", "")
    if isinstance(advisory_id, str) and (
        not advisory_id.startswith("RUSTSEC-") or "*" in advisory_id
    ):
        errors.append("id must be an exact RUSTSEC identifier")
    if isinstance(package, str) and (not package or "*" in package):
        errors.append("package must be exact and non-wildcard")
    if isinstance(version, str) and (not version or "*" in version):
        errors.append("version must be exact and non-wildcard")

    try:
        created = parse_date(entry.get("created"), "created")
        expires = parse_date(entry.get("expires"), "expires")
        if expires <= created:
            errors.append("expires must be after created")
        if expires - created > timedelta(days=30):
            errors.append("exception duration must be at most 30 days")
        if created > today:
            errors.append("created cannot be in the future")
        if expires <= today:
            errors.append("exception is expired")
    except ValueError as error:
        errors.append(str(error))

    return errors


def load_exceptions(path: Path, today: date | None = None) -> tuple[list[dict[str, Any]], list[str]]:
    """Load and validate the documented exception file."""

    if not path.is_file():
        return [], [f"exception file does not exist: {path}"]
    try:
        document = tomllib.loads(path.read_text(encoding="utf-8"))
    except (OSError, tomllib.TOMLDecodeError) as error:
        return [], [f"cannot read exception file: {error}"]
    entries = document.get("exceptions", [])
    if not isinstance(entries, list):
        return [], ["exceptions must be an array of tables"]
    errors: list[str] = []
    valid: list[dict[str, Any]] = []
    for index, entry in enumerate(entries):
        if not isinstance(entry, dict):
            errors.append(f"exceptions[{index}] must be a table")
            continue
        entry_errors = validate_exception(entry, today=today)
        if entry_errors:
            errors.extend(f"exceptions[{index}]: {error}" for error in entry_errors)
        else:
            valid.append(entry)
    return valid, errors


def _finding_key(finding: dict[str, Any]) -> tuple[str, str, str]:
    return (
        str(finding.get("id", "")),
        str(finding.get("package", "")),
        str(finding.get("version", "")),
    )


def evaluate_findings(
    findings: Iterable[dict[str, Any]],
    exceptions: Iterable[dict[str, Any]],
    today: date | None = None,
) -> dict[str, Any]:
    """Classify normalized advisory findings using the Q-007 gate."""

    today = today or date.today()
    exception_map: dict[tuple[str, str, str], dict[str, Any]] = {}
    exception_errors: list[str] = []
    for index, entry in enumerate(exceptions):
        errors = validate_exception(entry, today=today)
        if errors:
            exception_errors.extend(f"exceptions[{index}]: {error}" for error in errors)
        else:
            exception_map[_finding_key(entry)] = entry

    decisions: list[dict[str, Any]] = []
    blocking = False
    warnings = False
    for finding in findings:
        kind = str(finding.get("kind", "unknown")).lower()
        key = _finding_key(finding)
        if kind in BLOCKING_KINDS:
            exception = exception_map.get(key)
            waived = exception is not None
            if not waived:
                blocking = True
            decisions.append(
                {
                    "finding": finding,
                    "decision": "waived" if waived else "block",
                    "exception": exception,
                }
            )
        elif kind in WARNING_KINDS:
            warnings = True
            decisions.append({"finding": finding, "decision": "warn", "exception": None})
        else:
            blocking = True
            decisions.append({"finding": finding, "decision": "block_unknown_kind", "exception": None})

    if exception_errors or blocking:
        outcome = "fail"
    elif warnings:
        outcome = "warn"
    else:
        outcome = "pass"
    return {
        "outcome": outcome,
        "blocking": blocking or bool(exception_errors),
        "warnings": warnings,
        "exception_errors": exception_errors,
        "decisions": decisions,
    }


def extract_findings(payload: dict[str, Any]) -> list[dict[str, Any]]:
    """Normalize the common cargo-audit JSON vulnerability/warning shapes."""

    findings: list[dict[str, Any]] = []
    vulnerabilities = payload.get("vulnerabilities", {})
    if isinstance(vulnerabilities, dict):
        items = vulnerabilities.get("list", [])
        if isinstance(items, list):
            for item in items:
                finding = _item_to_finding(item, "vulnerability")
                if finding:
                    findings.append(finding)

    warnings = payload.get("warnings", {})
    if isinstance(warnings, dict):
        for kind in WARNING_KINDS | {"unsound"}:
            items = warnings.get(kind, [])
            if isinstance(items, dict):
                items = items.get("list", [])
            if isinstance(items, list):
                for item in items:
                    finding = _item_to_finding(item, kind)
                    if finding:
                        findings.append(finding)

    unique: dict[tuple[str, str, str], dict[str, Any]] = {}
    for finding in findings:
        unique[_finding_key(finding)] = finding
    return list(unique.values())


def _item_to_finding(item: Any, default_kind: str) -> dict[str, Any] | None:
    if not isinstance(item, dict):
        return None
    advisory = item.get("advisory", {})
    if not isinstance(advisory, dict):
        advisory = {}
    package = item.get("package", {})
    if not isinstance(package, dict):
        package = {}
    advisory_id = advisory.get("id") or item.get("id")
    package_name = package.get("name") or item.get("package_name")
    version = package.get("version") or item.get("version")
    if not advisory_id or not package_name or not version:
        return None
    text = " ".join(
        str(value).lower()
        for value in (
            advisory.get("informational"),
            advisory.get("title"),
            advisory.get("description"),
            advisory.get("categories"),
        )
    )
    kind = "unsound" if "unsound" in text else default_kind
    return {
        "id": str(advisory_id),
        "package": str(package_name),
        "version": str(version),
        "kind": kind,
        "severity": advisory.get("cvss") or advisory.get("severity"),
        "title": advisory.get("title"),
    }
