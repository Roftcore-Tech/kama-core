#!/usr/bin/env python3
"""[YOL3 · Ö] mini-Ö okuması (KK-Ö-11, EK-A §6 / KK-Ö-EKA-1): ön kaydın (ONKAYIT.md) kuralları AYNEN.

    python3 -I scripts/yol3/mini-o/oku.py --kok <mini-Ö kökü> [--out <dizin>]
                                          [--onkayit scripts/yol3/mini-o/ONKAYIT.md] [--kol K1] [--taban K0]
    python3 -I scripts/yol3/mini-o/oku.py --surum v2 --kok <v2 kökü> [--out <dizin>]
                                          [--onkayit scripts/yol3/mini-o/ONKAYIT-V2.md]
    python3 -I scripts/yol3/mini-o/oku.py --surum abl --hucre r25|d1000 --kok <ablasyon kökü>
                                          [--out <dizin>] [--onkayit scripts/yol3/mini-o/ONKAYIT-V2.md]
    python3 -I scripts/yol3/mini-o/oku.py --oz-test

`--surum v1` (varsayılan) ONKAYIT.md'nin okumasıdır ve v2/abl eklenince AYNEN kalır. `--surum v2`
ONKAYIT-V2.md'nin okumasıdır: v1 tanımları aynen (kol A = K1A6s, kol b = K1A6, taban K0) + R3-d1000 l0
(teyit p95 medyanı ≤ K0 × 1,10) + v2 "yeşil" (v1 §3.8 ∧ R3-r25 ∧ R3-d1000) + geçerlilik denetimi
(başlıkta uygulanan bayrak seti ve kaynağı, bench manifest `profile_delta`, T1 sayaçlarının varlığı)
+ ablasyon tetiği. `--surum abl` ONKAYIT-V2 §6'nın ablasyon kararını okur (K0 ve K1A6 aynı kökte).

`--onkayit` verilirse dosyanın sha256'sı ONKAYIT_SHA256 ile denetlenir; farklıysa okuma YAPILMAZ
(çıkış 3). Verilmezse okuma yapılır ama çıktının başına "ön kayıt denetlenmedi" yazılır (eski veride
araç sınaması için). Ham veriye YAZMAZ; çıktı `<kök>/okuma/{OKUMA.md,okuma.json}` (ya da `--out`).

Kaynaklar (hepsi mevcut F0 analizörleri; bu dosya yalnız ön kayıttaki tanımları uygular):
  A hücreleri  scripts/ktp2/f0/a/f0a_analyze.py `analyze_run` (öğe başına t_send, t_released =
               bob iz "R" = Transport'un birimi Application'a verdiği an [oynatma tamponu ÖNCESİ],
               t_rx = bob dinleyici olayı [tampon açıkken oynatma anı]) ve sayaçları.
  B hücresi    scripts/ktp2/f0/b/analiz_b.py `analyze_run` (teslim@60 = deliv_D × n_msgs;
               gönderim = tx_total_B), geometri fizik.by_cell().
Tanımlar ONKAYIT.md §3'tedir; buradaki sabitler oradaki sayıların aynısıdır.
"""

import argparse
import hashlib
import json
import math
import os
import pathlib
import statistics
import sys
import tempfile

sys.dont_write_bytecode = True  # F0 analizörleri içe aktarılır; depoya __pycache__ yazılmaz
HERE = pathlib.Path(__file__).resolve().parent
REPO = HERE.parents[2]
F0 = REPO / "scripts" / "ktp2" / "f0"

# ONKAYIT.md sha256 (ön kayıt donunca yazılır; okuma bu dosyaya bağlıdır).
ONKAYIT_SHA256 = "8a96ca0ab660e83a09be9f17e5b3aadde48c17e2e3ad9e6a1f1cc39c2c657a8c"
# ONKAYIT-V2.md sha256 (v2 ve ablasyon okuması buna bağlıdır; ön kayıt donunca yazılır).
ONKAYIT_V2_SHA256 = "136f4bc457a04d5708a59cd0fdd971c9b23a26c313f9f1fd924f7b480ba3ca53"

Y10 = "a_r12k_d150_l20_foto150k"
L30 = "a_r12k_d300_l30_dosyali"
L0 = "a_r12k_d300_l0_dosyali"
R25 = "b_r25_d300_l20_p65"
D1000 = "b_r12k_d1000_l0_p1000"
GK8 = {L30: "kötü", L0: "iyi"}

# [v2] Gösterim seti (ONKAYIT-V2 §1): A hücresi kolu, b hücresi kolu (bench --e1-arm), taban.
V2_A_KOL, V2_B_KOL, V2_TABAN = "K1A6s", "K1A6", "K0"
GOSTER_E1_LABEL = "c+d+DrainRtprop+e-dar+a3_burst_floor+a4_probe_srtt+a6_iw_floor"
GOSTER_E2A_LABEL = ("tt_rfc(g=1ms)+rtt_rule_v11+iw_idle_hold+pto_probes=2+gap_ack+"
                    "return_prior_gated+pto_rfc")
# A hücresinde iki ucun başlığında beklenen `yol3` alanları (ONKAYIT-V2 §3.12).
V2_HDR = {"e1_arm": "K1A6", "flag_src": "goster.rs", "e1_set": GOSTER_E1_LABEL,
          "e2a_set": GOSTER_E2A_LABEL, "fec_off": True, "goster": True, "lanes_w": 5,
          "playout_ms": 2000}
R3_KAT = 1.10              # R3: gönderim / p95 ≤ taban × 1,10
# Bench manifest `profile_delta` beklentisi (bench e1arm.rs ile aynı): gösterim seti alan alan.
GOSTER_E1 = {"e1_startup_cwnd": True, "e1_applimited_plateau": True, "e1_drainrt_gate": True,
             "e1_probe_keep": True, "retx_first": False, "a3_burst_floor": True,
             "a4_probe_srtt": True, "a6_iw_floor": True}
GOSTER_E2A = {"tt_rfc": True, "tt_granularity_ns": 1000000, "rtt_rule_v11": True,
              "iw_idle_hold": True, "pto_probes": 2, "gap_ack": True, "return_prior_gated": True,
              "pto_rfc": True}
# Ablasyon kolu → (A bayrağı, yapı, alan). K1A6mA6 = bench --e1-arm K1A.
ABL = {"K1A6mA1": ("A-1", "e2a", "gap_ack"), "K1A6mA2": ("A-2", "e2a", "return_prior_gated"),
       "K1A6mA3": ("A-3", "e1", "a3_burst_floor"), "K1A6mA4": ("A-4", "e1", "a4_probe_srtt"),
       "K1A6mA5": ("A-5", "e2a", "pto_rfc"), "K1A6mA6": ("A-6", "e1", "a6_iw_floor")}
ABL_HUCRE = {"r25": R25, "d1000": D1000}

# Ölçütler (ONKAYIT §3; v4 III.1, EK-A §6).
ILK_DEMET_S = 3.0          # basış → ilk demet varışı (bağlayıcı)
TEXT_S = 10.0              # dosya sürerken TEXT (bağlayıcı)
FILE_S = 30.0              # fotoğraf bitişi (bağlayıcı)
T_EK_S, T_MIN_S, T_MAX_S, T_YOK_S = 0.3, 1.5, 2.5, 2.0   # T = p95(D) + 0,3 s, [1,5; 2,5]; yoksa 2,0
STALL_TOL_MS = 40.0        # doğrudan durma toleransı = 2 × NODE_TICK (kama-node 20 ms)
YESIL_MIN = 8              # Paz "yeşil": Y10'da ≥ 8/10 başarılı koşu
GOSTER_PRESS = 0           # bağlayıcı basış: 15 s basışı (akış 0; karşı örnek 90 hariç)
KARSI_SPURT = 90
GK8_B1 = {"kötü": 3.0, "iyi": 1.5}
GK8_B2 = {"kötü": 10.0, "iyi": 2.0}
GK8_TEXT_ALL_S = 60.0
MSGS_R25 = 6               # r25 hücresi mesaj sayısı (betimsel "bütün mesajlar 60 s içinde")
INF = math.inf


# ── yardımcılar ──────────────────────────────────────────────────────────────────────────────

def rank(values, q):
    """Nearest-rank (f0a_tam.rank ile aynı); ∞ değerler dahil (sansür)."""
    v = sorted(values)
    if not v:
        return None
    return v[max(1, math.ceil(q * len(v))) - 1]


def med(xs):
    xs = [x for x in xs if x is not None]
    return statistics.median(xs) if xs else None


def fmt(x, nd=2):
    if x is None:
        return "—"
    if isinstance(x, float) and math.isinf(x):
        return "∞"
    if isinstance(x, bool):
        return "evet" if x else "hayır"
    if isinstance(x, float):
        return f"{x:.{nd}f}".replace(".", ",")
    return str(x)


def jsafe(x):
    if isinstance(x, float) and math.isinf(x):
        return "inf"
    if isinstance(x, dict):
        return {str(k): jsafe(v) for k, v in x.items()}
    if isinstance(x, (list, tuple)):
        return [jsafe(v) for v in x]
    return x


def sha256(path):
    with open(path, "rb") as f:
        return hashlib.sha256(f.read()).hexdigest()


def read_jsonl(path):
    out = []
    try:
        with open(path, encoding="utf-8", errors="replace") as f:
            for ln in f:
                ln = ln.strip()
                if ln:
                    try:
                        out.append(json.loads(ln))
                    except ValueError:
                        pass
    except OSError:
        return []
    return out


# ── Kural katmanı (saf; öz-testin konusu) ─────────────────────────────────────────────────────

def press_metrics(items, sp, bundle_ms, playout_ms):
    """Bir basışın ölçüleri (ONKAYIT §3.1–§3.2). items: f0a öğeleri. Süreler saniye.
    varış r_i = t_released (iz "R", tampon öncesi); eşlenmediyse t_rx (üst sınır, `eslenmemis`).
    oynatma p_i = t_rx (bob PttBundle olayı). Teslim edilmeyen demet ∞ (sansür)."""
    o = next((it for it in items if it.get("raw_c") == "O" and it.get("sp") == sp), None)
    ps = sorted((it for it in items if it.get("raw_c") == "P" and it.get("sp") == sp),
                key=lambda it: it["i"])
    if o is None or not ps:
        return None
    t_open = o["t_send"]
    arr, play, unmapped = [], [], 0
    for it in ps:
        if it.get("t_rx") is None:
            arr.append(INF)
            play.append(INF)
            continue
        if it.get("t_released") is not None:
            arr.append(it["t_released"])
        else:
            arr.append(it["t_rx"])
            unmapped += 1
        play.append(it["t_rx"])
    n_rx = sum(1 for p in play if p != INF)
    r0, p0 = arr[0], play[0]
    ilk_demet = (r0 - t_open) / 1000.0 if r0 != INF else INF
    ilk_ses = (p0 - t_open) / 1000.0 if p0 != INF else INF
    if INF in arr:
        d = INF
    else:
        d = max(arr[i] - r0 - i * bundle_ms for i in range(len(arr))) / 1000.0
    # Doğrudan oynatma (yalnız tampon açıkken anlamlı): ardışık oynatma aralığı > bundle + tol.
    stalls, stall_ms, sirasiz = 0, 0.0, 0
    for a, b in zip(play, play[1:]):
        if a == INF or b == INF:
            continue
        g = b - a
        if g < 0:
            sirasiz += 1
        if g > bundle_ms + STALL_TOL_MS:
            stalls += 1
            stall_ms += g - bundle_ms
    lat = [p - it["t_send"] for p, it in zip(play, ps) if p != INF]
    varis = [(r - it["t_send"]) / 1000.0 if r != INF else INF for r, it in zip(arr, ps)]
    acc_end = (lat[-1] - lat[0]) / 1000.0 if len(lat) >= 2 and n_rx == len(ps) else None
    acc_max = (max(lat) - lat[0]) / 1000.0 if lat else None
    if n_rx < len(ps):
        stalls_eff = None  # eksik demet: doğrudan ölçü tanımsız (kesintisiz sayılmaz)
    else:
        stalls_eff = stalls
    t_s = (playout_ms / 1000.0) if playout_ms else None
    kalinti = None
    if t_s is not None and r0 != INF and p0 != INF:
        kalinti = (p0 - r0) / 1000.0 - t_s
    return {"ilk_demet_s": ilk_demet, "ilk_ses_s": ilk_ses, "D_s": d,
            "duraklama_n": stalls_eff, "duraklama_ms": stall_ms if stalls_eff is not None else None,
            "sirasiz": sirasiz, "biriken_son_s": acc_end, "biriken_max_s": acc_max,
            "varis_p95_s": rank(varis, .95),
            "n_gonderilen": len(ps), "n_alinan": n_rx, "eslenmemis": unmapped,
            "tampon_kalinti_s": kalinti}


def kesintisiz(pm, playout_ms):
    """ONKAYIT §3.2: tampon açık → doğrudan (duraklama_n == 0); kapalı → analitik D ≤ T (T = 2,0)."""
    if pm is None:
        return None
    if playout_ms:
        return pm["duraklama_n"] == 0 if pm["duraklama_n"] is not None else False
    return pm["D_s"] <= T_YOK_S


def text_metrics(items, t_lo, t_hi, cls="T"):
    """t_send ∈ [t_lo, t_hi] olan `cls` TEXT'leri: (n, en büyük s, p95 s). Teslimsiz ∞."""
    vals = []
    for it in items:
        if it.get("raw_c") != cls or t_lo is None or t_hi is None:
            continue
        if not t_lo <= it["t_send"] <= t_hi:
            continue
        vals.append((it["t_rx"] - it["t_send"]) / 1000.0 if it.get("t_rx") is not None else INF)
    if not vals:
        return {"n": 0, "max_s": None, "p95_s": None}
    return {"n": len(vals), "max_s": max(vals), "p95_s": rank(vals, .95)}


def basari(run):
    """ONKAYIT §3.4: dört ölçüt birlikte (Y10, 15 s basışı). Boş TEXT penceresi geçmez."""
    pm = run.get("basis", {}).get(GOSTER_PRESS)
    td = run.get("text_dosya") or {}
    parts = {
        "ilk_demet": pm is not None and pm["ilk_demet_s"] <= ILK_DEMET_S,
        "text": td.get("n", 0) >= 1 and td.get("max_s") is not None and td["max_s"] <= TEXT_S,
        "dosya": run.get("dosya_s") is not None and run["dosya_s"] <= FILE_S,
        "kesintisiz": bool(kesintisiz(pm, run.get("playout_ms"))),
        "gecerli": run.get("rc") == 0,
    }
    return all(parts.values()), parts


def t_oneri(d_values):
    """ONKAYIT §3.3: T = p95(D) + 0,3 s, [1,5; 2,5]'e kırpılır; ölçüm yoksa 2,0 s."""
    if not d_values:
        return T_YOK_S, None
    p = rank(d_values, .95)
    return min(T_MAX_S, max(T_MIN_S, p + T_EK_S)), p


def ackli_kayip(run):
    """ONKAYIT §3.5 (W teyidi): ACK'li birim kaybı bileşenleri; (toplam, belirsiz mi)."""
    a = run["ackli"]
    tot = a["dcd"] + a["reorder_atlama"] + a["teyitli_alinmayan_text"] + a["dosya_teyitli_alinmayan"]
    belirsiz = False
    if a["alice_out_son"] == 0:
        tot += a["suzulmeyen_birim"]
    elif a["suzulmeyen_birim"]:
        belirsiz = True
    return tot, belirsiz


def r3_r25(kol_teslim, kol_tx, taban_teslim, taban_tx):
    """ONKAYIT §3.6 (CC §4.2 R3, aynen): teslim@60 ≥ taban − 1 ∧ gönderim ≤ taban × 1,10."""
    if None in (kol_teslim, kol_tx, taban_teslim, taban_tx):
        return None
    return kol_teslim >= taban_teslim - 1 and kol_tx <= taban_tx * 1.10


def w_karari(kayip_toplam):
    """ONKAYIT §3.5: bütün A hücrelerinde ACK'li birim kaybı 0 ⇒ W = 5; > 0 ⇒ Paz W = 4."""
    return 5 if kayip_toplam == 0 else 4


# ── Çıkarım (ham veri → koşu kayıtları) ──────────────────────────────────────────────────────

def hdr_of(lines):
    return next((ln for ln in lines if ln.get("k") == "hdr"), {}) or {}


def queue_peaks(alice, bob):
    """Örneklenmiş (ts, ≈ 200 ms) en yüksek doluluklar: motor (alice → bob oturumu) ve bob Reorder;
    T1 sayaçları ts satırında `t1` varsa onlar (yoksa None = eksik)."""
    pk = {"alice_out": 0, "alice_txq": 0, "alice_un": 0, "alice_inf": 0, "alice_pl": 0,
          "bob_reorder_h": 0}
    t1_last, t1_max, t1_seen = {}, {}, False
    for ln in alice:
        if ln.get("k") != "ts":
            continue
        pk["alice_out"] = max(pk["alice_out"], ln.get("out") or 0)
        for s in ln.get("s", []):
            if s.get("peer") == "bob" and s.get("rec"):
                r = s["rec"]
                for k, src in (("alice_txq", "txq"), ("alice_un", "un"), ("alice_inf", "inf"),
                               ("alice_pl", "pl")):
                    pk[k] = max(pk[k], r.get(src) or 0)
    for who, lines in (("alice", alice), ("bob", bob)):
        for ln in lines:
            if ln.get("k") != "ts":
                continue
            if who == "bob":
                for s in ln.get("s", []):
                    if s.get("peer") == "alice" and s.get("reo"):
                        pk["bob_reorder_h"] = max(pk["bob_reorder_h"], s["reo"].get("h") or 0)
            t1 = ln.get("t1")
            if isinstance(t1, dict):
                t1_seen = True
                for k, v in flatten(t1, f"{who}."):
                    t1_last[k] = v
                    t1_max[k] = max(t1_max.get(k, v), v)
    return pk, ({"son": t1_last, "en_yuksek": t1_max} if t1_seen else None)


def flatten(d, pre=""):
    for k, v in d.items():
        if isinstance(v, dict):
            yield from flatten(v, f"{pre}{k}.")
        elif isinstance(v, list):
            for i, x in enumerate(v):
                if isinstance(x, (int, float)) and not isinstance(x, bool):
                    yield f"{pre}{k}[{i}]", x
        elif isinstance(v, (int, float)) and not isinstance(v, bool):
            yield f"{pre}{k}", v


def a_run(rep_dir, cell, arm, rep, fa):
    res, items = fa.analyze_run(str(rep_dir), cell, arm, rep)
    alice = read_jsonl(rep_dir / "alice.jsonl")
    bob = read_jsonl(rep_dir / "bob.jsonl")
    hdr = hdr_of(alice)
    y3 = hdr.get("yol3") or {}
    prm = hdr.get("params") or {}
    bundle_ms = prm.get("ptt_bundle_ms") or 200
    playout_ms = y3.get("playout_ms")
    a_sum = res.get("alice_summary") or {}
    b_sum = res.get("bob_summary") or {}
    run = {"rep": rep, "rc": res.get("rc"), "gecerli": res.get("valid"),
           "sorunlar": res.get("problems"), "goster": bool(y3.get("goster")),
           "playout_ms": playout_ms, "lanes_w": y3.get("lanes_w"), "e1_arm": y3.get("e1_arm"),
           "e1_set": y3.get("e1_set"), "e2a_set": y3.get("e2a_set"), "fec_off": y3.get("fec_off"),
           "bundle_ms": bundle_ms,
           # [v2] eklemeli: bayrak kaynağı, bob başlığı, T1 alanının uç başına varlığı.
           "flag_src": y3.get("flag_src"), "bob_yol3": hdr_of(bob).get("yol3") or {},
           "t1_uclar": {who: any(isinstance(ln.get("t1"), dict) for ln in lines
                                 if ln.get("k") == "ts")
                        for who, lines in (("alice", alice), ("bob", bob))}}
    # Basışlar.
    sps = sorted({it.get("sp") for it in items if it.get("raw_c") == "O"
                  and it.get("sp") is not None})
    run["basis"] = {sp: press_metrics(items, sp, bundle_ms, playout_ms) for sp in sps}
    # Dosya.
    f = next((it for it in items if it.get("raw_c") == "F"), None)
    if f is None:
        run["dosya_s"] = None
        lo = hi = None
    else:
        run["dosya_s"] = (f["t_rx"] - f["t_send"]) / 1000.0 if f.get("t_rx") is not None else INF
        lo = f["t_send"]
        hi = f["t_rx"] if f.get("t_rx") is not None else a_sum.get("ended")
    run["dosya_bayt"] = prm.get("file_bytes")
    run["text_dosya"] = text_metrics(items, lo, hi, "T")
    run["text_karsi"] = text_metrics(items, lo, hi, "K")
    all_t = [(it["t_rx"] - it["t_send"]) / 1000.0 if it.get("t_rx") is not None else INF
             for it in items if it.get("raw_c") == "T"]
    run["text_hepsi"] = {"n": len(all_t), "p95_s": rank(all_t, .95) if all_t else None,
                         "hepsi_60s": bool(all_t) and max(all_t) <= GK8_TEXT_ALL_S,
                         "degerler": all_t}
    # Callback + tur (vekil): TEXT için dinleyici − iz "R".
    run["cb_text_ms"] = [it["t_rx"] - it["t_released"] for it in items
                         if it.get("raw_c") == "T" and it.get("t_rx") is not None
                         and it.get("t_released") is not None]
    # ACK'li birim kaybı ve olay kaybı.
    c = res.get("counters") or {}
    a_last = next((ln for ln in reversed(alice) if ln.get("k") == "ts"), {}) or {}
    rx_keys = {("T" if ln.get("c") in ("T", "W", "K") else ln.get("c"), ln.get("c"), ln.get("i"))
               for ln in bob if ln.get("k") == "rx"}
    sends_by_mid = {ln.get("mid"): (ln.get("c"), ln.get("i")) for ln in alice
                    if ln.get("k") == "send" and ln.get("mid") is not None}
    rcpt_no_rx = 0
    for ln in alice:
        if ln.get("k") == "rcpt" and ln.get("first", True):
            ci = sends_by_mid.get(ln.get("mid"))
            if ci and ("T", ci[0], ci[1]) not in rx_keys:
                rcpt_no_rx += 1
    file_bad = 0
    if a_sum.get("file", {}).get("delivered") and not (
            (b_sum.get("file") or {}).get("status") == "received"
            and (b_sum.get("file") or {}).get("match")):
        file_bad = 1
    run["ackli"] = {"dcd": c.get("bob_delivery_channel_drops") or 0,
                    "reorder_atlama": c.get("units_skipped_below_next") or 0,
                    "suzulmeyen_birim": c.get("units_never_drained") or 0,
                    "alice_out_son": a_last.get("out"),
                    "teyitli_alinmayan_text": rcpt_no_rx,
                    "dosya_teyitli_alinmayan": file_bad}
    sent = a_sum.get("sent") or {}
    got = b_sum.get("counts") or {}
    run["olay"] = {"dinleyici_dusus_alice": a_sum.get("listener_drops"),
                   "dinleyici_dusus_bob": b_sum.get("listener_drops"),
                   "iz_dusus": any(str(p).startswith("iz düşüşü") for p in res.get("problems", [])),
                   "O": (sent.get("O"), got.get("O")), "C": (sent.get("C"), got.get("C")),
                   "P": (sent.get("P"), got.get("P")),
                   "yinelenen": b_sum.get("duplicates"), "ayrisamayan": b_sum.get("unparsed")}
    run["kuyruk"], run["t1"] = queue_peaks(alice, bob)
    if cell.startswith(Y10 + "__"):
        run["basarili"], run["basari_parca"] = basari(run)
    return run


def b_runs(cell_dir, base, arm, analiz_b, geoms):
    out = []
    g = geoms.get(base)
    if g is None:
        return out
    armd = cell_dir / arm
    for rep in sorted(armd.glob("rep*"), key=lambda q: int(q.name[3:])):
        r = analiz_b.analyze_run(rep, g, arm)
        n = r.get("n_msgs")
        out.append({"rep": int(rep.name[3:]), "rc": r.get("rc"),
                    "teslim60": r["deliv_D"] * n if r.get("deliv_D") is not None and n else None,
                    "gonderim_B": r.get("tx_total_B"), "p95_s": r.get("conf_p95_s"),
                    # [v2] eklemeli: istemci manifest'inin E1/E2a sapması (kol doğrulaması).
                    "bayrak": manifest_flags(rep / "manifest.json")})
    return out


def debug_struct(text):
    """`E1Flags { a: true, b: 2 }` → {a: True, b: 2} (ilk `{…}`); ayrışmazsa None."""
    i, j = text.find("{"), text.find("}")
    if i < 0 or j < i:
        return None
    out = {}
    for part in text[i + 1:j].split(","):
        if ":" not in part:
            continue
        k, v = (x.strip() for x in part.split(":", 1))
        out[k] = True if v == "true" else False if v == "false" else (
            int(v) if v.lstrip("-").isdigit() else v)
    return out


def manifest_flags(path):
    """Bench manifest `profile_delta`: {"e1": dict|None, "e2a": dict|None} (None = ürün değeri,
    sapma satırı yok); dosya yoksa/okunamazsa None (doğrulanamadı)."""
    try:
        with open(path, encoding="utf-8") as f:
            m = json.load(f)
    except (OSError, ValueError):
        return None
    delta = m.get("profile_delta")
    if not isinstance(delta, list):
        return None
    got = {"e1": None, "e2a": None}
    for line in delta:
        if isinstance(line, str) and line.startswith("e1_flags "):
            got["e1"] = debug_struct(line)
        elif isinstance(line, str) and line.startswith("e2a_flags "):
            got["e2a"] = debug_struct(line)
    return got


def extract(kok):
    sys.path.insert(0, str(F0 / "a"))
    sys.path.insert(0, str(F0 / "b"))
    import f0a_analyze as fa  # noqa: E402
    d = {"A": {}, "B": {}}
    for cd in sorted(p for p in kok.iterdir() if p.is_dir() and "__" in p.name):
        base, arm = cd.name.split("__", 1)
        if base.startswith("a_"):
            reps = sorted((p for p in (cd / arm).glob("rep*") if p.is_dir()),
                          key=lambda q: int(q.name[3:])) if (cd / arm).is_dir() else []
            d["A"].setdefault(base, {})[arm] = [a_run(p, cd.name, arm, int(p.name[3:]), fa)
                                               for p in reps]
        elif base.startswith("b_"):
            import analiz_b  # noqa: E402
            import fizik  # noqa: E402
            d["B"].setdefault(base, {})[arm] = b_runs(cd, base, arm, analiz_b, fizik.by_cell())
    return d


# ── Rapor ────────────────────────────────────────────────────────────────────────────────────

def pooled(runs, key, sps=None):
    vals = []
    for r in runs:
        for sp, pm in (r.get("basis") or {}).items():
            if pm is None or sp == KARSI_SPURT or (sps is not None and sp not in sps):
                continue
            vals.append(pm[key])
    return vals


def summarize(d, kol, taban, kol_b=None):
    """kol: A hücrelerinin kolu; kol_b: r25 kolu (v1'de None = kol; v2'de bench kolu K1A6)."""
    kol_b = kol if kol_b is None else kol_b
    S = {"hucreler": {}, "kol": kol, "taban": taban, "kol_b": kol_b}
    kayip_toplam, belirsiz_kosu = 0, []
    for base, arms in sorted(d["A"].items()):
        for arm, runs in sorted(arms.items()):
            key = f"{base}__{arm}"
            n = len(runs)
            e = {"n": n, "rc0": sum(1 for r in runs if r["rc"] == 0)}
            p0 = [r["basis"].get(GOSTER_PRESS) for r in runs]
            p0 = [p for p in p0 if p is not None]
            e["ilk_demet_s"] = [p["ilk_demet_s"] for p in p0]
            e["ilk_ses_s"] = [p["ilk_ses_s"] for p in p0]
            e["varis_p95_s"] = [p["varis_p95_s"] for p in p0]
            e["D_s"] = [p["D_s"] for p in p0]
            e["duraklama_n"] = [p["duraklama_n"] for p in p0]
            e["duraklama_ms"] = [p["duraklama_ms"] for p in p0]
            e["biriken_son_s"] = [p["biriken_son_s"] for p in p0]
            e["biriken_max_s"] = [p["biriken_max_s"] for p in p0]
            e["eslenmemis_ilk"] = sum(1 for r in runs if (r["basis"].get(GOSTER_PRESS) or {})
                                      .get("eslenmemis"))
            # Analitik kesintisiz (raporlanan; ONKAYIT §3.2): D ≤ T, T = koşunun tamponu (yoksa 2,0).
            e["analitik_kesintisiz"] = sum(
                1 for r in runs if (r["basis"].get(GOSTER_PRESS) or {}).get("D_s") is not None
                and r["basis"][GOSTER_PRESS]["D_s"]
                <= ((r["playout_ms"] / 1000.0) if r["playout_ms"] else T_YOK_S))
            e["text_dosya_max_s"] = [r["text_dosya"]["max_s"] for r in runs]
            e["text_dosya_p95_s"] = [r["text_dosya"]["p95_s"] for r in runs]
            e["text_dosya_n"] = [r["text_dosya"]["n"] for r in runs]
            e["text_karsi_max_s"] = [r["text_karsi"]["max_s"] for r in runs]
            e["dosya_s"] = [r["dosya_s"] for r in runs]
            e["playout_ms"] = sorted({r["playout_ms"] for r in runs}, key=str)
            e["lanes_w"] = sorted({r["lanes_w"] for r in runs}, key=str)
            e["e1_arm"] = sorted({r["e1_arm"] for r in runs}, key=str)
            e["goster"] = sorted({r["goster"] for r in runs}, key=str)
            ack = []
            for r in runs:
                tot, bel = ackli_kayip(r)
                ack.append(tot)
                if bel:
                    belirsiz_kosu.append(f"{key}/rep{r['rep']}")
            e["ackli_kayip"] = ack
            if arm == kol:
                kayip_toplam += sum(ack)
            e["ackli_bilesen"] = {k: sum((r["ackli"][k] or 0) for r in runs)
                                  for k in ("dcd", "reorder_atlama", "suzulmeyen_birim",
                                            "teyitli_alinmayan_text", "dosya_teyitli_alinmayan")}
            e["olay"] = {
                "dinleyici_dusus": sum((r["olay"]["dinleyici_dusus_alice"] or 0)
                                       + (r["olay"]["dinleyici_dusus_bob"] or 0) for r in runs),
                "iz_dusus_kosu": sum(1 for r in runs if r["olay"]["iz_dusus"]),
                "O_eksik": sum((r["olay"]["O"][0] or 0) - (r["olay"]["O"][1] or 0) for r in runs),
                "C_eksik": sum((r["olay"]["C"][0] or 0) - (r["olay"]["C"][1] or 0) for r in runs),
                "P_eksik": sum((r["olay"]["P"][0] or 0) - (r["olay"]["P"][1] or 0) for r in runs),
                "yinelenen": sum(r["olay"]["yinelenen"] or 0 for r in runs),
                "ayrisamayan": sum(r["olay"]["ayrisamayan"] or 0 for r in runs),
                "sirasiz": sum(p["sirasiz"] for r in runs for p in r["basis"].values() if p)}
            e["kuyruk_max"] = {k: max((r["kuyruk"][k] for r in runs), default=None)
                               for k in (runs[0]["kuyruk"] if runs else {})}
            t1 = [r["t1"] for r in runs if r["t1"]]
            e["t1"] = None if not t1 else {
                "en_yuksek": {k: max(x["en_yuksek"].get(k, 0) for x in t1)
                              for k in sorted({k for x in t1 for k in x["en_yuksek"]})},
                "son_toplam": {k: sum(x["son"].get(k, 0) for x in t1)
                               for k in sorted({k for x in t1 for k in x["son"]})}}
            cb = [v for r in runs for v in r["cb_text_ms"]]
            e["cb_text_ms"] = {"n": len(cb), "p50": rank(cb, .5), "p95": rank(cb, .95),
                               "max": max(cb) if cb else None}
            kal = [p["tampon_kalinti_s"] for r in runs for p in r["basis"].values()
                   if p and p["tampon_kalinti_s"] is not None]
            e["tampon_kalinti_ms"] = {"n": len(kal),
                                      "p50": rank([k * 1000 for k in kal], .5),
                                      "p95": rank([k * 1000 for k in kal], .95),
                                      "max": max(kal) * 1000 if kal else None}
            if base == Y10:
                e["basarili"] = sum(1 for r in runs if r.get("basarili"))
                e["basari_parca"] = {k: sum(1 for r in runs if r["basari_parca"][k])
                                     for k in ("ilk_demet", "text", "dosya", "kesintisiz",
                                               "gecerli")}
                tv, p95d = t_oneri(e["D_s"])
                e["T_oneri_s"], e["D_p95_s"] = tv, p95d
                tv2, p95d2 = t_oneri(pooled(runs, "D_s", {0, 1}))
                e["T_oneri_iki_basis_s"], e["D_p95_iki_basis_s"] = tv2, p95d2
            if base in GK8:
                kind = GK8[base]
                ilk_ses = pooled(runs, "ilk_ses_s")
                ilk_demet = pooled(runs, "ilk_demet_s")
                t_all = [v for r in runs for v in r["text_hepsi"]["degerler"]]
                e["gk8"] = {"tur": kind,
                            "B1_ilk_ses_p95_s": rank(ilk_ses, .95),
                            "B1_ilk_demet_p95_s": rank(ilk_demet, .95),
                            "B1_esik_s": GK8_B1[kind],
                            "B2_hepsi_60s_kosu": sum(1 for r in runs if r["text_hepsi"]["hepsi_60s"]),
                            "B2_text_p95_s": rank(t_all, .95), "B2_esik_s": GK8_B2[kind],
                            "B5_bozuk": e["olay"]["ayrisamayan"], "B5_sirasiz": e["olay"]["sirasiz"],
                            "B5_D_p95_s": rank(pooled(runs, "D_s"), .95)}
            S["hucreler"][key] = e
    # R3-r25.
    rb = d["B"].get(R25, {})
    kt = med([r["teslim60"] for r in rb.get(kol_b, [])])
    kx = med([r["gonderim_B"] for r in rb.get(kol_b, [])])
    tt = med([r["teslim60"] for r in rb.get(taban, [])])
    tx = med([r["gonderim_B"] for r in rb.get(taban, [])])
    S["r3_r25"] = {"kol": {"n": len(rb.get(kol_b, [])), "teslim60_med": kt, "gonderim_med_B": kx},
                   "taban": {"n": len(rb.get(taban, [])), "teslim60_med": tt,
                             "gonderim_med_B": tx},
                   "gecer": r3_r25(kt, kx, tt, tx)}
    S["ackli_kayip_toplam_kol"] = kayip_toplam
    S["belirsiz_kosu"] = belirsiz_kosu
    S["W"] = w_karari(kayip_toplam)
    y = S["hucreler"].get(f"{Y10}__{kol}")
    S["yesil"] = None if y is None else {
        "basarili": y["basarili"], "n": y["n"], "esik": YESIL_MIN,
        "y10": y["basarili"] >= YESIL_MIN, "ackli0": kayip_toplam == 0,
        "r3_r25": S["r3_r25"]["gecer"],
        "gecer": y["basarili"] >= YESIL_MIN and kayip_toplam == 0 and bool(S["r3_r25"]["gecer"])}
    return S


def t1_ozet(t1):
    """`t1` varsa kısa özet (koşular arası en yüksek / son değer toplamı; ayrıntı okuma.json)."""
    hi, last = t1.get("en_yuksek", {}), t1.get("son_toplam", {})
    ref = sum(v for k, v in last.items() if k.startswith("alice.ln.ref["))
    return (f"tampon tepe {hi.get('bob.po.peak', '—')} demet · tampon duraklama Σ "
            f"{last.get('bob.po.st', '—')} · atılan Σ {last.get('bob.po.disc', '—')} · şerit ret Σ "
            f"{ref} · şerit düşen Σ {last.get('alice.ln.drp', '—')} · şerit kuyruğu tepe (örn.) "
            f"{hi.get('alice.bl[0]', '—')} · dosya penceresi tepe (örn.) {hi.get('alice.bl[1]', '—')}")


def report(S, onkayit_line):
    kol, taban = S["kol"], S["taban"]
    v2 = "yesil_v2" in S
    # ONKAYIT-V2 §0 etiket satırı (sahip kararı 2026-11-10 madde 5); v1 metni aynen kalır.
    sig = "; sığ tamponlu ağ gösterim kapsamı dışı" if v2 else ""
    L = ["# mini-Ö v2 okuması" if v2 else "# mini-Ö okuması", "",
         f"Ön kayıt: {onkayit_line} · kol {kol} · r25 tabanı {taban}. Etiket: gösterim; kapılı PoC "
         f"ölçümü Aralık; ARQ yolu, FEC Aralık'ta; N < 30 betimsel{sig}. Değerler [ölçüldü] (ham veriden, "
         "F0 analizörleri), T önerisi ve kıyaslar [türetilmiş]. ∞ = teslim edilmedi (sansür).", ""]
    L += ["## 1. Bağlayıcı dört ölçüt (Y10, 15 s basışı; ONKAYIT §3.4)", "",
          "| hücre | n | rc=0 | başarılı | ilk demet ≤ 3 s | TEXT ≤ 10 s | dosya ≤ 30 s | kesintisiz |"
          " analitik D ≤ T (raporlanan) | eşlenmemiş ilk demet | T (ms) | W |",
          "|---|---|---|---|---|---|---|---|---|---|---|---|"]
    for key, e in S["hucreler"].items():
        if "basarili" not in e:
            continue
        bp = e["basari_parca"]
        L.append(f"| {key} | {e['n']} | {e['rc0']} | **{e['basarili']}** | {bp['ilk_demet']} | "
                 f"{bp['text']} | {bp['dosya']} | {bp['kesintisiz']} | "
                 f"{e['analitik_kesintisiz']} | {e['eslenmemis_ilk']} | "
                 f"{'/'.join(map(str, e['playout_ms']))} | {'/'.join(map(str, e['lanes_w']))} |")
    L += ["", "## 2. EK-A §6 beş madde (hücre başına; basış = 15 s basışı; medyan [en kötü])", "",
          "| hücre | ilk demet varışı s | bütün demetler varış p95 s | ilk duyulabilir ses s | "
          "D s (p95) | duraklama n / ms (toplam) | biriken gecikme son / en çok s | "
          "TEXT dosya sürerken en büyük / p95 s | dosya bitiş s |",
          "|---|---|---|---|---|---|---|---|---|"]
    for key, e in S["hucreler"].items():
        def mw(xs):
            xs2 = [x for x in xs if x is not None]
            return f"{fmt(med(xs2))} [{fmt(max(xs2) if xs2 else None)}]"
        dn = [x for x in e["duraklama_n"] if x is not None]
        dm = [x for x in e["duraklama_ms"] if x is not None]
        L.append(f"| {key} | {mw(e['ilk_demet_s'])} | {mw(e['varis_p95_s'])} | "
                 f"{mw(e['ilk_ses_s'])} | "
                 f"{fmt(rank(e['D_s'], .95) if e['D_s'] else None)} | "
                 f"{sum(dn)} / {fmt(sum(dm), 0)} | {mw(e['biriken_son_s'])} / "
                 f"{mw(e['biriken_max_s'])} | {mw(e['text_dosya_max_s'])} / "
                 f"{mw(e['text_dosya_p95_s'])} | {mw(e['dosya_s'])} |")
    L += ["", "Madde 5 — olay/demet kaybı (toplam, hücre başına) ve kuyrukların en yüksek doluluğu:",
          "", "| hücre | dinleyici düşüşü | iz düşüşlü koşu | O / C / P eksik | yinelenen | "
          "ayrışamayan | sırasız oynatma | motor kuyruğu en yüksek (out/txq/un/inf/pl) | "
          "bob Reorder tutulan en yüksek | T1 sayaçları |", "|---|---|---|---|---|---|---|---|---|---|"]
    for key, e in S["hucreler"].items():
        o, k = e["olay"], e["kuyruk_max"]
        t1 = "eksik (ts satırında `t1` yok)" if e["t1"] is None else t1_ozet(e["t1"])
        L.append(f"| {key} | {o['dinleyici_dusus']} | {o['iz_dusus_kosu']} | {o['O_eksik']} / "
                 f"{o['C_eksik']} / {o['P_eksik']} | {o['yinelenen']} | {o['ayrisamayan']} | "
                 f"{o['sirasiz']} | {k.get('alice_out')}/{k.get('alice_txq')}/{k.get('alice_un')}/"
                 f"{k.get('alice_inf')}/{k.get('alice_pl')} | {k.get('bob_reorder_h')} | {t1} |")
    L += ["", "Kuyruk doluluğu ≈ 200 ms örneklemeyle (ts satırları) — gerçek tepe daha yüksek olabilir. "
          "T1 şerit kuyruğu ve oynatma tamponu doluluğu bu düzenekte kayıtlı değilse \"eksik\" "
          "(ONKAYIT §3.7).", ""]
    L += ["## 3. T ve W teyidi (ONKAYIT §3.3, §3.5)", ""]
    y = S["hucreler"].get(f"{Y10}__{kol}")
    if y is not None:
        L.append(f"- T önerisi = p95(D) + 0,3 s, [1,5; 2,5]: D p95 = {fmt(y['D_p95_s'])} s → "
                 f"**T = {fmt(y['T_oneri_s'])} s** (15 s basışı, n = {len(y['D_s'])}); iki basış "
                 f"birlikte: D p95 = {fmt(y['D_p95_iki_basis_s'])} s → {fmt(y['T_oneri_iki_basis_s'])} s "
                 "(raporlanan).")
    else:
        L.append(f"- T: Y10 hücresinde {kol} verisi yok → ölçüm yok, T = 2,0 s.")
    L.append(f"- ACK'li birim kaybı ({kol} kolunun bütün A hücreleri, koşu toplamı): "
             f"{S['ackli_kayip_toplam_kol']} → **W = {S['W']}**"
             + (f"; belirsiz koşu (alice çıkışta teyitsiz birim > 0 ∧ süzülmeyen birim > 0, "
                f"sayılmadı): {', '.join(S['belirsiz_kosu'])}" if S["belirsiz_kosu"] else "") + ".")
    L += ["", "| hücre | dcd | Reorder atlaması | süzülmeyen birim | teyitli ∧ alınmayan TEXT | "
          "teyitli ∧ alınmayan dosya |", "|---|---|---|---|---|---|"]
    for key, e in S["hucreler"].items():
        b = e["ackli_bilesen"]
        L.append(f"| {key} | {b['dcd']} | {b['reorder_atlama']} | {b['suzulmeyen_birim']} | "
                 f"{b['teyitli_alinmayan_text']} | {b['dosya_teyitli_alinmayan']} |")
    r = S["r3_r25"]
    L += ["", "## 4. K-3 (R3-r25; ONKAYIT §3.6)", "",
          f"- {S.get('kol_b', kol)}: n = {r['kol']['n']}, teslim@60 medyan "
          f"{fmt(r['kol']['teslim60_med'])}, "
          f"gönderim medyan {fmt(r['kol']['gonderim_med_B'], 0)} B · {taban}: n = {r['taban']['n']}, "
          f"teslim@60 medyan {fmt(r['taban']['teslim60_med'])}, gönderim medyan "
          f"{fmt(r['taban']['gonderim_med_B'], 0)} B → **{fmt(r['gecer']) if r['gecer'] is not None else 'veri yok'}**"
          " (teslim@60 ≥ taban − 1 ∧ gönderim ≤ taban × 1,10)."]
    L += ["", "## 5. G-K8 (d300 hücreleri; raporlanan, N = 10 → UCB yok, betimsel)", "",
          "| hücre | tür | B1 ilk ses p95 s | B1 ilk demet p95 s | B1 eşik s | B2 60 s'de %100 koşu | "
          "B2 TEXT p95 s | B2 eşik s | B5 bozuk / sırasız | B5 D p95 s |",
          "|---|---|---|---|---|---|---|---|---|---|"]
    for key, e in S["hucreler"].items():
        g = e.get("gk8")
        if not g:
            continue
        L.append(f"| {key} | {g['tur']} | {fmt(g['B1_ilk_ses_p95_s'])} | "
                 f"{fmt(g['B1_ilk_demet_p95_s'])} | {fmt(g['B1_esik_s'])} (+T ses için) | "
                 f"{g['B2_hepsi_60s_kosu']}/{e['n']} | {fmt(g['B2_text_p95_s'])} | "
                 f"{fmt(g['B2_esik_s'])} | {g['B5_bozuk']} / {g['B5_sirasiz']} | "
                 f"{fmt(g['B5_D_p95_s'])} |")
    L += ["", "B3: 150 KB ölçütü yalnız Y10'da (d300 hücrelerinde dosya 524 700 B, bitiş §2'de). "
          "B4 (60 s kesinti): bu hücrelerde yok → ölçülemedi.", ""]
    L += ["## 6. EK-A §3.7 — callback + tur süresi (vekil) ve Kotlin tüketimi", "",
          "| hücre | TEXT: iz R → dinleyici ms (n / p50 / p95 / en çok) | PTT: ilk oynatma − (varış + T) ms "
          "(n / p50 / p95 / en çok) |", "|---|---|---|"]
    for key, e in S["hucreler"].items():
        c, k = e["cb_text_ms"], e["tampon_kalinti_ms"]
        L.append(f"| {key} | {c['n']} / {fmt(c['p50'], 1)} / {fmt(c['p95'], 1)} / "
                 f"{fmt(c['max'], 1)} | {k['n']} / {fmt(k['p50'], 1)} / {fmt(k['p95'], 1)} / "
                 f"{fmt(k['max'], 1)} |")
    L += ["", "Düğüm turu süresi doğrudan ölçülmüyor (aynada yok); vekil = aynı turdaki iz R ile "
          "dinleyici çağrısı arası. Kotlin tüketimi bu düzenekte yok → ölçülemedi.", ""]
    if v2:
        return "\n".join(L + report_v2(S))
    yz = S["yesil"]
    L += ["## 7. \"Yeşil\" satırı (ONKAYIT §3.8; dondurma şartı Paz yeniden koşusunda)", ""]
    if yz is None:
        L.append(f"- Y10 × {kol} verisi yok → hesaplanmadı.")
    else:
        L.append(f"- Y10 başarılı {yz['basarili']}/{yz['n']} (eşik ≥ {yz['esik']}): {fmt(yz['y10'])} · "
                 f"ACK'li birim kaybı = 0: {fmt(yz['ackli0'])} · R3-r25: "
                 f"{fmt(yz['r3_r25']) if yz['r3_r25'] is not None else 'veri yok'} → "
                 f"**{'yeşil' if yz['gecer'] else 'yeşil değil'}**")
    L.append("")
    return "\n".join(L)


# ── [v2] Kural katmanı (ONKAYIT-V2; saf, öz-testin konusu) ─────────────────────────────────

def r3_d1000(kol_p95, taban_p95):
    """ONKAYIT-V2 §3.11 (CC §4.2 R3, aynen): kol teyit p95 medyanı ≤ taban × 1,10."""
    if kol_p95 is None or taban_p95 is None:
        return None
    return kol_p95 <= taban_p95 * R3_KAT


def hdr_uyusmazlik(y3):
    """ONKAYIT-V2 §3.12: bir ucun başlık `yol3` nesnesi ↔ V2_HDR; uyuşmayan alanlar listesi."""
    return [k for k, v in V2_HDR.items() if (y3 or {}).get(k) != v]


def beklenen_bench(arm):
    """Bench kolunun manifest beklentisi {"e1": …, "e2a": …} (None = ürün değeri, sapma yok);
    bilinmeyen kol → None (denetlenmez)."""
    if arm == V2_TABAN:
        return {"e1": None, "e2a": None}
    if arm == V2_B_KOL:
        return {"e1": dict(GOSTER_E1), "e2a": dict(GOSTER_E2A)}
    if arm in ABL:
        _, yapi, alan = ABL[arm]
        want = {"e1": dict(GOSTER_E1), "e2a": dict(GOSTER_E2A)}
        want[yapi][alan] = False
        return want
    return None


def bench_dogrula(arm, got):
    """True = manifest beklentiyle aynı · False = farklı · None = doğrulanamadı (manifest yok)."""
    want = beklenen_bench(arm)
    if want is None or got is None:
        return None
    return got == want


def yesil_v2(y10_basarili, y10_n, ackli0, r3_r25_v, r3_d1000_v):
    """ONKAYIT-V2 §4: v1 §3.8 (Y10 ≥ 8/10 ∧ ACK'li kayıp 0 ∧ R3-r25) ∧ R3-d1000."""
    if y10_basarili is None:
        return None
    return (y10_basarili >= YESIL_MIN and bool(ackli0) and bool(r3_r25_v) and bool(r3_d1000_v))


def abl_marj(which, row, taban):
    """ONKAYIT-V2 §6.3 sıralama anahtarı (küçük = daha iyi): d1000 p95 oranı; r25 (gönderim oranı,
    −teslim farkı). Veri eksikse None."""
    if which == "d1000":
        if row.get("p95_med") is None or not taban.get("p95_med"):
            return None
        return (round(row["p95_med"] / taban["p95_med"], 3),)
    if None in (row.get("tx_med"), row.get("teslim_med")) or not taban.get("tx_med") \
            or taban.get("teslim_med") is None:
        return None
    return (round(row["tx_med"] / taban["tx_med"], 3), -(row["teslim_med"] - taban["teslim_med"]))


def abl_r3(which, row, taban):
    if which == "d1000":
        return r3_d1000(row.get("p95_med"), taban.get("p95_med"))
    return r3_r25(row.get("teslim_med"), row.get("tx_med"), taban.get("teslim_med"),
                  taban.get("tx_med"))


def abl_karar(which, rows):
    """ONKAYIT-V2 §6 karar kuralı. rows: {kol: {teslim_med, tx_med, p95_med, n}} (K0, K1A6,
    K1A6mA1…mA6). Döner {"durum", "dusen" (A-i | None), "aday", "r3", "marj"}."""
    taban, d = rows.get(V2_TABAN), rows.get(V2_B_KOL)
    out = {"durum": None, "dusen": None, "aday": [], "r3": {}, "marj": {}}
    if not taban or not taban.get("n"):
        out["durum"] = "veri yok (taban K0 yok)"
        return out
    for arm in ABL:
        row = rows.get(arm) or {}
        out["r3"][arm] = abl_r3(which, row, taban) if row.get("n") else None
        out["marj"][arm] = abl_marj(which, row, taban) if row.get("n") else None
    d_r3 = abl_r3(which, d, taban) if d and d.get("n") else None
    out["r3"][V2_B_KOL] = d_r3
    out["aday"] = [arm for arm in ABL if out["r3"][arm] is True]
    if d_r3 is None:
        out["durum"] = "veri yok (gösterim seti K1A6 bu kökte yok)"
    elif d_r3:
        out["durum"] = "ihlal bu kökte yeniden üretilmedi → bayrak düşmez"
    elif not out["aday"]:
        out["durum"] = "tek bayrak açıklamıyor → bayrak düşmez (sahibe)"
    elif len(out["aday"]) == 1:
        out["durum"] = "tek aday"
        out["dusen"] = ABL[out["aday"][0]][0]
    else:
        # Beraberlik: en iyi marj; marj eşitse numarası büyük A bayrağı (A-6 > … > A-1).
        best = min(out["aday"], key=lambda a: (out["marj"][a] or (math.inf,),
                                              -int(ABL[a][0][2:])))
        out["durum"] = f"beraberlik ({len(out['aday'])} aday) → en iyi marj"
        out["dusen"] = ABL[best][0]
    return out


def kol_dogrulama_v2(d):
    """ONKAYIT-V2 §3.12: A hücrelerinde iki ucun başlığı, b hücrelerinde istemci manifest'i."""
    out = {"A": {}, "B": {}}
    for base, arms in sorted(d["A"].items()):
        runs = arms.get(V2_A_KOL, [])
        bad = []
        for r in runs:
            ali = {"e1_arm": r.get("e1_arm"), "flag_src": r.get("flag_src"),
                   "e1_set": r.get("e1_set"), "e2a_set": r.get("e2a_set"),
                   "fec_off": r.get("fec_off"), "goster": r.get("goster"),
                   "lanes_w": r.get("lanes_w"), "playout_ms": r.get("playout_ms")}
            u = hdr_uyusmazlik(ali) + [f"bob.{k}" for k in hdr_uyusmazlik(r.get("bob_yol3"))]
            if u:
                bad.append(f"rep{r['rep']}: {', '.join(u)}")
        out["A"][f"{base}__{V2_A_KOL}"] = {"n": len(runs), "uyusmayan": bad}
    for base, arms in sorted(d["B"].items()):
        for arm, runs in sorted(arms.items()):
            v = [bench_dogrula(arm, r.get("bayrak")) for r in runs]
            out["B"][f"{base}__{arm}"] = {
                "n": len(runs), "dogru": sum(1 for x in v if x is True),
                "yanlis": [f"rep{r['rep']}" for r, x in zip(runs, v) if x is False],
                "dogrulanamadi": sum(1 for x in v if x is None)}
    out["gecer"] = (all(not e["uyusmayan"] for e in out["A"].values())
                    and all(not e["yanlis"] for e in out["B"].values()))
    return out


def t1_denetim(d):
    """ONKAYIT-V2 §3.13: A hücrelerinin her koşusunda iki uçta da ts `t1` nesnesi var mı."""
    eksik = []
    for base, arms in sorted(d["A"].items()):
        for r in arms.get(V2_A_KOL, []):
            for who, ok in (r.get("t1_uclar") or {}).items():
                if not ok:
                    eksik.append(f"{base}__{V2_A_KOL}/rep{r['rep']}/{who}")
            if not r.get("t1_uclar"):
                eksik.append(f"{base}__{V2_A_KOL}/rep{r['rep']}")
    return eksik


def summarize_v2(d):
    S = summarize(d, V2_A_KOL, V2_TABAN, kol_b=V2_B_KOL)
    rb = d["B"].get(D1000, {})
    kp = med([r["p95_s"] for r in rb.get(V2_B_KOL, [])])
    tp = med([r["p95_s"] for r in rb.get(V2_TABAN, [])])
    S["r3_d1000"] = {"kol": {"n": len(rb.get(V2_B_KOL, [])), "p95_med_s": kp},
                     "taban": {"n": len(rb.get(V2_TABAN, [])), "p95_med_s": tp},
                     "oran": (kp / tp) if kp is not None and tp else None,
                     "gecer": r3_d1000(kp, tp)}
    r25 = d["B"].get(R25, {})
    S["r3_r25"]["kol"]["tamam"] = sum(1 for r in r25.get(V2_B_KOL, [])
                                      if r.get("teslim60") is not None
                                      and r["teslim60"] >= MSGS_R25)
    S["r3_r25"]["taban"]["tamam"] = sum(1 for r in r25.get(V2_TABAN, [])
                                        if r.get("teslim60") is not None
                                        and r["teslim60"] >= MSGS_R25)
    S["r3_r25"]["oran"] = (S["r3_r25"]["kol"]["gonderim_med_B"]
                           / S["r3_r25"]["taban"]["gonderim_med_B"]
                           if S["r3_r25"]["kol"]["gonderim_med_B"] is not None
                           and S["r3_r25"]["taban"]["gonderim_med_B"] else None)
    S["kol_dogrulama"] = kol_dogrulama_v2(d)
    S["t1_eksik"] = t1_denetim(d)
    y = S["yesil"]
    S["yesil_v2"] = None if y is None else {
        "basarili": y["basarili"], "n": y["n"], "esik": YESIL_MIN, "y10": y["y10"],
        "ackli0": y["ackli0"], "r3_r25": S["r3_r25"]["gecer"], "r3_d1000": S["r3_d1000"]["gecer"],
        "gecer": yesil_v2(y["basarili"], y["n"], y["ackli0"], S["r3_r25"]["gecer"],
                          S["r3_d1000"]["gecer"]),
        "gecerli": S["kol_dogrulama"]["gecer"] and not S["t1_eksik"]}
    S["ablasyon_tetik"] = {"r25": S["r3_r25"]["gecer"] is False,
                           "d1000": S["r3_d1000"]["gecer"] is False}
    return S


def t1_tablo(e):
    """v2 madde 5 T1 satırı (koşular arası en yüksek / son değer toplamı)."""
    if e["t1"] is None:
        return " | ".join(["EKSİK"] + ["—"] * 6)
    hi, last = e["t1"]["en_yuksek"], e["t1"]["son_toplam"]

    def sm(pre):
        return sum(v for k, v in last.items() if k.startswith(pre))
    return (f"{hi.get('bob.po.peak', '—')} | {last.get('bob.po.st', '—')} / "
            f"{last.get('bob.po.stms', '—')} | {last.get('bob.po.disc', '—')} / "
            f"{last.get('bob.po.late', '—')} / {last.get('bob.po.ov', '—')} | {sm('alice.ln.ref[')} | "
            f"{last.get('alice.ln.drp', '—')} / {last.get('alice.ln.exp', '—')} | "
            f"{hi.get('alice.bl[0]', '—')} | {hi.get('alice.bl[1]', '—')}")


def report_v2(S):
    """v2 bölümleri (v1 §1–§6 report()'tan aynen; v1 §7 yerine bunlar)."""
    L = []
    r = S["r3_d1000"]
    r25 = S["r3_r25"]
    L += ["## 7. R3 koruma hücreleri (ONKAYIT-V2 §3.6, §3.11; taban K0 aynı kök ve ikili)", "",
          "| hücre | kol | n | ölçü | kol medyanı | taban K0 medyanı | oran | R3 |", "|---|---|---|---|---|---|---|---|",
          f"| {R25} | {V2_B_KOL} | {r25['kol']['n']} / K0 {r25['taban']['n']} | teslim@60 · gönderim B | "
          f"{fmt(r25['kol']['teslim60_med'])} · {fmt(r25['kol']['gonderim_med_B'], 0)} | "
          f"{fmt(r25['taban']['teslim60_med'])} · {fmt(r25['taban']['gonderim_med_B'], 0)} | "
          f"{fmt(r25.get('oran'), 3)} | "
          f"**{fmt(r25['gecer']) if r25['gecer'] is not None else 'veri yok'}** |",
          f"| {D1000} | {V2_B_KOL} | {r['kol']['n']} / K0 {r['taban']['n']} | teyit p95 s | "
          f"{fmt(r['kol']['p95_med_s'])} | {fmt(r['taban']['p95_med_s'])} | {fmt(r['oran'], 3)} | "
          f"**{fmt(r['gecer']) if r['gecer'] is not None else 'veri yok'}** |", "",
          f"r25 betimsel: bütün mesajlar 60 s içinde {V2_B_KOL} {r25['kol'].get('tamam')}/"
          f"{r25['kol']['n']} · K0 {r25['taban'].get('tamam')}/{r25['taban']['n']}.", ""]
    L += ["## 8. Madde 5 — T1 sayaçları (ONKAYIT-V2 §3.13; raporlanan)", "",
          "| hücre | tampon tepe (demet) | T1 duraklama Σ / ms | atılan / geç elenen / taşma Σ | "
          "şerit ret Σ | şerit düşen / süresi dolan Σ | şerit kuyruğu tepe (örn.) | "
          "dosya penceresi tepe (örn.) |", "|---|---|---|---|---|---|---|---|"]
    for key, e in S["hucreler"].items():
        L.append(f"| {key} | {t1_tablo(e)} |")
    L += ["", "Tepe değerleri ts satırlarından (≈ 200 ms); `po.peak` T1'in kendi en yüksek doluluk "
          "sayacıdır (koşu içi). T1 duraklama sayacı düğüm başına bütün akışları sayar; bağlayıcı "
          "duraklama ölçüsü §3.2'dir (KK-Ö-40).", ""]
    k = S["kol_dogrulama"]
    L += ["## 9. Geçerlilik (ONKAYIT-V2 §3.12, §3.13)", ""]
    for key, e in k["A"].items():
        L.append(f"- {key}: başlık (iki uç) {e['n'] - len(e['uyusmayan'])}/{e['n']} beklenen set"
                 + (f"; UYUŞMAYAN: {'; '.join(e['uyusmayan'])}" if e["uyusmayan"] else ""))
    for key, e in k["B"].items():
        L.append(f"- {key}: manifest {e['dogru']}/{e['n']} beklenen set, doğrulanamadı "
                 f"{e['dogrulanamadi']}" + (f"; YANLIŞ: {', '.join(e['yanlis'])}" if e["yanlis"] else ""))
    if not S["t1_eksik"]:
        L.append(f"- T1 sayaçları: {V2_A_KOL} A koşularının hepsinde iki uçta var.")
    else:
        say = {}
        for x in S["t1_eksik"]:
            say[x.split("/")[0]] = say.get(x.split("/")[0], 0) + 1
        L.append("- T1 sayaçları EKSİK (koşu × uç): "
                 + ", ".join(f"{k} {v}" for k, v in sorted(say.items()))
                 + " (ayrıntı okuma.json `t1_eksik`).")
    L.append(f"- Kol doğrulaması: **{'geçti' if k['gecer'] else 'KALDI'}**")
    L.append("")
    yz = S["yesil_v2"]
    L += ["## 10. \"Yeşil\" satırı (ONKAYIT-V2 §4 = v1 §3.8 ∧ R3-d1000)", ""]
    if yz is None:
        L.append(f"- Y10 × {V2_A_KOL} verisi yok → hesaplanmadı.")
    else:
        L.append(f"- Y10 başarılı {yz['basarili']}/{yz['n']} (eşik ≥ {yz['esik']}): {fmt(yz['y10'])} · "
                 f"ACK'li birim kaybı = 0: {fmt(yz['ackli0'])} · R3-r25: "
                 f"{fmt(yz['r3_r25']) if yz['r3_r25'] is not None else 'veri yok'} · R3-d1000: "
                 f"{fmt(yz['r3_d1000']) if yz['r3_d1000'] is not None else 'veri yok'} → "
                 f"**{'yeşil' if yz['gecer'] else 'yeşil değil'}**"
                 + ("" if yz["gecerli"] else " — **GEÇERSİZ KOŞU** (§9: kol doğrulaması ya da T1)"))
    t = S["ablasyon_tetik"]
    L += ["", "## 11. Ablasyon tetiği (ONKAYIT-V2 §6.1)", "",
          f"- r25: {'TETİK — SIRA-ABL-r25.txt' if t['r25'] else 'yok'} · d1000: "
          f"{'TETİK — SIRA-ABL-d1000.txt' if t['d1000'] else 'yok'} (R3 \"veri yok\" tetik "
          "değildir; koşu kaydı incelenir).", ""]
    return L


def abl_rows(d, which):
    base = ABL_HUCRE[which]
    rows = {}
    for arm, runs in (d["B"].get(base) or {}).items():
        rows[arm] = {"n": len(runs), "rc0": sum(1 for r in runs if r["rc"] == 0),
                     "teslim_med": med([r["teslim60"] for r in runs]),
                     "tx_med": med([r["gonderim_B"] for r in runs]),
                     "p95_med": med([r["p95_s"] for r in runs]),
                     "dogrula": [bench_dogrula(arm, r.get("bayrak")) for r in runs]}
    return rows


def report_abl(which, rows, K, onkayit_line):
    L = [f"# mini-Ö v2 ablasyonu — {ABL_HUCRE[which]}", "",
         f"Ön kayıt: {onkayit_line} (§6). Etiket: gösterim; kapılı PoC ölçümü Aralık; ARQ yolu, FEC "
         "Aralık'ta; N < 30 betimsel; sığ tamponlu ağ gösterim kapsamı dışı. Değerler [ölçüldü], "
         "oran ve karar [türetilmiş].", "",
         "| kol | çıkan bayrak | n | rc=0 | teslim@60 med | gönderim med B | teyit p95 med s | "
         "marj | R3 (taban K0) | manifest doğru |", "|---|---|---|---|---|---|---|---|---|---|"]
    for arm in [V2_TABAN, V2_B_KOL] + list(ABL):
        r = rows.get(arm)
        if r is None:
            L.append(f"| {arm} | {ABL.get(arm, ('—',))[0]} | 0 | — | — | — | — | — | veri yok | — |")
            continue
        dv = r["dogrula"]
        L.append(f"| {arm} | {ABL[arm][0] + ' ' + ABL[arm][2] if arm in ABL else '—'} | {r['n']} | "
                 f"{r['rc0']} | {fmt(r['teslim_med'])} | {fmt(r['tx_med'], 0)} | {fmt(r['p95_med'])} | "
                 f"{K['marj'].get(arm) if arm in ABL else '—'} | "
                 f"{'taban' if arm == V2_TABAN else fmt(K['r3'].get(arm)) if K['r3'].get(arm) is not None else 'veri yok'} | "
                 f"{sum(1 for x in dv if x is True)}/{len(dv)} |")
    L += ["", f"- Durum: **{K['durum']}**",
          f"- Aday (çıkınca R3 geçen): {', '.join(ABL[a][0] for a in K['aday']) or '—'}",
          f"- Düşen bayrak: **{K['dusen'] or 'yok'}**", ""]
    return "\n".join(L)


def onkayit_line(path, want, name):
    """(satır, hata kodu | None)."""
    if not path:
        return "ön kayıt denetlenmedi (araç sınaması)", None
    got = sha256(path)
    if got != want:
        print(f"ön kayıt sha256 uyuşmuyor: {got} ≠ {want}; okuma YAPILMADI", file=sys.stderr)
        return None, 3
    return f"{name} sha256 {got} (denetlendi)", None


def main_read_v(a):
    kok = pathlib.Path(a.kok).resolve()
    line, err = onkayit_line(a.onkayit, ONKAYIT_V2_SHA256, "ONKAYIT-V2.md")
    if err:
        return err
    d = extract(kok)
    out = pathlib.Path(a.out).resolve() if a.out else kok / "okuma"
    out.mkdir(parents=True, exist_ok=True)
    if a.surum == "v2":
        S = summarize_v2(d)
        md = report(S, line)
        js = {"onkayit": line, "oku_py_sha256": sha256(__file__), "surum": "v2", "ozet": jsafe(S)}
    else:
        if a.hucre not in ABL_HUCRE:
            print("--surum abl: --hucre r25|d1000 gerekli", file=sys.stderr)
            return 2
        rows = abl_rows(d, a.hucre)
        K = abl_karar(a.hucre, rows)
        md = report_abl(a.hucre, rows, K, line)
        js = {"onkayit": line, "oku_py_sha256": sha256(__file__), "surum": "abl",
              "hucre": a.hucre, "satirlar": jsafe(rows), "karar": jsafe(K)}
    (out / "OKUMA.md").write_text(md, encoding="utf-8")
    (out / "okuma.json").write_text(json.dumps(js, ensure_ascii=False, indent=1, sort_keys=True),
                                    encoding="utf-8")
    print(md)
    print(f"→ {out}/OKUMA.md, {out}/okuma.json")
    return 0


def main_read(a):
    kok = pathlib.Path(a.kok).resolve()
    if a.onkayit:
        got = sha256(a.onkayit)
        if got != ONKAYIT_SHA256:
            print(f"ön kayıt sha256 uyuşmuyor: {got} ≠ {ONKAYIT_SHA256}; okuma YAPILMADI",
                  file=sys.stderr)
            return 3
        line = f"ONKAYIT.md sha256 {got} (denetlendi)"
    else:
        line = "ön kayıt denetlenmedi (araç sınaması)"
    d = extract(kok)
    S = summarize(d, a.kol, a.taban)
    out = pathlib.Path(a.out).resolve() if a.out else kok / "okuma"
    out.mkdir(parents=True, exist_ok=True)
    md = report(S, line)
    (out / "OKUMA.md").write_text(md, encoding="utf-8")
    js = {"onkayit": line, "oku_py_sha256": sha256(__file__), "ozet": jsafe(S)}
    (out / "okuma.json").write_text(json.dumps(js, ensure_ascii=False, indent=1, sort_keys=True),
                                    encoding="utf-8")
    print(md)
    print(f"→ {out}/OKUMA.md, {out}/okuma.json")
    return 0


# ── Öz-test ──────────────────────────────────────────────────────────────────────────────────

def oz_test():
    ok_all = True

    def chk(name, cond):
        nonlocal ok_all
        print(("geçti  " if cond else "KALDI  ") + name)
        ok_all = ok_all and bool(cond)

    def P(sp, i, send, rel, rx):
        return {"raw_c": "P", "sp": sp, "i": i, "t_send": send, "t_released": rel, "t_rx": rx}

    o = {"raw_c": "O", "sp": 0, "i": 0, "t_send": 0.0}
    # Basış: açılış 0; demet i gönderimi 200(i+1); varış 1000 + 200 i; i = 2 geç (+500 ms).
    items = [o] + [P(0, i, 200 * (i + 1), 1000 + 200 * i + (500 if i == 2 else 0),
                     3000 + 200 * i + (300 if i >= 2 else 0)) for i in range(5)]
    pm = press_metrics(items, 0, 200, 2000)
    chk("ilk demet = varış(0) − açılış = 1,0 s", pm["ilk_demet_s"] == 1.0)
    chk("ilk ses = oynatma(0) − açılış = 3,0 s", pm["ilk_ses_s"] == 3.0)
    chk("bütün demetler varış p95 = en geç (r_2 − s_2) = 1,3 s", abs(pm["varis_p95_s"] - 1.3) < 1e-9)
    chk("D = max(r_i − r_0 − 200 i) = 0,5 s", abs(pm["D_s"] - 0.5) < 1e-9)
    chk("doğrudan duraklama: 1 aralık 500 ms > 240 → 1, 300 ms", pm["duraklama_n"] == 1
        and abs(pm["duraklama_ms"] - 300) < 1e-9)
    chk("biriken gecikme son = 0,3 s", abs(pm["biriken_son_s"] - 0.3) < 1e-9)
    chk("tampon kalıntısı = (3000 − 1000)/1000 − 2,0 = 0", abs(pm["tampon_kalinti_s"]) < 1e-9)
    chk("kesintisiz (tampon açık, doğrudan) = hayır", kesintisiz(pm, 2000) is False)
    # Tolerans içi (≤ 240 ms) aralık duraklama sayılmaz; sırasız sayılır.
    it2 = [o] + [P(0, i, 200 * (i + 1), 1000 + 200 * i, 3000 + 200 * i + (35 if i == 1 else 0))
                 for i in range(3)]
    pm2 = press_metrics(it2, 0, 200, 2000)
    chk("235 ms aralık duraklama değil", pm2["duraklama_n"] == 0 and kesintisiz(pm2, 2000))
    it3 = [o, P(0, 0, 200, 1000, 3000), P(0, 1, 400, 1200, 2900)]
    chk("geri giden oynatma sırasız = 1", press_metrics(it3, 0, 200, 2000)["sirasiz"] == 1)
    # Eksik demet: D = ∞, doğrudan ölçü tanımsız → kesintisiz değil.
    it4 = [o, P(0, 0, 200, 1000, 3000), P(0, 1, 400, None, None)]
    pm4 = press_metrics(it4, 0, 200, 2000)
    chk("teslimsiz demet: D = ∞, kesintisiz değil", pm4["D_s"] == INF and not kesintisiz(pm4, 2000))
    # Eşlenmemiş varış: t_rx üst sınır.
    it5 = [o, P(0, 0, 200, None, 2500)]
    pm5 = press_metrics(it5, 0, 200, None)
    chk("eşlenmemiş varış → t_rx üst sınırı, sayılır", pm5["ilk_demet_s"] == 2.5
        and pm5["eslenmemis"] == 1)
    chk("tampon kapalı: analitik D ≤ 2,0 s", kesintisiz(pm5, None) is True)
    # TEXT penceresi ve sansür.
    T = [{"raw_c": "T", "t_send": s, "t_rx": x} for s, x in
         ((500, 900), (1000, 5000), (2000, None), (9000, 9100))]
    tm = text_metrics(T, 900, 3000)
    chk("TEXT penceresi [900, 3000]: n = 2, en büyük ∞", tm["n"] == 2 and tm["max_s"] == INF)
    chk("boş TEXT penceresi: n = 0", text_metrics(T, 3000, 4000)["n"] == 0)
    # Başarı.
    run = {"rc": 0, "playout_ms": 2000, "dosya_s": 25.0,
           "text_dosya": {"n": 3, "max_s": 4.0, "p95_s": 4.0},
           "basis": {0: dict(pm2)}}
    chk("dört ölçüt birlikte → başarılı", basari(run)[0])
    run_b = dict(run, text_dosya={"n": 0, "max_s": None, "p95_s": None})
    chk("boş TEXT penceresi → başarısız (boş geçiş yok)", not basari(run_b)[0])
    run_c = dict(run, dosya_s=30.01)
    chk("dosya 30,01 s → başarısız", not basari(run_c)[0])
    run_d = dict(run, rc=1)
    chk("rc ≠ 0 → başarısız", not basari(run_d)[0])
    # T önerisi.
    chk("T: p95(D) = 1,0 → 1,3 → [1,5] kırpma", t_oneri([0.2] * 9 + [1.0])[0] == 1.5)
    chk("T: p95(D) = 2,0 → 2,3", abs(t_oneri([2.0] * 10)[0] - 2.3) < 1e-9)
    chk("T: ∞ → 2,5 kırpma", t_oneri([0.1, INF])[0] == 2.5)
    chk("T: ölçüm yok → 2,0", t_oneri([])[0] == 2.0)
    # ACK'li kayıp.
    base = {"dcd": 0, "reorder_atlama": 0, "suzulmeyen_birim": 2, "alice_out_son": 0,
            "teyitli_alinmayan_text": 0, "dosya_teyitli_alinmayan": 0}
    chk("alice çıkışta 0 teyitsiz ∧ süzülmeyen 2 → kayıp 2", ackli_kayip({"ackli": base}) == (2, False))
    chk("alice çıkışta teyitsiz > 0 → belirsiz, sayılmaz",
        ackli_kayip({"ackli": dict(base, alice_out_son=3)}) == (0, True))
    chk("dcd 1 → kayıp 1", ackli_kayip({"ackli": dict(base, suzulmeyen_birim=0, dcd=1)}) == (1, False))
    chk("W: 0 → 5, > 0 → 4", w_karari(0) == 5 and w_karari(1) == 4)
    # R3-r25.
    chk("R3: teslim 5 ≥ 6 − 1 ∧ 1100 ≤ 1000 × 1,10 → geçer", r3_r25(5, 1100, 6, 1000) is True)
    chk("R3: gönderim 1101 > 1100 → kalır", r3_r25(6, 1101, 6, 1000) is False)
    chk("R3: veri yok → None", r3_r25(None, 1, 1, 1) is None)
    chk("nearest-rank p95 (10 değer) = en büyük", rank(list(range(10)), .95) == 9)
    # Sentetik kök uçtan uca: f0_flows düzeninde iki koşu (biri başarılı).
    with tempfile.TemporaryDirectory() as td:
        kok = pathlib.Path(td)
        for rep, late in ((1, 0), (2, 900)):
            rd = kok / f"{Y10}__K1" / "K1" / f"rep{rep}"
            rd.mkdir(parents=True)
            write_synthetic_run(rd, late)
        d = extract(kok)
        S = summarize(d, "K1", "K0")
        e = S["hucreler"][f"{Y10}__K1"]
        chk("uçtan uca: 2 koşu, 1 başarılı (geç demetli koşu duraklıyor)",
            e["n"] == 2 and e["basarili"] == 1)
        chk("uçtan uca: ilk demet 1,0 s; T önerisi [1,5; 2,5] içinde",
            e["ilk_demet_s"][0] == 1.0 and T_MIN_S <= e["T_oneri_s"] <= T_MAX_S)
        chk("uçtan uca: W = 5, R3 veri yok → yeşil değil",
            S["W"] == 5 and S["yesil"]["gecer"] is False)
        chk("uçtan uca: T1 sayaçları okunur (tampon tepe 4, şerit ret Σ 4, duraklama Σ 1)",
            e["t1"]["en_yuksek"]["bob.po.peak"] == 4
            and sum(v for k, v in e["t1"]["son_toplam"].items()
                    if k.startswith("alice.ln.ref[")) == 4
            and e["t1"]["son_toplam"]["bob.po.st"] == 1)
        md = report(S, "test")
        chk("uçtan uca: T1 özeti raporda", "tampon tepe 4 demet" in md)
        chk("uçtan uca: rapor beş bölümü yazar", all(h in md for h in (
            "## 1.", "## 2.", "## 3.", "## 4.", "## 5.", "## 6.", "## 7.")))
    oz_test_v2(chk)
    print("ÖZ-TEST " + ("TEMİZ" if ok_all else "KALDI"))
    return 0 if ok_all else 1


def oz_test_v2(chk):
    """[v2] ONKAYIT-V2 kuralları: R3-d1000, kol doğrulaması, manifest, yeşil, ablasyon kararı."""
    chk("v2 R3-d1000: 3,3 ≤ 3,0 × 1,10 → geçer", r3_d1000(3.3, 3.0) is True)
    chk("v2 R3-d1000: 3,31 > 3,30 → kalır", r3_d1000(3.31, 3.0) is False)
    chk("v2 R3-d1000: veri yok → None", r3_d1000(None, 3.0) is None)
    chk("v2 başlık: beklenen set → uyuşmazlık yok", hdr_uyusmazlik(dict(V2_HDR)) == [])
    chk("v2 başlık: flag_src yok → yakalanır",
        hdr_uyusmazlik(dict(V2_HDR, flag_src=None)) == ["flag_src"])
    e1 = debug_struct("e1_flags E1Flags { e1_startup_cwnd: true, e1_applimited_plateau: true, "
                      "e1_drainrt_gate: true, e1_probe_keep: true, retx_first: false, "
                      "a3_burst_floor: true, a4_probe_srtt: true, a6_iw_floor: true } != "
                      "E1Flags { e1_startup_cwnd: false }")
    chk("v2 manifest: Debug yapısı ilk {…}'ten ayrışır", e1 == GOSTER_E1)
    e2 = debug_struct("e2a_flags E2aFlags { tt_rfc: true, tt_granularity_ns: 1000000, "
                      "rtt_rule_v11: true, iw_idle_hold: true, pto_probes: 2, gap_ack: true, "
                      "return_prior_gated: true, pto_rfc: true } != E2aFlags { }")
    chk("v2 manifest: K1A6 beklentisi tutar", bench_dogrula("K1A6", {"e1": e1, "e2a": e2}))
    chk("v2 manifest: K0 sapmasız beklenir", bench_dogrula("K0", {"e1": None, "e2a": None}))
    chk("v2 manifest: K0'da sapma → yanlış", bench_dogrula("K0", {"e1": e1, "e2a": None}) is False)
    chk("v2 manifest: K1A6mA5 = pto_rfc kapalı, PTO 2 kalır",
        bench_dogrula("K1A6mA5", {"e1": e1, "e2a": dict(e2, pto_rfc=False)}) is True
        and beklenen_bench("K1A6mA5")["e2a"]["pto_probes"] == 2)
    chk("v2 manifest: K1A6mA6 = a6 kapalı (bench K1A)",
        beklenen_bench("K1A6mA6")["e1"] == dict(GOSTER_E1, a6_iw_floor=False))
    chk("v2 manifest: dosya yok → doğrulanamadı", bench_dogrula("K1A6", None) is None)
    chk("v2 yeşil: 8/10 ∧ kayıp 0 ∧ r25 ∧ d1000", yesil_v2(8, 10, True, True, True) is True)
    chk("v2 yeşil: d1000 kalırsa yeşil değil", yesil_v2(10, 10, True, True, False) is False)
    chk("v2 yeşil: d1000 veri yok → yeşil değil", yesil_v2(10, 10, True, True, None) is False)

    def row(teslim=6.0, tx=1000.0, p95=5.0, n=20):
        return {"n": n, "teslim_med": teslim, "tx_med": tx, "p95_med": p95}
    base = {"K0": row(tx=1000.0), "K1A6": row(tx=1200.0)}
    ok_r25 = {a: row(tx=1300.0) for a in ABL}           # hepsi kalır (1300 > 1100)
    k = abl_karar("r25", dict(base, **ok_r25))
    chk("abl: hiçbir çıkarma R3'ü düzeltmiyor → bayrak düşmez",
        k["dusen"] is None and "açıklamıyor" in k["durum"])
    k = abl_karar("r25", dict(base, **dict(ok_r25, K1A6mA5=row(tx=1050.0))))
    chk("abl: tek aday A-5 → A-5 düşer", k["dusen"] == "A-5" and k["durum"] == "tek aday")
    k = abl_karar("r25", dict(base, **dict(ok_r25, K1A6mA1=row(tx=1090.0),
                                          K1A6mA5=row(tx=1040.0))))
    chk("abl: beraberlik → en iyi gönderim oranı (A-5)", k["dusen"] == "A-5")
    k = abl_karar("r25", dict(base, **dict(ok_r25, K1A6mA2=row(tx=1040.0),
                                          K1A6mA4=row(tx=1040.0))))
    chk("abl: marj eşit → numarası büyük (A-4)", k["dusen"] == "A-4")
    k = abl_karar("r25", dict(base, K1A6=row(tx=1050.0), **ok_r25))
    chk("abl: gösterim seti bu kökte geçer → yeniden üretilmedi, düşmez",
        k["dusen"] is None and "yeniden üretilmedi" in k["durum"])
    k = abl_karar("r25", {"K1A6": row()})
    chk("abl: taban yok → veri yok", k["dusen"] is None and "veri yok" in k["durum"])
    dbase = {"K0": row(p95=4.0, n=10), "K1A6": row(p95=5.0, n=10)}
    dabl = {a: row(p95=4.8, n=10) for a in ABL}
    k = abl_karar("d1000", dict(dbase, **dict(dabl, K1A6mA6=row(p95=4.2, n=10),
                                              K1A6mA3=row(p95=4.3, n=10))))
    chk("abl d1000: iki aday → en küçük p95 oranı (A-6)",
        k["dusen"] == "A-6" and sorted(k["aday"]) == ["K1A6mA3", "K1A6mA6"])
    # Sentetik v2 A koşusu: başlık iki uçta beklenen set ve T1 iki uçta → geçerli.
    with tempfile.TemporaryDirectory() as td:
        kok = pathlib.Path(td)
        rd = kok / f"{Y10}__{V2_A_KOL}" / V2_A_KOL / "rep1"
        rd.mkdir(parents=True)
        write_synthetic_run(rd, 0, y3=dict(V2_HDR))
        rd2 = kok / f"{Y10}__{V2_A_KOL}" / V2_A_KOL / "rep2"
        rd2.mkdir(parents=True)
        write_synthetic_run(rd2, 0, y3=dict(V2_HDR, flag_src="e1-arm"))
        d = extract(kok)
        S = summarize_v2(d)
        kd = S["kol_dogrulama"]["A"][f"{Y10}__{V2_A_KOL}"]
        chk("v2 uçtan uca: rep2'nin flag_src'si yakalanır, rep1 geçer",
            kd["n"] == 2 and len(kd["uyusmayan"]) == 1 and kd["uyusmayan"][0].startswith("rep2"))
        chk("v2 uçtan uca: T1 iki uçta var", S["t1_eksik"] == [])
        chk("v2 uçtan uca: R3 veri yok → yeşil değil, tetik yok",
            S["yesil_v2"]["gecer"] is False and S["ablasyon_tetik"] == {"r25": False,
                                                                       "d1000": False})
        md = report(S, "test")
        chk("v2 uçtan uca: rapor v2 bölümlerini yazar, v1 §7 yok",
            all(h in md for h in ("## 7. R3", "## 8.", "## 9.", "## 10.", "## 11."))
            and "ONKAYIT §3.8; dondurma" not in md)


def write_synthetic_run(rd, late_ms, y3=None):
    """Öz-test: f0_flows çıktısının en küçük alt kümesi (hdr, send, rx, tr, ts, özetler). `y3`
    verilirse iki ucun başlığına o `yol3` nesnesi yazılır (v2 öz-testi); yoksa v1 başlığı."""
    t0 = 1_000_000.0
    A, B = [], []
    A.append({"k": "hdr", "role": "alice", "params": {
        "ptt_bundle_ms": 200, "ptt_bytes": 80, "text_bytes": 64, "karsi_text_bytes": 48,
        "sig_bytes": 120, "file_bytes": 150480},
        "yol3": y3 if y3 is not None else {"goster": True, "playout_ms": 2000, "lanes_w": 5,
                                           "e1_arm": "K1"}})
    if y3 is not None:
        B.append({"k": "hdr", "role": "bob", "yol3": y3})
    A.append({"k": "mark", "t": t0, "m": "t0"})
    trs, trr = [], []
    unit = 0

    def send(c, i, sp, t, ln, extra=None):
        nonlocal unit
        rec = {"k": "send", "c": c, "i": i, "sp": sp, "t": t, "tr": t + 0.1, "try": 0}
        rec.update(extra or {})
        A.append(rec)
        if ln is None:
            return None  # dosya çağrısı birim numarası tüketmez (parçalar ayrı birimlerdir)
        trs.append(["S", "SessionHandle(0)", unit, ln, t + 0.05])
        unit += 1
        return unit - 1

    send("F", 0, None, t0 + 5000, None, {"bytes": 150480})
    u = send("T", 0, None, t0 + 6000, 5 + 6 + 64, {"mid": 1})
    trr += [["D", "SessionHandle(0)", u, 75, t0 + 6400], ["R", "SessionHandle(0)", u + 1, 0, t0 + 6400]]
    B.append({"k": "rx", "t": t0 + 6420, "c": "T", "i": 0, "mid": 1, "first": True})
    A.append({"k": "rcpt", "t": t0 + 6800, "mid": 1, "c": "T", "i": 0, "first": True})
    uo = send("O", 0, 0, t0 + 15000, 5 + 6)
    trr += [["D", "SessionHandle(0)", uo, 11, t0 + 15500], ["R", "SessionHandle(0)", uo + 1, 0, t0 + 15500]]
    B.append({"k": "rx", "t": t0 + 15510, "c": "O", "stream": 0, "first": True})
    for i in range(5):
        s = t0 + 15000 + 200 * (i + 1)
        up = send("P", i, 0, s, 5 + 8 + 80)
        arr = t0 + 16000 + 200 * i + (late_ms if i == 3 else 0)
        trr += [["D", "SessionHandle(0)", up, 93, arr], ["R", "SessionHandle(0)", up + 1, 0, arr]]
        play = t0 + 18000 + 200 * i + (late_ms - 0 if i >= 3 and late_ms else 0)
        B.append({"k": "rx", "t": play, "c": "P", "sp": 0, "i": i, "stream": 0, "first": True})
    uc = send("C", 0, 0, t0 + 16200, 5 + 4)
    trr += [["D", "SessionHandle(0)", uc, 9, t0 + 17000], ["R", "SessionHandle(0)", uc + 1, 0, t0 + 17000]]
    B.append({"k": "rx", "t": t0 + 19200 + late_ms, "c": "C", "stream": 0, "first": True})
    B.append({"k": "rx", "t": t0 + 25000, "c": "F", "size": 150480, "match": True})
    A.append({"k": "tr", "t": t0 + 30000, "drop": 0, "r": trs})
    B.append({"k": "tr", "t": t0 + 30000, "drop": 0, "r": trr})
    t1a = {"ln": {"acc": [5, 0, 1, 0], "giv": [5, 0, 1, 0], "ref": [0, 0, 0, 2], "exp": 0,
                  "drp": 0, "gf": 0, "fack": 0, "ackd": 7}, "bl": [3, 5], "po": None}
    t1b = {"ln": None, "bl": None, "po": {"st": 1 if late_ms else 0, "stms": late_ms, "ov": 0,
                                          "sc": 0, "tc": 0, "disc": 0, "late": 0, "rel": 5,
                                          "peak": 4}}
    A.append({"k": "ts", "t": t0 + 30000, "out": 0, "nrd": 0, "dcd": 0, "cmd": 0, "s": [
        {"peer": "bob", "rec": {"ta": t0 + 30000, "un": 0, "txq": 0, "pl": 0, "inf": 0, "dq": 0}}],
        "t1": t1a})
    B.append({"k": "ts", "t": t0 + 30000, "out": 0, "nrd": 0, "dcd": 0, "cmd": 0, "s": [
        {"peer": "alice", "reo": {"n": unit, "h": 0, "f": None}}], "t1": t1b})
    for name, lines in (("alice.jsonl", A), ("bob.jsonl", B)):
        (rd / name).write_text("".join(json.dumps(x) + "\n" for x in lines), encoding="utf-8")
    (rd / "alice.json").write_text(json.dumps({
        "result": "complete", "ended": t0 + 40000, "listener_drops": 0,
        "sent": {"F": 1, "T": 1, "O": 1, "P": 5, "C": 1},
        "file": {"sent": t0 + 5000, "delivered": t0 + 25500}}), encoding="utf-8")
    (rd / "bob.json").write_text(json.dumps({
        "result": "alice_done", "listener_drops": 0, "duplicates": 0, "unparsed": 0,
        "counts": {"T": 1, "O": 1, "P": 5, "C": 1},
        "file": {"status": "received", "size": 150480, "match": True}}), encoding="utf-8")
    (rd / "rc").write_text("0\n", encoding="utf-8")


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--kok")
    ap.add_argument("--out")
    ap.add_argument("--onkayit")
    ap.add_argument("--kol", default="K1")
    ap.add_argument("--taban", default="K0")
    ap.add_argument("--oz-test", action="store_true")
    ap.add_argument("--surum", choices=("v1", "v2", "abl"), default="v1")
    ap.add_argument("--hucre", choices=tuple(ABL_HUCRE))
    a = ap.parse_args()
    if a.oz_test:
        return oz_test()
    if not a.kok:
        ap.error("--kok gerekli (ya da --oz-test)")
    if a.surum != "v1":
        return main_read_v(a)
    return main_read(a)


if __name__ == "__main__":
    sys.exit(main())
