import os, re, json, statistics, sys
ROOT="/home/ryuzaki/kama-k4g-work/ktp2-f0/runs/tam/b/out"
for cell in sys.argv[1:]:
    for arm in ("taban","aday","quinn"):
        ad=os.path.join(ROOT,cell,arm)
        par=[];pden=[];fb=[];rep_tx=[];loss=[];deliv=[];fecB=[]
        for rep in sorted(os.listdir(ad), key=lambda x:int(x[3:])):
            d=os.path.join(ad,rep)
            ce=open(os.path.join(d,"c.err")).read()
            if arm!="quinn":
                m=re.search(r"KP-ACTIVATION .*?parity=(\d+) parity_denied=(\d+)",ce); par.append(int(m.group(1))); pden.append(int(m.group(2)))
                m=re.search(r"KP-CLIENT delivered=(\d+)/(\d+).*?fec=(\d+).*?fec_blocks=(\d+) fec_repairs_tx=(\d+)",ce)
                deliv.append(int(m.group(1))); fecB.append(int(m.group(3))); fb.append(int(m.group(4))); rep_tx.append(int(m.group(5)))
            pre=json.load(open(os.path.join(d,"realized_pre.json"))); post=json.load(open(os.path.join(d,"realized_post.json")))
            def q(js,dev):
                for x in js["mid_qdisc"]:
                    if x.get("dev")==dev and x["kind"]=="netem": return x
            # forward = client->server: which dev? use the one with more bytes
            ls=[]
            for dev in ("ma0","mb0"):
                a=q(pre,dev); b=q(post,dev)
                pk=b["packets"]-a["packets"]; dr=b["drops"]-a["drops"]
                ls.append((dev, b["bytes"]-a["bytes"], dr/(pk+dr) if pk+dr else float('nan')))
            loss.append(ls)
        fwd=[max(l,key=lambda z:z[1])[2] for l in loss]
        s=f"{cell} {arm}: fwd_loss_med={statistics.median(fwd):.3f}"
        if arm!="quinn":
            s+=f" parity>0 runs={sum(1 for p in par if p>0)}/{len(par)} parity_vals={[p for p in par if p>0]} pden_when={[pden[i] for i,p in enumerate(par) if p>0]} fec_blocks>0={sum(1 for x in fb if x>0)} max_repairs_tx={max(rep_tx)} fecB>0={sum(1 for x in fecB if x>0)} max_fecB={max(fecB)} deliv_min={min(deliv)}"
        print(s)
