#!/usr/bin/env python3
"""[FEC-MARGINAL-R-V1] Ön-kayıtlı analiz. Sözleşme: reports/fec-marginal-r-v1/PRE-REGISTRATION.md

Kurallar koda GÖMÜLÜ (yoruma bırakılmaz):
  · birim = REP (mesajlar rep içinde korelasyonlu; çöküş 96'yı birden götürür)
  · kollar EŞLEŞTİRİLMEMİŞ (kernel-rastgele netem) → eşli test YOK
  · iki parçalı tahmin edici: çöküş oranı (Clopper-Pearson) + koşullu completion (bootstrap)
  · geçersiz rep VERİ NOKTASI DEĞİLDİR; sayısı ve sebebi raporlanır
  · birincil uç nokta ALICI-taraflı teslim; istemci ACK'i ikincil
  · deadline burada POST-HOC uygulanır (ham gecikmeler out.jsonl'de)
Deterministik: bootstrap seed = SHA256(tag‖cell‖arm); RNG kütüphanesi yok, kendi PRNG'i.
"""
import hashlib
import json
import math
import os
import re
import sys
from collections import defaultdict

TAG = "FEC-MARGINAL-R-V1"
B = 10_000
COLLAPSE = 0.5


def lcg(seed_bytes):
    s = int.from_bytes(hashlib.sha256(seed_bytes).digest()[:8], "big") | 1
    while True:
        s = (s * 6364136223846793005 + 1442695040888963407) & ((1 << 64) - 1)
        yield (s >> 11) / float(1 << 53)


def clopper_pearson(k, n, alpha=0.05):
    """Kesin binom %95 aralık. n küçükken normal-yaklaşım GEÇERSİZDİR."""
    if n == 0:
        return (0.0, 1.0)
    lo = 0.0 if k == 0 else _beta_inv(alpha / 2, k, n - k + 1)
    hi = 1.0 if k == n else _beta_inv(1 - alpha / 2, k + 1, n - k)
    return (lo, hi)


def _beta_inv(p, a, b, it=200):
    lo, hi = 0.0, 1.0
    for _ in range(it):
        m = (lo + hi) / 2
        if _beta_cdf(m, a, b) < p:
            lo = m
        else:
            hi = m
    return (lo + hi) / 2


def _beta_cdf(x, a, b, n=2000):
    if x <= 0:
        return 0.0
    if x >= 1:
        return 1.0
    # Basit Simpson; a,b küçük tamsayılar olduğundan yeterli ve deterministik.
    h = x / n
    lnB = math.lgamma(a) + math.lgamma(b) - math.lgamma(a + b)

    def f(t):
        if t <= 0 or t >= 1:
            return 0.0
        return math.exp((a - 1) * math.log(t) + (b - 1) * math.log1p(-t) - lnB)

    s = f(0) + f(x)
    for i in range(1, n):
        s += f(i * h) * (4 if i % 2 else 2)
    return min(1.0, max(0.0, s * h / 3))


def boot_ci(vals, seed_bytes):
    if len(vals) < 2:
        return (float("nan"), float("nan"))
    rng = lcg(seed_bytes)
    n = len(vals)
    means = []
    for _ in range(B):
        means.append(sum(vals[int(next(rng) * n) % n] for _ in range(n)) / n)
    means.sort()
    return (means[int(0.025 * B)], means[int(0.975 * B)])


def srv_int(txt, key):
    m = re.search(rf"{key}:\s*(\d+)", txt)
    return int(m.group(1)) if m else None


def load_rep(d):
    """Bir rep'i yükle → (valid, reason, record)."""
    st = open(os.path.join(d, "rep_status")).read().strip() if os.path.exists(os.path.join(d, "rep_status")) else "MISSING"
    if st != "COMPLETED":
        return False, st, None
    oj = os.path.join(d, "out.jsonl")
    se = os.path.join(d, "s.err")
    ce = os.path.join(d, "c.err")
    if not (os.path.exists(oj) and os.path.exists(se)):
        return False, "NO_ARTIFACT", None
    meta, evs = None, []
    for ln in open(oj):
        ln = ln.strip()
        if not ln:
            continue
        o = json.loads(ln)
        if "meta" in o:
            meta = o["meta"]
        elif "event" in o:
            evs.append(o["event"])
    s = open(se).read()
    c = open(ce).read() if os.path.exists(ce) else ""
    rec = {
        "meta": meta,
        "events": evs,
        "srv_delivered": srv_int(s, "delivered"),
        "msgs_reassembled": srv_int(s, "msgs_reassembled"),
        "fec_repairs_rx": srv_int(s, "fec_repairs_rx"),
        "fec_recovered_units": srv_int(s, "fec_recovered_units"),
        "msg_asm_evicted": srv_int(s, "msg_asm_evicted"),
        "frag_chunk_dropped": srv_int(s, "frag_chunk_dropped"),
        "left_preconverge": "left_preconv=true" in c,
        "fec_repairs_tx": int(re.search(r"fec_repairs_tx=(\d+)", c).group(1)) if re.search(r"fec_repairs_tx=(\d+)", c) else 0,
        "fec_repair_missing": int(re.search(r"fec_repair_missing=(\d+)", s).group(1)) if re.search(r"fec_repair_missing=(\d+)", s) else 0,
        "stale": int(re.search(r"stale=(\d+)", c).group(1)) if re.search(r"stale=(\d+)", c) else 0,
    }
    # §7 geçerlilik kapıları
    if rec["srv_delivered"] is None:
        return False, "NO_SERVER_STATS", rec
    if not rec["left_preconverge"]:
        return False, "NEVER_ESTABLISHED", rec
    if rec["msg_asm_evicted"]:
        return False, "MSG_ASM_EVICTED", rec
    if rec["frag_chunk_dropped"]:
        return False, "FRAG_CHUNK_DROPPED", rec
    return True, "OK", rec


def completion_at(rec, d_msg_ns, n_msgs):
    """Post-hoc deadline: mesaj-başı (delivered - sent) ≤ d_msg."""
    hit = sum(
        1
        for e in rec["events"]
        if e.get("delivered_ns") is not None and (e["delivered_ns"] - e["sent_ns"]) <= d_msg_ns
    )
    return hit / n_msgs


def main():
    root = sys.argv[1]
    rtt_ms = float(sys.argv[2]) if len(sys.argv) > 2 else 200.0
    primary_mult = 4
    grid = [1, 2, 4, 8, 16]
    cells = sorted(d for d in os.listdir(root) if os.path.isdir(os.path.join(root, d)))
    print(f"# {TAG}  RTT={rtt_ms} ms  birincil d_msg = {primary_mult}×RTT = {primary_mult*rtt_ms:.0f} ms")
    print(f"# birim = REP · kollar eşleştirilmemiş · çöküş eşiği = completion < {COLLAPSE}")

    data = defaultdict(list)
    invalid = defaultdict(lambda: defaultdict(int))
    # KEŞFEDİLEN tüm (hücre, kol) çiftleri — veri sıfır olsa BİLE raporlanır. Bir kolun çıktıdan
    # sessizce kaybolması, bu reponun defterindeki "başarısız repler sessizce kaybolur ve kayıp
    # KOLA BAĞLIDIR" kusurunun ta kendisidir; ilk sürümümde tam bu oldu (r0 kolu hiç görünmedi).
    seen = []
    for cell in cells:
        for arm in sorted(os.listdir(os.path.join(root, cell))):
            ad = os.path.join(root, cell, arm)
            if not os.path.isdir(ad):
                continue
            seen.append((cell, arm))
            for rep in sorted(os.listdir(ad)):
                ok, why, rec = load_rep(os.path.join(ad, rep))
                if not ok:
                    invalid[(cell, arm)][why] += 1
                    continue
                data[(cell, arm)].append(rec)

    # Kol canlılığı (§7.5/7.6) — VAKUM KOL REDDİ
    print("\n## kol canlılığı ve geçerlilik")
    for (cell, arm) in sorted(seen):
        recs = data.get((cell, arm), [])
        if not recs:
            print(f"  {cell:6s} {arm:3s} n_valid=  0  ** KOL TAMAMEN GEÇERSİZ ** "
                  f"geçersiz={dict(invalid[(cell, arm)])}")
    for (cell, arm), recs in sorted(data.items()):
        r = int(arm.lstrip("r"))
        tx = sum(x["fec_repairs_tx"] for x in recs)
        rx = sum(x["fec_repairs_rx"] or 0 for x in recs)
        recov = sum(x["fec_recovered_units"] or 0 for x in recs)
        miss = sum(x["fec_repair_missing"] for x in recs)
        inv = dict(invalid[(cell, arm)])
        flag = ""
        if r == 0 and tx > 0:
            flag = "  ** KOL FARKLILAŞMASI İHLALİ: r=0 parity bastı **"
        if r > 0 and (tx == 0 or rx == 0):
            flag = "  ** VAKUM KOL: parity yok/ulaşmadı → FEC kolu SAYILMAZ **"
        if r > 0 and miss > 0:
            flag += "  ** repair_missing>0: boş-symbol çerçeveler **"
        print(f"  {cell:6s} {arm:3s} n_valid={len(recs):3d} geçersiz={inv} "
              f"repairs_tx={tx} repairs_rx={rx} recovered={recov} miss={miss}{flag}")

    # Birincil uç nokta
    print(f"\n## birincil: ALICI teslim @ d_msg={primary_mult}×RTT")
    n_msgs = 96
    d_primary = int(primary_mult * rtt_ms * 1e6)
    summary = {}
    for (cell, arm), recs in sorted(data.items()):
        comps = [completion_at(x, d_primary, n_msgs) for x in recs]
        srv = [(x["srv_delivered"] or 0) / n_msgs for x in recs]
        ncol = sum(1 for c in comps if c < COLLAPSE)
        lo, hi = clopper_pearson(ncol, len(comps)) if comps else (float("nan"),) * 2
        good = [c for c in comps if c >= COLLAPSE]
        blo, bhi = boot_ci(good, f"{TAG}|{cell}|{arm}".encode())
        bytes_per = [
            (x["meta"]["udp_tx_bytes"] / max(1, x["srv_delivered"] or 1)) for x in recs
        ]
        summary[(cell, arm)] = (good, (blo, bhi), (ncol, len(comps), lo, hi))
        print(f"  {cell:6s} {arm:3s} n={len(comps):3d}  çöküş={ncol}/{len(comps)} "
              f"CP95=[{lo:.2f},{hi:.2f}]  koşullu-completion="
              f"{(sum(good)/len(good) if good else float('nan')):.3f} "
              f"boot95=[{blo:.3f},{bhi:.3f}]  srv_deliv={sum(srv)/max(1,len(srv)):.3f} "
              f"byte/teslim={sum(bytes_per)/max(1,len(bytes_per)):.0f}")

    # Güç: rep-düzeyi sd ve %80-güç MDE (ön-kayıt §8 birincil satır)
    print("\n## güç (rep-düzeyi sd → %80-güç MDE, kabaca 2.8·sd·sqrt(2/n))")
    for (cell, arm), (good, _, (ncol, ntot, _, _)) in sorted(summary.items()):
        if len(good) >= 2:
            m = sum(good) / len(good)
            sd = math.sqrt(sum((g - m) ** 2 for g in good) / (len(good) - 1))
            mde = 2.8 * sd * math.sqrt(2 / max(1, len(good)))
            print(f"  {cell:6s} {arm:3s} sd={sd:.4f} n={len(good)} → MDE≈{mde*100:.1f} pp")

    # Deadline eğrisi (post-hoc, §5)
    print("\n## completion-vs-deadline (post-hoc, d_msg = k×RTT)")
    hdr = "  hücre  kol " + " ".join(f"{k:>7d}×" for k in grid)
    print(hdr)
    for (cell, arm), recs in sorted(data.items()):
        row = []
        for k in grid:
            d = int(k * rtt_ms * 1e6)
            cs = [completion_at(x, d, n_msgs) for x in recs]
            row.append(f"{(sum(cs)/len(cs) if cs else float('nan')):8.3f}")
        print(f"  {cell:6s} {arm:3s} " + " ".join(row))


if __name__ == "__main__":
    main()
