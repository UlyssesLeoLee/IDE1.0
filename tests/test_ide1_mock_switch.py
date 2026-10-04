#!/usr/bin/env python3
"""IDE1.0 mock_switch reader tests (per ULYS-190 §4.4 stage5 cross-project pattern).

Mirrors RGS `tests/test_rgs_mock_switch.py` + IM1.0/CATs/Star 範式.
"""

import json
import subprocess
import sys
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]  # tests/ → <worktree_root>
ACI = ROOT / ".aci.json"
CLUSTER = ROOT / ".mock-cluster.json"
HELPER = ROOT / "scripts" / "_lib_mock_switch_ide1.py"


class TestIde1ModuleSwitch(unittest.TestCase):
    def setUp(self):
        self.aci = json.loads(ACI.read_text(encoding="utf-8"))
        self.cluster = json.loads(CLUSTER.read_text(encoding="utf-8"))

    def test_aci_file_exists(self):
        self.assertTrue(ACI.exists(), f".aci.json missing at {ACI}")

    def test_cluster_file_exists(self):
        self.assertTrue(CLUSTER.exists(), f".mock-cluster.json missing at {CLUSTER}")

    def test_helper_exists(self):
        self.assertTrue(HELPER.exists(), f"helper script missing at {HELPER}")

    def test_module_switch_total_count_is_8(self):
        plugins = self.aci["plugins"]
        total = sum(len(p.get("modules", {})) for p in plugins.values())
        self.assertEqual(total, 8, f"expected 8 modules across 2 plugins, got {total}")

    def test_module_switch_all_enabled_by_default(self):
        for pid, pconf in self.aci["plugins"].items():
            for mid, mconf in pconf.get("modules", {}).items():
                self.assertTrue(mconf.get("enabled", False), f"{pid}.{mid}")

    def test_ide_cli_plugin_has_4_modules(self):
        mods = self.aci["plugins"]["ide-cli"]["modules"]
        self.assertEqual(set(mods.keys()), {"emit", "version", "help", "shell"})

    def test_ide_kernel_core_plugin_has_4_modules(self):
        mods = self.aci["plugins"]["ide-kernel-core"]["modules"]
        self.assertEqual(
            set(mods.keys()), {"version", "kernel_status", "aci_emit", "init"}
        )

    def test_cluster_enabled_true_and_mode_offline(self):
        self.assertEqual(self.cluster["enabled"], True)
        self.assertEqual(self.cluster["mode"], "offline")

    def test_aci_compat_version_consistent_across_cluster_and_aci(self):
        cv = self.cluster.get("aci_compat_version")
        av = self.aci.get("aci_compat_version")
        self.assertIsNotNone(cv)
        self.assertIsNotNone(av)
        self.assertEqual(cv, av)

    def test_plugins_count_matches_summary_plugins_total(self):
        self.assertEqual(len(self.aci["plugins"]), 2)

    def test_run_helper_read_plugins(self):
        proc = subprocess.run(
            [sys.executable, str(HELPER), "--aci-config", str(ACI), "read-plugins"],
            capture_output=True, text=True, cwd=str(ROOT),
        )
        self.assertEqual(proc.returncode, 0, f"helper failed: {proc.stderr}")
        out = json.loads(proc.stdout)
        self.assertEqual(out["plugins_total"], 2)
        self.assertEqual(out["modules_total"], 8)
        self.assertEqual(out["modules_enabled"], 8)

    def test_run_helper_trace(self):
        proc = subprocess.run(
            [sys.executable, str(HELPER), "--aci-config", str(ACI), "trace"],
            capture_output=True, text=True, cwd=str(ROOT),
        )
        self.assertEqual(proc.returncode, 0, f"trace failed: {proc.stderr}")
        self.assertIn("=8/8 modules", proc.stdout)

    def test_run_helper_validate_compat(self):
        proc = subprocess.run(
            [sys.executable, str(HELPER), "--aci-config", str(ACI), "validate-compat"],
            capture_output=True, text=True, cwd=str(ROOT),
        )
        self.assertEqual(proc.returncode, 0)
        self.assertIn("ACI_COMPAT=OK", proc.stdout)

    def test_cluster_module_count_total_8(self):
        self.assertEqual(self.cluster.get("module_count_total"), 8)

    def test_cluster_module_count_enabled_8(self):
        self.assertEqual(self.cluster.get("module_count_enabled"), 8)


if __name__ == "__main__":
    unittest.main(verbosity=2)
