import json, os, glob, sys, math
cell=sys.argv[1]; lim=1.8e9
for arm in ['arq_taban','uyar_taban','rs2_taban','rs4_taban']:
    tot=0; ok=0
    for rep in sorted(glob.glob(os.path.join(cell,arm,'rep*'))):
        meta=None; ev=[]
        for l in open(os.path.join(rep,'out.jsonl')):
            o=json.loads(l)
            if 'meta' in o: meta=o['meta']
            elif 'event' in o: ev.append(o['event'])
        t0=int(meta['t0_unix_ns']); off=int(meta['events_epoch_offset_ns'])
        rx={}
        for l in open(os.path.join(rep,'rx.jsonl')):
            o=json.loads(l)
            if 'u' in o: rx[o['u']]=min(rx.get(o['u'],math.inf),o['t'])
        ev.sort(key=lambda e:e['msg_id']); rm=-math.inf
        for e in ev:
            rm=max(rm, rx.get(e['msg_id']+16, math.inf)); tot+=1
            if rm-(t0+e['sent_ns']-off)<=lim: ok+=1
    print(arm,'pooled ordered %.3f'%(ok/tot))
