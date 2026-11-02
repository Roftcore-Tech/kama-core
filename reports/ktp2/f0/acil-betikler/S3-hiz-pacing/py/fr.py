import sys, os, glob, csv, re
root=sys.argv[1]
tot=0; ge62=0; mx=0; mxcell=None; per={}
for f in glob.glob(os.path.join(root,'*','*','rep*','ts.csv')):
    parts=f.split(os.sep); cell=parts[-4]; arm=parts[-3]
    if arm not in ('taban','aday'): continue
    try:
        rows=list(csv.DictReader(open(f)))
    except Exception: continue
    if not rows: continue
    m=max(int(r['final_ratio']) for r in rows)
    tok=sum(int(r['pacer_tokens'])<1027 for r in rows)/len(rows)
    tot+=1
    if m>=63: ge62+=1
    if m>mx: mx=m; mxcell=f
    rate=re.match(r'b_(r\w+?)_',cell).group(1) if re.match(r'b_(r\w+?)_',cell) else cell
    per.setdefault(rate,[]).append(m)
print("runs",tot,"runs with max final_ratio>=63:",ge62,"max",mx,mxcell)
for k,v in sorted(per.items()):
    v=sorted(v); print(k, "n",len(v),"max_final_ratio median",v[len(v)//2],"max",v[-1])
