import re,os,glob,collections,json,subprocess
import sys
REF=sys.argv[1]  # tsgolint checkout (internal/)
TSRS=sys.argv[2]  # tsrs crates/ dir
uses=collections.defaultdict(set)
for f in glob.glob(REF+'/internal/**/*.go',recursive=True):
    if f.endswith('_test.go') or '/rule_tester/' in f: continue
    src=open(f).read()
    where='rules/'+f.split('/internal/rules/')[1].split('/')[0] if '/internal/rules/' in f else f.split('/internal/')[1]
    for m in re.finditer(r'\b(?:ctx\.TypeChecker|typeChecker|checker|tc|c)\.([A-Za-z_][A-Za-z0-9_]*)\s*\(',src):
        uses[m.group(1)].add(where)
def snake(n):
    n=n.replace('Checker_','').replace('Type_','type.').replace('Signature_','signature.')
    s=re.sub(r'(?<=[a-z0-9])([A-Z])',r'_\1',n)
    s=re.sub(r'(?<=[A-Z])([A-Z][a-z])',r'_\1',s)
    return s.lower()
# collect tsrs fns
defs={}
for f in glob.glob(TSRS+'/tsrs_checker/src/*.rs')+glob.glob(TSRS+'/tsrs_ast/src/*.rs'):
    for m in re.finditer(r'^\s*(pub(?:\(crate\))?\s+)?fn\s+([a-z0-9_]+)',open(f).read(),re.M):
        vis='pub' if (m.group(1) or '').strip()=='pub' else ('crate' if m.group(1) else 'private')
        name=m.group(2); prev=defs.get(name)
        rank={'pub':2,'crate':1,'private':0}
        if prev is None or rank[vis]>rank[prev[0]]: defs[name]=(vis,os.path.basename(f))
rows=[]
for k in sorted(uses):
    if k[0].islower() and k not in ('getResolvedSignature','getContextualType','isTypeAssignableTo'): pass
    sn=snake(k); base=sn.split('.')[-1]
    cand=[base, base+'_exported']
    st='missing'
    for c in cand:
        if c in defs:
            v=defs[c][0]; st={'pub':'exists','crate':'exists-but-private','private':'exists-but-private'}[v]
            if st=='exists': break
    if sn.startswith('type.') or sn.startswith('signature.'): st='field/accessor ('+st+')'
    rows.append((k,sn,st,len(uses[k]),sorted(uses[k])[:4]))
json.dump(rows,open(sys.argv[3],'w'),indent=0)
c=collections.Counter(r[2] for r in rows)
print(c)
for r in rows:
    if r[2]!='exists': print(r[2].ljust(34), r[0].ljust(45), r[1].ljust(40), r[3], ','.join(r[4]))

