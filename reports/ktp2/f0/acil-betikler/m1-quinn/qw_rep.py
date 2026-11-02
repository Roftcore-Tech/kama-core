import json,gzip,sys,re
d=sys.argv[1]
L=[json.loads(l) for l in open(f"{d}/out.jsonl")]
meta=L[0]['meta']; ev=[x['event'] for x in L[1:] if 'event' in x]
t0=meta['t0_unix_ns']; off=meta['events_epoch_offset_ns']
port=int(re.search(r'--server 10\.10\.1\.1:(\d+)',open(f"{d}/cmd.txt").read()).group(1))
first={};cnt={}
for l in gzip.open(f"{d}/capb_cli.csv.gz",'rt'):
    if l.startswith('#'):continue
    a=l.strip().split(',')
    if a[1]!='o' or int(a[5])!=port: continue
    b=bytes.fromhex(a[6].split(':')[0])
    if b and b[0]==0 and len(b)>=11:
        u=int.from_bytes(b[3:11],'big'); cnt[u]=cnt.get(u,0)+1
        first.setdefault(u,int(a[0]))
rx={}
for l in open(f"{d}/rx_log.jsonl").readlines()[1:]:
    o=json.loads(l); rx.setdefault(o['u'],o['t'])
for e in ev[::7]:
    m=e['msg_id']; su=t0+e['sent_ns']-off
    print(m, f"submit={(su-t0)/1e9:6.2f} first_tx_wait={(first.get(m,0)-su)/1e9:6.2f} tx_count={cnt.get(m,0)} ow={(rx.get(m,su)-su)/1e9:6.2f} ack_lat={((e['delivered_ns'] or 0)-e['sent_ns'])/1e9:6.2f}")
