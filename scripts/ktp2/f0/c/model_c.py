#!/usr/bin/env python3
"""[KTP2-F0 · Ajan C] PTT / FEC ön-kayıt modeli. Etiket: [çıkarım] — KOŞU DEĞİL.

Ne yapar: ölçüm başlamadan ÖNCE her hücre × kol için (a) ses tel hızını ve hat doluluğunu,
(b) zamanlamasız çerçeve oynatılabilirliğini (binom) ve (c) son tarihe yetişme oranını
(tohumlu Monte Carlo: binom kayıp + blok zamanlaması + netem FIFO/serileştirme + jitter) hesaplar.
Ölçüm sonrası sapma bu tabloya karşı dekompoze edilir (beklenti-kaydı disiplini).

Model varsayımları (RAPOR-C §5'te aynen yazılı; ölçümle sınanır):
  M1  Kayıp iki yönde bağımsız Bernoulli(p) (netem `loss p%`, tohumsuz).
  M2  netem ileri kuyruğu: varış_i = max(varış_{i-1}, gönderim_i + d_i) + boy_i/hız
      (rate varken netem sırayı korur; kayıp paket kuyruğa girmez). d_i ~ N(μ, σ), alt sınır 0.
  M3  Veri birimi sunum anında gönderilir (ses hızı ≪ pacer hızı). Parite gönderimi üç varyant:
      'ideal' = blok kapanınca hemen; 'pacer1.25' / 'pacer1.00' = ürün pacer'ı (token kovası,
      hız g×A, A = ortalama sunulan UDP hızı; DATA önce, parite en sonda; her yeni DATA'da kova
      max(DATA boyu, hız×40 ms)'ye kırpılır — sender.rs:2366-2381 ve :4292/:4311 sırası).
  M4  ARQ: kaybolan ilk iletim bir kez yeniden gönderilir, tespit anı
      gönderim + max(sonraki birim aralığı + RTT + 20 ms, 9/8·RTT) (loss.rs:14-21, ACK_TICK 20 ms).
      İkinci yeniden gönderim modellenmez (son tarihlerin çok ötesinde).
  M5  FEC kurtarma: bloktaki K'ıncı farklı sembolün varış anında (runtime.rs fec_ingest).
  M6  Kurtarılan birim yine de yeniden gönderilir (karamsar bant; RECOVERED iptali modellenmez).
  M7  Isınma (warmup) sonrası durağan oturum: PreConverge/kuruluş etkisi YOK.

Kullanım:  python3 -I scripts/ktp2/f0/c/model_c.py ÇIKTI_DİZİNİ
Çıktı:     geometri.csv · butce.csv · beklenen.csv · MODEL-C.md (deterministik; tohum sabit)
"""

import csv
import math
import os
import random
import sys

# ── Tel sabitleri (kaynak dosya:satır; taban a858f3fc) ───────────────────────────────────────────
SEAM_BENCH = 16  # client-core/src/seam.rs:105 IDENTITY_TAG (bench güvenlik dikişi)
ENV_LEGACY = 22  # kama-noise-wire/src/legacy.rs:25 LEGACY_OVERHEAD (hub zarfı, taslak §4)
ENV_EXT = 26  # kama-transport-schema/src/lib.rs:41 EXTENDED_OUTER_OVERHEAD
HDR_DATA = 11  # client-core/src/wire.rs:17
HDR_FEC = 12  # client-core/src/wire.rs:105
PREFIX = 11  # kama-transport-schema/src/lib.rs:37 SOURCE_PREFIX_LEN (sembolün İÇİNDE)
AMP = 5 + 8  # kama-app/src/codec/limits.rs:24 kayıt başlığı + :93 MEDIA_BUNDLE öneki
L2 = 42  # KATALOG §2: Ethernet 14 + IP 20 + UDP 8 (netem `rate` bunu sayar)
CODEC_BPS = 1200  # 1,2 kbit/s (RAPOR-C §4.1 gerekçe)
MTU = 1200  # adaptive-transport/src/rate.rs:18
BURST_WINDOW_S = 0.040  # sender.rs:1563

RATES = {"r1k": 1000.0, "r12k": 12500.0}  # B/s, L2 dahil (KATALOG §2)
DELAYS = {"d300": (300.0, 60.0), "d1000": (1000.0, 200.0)}  # tek yön ms (μ, σ)
LOSSES = [10, 20, 30, 50]
BUNDLES = [60, 120, 200]
DEADLINES_T1 = [400, 800]  # ağızdan kulağa (K-6)
BUDGETS_T2 = [400, 800]  # tek yön + bütçe
TALK_S = 20.0  # konuşma süresi (koşu başına)
RHO_MAX = 0.85  # sabit RS'te r seçimi: ileri hat doluluğu (ARQ hariç) bu sınırı aşmaz

# Sabit RS kolları: (profil, K, R_max, E) — kama-fec-codec/src/profile.rs PROFILES.
RS_ARMS = {
    "rs2": {"profile": 1, "K": 2, "rmax": 2, "E": 64},
    "rs4": {"profile": 2, "K": 4, "rmax": 4, "E": 64},
}
# Kayba göre r tablosu. rs4 taslak §3.8 tablosunu izler; (3,2) ve (4,9) katalogda yok → (4,2), (4,4).
R_TABLE = {"rs2": {10: 1, 20: 2, 30: 2, 50: 2}, "rs4": {10: 1, 20: 2, 30: 4, 50: 4}}
# Karşı-olgu: ürün uyarlamalı FEC'i ateşleseydi alacağı en yüksek öneri (gate 250‰ = r2 @ K8, E1024).
UYAR_ATES = {"profile": 5, "K": 8, "r": 2, "E": 1024}

SEED = 20261029
RUNS = 120  # Monte Carlo koşu sayısı (hücre × kol × varyant)


def codec_bytes(t_ms):
    return CODEC_BPS / 8.0 * t_ms / 1000.0


def bench_payload(t_ms):
    """Bench `--payload`: kodlayıcı + AMP 13 B + zarf düzeltmesi (22 − 16 = 6 B)."""
    return int(round(codec_bytes(t_ms))) + AMP + (ENV_LEGACY - SEAM_BENCH)


def data_udp(t_ms):
    return SEAM_BENCH + HDR_DATA + bench_payload(t_ms)


def par_udp(e):
    return SEAM_BENCH + HDR_FEC + e


def wire_rate(t_ms, k=None, r=0, e=64, copies=1, loss=0.0, rate=None):
    """İleri hat L2 hızı (B/s): veri (+ parite) + beklenen ARQ (M6 karamsar). Döner (A_l2, ρ)."""
    d_l2 = data_udp(t_ms) + L2
    per_s = copies * d_l2 / (t_ms / 1000.0)
    if k:
        per_s += r * (par_udp(e) + L2) / (k * t_ms / 1000.0)
    per_s_arq = per_s + loss * copies * d_l2 / (t_ms / 1000.0)
    return per_s, per_s_arq, (per_s / rate if rate else None), (per_s_arq / rate if rate else None)


def r_select(arm, loss, t_ms, rate):
    """Tablodaki r; hat dolmasın diye (ρ ≤ RHO_MAX, ARQ hariç) gerekirse düşürülür; en az 1."""
    a = RS_ARMS[arm]
    r = R_TABLE[arm][loss]
    while r > 1:
        _, _, rho, _ = wire_rate(t_ms, a["K"], r, a["E"], rate=rate)
        if rho <= RHO_MAX:
            break
        r -= 1
    return r


def p_play(k, r, p):
    """Zamanlamasız çerçeve oynatılabilirliği: doğrudan gelir ya da blok çözülür."""
    if not k:
        return 1.0 - p
    n_other = k + r - 1
    need = k  # çerçeve kayıpsa diğer k+r−1 sembolden en az k tanesi gelmeli
    s = 0.0
    for j in range(need, n_other + 1):
        s += math.comb(n_other, j) * (1 - p) ** j * p ** (n_other - j)
    return (1 - p) + p * s


def p_block(k, r, p):
    n = k + r
    return sum(math.comb(n, j) * (1 - p) ** j * p ** (n - j) for j in range(k, n + 1))


def phi(x):
    return 0.5 * (1.0 + math.erf(x / math.sqrt(2.0)))


# ── Monte Carlo ────────────────────────────────────────────────────────────────────────────────
def pacer_schedule(offers, size_d, blocks, size_p, rate_udp):
    """Ürün pacer'ı (M3): DATA önce; parite kuyruğu kalan token'la; DATA gelişinde kova kırpılır.

    offers: [(t_s, frame_idx)] artan; blocks: {frame_idx_of_close: n_parity}.
    Döner: (data_send[t per offer], parity_send[list of (t, block_close_idx, j)]).
    """
    cap_idle = max(MTU, min(rate_udp * BURST_WINDOW_S, 2 * MTU))
    tokens = cap_idle
    t_last = offers[0][0] if offers else 0.0
    data_send = []
    pq = []  # (close_idx, j)
    par_send = []
    for n, (t, idx) in enumerate(offers):
        # Önce bu DATA'ya kadar bekleyen pariteyi idle kovayla boşalt (kova = cap_idle).
        while pq:
            need = max(0.0, size_p - tokens)
            te = t_last + need / rate_udp
            if te < t:
                tokens = min(cap_idle, tokens + rate_udp * (te - t_last)) - size_p
                tokens = max(tokens, 0.0)
                t_last = te
                ci, j = pq.pop(0)
                par_send.append((te, ci, j))
            else:
                break
        tokens = min(cap_idle, tokens + rate_udp * (t - t_last))
        t_last = t
        cap_data = max(size_d, min(rate_udp * BURST_WINDOW_S, max(size_d, 2 * MTU)))
        tokens = min(tokens, cap_data)  # sender.rs:2381 kırpma (AppData olayında ön = bu DATA)
        if tokens >= size_d:
            ts = t
            tokens -= size_d
        else:
            ts = t + (size_d - tokens) / rate_udp
            tokens = 0.0
            t_last = ts
        data_send.append(ts)
        if idx in blocks:
            for j in range(blocks[idx]):
                pq.append((idx, j))
        t_last = max(t_last, ts)
    # kuyruk sonu
    while pq:
        need = max(0.0, size_p - tokens)
        te = t_last + need / rate_udp
        tokens = 0.0
        t_last = te
        ci, j = pq.pop(0)
        par_send.append((te, ci, j))
    return data_send, par_send


def simulate(cell, arm, variant, rng):
    rate_key, d_key, loss, t_ms = cell
    rate = RATES[rate_key]
    mu, sig = DELAYS[d_key]
    mu /= 1000.0
    sig /= 1000.0
    T = t_ms / 1000.0
    p = loss / 100.0
    n_frames = int(round(TALK_S / T))
    sd = data_udp(t_ms)
    copies = 2 if arm == "tekrar" else 1
    gap = T / copies
    if arm in RS_ARMS:
        a = RS_ARMS[arm]
        K, E = a["K"], a["E"]
        r = r_select(arm, loss, t_ms, rate)
    elif arm == "uyar_ates":
        K, E, r = UYAR_ATES["K"], UYAR_ATES["E"], UYAR_ATES["r"]
    else:
        K, E, r = None, None, 0
    n_frames = (n_frames // K) * K if K else n_frames
    sp = par_udp(E) if K else 0
    rtt = 2 * mu
    ratios = {("T1", D): [] for D in DEADLINES_T1}
    ratios.update({("T2", b): [] for b in BUDGETS_T2})
    for _run in range(RUNS):
        # 1) ilk iletimler
        pk = []  # [t_send, size_l2, kind, ref]
        offers = []
        for f in range(n_frames):
            for c in range(copies):
                offers.append((f * T + c * gap, f))
        blocks = {}
        if K:
            for b in range(n_frames // K):
                blocks[b * K + K - 1] = r
        if K and variant.startswith("pacer"):
            g = float(variant[len("pacer"):])
            a_udp = (K * sd + r * sp) / (K * T)
            dsend, psend = pacer_schedule(offers, sd, blocks, sp, g * a_udp)
        else:
            dsend = [t for t, _ in offers]
            psend = []
            for ci, n_par in blocks.items():
                for j in range(n_par):
                    psend.append((ci * T + 1e-6 * (j + 1), ci, j))
        for (t_off, f), ts in zip(offers, dsend):
            pk.append([ts, sd + L2, "d", f, t_off])
        for ts, ci, j in psend:
            pk.append([ts, sp + L2, "p", ci // K, j])
        # 2) kayıp + ARQ (M4)
        lost = [rng.random() < p for _ in pk]
        retx = []
        for q, is_lost in zip(pk, lost):
            if is_lost and q[2] == "d":
                td = q[0] + max(gap + rtt + 0.020, 1.125 * rtt)
                retx.append([td, q[1], "d", q[3], q[4]])
        lost += [rng.random() < p for _ in retx]
        pk += retx
        order = sorted(range(len(pk)), key=lambda i: pk[i][0])
        # 3) netem FIFO (M2)
        prev = -1e9
        arr = [None] * len(pk)
        for i in order:
            if lost[i]:
                continue
            d = max(0.0, rng.gauss(mu, sig))
            a_t = max(prev, pk[i][0] + d) + pk[i][1] / rate
            arr[i] = a_t
            prev = a_t
        # 4) çerçeve varışı
        direct = [math.inf] * n_frames
        for i, q in enumerate(pk):
            if q[2] == "d" and arr[i] is not None:
                direct[q[3]] = min(direct[q[3]], arr[i])
        best = list(direct)
        if K:
            par_arr = {}
            for i, q in enumerate(pk):
                if q[2] == "p" and arr[i] is not None:
                    par_arr.setdefault(q[3], []).append(arr[i])
            for b in range(n_frames // K):
                syms = sorted([direct[b * K + e] for e in range(K) if direct[b * K + e] < math.inf]
                              + par_arr.get(b, []))
                if len(syms) >= K:
                    t_dec = syms[K - 1]
                    for e in range(K):
                        best[b * K + e] = min(best[b * K + e], t_dec)
        for D in DEADLINES_T1:
            ok = sum(1 for f in range(n_frames) if T + (best[f] - f * T) <= D / 1000.0)
            ratios[("T1", D)].append(ok / n_frames)
        for b in BUDGETS_T2:
            lim = mu + b / 1000.0
            ok = sum(1 for f in range(n_frames) if T + (best[f] - f * T) <= lim)
            ratios[("T2", b)].append(ok / n_frames)
    return K, r, E, ratios


def q(vals, x):
    s = sorted(vals)
    if not s:
        return float("nan")
    k = (len(s) - 1) * x
    lo, hi = math.floor(k), math.ceil(k)
    return s[lo] + (s[hi] - s[lo]) * (k - lo)


def main():
    if len(sys.argv) != 2:
        print(__doc__, file=sys.stderr)
        return 2
    out = sys.argv[1]
    os.makedirs(out, exist_ok=True)

    # ── geometri.csv: ses tel hızı ───────────────────────────────────────────────────────────────
    geo_rows = []
    for t in BUNDLES:
        cb = codec_bytes(t)
        pps = 1000.0 / t
        prod_legacy = ENV_LEGACY + HDR_DATA + AMP + cb
        prod_ext = ENV_EXT + HDR_DATA + AMP + cb
        bench = data_udp(t)
        geo_rows.append({
            "demet_ms": t, "kodlayici_B": cb, "paket_s": round(pps, 3),
            "bench_payload_B": bench_payload(t), "bench_udp_B": bench,
            "urun_udp_legacy22_B": prod_legacy, "urun_udp_ext26_B": prod_ext,
            "udp_Bps_legacy22": round(prod_legacy * pps, 1),
            "ip_Bps_legacy22": round((prod_legacy + 28) * pps, 1),
            "l2_Bps_bench": round((bench + L2) * pps, 1),
            "baslik_payi_udp": round(1 - cb / prod_legacy, 3),
            "rho_r1k": round((bench + L2) * pps / RATES["r1k"], 3),
            "rho_r12k": round((bench + L2) * pps / RATES["r12k"], 3),
            "parite_E64_udp_B": par_udp(64), "parite_E1024_udp_B": par_udp(1024),
        })
    with open(os.path.join(out, "geometri.csv"), "w", newline="") as f:
        w = csv.DictWriter(f, fieldnames=list(geo_rows[0].keys()))
        w.writeheader()
        w.writerows(geo_rows)

    # ── butce.csv: K-6 gecikme bütçesi bileşenleri (yapısal) ─────────────────────────────────────
    bud_rows = []
    for t in BUNDLES:
        for d_key, (mu, sig) in DELAYS.items():
            for rate_key, rate in RATES.items():
                ser_d = (data_udp(t) + L2) / rate * 1000.0
                ser_p = (par_udp(64) + L2) / rate * 1000.0
                for kname, k in (("ARQ/tekrar", 1), ("K2", 2), ("K4", 4), ("K8-E1024", 8)):
                    blk_max = (k - 1) * t
                    sp = (par_udp(1024) + L2) / rate * 1000.0 if k == 8 else ser_p
                    base = t + ser_d + mu
                    rows = {"demet_ms": t, "gecikme": d_key, "hiz": rate_key, "kol": kname,
                            "demet": t, "blok_olusturma_max": blk_max,
                            "blok_olusturma_ort": blk_max / 2.0,
                            "seri_veri_ms": round(ser_d, 1), "seri_parite_ms": round(sp, 1),
                            "ag_tekyon_ms": mu, "titresim_2sigma_ms": 2 * sig,
                            "dogrudan_p50_ms": round(base, 1),
                            "kurtarilan_en_kotu_ms": round(base + blk_max + (sp if k > 1 else 0), 1)}
                    for D in DEADLINES_T1:
                        rows[f"tavan_dogrudan_D{D}"] = round(phi((D - base) / sig), 4)
                    for b in BUDGETS_T2:
                        rows[f"tavan_dogrudan_OWD+{b}"] = round(phi((mu + b - base) / sig), 4)
                    bud_rows.append(rows)
    with open(os.path.join(out, "butce.csv"), "w", newline="") as f:
        w = csv.DictWriter(f, fieldnames=list(bud_rows[0].keys()))
        w.writeheader()
        w.writerows(bud_rows)

    # ── beklenen.csv: Monte Carlo ───────────────────────────────────────────────────────────────
    rows = []
    cells = []
    for rate_key in RATES:
        for d_key in DELAYS:
            for loss in LOSSES:
                for t in BUNDLES:
                    cells.append((rate_key, d_key, loss, t))
    for cell in cells:
        rate_key, d_key, loss, t = cell
        rate = RATES[rate_key]
        for arm, variants in (("arq", ["ideal"]), ("tekrar", ["ideal"]),
                              ("rs2", ["ideal", "pacer1.25", "pacer1.00"]),
                              ("rs4", ["ideal", "pacer1.25", "pacer1.00"]),
                              ("uyar_ates", ["ideal", "pacer1.25"])):
            for v in variants:
                seed = hash_seed(cell, arm, v)
                rng = random.Random(seed)
                K, r, E, ratios = simulate(cell, arm, v, rng)
                copies = 2 if arm == "tekrar" else 1
                a_l2, a_l2_arq, rho, rho_arq = wire_rate(t, K, r, E or 64, copies, loss / 100.0, rate)
                pp = (1 - (loss / 100.0) ** 2) if arm == "tekrar" else p_play(K, r, loss / 100.0)
                row = {"hiz": rate_key, "gecikme": d_key, "kayip": loss, "demet_ms": t,
                       "kol": arm, "varyant": v, "K": K or "", "r": r if K else "",
                       "E": E or "", "l2_Bps": round(a_l2, 1), "l2_Bps_arq_dahil": round(a_l2_arq, 1),
                       "rho": round(rho, 3), "rho_arq_dahil": round(rho_arq, 3),
                       "sigmaz": int(rho_arq >= 1.0),
                       "oynatilabilirlik_binom": round(pp, 4),
                       "blok_kurtarma_binom": round(p_block(K, r, loss / 100.0), 4) if K else ""}
                for key, vals in ratios.items():
                    tag = f"{key[0]}_{key[1]}"
                    row[f"{tag}_ort"] = round(sum(vals) / len(vals), 4)
                    row[f"{tag}_p25"] = round(q(vals, 0.25), 4)
                    row[f"{tag}_p75"] = round(q(vals, 0.75), 4)
                rows.append(row)
    with open(os.path.join(out, "beklenen.csv"), "w", newline="") as f:
        w = csv.DictWriter(f, fieldnames=list(rows[0].keys()))
        w.writeheader()
        w.writerows(rows)

    # ── MODEL-C.md: rapor için özet ─────────────────────────────────────────────────────────────
    with open(os.path.join(out, "MODEL-C.md"), "w") as f:
        f.write("# Ajan C ön-kayıt modeli — çıktı özeti [çıkarım]\n\n")
        f.write(f"Üreten: `scripts/ktp2/f0/c/model_c.py` · tohum {SEED} · {RUNS} MC koşu/hücre-kol-varyant · "
                f"konuşma {TALK_S:.0f} s.\n\n")
        f.write("## Ses tel hızı (FEC'siz)\n\n| demet ms | kodlayıcı B | paket/s | ürün UDP B (zarf 22) | "
                "UDP B/s | L2 B/s (netem) | başlık payı (UDP) | ρ r1k | ρ r12k |\n|---|---|---|---|---|---|---|---|---|\n")
        for g in geo_rows:
            f.write(f"| {g['demet_ms']} | {g['kodlayici_B']:.0f} | {g['paket_s']:.2f} | {g['urun_udp_legacy22_B']:.0f} | "
                    f"{g['udp_Bps_legacy22']:.0f} | {g['l2_Bps_bench']:.0f} | %{100 * g['baslik_payi_udp']:.0f} | "
                    f"{g['rho_r1k']:.2f} | {g['rho_r12k']:.3f} |\n")
        f.write("\n## Beklenen son-tarih oranı (ortalama; T1 = ağızdan kulağa D, T2 = tek yön + b)\n\n")
        f.write("| hız | gecikme | kayıp | demet | kol | varyant | K,r | ρ(ARQ dahil) | binom | T1-400 | T1-800 | T2-400 | T2-800 |\n")
        f.write("|---|---|---|---|---|---|---|---|---|---|---|---|---|\n")
        for rw in rows:
            kr = f"{rw['K']},{rw['r']}" if rw["K"] != "" else "—"
            f.write(f"| {rw['hiz']} | {rw['gecikme']} | %{rw['kayip']} | {rw['demet_ms']} | {rw['kol']} | {rw['varyant']} | "
                    f"{kr} | {rw['rho_arq_dahil']:.2f}{' SIĞMAZ' if rw['sigmaz'] else ''} | {rw['oynatilabilirlik_binom']:.3f} | "
                    f"{rw['T1_400_ort']:.3f} | {rw['T1_800_ort']:.3f} | {rw['T2_400_ort']:.3f} | {rw['T2_800_ort']:.3f} |\n")
    print(f"yazıldı: {out}/geometri.csv butce.csv beklenen.csv MODEL-C.md ({len(rows)} satır)")
    return 0


def hash_seed(cell, arm, v):
    """Python hash() süreç-tohumludur; deterministik tohum için basit FNV-1a."""
    s = f"{SEED}|{cell}|{arm}|{v}".encode()
    h = 0xCBF29CE484222325
    for b in s:
        h ^= b
        h = (h * 0x100000001B3) & 0xFFFFFFFFFFFFFFFF
    return h


if __name__ == "__main__":
    sys.exit(main())
