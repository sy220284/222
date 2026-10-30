"""Structural guardrail for handwritten source view entries.

Passing this test only proves routes exist and known mass-generated placeholders
are absent. It does *not* certify complete behavior or replace Cargo tests.
"""
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[2]
PORTS = ROOT / 'src/generated_ports'

VIEW_PORTS = {
    'src/components/PromptInput/PromptInputFooterSuggestions.tsx': ('p_a680bef86b91f.rs', 'PromptInputFooterSuggestions'),
    'src/components/PromptInput/PromptInputFooterLeftSide.tsx': ('p_100d28512b001.rs', 'PromptInputFooterLeftSide'),
    'src/components/PromptInput/PromptInputQueuedCommands.tsx': ('p_9b6293319d85b.rs', 'PromptInputQueuedCommands'),
    'src/components/PromptInput/PromptInputHelpMenu.tsx': ('p_ed2482259a075.rs', 'PromptInputHelpMenu'),
    'src/components/PromptInput/Notifications.tsx': ('p_4375034f382f7.rs', 'Notifications'),
    'src/components/PromptInput/VoiceIndicator.tsx': ('p_92d31b9e1cfc8.rs', 'VoiceIndicator'),
    'src/components/PromptInput/ShimmeredInput.tsx': ('p_2f84f9e35605c.rs', 'HighlightedInput'),
    'src/components/PromptInput/PromptInputFooter.tsx': ('p_c39f8f63b92a4.rs', 'PromptInputFooter'),
    'src/components/PromptInput/PromptInputModeIndicator.tsx': ('p_4ae3f27ee4b20.rs', 'PromptInputModeIndicator'),
}


class HandwrittenViewRegistryTests(unittest.TestCase):
    def test_all_handwritten_views_are_dispatchable_by_source_export_name(self):
        registry = '\n'.join(path.read_text() for path in PORTS.glob('registry_*.rs'))
        for source, (filename, alias) in VIEW_PORTS.items():
            with self.subTest(source=source):
                self.assertIn(f'"{source}" => match export', registry)
                segment = registry.split(f'"{source}" => match export', 1)[1].split('\n        },', 1)[0]
                self.assertIn('"view"', segment)
                self.assertIn(f'"{alias}"', segment)
                code = (PORTS / filename).read_text()
                self.assertIn('pub fn render_source_view', code)
                self.assertNotIn('Native Rust UI structure derived from original', code)

    def test_source_view_adapters_do_not_hide_null_placeholders(self):
        for source, (filename, _) in VIEW_PORTS.items():
            with self.subTest(source=source):
                code = (PORTS / filename).read_text()
                self.assertNotIn('runtime::truthy(&Value::Null)', code)
                self.assertNotIn('children: vec![VNode::Empty, VNode::Value(Value::Null)', code)


if __name__ == '__main__':
    unittest.main()
