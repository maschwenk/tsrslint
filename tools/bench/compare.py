import sys,json
sys.path.insert(0, __import__("os").path.dirname(__file__))
from decode import decode
def canon(path):
    out=[]
    for t,p in decode(path):
        if t==1:
            p=dict(p)
            for k in ('fixes','suggestions','labeled_ranges'):
                if not p.get(k): p.pop(k,None)
            if p.get('message',{}).get('help') in (None,''): p['message'].pop('help',None)
            out.append(json.dumps(p,sort_keys=True))
        elif t==0: out.append('ERROR '+json.dumps(p))
    return sorted(out)
a=canon(sys.argv[1]); b=canon(sys.argv[2])
sa,sb=set(a),set(b)
only_a=[x for x in a if x not in sb]; only_b=[x for x in b if x not in sa]
print(f'{sys.argv[1]}: {len(a)}  {sys.argv[2]}: {len(b)}  common: {len(sa&sb)}  only-first: {len(only_a)}  only-second: {len(only_b)}')
for x in only_a[:int(sys.argv[3]) if len(sys.argv)>3 else 20]: print('- ',x[:600])
for x in only_b[:int(sys.argv[3]) if len(sys.argv)>3 else 20]: print('+ ',x[:600])
sys.exit(0 if not only_a and not only_b else 1)

