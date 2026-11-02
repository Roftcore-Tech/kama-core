import json, os, sys, glob, statistics as st
root=sys.argv[1]; cell=sys.argv[2]; owd=float(sys.argv[3]); budget=float(sys.argv[4]); off=int(sys.argv[5])
for arm in ['arq_taban','uyar_taban','rs2_taban','rs4_taban']:
    un=[];orr=[];lat0=[]
    for rep in sorted(glob.glob(os.path.join(root,cell,arm,'rep*'))):
        f=os.path.join(rep,'out.jsonl'); r=os.path.join(rep,'rx.jsonl')
        if not (os.path.exists(f) and os.path.exists(r)): continue
        L=open(f).read().splitlines(); m=json.loads(L[0])['meta']
        t0=m['t0_unix_ns']; eo=m['events_epoch_offset_ns']
        sent={}
        for l in L[1:]:
            e=json.loads(l).get('event')
            if e: sent[e['msg_id']]=t0+(e['sent_ns']-eo)
        arr={}
        for l in open(r):
            d=json.loads(l)
            if 'u' in d:
                mid=d['u']-off
                if mid in sent and mid not in arr: arr[mid]=d['t']
        ids=sorted(sent)
        runmax=None
        ok_u=0; ok_o=0
        for i in ids:
            a=arr.get(i)
            # ordered delivery time = max arrival over all j<=i (inf if any missing)
            if a is None: runmax=float('inf')
            else: runmax = a if runmax is None else max(runmax,a)
            lim=sent[i]+ (owd+budget)*1e9
            if a is not None and a<=lim: ok_u+=1
            if runmax!=float('inf') and runmax<=lim: ok_o+=1
            if a is not None and i==ids[0]: lat0.append((a-sent[i])/1e9)
        un.append(ok_u/len(ids)); orr.append(ok_o/len(ids))
    if un:
        print(cell,arm,'N',len(un),'unordered_ontime_mean',round(st.mean(un),3),'ordered_ontime_mean',round(st.mean(orr),3),'first-frame-lat-med',round(st.median(lat0),3) if lat0 else None)
