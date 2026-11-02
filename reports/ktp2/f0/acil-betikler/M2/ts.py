import os,sys,csv,collections
T="/home/ryuzaki/kama-k4g-work/ktp2-f0/runs/tam"
lane=sys.argv[1]; cells=sys.argv[2].split(',') if len(sys.argv)>2 else None
out=collections.defaultdict(lambda: collections.Counter())
for cell in sorted(os.listdir(f"{T}/{lane}/out")):
    if cells and cell not in cells: continue
    cd=f"{T}/{lane}/out/{cell}"
    if not os.path.isdir(cd): continue
    for arm in os.listdir(cd):
        if arm not in ('taban','aday','uyar_taban','uyar_aday'): continue
        for rep in os.listdir(f"{cd}/{arm}"):
            p=f"{cd}/{arm}/{rep}/ts.csv"
            if not os.path.exists(p): continue
            c=out[(cell,arm)]; c['runs']+=1
            mx=dict(structure_prop=0,spec_prop=0,confirmed_prop=0,final_ratio=0)
            ge63=0; nz=0; n=0
            for row in csv.DictReader(open(p)):
                n+=1
                for k in mx: mx[k]=max(mx[k],int(row[k]))
                fr=int(row['final_ratio'])
                if fr>0: nz+=1
                if fr>=63: ge63+=1
                prop=max(int(row['structure_prop']),int(row['spec_prop']),int(row['confirmed_prop']))
                if prop>0: c['samples_prop>0']+=1
                if prop>=63: c['samples_prop>=63']+=1
            c['samples']+=n; c['samples_final>0']+=nz; c['samples_final>=63']+=ge63
            if ge63: c['runs_final>=63']+=1
            for k,v in mx.items(): c['max_'+k]=max(c['max_'+k],v)
for k in sorted(out):
    print(k, dict(out[k]))
