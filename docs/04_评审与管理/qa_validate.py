#!/usr/bin/env python3
"""LightIDE Markdown documentation package static consistency audit.

No real IDE tests are executed. This script checks document/test artifact integrity only.
Usage: python 04_评审与管理/qa_validate.py
"""
from pathlib import Path
from collections import Counter
import json,re,sys,hashlib

ROOT=Path(__file__).resolve().parents[1]
TESTS=json.loads((ROOT/'03_测试资料/共通/testcases.json').read_text(encoding='utf-8'))
FIX=ROOT/'03_测试资料/fixtures'
errors=[];warnings=[];passed=[]
def ok(name,condition,detail=''):
    (passed if condition else errors).append(name if condition else f'{name}: {detail}')

allcases=[r for group in TESTS.values() for r in group]
fr={f'FR-{n:03}' for n in range(1,50)}
nfr={f'NFR-{n:03}' for n in range(1,29)}
for typ,n in [('UT',98),('IT',49),('ST',45)]:
    ok(f'{typ}数量',len(TESTS[typ])==n,f'actual={len(TESTS[typ])}')
ids=[r['case_id'] for r in allcases]
ok('用例ID唯一',len(set(ids))==len(ids))
for r in allcases:
    reqs=set(r['requirements'].split(','))
    if not reqs.issubset(fr|nfr): errors.append(f'无效需求引用 {r["case_id"]}: {reqs-(fr|nfr)}')
    if r['status']!='未执行' or r['result'] or r['evidence']:
        errors.append(f'伪执行状态 {r["case_id"]}')
    if not r['steps'].strip() or not r['expected'].strip(): errors.append(f'空步骤/断言 {r["case_id"]}')
    for raw in r['data_id'].split(','):
        fid=raw.strip()
        if fid.startswith('FX-P'):
            if fid not in {'FX-P1','FX-P10','FX-P50'}:errors.append(f'生成数据ID无效 {r["case_id"]}: {fid}')
            continue
        if not list(FIX.glob(fid+'_*')):errors.append(f'样本不存在 {r["case_id"]}: {fid}')
ut=Counter(r['requirements'] for r in TESTS['UT']);it=Counter(r['requirements'] for r in TESTS['IT'])
ok('每FR具有正常异常UT',all(ut[k]==2 for k in fr))
ok('每FR具有IT',set(it)==fr and all(it[k]==1 for k in fr))
stn={q for r in TESTS['ST'] for q in r['requirements'].split(',') if q.startswith('NFR-')}
ok('每NFR具有ST编号映射',stn==nfr)
assertions={r['case_id']:r for r in allcases}
for test_id,needed in [
    ('ST-F001','FX-15'),('ST-N001','FX-15'),('ST-N002','FX-15'),('ST-N003','FX-15'),
    ('ST-F009','FX-16'),('IT-035','FX-16'),('IT-039','FX-16'),('ST-N028','FX-16'),
]:
    ok(f'测试数据匹配 {test_id}',needed in assertions[test_id]['data_id'])
for name in ['FX-15_plain_text.txt','FX-16_plugin_valid.json','FX-13_plugin_invalid.json']:
    ok(f'存在 {name}',(FIX/name).is_file())
ok('真实TXT样本', (FIX/'FX-15_plain_text.txt').suffix.lower()=='.txt')
ok('Mermaid源图9份', len(list((ROOT/'01_基本设计/diagrams').glob('*.mmd')))==9)
ok('Excel交付', (ROOT/'03_测试资料/共通/追踪矩阵与测试用例.xlsx').is_file())
ok('自审报告和G0计划', all((ROOT/'04_评审与管理'/n).is_file() for n in ['QA-002_深度自审与修订记录.md','G0_宿主可行性验证清单.md']))
# Validate referenced relative files outside intentionally invalid fixture directory.
for path in ROOT.rglob('*.md'):
    if 'fixtures' in path.parts:continue
    content=path.read_text(encoding='utf-8')
    content=re.sub(r'```[\s\S]*?```','',content)
    content=re.sub(r'`[^`\n]*`','',content)
    for target in re.findall(r'\]\(([^)]+)\)',content):
        if target.startswith(('http:','https:','#','data:','javascript:')):continue
        target=target.split('#',1)[0].split('?',1)[0]
        if target and not (path.parent/target).exists():errors.append(f'MD链接失效: {path.relative_to(ROOT)} -> {target}')
# Validate plugin manifest if jsonschema available; distinguish Schema from OS enforcement.
try:
    import jsonschema
    schema=json.loads((ROOT/'02_详细设计/contracts/plugin_manifest.schema.json').read_text())
    good=json.loads((FIX/'FX-16_plugin_valid.json').read_text())
    bad=json.loads((FIX/'FX-13_plugin_invalid.json').read_text())
    validator=jsonschema.Draft202012Validator(schema)
    validator.check_schema(schema)
    ok('有效Manifest通过schema',not list(validator.iter_errors(good)))
    ok('无效Manifest被schema拒绝',bool(list(validator.iter_errors(bad))))
except ImportError:
    warnings.append('缺少jsonschema: Manifest校验本轮未执行')
# These are intentional review blockers, do NOT assert they pass.
warnings.extend([
    'UT/IT多数仍为设计级场景，须基于真实实现补全精确步骤与断言',
    '真实宿主API/Undo/IME/编码POC未完成',
    '插件OS隔离、预览渲染和性能实测未完成',
    'Mermaid源图仅检查文件数量，未进行图形渲染',
])
print(json.dumps({'result':'静态检查通过' if not errors else '静态检查失败',
                  'checks_passed':len(passed),'errors':errors,'warnings':warnings,
                  'counts':{k:len(v) for k,v in TESTS.items()}},ensure_ascii=False,indent=2))
sys.exit(1 if errors else 0)
