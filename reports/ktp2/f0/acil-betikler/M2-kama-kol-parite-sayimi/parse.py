import os, re, sys, json, csv
ROOT = sys.argv[1]
OUT = sys.argv[2]
def g(rx, s, cast=int, default=None):
    m = re.search(rx, s)
    if not m: return default
    try: return cast(m.group(1))
    except Exception: return m.group(1)
rows = []
for lane in ['a','b','c','d']:
    base = os.path.join(ROOT, lane, 'out')
    if not os.path.isdir(base): continue
    for cell in sorted(os.listdir(base)):
        cp = os.path.join(base, cell)
        if not os.path.isdir(cp): continue
        for arm in sorted(os.listdir(cp)):
            ap = os.path.join(cp, arm)
            if not os.path.isdir(ap): continue
            for rep in sorted(os.listdir(ap)):
                rp = os.path.join(ap, rep)
                if not os.path.isdir(rp): continue
                cmdf = os.path.join(rp, 'cmd.txt')
                if not os.path.exists(cmdf): continue
                cmd = open(cmdf, errors='replace').read()
                if 'kama-product' not in cmd: continue
                ce = open(os.path.join(rp,'c.err'), errors='replace').read() if os.path.exists(os.path.join(rp,'c.err')) else ''
                se = open(os.path.join(rp,'s.err'), errors='replace').read() if os.path.exists(os.path.join(rp,'s.err')) else ''
                rc = open(os.path.join(rp,'rc')).read().strip() if os.path.exists(os.path.join(rp,'rc')) else ''
                r = dict(lane=lane, cell=cell, arm=arm, rep=rep, rc=rc)
                r['bin'] = 'taban' if 'taban-' in cmd else ('aday' if 'aday-' in cmd else '?')
                r['fec_profile_arg'] = g(r'--fec-profile (\d+)', cmd, str, '')
                r['fec_mode_arg'] = g(r'--fec-mode (\w+)', cmd, str, '')
                r['fec_r_arg'] = g(r'--fec-r (\d+)', cmd, str, '')
                r['payload'] = g(r'--payload (\d+)', cmd)
                r['manifest_fec'] = g(r'role=(?:client|mixed-client)[^\n]*? fec=(ON adaptive p0x\w+|ON static-r\d+ p0x\w+|OFF\(subsystem yok\))', ce, str, '')
                r['profile'] = g(r'role=(?:client|mixed-client) profile=(\S+)', ce, str, '')
                r['unit'] = g(r'role=(?:client|mixed-client)[^\n]*? unit=(\d+)B', ce)
                r['msg_cap'] = g(r'role=(?:client|mixed-client)[^\n]*? msg_cap=(\S+)', ce, str, '')
                # KP-CLIENT
                r['delivered'] = g(r'KP-CLIENT delivered=(\d+)/', ce)
                r['tx_fec_B'] = g(r'KP-CLIENT [^\n]*? fec=(\d+) ctrl', ce)
                if r['tx_fec_B'] is None:
                    r['tx_fec_B'] = g(r'MIXED-CLIENT [^\n]*? fec=(\d+) ctrl', ce)
                r['tx_retx_B'] = g(r'(?:KP-CLIENT|MIXED-CLIENT) [^\n]*?retx=(\d+)', ce)
                r['tx_data_B'] = g(r'(?:KP-CLIENT|MIXED-CLIENT) [^\n]*?data=(\d+)', ce)
                r['fec_blocks'] = g(r'KP-CLIENT [^\n]*? fec_blocks=(\d+)', ce)
                r['fec_repairs_tx'] = g(r'KP-CLIENT [^\n]*? fec_repairs_tx=(\d+)', ce)
                r['fec_recovered_ctrl_rx'] = g(r'KP-CLIENT [^\n]*? fec_recovered_ctrl=(\d+)', ce)
                r['parity_fired'] = g(r'fired=\[[^\]]*? parity=(\d+)', ce)
                r['parity_denied'] = g(r'fired=\[[^\]]*? parity_denied=(\d+)', ce)
                r['retx_fired'] = g(r'fired=\[[^\]]*? retx=(\d+)', ce)
                r['recover_fwd'] = g(r'recover_fwd=(\d+)', ce)
                r['recover_arq'] = g(r'recover_arq=(\d+)', ce)
                r['gate_opened'] = g(r'gate_opened=(\d+)', ce)
                r['fec2_clamp'] = g(r'fec2=\[[^\]]*?clamp=(\d+)', ce)
                r['fec2_would_zero'] = g(r'fec2=\[[^\]]*?would_zero=(\d+)', ce)
                r['fec2_partial'] = g(r'fec2=\[[^\]]*?partial_blocks=(\d+)', ce)
                r['fec2_repair_not_arqed'] = g(r'fec2=\[[^\]]*?repair_not_arqed=(\d+)', ce)
                r['btlbw'] = g(r'btlbw=(\d+)B/s', ce)
                r['srtt_us'] = g(r'srtt=(\d+)us', ce)
                r['arbiter'] = g(r'KP-ACTIVATED [^\n]*?arbiter=(\[[^\]]*\])', ce, str, '')
                r['spec_levels'] = g(r'KP-ACTIVATED [^\n]*?spec_levels=(\[[^\]]*\])', ce, str, '')
                # server
                for k in ['fec_repairs_rx','fec_recovered_units','fec_decode_failures','fec_blocks_decoded','fec_repairs_useful','fec_repairs_wasted_complete','fec_repairs_wasted_evicted','fec_blocks_evicted','fec_recovered_ctrl_tx','fec_blocks_complete_with_repairs','fec_blocks_evicted_with_repairs','delivered','param_bound_peers','frag_chunks_rx']:
                    r['srv_'+k] = g(k+r': (\d+)', se)
                r['has_cerr'] = int(bool(ce)); r['has_serr']=int(bool(se))
                rows.append(r)
keys = list(rows[0].keys())
with open(OUT,'w',newline='') as f:
    w = csv.DictWriter(f, fieldnames=keys); w.writeheader(); [w.writerow(x) for x in rows]
print(len(rows))
