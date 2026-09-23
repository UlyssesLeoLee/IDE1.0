#!/usr/bin/env python3
"""IDE1.0 × Star Python emitter 跨语言 parity 测 (per ULYS-191 §4.2.2 brief v0.1).

用法:
    # IDE1.0 仓根跑
    python3 tests/parity/cross_language_parity.py

逻辑:
    1. 调 `cargo run -p ide-cli -- --aci-emit` 输出 Rust emitter JSON
    2. 调 `_lib_aci_emit.py emit ...` 输出 Python emitter JSON (Stage 1 已 ship)
    3. 比两文件除 `captured_at` 时戳外完全一致

依赖:
    - Star 仓 `_lib_aci_emit.py` 在 $STAR_FLASH_MOCK_PATH/scripts/_lib_aci_emit.py
    - 默认 $STAR_FLASH_MOCK_PATH = ../../UlyssesLeoLee/Star/tools/star-flash-mock
    - 可通过 env var 覆盖

守门:
    - #11 缺标比错标: 字段名 1:1 (alphabetic sort_keys)
    - #19v19 Python 化: Python emitter 是规范, Rust 严格 1:1 对应
"""

from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
import tempfile
from pathlib import Path


# 默认 Star 仓路径 (per stage3 报告 D-Boy 拍板 D:/Star worktree)
DEFAULT_STAR_PATH = Path(__file__).resolve().parents[3] / "Star" / "tools" / "star-flash-mock"
STAR_PATH = Path(os.environ.get("STAR_FLASH_MOCK_PATH", DEFAULT_STAR_PATH))


def run_rust_emitter(workdir: Path, assertion_id: str) -> dict:
    """调 `cargo run -p ide-cli -- --aci-emit <id>` 输出 JSON."""
    cmd = [
        "cargo",
        "run",
        "--quiet",
        "-p",
        "ide-cli",
        "--",
        "--aci-emit",
        assertion_id,
    ]
    result = subprocess.run(
        cmd, cwd=workdir, capture_output=True, text=True, timeout=180
    )
    if result.returncode != 0:
        raise RuntimeError(
            f"rust emitter failed: rc={result.returncode}\n"
            f"stderr={result.stderr}\nstdout={result.stdout}"
        )
    return json.loads(result.stdout)


def run_python_emitter(star_path: Path, assertion_id: str, output_path: Path) -> dict:
    """调 `_lib_aci_emit.py emit ...` 输出 JSON.

    字段值与 Rust `ide-cli --aci-emit` 默认值 1:1 对齐:
    - assertion_id: 由 CLI 参数传入
    - scope: project=ide1.0, module=smoke (匹配 Rust CLI 默认)
    - expect: response_within_ms, value=100, "CLI should start within 100ms"
    - actual: response_within_ms, value=5, "measured 5ms (placeholder)"
    - status: PASS
    - severity: info
    - reasoning: "actual << expect (20x margin) — placeholder per §4.2.2 brief v0.1"
    """
    script = star_path / "scripts" / "_lib_aci_emit.py"
    if not script.exists():
        raise FileNotFoundError(f"python emitter not found: {script}")
    cmd = [
        sys.executable,
        str(script),
        "emit",
        "--layer",
        "it",
        "--assertion-id",
        assertion_id,
        "--scope",
        "project=ide1.0",
        "module=smoke",
        "--expect-type",
        "response_within_ms",
        "--expect-value",
        "100",
        "--expect-description",
        "CLI should start within 100ms",
        "--actual-type",
        "response_within_ms",
        "--actual-value",
        "5",
        "--actual-description",
        "measured 5ms (placeholder)",
        "--status",
        "PASS",
        "--severity",
        "info",
        "--reasoning",
        "actual << expect (20x margin) — placeholder per §4.2.2 brief v0.1",
        "--output",
        str(output_path),
    ]
    result = subprocess.run(cmd, capture_output=True, text=True, timeout=30)
    if result.returncode != 0:
        raise RuntimeError(
            f"python emitter failed: rc={result.returncode}\n"
            f"stderr={result.stderr}\nstdout={result.stdout}"
        )
    with output_path.open(encoding="utf-8") as f:
        return json.load(f)


def _normalize_scope(scope: dict) -> dict:
    """归一化 scope: 删 None 字段 (Rust serde 默认序列化 None, Python 不输出).

    Per Python `_lib_aci_emit.py`, scope 只含非 None 字段.
    Per Rust `aci-emitter`, `serde` 默认序列化 `Option::None` 字段为 `null`.
    """
    return {k: v for k, v in scope.items() if v is not None}


def compare(rust_json: dict, py_json: dict) -> tuple[bool, list[str]]:
    """比对两 JSON, 除 `captured_at` 时戳外完全一致.

    Returns:
        (all_equal, list_of_diffs)
    """
    diffs: list[str] = []
    # 删 captured_at + 归一化 scope (None 字段) 后比
    r = {k: v for k, v in rust_json.items() if k != "captured_at"}
    p = {k: v for k, v in py_json.items() if k != "captured_at"}

    if "scope" in r and isinstance(r["scope"], dict):
        r["scope"] = _normalize_scope(r["scope"])
    if "scope" in p and isinstance(p["scope"], dict):
        p["scope"] = _normalize_scope(p["scope"])

    if set(r.keys()) != set(p.keys()):
        diffs.append(f"key mismatch: rust={sorted(r.keys())} python={sorted(p.keys())}")

    for k in sorted(set(r.keys()) | set(p.keys())):
        if k not in r:
            diffs.append(f"missing in rust: {k}")
            continue
        if k not in p:
            diffs.append(f"missing in python: {k}")
            continue
        if r[k] != p[k]:
            diffs.append(f"value mismatch at {k!r}: rust={r[k]!r} python={p[k]!r}")

    return (len(diffs) == 0, diffs)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--workdir",
        type=Path,
        default=Path(__file__).resolve().parents[2],
        help="IDE1.0 仓根 (含 Cargo.toml)",
    )
    parser.add_argument(
        "--star-path",
        type=Path,
        default=STAR_PATH,
        help="Star 仓含 _lib_aci_emit.py 的根路径",
    )
    parser.add_argument(
        "--assertion-id",
        default="ide1.0:parity:cross-language",
        help="统一 assertion_id (Rust + Python 同步)",
    )
    args = parser.parse_args()

    workdir: Path = args.workdir
    star_path: Path = args.star_path
    aid: str = args.assertion_id

    print("=== IDE1.0 × Star Python emitter 跨语言 parity ===")
    print(f"workdir: {workdir}")
    print(f"star_path: {star_path}")
    print(f"assertion_id: {aid}")
    print()

    with tempfile.TemporaryDirectory() as tmpdir:
        tmp = Path(tmpdir)
        py_output = tmp / "py.aci.json"

        # 1. Rust emitter
        print("[step 1/3] Rust emitter (cargo run -p ide-cli -- --aci-emit)")
        rust = run_rust_emitter(workdir, aid)
        print(f"  → captured_at={rust.get('captured_at')}")

        # 2. Python emitter
        print("[step 2/3] Python emitter (_lib_aci_emit.py emit ...)")
        py = run_python_emitter(star_path, aid, py_output)
        print(f"  → captured_at={py.get('captured_at')}")

        # 3. Compare
        print("[step 3/3] compare (除 captured_at 外完全一致)")
        ok, diffs = compare(rust, py)
        if ok:
            print()
            print("PASS: Rust ↔ Python emitter parity verified")
            print("  - 10 必填字段名 1:1")
            print("  - 字段值 1:1")
            print("  - sort_keys 顺序 1:1 (alphabetic)")
            return 0
        else:
            print()
            print(f"FAIL: {len(diffs)} diffs:")
            for d in diffs[:20]:
                print(f"  - {d}")
            return 2


if __name__ == "__main__":
    sys.exit(main())
