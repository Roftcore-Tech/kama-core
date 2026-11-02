import pickle, csv, gzip, math, sys
out = pickle.load(open('out.pkl','rb'))
W='/home/ryuzaki/kama-k4g-work/wt/ktp2-f0/reports/ktp2/f0/'
D = {}
for r in csv.DictReader(open(W+'d/tam/kosular.csv')):
    D[(r['cell'], r['arm'], int(r['rep']))] = r
B = {}
for r in csv.DictReader(gzip.open(W+'b/tam/kosular_b.csv.gz','rt')):
    B[(r['cell'], r['arm'], int(r['rep']))] = r
def f(x):
    try: return float(x)
    except: return None
for (c,arm), rs in out.items():
    mx = {'d_lat95':0,'d_ow95':0,'d_cD':0,'d_cDow':0,'b_conf95':0,'b_rx95':0,'b_rxD':0}
    miss=0
    for r in rs:
        k=(c,arm,r['rep'])
        d=D.get(k); b=B.get(k)
        if d is None or b is None: miss+=1; continue
        def dif(a,bv):
            if a is None or bv is None: return 0 if (a is None)==(bv is None) else 999
            if math.isinf(a) and math.isinf(bv): return 0
            return abs(a-bv)
        mx['d_lat95']=max(mx['d_lat95'],dif(r['conf_nr95'],f(d['lat_p95_s'])))
        mx['d_ow95']=max(mx['d_ow95'],dif(r['ow_nr95'],f(d['ow_p95_s'])))
        mx['d_cD']=max(mx['d_cD'],dif(r['confD'],f(d['cD_ss'])))
        mx['d_cDow']=max(mx['d_cDow'],dif(r['owD'],f(d['cD_ow'])))
        mx['b_conf95']=max(mx['b_conf95'],dif(r['conf_lin95'],f(b.get('conf_p95_s'))))
        mx['b_rx95']=max(mx['b_rx95'],dif(r['ow_lin95'],f(b.get('rx_ow_p95_s'))))
        mx['b_rxD']=max(mx['b_rxD'],dif(r['owD'],f(b.get('rx_deliv_D'))))
    print(c,arm,'miss',miss,{k:round(v,4) for k,v in mx.items()})
