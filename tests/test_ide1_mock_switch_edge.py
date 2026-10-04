#!/usr/bin/env python3
"""IDE1.0 mock_switch reader — edge / failure-path tests (ULYS-190 §4.4 stage6).

Per ULYS-190 §4.4 cross-project pattern G-MS-04 (mirrors IM1.0/CATs/Star/RGS):
补 test_ide1_mock_switch.py 的失败路径 + 边界用例 — disabled module / mode=online /
cluster.enabled=false / 各 module 单独 is-enabled / cluster 缺字段 / ACI 缺字段 /
validate-compat mismatch / validate-compat missing / aci.plugins missing。

每个测试都用临时 fixture (tmp_path) 写一个隔离的 ACI/cluster JSON,然后调
helper subprocess。这样测试不污染真实 .aci.json / .mock-cluster.json。

用法:
    python tests/test_ide1_mock_switch_edge.py -v
"""

import json
import shutil
import subprocess
import sys
import tempfile
import unittest
import uuid
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]  # tests/ → <worktree_root>
HELPER = ROOT / "scripts" / "_lib_mock_switch_ide1.py"


def _load_real_aci() -> dict:
    return json.loads((ROOT / ".aci.json").read_text(encoding="utf-8"))


def _load_real_cluster() -> dict:
    return json.loads((ROOT / ".mock-cluster.json").read_text(encoding="utf-8"))


def _run_helper(aci_path: Path, cluster_path: "Path | None", cmd: str):
    """调 helper 子命令,返回 (exit_code, stdout).

    cluster_path=None 表示不传 --cluster-config(让 helper 走默认查找,
    默认情况下若 .mock-cluster.json 不在 aci_path 同目录,等价于 cluster 缺)。
    """
    args = [sys.executable, str(HELPER)]
    if cluster_path is not None:
        args += ["--aci-config", str(aci_path), "--cluster-config", str(cluster_path)]
    else:
        args += ["--aci-config", str(aci_path)]
    args.append(cmd)
    proc = subprocess.run(args, capture_output=True, text=True)
    return proc.returncode, proc.stdout


class TestMockSwitchEdge(unittest.TestCase):
    """失败路径 + 边界用例 — 临时 fixture,隔离真实 .aci.json/.mock-cluster.json。"""

    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp(prefix=f"ide1_edge_{uuid.uuid4().hex[:8]}_"))
        self.aci_data = _load_real_aci()
        self.cluster_data = _load_real_cluster()
        self.aci_path = self.tmp / "aci.json"
        self.cluster_path = self.tmp / "cluster.json"
        self.aci_path.write_text(json.dumps(self.aci_data, indent=2, ensure_ascii=False), encoding="utf-8")
        self.cluster_path.write_text(json.dumps(self.cluster_data, indent=2, ensure_ascii=False), encoding="utf-8")

    def tearDown(self):
        shutil.rmtree(self.tmp, ignore_errors=True)

    # ---------- cluster enabled 边界 ----------

    def test_cluster_enabled_false_is_enabled_returns_false_exit_1(self):
        """cluster.enabled=false → is-enabled exit 1, stdout=false."""
        cluster = dict(self.cluster_data)
        cluster["enabled"] = False
        p = self.tmp / "c.json"
        p.write_text(json.dumps(cluster), encoding="utf-8")
        rc, out = _run_helper(self.aci_path, p, "is-enabled")
        self.assertEqual(rc, 1, f"is-enabled should exit 1 when cluster disabled, got {rc}")
        self.assertEqual(out.strip(), "CLUSTER_ENABLED=false")

    def test_cluster_enabled_missing_is_enabled_returns_false_exit_1(self):
        """cluster 缺 enabled 字段 → 默认 false,exit 1。"""
        cluster = dict(self.cluster_data)
        del cluster["enabled"]
        p = self.tmp / "c.json"
        p.write_text(json.dumps(cluster), encoding="utf-8")
        rc, out = _run_helper(self.aci_path, p, "is-enabled")
        self.assertEqual(rc, 1, f"is-enabled should exit 1 when enabled field missing, got {rc}")
        self.assertEqual(out.strip(), "CLUSTER_ENABLED=false")

    def test_cluster_missing_file_is_enabled_returns_false_exit_1(self):
        """cluster 文件不存在 → is-enabled 默认 false,exit 1。"""
        rc, out = _run_helper(self.aci_path, None, "is-enabled")
        self.assertEqual(rc, 1, f"is-enabled should exit 1 when cluster missing, got {rc}")
        self.assertEqual(out.strip(), "CLUSTER_ENABLED=false")

    # ---------- mode 边界 ----------

    def test_cluster_mode_missing_get_mode_returns_unknown(self):
        """cluster 缺 mode → get-mode 返回 unknown,exit 0。"""
        cluster = dict(self.cluster_data)
        del cluster["mode"]
        p = self.tmp / "c.json"
        p.write_text(json.dumps(cluster), encoding="utf-8")
        rc, out = _run_helper(self.aci_path, p, "get-mode")
        self.assertEqual(rc, 0)
        self.assertEqual(out.strip(), "CLUSTER_MODE=unknown")

    def test_cluster_mode_missing_file_get_mode_returns_unknown(self):
        """cluster 文件不存在 → get-mode 返回 unknown。"""
        rc, out = _run_helper(self.aci_path, None, "get-mode")
        self.assertEqual(rc, 0)
        self.assertEqual(out.strip(), "CLUSTER_MODE=unknown")

    def test_cluster_mode_online_get_mode_returns_online(self):
        """mode=online 显式设置 → get-mode 返回 online。"""
        cluster = dict(self.cluster_data)
        cluster["mode"] = "online"
        p = self.tmp / "c.json"
        p.write_text(json.dumps(cluster), encoding="utf-8")
        rc, out = _run_helper(self.aci_path, p, "get-mode")
        self.assertEqual(rc, 0)
        self.assertEqual(out.strip(), "CLUSTER_MODE=online")

    # ---------- aci_compat_version 失败路径 ----------

    def test_cluster_missing_compat_version_validate_compat_fails(self):
        """cluster 缺 aci_compat_version → validate-compat 失败 (FAIL + missing)。"""
        cluster = dict(self.cluster_data)
        del cluster["aci_compat_version"]
        p = self.tmp / "c.json"
        p.write_text(json.dumps(cluster), encoding="utf-8")
        rc, out = _run_helper(self.aci_path, p, "validate-compat")
        self.assertEqual(rc, 1, f"validate-compat should fail when cluster missing compat version, got {rc}")
        self.assertIn("ACI_COMPAT=FAIL", out)
        self.assertIn("missing:", out)
        self.assertIn("cluster=None", out)

    def test_aci_missing_compat_version_validate_compat_fails(self):
        """aci 缺 aci_compat_version → validate-compat 失败 (FAIL + missing)。"""
        aci = dict(self.aci_data)
        del aci["aci_compat_version"]
        p = self.tmp / "a.json"
        p.write_text(json.dumps(aci, indent=2, ensure_ascii=False), encoding="utf-8")
        rc, out = _run_helper(p, self.cluster_path, "validate-compat")
        self.assertEqual(rc, 1, f"validate-compat should fail when ACI missing compat version, got {rc}")
        self.assertIn("ACI_COMPAT=FAIL", out)
        self.assertIn("aci=None", out)

    def test_compat_version_mismatch_validate_compat_fails(self):
        """cluster 与 aci 版本不匹配 → validate-compat 失败 (FAIL + 值)。"""
        cluster = dict(self.cluster_data)
        cluster["aci_compat_version"] = "99.99.99-mismatch"
        p = self.tmp / "c.json"
        p.write_text(json.dumps(cluster), encoding="utf-8")
        rc, out = _run_helper(self.aci_path, p, "validate-compat")
        self.assertEqual(rc, 1, f"validate-compat should fail on mismatch, got {rc}")
        self.assertIn("ACI_COMPAT=FAIL", out)
        self.assertIn("cluster=99.99.99-mismatch", out)
        self.assertIn("aci=0.1.0-draft", out)

    # ---------- module disabled 边界 ----------

    def test_one_module_disabled_read_plugins_reflects_count(self):
        """一个 module enabled=false → read-plugins modules_enabled 减 1,且 emit.enabled=false。"""
        aci = json.loads(json.dumps(self.aci_data))  # deep copy
        aci["plugins"]["ide-cli"]["modules"]["emit"]["enabled"] = False
        p = self.tmp / "a.json"
        p.write_text(json.dumps(aci, indent=2, ensure_ascii=False), encoding="utf-8")
        rc, out = _run_helper(p, self.cluster_path, "read-plugins")
        self.assertEqual(rc, 0, f"read-plugins should succeed, got {rc}: {out}")
        d = json.loads(out)
        self.assertEqual(d["modules_total"], 8, "total still 8")
        self.assertEqual(d["modules_enabled"], 7, "enabled drops to 7")
        self.assertFalse(
            d["plugins"]["ide-cli"]["modules"]["emit"]["enabled"],
            "ide-cli.emit should be disabled",
        )
        # 其余 7 个仍 enabled
        for pid, pconf in d["plugins"].items():
            for mid, mconf in pconf["modules"].items():
                if pid == "ide-cli" and mid == "emit":
                    continue
                self.assertTrue(
                    mconf["enabled"], f"{pid}.{mid} should still be enabled"
                )

    def test_aci_plugins_field_missing_read_plugins_zero(self):
        """aci 缺 plugins 字段 → read-plugins 全 0,exit 0 (helper 不抛)。"""
        aci = dict(self.aci_data)
        del aci["plugins"]
        p = self.tmp / "a.json"
        p.write_text(json.dumps(aci, indent=2, ensure_ascii=False), encoding="utf-8")
        rc, out = _run_helper(p, self.cluster_path, "read-plugins")
        self.assertEqual(rc, 0, f"read-plugins should not crash, got {rc}: {out}")
        d = json.loads(out)
        self.assertEqual(d["plugins_total"], 0)
        self.assertEqual(d["plugins_enabled"], 0)
        self.assertEqual(d["modules_total"], 0)
        self.assertEqual(d["modules_enabled"], 0)
        self.assertEqual(d["plugins"], {})


if __name__ == "__main__":
    unittest.main(verbosity=2)
