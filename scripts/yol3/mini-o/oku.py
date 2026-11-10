#!/usr/bin/env python3
"""[YOL3 · Ö] mini-Ö okuması (KK-Ö-11, EK-A §6 / KK-Ö-EKA-1): ön kaydın (ONKAYIT.md) kuralları AYNEN.

    python3 -I scripts/yol3/mini-o/oku.py --kok <mini-Ö kökü> [--out <dizin>]
                                          [--onkayit scripts/yol3/mini-o/ONKAYIT.md] [--kol K1] [--taban K0]
    python3 -I scripts/yol3/mini-o/oku.py --oz-test

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

Y10 = "a_r12k_d150_l20_foto150k"
L30 = "a_r12k_d300_l30_dosyali"
L0 = "a_r12k_d300_l0_dosyali"
R25 = "b_r25_d300_l20_p65"
GK8 = {L30: "kötü", L0: "iyi"}

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
           "bundle_ms": bundle_ms}
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
                    "gonderim_B": r.get("tx_total_B"), "p95_s": r.get("conf_p95_s")})
    return out


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


def summarize(d, kol, taban):
    S = {"hucreler": {}, "kol": kol, "taban": taban}
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
    kt = med([r["teslim60"] for r in rb.get(kol, [])])
    kx = med([r["gonderim_B"] for r in rb.get(kol, [])])
    tt = med([r["teslim60"] for r in rb.get(taban, [])])
    tx = med([r["gonderim_B"] for r in rb.get(taban, [])])
    S["r3_r25"] = {"kol": {"n": len(rb.get(kol, [])), "teslim60_med": kt, "gonderim_med_B": kx},
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
    L = ["# mini-Ö okuması", "",
         f"Ön kayıt: {onkayit_line} · kol {kol} · r25 tabanı {taban}. Etiket: gösterim; kapılı PoC "
         "ölçümü Aralık; ARQ yolu, FEC Aralık'ta; N < 30 betimsel. Değerler [ölçüldü] (ham veriden, "
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
          f"- {kol}: n = {r['kol']['n']}, teslim@60 medyan {fmt(r['kol']['teslim60_med'])}, "
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
    print("ÖZ-TEST " + ("TEMİZ" if ok_all else "KALDI"))
    return 0 if ok_all else 1


def write_synthetic_run(rd, late_ms):
    """Öz-test: f0_flows çıktısının en küçük alt kümesi (hdr, send, rx, tr, ts, özetler)."""
    t0 = 1_000_000.0
    A, B = [], []
    A.append({"k": "hdr", "role": "alice", "params": {
        "ptt_bundle_ms": 200, "ptt_bytes": 80, "text_bytes": 64, "karsi_text_bytes": 48,
        "sig_bytes": 120, "file_bytes": 150480},
        "yol3": {"goster": True, "playout_ms": 2000, "lanes_w": 5, "e1_arm": "K1"}})
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
    a = ap.parse_args()
    if a.oz_test:
        return oz_test()
    if not a.kok:
        ap.error("--kok gerekli (ya da --oz-test)")
    return main_read(a)


if __name__ == "__main__":
    sys.exit(main())
