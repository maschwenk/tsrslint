import os,struct,json,sys
def decode(path):
    b=open(path,'rb').read(); i=0; out=[]
    while i<len(b):
        n,=struct.unpack('<I',b[i:i+4]); t=b[i+4]; p=json.loads(b[i+5:i+5+n]); i+=5+n
        out.append((t,p))
    return out
if __name__=='__main__':
    msgs=decode(sys.argv[1])
    for t,p in msgs:
        if t==1: print('DIAG', p.get('rule'), os.path.relpath(p.get('file_path',''), os.environ.get('ROOT', '.')), p.get('range'), p['message']['id'])
        elif t==2:
            for r in sorted(p['rules'], key=lambda r:-r['duration']): print('TIMING', r['rule_name'], round(r['duration']/1e9,3),'s', r['calls'])
        else: print('ERR', p)

