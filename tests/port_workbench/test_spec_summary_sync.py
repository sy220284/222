"""Ensure the specification summary is derived from the one migration matrix."""
from pathlib import Path
import sys
import unittest

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / 'scripts'))
import sync_spec_progress_summary as summary


class SummarySyncTests(unittest.TestCase):
    def test_only_summary_table_changes_and_check_is_idempotent(self):
        original = (
            '# 说明\n\n## 实际状态\n\n'
            '| 范围 | 数量 | 说明 |\n|---|---:|---|\n'
            '| 已验收 | 38 | 原始状态 |\n| 未迁移 | 473 | 待迁移 |\n\n'
            '## 旧记录\n\n历史已验收 38，未迁移 473。\n'
        )
        updated, differences = summary.reconcile_summary(
            original, {'已验收': 40, '未迁移': 471})
        self.assertEqual(differences, {'已验收': (38, 40), '未迁移': (473, 471)})
        self.assertIn('| 已验收 | 40 | 原始状态 |', updated)
        self.assertIn('历史已验收 38，未迁移 473。', updated)
        again, differences = summary.reconcile_summary(
            updated, {'已验收': 40, '未迁移': 471})
        self.assertFalse(differences)
        self.assertEqual(again, updated)

    def test_missing_or_repeated_summary_row_is_fatal(self):
        for section in (
            '## 实际状态\n\n| 缺失 | 1 | 备注 |\n\n## 下节',
            '## 实际状态\n\n| 已验收 | 2 | a |\n| 已验收 | 3 | b |\n\n## 下节',
        ):
            with self.subTest(section=section):
                with self.assertRaises(ValueError):
                    summary.reconcile_summary(section, {'已验收': 2})

    def test_full_repository_summary_matches_matrix(self):
        expected = summary.expected_rows(ROOT)
        text = (ROOT / summary.SUMMARY).read_text(encoding='utf-8')
        _, differences = summary.reconcile_summary(text, expected)
        self.assertFalse(differences)


if __name__ == '__main__':
    unittest.main()
