#!/usr/bin/env python3
"""[KTP2-F0 / Ajan B] Düşük bant + yüksek RTT ızgarasının fiziksel modeli (tek kaynak).

Bu dosya üç işin ORTAK kaynağıdır; sayılar yalnız burada yazılır:
  1. hücre geometrisi (mesaj sayısı, sunum aralığı, son tarih) — `gen_cells.py` buradan üretir;
  2. hücre başına fiziksel alt sınırlar ve ideal-ARQ sınırları — `RAPOR-B.md` tabloları;
  3. koşu süresi / bütçe tahmini — KOŞU İSTEĞİ.
`analiz_b.py` ölçümü buradaki alt sınırlarla kıyaslar.

Etiketler: tel boyları [kod] (dosya:satır aşağıda), süre/bütçe tahminleri [çıkarım].

Tel paketi (L2 dahil; netem `rate` L2 boyunu sayar — KATALOG §2):
  Kama veri  = 42 (Eth 14 + IPv4 20 + UDP 8) + 16 (bench zarfı: IdentitySeam etiketi,
               crates/client-core/src/seam.rs:105) + 11 (DATA başlığı, wire.rs:17) + yük
  Kama ACK   = 42 + 16 + 15 (ACK V2 gövdesi, ack.rs:204-232) + 4 × aralık (≤ 16, ack.rs:36)
  Spec zarfı = 22 B (hub_handle 4 + seq 2 + tag 16; specs/transport-layer-tdd.md:149) → bench'ten
               paket başına +6 B (ayrı sütun; bench bunu taşımıyor).
  TCP veri   = 14 + 20 + 32 (TCP + zaman damgası seçeneği, Linux varsayılanı) + 12 (bench
               çerçevesi u32 len + u64 id, tcp.rs:41) + yük;  uygulama ACK'i = 66 + 8 (tcp.rs:43)
  QUIC       = şifreli; tahmin [çıkarım]: kısa başlık 1 + CID 8 + PN 2 + etiket 16 + STREAM
               başlığı ≈ 5 → 42 + 32 + yük. İlk paketler ≥ 1200 B UDP yükü (RFC 9000 §14.1;
               quinn-proto 0.11.18 lib.rs:330 MIN_INITIAL_SIZE) → 1242 B tel.

Kullanım:
  python3 -I fizik.py tablo        # hücre başına fiziksel/ideal sınırlar (markdown)
  python3 -I fizik.py butce        # koşu süresi ve koşu-saati (öncelik sırasıyla)
  python3 -I fizik.py json         # hücre tanımları (gen_cells.py / analiz_b.py girdisi)
"""
import json
import math
import sys

# ── tel sabitleri [kod] ────────────────────────────────────────────────────────────────────────
L234 = 42                # Eth 14 + IPv4 20 + UDP 8
KAMA_TAG = 16            # bench IdentitySeam (seam.rs:105)
SPEC_ENV = 22            # spec SESSION_DATA zarfı (transport-layer-tdd.md:149)
HDR_DATA = 11            # wire.rs:17
HDR_FEC = 12             # wire.rs:104
FEC_E = 1024             # profil 0x05 sembol boyu (kama-fec-codec profile.rs:85-90)
ACK_BODY0 = 15           # ACK V2 sabit gövde (ack.rs:204)
ACK_RANGE = 4            # aralık başına (ack.rs)
ACK_MAX_RANGES = 16      # ack.rs:36
PARAM_BODY = 35          # wire.rs LEN_PARAM
TCP_L234 = 14 + 20 + 32  # Eth + IPv4 + TCP(20) + TS seçeneği(12)
TCP_FRAME = 12           # tcp.rs:41
TCP_APPACK = 8           # tcp.rs:43
QUIC_OVH_EST = 32        # [çıkarım] kısa başlık + etiket + STREAM başlığı
QUIC_INITIAL_UDP = 1200  # RFC 9000 §14.1

# ── profil eksenleri (KATALOG §2) ─────────────────────────────────────────────────────────────
RATES = {"r25": (25, "200bit"), "r1k": (1000, "8kbit"), "r12k": (12500, "100kbit")}
DELAYS = {"d300": (300, 60), "d1000": (1000, 200)}
LOSSES_PRIMARY = [0, 10, 20, 30]
LOSSES_STRESS = [50, 80]
PAYLOADS = [65, 1000]

# Sunulan yük: TEK iletimli Kama veri paketinin (L2 dahil) ileri hat kapasitesine oranı.
RHO = 0.30

# ── hücre geometrisi: (hız, yük) → (aralık ms, mesaj sayısı, son tarih s) ─────────────────────
# Aralık = Kama veri paketi / (RHO × hız), yukarı yuvarlanmış. Mesaj sayısı: koşu ≈ 20–130 s
# sunum + kuyruk; r25'te bütçe sınırı. Son tarih: K-2 (1 KB, 60 s) ve K-3 (r25, 65 B) için 60 s;
# r1k/r12k 65 B için 30 s (≥ 13 × RTT_fiz; TCP kolu bu süreyi her koşuda bekler — tcp.rs:165).
GEOMETRY = {
    ("r25", 65): (18000, 6, 60),
    ("r25", 1000): (143000, 3, 60),
    ("r1k", 65): (450, 100, 30),
    ("r1k", 1000): (3600, 20, 60),
    ("r12k", 65): (36, 600, 30),
    ("r12k", 1000): (285, 100, 60),
}

FWD_PLAN = {"N_HUKUM": 30, "N_STRES": 10}


def kama_data_l2(p):
    return L234 + KAMA_TAG + HDR_DATA + p


def kama_ack_l2(ranges=0):
    return L234 + KAMA_TAG + ACK_BODY0 + ACK_RANGE * min(ranges, ACK_MAX_RANGES)


def kama_fec_l2():
    return L234 + KAMA_TAG + HDR_FEC + FEC_E


def tcp_data_l2(p):
    return TCP_L234 + TCP_FRAME + p


def quic_data_l2(p):
    return L234 + QUIC_OVH_EST + p


def ser(nbytes, rate):
    return nbytes / rate


_FIFO_CACHE = {}


def fifo_owd(d, sigma, spacing, ser_s, n=20000, seed=1):
    """netem `rate` + jitter tek yön gecikmesi (kendi serileştirmesi hariç) — ortalama ve p95.

    netem_enqueue (sch_netem.c, rate kolu): kuyrukta paket varsa
      time_to_send = max(son.time_to_send, şimdi + delay_j) + ser_j
    → paketler sıra DEĞİŞTİRMEZ (KATALOG §2 beyanı) ve gecikme, önceki paketlerin gecikmesinin
    "aralık kadar azaltılmış" değeriyle maks alınır: σ ≥ aralık olduğunda ortalama tek yön gecikmesi
    profil değerinin ÜSTÜNE çıkar. Monte Carlo, sabit tohum (model kodu; ürün değil). [çıkarım]
    """
    key = (round(d, 6), round(sigma, 6), round(spacing, 6), round(ser_s, 6))
    if key in _FIFO_CACHE:
        return _FIFO_CACHE[key]
    import random
    r = random.Random(seed)
    last_tts = -1e18
    t = 0.0
    out = []
    for _ in range(n):
        t += spacing
        dj = max(0.0, r.gauss(d, sigma))
        start = max(last_tts, t + dj) if last_tts > t else t + dj
        last_tts = start + ser_s
        out.append(start - t)
    out = sorted(out[500:])
    res = (sum(out) / len(out), out[int(0.95 * (len(out) - 1))])
    _FIFO_CACHE[key] = res
    return res


def netem_str(rate_id, delay_id, loss):
    """KATALOG §2 sırası: delay … distribution normal [loss N%] rate X."""
    d, j = DELAYS[delay_id]
    _, rate_tc = RATES[rate_id]
    s = f"delay {d}ms {j}ms distribution normal"
    if loss:
        s += f" loss {loss}%"
    return s + f" rate {rate_tc}"


def cell_name(rate_id, delay_id, loss, p):
    return f"b_{rate_id}_{delay_id}_l{loss}_p{p}"


def phys(rate_id, delay_id, loss, p):
    """Hücre başına fiziksel model [kod boylar + çıkarım formüller]."""
    R, _ = RATES[rate_id]
    d_ms, j_ms = DELAYS[delay_id]
    d = d_ms / 1000.0
    ival_ms, m, dl = GEOMETRY[(rate_id, p)]
    I = ival_ms / 1000.0
    s_data = kama_data_l2(p)
    s_ack0 = kama_ack_l2(0)
    s_ack16 = kama_ack_l2(16)
    t_data = ser(s_data, R)
    rtt_phys = 2 * d + t_data + ser(s_ack0, R)          # boş hat, 0 aralıklı ACK
    rtt_phys_r16 = 2 * d + t_data + ser(s_ack16, R)     # kalıcı deliklerle 16 aralıklı ACK
    oneway_lb = t_data + d                              # alıcıda ilk varış alt sınırı
    rho_fwd = s_data / (R * I)
    q = loss / 100.0
    # ideal ARQ: kayıp her denemede bağımsız q; yeniden deneme aralığı = RTT_fiz (kusursuz RTO).
    # alıcı varış gecikmesi k. denemede: oneway_lb + (k−1)·RTT_fiz.
    def p_within(T, rtt):
        if T < oneway_lb:
            return 0.0
        kmax = 1 + math.floor((T - oneway_lb) / rtt)
        return 1 - q ** kmax
    def q_lat(pct, rtt):
        # P(K ≤ k) = 1 − q^k ≥ pct → k = ⌈ln(1−pct)/ln q⌉
        if q == 0:
            return oneway_lb
        k = max(1, math.ceil(math.log(1 - pct) / math.log(q) - 1e-12))
        return oneway_lb + (k - 1) * rtt
    # 2 kopya (eşzamanlı, kör çoğaltma) — K-3 hedefiyle kıyas için: ikinci kopya +1 serileştirme.
    def q_lat_dup2(pct):
        qq = q * q
        if qq == 0:
            return oneway_lb + t_data
        k = max(1, math.ceil(math.log(1 - pct) / math.log(qq) - 1e-12))
        return oneway_lb + t_data + (k - 1) * rtt_phys
    exp_tx = 1 / (1 - q) if q < 1 else float("inf")
    # netem FIFO-jitter düzeltmesi: ileri aralık = I; dönüş ACK aralığı ≈ 2I (ACK_FREQ_N = 2,
    # receiver.rs:76) — I ≥ 8,39 s'de (cadence tabanı, receiver.rs:93) mesaj başına ACK → I.
    j = j_ms / 1000.0
    ack_sp = I if I >= 8.387 else 2 * I
    owd_f, owd_f95 = fifo_owd(d, j, I, t_data)
    owd_r, _ = fifo_owd(d, j, ack_sp, ser(s_ack0, R))
    rtt_fifo = owd_f + t_data + owd_r + ser(s_ack0, R)
    return {
        "cell": cell_name(rate_id, delay_id, loss, p),
        "rate_id": rate_id, "delay_id": delay_id, "loss": loss, "payload": p,
        "rate_Bps": R, "delay_ms": d_ms, "jitter_ms": j_ms,
        "ival_ms": ival_ms, "msgs": m, "deadline_s": dl,
        "s_data_l2": s_data, "s_ack_l2_0": s_ack0, "s_ack_l2_16": s_ack16,
        "s_data_spec22": s_data + (SPEC_ENV - KAMA_TAG),
        "t_ser_data_s": t_data, "t_ser_ack0_s": ser(s_ack0, R), "t_ser_ack16_s": ser(s_ack16, R),
        "rtt_phys_s": rtt_phys, "rtt_phys_r16_s": rtt_phys_r16,
        "oneway_lb_s": oneway_lb, "conf_lb_s": rtt_phys,
        "owd_fwd_fifo_s": owd_f, "owd_fwd_fifo_p95_s": owd_f95, "owd_rev_fifo_s": owd_r,
        "rtt_fifo_s": rtt_fifo, "oneway_fifo_s": owd_f + t_data,
        "rho_fwd": rho_fwd, "rho_fwd_arq": rho_fwd * exp_tx,
        "rho_rev_ack_per_msg_r16": ser(s_ack16, R) / I,
        "useful_share_lb": p / s_data,  # tek iletim, ACK hariç: faydalı yük / veri paketi
        "fec_repair_l2": kama_fec_l2(), "fec_vs_data": kama_fec_l2() / s_data,
        "ideal_arq_within_D": p_within(dl, rtt_phys),
        "ideal_arq_oneway_p50_s": q_lat(0.50, rtt_phys),
        "ideal_arq_oneway_p95_s": q_lat(0.95, rtt_phys),
        "ideal_arq_oneway_p99_s": q_lat(0.99, rtt_phys),
        "dup2_oneway_p95_s": q_lat_dup2(0.95),
        "tcp_data_l2": tcp_data_l2(p), "quic_data_l2_est": quic_data_l2(p),
        "quic_initial_ser_s": ser(QUIC_INITIAL_UDP + L234, R),
        "netem": netem_str(rate_id, delay_id, loss),
    }


# ── ızgara + öncelik ─────────────────────────────────────────────────────────────────────────
# Öncelik (P1 en önce). Budama açık yazılı:
#   P1  r1k/r12k × d300/d1000 × l0/l20/l30 × 65 B/1 KB         (birincil, N = 30)
#   P2  r25 × d300/d1000 × l0/l20 × yalnız 65 B                 (birincil, N = 30)
#   P3  r1k/r12k × d300/d1000 × l10 × 65 B/1 KB                 (birincil ara değer, N = 30)
#   P4  stres: r1k/r12k × d300 × l50/l80 × 65 B/1 KB            (betimsel, N = 10)
#   P5  BÜTÇE DIŞI (budandı): r25 × l10/l30 × 65 B              (birincil, N = 30)
#   P6  BÜTÇE DIŞI (budandı): r25 × d300 × l0 × 1 KB            (betimsel, N = 10)
# r25'te 1 KB birincil ızgaradan çıkarıldı: tek mesajın serileştirmesi 42,8 s; ρ = 0,3'te aralık
# 143 s → hücre başına ≈ 4 saat yalnız 10 tekrar için.
def grid():
    cells = []
    def add(rate_id, delay_id, loss, p, prio, reps, kind):
        c = phys(rate_id, delay_id, loss, p)
        c.update({"prio": prio, "reps": reps, "kind": kind})
        cells.append(c)
    for loss_set, prio in (((0, 20, 30), "P1"), ((10,), "P3")):
        for rate_id in ("r1k", "r12k"):
            for delay_id in DELAYS:
                for loss in loss_set:
                    for p in PAYLOADS:
                        add(rate_id, delay_id, loss, p, prio, FWD_PLAN["N_HUKUM"], "hukum")
    for delay_id in DELAYS:
        for loss in (0, 20):
            add("r25", delay_id, loss, 65, "P2", FWD_PLAN["N_HUKUM"], "hukum")
    for rate_id in ("r1k", "r12k"):
        for loss in LOSSES_STRESS:
            for p in PAYLOADS:
                add(rate_id, "d300", loss, p, "P4", FWD_PLAN["N_STRES"], "stres")
    for delay_id in DELAYS:
        for loss in (10, 30):
            add("r25", delay_id, loss, 65, "P5", FWD_PLAN["N_HUKUM"], "hukum")
    add("r25", "d300", 0, 1000, "P6", 10, "betimsel")
    return sorted(cells, key=lambda c: (c["prio"], c["cell"]))


ARMS = ["taban", "aday", "quinn", "tcp"]
OVERHEAD_S = 1.5 + 0.5 + 1.0  # SETTLE_S (hücre: 1,5 s) + kapanış uykusu + kurulum/sökme (f0_cell.sh)


def run_time(c, arm):
    """Koşu süresi tahmini (s): (beklenen, en kötü). [çıkarım]"""
    I = c["ival_ms"] / 1000.0
    m, dl = c["msgs"], c["deadline_s"]
    pres = (m - 1) * I
    rtt = c["rtt_phys_r16_s"] if c["loss"] else c["rtt_phys_s"]
    q = c["loss"] / 100.0
    # Kama istemcisi tüm teyitler gelince ya da son sunum + D'de biter (kama_product.rs:757-781).
    # Beklenen kuyruk: kayıpsızda ≈ RTT + I (ACK_FREQ_N=2 tutması), kayıpta en kötü mesajın
    # birkaç RTO'su; stres hücrelerinde D'ye dayanır.
    if q == 0:
        tail = min(dl, rtt + I)
    elif q <= 0.30:
        tail = min(dl, 4 * rtt + I)
    else:
        tail = dl
    worst = pres + dl + OVERHEAD_S
    if arm in ("taban", "aday"):
        return pres + tail + OVERHEAD_S, worst
    if arm == "quinn":
        # El sıkışma D ile sınırlı (quic.rs:256); r25'te ilk paket ≥ 49,7 s/yön → başarısız → koşu
        # handshake_failed çıktısıyla biter (mesaj sunulmaz).
        hs_fail = c["rate_id"] == "r25"
        if hs_fail:
            return dl + 3 + OVERHEAD_S, dl + 3 + OVERHEAD_S
        hs = 3 * (2 * c["delay_ms"] / 1000.0 + c["quic_initial_ser_s"])
        hs = min(hs, dl)
        return hs + pres + tail + 3 + OVERHEAD_S, dl + pres + dl + 3 + OVERHEAD_S
    if arm == "tcp":
        # TCP istemcisi tüm mesajları yazdıktan sonra SABİT min(D, 60 s) bekler (tcp.rs:164-165).
        setup = 2 * c["delay_ms"] / 1000.0 + 2 * ser(74, c["rate_Bps"])
        t = setup + m * I + min(dl, 60) + 2 + OVERHEAD_S
        return t, 20 + m * I + min(dl, 60) + 2 + OVERHEAD_S
    raise ValueError(arm)


def budget_s(c):
    """Hücre zaman aşımı (BUDGET_S): her kolun en kötü süresinin üstü + 30 s pay."""
    return int(math.ceil(max(run_time(c, a)[1] for a in ARMS))) + 30


def cmd_tablo():
    cells = grid()
    print("| hücre | öncelik | N | I (ms) | M | D (s) | ρ ileri | ρ ileri (ideal ARQ) | veri paketi (B) | "
          "ser. veri (s) | ser. ACK 0/16 (s) | RTT_fiz 0/16 (s) | RTT_fifo (s) | tek yön alt sınır / fifo (s) | "
          "ideal ARQ p95 tek yön (s) | ideal ARQ D içinde | faydalı yük payı ≤ |")
    print("|" + "---|" * 17)
    for c in cells:
        print(f"| {c['cell']} | {c['prio']} | {c['reps']} | {c['ival_ms']} | {c['msgs']} | {c['deadline_s']} | "
              f"{c['rho_fwd']:.3f} | {c['rho_fwd_arq']:.3f} | {c['s_data_l2']} | {c['t_ser_data_s']:.3f} | "
              f"{c['t_ser_ack0_s']:.3f}/{c['t_ser_ack16_s']:.3f} | {c['rtt_phys_s']:.3f}/{c['rtt_phys_r16_s']:.3f} | "
              f"{c['rtt_fifo_s']:.3f} | {c['oneway_lb_s']:.3f}/{c['oneway_fifo_s']:.3f} | "
              f"{c['ideal_arq_oneway_p95_s']:.2f} | {c['ideal_arq_within_D']:.4f} | "
              f"{c['useful_share_lb']:.3f} |")


def cmd_butce():
    cells = grid()
    tot = {}
    tot_w = {}
    print("| hücre | öncelik | N | taban/aday (s, beklenen) | quinn (s) | tcp (s) | hücre koşu-saati (beklenen) | en kötü (saat) | BUDGET_S |")
    print("|---|---|---|---|---|---|---|---|---|")
    for c in cells:
        e = {a: run_time(c, a) for a in ARMS}
        h = c["reps"] * sum(v[0] for v in e.values()) / 3600
        w = c["reps"] * sum(v[1] for v in e.values()) / 3600
        tot[c["prio"]] = tot.get(c["prio"], 0) + h
        tot_w[c["prio"]] = tot_w.get(c["prio"], 0) + w
        print(f"| {c['cell']} | {c['prio']} | {c['reps']} | {e['taban'][0]:.0f} | {e['quinn'][0]:.0f} | "
              f"{e['tcp'][0]:.0f} | {h:.2f} | {w:.2f} | {budget_s(c)} |")
    print()
    cum = 0.0
    cum_w = 0.0
    for p in sorted(tot):
        cum += tot[p]
        cum_w += tot_w[p]
        print(f"{p}: beklenen {tot[p]:.1f} sa (kümülatif {cum:.1f}) · en kötü {tot_w[p]:.1f} sa (kümülatif {cum_w:.1f})")


def cmd_json():
    out = []
    for c in grid():
        c = dict(c)
        c["budget_s"] = budget_s(c)
        c["run_time_expected_s"] = {a: run_time(c, a)[0] for a in ARMS}
        out.append(c)
    json.dump(out, sys.stdout, indent=1, ensure_ascii=False)
    print()


def by_cell():
    return {c["cell"]: c for c in grid()}


if __name__ == "__main__":
    {"tablo": cmd_tablo, "butce": cmd_butce, "json": cmd_json}.get(
        sys.argv[1] if len(sys.argv) > 1 else "tablo", cmd_tablo)()
