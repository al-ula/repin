import unittest
from datetime import date

from q_policy import evaluate_findings, validate_exception


TODAY = date(2026, 8, 18)


def finding(kind: str) -> dict[str, str]:
    return {
        "id": "RUSTSEC-2020-0071",
        "package": "time",
        "version": "0.1.45",
        "kind": kind,
    }


class AdvisoryPolicyTests(unittest.TestCase):
    def test_vulnerability_blocks(self) -> None:
        result = evaluate_findings([finding("vulnerability")], [], today=TODAY)
        self.assertEqual(result["outcome"], "fail")

    def test_unsound_blocks(self) -> None:
        result = evaluate_findings([finding("unsound")], [], today=TODAY)
        self.assertEqual(result["outcome"], "fail")

    def test_unmaintained_and_notice_warn(self) -> None:
        result = evaluate_findings(
            [finding("unmaintained"), finding("notice")], [], today=TODAY
        )
        self.assertEqual(result["outcome"], "warn")
        self.assertFalse(result["blocking"])

    def test_valid_exception_waives_exact_finding(self) -> None:
        exception = {
            **finding("vulnerability"),
            "owner": "repin-security",
            "rationale": "temporary evidence fixture",
            "remediation_issue": "REP-123",
            "compensating_control": "fixture is never shipped",
            "created": "2026-08-18",
            "expires": "2026-08-31",
        }
        result = evaluate_findings([finding("vulnerability")], [exception], today=TODAY)
        self.assertEqual(result["outcome"], "pass")
        self.assertEqual(result["decisions"][0]["decision"], "waived")

    def test_expired_exception_fails(self) -> None:
        exception = {
            **finding("vulnerability"),
            "owner": "repin-security",
            "rationale": "expired fixture",
            "remediation_issue": "REP-123",
            "compensating_control": "none",
            "created": "2026-07-01",
            "expires": "2026-07-31",
        }
        self.assertTrue(validate_exception(exception, today=TODAY))
        result = evaluate_findings([finding("vulnerability")], [exception], today=TODAY)
        self.assertEqual(result["outcome"], "fail")

    def test_exception_cannot_exceed_thirty_days(self) -> None:
        exception = {
            **finding("vulnerability"),
            "owner": "repin-security",
            "rationale": "too long",
            "remediation_issue": "REP-123",
            "compensating_control": "none",
            "created": "2026-08-18",
            "expires": "2026-09-18",
        }
        self.assertIn("exception duration must be at most 30 days", validate_exception(exception, today=TODAY))


if __name__ == "__main__":
    unittest.main()
