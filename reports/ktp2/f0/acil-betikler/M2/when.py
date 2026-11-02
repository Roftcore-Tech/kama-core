import json,csv,sys,collections
rows=json.load(open('rows.json'))
T="/home/ryuzaki/kama-k4g-work/ktp2-f0/runs/tam"
hist=collections.Counter(); first=[]
for r in rows:
    if r['lane']!='b' or not (r.get('parity') or 0)>0: continue
    p=f"{T}/b/out/{r['cell']}/{r['arm']}/{r['rep']}/ts.csv"
    prev=0
    for row in csv.DictReader(open(p)):
        pe=int(row['parity_emitted'])
        if pe>prev:
            t=int(row['t_ms']); first.append((r['cell'],r['arm'],r['rep'],t,row['final_ratio'],pe-prev))
            hist['<1s' if t<1000 else '1-3s' if t<3000 else '3-10s' if t<10000 else '>=10s']+=pe-prev
        prev=pe
print(hist)
for x in first:
    if 'r25' not in x[0]: print(x)
