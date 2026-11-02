import json, os, glob, sys, math, statistics as st, re
cell=sys.argv[1]; mu=1.0; budget=0.8
for arm in ['arq_taban','uyar_taban','rs2_taban','rs4_taban']:
    tot=0; ok=0; ok_o=0; ok_w=0; tot_w=0
    for rep in sorted(glob.glob(os.path.join(cell,arm,'rep*'))):
        lines=open(os.path.join(rep,'out.jsonl')).read().splitlines()
        meta=None; ev=[]
        for l in lines:
            o=json.loads(l)
            if 'meta' in o: meta=o['meta']
            elif 'event' in o: ev.append(o['event'])
        t0=int(meta['t0_unix_ns']); off=int(meta['events_epoch_offset_ns'])
        rx={}
        for l in open(os.path.join(rep,'rx.jsonl')):
            o=json.loads(l)
            if 'u' in o:
                rx[o['u']]=min(rx.get(o['u'],1<<80),o['t'])
        # variant: no warmup offset (u = msg_id)
        for e in ev:
            mid=e['msg_id']; offu=t0+e['sent_ns']-off
            tr=rx.get(mid+16); tot+=1
            if tr is not None and tr-offu<=(mu+budget)*1e9: ok+=1
            tr2=rx.get(mid); tot_w+=1
            if tr2 is not None and tr2-offu<=(mu+budget)*1e9: ok_w+=1
    print(arm,'pooled W=16 %.3f'%(ok/tot),'pooled W=0(misaligned) %.3f'%(ok_w/tot_w))
