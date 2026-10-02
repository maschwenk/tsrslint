import re,sys,collections
def parse(path):
    txt=open(path).read().split('-----------+-------------------------------------------------------')
    for blk in txt[1:]:
        lines=[l for l in blk.strip('\n').split('\n') if l.strip()]
        if not lines: continue
        m=re.match(r'\s*([\d.]+)(ms|s|us|µs)\s+(.*)',lines[0])
        if not m: continue
        v=float(m.group(1))*{'s':1,'ms':1e-3,'us':1e-6,'µs':1e-6}[m.group(2)]
        frames=[m.group(3).strip()]+[l.strip() for l in lines[1:]]
        yield v,frames
CATS=[
 ('gc', lambda s: 'runtime.gcBgMarkWorker' in s or 'runtime.bgsweep' in s or 'runtime.bgscavenge' in s or 'runtime.gcDrain' in s),
 ('tsconfig_resolution', lambda s: 'TsConfigResolver' in s),
 ('protocol_output', lambda s: 'main.writeMessage' in s or ('main.runHeadless.func' in s and 'json' in s)),
 ('payload_parse', lambda s: 'deserializePayload' in s),
 ('program_bind', lambda s: 'BindSourceFiles' in s or 'internal/binder.' in s),
 ('program_create', lambda s: 'compiler.NewProgram' in s or 'GetParsedCommandLineOfConfigFile' in s or 'internal/parser.' in s or 'internal/module.' in s or 'compiler.(*fileLoader)' in s or 'processAllProgramFiles' in s),
 ('checker_via_rules', lambda s: 'internal/checker.' in s and ('visitLintNodes' in s or 'internal/rules/' in s or 'RunLinterOnProgram' in s)),
 ('checker_other', lambda s: 'internal/checker.' in s),
 ('rule_logic', lambda s: 'internal/rules/' in s or 'visitLintNodes' in s or 'internal/utils.' in s or 'RunLinterOnProgram' in s or 'internal/linter.' in s),
 ('alloc_unattributed', lambda s: 'runtime.madvise' in s or 'runtime.(*mheap)' in s or 'runtime.systemstack' in s),
]
tot=collections.Counter(); total=0; ex=collections.defaultdict(collections.Counter)
for v,fr in parse(sys.argv[1]):
    s='\n'.join(fr); total+=v
    for name,f in CATS:
        if f(s): tot[name]+=v; ex[name][fr[0]]+=v; break
    else:
        tot['other']+=v; ex['other'][fr[0]+' <- '+(fr[-1] if fr else '')]+=v
print('total CPU s', round(total,2))
for k,v in tot.most_common(): print(f'{k:22s} {v:8.2f}s {100*v/total:5.1f}%')
if len(sys.argv)>2:
    for k in sys.argv[2:]:
        print('--',k); [print('  ',round(v,2),f) for f,v in ex[k].most_common(12)]

