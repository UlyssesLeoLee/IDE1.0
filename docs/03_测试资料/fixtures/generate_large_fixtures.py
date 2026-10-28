#!/usr/bin/env python3
"""Deterministic big Markdown fixtures. Only call in isolated test workspace.
Usage: python generate_large_fixtures.py --out ./generated --mib 1 10 50
"""
from pathlib import Path
import argparse
p=argparse.ArgumentParser()
p.add_argument('--out',default='./generated')
p.add_argument('--mib',type=int,nargs='+',default=[1,10,50])
a=p.parse_args()
out=Path(a.out).resolve();out.mkdir(parents=True,exist_ok=True)
line=('# Section / 测试标题\n- item 1\n- item 2\n中文 日本語 Unicode 🚀\n\n').encode('utf-8')
for mib in a.mib:
    if mib<1 or mib>100: raise ValueError('Only 1..100 MiB fixtures are allowed')
    target=mib*1024*1024
    path=out/f'FX-P{mib}_{mib}MiB.md'
    with path.open('wb') as f:
        remaining=target
        block=line*(max(1,65536//len(line)))
        while remaining>=len(block):
            f.write(block)
            remaining-=len(block)
        while remaining>=len(line):
            f.write(line)
            remaining-=len(line)
        if remaining:  # pad with ASCII; never truncate a UTF-8 code point
            f.write(b' '*remaining)
    print(path, path.stat().st_size)
