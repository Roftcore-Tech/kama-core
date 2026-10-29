#!/usr/bin/env python3
"""[yeniden-hesap şeridi] NETEM-7 bağımsız yeniden hesap.

netem7_analyze.py ÇAĞRILMAZ ve İÇE AKTARILMAZ. Tanımlar ONKAYIT.md §2'den:
  M1 = D=20 s içinde teslim oranı (payda = olay sayısı; phase5_cell_report.py comp() ile aynı)
  M2 = teslim edilenlerin gecikme medyanı (sent->delivered)
  Test = iki yanlı, kesin, bağ-düzeltmeli Mann-Whitney, RNG yok, alpha = 0.05/7
  KAYMA(M1) <=> |med(a)-med(t)| >= 0.05 ve p < alpha
  KAYMA(M2) <=> med oranı not in [0.8, 1.25] ve p < alpha
  Altyapı arızası = olay içermeyen ya da out.jsonl üretmeyen koşu; kol-profil başına > 2 -> ALTYAPI-DUR

Bağımsızlık: (1) her satır tam JSON olarak ayrıştırılır (alt-dize süzgeci değil);
(2) değerler kesin rasyonel (Fraction) tutulur; (3) MWU null dağılımı BAĞ GRUPLARI üstünde
çok-terimli (binom çarpımı) DP ile kurulur (eleman-eleman DP değil); gözlenen U çift-sayımla
(x>y + 1/2 x=y) ayrıca hesaplanıp sıra-toplamı yoluyla bulunanla eşitlenir;
(4) DP küçük örneklerde kaba-kuvvet (itertools.combinations) ile doğrulanır.

Kullanım: python3 -I recompute.py RUNDIR [ANALIZ.txt] [--p2-detail]
"""
import itertools
import json
import math
import pathlib
import re
import sys
from fractions import Fraction as F

PROFILES = ["p1_loss50", "p2_loss80", "p3_loss95", "p4_jitter", "p5_revpath", "p6_blackout", "p7_bbr"]
D_NS = 20 * 10**9
ALPHA = F(5, 100) / 7
M1_THR = F(5, 100)
M2_LO, M2_HI = F(8, 10), F(125, 100)


# ---------------------------------------------------------------- ham veri
def parse_run(repdir):
    """Bir tekrarın ham çıktısını tam-JSON ile ayrıştır; tutarlılık denetimleri topla."""
    info = {"rep": repdir.name, "has_out": False, "n_ev": 0, "anomalies": []}
    rcf = repdir / "rc"
    info["rc"] = rcf.read_text().strip() if rcf.exists() else None
    out = repdir / "out.jsonl"
    if not out.exists():
        return info
    info["has_out"] = True
    evs, metas, other, bad, substr_lines = [], [], 0, 0, 0
    for ln in out.read_text().splitlines():
        if '"event"' in ln:
            substr_lines += 1
        if not ln.strip():
            continue
        try:
            o = json.loads(ln)
        except json.JSONDecodeError:
            bad += 1
            continue
        if isinstance(o, dict) and isinstance(o.get("event"), dict):
            evs.append(o["event"])
        elif isinstance(o, dict) and "meta" in o:
            metas.append(o["meta"])
        else:
            other += 1
    info.update(n_ev=len(evs), n_meta=len(metas), n_other=other, n_bad_json=bad, substr_lines=substr_lines)
    if substr_lines != len(evs):
        info["anomalies"].append(f"alt-dize '\"event\"' satırı {substr_lines} != tam-JSON olay {len(evs)}")
    if bad or other:
        info["anomalies"].append(f"ayrıştırılamayan={bad} diğer={other}")
    meta = metas[0] if metas else {}
    info["meta_msg_count"] = meta.get("msg_count")
    info["meta_transport"] = meta.get("transport")
    info["meta_profile_id"] = meta.get("profile_id")
    info["meta_kama_arm"] = meta.get("kama_arm")
    info["meta_seed"] = meta.get("seed")
    info["meta_max_deadline_ns"] = meta.get("max_deadline_ns")
    ids = [e.get("msg_id") for e in evs]
    if len(set(ids)) != len(ids):
        info["anomalies"].append("yinelenen msg_id")
    if meta.get("msg_count") is not None and len(evs) != meta["msg_count"]:
        info["anomalies"].append(f"olay sayısı {len(evs)} != meta.msg_count {meta['msg_count']}")
    lat_ns, ok, at_boundary, neg = [], 0, 0, 0
    for e in evs:
        d = e.get("delivered_ns")
        if d is None:
            continue
        L = d - e["sent_ns"]
        if L < 0:
            neg += 1
        if L == D_NS:
            at_boundary += 1
        if L <= D_NS:
            ok += 1
        lat_ns.append(L)
    if neg:
        info["anomalies"].append(f"negatif gecikme {neg}")
    info.update(ok=ok, n_deliv=len(lat_ns), at_boundary=at_boundary)
    info["m1"] = F(ok, len(evs)) if evs else None
    info["m1_over_msgcount"] = F(ok, meta["msg_count"]) if meta.get("msg_count") else None
    if lat_ns:
        s = sorted(lat_ns)
        n = len(s)
        med_ns = F(s[n // 2]) if n % 2 else F(s[n // 2 - 1] + s[n // 2], 2)
        info["m2_ms"] = med_ns / 10**6
        w = sorted(x for x in lat_ns if x <= D_NS)
        if w:
            k = len(w)
            info["m2_withinD_ms"] = (F(w[k // 2]) if k % 2 else F(w[k // 2 - 1] + w[k // 2], 2)) / 10**6
        else:
            info["m2_withinD_ms"] = None
    else:
        info["m2_ms"] = None
        info["m2_withinD_ms"] = None
    # c.err KP-CLIENT delivered=X/Y ile çapraz denetim [betimsel]
    cerr = repdir / "c.err"
    if cerr.exists():
        m = re.search(r"KP-CLIENT delivered=(\d+)/(\d+)", cerr.read_text(errors="replace"))
        if m:
            info["kp_client_delivered"] = (int(m.group(1)), int(m.group(2)))
            if int(m.group(1)) != len(lat_ns):
                info["anomalies"].append(f"KP-CLIENT delivered={m.group(1)} != out.jsonl teslim {len(lat_ns)}")
    info["mtime"] = out.stat().st_mtime
    return info


def median_exact(vals):
    s = sorted(vals)
    n = len(s)
    return s[n // 2] if n % 2 else (s[n // 2 - 1] + s[n // 2]) / 2


# ---------------------------------------------------------------- Mann-Whitney (bağımsız uygulama)
def u2_pairs(x, y):
    """2*U_x çift-sayımla: her (xi, yj) için x>y -> 2, x=y -> 1."""
    t = 0
    for a in x:
        for b in y:
            t += 2 if a > b else (1 if a == b else 0)
    return t


def mwu_null_groups(x, y):
    """Bağ grupları üzerinde null dağılım: {2*U_x: yol sayısı}. Grup j'den a_j tanesi x'e:
    ağırlık prod C(t_j, a_j); 2*R_x = sum a_j * (2*orta-sıra_j); 2U = 2R - m(m+1)."""
    m, n = len(x), len(y)
    vals = sorted(set(x) | set(y))
    pooled = list(x) + list(y)
    groups = []
    start = 1
    for v in vals:
        t = sum(1 for p in pooled if p == v)
        mid2 = start + (start + t - 1)  # 2 * orta-sıra
        groups.append((t, mid2))
        start += t
    dist = {0: {0: 1}}  # k (x'e alınan) -> {2R: sayı}
    for t, mid2 in groups:
        nd = {}
        for k, sums in dist.items():
            for a in range(0, min(t, m - k) + 1):
                w = math.comb(t, a)
                kk = k + a
                bucket = nd.setdefault(kk, {})
                add = a * mid2
                for s, c in sums.items():
                    bucket[s + add] = bucket.get(s + add, 0) + c * w
        dist = nd
    r2 = dist[m]
    off = m * (m + 1)
    out = {}
    for s, c in r2.items():
        out[s - off] = out.get(s - off, 0) + c
    assert sum(out.values()) == math.comb(m + n, m)
    return out, groups


def mwu_exact(x, y):
    """İki yanlı kesin p. Birincil tanım: P(|2U - mn| >= |2U_obs - mn|). İkincil: 2*min(tek kuyruk)."""
    m, n = len(x), len(y)
    null, groups = mwu_null_groups(x, y)
    u2 = u2_pairs(x, y)
    # sıra-toplamı yolu ile tutarlılık
    pooled = sorted(list(x) + list(y))
    rank2 = {}
    i = 0
    while i < len(pooled):
        j = i
        while j + 1 < len(pooled) and pooled[j + 1] == pooled[i]:
            j += 1
        rank2[pooled[i]] = (i + 1) + (j + 1)
        i = j + 1
    r2x = sum(rank2[v] for v in x)
    assert r2x - m * (m + 1) == u2, "U çift-sayım != sıra-toplamı"
    tot = math.comb(m + n, m)
    c = m * n
    dev = abs(u2 - c)
    p_abs = F(sum(k for s, k in null.items() if abs(s - c) >= dev), tot)
    lo = F(sum(k for s, k in null.items() if s <= u2), tot)
    hi = F(sum(k for s, k in null.items() if s >= u2), tot)
    p_dbl = min(F(1), 2 * min(lo, hi))
    return {"p": p_abs, "p_dbl": p_dbl, "U": F(u2, 2), "mn": m * n, "groups": groups, "tot": tot}


def selftest():
    """DP'yi küçük örneklerde kaba-kuvvet permütasyonla doğrula (deterministik örnekler)."""
    cases = [
        ([0, 0, 1, 2], [1, 1, 2, 3, 3]),
        ([F(1, 2), F(1, 2), F(1, 2)], [F(1, 2), 1, 0]),
        ([5, 5, 5, 5], [5, 5, 5]),
        ([0.1, 0.2, 0.3, 0.3, 0.9, 0.0], [0.3, 0.3, 0.0, 0.0, 1.0, 0.5, 0.2]),
        ([1, 2, 3], [4, 5, 6]),
    ]
    for x, y in cases:
        r = mwu_exact(x, y)
        pooled = list(x) + list(y)
        m, n = len(x), len(y)
        c = m * n
        obs = abs(u2_pairs(x, y) - c)
        hit = tot = 0
        for idx in itertools.combinations(range(m + n), m):
            sx = [pooled[i] for i in idx]
            sy = [pooled[i] for i in range(m + n) if i not in idx]
            tot += 1
            if abs(u2_pairs(sx, sy) - c) >= obs:
                hit += 1
        assert F(hit, tot) == r["p"], (x, y, F(hit, tot), r["p"])
    return len(cases)


# ---------------------------------------------------------------- kural
def rule(a_runs, t_runs):
    res = {}
    a1 = [r["m1"] for r in a_runs]
    t1 = [r["m1"] for r in t_runs]
    if a1 and t1:
        d = median_exact(a1) - median_exact(t1)
        mw = mwu_exact(a1, t1)
        res["m1"] = dict(med_a=median_exact(a1), med_t=median_exact(t1), delta=d, mw=mw,
                         kayma=(abs(d) >= M1_THR and mw["p"] < ALPHA),
                         kayma_dbl=(abs(d) >= M1_THR and mw["p_dbl"] < ALPHA),
                         float_kayma=(abs(float(median_exact(a1)) - float(median_exact(t1))) >= 0.05 and float(mw["p"]) < 0.05 / 7))
    a2 = [r["m2_ms"] for r in a_runs if r["m2_ms"] is not None]
    t2 = [r["m2_ms"] for r in t_runs if r["m2_ms"] is not None]
    if a2 and t2:
        ma, mt = median_exact(a2), median_exact(t2)
        ratio = ma / mt if mt > 0 else None
        mw = mwu_exact(a2, t2)
        out = ratio is None or ratio < M2_LO or ratio > M2_HI
        res["m2"] = dict(med_a=ma, med_t=mt, ratio=ratio, mw=mw, n_a=len(a2), n_t=len(t2),
                         kayma=(out and mw["p"] < ALPHA), kayma_dbl=(out and mw["p_dbl"] < ALPHA))
    return res


def load_profile(root, p):
    pd = root / p
    cfg = json.loads((pd / "cell.json").read_text())
    arms, infra, allruns = {}, {}, {}
    for arm in ("taban", "aday"):
        runs = []
        bad = []
        repdirs = sorted([d for d in (pd / arm).iterdir() if d.is_dir() and d.name.startswith("rep")],
                         key=lambda d: int(d.name[3:]))
        for d in repdirs:
            info = parse_run(d)
            info["arm"] = arm
            if not info["has_out"] or info["n_ev"] == 0:
                bad.append(info)
            else:
                runs.append(info)
        arms[arm] = runs
        infra[arm] = bad
        allruns[arm] = repdirs
    return cfg, arms, infra, (pd / "DONE").exists()


# ---------------------------------------------------------------- ANALIZ.txt ayrıştırma
def parse_analiz(path):
    txt = pathlib.Path(path).read_text()
    out = {}
    cur = None
    for ln in txt.splitlines():
        m = re.match(r"## (\S+)\s", ln)
        if m:
            cur = m.group(1)
            out[cur] = {}
            continue
        if cur is None:
            continue
        m = re.match(r"\s+(taban|aday)\s+N=(\d+) altyapı=(\d+)\s+M1 min=([\d.]+) med=([\d.]+) max=([\d.]+) ≥0.99:(\d+)/(\d+)\s+M2 med=(\S+) ms", ln)
        if m:
            out[cur][m.group(1)] = dict(N=int(m.group(2)), infra=int(m.group(3)), min=m.group(4), med=m.group(5),
                                        max=m.group(6), ge99=int(m.group(7)), m2med=m.group(9))
            continue
        m = re.match(r"\s+M1 Δmed=(\S+) p=(\S+) → (.*)$", ln)
        if m:
            out[cur]["M1"] = dict(d=m.group(1), p=m.group(2), v=m.group(3).strip())
            continue
        m = re.match(r"\s+M2 oran=(\S+) p=(\S+) → (.*)$", ln)
        if m:
            out[cur]["M2"] = dict(r=m.group(1), p=m.group(2), v=m.group(3).strip())
            continue
    summ = {}
    sec = txt.split("# ÖZET", 1)[1] if "# ÖZET" in txt else ""
    for ln in sec.splitlines():
        m = re.match(r"\s+(p\d_\S+)\s+(.*)$", ln)
        if m:
            summ[m.group(1)] = m.group(2).strip()
    hk = re.search(r"# HÜKÜM \(ön-kayıtlı kural\): (.*)", txt)
    return out, summ, hk.group(1).strip() if hk else None


def fmt_g4(x):
    return f"{float(x):.4g}"


def main():
    root = pathlib.Path(sys.argv[1])
    analiz = sys.argv[2] if len(sys.argv) > 2 and not sys.argv[2].startswith("--") else None
    detail = "--p2-detail" in sys.argv
    print(f"# BAĞIMSIZ YENİDEN HESAP — {root}")
    print(f"# MWU DP kaba-kuvvet öz-sınaması: {selftest()} örnek, hepsi eşit")
    A = parse_analiz(analiz) if analiz else None
    mismatches = []
    verdicts = {}
    per = {}
    for p in PROFILES:
        cfg, arms, infra, done = load_profile(root, p)
        print(f"\n## {p}  fwd='{cfg['fwd']}' rev='{cfg['rev']}'  taban={cfg['sha_taban'][:8]} aday={cfg['sha_aday'][:8]}  DONE={done}")
        flags = []
        if not done:
            flags.append("EKSİK")
        for arm in ("taban", "aday"):
            runs = arms[arm]
            an = [a for r in runs for a in r["anomalies"]]
            rcs = sorted({r["rc"] for r in runs + infra[arm]})
            nev = sorted({r["n_ev"] for r in runs})
            bnd = sum(r["at_boundary"] for r in runs)
            alt_m1 = sum(1 for r in runs if r["m1_over_msgcount"] != r["m1"])
            ids = sorted({(r["meta_transport"], r["meta_profile_id"], r["meta_kama_arm"]) for r in runs})
            seeds_ok = all(str(r["meta_seed"]) == r["rep"][3:] for r in runs)
            print(f"   {arm:<5} rep_dizini={len(runs) + len(infra[arm])} N={len(runs)} altyapı={len(infra[arm])} "
                  f"rc={rcs} olay_sayısı={nev} D-sınırında={bnd} payda(olay)!=payda(msg_count):{alt_m1} "
                  f"seed=rep:{seeds_ok} kimlik={ids} anomali={an if an else 'yok'}")
            for b in infra[arm]:
                print(f"      ALTYAPI-ARIZASI {b['rep']}: out.jsonl={'var' if b['has_out'] else 'YOK'} olay={b['n_ev']} rc={b['rc']}")
            m1 = sorted(r["m1"] for r in runs)
            m2 = sorted(r["m2_ms"] for r in runs if r["m2_ms"] is not None)
            if m1:
                print(f"         M1 min={float(m1[0]):.3f} med={float(median_exact(m1)):.3f} max={float(m1[-1]):.3f} "
                      f"≥0.99:{sum(1 for v in m1 if v >= F(99, 100))}/{len(m1)}  M2 med={float(median_exact(m2)) if m2 else float('nan'):.0f} ms "
                      f"(M2 N={len(m2)})")
        if max(len(infra["taban"]), len(infra["aday"])) > 2:
            flags.append("ALTYAPI-DUR")
        r = rule(arms["aday"], arms["taban"])
        per[p] = (cfg, arms, infra, r)
        if "m1" in r:
            x = r["m1"]
            print(f"   M1 med_t={float(x['med_t']):.4f} med_a={float(x['med_a']):.4f} Δmed={float(x['delta']):+.4f} (kesin {x['delta']}) "
                  f"U_aday={x['mw']['U']} (mn={x['mw']['mn']}) p={fmt_g4(x['mw']['p'])} [kesin {x['mw']['p']}] "
                  f"p_çift-kuyruk={fmt_g4(x['mw']['p_dbl'])} α={float(ALPHA):.6f} → {'KAYMA' if x['kayma'] else 'kayma yok'}"
                  f"{'' if x['kayma'] == x['kayma_dbl'] == x['float_kayma'] else '  [UYARI: tanım/float hükmü farklı]'}")
            if x["kayma"]:
                flags.append(f"KAYMA-M1({'düşüş' if x['delta'] < 0 else 'artış'})")
        if "m2" in r:
            x = r["m2"]
            print(f"   M2 med_t={float(x['med_t']):.1f} med_a={float(x['med_a']):.1f} oran={float(x['ratio']):.4f} "
                  f"n=({x['n_t']},{x['n_a']}) p={fmt_g4(x['mw']['p'])} p_çift-kuyruk={fmt_g4(x['mw']['p_dbl'])} → "
                  f"{'KAYMA' if x['kayma'] else 'kayma yok'}{'' if x['kayma'] == x['kayma_dbl'] else '  [UYARI: p tanımı hükmü değiştiriyor]'}")
            if x["kayma"]:
                flags.append(f"KAYMA-M2({'yavaşlama' if x['ratio'] > 1 else 'hızlanma'})")
        verdicts[p] = ",".join(flags) if flags else "kayma yok"

        # ---- ANALIZ.txt hücre hücre
        if A:
            ap = A[0].get(p, {})
            for arm in ("taban", "aday"):
                runs = arms[arm]
                m1 = sorted(r["m1"] for r in runs)
                m2 = sorted(r["m2_ms"] for r in runs if r["m2_ms"] is not None)
                mine = dict(N=len(m1), infra=len(infra[arm]), min=f"{float(m1[0]):.3f}", med=f"{float(median_exact(m1)):.3f}",
                            max=f"{float(m1[-1]):.3f}", ge99=sum(1 for v in m1 if v >= F(99, 100)),
                            m2med=f"{float(median_exact(m2)):.0f}")
                theirs = ap.get(arm, {})
                for k, v in mine.items():
                    if theirs.get(k) != v:
                        mismatches.append(f"{p}/{arm}/{k}: ANALIZ={theirs.get(k)} bağımsız={v}")
            if "m1" in r:
                x = r["m1"]
                mine = dict(d=f"{float(x['delta']):+.3f}", p=fmt_g4(x["mw"]["p"]), v="KAYMA" if x["kayma"] else "kayma yok")
                for k, v in mine.items():
                    if ap.get("M1", {}).get(k) != v:
                        mismatches.append(f"{p}/M1/{k}: ANALIZ={ap.get('M1', {}).get(k)} bağımsız={v}")
            if "m2" in r:
                x = r["m2"]
                mine = dict(r=f"{float(x['ratio']):.3f}", p=fmt_g4(x["mw"]["p"]), v="KAYMA" if x["kayma"] else "kayma yok")
                for k, v in mine.items():
                    if ap.get("M2", {}).get(k) != v:
                        mismatches.append(f"{p}/M2/{k}: ANALIZ={ap.get('M2', {}).get(k)} bağımsız={v}")
            if A[1].get(p) != verdicts[p]:
                mismatches.append(f"{p}/ÖZET: ANALIZ={A[1].get(p)} bağımsız={verdicts[p]}")

    print("\n# ÖZET (bağımsız)")
    for p in PROFILES:
        print(f"   {p:<12} {verdicts[p]}")
    anyk = any(v != "kayma yok" for v in verdicts.values())
    hk = "KAYMA/EKSİK VAR → DUR, sahibe rapor" if anyk else "yedi profilde ön-kayıtlı kayma saptanmadı"
    print(f"\n# HÜKÜM (bağımsız, ön-kayıtlı kural): {hk}")
    if A:
        if A[2] != hk:
            mismatches.append(f"HÜKÜM: ANALIZ={A[2]} bağımsız={hk}")
        print(f"\n# ANALIZ.txt KARŞILAŞTIRMA: {len(mismatches)} uyuşmazlık")
        for mm in mismatches:
            print(f"   UYUŞMAZLIK {mm}")

    if detail:
        p2_detail(per["p2_loss80"])


def verdict_m1(a, t):
    d = median_exact(a) - median_exact(t)
    mw = mwu_exact(a, t)
    return abs(d) >= M1_THR and mw["p"] < ALPHA, d, mw["p"]


def p2_detail(pack):
    cfg, arms, infra, r = pack
    print("\n\n######## p2_loss80 AYRINTI ########")
    for arm in ("taban", "aday"):
        rs = sorted(arms[arm], key=lambda q: int(q["rep"][3:]))
        print(f"  {arm} rep sırasıyla (rep:M1[ok/olay] M2ms):")
        print("    " + "  ".join(f"{q['rep'][3:]}:{float(q['m1']):.2f}[{q['ok']}/{q['n_ev']}]"
                                  f"/{(float(q['m2_ms']) if q['m2_ms'] is not None else float('nan')):.0f}" for q in rs))
        print(f"  {arm} sıralı M1: {[f'{float(v):.2f}' for v in sorted(q['m1'] for q in rs)]}")
    x = r["m1"]
    a = [q["m1"] for q in arms["aday"]]
    t = [q["m1"] for q in arms["taban"]]
    print(f"\n  medyan taban={float(x['med_t']):.4f}  aday={float(x['med_a']):.4f}  Δ={float(x['delta']):+.4f}")
    print(f"  U_aday={x['mw']['U']}  U_taban={x['mw']['mn'] - x['mw']['U']}  mn={x['mw']['mn']}  "
          f"Pr(aday>taban)+½Pr(=)={float(x['mw']['U'] / x['mw']['mn']):.4f}")
    print(f"  p (|2U-mn| tanımı) = {x['mw']['p']} = {float(x['mw']['p']):.6g}")
    print(f"  p (2×min tek-kuyruk) = {float(x['mw']['p_dbl']):.6g}")
    print(f"  α = 1/140 = {float(ALPHA):.6g};  p/α = {float(x['mw']['p'] / ALPHA):.4f}")
    # bağ yapısı
    vals = sorted(set(a) | set(t))
    ties = [(v, sum(1 for z in t if z == v), sum(1 for z in a if z == v)) for v in vals]
    multi = [(f"{float(v):.2f}", ct, ca) for v, ct, ca in ties if ct + ca > 1]
    print(f"  ayrık değer sayısı={len(vals)} / 40; bağlı gruplar (değer, taban#, aday#): {multi}")
    # bağsız varsayımla p (bağ düzeltmesi etkisi) [betimsel]
    # sıra-toplamı aynı kalırken bağsız dağılım: tüm değerleri ayrık kabul edip U'yu tam sayıya yuvarlamak yerine
    # normal yaklaşık (bağ-düzeltmeli ve düzeltmesiz) yalnız bağlam için
    m = n = 20
    N = m + n
    U = float(x["mw"]["U"])
    tie_term = sum((tc + ac) ** 3 - (tc + ac) for _, tc, ac in ties)
    var_corr = m * n / 12 * ((N + 1) - tie_term / (N * (N - 1)))
    var_nc = m * n * (N + 1) / 12
    z_c = (abs(U - m * n / 2) - 0.5) / math.sqrt(var_corr)
    z_n = (abs(U - m * n / 2) - 0.5) / math.sqrt(var_nc)
    pn = lambda z: math.erfc(z / math.sqrt(2))
    print(f"  [betimsel] normal yaklaşık (süreklilik düz.): bağ-düz. p≈{pn(z_c):.4g}; bağ-düz.-siz p≈{pn(z_n):.4g}")

    # tek-koşu duyarlılığı: her koşunun M1'i k/100 (k=0..100) değerine taşınırsa hüküm
    print("\n  TEK-KOŞU DUYARLILIĞI (her koşu tek başına 0..1 aralığında 0.01 adımlı her değere taşınır):")
    best = None
    flips_1msg = []
    for arm in ("taban", "aday"):
        for qi, q in enumerate(arms[arm]):
            n_ev = q["n_ev"]
            minflip = None
            for k in range(0, n_ev + 1):
                nv = F(k, n_ev)
                if nv == q["m1"]:
                    continue
                aa = [z["m1"] for z in arms["aday"]]
                tt = [z["m1"] for z in arms["taban"]]
                (aa if arm == "aday" else tt)[qi] = nv
                kv, d, pp = verdict_m1(aa, tt)
                if not kv:
                    ch = abs(nv - q["m1"])
                    if minflip is None or ch < minflip[0]:
                        minflip = (ch, nv, pp, d)
                    if ch == F(1, n_ev):
                        flips_1msg.append((arm, q["rep"], float(q["m1"]), float(nv), float(pp), float(d)))
            if minflip and (best is None or minflip[0] < best[0]):
                best = (minflip[0], arm, q["rep"], q["m1"], minflip[1], minflip[2], minflip[3])
            q["_minflip"] = minflip
    for arm in ("taban", "aday"):
        rs = sorted(arms[arm], key=lambda q: q["m1"])
        print(f"   {arm}: " + "  ".join(
            f"{q['rep']}({float(q['m1']):.2f})→" + (f"{float(q['_minflip'][1]):.2f}[|Δ|={float(q['_minflip'][0]):.2f},p={float(q['_minflip'][2]):.4g}]"
                                                    if q["_minflip"] else "çevrilemez") for q in rs))
    if best:
        print(f"  EN KÜÇÜK ÇEVİREN TEK DEĞİŞİKLİK: {best[1]} {best[2]} M1 {float(best[3]):.2f} → {float(best[4]):.2f} "
              f"(|Δ|={float(best[0]):.2f} = {int(best[0] * 100)} mesaj), yeni p={float(best[5]):.5g}, yeni Δmed={float(best[6]):+.3f}")
    print(f"  1 mesajlık (|Δ|=0.01) değişiklikle hükmü çeviren koşu sayısı: {len(flips_1msg)}")
    for f_ in flips_1msg[:40]:
        print(f"     {f_}")

    # leave-one-out [betimsel]
    print("\n  [betimsel] TEK-KOŞU ÇIKARMA (leave-one-out) p ve hüküm:")
    for arm in ("taban", "aday"):
        row = []
        for qi, q in enumerate(arms[arm]):
            aa = [z["m1"] for z in arms["aday"]]
            tt = [z["m1"] for z in arms["taban"]]
            (aa if arm == "aday" else tt).pop(qi)
            kv, d, pp = verdict_m1(aa, tt)
            row.append((q["rep"], float(pp), kv))
        mx = max(row, key=lambda z: z[1])
        mn = min(row, key=lambda z: z[1])
        nflip = sum(1 for z in row if not z[2])
        print(f"   {arm} çıkarılınca: p aralığı [{mn[1]:.4g} ({mn[0]}), {mx[1]:.4g} ({mx[0]})]; hükmü çeviren çıkarma sayısı={nflip}/20")

    # kol sırası [betimsel]
    print("\n  [betimsel] KOL SIRASI (tek rep: aday önce; çift rep: taban önce — netem7_cell.sh:103-107):")
    for arm in ("taban", "aday"):
        first = [float(q["m1"]) for q in arms[arm] if (int(q["rep"][3:]) % 2 == 1) == (arm == "aday")]
        second = [float(q["m1"]) for q in arms[arm] if (int(q["rep"][3:]) % 2 == 1) != (arm == "aday")]
        print(f"   {arm}: rep içinde ilk koşan medyan={median_exact(first):.3f} (n={len(first)}), ikinci koşan medyan={median_exact(second):.3f} (n={len(second)})")
    # zaman sırası [betimsel]
    allr = sorted(arms["taban"] + arms["aday"], key=lambda q: q["mtime"])
    half = len(allr) // 2
    for lab, seg in (("ilk yarı", allr[:half]), ("ikinci yarı", allr[half:])):
        for arm in ("taban", "aday"):
            v = [float(q["m1"]) for q in seg if q["arm"] == arm]
            print(f"   {lab} {arm}: n={len(v)} medyan={median_exact(v):.3f}")


if __name__ == "__main__":
    main()
