#!/usr/bin/env python3
"""[YOL3 · Ö] E1 teşhis okuması (KK-Ö-10): donmuş manifestin S, R1, R3, R4 kuralları AYNEN.

    python3 -I scripts/yol3/tesis/oku.py --kok <teşhis kökü> [--out <dizin>] [--manifest <md>]
    python3 -I scripts/yol3/tesis/oku.py --oz-test

Manifest: docs/design/E1-TESHIS-G2.md (yol3/m2-e1-bayrak @ 41b41d21), sha256 MANIFEST_SHA256.
`--manifest` verilirse dosyanın sha256'sı denetlenir; farklıysa okuma YAPILMAZ (çıkış 3).
Kural metni bu dosyada değiştirilmez; eşik eklenmez. Ölçülerin tanımı (hangi analiz çıktısı)
aşağıdaki TANIMLAR'dadır — her biri mevcut F0 analizörünün ya da ACIL betiğinin alanıdır:

  (i)  dosya bitiş   f0a_analyze `file.bob_received_ms` (bob dosya tamam − alice send_file), ms;
                     f0a_tam `run_row` `file_bob_ms`. Alınmayan dosya = +∞ (medyan sansürlü).
  (ii) PTT alıcı p95 f0a_tam `run_row` `P_p95` (iki basış birlikte; nearest-rank; teslim
                     edilmeyen demet sansürlü), ms; gösterim analoğu hücresi.
  (iii) emisyon/sunulan  pay ACIL #15 tanımı AYNEN: alice `ts` satırlarında eş `bob` oturumunun
                     pacer hızı `pr` (Idle örnekleri hariç; acil-betikler/v-s3/a_ts.py), koşu
                     başına medyan, mühürlü UDP yükü B/s (pacer jetonu `sealed_len(HDR_DATA +
                     birim)` ile düşer: client-core runtime.rs `wire_len`). Payda AYNI katmanda
                     (KK-ORK-03; 640 B/s L2 DEĞİL): hücrenin kendi zaman çizelgesinden
                     `sunulan_ptt_bps` = (ptt_bytes + 5 + 8 [birim çerçevesi, f0a_analyze
                     unit_len P] + 11 [HDR_DATA, client-core wire.rs] + 22 [mühür zarfı,
                     f0a_analyze :51 / RAPOR-A §3 128 B tel]) B × demet ÷ basış süresi (demet ×
                     ptt_bundle_ms) — dosyasız hücrede 86 B / 0,2 s = 430 B/s. Dosyasız hücrede
                     ölçülmeyen K1 ve K2−e için K2'nin sonucu (manifest §2).
  R1   gönderim/gereken  ACIL srate.py `S(d, 3, 28)` (capb_cli 'o' satırları ip_len toplamı,
                     [3, 28) s, IP B/s) ÷ gereken = 3702/(1 − p) B/s (acil-betikler/v-s3/bins.py
                     `off`; p = hücrenin kaybı). Tuzaksız ⇔ 0 < left_preconv ≤ 10 000 ms (0 =
                     PreConverge'den hiç çıkılmadı = tuzak).
  R3   d1000 l0 p95  analiz_b `conf_p95_s`; r25 teslim@60 = `deliv_D` × `n_msgs` (adet),
                     gönderim = `tx_total_B` (KP-CLIENT tx, UDP yükü). Kıyas kol medyanlarıyla.
  R4   sw95 = analiz_b `sender_wait_p95_s`; pace = koşu sonu `est_pace` (B), A'da `pr` medyanı;
                     EAI = 2·U/pace + 65/31 s, kayıp eşiği = max(9/8·srtt, 1 ms, 1,5·EAI)
                     (acil-betikler/v-s4-b/floor.py; sender.rs expected_ack_interval /
                     cadence_loss_floor), U = 1013 B; RTprop = `est_rtprop` (B), A'da son `rtp`;
                     diag_probe aynada yok → `probe_up` (B) / son `pu` (A) raporlanır; Startup
                     dönemi kaybı [0, 20) s: araç yok → "ölçülmedi".

Bütün medyanlar koşu (rep) değerleri üzerindendir (birim = koşu). Veri eksik kol uygun sayılmaz.
"""

import argparse
import gzip
import hashlib
import json
import math
import os
import pathlib
import statistics
import subprocess
import sys
import tempfile

sys.dont_write_bytecode = True  # F0 analizörleri içe aktarılır; depoya __pycache__ yazılmaz
HERE = pathlib.Path(__file__).resolve().parent
REPO = HERE.parents[2]
F0 = REPO / "scripts" / "ktp2" / "f0"
MANIFEST_SHA256 = "37bcabe31107a464970a54f5073a36d1d0e65e1cba558c1bf363360c0cb56e1a"

ANALOG = "a_r12k_d150_l20_foto150k"
ANALOG_L30 = "a_r12k_d300_l30_dosyali"
DOSYASIZ = "a_r12k_d300_l30_dosyasiz"
MANSET = "b_r12k_d1000_l30_p1000"
L0 = "b_r12k_d1000_l0_p1000"
R25 = "b_r25_d300_l20_p65"

E1_ARMS = ["K1", "K2", "K2mc", "K2me"]
ALL_ARMS = ["K0", "K0r"] + E1_ARMS
FLAGS = {"K0": 0, "K0r": 1, "K1": 4, "K2": 5, "K2mc": 4, "K2me": 4}
SHOW = {"K2mc": "K2−c", "K2me": "K2−e"}
# KK-ORK-03: (iii) paydası pr ile aynı katmanda (mühürlü UDP yükü). Demeti taşıyan birimin ek
# baytları: birim çerçevesi 5 + 8 (f0a_analyze unit_len "P"), DATA başlığı 11 (client-core
# wire.rs HDR_DATA; FEC kapalı → parça başlığı yok, KK-ORK-01), mühür zarfı 22 (f0a_analyze :51).
PTT_UNIT_FRAMING = 5 + 8
DATA_HDR = 11
SEAL_OVERHEAD = 22
OFFERED_IP_BPS = 3702.0    # acil-betikler/v-s3/bins.py `off` (r12k, 1000 B, 285 ms)
U_UNIT = 1013              # floor.py: p1000 → 1013
INF = math.inf


def show(arm):
    return SHOW.get(arm, arm)


def med(xs):
    xs = [x for x in xs if x is not None]
    return statistics.median(xs) if xs else None


# ── Kural katmanı (saf; öz-testin konusu) ─────────────────────────────────────────────────────

def m_of(d, cell, arm, key):
    return med((d.get(cell, {}).get(arm) or {}).get(key, []))


def r3(d):
    """R3 (bağlayıcı): d1000 l0 p95 ve r25; K2'de koşulur, seçilen sete de uygulanır."""
    k0, k2 = m_of(d, L0, "K0", "p95"), m_of(d, L0, "K2", "p95")
    l0 = None if k0 is None or k2 is None else k2 <= k0 * 1.10
    t0, t2 = m_of(d, R25, "K0", "teslim"), m_of(d, R25, "K2", "teslim")
    x0, x2 = m_of(d, R25, "K0", "tx"), m_of(d, R25, "K2", "tx")
    if None in (t0, t2, x0, x2):
        r = None
    else:
        r = t2 >= t0 - 1 and x2 <= x0 * 1.10
    ok = bool(l0) and bool(r)
    return {"l0": l0, "r25": r, "gecer": ok,
            "l0_med": {"K0": k0, "K2": k2},
            "r25_med": {"K0": (t0, x0), "K2": (t2, x2)}}


def select(d):
    """S (bağlayıcı) + geri dönüş. Döner: (kol, ayrıntı)."""
    a = d.get("analog", ANALOG)
    file_m = {k: m_of(d, a, k, "file") for k in ALL_ARMS}
    ptt_m = {k: m_of(d, a, k, "ptt") for k in ALL_ARMS}
    ratio_m = {k: m_of(d, DOSYASIZ, k, "ratio") for k in ("K0", "K2", "K2mc")}
    ratio_m["K1"] = ratio_m["K2me"] = ratio_m["K2"]   # manifest §2: K2'nin sonucu
    R3 = r3(d)
    f0, p0 = file_m["K0"], ptt_m["K0"]
    rows = {}
    for k in E1_ARMS:
        c1 = None if f0 is None or file_m[k] is None else file_m[k] <= f0 * 1.10
        c2 = None if p0 is None or ptt_m[k] is None else ptt_m[k] <= p0 * 1.10
        c3 = None if ratio_m[k] is None else ratio_m[k] >= 0.95
        rows[k] = {"i": c1, "ii": c2, "iii": c3, "iv": R3["gecer"],
                   "uygun": bool(c1) and bool(c2) and bool(c3) and R3["gecer"]}
    ok = [k for k in E1_ARMS if rows[k]["uygun"]]
    if ok:
        best = min(ptt_m[k] for k in ok)
        band = [k for k in ok if ptt_m[k] <= best * 1.10]
        band.sort(key=lambda k: (FLAGS[k], file_m[k], ptt_m[k], k))
        choice, why = band[0], f"uygun {ok}; en düşük PTT %10 bandı {band}"
    else:
        c1 = (None if f0 is None or file_m["K0r"] is None else file_m["K0r"] <= f0 * 1.10)
        c2 = (None if p0 is None or ptt_m["K0r"] is None else ptt_m["K0r"] <= p0 * 0.90)
        if c1 and c2:
            choice, why = "K0r", "hiçbir E1 kolu uygun değil; K0r: PTT ≤ K0×0,90 ∧ (i)"
        else:
            choice, why = "K0", "hiçbir E1 kolu uygun değil; K0r şartı sağlanmadı → K0"
        rows["K0r"] = {"i": c1, "ii(0,90)": c2}
    return choice, {"neden": why, "satirlar": rows, "file_med": file_m, "ptt_med": ptt_m,
                    "ratio_med": ratio_m, "R3": R3}


def r1(d):
    """R1 (raporlanan): manşette tuzaksız gönderim/gereken medyanı ≥ 0,90; <7 tuzaksız → bütün."""
    out = {}
    for k in ("K0", "K2", "K2mc"):
        runs = (d.get(MANSET, {}).get(k) or {}).get("runs", [])
        allv = [r["sr"] for r in runs if r.get("sr") is not None]
        clean = [r["sr"] for r in runs
                 if r.get("sr") is not None and r.get("lpc") and 0 < r["lpc"] <= 10_000]
        used = clean if len(clean) >= 7 else allv
        m_used, m_all = med(used), med(allv)
        out[k] = {"tuzaksiz_n": len(clean), "n": len(allv), "kullanilan": "tuzaksız"
                  if len(clean) >= 7 else "bütün (tuzaksız < 7)", "med": m_used,
                  "med_butun": m_all, "gecer": None if m_used is None else m_used >= 0.90}
    return out


def r4(d):
    """R4 (raporlanan, kapı değil)."""
    out = {"manset": {}, "analog": {}}
    for k in ("K0", "K2", "K2mc"):
        e = d.get(MANSET, {}).get(k) or {}
        sw = sorted(x for x in e.get("sw95", []) if x is not None)
        pace, srtt = med(e.get("pace", [])), med(e.get("srtt", []))
        out["manset"][k] = {"sw95_med": med(sw), "sw95_9": sw[8] if len(sw) >= 9 else None,
                            "n": len(sw), **eai(pace, srtt),
                            "rtprop_med": med(e.get("rtprop", [])),
                            "probe_up_med": med(e.get("pu", []))}
    a = d.get("analog", ANALOG)
    for k in ALL_ARMS:
        e = d.get(a, {}).get(k) or {}
        out["analog"][k] = {**eai(med(e.get("pace", [])), med(e.get("srtt", []))),
                            "rtprop_med": med(e.get("rtprop", [])),
                            "probe_up_med": med(e.get("pu", []))}
    for part in ("manset", "analog"):
        base = out[part].get("K0", {}).get("rtprop_med")
        for k, v in out[part].items():
            v["rtprop_K0_orani"] = (v["rtprop_med"] / base) if (base and v["rtprop_med"]) else None
    out["startup_kaybi_0_20"] = "ölçülmedi (araç yok; capture eşlemesi gerekir)"
    out["diag_probe"] = "ölçülmedi (aynada yok); probe_up_entries raporlanır"
    return out


def eai(pace, srtt):
    if not pace:
        return {"pace_med": pace, "srtt_med": srtt, "eai_s": None, "kayip_esigi_s": None}
    e = 2 * U_UNIT / pace + 65 / 31
    thr = max(9 / 8 * srtt, 0.001, 1.5 * e) if srtt is not None else None
    return {"pace_med": pace, "srtt_med": srtt, "eai_s": e, "kayip_esigi_s": thr}


def fmt(x, nd=2):
    if x is None:
        return "—"
    if x == INF:
        return "∞"
    return f"{x:.{nd}f}"


def tek_satir(choice, R1):
    return (f"Gösterim seti: {show(choice)}. E1 (manşet, tuzaksız): "
            f"K2 {fmt(R1['K2']['med'])}, K2−c {fmt(R1['K2mc']['med'])}.")


# ── Çıkarım katmanı (F0 analizörleri + ACIL tanımları) ────────────────────────────────────────

def sunulan_ptt_bps(params):
    """KK-ORK-03: PTT sunulan yükü, `pr`'nin katmanında (mühürlü UDP yükü B/s): demet başına
    (ptt_bytes + birim çerçevesi + DATA başlığı + mühür zarfı) bayt, basış süresine bölünür
    (basış süresi = demet × ptt_bundle_ms). `params` = f0_flows hdr `params`."""
    n, ms = params["ptt_bundles"], params["ptt_bundle_ms"]
    if not n or not ms:
        return None
    per = params["ptt_bytes"] + PTT_UNIT_FRAMING + DATA_HDR + SEAL_OVERHEAD
    return n * per / (n * ms / 1000.0)


def hdr_params(alice_jsonl):
    """f0_flows koşu başlığının `params` nesnesi (ilk `hdr` satırı)."""
    try:
        with open(alice_jsonl, encoding="utf-8") as f:
            for ln in f:
                if '"k":"hdr"' in ln:
                    return json.loads(ln).get("params")
    except (OSError, ValueError):
        return None
    return None


def acil_pacer_med(alice_jsonl):
    """acil-betikler/v-s3/a_ts.py: eş `bob`, Idle hariç `pr`; burada koşu başına medyan."""
    pr, srtt, rtp, pu = [], [], None, None
    try:
        f = open(alice_jsonl, encoding="utf-8")
    except OSError:
        return None, None, None, None
    with f:
        for ln in f:
            if '"k":"ts"' not in ln:
                continue
            j = json.loads(ln)
            for s in j.get("s", []):
                if s.get("peer") != "bob":
                    continue
                if s.get("rtp") is not None:
                    rtp = s["rtp"]
                if s.get("pu") is not None:
                    pu = s["pu"]
                if s.get("st") == "Idle":
                    continue
                pr.append(s["pr"])
                if s.get("srtt") is not None:
                    srtt.append(s["srtt"] / 1000.0)
    return med(pr), med(srtt), rtp, pu


def acil_srate(rep_dir, a=3, b=28):
    """acil-betikler/v-s3/srate.py `S` aynen (BASE=t0)."""
    try:
        with open(os.path.join(rep_dir, "out.jsonl"), encoding="utf-8") as f:
            z = json.loads(f.readline())["meta"]["t0_unix_ns"]
    except (OSError, KeyError, ValueError):
        return None
    tot = 0
    try:
        with gzip.open(os.path.join(rep_dir, "capb_cli.csv.gz"), "rt") as f:
            for ln in f:
                if ln.startswith("#"):
                    continue
                p = ln.split(",", 6)
                if p[1] != "o":
                    continue
                if a <= (int(p[0]) - z) / 1e9 < b:
                    tot += int(p[3])
    except OSError:
        return None
    return tot / (b - a)


def cells_of(kok, prefix):
    for cd in sorted(p for p in kok.iterdir() if p.is_dir() and p.name.startswith(prefix)
                     and "__" in p.name):
        base, arm = cd.name.split("__", 1)
        yield cd, base, arm


def extract(kok, work):
    d = {}
    # A: f0a_analyze (kök başına), sonra f0a_tam.run_row her koşuya.
    adir = work / "A"
    subprocess.run([sys.executable, "-I", str(F0 / "a" / "f0a_analyze.py"), "--out", str(adir),
                    str(kok)], check=True, stdout=subprocess.DEVNULL)
    sys.path.insert(0, str(F0 / "a"))
    import f0a_tam  # noqa: E402
    for p in sorted((adir / "runs").glob("*.json")):
        rj = json.loads(p.read_text(encoding="utf-8"))
        cell, arm = rj["cell"], rj["arm"]
        if "__" not in cell:
            continue
        base = cell.split("__", 1)[0]
        row = f0a_tam.run_row(rj, str(p).replace(os.sep + "runs" + os.sep,
                                                  os.sep + "items" + os.sep)
                              .replace(".json", ".jsonl"))
        pace, srtt, rtp, pu = acil_pacer_med(os.path.join(rj["dir"], "alice.jsonl"))
        prm = hdr_params(os.path.join(rj["dir"], "alice.jsonl"))
        offered = sunulan_ptt_bps(prm) if prm else None
        if offered is not None:
            d.setdefault("sunulan_bps", {}).setdefault(base, set()).add(offered)
        e = d.setdefault(base, {}).setdefault(arm, {k: [] for k in
                                                    ("file", "ptt", "ratio", "pace", "srtt",
                                                     "rtprop", "pu")})
        fb = row.get("file_bob_ms")
        e["file"].append(fb / 1000.0 if fb is not None else INF)
        e["ptt"].append(row["P_p95"] / 1000.0 if row.get("P_p95") is not None else None)
        e["ratio"].append(pace / offered if (pace is not None and offered) else None)
        e["pace"].append(pace)
        e["srtt"].append(srtt)
        e["rtprop"].append(rtp / 1000.0 if rtp is not None else None)
        e["pu"].append(pu)
    # B: analiz_b.analyze_run (main sonekli hücreyi atlar; fizik geometrisi taban adla).
    sys.path.insert(0, str(F0 / "b"))
    import analiz_b  # noqa: E402
    import fizik  # noqa: E402
    geoms = fizik.by_cell()
    for cd, base, arm in cells_of(kok, "b_"):
        g = geoms[base]
        e = d.setdefault(base, {}).setdefault(arm, {k: [] for k in
                                                    ("p95", "teslim", "tx", "sw95", "pace",
                                                     "srtt", "rtprop", "pu", "runs")})
        armd = cd / arm
        for rep in sorted(armd.glob("rep*"), key=lambda q: int(q.name[3:])):
            r = analiz_b.analyze_run(rep, g, arm)
            e["p95"].append(r.get("conf_p95_s"))
            n = r.get("n_msgs")
            e["teslim"].append(r["deliv_D"] * n if r.get("deliv_D") is not None and n else None)
            e["tx"].append(r.get("tx_total_B"))
            e["sw95"].append(r.get("sender_wait_p95_s"))
            e["pace"].append(r.get("est_pace"))
            e["srtt"].append(r["est_srtt"] / 1e6 if r.get("est_srtt") else None)
            e["rtprop"].append(r["est_rtprop"] / 1e6 if r.get("est_rtprop") else None)
            e["pu"].append(r.get("probe_up"))
            sr = acil_srate(str(rep))
            need = OFFERED_IP_BPS / (1 - g["loss"] / 100.0) if g["loss"] < 100 else None
            e["runs"].append({"rep": int(rep.name[3:]), "lpc": r.get("left_preconv"),
                              "sr": (sr / need) if (sr is not None and need) else None})
    if ANALOG not in d and ANALOG_L30 in d:
        d["analog"] = ANALOG_L30
    return d


def report(d, man_sha):
    choice, S = select(d)
    R1, R4 = r1(d), r4(d)
    L = [tek_satir(choice, R1), "",
         f"manifest sha256 {man_sha or MANIFEST_SHA256 + ' (beyan; dosya denetlenmedi)'} · "
         f"analog {d.get('analog', ANALOG)} · etiket: gösterim; kapılı PoC ölçümü Aralık; ARQ "
         "yolu, FEC Aralık'ta · N < 30: betimsel", "",
         "## S (bağlayıcı) — " + S["neden"], "",
         "| kol | dosya bitiş med (s) | PTT p95 med (s) | emisyon/sunulan | (i) | (ii) | (iii) | (iv) R3 | uygun |",
         "|---|---|---|---|---|---|---|---|---|"]
    for k in ALL_ARMS:
        row = S["satirlar"].get(k, {})
        L.append(f"| {show(k)} | {fmt(S['file_med'][k], 1)} | {fmt(S['ptt_med'][k])} | "
                 f"{fmt(S['ratio_med'].get(k))} | {row.get('i', '—')} | "
                 f"{row.get('ii', row.get('ii(0,90)', '—'))} | {row.get('iii', '—')} | "
                 f"{row.get('iv', '—')} | {row.get('uygun', '—')} |")
    for cell, vals in sorted((d.get("sunulan_bps") or {}).items()):
        if cell == DOSYASIZ:
            L.append(f"\n(iii) payda (KK-ORK-03): {cell} sunulan = "
                     f"{', '.join(fmt(v, 1) for v in sorted(vals))} B/s [türetilmiş; mühürlü UDP "
                     "yükü, pr ile aynı katman]; pay = alice pr koşu medyanı (Idle hariç)")
    R3 = S["R3"]
    L += ["", f"## R3 (bağlayıcı): geçer = {R3['gecer']} · d1000 l0 p95 med K0 "
          f"{fmt(R3['l0_med']['K0'])} / K2 {fmt(R3['l0_med']['K2'])} → {R3['l0']} · r25 "
          f"teslim@60, gönderim B: K0 {R3['r25_med']['K0']} / K2 {R3['r25_med']['K2']} → "
          f"{R3['r25']}", "", "## R1 (raporlanan): manşet gönderim/gereken", "",
          "| kol | kullanılan | tuzaksız n / n | medyan | bütün medyan | ≥ 0,90 |", "|---|---|---|---|---|---|"]
    for k, v in R1.items():
        L.append(f"| {show(k)} | {v['kullanilan']} | {v['tuzaksiz_n']} / {v['n']} | "
                 f"{fmt(v['med'])} | {fmt(v['med_butun'])} | {v['gecer']} |")
    L += ["", "## R4 (raporlanan)", "",
          "| hücre | kol | sw95 med / 9. sıra (s) | pace med (B/s) | EAI (s) | kayıp eşiği (s) | RTprop med (s) | RTprop/K0 | probe_up med |",
          "|---|---|---|---|---|---|---|---|---|"]
    for part in ("manset", "analog"):
        for k, v in R4[part].items():
            sw = (f"{fmt(v.get('sw95_med'))} / {fmt(v.get('sw95_9'))}" if part == "manset"
                  else "—")
            L.append(f"| {part} | {show(k)} | {sw} | {fmt(v['pace_med'], 0)} | "
                     f"{fmt(v['eai_s'])} | {fmt(v['kayip_esigi_s'])} | "
                     f"{fmt(v['rtprop_med'], 3)} | {fmt(v['rtprop_K0_orani'])} | "
                     f"{fmt(v['probe_up_med'], 0)} |")
    L += ["", f"Startup dönemi kaybı [0, 20) s: {R4['startup_kaybi_0_20']}",
          f"diag_probe: {R4['diag_probe']}"]
    sun = {c: sorted(v) for c, v in (d.get("sunulan_bps") or {}).items()}
    return choice, "\n".join(L) + "\n", {"secim": choice, "S": S, "R1": R1, "R4": R4,
                                          "iii_sunulan_bps_KK_ORK_03": sun}


# ── Öz-test (sahte veri; her kural dalı en az bir vaka) ──────────────────────────────────────

def _base(**over):
    """Beklenti kaydı (manifest §5) değerlerine yakın sahte koşular (N = 10, sabit)."""
    ptt = {"K0": 2.13, "K0r": 1.10, "K1": 2.71, "K2": 1.37, "K2mc": 1.30, "K2me": 1.24}
    fil = {"K0": 49.3, "K0r": 52.2, "K1": 43.6, "K2": 43.7, "K2mc": 43.2, "K2me": 47.8}
    ptt.update(over.get("ptt", {}))
    fil.update(over.get("file", {}))
    d = {ANALOG: {k: {"file": [fil[k]] * 10, "ptt": [ptt[k]] * 10, "pace": [500] * 10,
                      "srtt": [0.35] * 10, "rtprop": [0.30] * 10, "pu": [3] * 10}
                  for k in ALL_ARMS},
         DOSYASIZ: {k: {"ratio": [v] * 10} for k, v in
                    {"K0": 0.55, "K2": 0.98, "K2mc": 0.97, **over.get("ratio", {})}.items()},
         L0: {"K0": {"p95": [4.1] * 10}, "K2": {"p95": [over.get("l0_k2", 3.4)] * 10}},
         R25: {"K0": {"teslim": [3] * 10, "tx": [1000] * 10},
               "K2": {"teslim": [over.get("r25_t", 3)] * 10,
                      "tx": [over.get("r25_tx", 1050)] * 10}},
         MANSET: {}}
    for k, (sr, clean) in {"K0": (0.5, 10), "K2": (0.99, 10), "K2mc": (0.45, 10),
                           **over.get("r1", {})}.items():
        runs = [{"rep": i + 1, "sr": sr if i < clean else sr / 2,
                 "lpc": 5000 if i < clean else 0} for i in range(10)]
        d[MANSET][k] = {"runs": runs, "sw95": [float(i) for i in range(1, 11)],
                        "pace": [400] * 10, "srtt": [2.1] * 10, "rtprop": [2.0] * 10,
                        "pu": [1] * 10}
    for k in over.get("drop", []):
        del d[ANALOG][k]
    return d


def oz_test():
    ok = [0]

    def chk(name, cond):
        print(("  geçti: " if cond else "  KALDI: ") + name)
        if cond:
            ok[0] += 1
        else:
            raise SystemExit(f"öz-test kaldı: {name}")

    n = 0
    # S1: model beklentisi — bant {K2−e, K2−c}, bayrak eşit, dosya kısa → K2−c; K1 (ii)'de kalır.
    c, s = select(_base()); n += 2
    chk("S: %10 bandı + eşit bayrak → kısa dosya (K2−c)", c == "K2mc")
    chk("S: K1 PTT şartında kalır (ii)", s["satirlar"]["K1"]["ii"] is False)
    # S2: bantta 5 bayraklı K2 ile 4 bayraklı K2−c → az bayraklı.
    c, _ = select(_base(ptt={"K2": 1.20, "K2mc": 1.25, "K2me": 2.5})); n += 1
    chk("S: en az bayraklı kazanır (K2 düşük PTT'ye rağmen K2−c)", c == "K2mc")
    # S3: bant tek kol → en düşük PTT.
    c, _ = select(_base(ptt={"K2": 1.0})); n += 1
    chk("S: en düşük PTT bant dışında rakipsiz (K2)", c == "K2")
    # S4: (iii) K2 sonucu K1/K2−e'ye taşınır; K2 0,90 → K1, K2, K2−e uygunsuz, K2−c kalır.
    c, s = select(_base(ratio={"K2": 0.90})); n += 2
    chk("S: (iii) K2'nin sonucu K2−e'ye uygulanır", s["satirlar"]["K2me"]["iii"] is False)
    chk("S: (iii) kalanlar arasında K2−c", c == "K2mc")
    # S5: (i) dosya bitiş K0×1,10'u aşan kol uygunsuz.
    c, s = select(_base(file={"K2mc": 60.0, "K2me": 60.0})); n += 2
    chk("S: (i) dosya > K0×1,10 → uygunsuz", s["satirlar"]["K2mc"]["i"] is False)
    chk("S: (i) sonrası K2", c == "K2")
    # S6: R3 d1000 l0 kalır → hiçbir E1 kolu uygun değil → K0r (PTT ≤ K0×0,90 ∧ (i)).
    c, s = select(_base(l0_k2=5.0)); n += 2
    chk("R3: l0 p95 > K0×1,10 → R3 kalır", s["R3"]["l0"] is False and not s["R3"]["gecer"])
    chk("S: geri dönüş K0r", c == "K0r")
    # S7: r25 teslim K0 − 2 → R3 kalır; K0r PTT ≤ 0,90 değil → K0.
    c, s = select(_base(r25_t=1, ptt={"K0r": 2.0})); n += 2
    chk("R3: r25 teslim@60 < K0 − 1 → kalır", s["R3"]["r25"] is False)
    chk("S: K0r şartı yok → K0", c == "K0")
    # S8: r25 gönderim > K0×1,10 → R3 kalır; K0r (i) kalır → K0.
    c, s = select(_base(r25_tx=1200, file={"K0r": 60.0})); n += 2
    chk("R3: r25 gönderim > K0×1,10 → kalır", s["R3"]["r25"] is False)
    chk("S: K0r (i) kalır → K0", c == "K0")
    # S9: veri eksik kol uygun sayılmaz (fail-closed).
    c, s = select(_base(drop=["K2mc", "K2me"])); n += 2
    chk("S: verisiz kol uygun değil", s["satirlar"]["K2mc"]["uygun"] is False)
    chk("S: verisiz kollar düşünce K2", c == "K2")
    # S10: alınmayan dosya = ∞ (medyan sansürlü).
    d = _base()
    d[ANALOG]["K2mc"]["file"] = [INF] * 6 + [43.0] * 4
    c, s = select(d); n += 1
    chk("S: alınmayan dosya ∞ → (i) kalır", s["satirlar"]["K2mc"]["i"] is False)
    # R1: tuzaksız ≥ 7 → tuzaksız medyan; < 7 → bütün koşular.
    R1 = r1(_base(r1={"K2": (0.99, 8), "K2mc": (0.80, 5)})); n += 4
    chk("R1: tuzaksız 8 ≥ 7 → tuzaksız medyan 0,99", R1["K2"]["kullanilan"] == "tuzaksız"
        and abs(R1["K2"]["med"] - 0.99) < 1e-9)
    chk("R1: tuzaksız 5 < 7 → bütün koşular", R1["K2mc"]["kullanilan"].startswith("bütün"))
    chk("R1: bütün medyan ayrıca yazılır", abs(R1["K2mc"]["med_butun"] - 0.60) < 1e-9)
    chk("R1: ≥ 0,90 bayrağı", R1["K2"]["gecer"] is True and R1["K2mc"]["gecer"] is False)
    # R1: lpc = 0 (PreConverge'den çıkılmadı) tuzaklı sayılır; > 10 s tuzaklı.
    d = _base()
    for i, r in enumerate(d[MANSET]["K2"]["runs"]):
        r["lpc"] = [0, 12_000, 10_000][i % 3]
    n += 1
    chk("R1: lpc 0 ve > 10 000 ms tuzaklı, 10 000 ms tuzaksız", r1(d)["K2"]["tuzaksiz_n"] == 3)
    # R4: sw95 9. sıra, EAI ve kayıp eşiği (floor.py), RTprop/K0.
    R4 = r4(_base()); n += 3
    chk("R4: sw95 medyan 5,5 ve 9. sıra 9", R4["manset"]["K2"]["sw95_med"] == 5.5
        and R4["manset"]["K2"]["sw95_9"] == 9.0)
    e = 2 * 1013 / 400 + 65 / 31
    chk("R4: EAI = 2U/pace + 65/31, eşik = max(9/8·srtt, 1 ms, 1,5·EAI)",
        abs(R4["manset"]["K2"]["eai_s"] - e) < 1e-9
        and abs(R4["manset"]["K2"]["kayip_esigi_s"] - max(9 / 8 * 2.1, 0.001, 1.5 * e)) < 1e-9)
    chk("R4: RTprop/K0 oranı 1", R4["analog"]["K2"]["rtprop_K0_orani"] == 1.0)
    # Tek satır biçimi.
    c, _ = select(_base())
    line = tek_satir(c, r1(_base())); n += 1
    chk("tek satır biçimi", line == "Gösterim seti: K2−c. E1 (manşet, tuzaksız): K2 0.99, "
        "K2−c 0.45.")
    # KK-ORK-03: (iii) paydası pr katmanında (dosyasız 40 B → 86 B / 0,2 s; Y10 80 B → 126 B).
    n += 2
    chk("sunulan: dosyasız 40 B demet → 430 B/s",
        sunulan_ptt_bps({"ptt_bytes": 40, "ptt_bundles": 25, "ptt_bundle_ms": 200}) == 430.0)
    chk("sunulan: 80 B demet → 630 B/s; demet yok → None",
        sunulan_ptt_bps({"ptt_bytes": 80, "ptt_bundles": 20, "ptt_bundle_ms": 200}) == 630.0
        and sunulan_ptt_bps({"ptt_bytes": 40, "ptt_bundles": 0, "ptt_bundle_ms": 200}) is None)
    # Çıkarım yardımcıları: srate (ACIL S) ve pacer (a_ts) sahte dosyalarla.
    with tempfile.TemporaryDirectory() as t:
        z = 1_000_000_000_000
        with open(os.path.join(t, "out.jsonl"), "w", encoding="utf-8") as f:
            f.write(json.dumps({"meta": {"t0_unix_ns": z}}) + "\n")
        with gzip.open(os.path.join(t, "capb_cli.csv.gz"), "wt") as f:
            f.write("# t_ns,dir,x,ip_len\n")
            for sec, d_, ln in ((2.9, "o", 100), (3.0, "o", 1000), (27.9, "o", 1500),
                                (28.0, "o", 7), (10.0, "i", 999)):
                f.write(f"{z + int(sec * 1e9)},{d_},0,{ln},a,b,c\n")
        n += 1
        chk("srate: [3, 28) s yalnız 'o' satırları / 25", acil_srate(t) == 2500 / 25)
        with open(os.path.join(t, "alice.jsonl"), "w", encoding="utf-8") as f:
            f.write(json.dumps({"k": "hdr", "params": {"ptt_bytes": 40, "ptt_bundles": 25,
                                                        "ptt_bundle_ms": 200}},
                               separators=(",", ":")) + "\n")
            for pr_, st_ in ((300, "ProbeUp"), (900, "Idle"), (500, "Drain")):
                f.write(json.dumps({"k": "ts", "s": [
                    {"peer": "bob", "st": st_, "pr": pr_, "srtt": 300.0, "rtp": 290.0, "pu": 2},
                    {"peer": "carol", "st": "ProbeUp", "pr": 9999}]},
                    separators=(",", ":")) + "\n")   # f0_flows biçimi: boşluksuz
        n += 1
        chk("pacer: eş bob, Idle hariç medyan; son rtp/pu",
            acil_pacer_med(os.path.join(t, "alice.jsonl")) == (400, 0.3, 290.0, 2))
        n += 1
        prm = hdr_params(os.path.join(t, "alice.jsonl"))
        chk("hdr params → sunulan 430; oran = 400/430",
            prm is not None and sunulan_ptt_bps(prm) == 430.0
            and abs(400 / sunulan_ptt_bps(prm) - 0.930232558) < 1e-6)
    print(f"== öz-test: {ok[0]}/{n} geçti")
    return 0


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--kok")
    ap.add_argument("--out")
    ap.add_argument("--manifest")
    ap.add_argument("--oz-test", action="store_true")
    a = ap.parse_args()
    if a.oz_test:
        return oz_test()
    if not a.kok:
        ap.error("--kok zorunlu")
    man_sha = None
    if a.manifest:
        man_sha = hashlib.sha256(pathlib.Path(a.manifest).read_bytes()).hexdigest()
        if man_sha != MANIFEST_SHA256:
            print(f"oku: manifest sha256 {man_sha} ≠ donmuş {MANIFEST_SHA256} — okuma yok",
                  file=sys.stderr)
            return 3
    kok = pathlib.Path(a.kok).expanduser().resolve()
    out = pathlib.Path(a.out).expanduser() if a.out else kok / "okuma"
    out.mkdir(parents=True, exist_ok=True)
    d = extract(kok, out)
    choice, md, js = report(d, man_sha)
    (out / "OKUMA.md").write_text(md, encoding="utf-8")
    (out / "okuma.json").write_text(json.dumps(js, ensure_ascii=False, indent=1, default=str),
                                    encoding="utf-8")
    print(md, end="")
    return 0


if __name__ == "__main__":
    sys.exit(main())
