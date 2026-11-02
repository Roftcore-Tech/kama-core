import csv,sys,statistics as st,collections
for p in sys.argv[1:]:
    rows=list(csv.DictReader(open(p)))
    fr=[int(r['final_ratio']) for r in rows]
    nz=[x for x in fr if x>0]
    tok=[int(r['pacer_tokens']) for r in rows]
    pl=[int(r['pending_lost']) for r in rows]
    pp=[(int(r['t_ms']),int(r['pending_parity']),int(r['pacer_tokens'])) for r in rows if int(r['pending_parity'])>0]
    c=collections.Counter(nz)
    print(p.split('/out/')[1], 'n',len(rows),'final_ratio>0:',len(nz),'min/max',(min(nz),max(nz)) if nz else None,'top',c.most_common(5),
      'tok<1027 frac %.3f'%(sum(1 for t in tok if t<1027)/len(tok)), 'pending_lost>0 frac %.3f'%(sum(1 for x in pl if x>0)/len(pl)),
      'pend_parity span', (pp[0][0],pp[-1][0],len(pp), st.median([x[2] for x in pp])) if pp else None,
      'parity_emitted_last', rows[-1]['parity_emitted'], 'parity_denied_last', rows[-1]['parity_denied'])
