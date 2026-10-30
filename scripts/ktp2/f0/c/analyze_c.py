#!/usr/bin/env python3
"""[KTP2-F0 · Ajan C] PTT / FEC analizi — f0_grid.sh ham çıktısından hücre × kol özeti.

Girdi  : bir ya da daha çok sonuç kökü (`<kök>/<HÜCRE>/<kol>/rep<N>/…`, f0_cell.sh düzeni).
         Hücre adı `c_<hız>_<gecikme>_l<kayıp>_b<demet>` (ya da `cx_…`, `…_duman`), kol adı
         `<tür>_<ikili>`; tür ∈ arq/uyar/uyar64/pol/rs2/rs4/tekrar, ikili ∈ taban/aday.
Çıktı  : <çıktı>/kosular.csv (koşu başına), ozet.csv (hücre × kol dağılımı), OZET-C.md.
Kullanım:
  python3 -I scripts/ktp2/f0/c/analyze_c.py --out DİZİN KÖK [KÖK …]
  python3 -I scripts/ktp2/f0/c/analyze_c.py --duman KÖK      (yalnız bütünlük denetimi, tablo)

KARAR ÖLÇÜSÜ (RAPOR-C §4): son tarihe yetişen ses çerçevesi oranı ve tel baytı.
  Çerçeve f'nin ağızdan kulağa gecikmesi  m2e_f = T_demet + (alıcı varışı − sunum anı).
  Tanım-1 (K-6):           m2e_f ≤ D,           D ∈ {400, 800} ms.
  Tanım-2 (tek yön+bütçe): m2e_f ≤ OWD_nom + b, b ∈ {400, 800} ms.  İKİ TANIM KARIŞTIRILMAZ.
  Alıcı varışı = sunucu `--rx-log` damgası (CLOCK_REALTIME; İSTEK-C1). Yoksa VEKİL: gönderenin
  ACK gözlem anı − OWD_nom (etiket `vekil`; hüküm üretmez). Gönderen teyidi ayrıca raporlanır.
  Tekrar kolu: çerçeve f = birimler (2f, 2f+1); varış = ikisinin en erkeni; sunum = ilk kopya.
  Sıralı teslim (türetilmiş): client-core oturum-geneli sıralamasının (transport.rs `Reorder`)
  karşılığı: rel_u = max(varış_v, v ≤ u); çerçeve sıralı m2e'si rel'den hesaplanır.
Gönderen birikimi (duman-1 bulgusu): pencere-minimum gecikmesinin konuşma boyunca artışı
(`birikim_ms`), motorun son btlbw'si / sunulan UDP hızı (`btlbw_sunulan`), ilk iletimlerin pencerede
tele çıkan payı (`ilk_iletim_payi`), pacer aç kalma payı (`jeton_ac_payi`, ts.csv).
İstatistik: birim = koşu. Hücre × kol başına medyan, IQR, min–maks; kola karşı arq farkı
medyan farkı + tohumlu bootstrap %95 aralığı. N < 30 → `betimsel`.
"""

import argparse
import csv
import json
import math
import os
import random
import re
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import model_c as M  # noqa: E402

CELL_RE = re.compile(r"^(c|cx)_(r1k|r12k)_(d300|d1000)_l(\d+)_b(\d+)(_duman)?$")
ARM_RE = re.compile(r"^(arq|uyar|uyar64|pol|rs2|rs4|tekrar)_(taban|aday)$")
FEC_K = {"rs2": 2, "rs4": 4, "uyar": 8, "uyar64": 4, "pol": 8}
METRICS = ["ot_T1_400", "ot_T1_800", "ot_T2_400", "ot_T2_800", "ot_sirali_T1_800",
           "ot_sirali_T2_800", "ack_T1_800", "teslim_orani", "B_cerceve_udp", "B_cerceve_l2",
           "rho_konusma", "zamaninda_per_kB", "parite_tx", "fec_kurtarilan", "parite_bekleme_ms",
           "retx_B_cerceve", "clamp", "lat_p50_ms", "lat_p95_ms", "btlbw_sunulan", "ilk_iletim_payi",
           "birikim_ms", "birikim_tepe_ms", "birikim_egim_ms_s", "jeton_ac_payi", "kilitli"]


def qtl(vals, x):
    s = sorted(v for v in vals if v is not None and not (isinstance(v, float) and math.isnan(v)))
    if not s:
        return None
    k = (len(s) - 1) * x
    lo, hi = math.floor(k), math.ceil(k)
    return s[lo] + (s[hi] - s[lo]) * (k - lo)


def med(vals):
    return qtl(vals, 0.5)


def fnv(s):
    h = 0xCBF29CE484222325
    for b in s.encode():
        h ^= b
        h = (h * 0x100000001B3) & 0xFFFFFFFFFFFFFFFF
    return h


def boot_diff(a, b, seed, n=2000):
    a = [v for v in a if v is not None]
    b = [v for v in b if v is not None]
    if len(a) < 3 or len(b) < 3:
        return None, None, None
    rng = random.Random(seed)
    d0 = med(a) - med(b)
    ds = []
    for _ in range(n):
        ra = [a[rng.randrange(len(a))] for _ in a]
        rb = [b[rng.randrange(len(b))] for _ in b]
        ds.append(med(ra) - med(rb))
    return d0, qtl(ds, 0.025), qtl(ds, 0.975)


# ── dosya okuyucular ───────────────────────────────────────────────────────────────────────────
def read_text(path):
    try:
        with open(path, errors="replace") as f:
            return f.read()
    except OSError:
        return None


def read_out(path):
    txt = read_text(path)
    if txt is None:
        return None, None
    meta, ev = None, []
    for line in txt.splitlines():
        line = line.strip()
        if not line:
            continue
        o = json.loads(line)
        if "meta" in o:
            meta = o["meta"]
        elif "event" in o:
            ev.append(o["event"])
    return meta, ev


def read_rx(path):
    """İSTEK-C1 şeması: 1. satır {"rx_log_schema":…,"dropped":n}; sonra {"u":id,"t":unix_ns,"n":len}.
    Esnek: "unit_id"/"rx_unix_ns" adları da kabul edilir."""
    txt = read_text(path)
    if txt is None:
        return None, None
    rx, info = {}, {"dropped": 0, "lines": 0}
    for line in txt.splitlines():
        line = line.strip()
        if not line:
            continue
        o = json.loads(line)
        if "rx_log_schema" in o:
            info.update(o)
            continue
        u = o.get("u", o.get("unit_id"))
        t = o.get("t", o.get("rx_unix_ns"))
        if u is None or t is None:
            continue
        info["lines"] += 1
        if u not in rx or t < rx[u]:
            rx[u] = t
    return rx, info


def parse_cmd(path):
    txt = read_text(path) or ""
    cli = ""
    for line in txt.splitlines():
        if line.startswith("CLI:"):
            cli = line[4:]
    flags = {}
    for k in ("--msgs", "--submit-interval-ms", "--warmup-msgs", "--fec-profile", "--fec-mode",
              "--fec-r", "--payload", "--fec-deadline-ms", "--dup-copies"):
        m = re.search(re.escape(k) + r"\s+(\S+)", cli)
        if m:
            flags[k] = m.group(1)
    return flags


def _int(m, i=1):
    return int(m.group(i)) if m else None


def parse_client_err(path):
    txt = read_text(path) or ""
    d = {"kp_client": "KP-CLIENT" in txt}
    g = lambda pat: re.search(pat, txt)  # noqa: E731
    d["fec_blocks"] = _int(g(r"KP-CLIENT .*?fec_blocks=(\d+)"))
    d["fec_repairs_tx"] = _int(g(r"KP-CLIENT .*?fec_repairs_tx=(\d+)"))
    d["fec_recovered_ctrl"] = _int(g(r"fec_recovered_ctrl=(\d+)"))
    m = g(r"latch=\((\d+), (\d+), (\d+), (\d+), (\d+)\)")
    d["latch_enq_r"] = _int(m, 4)
    d["latch_emit_r"] = _int(m, 5)
    m = g(r"arb=\[([0-9, ]+)\]")
    if m:
        arb = [int(x) for x in m.group(1).split(",") if x.strip()]
        d["arb_none"] = arb[0] if arb else None
        d["arb_oneri"] = sum(arb[1:]) if len(arb) > 1 else 0
    m = g(r"fec2=\[repair_not_arqed=(\d+) clamp=(\d+) would_zero=(\d+) partial_blocks=(\d+)\]")
    d["clamp"] = _int(m, 2)
    d["would_zero"] = _int(m, 3)
    d["preconv_parity_blocked"] = _int(g(r"preconv_parity_blocked=(\d+)"))
    d["gate_opened"] = _int(g(r"KP-ACTIVATED .*?gate_opened=(\d+)"))
    m = g(r"fired=\[tail_probe=(\d+)@(\d+)ms tail_cancel=(\d+) parity=(\d+) parity_denied=(\d+) retx=(\d+) dup=(\d+)\]")
    d["parity_denied"] = _int(m, 5)
    d["retx_n"] = _int(m, 6)
    m = g(r"est=\[btlbw=(\d+)B/s srtt=(\d+)us rtprop=(\d+)us")
    d["btlbw"] = _int(m, 1)
    d["srtt_ms"] = _int(m, 2) / 1000.0 if m else None
    m = g(r"KP-CLIENT .*?state=(\w+)")
    d["state"] = m.group(1) if m else None
    return d


def parse_server_err(path):
    txt = read_text(path) or ""
    d = {"kp_server": "KP-SERVER" in txt}
    i = txt.find("stats=ServerStats")
    if i >= 0:
        for k, v in re.findall(r"(\w+): (\d+)", txt[i:i + 4000]):
            d.setdefault("s_" + k, int(v))
    return d


def parse_ts(path, repairs_tx, data_udp=None):
    txt = read_text(path)
    if not txt:
        return {}
    lines = txt.splitlines()
    hdr = lines[0].split(",")
    try:
        it, ip, ifr = hdr.index("t_ms"), hdr.index("pending_parity"), hdr.index("final_ratio")
        itk = hdr.index("pacer_tokens")
    except ValueError:
        return {}
    prev_t = None
    area = 0.0
    pmax = 0
    fr = []
    toks = []
    for line in lines[1:]:
        c = line.split(",")
        if len(c) <= max(it, ip, ifr, itk):
            continue
        t, pend, frv = int(c[it]), int(c[ip]), int(c[ifr])
        if prev_t is not None:
            area += pend * (t - prev_t)
        prev_t = t
        pmax = max(pmax, pend)
        fr.append(frv)
        toks.append(int(c[itk]))
    out = {"parite_kuyruk_max": pmax,
           "final_ratio_ort": (sum(fr) / len(fr)) if fr else None,
           "final_ratio_pozitif_pay": (sum(1 for v in fr if v > 0) / len(fr)) if fr else None}
    if data_udp and toks:
        # [DUMAN-1] Pacer aç kalma payı: koşunun son 2/3'ünde jetonun tek DATA paketine yetmediği
        # örneklerin payı. Kırpma (B-C4) yüzünden kilitsiz koşuda da yüksektir (~0,75); kilitli
        # koşuda ≈ 1 — tek başına değil, birikim_ms ile birlikte okunur.
        tail = toks[len(toks) // 3:]
        out["jeton_ac_payi"] = sum(1 for v in tail if v < data_udp) / len(tail)
    out["parite_alan_ms"] = area  # ∫kuyruk dt (örnekli, 50 ms); tam_c.py alt sınırı buradan
    if repairs_tx:
        out["parite_bekleme_ms"] = area / repairs_tx  # Little: ∫kuyruk dt / emisyon
    return out


def parse_realized(pre_p, post_p):
    def load(p):
        txt = read_text(p)
        if not txt:
            return None
        try:
            return json.loads(txt)
        except json.JSONDecodeError:
            return None

    pre, post = load(pre_p), load(post_p)
    if not pre or not post:
        return {}

    def dev(o, name):
        for q in o.get("mid_qdisc", []):
            if q.get("dev") == name and q.get("kind") == "netem":
                return q
        return None

    out = {}
    for name, tag in (("mb0", "ileri"), ("ma0", "donus")):
        a, b = dev(pre, name), dev(post, name)
        if a and b:
            sent = b.get("packets", 0) - a.get("packets", 0)
            drop = b.get("drops", 0) - a.get("drops", 0)
            out[f"gerceklesen_kayip_{tag}"] = drop / (sent + drop) if sent + drop else None
            out[f"netem_B_{tag}"] = b.get("bytes", 0) - a.get("bytes", 0)
    return out


# ── koşu analizi ──────────────────────────────────────────────────────────────────────────────
def analyze_run(cell, cm, arm, am, d):
    _, rate_key, d_key, loss, t_ms = cm.group(1), cm.group(2), cm.group(3), int(cm.group(4)), int(cm.group(5))
    kind, binv = am.group(1), am.group(2)
    rate = M.RATES[rate_key]
    mu, sig = M.DELAYS[d_key]
    row = {"hucre": cell, "kol": arm, "tur": kind, "ikili": binv, "rep": os.path.basename(d)}
    rc_txt = (read_text(os.path.join(d, "rc")) or "").strip()
    row["rc"] = int(rc_txt) if rc_txt.lstrip("-").isdigit() else None
    flags = parse_cmd(os.path.join(d, "cmd.txt"))
    meta, ev = None, None
    try:
        meta, ev = read_out(os.path.join(d, "out.jsonl"))
    except (json.JSONDecodeError, ValueError) as e:
        row["gecersiz"] = f"out.jsonl çözülemedi: {e}"
    row.update(parse_client_err(os.path.join(d, "c.err")))
    row.update(parse_server_err(os.path.join(d, "s.err")))
    row.update(parse_realized(os.path.join(d, "realized_pre.json"), os.path.join(d, "realized_post.json")))
    if meta is None:
        row.setdefault("gecersiz", "out.jsonl yok")
        row["gecerli"] = 0
        return row, None
    # Kopya düzeni: (a) İSTEK-C2 `--dup-copies N` (gerçek tekrar): mesaj f = birimler W + f·N + c,
    # teslim = ilk kopya; (b) eski tekrar vekili (C2 öncesi): çerçeve f = mesajlar (2f, 2f+1);
    # (c) diğer kollar: çerçeve = mesaj = birim.
    dup = int(flags.get("--dup-copies", 1) or 1)
    if dup > 1:
        copies_msg, ucopies = 1, dup
    elif kind == "tekrar":
        copies_msg, ucopies = 2, 1
    else:
        copies_msg, ucopies = 1, 1
    copies = copies_msg * ucopies
    n_msgs = int(meta.get("msg_count", 0))
    exp_msgs = int(flags.get("--msgs", n_msgs))
    W = int(flags.get("--warmup-msgs", 0))
    n_frames = n_msgs // copies_msg
    row["cerceve"] = n_frames
    row["kopya"] = copies
    row["warmup_residue"] = meta.get("warmup_residue")
    if row["rc"] != 0 or n_msgs != exp_msgs or not row.get("kp_server"):
        row["gecersiz"] = (f"rc={row['rc']}" if row["rc"] != 0 else
                           ("msg_count uyuşmuyor" if n_msgs != exp_msgs else "sunucu KP-SERVER satırı yok"))
        row["gecerli"] = 0
        return row, None
    row["gecerli"] = 1
    t0u = int(meta.get("t0_unix_ns", 0))
    off = int(meta.get("events_epoch_offset_ns", 0))
    sent = {int(e["msg_id"]): int(e["sent_ns"]) for e in ev}
    acked = {int(e["msg_id"]): e.get("delivered_ns") for e in ev}
    rx, rxinfo = read_rx(os.path.join(d, "rx.jsonl"))
    row["rx_kaynak"] = "alici" if rx is not None else "vekil"
    row["rx_satir"] = rxinfo["lines"] if rxinfo else None
    row["rx_dusen"] = rxinfo.get("dropped") if rxinfo else None
    mu_ns = int(mu * 1e6)

    def offer_unix(mid):
        return t0u + (sent[mid] - off)

    def frame_msgs(f):
        return [f * copies_msg + c for c in range(copies_msg)]

    def frame_units(f):
        return [W + m * ucopies + c for m in frame_msgs(f) for c in range(ucopies)]

    def ack_abs(mid):  # vekil: gönderen ACK anı − OWD_nom, mutlak (unix ns)
        a = acked.get(mid)
        return offer_unix(mid) + (int(a) - sent[mid]) - mu_ns if a is not None else None

    lat = []  # çerçeve başına alıcı-varış gecikmesi (ms; inf = yok)
    ack_lat = []
    for f in range(n_frames):
        o = offer_unix(frame_msgs(f)[0])
        best = math.inf
        if rx is not None:
            for u in frame_units(f):
                tr = rx.get(u)
                if tr is not None:
                    best = min(best, (tr - o) / 1e6)
        else:
            for mid in frame_msgs(f):
                t = ack_abs(mid)
                if t is not None:
                    best = min(best, (t - o) / 1e6)
        lat.append(best)
        ab = math.inf
        for mid in frame_msgs(f):
            a = acked.get(mid)
            if a is not None:
                ab = min(ab, (int(a) - sent[frame_msgs(f)[0]]) / 1e6)
        ack_lat.append(ab)
    m2e = [t_ms + v for v in lat]
    # Sıralı teslim (oturum geneli tek sıra): birimler sırayla serbest kalır, rel_u = max(varış_v, v ≤ u)
    # (mutlak zaman). Çerçeve, ilk biriminin serbest kaldığı anda oynatılabilir (rel monoton).
    if rx is not None:
        order = [u for f in range(n_frames) for u in frame_units(f)]
        times = [rx.get(u, math.inf) for u in order]
        first_idx = [f * copies for f in range(n_frames)]
    else:
        order = [m for f in range(n_frames) for m in frame_msgs(f)]
        times = [(ack_abs(m) if ack_abs(m) is not None else math.inf) for m in order]
        first_idx = [f * copies_msg for f in range(n_frames)]
    rel = []
    run_max = -math.inf
    for t in times:
        run_max = max(run_max, t)
        rel.append(run_max)
    sirali = []
    for f in range(n_frames):
        o = offer_unix(frame_msgs(f)[0])
        r = rel[first_idx[f]]
        sirali.append(t_ms + ((r - o) / 1e6 if r < math.inf else math.inf))
    for D in M.DEADLINES_T1:
        row[f"ot_T1_{D}"] = sum(1 for v in m2e if v <= D) / n_frames
    for b in M.BUDGETS_T2:
        row[f"ot_T2_{b}"] = sum(1 for v in m2e if v <= mu + b) / n_frames
    for D in M.DEADLINES_T1:
        row[f"ot_sirali_T1_{D}"] = sum(1 for v in sirali if v <= D) / n_frames
    for b in M.BUDGETS_T2:
        row[f"ot_sirali_T2_{b}"] = sum(1 for v in sirali if v <= mu + b) / n_frames
    row["ack_T1_800"] = sum(1 for v in ack_lat if t_ms + v <= 800) / n_frames
    fin = [v for v in m2e if v < math.inf]
    row["teslim_orani"] = len(fin) / n_frames
    row["lat_p50_ms"] = qtl(fin, 0.5)
    row["lat_p95_ms"] = qtl(fin, 0.95)
    # tel
    tx = meta.get("tx", {}) or {}
    bud = meta.get("budget", {}) or {}
    udp = int(meta.get("udp_tx_bytes", 0))
    inj_b, inj_p = int(bud.get("injected_bytes", 0)), int(bud.get("injected_pkts", 0))
    l2b = inj_b + 14 * inj_p  # IP baytı + Ethernet 14 (KATALOG §2)
    row.update({"udp_tx_B": udp, "data_B": tx.get("data"), "retx_B": tx.get("retx"), "dup_B": tx.get("dup"),
                "fec_B": tx.get("fec"), "ctrl_B": tx.get("ctrl"), "inj_ip_B": inj_b, "inj_pkt": inj_p,
                "l2_B": l2b})
    row["B_cerceve_udp"] = udp / n_frames
    row["B_cerceve_l2"] = l2b / n_frames
    row["retx_B_cerceve"] = (tx.get("retx") or 0) / n_frames
    row["rho_konusma"] = (l2b / (n_frames * t_ms / 1000.0)) / rate  # üst sınır vekili (kuyruk dahil)
    row["zamaninda_per_kB"] = row["ot_T1_800"] * n_frames / (l2b / 1000.0) if l2b else None
    row["parite_tx"] = row.get("fec_repairs_tx")
    row["fec_kurtarilan"] = row.get("s_fec_recovered_units")
    # [DUMAN-1 bulgusu] GÖNDEREN BİRİKİMİ teşhisi. Sunum başına UDP boyu (bench: dikiş 16 + DATA 11 +
    # yük) ve sunulan hız; motorun son btlbw'si buna oranla; ilk iletimlerin pencerede tele çıkan payı;
    # pencere-minimum gecikmesinin konuşma boyunca artışı (alıcı damgası varsa ondan, yoksa ham ACK
    # gecikmesinden — sabit ofset artışı etkilemez). Kayıp çerçeve minimumu etkilemez; artış =
    # kaybolmamış çerçevelerin de geç çıkması (gönderen kuyruğu).
    pay = int(flags.get("--payload", 0) or 0)
    d_udp = M.SEAM_BENCH + M.HDR_DATA + pay
    talk_s = n_frames * t_ms / 1000.0
    n_units = n_msgs * ucopies
    row["sunulan_udp_Bps"] = n_units * d_udp / talk_s if talk_s else None
    if row.get("btlbw") is not None and row["sunulan_udp_Bps"]:
        row["btlbw_sunulan"] = row["btlbw"] / row["sunulan_udp_Bps"]
    row["ilk_iletim_payi"] = (tx.get("data") or 0) / (n_units * d_udp) if n_units and d_udp else None
    src = lat if rx is not None else ack_lat
    nwin = 12
    wlen = max(1, n_frames // nwin)
    wmins = []
    for k in range(0, n_frames - wlen + 1, wlen):
        v = [x for x in src[k:k + wlen] if x < math.inf]
        wmins.append(min(v) if v else math.inf)
    fin_w = [(i, v) for i, v in enumerate(wmins) if v < math.inf]
    if len(fin_w) >= 2:
        row["birikim_ms"] = fin_w[-1][1] - fin_w[0][1]
        # [duman-2] tepe: birikim konuşma ortasında yükselip sonda kısmen sönebiliyor (son − ilk
        # bunu kaçırır); ilk pencereye göre en yüksek pencere-minimumu.
        row["birikim_tepe_ms"] = max(v for _, v in fin_w) - fin_w[0][1]
        span = (fin_w[-1][0] - fin_w[0][0]) * wlen * t_ms / 1000.0
        row["birikim_egim_ms_s"] = row["birikim_ms"] / span if span else None
    # Pencerede hiç çerçeve gelmeyen pencereler (gönderen kuyruğu ölçüm sonuna taştı) tepeye
    # sonsuz birikim olarak sayılır.
    if any(v == math.inf for v in wmins[len(wmins) // 2:]):
        row["birikim_tepe_ms"] = math.inf
    row["birikim_bos_pencere"] = sum(1 for v in wmins if v == math.inf)
    # Sınıflama (betimsel eşik, karar değil; eşik duman-2 sonrası konuldu — RAPOR-C §10): tepe
    # birikim ≥ max(500 ms, 4σ_jitter) (d300/T=120/D=800'de son tarih payı ≈ 380 ms; d1000'de
    # pencere-minimumunun jitter yayılımı büyük → 800 ms) YA DA ilk iletimlerin
    # ≥ %5'i pencere içinde tele çıkmamış → gönderen birikimi (B-C9).
    row["kilitli"] = int((row.get("birikim_tepe_ms") or 0) >= max(500.0, 4 * sig)
                         or (row["ilk_iletim_payi"] is not None and row["ilk_iletim_payi"] < 0.95))
    row.update(parse_ts(os.path.join(d, "ts.csv"), row.get("fec_repairs_tx"), d_udp))
    # gecikme bileşenleri (çerçeve düzeyi, hücre toplamı için)
    ser_d = (M.data_udp(t_ms) + M.L2) / rate * 1000.0
    extra = {"lat": lat, "m2e": m2e, "ser_d": ser_d, "mu": mu, "sig": sig, "t_ms": t_ms,
             "copies": copies, "K": FEC_K.get(kind) if kind in ("rs2", "rs4") else None}
    return row, extra


def walk(roots):
    for root in roots:
        for cell in sorted(os.listdir(root)):
            cdir = os.path.join(root, cell)
            cm = CELL_RE.match(cell)
            if not cm or not os.path.isdir(cdir):
                continue
            for arm in sorted(os.listdir(cdir)):
                adir = os.path.join(cdir, arm)
                am = ARM_RE.match(arm)
                if not am or not os.path.isdir(adir):
                    continue
                for rep in sorted(os.listdir(adir), key=lambda s: int(s[3:]) if s[3:].isdigit() else 0):
                    rdir = os.path.join(adir, rep)
                    if rep.startswith("rep") and os.path.isdir(rdir):
                        yield cell, cm, arm, am, rdir


def fmt(v, nd=3):
    if v is None:
        return "—"
    if isinstance(v, float):
        if math.isinf(v) or math.isnan(v):
            return "—"
        return f"{v:.{nd}f}"
    return str(v)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--out")
    ap.add_argument("--duman", action="store_true")
    ap.add_argument("roots", nargs="+")
    a = ap.parse_args()
    rows, extras = [], {}
    for cell, cm, arm, am, rdir in walk(a.roots):
        r, x = analyze_run(cell, cm, arm, am, rdir)
        rows.append(r)
        if x:
            extras.setdefault((cell, arm), []).append(x)
    if not rows:
        print("koşu bulunamadı", file=sys.stderr)
        return 1

    if a.duman:
        print("| hücre | kol | rep | rc | geçerli | rx | rx satır | çerçeve | T1-800 | T2-800 | parite tx | "
              "fec kurt. | clamp | btlbw/sunulan | ilk iletim payı | birikim ms (son/tepe) | jeton aç payı | kilitli | not |")
        print("|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|")
        for r in rows:
            print(f"| {r['hucre']} | {r['kol']} | {r['rep']} | {r.get('rc')} | {r.get('gecerli')} | "
                  f"{r.get('rx_kaynak', '—')} | {fmt(r.get('rx_satir'))} | {fmt(r.get('cerceve'))} | "
                  f"{fmt(r.get('ot_T1_800'))} | {fmt(r.get('ot_T2_800'))} | {fmt(r.get('parite_tx'))} | "
                  f"{fmt(r.get('fec_kurtarilan'))} | {fmt(r.get('clamp'))} | {fmt(r.get('btlbw_sunulan'), 2)} | "
                  f"{fmt(r.get('ilk_iletim_payi'), 2)} | {fmt(r.get('birikim_ms'), 0)}/{fmt(r.get('birikim_tepe_ms'), 0)} | "
                  f"{fmt(r.get('jeton_ac_payi'), 2)} | {fmt(r.get('kilitli'))} | {r.get('gecersiz', '')} |")
        return 0

    os.makedirs(a.out, exist_ok=True)
    keys = []
    for r in rows:
        for k in r:
            if k not in keys:
                keys.append(k)
    with open(os.path.join(a.out, "kosular.csv"), "w", newline="") as f:
        w = csv.DictWriter(f, fieldnames=keys)
        w.writeheader()
        for r in rows:
            w.writerow(r)

    # hücre × kol özeti
    groups = {}
    for r in rows:
        groups.setdefault((r["hucre"], r["kol"]), []).append(r)
    summ = []
    for (cell, arm), rs in sorted(groups.items()):
        ok = [r for r in rs if r.get("gecerli")]
        s = {"hucre": cell, "kol": arm, "N_toplam": len(rs), "N_gecerli": len(ok),
             "N_alici": sum(1 for r in ok if r.get("rx_kaynak") == "alici"),
             "etiket": "hüküm" if len(ok) >= 30 and all(r.get("rx_kaynak") == "alici" for r in ok) else "betimsel"}
        for mname in METRICS:
            vals = [r.get(mname) for r in ok if r.get(mname) is not None]
            s[f"{mname}_med"] = med(vals)
            s[f"{mname}_q1"] = qtl(vals, 0.25)
            s[f"{mname}_q3"] = qtl(vals, 0.75)
            s[f"{mname}_min"] = min(vals) if vals else None
            s[f"{mname}_max"] = max(vals) if vals else None
        s["kilit_n"] = sum(1 for r in ok if r.get("kilitli"))
        # arq'a karşı fark (aynı ikili)
        binv = arm.split("_")[1]
        base = groups.get((cell, f"arq_{binv}"), [])
        base_ok = [r for r in base if r.get("gecerli")]
        for mname in ("ot_T1_800", "ot_T2_800", "ot_T2_400"):
            d0, lo, hi = boot_diff([r.get(mname) for r in ok], [r.get(mname) for r in base_ok],
                                   fnv(f"{cell}|{arm}|{mname}"))
            s[f"{mname}_fark_arq"] = d0
            s[f"{mname}_fark_lo"] = lo
            s[f"{mname}_fark_hi"] = hi
        # gecikme bileşenleri
        xs = extras.get((cell, arm), [])
        if xs:
            x0 = xs[0]
            s["bilesen_demet_ms"] = x0["t_ms"]
            s["bilesen_seri_veri_ms"] = x0["ser_d"]
            s["bilesen_ag_ms"] = x0["mu"]
            if x0["K"]:
                s["bilesen_blok_max_ms"] = (x0["K"] - 1) * x0["t_ms"]
                s["bilesen_blok_ort_ms"] = (x0["K"] - 1) * x0["t_ms"] / 2.0
            # titreşim + kuyruk artığı: doğrudan-sınıf çerçeveler (varış < OWD + 4σ + seri)
            res = []
            for x in xs:
                lim = x["mu"] + 4 * x["sig"] + x["ser_d"]
                res += [v - x["ser_d"] - x["mu"] for v in x["lat"] if v < lim]
            s["bilesen_titresim_p50_ms"] = qtl(res, 0.5)
            s["bilesen_titresim_p95_ms"] = qtl(res, 0.95)
            if x0["K"]:
                K = x0["K"]
                pos_ok = [0] * K
                pos_n = [0] * K
                for x in xs:
                    for f, v in enumerate(x["m2e"]):
                        pos_n[f % K] += 1
                        pos_ok[f % K] += 1 if v <= 800 else 0
                s["blok_konum_T1_800"] = " ".join(f"{pos_ok[i] / pos_n[i]:.3f}" for i in range(K))
        summ.append(s)
    skeys = []
    for s in summ:
        for k in s:
            if k not in skeys:
                skeys.append(k)
    with open(os.path.join(a.out, "ozet.csv"), "w", newline="") as f:
        w = csv.DictWriter(f, fieldnames=skeys)
        w.writeheader()
        for s in summ:
            w.writerow(s)

    with open(os.path.join(a.out, "OZET-C.md"), "w") as f:
        f.write("# Ajan C — ölçüm özeti (ÜRETİLDİ: analyze_c.py)\n\n")
        f.write("Hücre başına: medyan [IQR] (min–maks), birim = koşu. `vekil` = alıcı damgası yok.\n")
        cur = None
        for s in summ:
            if s["hucre"] != cur:
                cur = s["hucre"]
                f.write(f"\n## {cur}\n\n| kol | N (geçerli/alıcı) | etiket | T1-400 | T1-800 | T2-400 | T2-800 | "
                        "sıralı T1-800 | Δ T1-800 vs arq [%95] | B/çerçeve L2 | parite tx | FEC kurt. | "
                        "parite bekleme ms | clamp | birikim ms | btlbw/sunulan | kilitli koşu |\n"
                        "|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|\n")

            def cellv(m):
                return (f"{fmt(s.get(m + '_med'))} [{fmt(s.get(m + '_q1'))}, {fmt(s.get(m + '_q3'))}] "
                        f"({fmt(s.get(m + '_min'))}–{fmt(s.get(m + '_max'))})")

            dlt = (f"{fmt(s.get('ot_T1_800_fark_arq'))} [{fmt(s.get('ot_T1_800_fark_lo'))}, "
                   f"{fmt(s.get('ot_T1_800_fark_hi'))}]")
            f.write(f"| {s['kol']} | {s['N_gecerli']}/{s['N_alici']} ({s['N_toplam']}) | {s['etiket']} | "
                    f"{cellv('ot_T1_400')} | {cellv('ot_T1_800')} | {cellv('ot_T2_400')} | {cellv('ot_T2_800')} | "
                    f"{cellv('ot_sirali_T1_800')} | {dlt} | {fmt(s.get('B_cerceve_l2_med'), 1)} | "
                    f"{fmt(s.get('parite_tx_med'), 0)} | {fmt(s.get('fec_kurtarilan_med'), 0)} | "
                    f"{fmt(s.get('parite_bekleme_ms_med'), 0)} | {fmt(s.get('clamp_med'), 0)} | "
                    f"{cellv('birikim_ms')} | {fmt(s.get('btlbw_sunulan_med'), 2)} | {s.get('kilit_n', 0)}/{s['N_gecerli']} |\n")
        f.write("\n## Gecikme bileşenleri (ms)\n\n| hücre | kol | demet | blok ort/max | seri veri | ağ (OWD) | "
                "titreşim+kuyruk p50/p95 | parite bekleme | konuma göre T1-800 |\n|---|---|---|---|---|---|---|---|---|\n")
        for s in summ:
            if "bilesen_demet_ms" not in s:
                continue
            blk = (f"{fmt(s.get('bilesen_blok_ort_ms'), 0)}/{fmt(s.get('bilesen_blok_max_ms'), 0)}"
                   if s.get("bilesen_blok_max_ms") is not None else "—")
            f.write(f"| {s['hucre']} | {s['kol']} | {s['bilesen_demet_ms']} | {blk} | "
                    f"{fmt(s.get('bilesen_seri_veri_ms'), 1)} | {fmt(s.get('bilesen_ag_ms'), 0)} | "
                    f"{fmt(s.get('bilesen_titresim_p50_ms'), 0)}/{fmt(s.get('bilesen_titresim_p95_ms'), 0)} | "
                    f"{fmt(s.get('parite_bekleme_ms_med'), 0)} | {s.get('blok_konum_T1_800', '—')} |\n")
    print(f"yazıldı: {a.out}/kosular.csv ozet.csv OZET-C.md ({len(rows)} koşu, {len(summ)} hücre×kol)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
