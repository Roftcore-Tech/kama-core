#!/usr/bin/env python3
"""[YOL3 · A-6 gece] Okuma: hücre × kol, koşular üstünden (sahip kararı v4 EK-A §9.3, §10.2,
§10.4, §2.1; KK-Ö-EKA-2…4). Eşik YOK; bütün sütunlar betimsel (N = 10 < 30). Ham veriye YAZMAZ.

    python3 -I scripts/yol3/tesis/oku_a6.py --kok <kök> [--kok <kök2> …] [--out <dizin>]
                                           [--r3-taban <K0 kolu olan kök>]
    python3 -I scripts/yol3/tesis/oku_a6.py --oz-test

Çıktı: <ilk kök>/okuma-a6/{OKUMA-A6.md, okuma-a6.json} (ya da --out). Hücreler gen_cells.py
--set a6 (cells-a6/, SIRA-A6.txt); eski kökler de okunur (kol adı dizinden, goster/quinn başlıktan).
Birim = koşu; medyan koşu değerleri üzerinden, p90 = nearest-rank (f0a_tam.rank), en kötü = en
büyük. Tanımlar:

  Tamamlanma (rc = 0 ≠ tamamlandı; EK-A §2.3): dosya teslim = bob.json `file.status` received ∧
      `match` true; PTT kapandı = planlanan her basışın (karşı örnek sp 90 hariç) kapanışı (C) bob'a
      ulaştı. Eksik koşu = ikisinden biri yok; kimliği `rep<N>` + neden (rc, alice sonucu/hata).
  Dosya bitiş = f0a_analyze `file.bob_received_ms` (bob dosya tamam − alice send_file); alınmayan
      = ∞ (oku.py (i) ile aynı sansür). Faydalı debi = dosya baytı / dosya bitiş medyanı
      (reports/yol3/2026-11-09/BULGULAR.md kuralı).
  Tel debisi, retx payı = reports/yol3/2026-11-09/ayristirma_util.py mantığı: pencere alice
      `file.sent` → `file.delivered` (alındı); uçlarda gönderici aynasının uçtan önceki son örneği.
      Kama: Δtxb / süre, Δrtxb / Δtxb (alice `ts`, eş bob oturumu; katmanı BULGULAR'da doğrulanmadı).
      quinn: `qs` Δtx_b (UDP gönderilen bayt) / süre, Δlost_b / Δtx_b — VEKİL: quinn yeniden gönderim
      baytını açmaz, kayıp ilan edilen bayt yazılır. `file.delivered` yoksa koşu bu sütunlara girmez.
  t(%10) = `file.sent`'ten gönderici sayacının (Kama txb, quinn tx_b) dosya boyunun %10'u kadar
      arttığı ilk örneğe dek süre: gönderici txb (yeniden gönderim ve dosya dışı trafik dahil), alıcı
      ilerlemesi DEĞİL; çözünürlük örnek aralığı (probe_ms, 200 ms; üst sınır). Ulaşılmadı = ∞.
  PTT (EK-A §10.4; üç ayrı sütun, karşı örnek basışı hariç; anlar alice gönderiminden):
      ilk demet varışı = her basışın ilk demetinin (i = 0) varışı, koşu başına basışların en geci;
      bütün demetler varış p95 = koşu başına nearest-rank p95 (oku.py `ptt_varis`; teslim edilmeyen
      demet sansürlü: ended − t_send); ilk duyulabilir ses = yalnız s-kollarında (başlık
      `yol3.goster`) her basışın ilk demetinin bob `PttBundle` olayı (oynatma tamponu sonrası),
      basışların en geci; şeritsiz Kama ve quinn kollarında "—". Varış: Kama = bob Transport →
      Application (iz R, `t_released`); quinn = bob stream okuma anı (tampon yok).
  A-6 kanıtı (EK-A §10.2; başlık `yol3.e1_set` a6_iw_floor içeren kollar): alice `ts` satırındaki
      eş bob oturumunun `a6` nesnesi (alan adları A6_KEYS). Etkinleşti = dosya penceresinde
      (file.sent → file.delivered, yoksa koşu sonu) tabanın hızı yükselttiği karar sayısı arttı;
      etkinleşme anı = tabanın kurulu görüldüğü ilk örnek − file.sent; kalkış nedeni ve anı; ısınmada
      tam tur = ilk nitelikli tam tur dosyadan ÖNCE kapandı mı (+ motorun ilk kurulma anı kaydı).
      Etkinleşmeyen koşu "A-6 ölçülmedi" sayılır. Aynada `a6` yoksa bölüm "aynada yok" yazar.
  netem: limit = hücrenin realized_fwd.txt'sindeki `limit N` (q_limit kolu ayrı); ileri düşüş payı =
      realized_post netem (mb0) drops / (packets + drops) — kayıp ve kuyruk taşması BİRLİKTE.
  b hücreleri (R3; oku.py tanımı aynen): d1000 l0 p95 = analiz_b `conf_p95_s`; r25 teslim@60 =
      `deliv_D` × `n_msgs` (adet), gönderim = `tx_total_B`; kol medyanları; tamamlanma = koşu
      başına bütün mesajlar D içinde. `--r3-taban` verilirse aynı hücrelerin K0 kolu o kökten okunur
      ve oku.py `r3` kuralı (l0 p95 ≤ K0 × 1,10; r25 teslim ≥ K0 − 1 ∧ gönderim ≤ K0 × 1,10) kol
      başına uygulanır — taban farklı ağaç ve ikili (karma sürüm), satırda etiketli.
"""

import argparse
import json
import math
import os
import pathlib
import re
import subprocess
import sys
import tempfile

sys.dont_write_bytecode = True  # F0 analizörleri ve oku.py içe aktarılır; depoya __pycache__ yok
HERE = pathlib.Path(__file__).resolve().parent
REPO = HERE.parents[2]
F0 = REPO / "scripts" / "ktp2" / "f0"
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(F0 / "a"))
sys.path.insert(0, str(F0 / "b"))
import oku  # noqa: E402  (ptt_varis, med, fmt, r3, L0, R25: tek kaynak)
import f0a_tam  # noqa: E402  (rank, realized_loss)

INF = math.inf
KARSI_SPURT = 90
# Kol gösterim sırası: A-6 yedisi, q_limit kolları, sonra eski köklerin kolları (alfabetik).
ORDER = ["K1", "K1A6", "K1A", "K1s", "K1A6s", "Q_bbr_rfc", "Q_cubic", "K1A6s_qlim",
         "Q_bbr_rfc_qlim", "K0", "K0r", "K2", "K2mc", "K2me", "K1a", "K0a", "K1g", "K1gr",
         "K1grp", "Q_bbr"]
LIMIT_RE = re.compile(r"\blimit (\d+)\b")
# [Q2 A-6] `ts` oturum nesnesindeki `a6` alanlarının adları (f0_flows aynası; Q2 API'si gelince
# kesinleşir). Anlam: on = bayrak açık · arms = kurulma sayısı · raise = tabanın hızı yükselttiği
# karar sayısı (birikimli) · armed = şu an kurulu · rel = son kalkış nedeni (null = kalkmadı) ·
# fr = ilk nitelikli tam tur kapandı mı · rda = ilk kurulmada tur izleyicisi tam tur görmüş müydü.
A6_KEYS = {"on": "on", "arms": "arms", "raise": "raise", "armed": "armed", "rel": "rel",
           "fr": "fr", "rda": "rda"}


def rank(vals, p):
    return f0a_tam.rank(vals, p)


def med(xs):
    return oku.med(xs)


def fmt(x, nd=2):
    return oku.fmt(x, nd)


def read_jsonl(path):
    out = []
    try:
        with open(path, encoding="utf-8") as f:
            for ln in f:
                ln = ln.strip()
                if ln:
                    try:
                        out.append(json.loads(ln))
                    except ValueError:
                        continue
    except OSError:
        pass
    return out


def read_json(path):
    try:
        with open(path, encoding="utf-8") as f:
            return json.load(f)
    except (OSError, ValueError):
        return None


# ── Koşu düzeyi (saf; öz-testin konusu) ──────────────────────────────────────────────────────

def sender_series(alice, kind):
    """Gönderici sayaçları [(t ms, tx B, retx B)]: Kama `ts` eş bob (txb, rtxb); quinn `qs`
    (tx_b, lost_b — retx VEKİLİ)."""
    out = []
    for ln in alice:
        if kind == "quinn":
            if ln.get("k") == "qs" and ln.get("tx_b") is not None:
                out.append((ln["t"], ln["tx_b"], ln.get("lost_b") or 0))
        elif ln.get("k") == "ts":
            s = next((x for x in ln.get("s", []) if x.get("peer") == "bob"), None)
            if s is not None and s.get("txb") is not None:
                out.append((ln["t"], s["txb"], s.get("rtxb") or 0))
    out.sort(key=lambda x: x[0])
    return out


def at(ser, t):
    """ayristirma_util `at`: t'den önceki (≤) son örnek."""
    best = None
    for x in ser:
        if x[0] <= t:
            best = x
        else:
            break
    return best


def wire_window(ser, t0, t1):
    """(tel B/s, retx payı) dosya penceresinde; uçlardan biri yoksa (None, None)."""
    if t0 is None or t1 is None or t1 <= t0:
        return None, None
    s0, s1 = at(ser, t0), at(ser, t1)
    if s0 is None or s1 is None:
        return None, None
    dur = (t1 - t0) / 1000.0
    tx, rtx = s1[1] - s0[1], s1[2] - s0[2]
    return tx / dur, rtx / max(tx, 1)


def t_frac(ser, t0, target):
    """t0'dan gönderici sayacının `target` B arttığı ilk örneğe dek süre (s); ulaşılmadı = ∞."""
    if t0 is None or not ser:
        return None
    s0 = at(ser, t0)
    base = s0[1] if s0 is not None else 0
    for t, tx, _ in ser:
        if t > t0 and tx - base >= target:
            return (t - t0) / 1000.0
    return INF


def ptt_cols(items, ended, kind, goster):
    """(ilk demet varışı s, bütün demetler varış p95 s, ilk duyulabilir ses s | None,
    teslim edilmeyen demet). Varış oku.ptt_varis ile AYNI (quinn'de t_released yok → rx)."""
    p95, first, _ = oku.ptt_varis(items, ended, rank)
    play = {}
    undeliv = 0
    for it in items:
        if it.get("c") != "P":
            continue
        if it.get("t_rx") is None:
            undeliv += 1
            v = (ended - it["t_send"]) if ended else INF
        else:
            v = it["t_rx"] - it["t_send"]
        if it.get("i") == 0:
            play[it.get("sp")] = v
    ses = max(play.values()) / 1000.0 if (goster and kind == "kama" and play) else None
    return (first / 1000.0 if first is not None else None,
            p95 / 1000.0 if p95 is not None else None, ses, undeliv)


def completion(rj, items):
    """(dosya teslim, PTT kapandı, neden listesi)."""
    b = rj.get("bob_summary") or {}
    a = rj.get("alice_summary") or {}
    bf = b.get("file") or {}
    prm = rj.get("params") or {}
    has_file = rj.get("scenario") not in ("dosyasiz", "dosyasiz_dur")
    file_ok = (bf.get("status") == "received" and bf.get("match") is True) if has_file else None
    presses = len(prm.get("ptt_at_ms") or [])
    closes = [it for it in items if it.get("c") == "C" and it.get("sp") != KARSI_SPURT]
    ptt_ok = (presses > 0 and len(closes) >= presses
              and all(it.get("t_rx") is not None for it in closes))
    why = []
    if file_ok is False:
        why.append(f"dosya {bf.get('status') or 'yok'}")
    if not ptt_ok:
        why.append(f"PTT kapanmadı ({sum(1 for it in closes if it.get('t_rx') is not None)}"
                   f"/{presses})")
    if why:
        if rj.get("rc") != 0:
            why.append(f"rc={rj.get('rc')}")
        if a.get("result") not in (None, "complete"):
            why.append(f"alice {a.get('result')}")
        if a.get("error"):
            why.append(str(a["error"])[:60])
    return file_ok, ptt_ok, why


def a6_evidence(alice, t_file, t_end):
    """A-6 deney kaydı (EK-A §10.2) alice ts → eş bob oturumu `a6` nesnesi; yoksa None."""
    k = A6_KEYS
    samples = []
    for ln in alice:
        if ln.get("k") != "ts":
            continue
        s = next((x for x in ln.get("s", []) if x.get("peer") == "bob"), None)
        if s is not None and isinstance(s.get("a6"), dict):
            samples.append((ln["t"], s["a6"]))
    if not samples or not any(o.get(k["on"]) for _, o in samples):
        return None
    samples.sort(key=lambda x: x[0])

    def val_at(t, key):
        best = None
        for ts, o in samples:
            if ts <= t:
                best = o.get(key)
            else:
                break
        return best

    end = t_end if t_end is not None else samples[-1][0]
    r0 = val_at(t_file, k["raise"]) if t_file is not None else 0
    r1 = val_at(end, k["raise"])
    act = r1 is not None and (r1 - (r0 or 0)) > 0
    t_arm = next((ts for ts, o in samples if o.get(k["armed"]) or (o.get(k["arms"]) or 0) > 0),
                 None)
    t_rel = next((ts for ts, o in samples if t_arm is not None and ts >= t_arm
                  and o.get(k["rel"]) is not None), None)
    t_fr = next((ts for ts, o in samples if o.get(k["fr"])), None)
    last = samples[-1][1]
    return {"etkin": act, "raise_dosya": (r1 - (r0 or 0)) if r1 is not None else None,
            "kurulma_rel_s": (t_arm - t_file) / 1000.0 if (t_arm and t_file) else None,
            "kalkis_rel_s": (t_rel - t_file) / 1000.0 if (t_rel and t_file) else None,
            "kalkis_neden": last.get(k["rel"]), "kurulma": last.get(k["arms"]),
            "isinmada_tam_tur": (t_fr < t_file) if (t_fr is not None and t_file) else False,
            "ilk_kurulmada_tur": last.get(k["rda"])}


def a_run(rj, items, alice, realized):
    a = rj.get("alice_summary") or {}
    prm = rj.get("params") or {}
    hdr = next((ln for ln in alice if ln.get("k") == "hdr"), {}) or {}
    y3, qn = hdr.get("yol3") or {}, hdr.get("quinn")
    kind = "quinn" if qn else "kama"
    goster = bool(y3.get("goster"))
    file_ok, ptt_ok, why = completion(rj, items)
    fms = (rj.get("file") or {}).get("bob_received_ms")
    fa = a.get("file") or {}
    ser = sender_series(alice, kind)
    tel, retx = wire_window(ser, fa.get("sent"), fa.get("delivered"))
    fb = prm.get("file_bytes")
    ilk, p95, ses, undeliv = ptt_cols(items, a.get("ended"), kind, goster)
    e1set = y3.get("e1_set") or ""
    return {
        "rep": rj.get("rep"), "rc": rj.get("rc"), "tur": kind, "goster": goster,
        "alindi": bool(fa.get("delivered")),
        "e1_arm": y3.get("e1_arm"), "e1_set": e1set or None, "e2a_set": y3.get("e2a_set"),
        "q_kol": (qn or {}).get("arm"), "dosya_ok": file_ok, "ptt_ok": ptt_ok, "neden": why,
        "dosya_s": fms / 1000.0 if fms is not None else INF, "file_bytes": fb,
        "tel_Bps": tel, "retx": retx,
        "t10_s": t_frac(ser, fa.get("sent"), 0.1 * fb) if fb else None,
        "ilk_demet_s": ilk, "varis_p95_s": p95, "ilk_ses_s": ses, "demet_eksik": undeliv,
        "netem_ileri_dusus": realized,
        "a6": (a6_evidence(alice, fa.get("sent"), fa.get("delivered") or a.get("ended"))
               if "a6_iw_floor" in e1set else None),
    }


# ── Hücre × kol toplamı ──────────────────────────────────────────────────────────────────────

def agg_a(runs, limit):
    runs = sorted(runs, key=lambda r: r["rep"] or 0)
    n = len(runs)
    fs = [r["dosya_s"] for r in runs]
    fin = [x for x in fs if x != INF]
    fb = next((r["file_bytes"] for r in runs if r["file_bytes"]), None)
    m_file = med(fs)
    eks = [f"rep{r['rep']} ({', '.join(r['neden'])})" for r in runs
           if r["dosya_ok"] is False or not r["ptt_ok"]]
    dosyali = any(r["dosya_ok"] is not None for r in runs)
    g = {r["goster"] for r in runs}
    a6 = [r["a6"] for r in runs if r["a6"] is not None]
    out = {
        "n": n, "rc0": sum(1 for r in runs if r["rc"] == 0),
        "dosya_ok": sum(1 for r in runs if r["dosya_ok"]) if dosyali else None,
        "alindi": sum(1 for r in runs if r["alindi"]) if dosyali else None,
        "ptt_ok": sum(1 for r in runs if r["ptt_ok"]), "eksik": eks,
        "tur": runs[0]["tur"] if runs else None,
        "goster": (g.pop() if len(g) == 1 else "karışık") if g else None,
        "e1_arm": runs[0]["e1_arm"] if runs else None, "q_kol": runs[0]["q_kol"] if runs else None,
        "limit": limit,
        "dosya_med": m_file, "dosya_p90": rank(fs, .9), "dosya_max": max(fs) if fs else None,
        "faydali_Bps": (fb / m_file) if (fb and m_file and m_file != INF) else None,
        "tel_med": med([r["tel_Bps"] for r in runs]),
        "retx_med": med([r["retx"] for r in runs]),
        "tel_n": sum(1 for r in runs if r["tel_Bps"] is not None),
        "t10_med": med([r["t10_s"] for r in runs]),
        "ilk_med": med([r["ilk_demet_s"] for r in runs]),
        "ilk_max": max((r["ilk_demet_s"] for r in runs if r["ilk_demet_s"] is not None),
                       default=None),
        "p95_med": med([r["varis_p95_s"] for r in runs]),
        "p95_max": max((r["varis_p95_s"] for r in runs if r["varis_p95_s"] is not None),
                       default=None),
        "ses_med": med([r["ilk_ses_s"] for r in runs]),
        "ses_max": max((r["ilk_ses_s"] for r in runs if r["ilk_ses_s"] is not None),
                       default=None),
        "demet_eksik": sum(r["demet_eksik"] for r in runs),
        "dusus_med": med([r["netem_ileri_dusus"] for r in runs]),
        "dosya_bitmeyen": n - len(fin),
        "a6_kol": any("a6_iw_floor" in (r["e1_set"] or "") for r in runs),
        "a6_aynada": len(a6),
        "a6_etkin": sum(1 for x in a6 if x["etkin"]),
        "a6_kurulma_med": med([x["kurulma_rel_s"] for x in a6 if x["etkin"]]),
        "a6_kalkis_neden": sorted({str(x["kalkis_neden"]) for x in a6 if x["etkin"]}),
        "a6_isinmada_tur": sum(1 for x in a6 if x["isinmada_tam_tur"]),
        "a6_olculmedi": [f"rep{r['rep']}" for r in runs
                         if "a6_iw_floor" in (r["e1_set"] or "")
                         and not (r["a6"] and r["a6"]["etkin"])],
    }
    return out


def cell_limit(cell_dir):
    try:
        txt = (cell_dir / "realized_fwd.txt").read_text(encoding="utf-8")
    except OSError:
        return None
    m = LIMIT_RE.search(txt)
    return int(m.group(1)) if m else None


def extract_a(roots, work):
    adir = work / "A"
    subprocess.run([sys.executable, "-I", str(F0 / "a" / "f0a_analyze.py"), "--out", str(adir)]
                   + [str(r) for r in roots], check=True, stdout=subprocess.DEVNULL)
    runs, dirs = {}, {}
    for p in sorted((adir / "runs").glob("*.json")):
        rj = json.loads(p.read_text(encoding="utf-8"))
        if "__" not in rj["cell"]:
            continue
        base = rj["cell"].split("__", 1)[0]
        ip = str(p).replace(os.sep + "runs" + os.sep, os.sep + "items" + os.sep)
        items = read_jsonl(ip[: -len(".json")] + ".jsonl")
        alice = read_jsonl(os.path.join(rj["dir"], "alice.jsonl"))
        lf, _ = f0a_tam.realized_loss(rj["dir"])
        runs.setdefault((base, rj["arm"]), []).append(a_run(rj, items, alice, lf))
        dirs[(base, rj["arm"])] = pathlib.Path(rj["dir"]).parents[1]
    return {k: agg_a(v, cell_limit(dirs[k])) for k, v in runs.items()}


def extract_b(roots, arms=None):
    """b hücreleri: {(taban, kol): {p95, teslim, tx, n, rc0, tamam, msgs}} (analiz_b; oku.py)."""
    import analiz_b  # noqa: E402
    import fizik  # noqa: E402
    geoms = fizik.by_cell()
    out = {}
    for root in roots:
        for cd, base, arm in oku.cells_of(pathlib.Path(root), "b_"):
            if arms is not None and arm not in arms:
                continue
            if base not in geoms:
                continue
            e = {"p95": [], "teslim": [], "tx": [], "rc": [], "tamam": [], "msgs": None}
            for rep in sorted((cd / arm).glob("rep*"), key=lambda q: int(q.name[3:])):
                r = analiz_b.analyze_run(rep, geoms[base], arm)
                n = r.get("n_msgs")
                e["msgs"] = e["msgs"] or n
                e["p95"].append(r.get("conf_p95_s"))
                e["teslim"].append(r["deliv_D"] * n if r.get("deliv_D") is not None and n
                                   else None)
                e["tx"].append(r.get("tx_total_B"))
                e["rc"].append(r.get("rc"))
                e["tamam"].append(r.get("deliv_D") == 1.0)
            out[(base, arm)] = e
    return out


def r3_rows(b, taban, taban_kok):
    """Kol başına R3: oku.py `r3` kuralı AYNEN (K2 yuvasına kol konur); taban yoksa yalnız değer."""
    rows = []
    for (base, arm), e in sorted(b.items(), key=lambda kv: (kv[0][0], order_key(kv[0][1]))):
        row = {"hucre": base, "kol": arm, "n": len(e["rc"]),
               "rc0": sum(1 for x in e["rc"] if x == 0), "tamam": sum(1 for x in e["tamam"] if x),
               "msgs": e["msgs"], "p95_med": med(e["p95"]), "teslim_med": med(e["teslim"]),
               "tx_med": med(e["tx"]), "l0": None, "r25": None, "taban": None}
        k0 = taban.get((base, "K0"))
        if k0 is not None and base in (oku.L0, oku.R25):
            d = {oku.L0: {}, oku.R25: {}}
            d[base] = {"K0": {"p95": k0["p95"], "teslim": k0["teslim"], "tx": k0["tx"]},
                       "K2": {"p95": e["p95"], "teslim": e["teslim"], "tx": e["tx"]}}
            v = oku.r3(d)
            row["l0" if base == oku.L0 else "r25"] = v["l0" if base == oku.L0 else "r25"]
            row["taban"] = {"kok": taban_kok, "p95_med": med(k0["p95"]),
                            "teslim_med": med(k0["teslim"]), "tx_med": med(k0["tx"])}
        rows.append(row)
    return rows


# ── Rapor ────────────────────────────────────────────────────────────────────────────────────

def order_key(arm):
    return (ORDER.index(arm), arm) if arm in ORDER else (len(ORDER), arm)


def frac(x, n):
    return "— (dosyasız)" if x is None else f"{x}/{n}"


def kol_etiketi(arm, e):
    bits = []
    if e.get("tur") == "quinn":
        bits.append(f"quinn {e.get('q_kol')}")
    else:
        bits.append(f"--e1-arm {e.get('e1_arm')}")
        bits.append("şeritli" if e.get("goster") is True else
                    ("şeritsiz" if e.get("goster") is False else str(e.get("goster"))))
    return f"{arm} ({', '.join(bits)})"


def report(A, R3, roots):
    keys = sorted(A, key=lambda k: (k[0], order_key(k[1])))
    L = ["# A-6 gece ölçümü okuması (betimsel; eşik yok)", "",
         "kökler: " + ", ".join(pathlib.Path(r).name for r in roots) + " · etiket: gösterim; "
         "kapılı PoC ölçümü Aralık; ARQ yolu, FEC Aralık'ta · N < 30: betimsel · tanımlar "
         "scripts/yol3/tesis/oku_a6.py belgesinde", "",
         "## 1. Tamamlanma (rc = 0 ≠ tamamlandı)", "",
         "dosya teslim = bob dosyayı tam ve doğru aldı; alındı = alice `file.delivered` (tel "
         "debisi penceresinin ucu; teslimden az olabilir).", "",
         "| hücre | kol | netem limit | n | rc = 0 | dosya teslim | alındı | PTT kapandı | "
         "eksik koşular |",
         "|---|---|---|---|---|---|---|---|---|"]
    for k in keys:
        e = A[k]
        L.append(f"| {k[0]} | {kol_etiketi(k[1], e)} | {e['limit'] or 1000} | {e['n']} | "
                 f"{e['rc0']}/{e['n']} | {frac(e['dosya_ok'], e['n'])} | "
                 f"{frac(e['alindi'], e['n'])} | "
                 f"{e['ptt_ok']}/{e['n']} | "
                 f"{'; '.join(e['eksik']) or '—'} |")
    L += ["", "## 2. Dosya", "",
          "Tel debisi ve retx payı: pencere alice file.sent → file.delivered (ayristirma_util "
          "mantığı); quinn retx = kayıp ilan edilen bayt / UDP gönderilen bayt (vekil). t(%10): "
          "gönderici txb (yeniden gönderim dahil), alıcı ilerlemesi değil; çözünürlük 200 ms.", "",
          "| hücre | kol | dosya bitiş med / p90 / en kötü (s) | faydalı debi (kB/s) | "
          "tel debisi med (B/s) [n] | retx payı med | t(%10) med (s) | ileri netem düşüş payı med |",
          "|---|---|---|---|---|---|---|---|"]
    for k in keys:
        e = A[k]
        fd = e["faydali_Bps"] / 1000.0 if e["faydali_Bps"] else None
        L.append(f"| {k[0]} | {k[1]} | {fmt(e['dosya_med'], 1)} / {fmt(e['dosya_p90'], 1)} / "
                 f"{fmt(e['dosya_max'], 1)} | {fmt(fd, 2)} | {fmt(e['tel_med'], 0)} "
                 f"[{e['tel_n']}] | {fmt(e['retx_med'], 3)} | {fmt(e['t10_med'], 1)} | "
                 f"{fmt(e['dusus_med'], 3)} |")
    L += ["", "## 3. PTT — üç ayrı ölçü (EK-A §10.4)", "",
          "Anlar alice demet gönderiminden; karşı örnek basışı hariç. Varış: Kama = bob Transport → "
          "Application (oynatma tamponu öncesi); quinn = bob stream okuma (tampon yok). İlk "
          "duyulabilir ses = s-kollarında ilk demetin bob PttBundle olayı (oynatma tamponu "
          "sonrası); şeritsiz ve quinn kollarında —.", "",
          "| hücre | kol | ilk demet varışı med / en kötü (s) | bütün demetler varış p95 med / en "
          "kötü (s) | ilk duyulabilir ses med / en kötü (s) | teslim edilmeyen demet (toplam) |",
          "|---|---|---|---|---|---|"]
    for k in keys:
        e = A[k]
        L.append(f"| {k[0]} | {k[1]} | {fmt(e['ilk_med'])} / {fmt(e['ilk_max'])} | "
                 f"{fmt(e['p95_med'])} / {fmt(e['p95_max'])} | {fmt(e['ses_med'])} / "
                 f"{fmt(e['ses_max'])} | {e['demet_eksik']} |")
    a6 = [k for k in keys if A[k]["a6_kol"]]
    L += ["", "## 4. A-6 deney kaydı (EK-A §10.2)", ""]
    if not a6:
        L.append("a6_iw_floor taşıyan kol yok.")
    elif not any(A[k]["a6_aynada"] for k in a6):
        L.append("A-6 kollarında `a6` aynası yok → bütün koşular A-6 ölçülmedi: "
                 + ", ".join(f"{k[0]}/{k[1]}" for k in a6))
    else:
        L += ["Etkinleşti = dosya penceresinde tabanın hızı yükselttiği karar sayısı arttı; "
              "etkinleşmeyen koşu A-6 ölçülmedi sayılır. Anlar file.sent'e göre (200 ms örnek).", "",
              "| hücre | kol | aynada | etkinleşti | etkinleşme anı med (s) | kalkış nedeni | "
              "ısınmada tam tur (dosyadan önce) | A-6 ölçülmedi |", "|---|---|---|---|---|---|---|---|"]
        for k in a6:
            e = A[k]
            L.append(f"| {k[0]} | {k[1]} | {e['a6_aynada']}/{e['n']} | {e['a6_etkin']}/{e['n']} | "
                     f"{fmt(e['a6_kurulma_med'])} | {', '.join(e['a6_kalkis_neden']) or '—'} | "
                     f"{e['a6_isinmada_tur']}/{e['a6_aynada']} | "
                     f"{', '.join(e['a6_olculmedi']) or '—'} |")
    L += ["", "## 5. b hücreleri — R3 (oku.py tanımı aynen)", ""]
    if not R3:
        L.append("b hücresi yok.")
    else:
        L += ["R3 ölçüleri: d1000 l0 hücresinde teyit p95; r25 hücresinde teslim@60 ve gönderim "
              "(D = 60 s iki hücrede). Diğer sütunlar betimsel.", "",
              "| hücre | kol | n | rc = 0 | bütün mesajlar D içinde | teyit p95 med (s) | "
              "teslim@D med (adet / mesaj) | gönderim med (B) | R3 (taban K0) |",
              "|---|---|---|---|---|---|---|---|---|"]
        for r in R3:
            t = r["taban"]
            if t is None:
                v = "— (taban yok)"
            elif r["hucre"] == oku.L0:
                v = f"{r['l0']} (K0 p95 {fmt(t['p95_med'])}; {t['kok']}, karma sürüm)"
            else:
                v = (f"{r['r25']} (K0 teslim {fmt(t['teslim_med'], 1)}, gönderim "
                     f"{fmt(t['tx_med'], 0)}; {t['kok']}, karma sürüm)")
            L.append(f"| {r['hucre']} | {r['kol']} | {r['n']} | {r['rc0']}/{r['n']} | "
                     f"{r['tamam']}/{r['n']} | {fmt(r['p95_med'])} | {fmt(r['teslim_med'], 1)} / "
                     f"{r['msgs'] or '—'} | {fmt(r['tx_med'], 0)} | {v} |")
    js = {"A": {f"{c}/{a}": v for (c, a), v in A.items()}, "R3": R3,
          "kokler": [str(pathlib.Path(r).name) for r in roots], "A6_KEYS": A6_KEYS}
    return "\n".join(L) + "\n", js


# ── Öz-test (sahte veri) ─────────────────────────────────────────────────────────────────────

def oz_test():
    n = [0]

    def chk(name, cond):
        n[0] += 1
        print(("  geçti: " if cond else "  KALDI: ") + name)
        if not cond:
            raise SystemExit(f"öz-test kaldı: {name}")

    ts = lambda t, txb, rtxb: {"k": "ts", "t": t, "s": [{"peer": "carol", "txb": 9e9},
                                                         {"peer": "bob", "txb": txb, "rtxb": rtxb}]}
    alice = [ts(t, tx, r) for t, tx, r in ((0, 0, 0), (1000, 1000, 0), (2000, 3000, 200),
                                            (3000, 6000, 500), (4000, 9000, 900))]
    ser = sender_series(alice, "kama")
    chk("seri: eş bob oturumu, carol yok sayılır", ser[2] == (2000, 3000, 200))
    tel, retx = wire_window(ser, 1500, 3500)
    chk("pencere: uçtan önceki son örnek (1000 → 3000): 5000 B / 2 s, retx 500/5000",
        tel == 2500.0 and abs(retx - 0.1) < 1e-12)
    chk("pencere: alındı yok → (None, None)", wire_window(ser, 1500, None) == (None, None))
    chk("t(%10): 1500'den (taban 1000 B) +4000 B ilk 3000 ms örneği → 1,5 s",
        t_frac(ser, 1500, 4000) == 1.5)
    chk("t(%10): ulaşılmadı → ∞", t_frac(ser, 1500, 1e9) == INF)
    q = [{"k": "qs", "t": 0, "tx_b": 100, "lost_b": 0}, {"k": "qs", "t": 2000, "tx_b": 4100,
                                                         "lost_b": 400}]
    chk("quinn: qs tx_b / lost_b vekili", wire_window(sender_series(q, "quinn"), 0, 2000)
        == (2000.0, 0.1))
    its = [{"c": "P", "sp": 0, "i": 0, "t_send": 0, "t_released": 900, "t_rx": 2900,
            "total": 2900},
           {"c": "P", "sp": 0, "i": 1, "t_send": 200, "t_released": 1300, "t_rx": 3300,
            "total": 3100},
           {"c": "P", "sp": 1, "i": 0, "t_send": 1000, "t_released": 2100, "t_rx": 4100,
            "total": 3100},
           {"c": "P", "sp": 1, "i": 1, "t_send": 1200, "t_released": None, "t_rx": None,
            "total": None},
           {"c": "KP", "sp": 90, "i": 0, "t_send": 0, "t_released": 9e6, "t_rx": 9e6,
            "total": 9e6}]
    ilk, p95, ses, und = ptt_cols(its, 10_000, "kama", True)
    chk("PTT ilk demet varışı: basışların en geci 1,1 s; KP hariç", ilk == 1.1)
    chk("PTT bütün demetler p95: teslim edilmeyen sansürlü (10 000 − 1200 = 8,8 s)", p95 == 8.8)
    chk("PTT ilk duyulabilir ses (s-kolu): en geç ilk demet oynatması 3,1 s", ses == 3.1)
    chk("PTT teslim edilmeyen demet 1", und == 1)
    chk("PTT şeritsiz Kama ve quinn: ilk duyulabilir ses yok",
        ptt_cols(its, 10_000, "kama", False)[2] is None
        and ptt_cols(its, 10_000, "quinn", True)[2] is None)
    base = {"params": {"ptt_at_ms": [15000, 60000]}, "rc": 0,
            "alice_summary": {"result": "complete"},
            "bob_summary": {"file": {"status": "received", "match": True}}}
    cl = [{"c": "C", "sp": 0, "t_rx": 1}, {"c": "C", "sp": 1, "t_rx": 2},
          {"c": "C", "sp": 90, "t_rx": None}]
    chk("tamamlanma: iki kapanış, karşı örnek kapanışı sayılmaz", completion(base, cl)[:2]
        == (True, True))
    bad = dict(base, rc=124, bob_summary={"file": {"status": "partial", "match": False}})
    f_ok, p_ok, why = completion(bad, cl[:1])
    chk("tamamlanma: dosya kısmi + PTT 1/2 + rc nedeni", not f_ok and not p_ok
        and why[:3] == ["dosya partial", "PTT kapanmadı (1/2)", "rc=124"])
    a6 = lambda t, **o: {"k": "ts", "t": t, "s": [{"peer": "bob", "txb": 0, "a6": o}]}
    al = [a6(0, on=True, arms=0), a6(1000, on=True, arms=1, armed=True, fr=False),
          a6(6000, on=True, arms=1, armed=True, fr=False),
          a6(8000, on=True, arms=1, armed=False, rel="FullRound", fr=True)]
    for ln in al:
        o = ln["s"][0]["a6"]
        o["raise"] = {0: 0, 1000: 2, 6000: 9, 8000: 14}[ln["t"]]
    ev = a6_evidence(al, 5000, 9000)
    chk("A-6: dosya penceresinde yükseltme 2 → 14 → etkin; kurulma −4 s; kalkış FullRound 3 s",
        ev["etkin"] and ev["raise_dosya"] == 12 and ev["kurulma_rel_s"] == -4.0
        and ev["kalkis_rel_s"] == 3.0 and ev["kalkis_neden"] == "FullRound"
        and ev["isinmada_tam_tur"] is False)
    chk("A-6: ayna yoksa None (ölçülmedi)", a6_evidence([ts(0, 0, 0)], 0, 1) is None)
    ev2 = a6_evidence(al, 9000, 9500)
    chk("A-6: pencerede yükseltme yok → etkin değil; tam tur dosyadan önce",
        not ev2["etkin"] and ev2["isinmada_tam_tur"] is True)
    with tempfile.TemporaryDirectory() as t:
        p = pathlib.Path(t)
        (p / "realized_fwd.txt").write_text("qdisc netem 8001: root refcnt 2 limit 6 delay 150ms "
                                            "loss 20% rate 100Kbit\n", encoding="utf-8")
        chk("netem limit realized_fwd'dan", cell_limit(p) == 6)
    b = {(oku.L0, "K1"): {"p95": [4.0] * 10, "teslim": [100] * 10, "tx": [1] * 10,
                          "rc": [0] * 10, "tamam": [True] * 10, "msgs": 100},
         (oku.R25, "K1"): {"p95": [9.0] * 10, "teslim": [5] * 10, "tx": [1100] * 10,
                           "rc": [0] * 10, "tamam": [False] * 10, "msgs": 6}}
    tb = {(oku.L0, "K0"): {"p95": [3.8] * 10, "teslim": [100] * 10, "tx": [1] * 10},
          (oku.R25, "K0"): {"p95": [9.0] * 10, "teslim": [5] * 10, "tx": [1000] * 10}}
    rr = {r["hucre"]: r for r in r3_rows(b, tb, "g2")}
    chk("R3: l0 4,0 ≤ 3,8 × 1,10 → geçer", rr[oku.L0]["l0"] is True)
    chk("R3: r25 gönderim 1100 ≤ 1000 × 1,10 sınırda geçer", rr[oku.R25]["r25"] is True)
    rr = {r["hucre"]: r for r in r3_rows(b, {}, None)}
    chk("R3: taban yoksa yalnız değer", rr[oku.L0]["l0"] is None and rr[oku.L0]["taban"] is None)
    print(f"== öz-test: {n[0]}/{n[0]} geçti")
    return 0


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--kok", action="append")
    ap.add_argument("--out")
    ap.add_argument("--r3-taban")
    ap.add_argument("--oz-test", action="store_true")
    a = ap.parse_args()
    if a.oz_test:
        return oz_test()
    if not a.kok:
        ap.error("--kok zorunlu")
    roots = [pathlib.Path(k).expanduser().resolve() for k in a.kok]
    out = pathlib.Path(a.out).expanduser() if a.out else roots[0] / "okuma-a6"
    out.mkdir(parents=True, exist_ok=True)
    A = extract_a(roots, out)
    taban, tkok = {}, None
    if a.r3_taban:
        tk = pathlib.Path(a.r3_taban).expanduser().resolve()
        taban, tkok = extract_b([tk], arms={"K0"}), tk.name
    R3 = r3_rows(extract_b(roots), taban, tkok)
    md, js = report(A, R3, roots)
    (out / "OKUMA-A6.md").write_text(md, encoding="utf-8")
    (out / "okuma-a6.json").write_text(json.dumps(js, ensure_ascii=False, indent=1, default=str),
                                       encoding="utf-8")
    print(md, end="")
    return 0


if __name__ == "__main__":
    sys.exit(main())
