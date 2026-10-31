"""Refuse implementation/source substitution and advisory-policy bypasses in compatibility facades."""
import importlib.util
from pathlib import Path
import shutil
import tempfile
import unittest

spec = importlib.util.spec_from_file_location("security_adapters", Path(__file__).with_name("check_security_adapters.py"))
guard = importlib.util.module_from_spec(spec)
spec.loader.exec_module(guard)


class AdapterInventoryTests(unittest.TestCase):
    def setUp(self):
        temp = tempfile.TemporaryDirectory()
        self.addCleanup(temp.cleanup)
        self.root = Path(temp.name)
        backend = Path(__file__).resolve().parents[1]
        for name in ("Cargo.toml", "Cargo.lock", "deny.toml"):
            shutil.copyfile(backend / name, self.root / name)
        shutil.copytree(backend / "compat", self.root / "compat")
        shutil.copytree(backend / ".cargo", self.root / ".cargo")

    def replace(self, path, old, new):
        target = self.root / path
        target.write_text(target.read_text().replace(old, new))

    def test_actual_checked_graph_is_auditable(self):
        self.assertEqual(guard.validate(self.root)[0], [])

    def test_facade_cannot_hide_implementation(self):
        source = self.root / "compat/zstd/src/lib.rs"
        source.write_text(source.read_text() + "pub fn extra_implementation() {}\n")
        self.assertTrue(any("direct re-export" in p for p in guard.validate(self.root)[0]))

    def test_no_build_script_can_be_added(self):
        (self.root / "compat/quick-xml/build.rs").write_text("fn main() {}")
        self.assertTrue(any("only its manifest" in p for p in guard.validate(self.root)[0]))

    def test_patched_dependency_cannot_be_downgraded(self):
        self.replace("compat/zstd/Cargo.toml", '=0.14.3', '=0.13.3')
        self.assertTrue(any("exact patched" in p for p in guard.validate(self.root)[0]))

    def test_feature_cannot_select_another_implementation(self):
        self.replace("compat/zstd/Cargo.toml", 'zstd_patched/legacy', 'zstd_patched/experimental')
        self.assertTrue(any("legacy feature" in p for p in guard.validate(self.root)[0]))

    def test_old_registry_implementation_cannot_return(self):
        self.replace("Cargo.lock", 'version = "0.14.3"', 'version = "0.13.2"')
        self.assertTrue(any("lock must contain" in p for p in guard.validate(self.root)[0]))

    def test_fixed_advisory_cannot_be_waived(self):
        self.replace(".cargo/audit.toml", 'ignore = [', 'ignore = ["RUSTSEC-2026-0391",')
        self.assertTrue(any("must not be waived" in p for p in guard.validate(self.root)[0]))


if __name__ == "__main__":
    unittest.main()
