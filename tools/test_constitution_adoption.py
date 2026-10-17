"""Prove the evidence classification fails closed without weakening its scanner."""
from __future__ import annotations

import importlib.util
import tempfile
import unittest
from pathlib import Path

SPEC = importlib.util.spec_from_file_location("quillforge_adoption", Path(__file__).with_name("check-constitution-adoption.py"))
assert SPEC is not None and SPEC.loader is not None
ADOPTION = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(ADOPTION)


class AdoptionEvidenceTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory(prefix="wpai-adoption-test-")
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name)
        self.file = self.root / "evidence.md"
        self.file.write_bytes(b"Reviewed evidence\r\n")
        self.finding = ("LEGACY_REFERENCE", "evidence.md", 1)
        self.reviewed = {"evidence.md": (ADOPTION.digest(b"Reviewed evidence\n"), {1})}

    def test_exact_reviewed_bytes_and_lines_are_classified(self):
        self.assertEqual(ADOPTION.classify(self.root, [self.finding], self.reviewed), ([self.finding], []))

    def test_modified_file_fails_even_without_new_scanner_findings(self):
        self.file.write_bytes(b"New active instructions\n")
        retained, problems = ADOPTION.classify(self.root, [self.finding], self.reviewed)
        self.assertEqual(retained, [])
        self.assertTrue(any("file changed" in problem for problem in problems))

    def test_new_code_line_path_or_duplicate_cannot_be_classified(self):
        for unexpected in [
            ("OLD_PACK_DIRECTORY", "evidence.md", 1),
            ("LEGACY_REFERENCE", "evidence.md", 2),
            ("LEGACY_REFERENCE", "other.md", 1),
            self.finding,
        ]:
            with self.subTest(unexpected=unexpected):
                _, problems = ADOPTION.classify(self.root, [self.finding, unexpected], self.reviewed)
                self.assertTrue(problems)

    def test_missing_finding_or_file_fails(self):
        self.assertTrue(ADOPTION.classify(self.root, [], self.reviewed)[1])
        self.file.unlink()
        self.assertTrue(ADOPTION.classify(self.root, [self.finding], self.reviewed)[1])

    def test_evidence_path_cannot_escape_project(self):
        child = self.root / "child"
        child.mkdir()
        finding = ("LEGACY_REFERENCE", "../evidence.md", 1)
        reviewed = {"../evidence.md": next(iter(self.reviewed.values()))}
        retained, problems = ADOPTION.classify(child, [finding], reviewed)
        self.assertEqual(retained, [])
        self.assertTrue(any("escapes project" in problem for problem in problems))

    def test_changed_checker_is_rejected_before_execution(self):
        source = self.root / "checker.py"
        source.write_text("raise RuntimeError('must not execute')\n", encoding="utf-8")
        with self.assertRaisesRegex(ValueError, "differs from pinned source"):
            ADOPTION.load_checker(source)


if __name__ == "__main__":
    unittest.main()
