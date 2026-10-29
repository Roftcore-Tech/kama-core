#!/usr/bin/env python3
"""[KTP2-F0 / Ajan B] Düşük bant + yüksek RTT analizi (salt okur; koşu başlatmaz).

Girdi: f0_grid.sh çıktı kökü — <KÖK>/<HÜCRE>/{cell.env, DONE}, <KÖK>/<HÜCRE>/<kol>/rep<N>/…
Çıktı (varsayılan <KÖK>/analiz_b/):
  kosular_b.csv   koşu başına bütün ölçüler (bir satır = kol × tekrar)
  hucre_b.csv     hücre × kol × ölçü: N, medyan, Q1, Q3, min, maks
  OZET-B.md       seçili ölçülerin hücre tabloları + fiziksel sınırlar (fizik.py)

Ölçü kaynakları ve etiketleri (rapora aynen taşınır):
  [olay]     out.jsonl (bench RawEvent): teslim oranı D içinde, gönderen-teyit gecikmesi
  [sayaç]    c.err/s.err (KP-CLIENT, KP-ACTIVATED, KP-ACTIVATION, KP-SERVER, BUDGET-REV,
             TCP-CLIENT/SERVER), manifest.json
  [qdisc]    realized_pre/post.json: netem yön başına geçen bayt/paket ve düşüş (mb0 = ileri,
             ma0 = dönüş; kapi3_harness.sh kapi3_netem)
  [yakalama] capb_cli.csv.gz (va0) + capb_srv.csv.gz (vb0) — cap_b.py
Yakalama yoksa ilgili sütunlar boş kalır ("ölçülemedi: yakalama yok").

Kullanım: python3 -I analiz_b.py KÖK [--out DİZİN] [--hucre DESEN]
"""
import argparse
import csv
import gzip
import json
import math
import pathlib
import re
import statistics
import sys

HERE = pathlib.Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))  # -I betik dizinini eklemez; kendi modülümüz açıkça
import fizik  # noqa: E402

KAMA_K = 8  # profil 0x05 k_shape (unit_id = block × K + esi; runtime.rs RECOVERED yolu)
ALT_DEADLINES_S = (5, 10, 20, 30, 60)

# ════════════════════════════════════════════════════════════════════════════════════════════
# Yardımcılar
# ════════════════════════════════════════════════════════════════════════════════════════════


def pct(vals, q):
    """Doğrusal ara değerli yüzdelik (q ∈ [0,1]); boş → None."""
    v = sorted(x for x in vals if x is not None)
    if not v:
        return None
    if len(v) == 1:
        return v[0]
    pos = q * (len(v) - 1)
    lo = math.floor(pos)
    hi = min(lo + 1, len(v) - 1)
    return v[lo] + (v[hi] - v[lo]) * (pos - lo)


def med(vals):
    return pct(vals, 0.5)


def num(s):
    if s is None:
        return None
    m = re.match(r"-?[0-9]+(?:\.[0-9]+)?", s)
    return float(m.group(0)) if m else None


def kv(line, key):
    m = re.search(r"(?:^|[\s\[,(])" + re.escape(key) + r"=([^\s\],)]+)", line)
    return m.group(1) if m else None


def read_text(p):
    try:
        return pathlib.Path(p).read_text(errors="replace")
    except OSError:
        return ""


# ════════════════════════════════════════════════════════════════════════════════════════════
# Koşu girdileri
# ════════════════════════════════════════════════════════════════════════════════════════════


def read_events(rep):
    meta, ev = None, []
    p = rep / "out.jsonl"
    if not p.exists():
        return None, []
    for line in open(p, errors="replace"):
        line = line.strip()
        if not line:
            continue
        try:
            j = json.loads(line)
        except json.JSONDecodeError:
            continue
        if "meta" in j:
            meta = j["meta"]
        elif "event" in j:
            ev.append(j["event"])
    ev.sort(key=lambda e: e["msg_id"])
    return meta, ev


def parse_client_err(txt):
    r = {}
    sched_on = "diag-sched-telemetry=true" in txt
    for line in txt.splitlines():
        if line.startswith("KP-CLIENT"):
            m = re.search(r"tx=(\d+)B\(data=(\d+) retx=(\d+) dup=(\d+) fec=(\d+) ctrl=(\d+)\)", line)
            if m:
                (r["tx_total_B"], r["tx_data_B"], r["tx_retx_B"], r["tx_dup_B"], r["tx_fec_B"],
                 r["tx_ctrl_B"]) = map(int, m.groups())
            m = re.search(r"refused=(\d+)B/(\d+)pkt", line)
            if m:
                r["cli_refused_B"], r["cli_refused_pkts"] = int(m.group(1)), int(m.group(2))
            for k, name in (("rx", "rx_B"), ("stale", "stale"), ("establish_tx", "establish_tx"),
                            ("fec_recovered_ctrl", "fec_recovered_ctrl"), ("send_err", "send_err"),
                            ("seal_fail", "seal_fail"), ("param_confirmed", "param_confirmed")):
                r[name] = num(kv(line, k))
            r["state_end"] = kv(line, "state")
        elif line.startswith("KP-ACTIVATED"):
            for k in ("rtt_samples", "data_denied", "retx_denied", "spurious", "gate_opened"):
                r[k] = num(kv(line, k))
            r["ret_ack_loss_permille"] = num(kv(line, "ret_ack_loss"))
            r["fwd_loss_permille"] = num(kv(line, "fwd_loss"))
            r["ack_int_ms"] = num(kv(line, "ack_int"))
            r["rp_bw_Bps"] = num(kv(line, "bw"))
        elif line.startswith("KP-ACTIVATION"):
            m = re.search(r"tail_probe=(\d+)@(\d+)ms", line)
            if m:
                r["tail_probes"], r["tail_first_ms"] = int(m.group(1)), int(m.group(2))
            for k in ("tail_cancel", "parity", "parity_denied", "retx", "dup", "transitions",
                      "recover_arq", "recover_fwd", "probe_up", "left_preconv"):
                r[k] = num(kv(line, k))
            for k in ("btlbw", "srtt", "rtprop", "qdelay", "pace", "tokens", "inflight"):
                r["est_" + k] = num(kv(line, k))
        elif line.startswith("TCP-CLIENT"):
            r["tcp_cli_delivered"] = num(kv(line, "delivered"))
            r["tcp_send_errors"] = num(kv(line, "send_errors"))
    if "KP-MANIFEST" in txt and not sched_on:
        # zamanlayıcı ret sayaçları yalnız diag-sched-telemetry derlemesinde dolar (sender.rs:4611-4632);
        # varsayılan derlemede 0 yazılır — "ret yok" diye okunmasın.
        r["data_denied"] = None
        r["retx_denied"] = None
    return r


SRV_FIELDS = ("delivered", "delivery_dropped", "dropped_new_peers", "rx_datagrams", "rx_bytes",
              "tx_datagrams", "tx_bytes", "fec_repairs_rx", "fec_recovered_units",
              "fec_decode_failures", "fec_blocks_evicted", "fec_repairs_wasted_complete",
              "fec_repairs_wasted_evicted", "fec_repairs_useful", "fec_recovered_ctrl_tx",
              "frag_chunk_dropped", "msg_asm_evicted", "fec_pre_param_dropped", "param_rejects",
              "egress_refused_pkts", "egress_refused_bytes", "open_failures", "unknown_frames")


def parse_server_err(txt):
    r = {}
    for line in txt.splitlines():
        if line.startswith("KP-SERVER"):
            for k in SRV_FIELDS:
                m = re.search(r"\b" + k + r": (\d+)", line)
                if m:
                    r["srv_" + k] = int(m.group(1))
        elif line.startswith("BUDGET-REV"):
            m = re.search(r"injected=(\d+)B/(\d+)pkt", line)
            if m:
                r["srv_inj_ip_B"], r["srv_inj_pkts"] = int(m.group(1)), int(m.group(2))
        elif line.startswith("TCP-SERVER"):
            r["tcp_srv_delivered"] = num(kv(line, "delivered"))
    return r


def parse_realized(rep):
    """mb0 = ileri (B'ye doğru), ma0 = dönüş: pre/post farkı."""
    out = {}
    snaps = {}
    for tag in ("pre", "post"):
        txt = read_text(rep / f"realized_{tag}.json")
        if not txt:
            return out
        try:
            j = json.loads(txt)
        except json.JSONDecodeError:
            return out
        snaps[tag] = {q.get("dev"): q for q in j.get("mid_qdisc", []) if q.get("kind") == "netem"}
    for dev, name in (("mb0", "fwd"), ("ma0", "rev")):
        a, b = snaps.get("pre", {}).get(dev), snaps.get("post", {}).get(dev)
        if a is None or b is None:
            continue
        out[f"q_{name}_bytes"] = b.get("bytes", 0) - a.get("bytes", 0)
        out[f"q_{name}_pkts"] = b.get("packets", 0) - a.get("packets", 0)
        out[f"q_{name}_drops"] = b.get("drops", 0) - a.get("drops", 0)
        out[f"q_{name}_backlog_end"] = b.get("backlog", 0)
        out[f"q_{name}_qlen_end"] = b.get("qlen", 0)
    return out


def read_cap(path):
    """→ [(t_ns, dir, proto, ip_len, sport, dport, x)] zaman sıralı; dosya yoksa None."""
    if not path.exists():
        return None
    rows = []
    with gzip.open(path, "rt", errors="replace") as f:
        for line in f:
            if not line or line[0] == "#":
                continue
            p = line.rstrip("\n").split(",", 6)
            if len(p) < 7:
                continue
            try:
                rows.append((int(p[0]), p[1], int(p[2]), int(p[3]), int(p[4]), int(p[5]), p[6]))
            except ValueError:
                continue
    rows.sort(key=lambda r: r[0])
    return rows


def server_port(rep):
    m = re.search(r"--bind\s+\S+:(\d+)", read_text(rep / "cmd.txt"))
    return int(m.group(1)) if m else None


def read_float(p):
    try:
        return float(read_text(p).strip())
    except ValueError:
        return None


# ════════════════════════════════════════════════════════════════════════════════════════════
# Kama tel çözücüsü (bench IdentitySeam: çerçeve ‖ etiket16, etiketin ilk 8 baytı PN)
# ════════════════════════════════════════════════════════════════════════════════════════════

FT = {0x00: "data", 0x05: "data_frag", 0x01: "fec", 0x02: "recovered", 0x03: "param",
      0x04: "param_ack", 0x06: "cs_probe", 0x07: "cs_echo", 0x10: "ack", 0x14: "ack"}
HDR_LEN = {"data": 11, "data_frag": 22, "fec": 12, "recovered": 6, "param": 35, "param_ack": 33,
           "cs_probe": 28, "cs_echo": 27}


def kama_decode(x, udp_len):
    """UDP yükü onaltılığından çerçeve özeti. Kısaltılmış kayıtta baş 40 B + son 16 B vardır."""
    if ":" in x:
        h, t = x.split(":", 1)
        head, tail = bytes.fromhex(h), bytes.fromhex(t)
        frame = head
    else:
        b = bytes.fromhex(x)
        if len(b) < 17:
            return None
        head, tail, frame = b, b[-16:], b[:-16]
    if len(tail) < 16 or not frame:
        return None
    pn = int.from_bytes(tail[:8], "big")
    ft = FT.get(frame[0], "other")
    flen = udp_len - 16
    d = {"pn": pn, "type": ft, "flen": flen, "frame": frame if ":" not in x else None}
    if ft in ("data", "data_frag") and len(frame) >= 11:
        d["unit"] = int.from_bytes(frame[3:11], "big")
        d["paylen"] = flen - HDR_LEN[ft]
    elif ft == "fec" and len(frame) >= 12:
        d["kind"] = frame[1]
        d["block"] = int.from_bytes(frame[3:7], "big")
        d["esi"] = int.from_bytes(frame[7:9], "big")
        d["paylen"] = flen - 12
    elif ft == "recovered" and d["frame"] is not None and len(frame) >= 6:
        n = frame[5]
        d["block"] = int.from_bytes(frame[1:5], "big")
        d["esis"] = [int.from_bytes(frame[6 + 2 * i:8 + 2 * i], "big") for i in range(n)
                     if 8 + 2 * i <= len(frame)]
    elif ft == "ack" and d["frame"] is not None:
        d["acked16"], d["ack_delay_us"], d["ranges"] = ack_decode(frame)
    return d


def ack_decode(fr):
    """ack.rs from_received/acked_full tersi (u16 uzayında). → (set, ack_delay_us, aralık sayısı)"""
    if len(fr) < 9:
        return set(), None, None
    v2 = fr[0] == 0x14
    largest = int.from_bytes(fr[2:4], "big")
    delay = int.from_bytes(fr[4:6], "big") * (256 if v2 else 1)
    b0 = int.from_bytes(fr[6:8], "big")
    n = fr[8]
    s = set()
    if b0 == 0:
        return s, delay, n
    cursor = (largest - b0 + 1) & 0xFFFF
    for k in range(b0):
        s.add((cursor + k) & 0xFFFF)
    off = 9
    for _ in range(n):
        if off + 4 > len(fr):
            break
        gap = int.from_bytes(fr[off:off + 2], "big")
        ln = int.from_bytes(fr[off + 2:off + 4], "big")
        off += 4
        cursor = (cursor - gap) & 0xFFFF
        if ln:
            for k in range(ln):
                s.add((cursor - k) & 0xFFFF)
            cursor = (cursor - ln) & 0xFFFF
    return s, delay, n


# ════════════════════════════════════════════════════════════════════════════════════════════
# Aralık yardımcıları (hat meşgul/boş ayrıştırması)
# ════════════════════════════════════════════════════════════════════════════════════════════


def merge_iv(ivs):
    out = []
    for a, b in sorted(ivs):
        if b <= a:
            continue
        if out and a <= out[-1][1]:
            out[-1][1] = max(out[-1][1], b)
        else:
            out.append([a, b])
    return out


def overlap_total(ivs_sorted, a, b):
    """Birleşik (sıralı, ayrık) aralık listesinin [a,b] ile kesişim uzunluğu."""
    tot = 0
    for x, y in ivs_sorted:
        if y <= a:
            continue
        if x >= b:
            break
        tot += min(y, b) - max(x, a)
    return tot


# ════════════════════════════════════════════════════════════════════════════════════════════
# Kama yakalama analizi
# ════════════════════════════════════════════════════════════════════════════════════════════


def kama_capture(rep, g, meta, ev):
    cli = read_cap(rep / "capb_cli.csv.gz")
    srv = read_cap(rep / "capb_srv.csv.gz")
    if cli is None or srv is None:
        return {"cap": "yok"}
    port = server_port(rep)
    R = g["rate_Bps"]
    # Referans tek yön gecikmesi: netem FIFO-jitter beklenen değeri (fizik.fifo_owd; aralık = I).
    # Profil değeri (delay_ms) σ ≥ aralıkta gerçek ortalamanın altında kalır → "kuyruk fazlası"
    # sıfır kuyrukta bile pozitif çıkardı.
    d_nom = g["owd_fwd_fifo_s"]
    jit = g["jitter_ms"] / 1000.0
    rtt_phys = max(g["rtt_fifo_s"], g["rtt_phys_r16_s"] if g["loss"] else g["rtt_phys_s"])

    def udp(rows, direction, to_server):
        out = []
        for t, dr, pr, ipl, sp, dp, x in rows:
            if pr != 17 or dr != direction or not x:
                continue
            if port is not None and (dp if to_server else sp) != port:
                continue
            d = kama_decode(x, ipl - 28)
            if d is None:
                continue
            d["t"] = t / 1e9
            d["l2"] = ipl + 14
            out.append(d)
        return out

    a_out = udp(cli, "o", True)    # ileri, enjekte (netem öncesi)
    b_in = udp(srv, "i", True)     # ileri, darboğazdan geçen
    b_out = udp(srv, "o", False)   # dönüş, enjekte
    a_in = udp(cli, "i", False)    # dönüş, darboğazdan geçen
    r = {"cap": "var"}
    if not a_out:
        r["cap"] = "boş"
        return r

    b_in_t = {}
    for p in b_in:
        b_in_t.setdefault(p["pn"], p)
    a_in_t = {}
    for p in a_in:
        a_in_t.setdefault(p["pn"], p)

    # ── birim yaşam çizelgesi ──────────────────────────────────────────────────────────────
    txs = {}
    for p in a_out:
        if p["type"] in ("data", "data_frag"):
            txs.setdefault(p["unit"], []).append(p)
    useful_pn = set()
    first_arr = {}
    for u, lst in txs.items():
        arrs = sorted((b_in_t[p["pn"]]["t"], p["pn"]) for p in lst if p["pn"] in b_in_t)
        if arrs:
            first_arr[u] = arrs[0]
            useful_pn.add(arrs[0][1])
    first_tx_pn = {lst[0]["pn"] for lst in txs.values()}
    # FEC ile kurtarılan birim: B'den çıkan RECOVERED çerçevesi (unit = block × K + esi).
    rec_t = {}
    for p in b_out:
        if p["type"] == "recovered":
            for e in p.get("esis", []):
                rec_t.setdefault(p["block"] * KAMA_K + e, p["t"])

    # ── sınıf baytları (ileri: varış düzlemi B_in; deneme düzlemi A_out) ──────────────────
    def classify_fwd(p):
        t = p["type"]
        if t in ("data", "data_frag"):
            orig = p["pn"] in first_tx_pn
            if p["pn"] in useful_pn:
                return "pay_useful_orig" if orig else "pay_useful_copy"
            return "pay_redund_orig" if orig else "pay_redund_copy"
        if t == "fec":
            return "fec_repair" if p.get("kind") == 0x02 else "fec_source"
        if t == "cs_probe":
            return "probe"
        if t == "param":
            return "ctrl"
        return "other"

    def account(plane, prefix, cls_fn):
        acc = {}
        for p in plane:
            c = cls_fn(p)
            acc[prefix + "l234"] = acc.get(prefix + "l234", 0) + 42
            acc[prefix + "env"] = acc.get(prefix + "env", 0) + 16
            # başlık sütunu yalnız yük taşıyan çerçevelerde (DATA 11 / DATA_FRAG 22 / FEC 12);
            # denetim çerçevelerinin (ACK, PARAM, RECOVERED, prob) bütün gövdesi kendi sınıfına yazılır.
            hl = HDR_LEN[p["type"]] if p["type"] in ("data", "data_frag", "fec") else 0
            acc[prefix + "hdr"] = acc.get(prefix + "hdr", 0) + min(hl, p["flen"])
            if p["type"] == "ack":
                acc[prefix + "ackpkt_l2"] = acc.get(prefix + "ackpkt_l2", 0) + p["l2"]
            body = max(0, p["flen"] - min(hl, p["flen"]))
            acc[prefix + c] = acc.get(prefix + c, 0) + body
            acc[prefix + "pkts"] = acc.get(prefix + "pkts", 0) + 1
            acc[prefix + "total"] = acc.get(prefix + "total", 0) + p["l2"]
        return acc

    def classify_rev(p):
        return {"ack": "ack", "recovered": "recovered", "param_ack": "ctrl",
                "cs_echo": "probe"}.get(p["type"], "other")

    r.update(account(b_in, "wf_", classify_fwd))       # ileri, hattan geçen
    r.update(account(a_out, "wfa_", classify_fwd))     # ileri, deneme
    r.update(account(a_in, "wr_", classify_rev))       # dönüş, hattan geçen
    r.update(account(b_out, "wra_", classify_rev))     # dönüş, deneme
    r["wf_lost_pkts"] = sum(1 for p in a_out if p["pn"] not in b_in_t)
    r["wr_lost_pkts"] = sum(1 for p in b_out if p["pn"] not in a_in_t)

    # ── pencere ve kapasite ────────────────────────────────────────────────────────────────
    t0u = meta.get("t0_unix_ns", 0) / 1e9 if meta else 0
    off = meta.get("events_epoch_offset_ns", 0) if meta else 0
    offer = {e["msg_id"]: t0u + (e["sent_ns"] - off) / 1e9 for e in ev if e.get("sent_ns")}
    conf = {e["msg_id"]: t0u + (e["delivered_ns"] - off) / 1e9 for e in ev
            if e.get("delivered_ns") is not None}
    t_lo = min(offer.values()) if offer else a_out[0]["t"]
    t_hi = max([p["t"] for p in b_in] + [p["t"] for p in a_in] + [t_lo])
    win = max(t_hi - t_lo, 1e-9)
    r["win_s"] = win
    r["wf_cap_B"] = R * win
    r["wf_util"] = r.get("wf_total", 0) / (R * win)
    r["wr_util"] = r.get("wr_total", 0) / (R * win)
    r["wfa_offered_load"] = r.get("wfa_total", 0) / (R * win)

    # ── ACK kapsamı (u16 PN → tam PN) ve gerçek RTT ────────────────────────────────────────
    low2full = {}
    for p in a_out:
        low2full[p["pn"] & 0xFFFF] = p["pn"]  # koşu < 65536 paket: sarma yok
    aout_t = {p["pn"]: p["t"] for p in a_out}
    cov_a = {}
    cov_a_spn = {}
    for p in a_in:
        if p["type"] != "ack":
            continue
        for lo in p.get("acked16", ()):
            full = low2full.get(lo)
            if full is not None and full not in cov_a:
                cov_a[full] = p["t"]
                cov_a_spn[full] = p["pn"]
    cov_b = {}
    bout_t = {}
    for p in b_out:
        bout_t[p["pn"]] = p["t"]
        if p["type"] != "ack":
            continue
        for lo in p.get("acked16", ()):
            full = low2full.get(lo)
            if full is not None and full not in cov_b:
                cov_b[full] = p["t"]
    rtt_wire, rtt_net, hold = [], [], []
    for p in a_out:
        if p["type"] != "data" or p["pn"] not in first_tx_pn:
            continue
        pn = p["pn"]
        if pn in cov_a:
            rtt_wire.append(cov_a[pn] - p["t"])
            spn = cov_a_spn[pn]
            if pn in b_in_t and spn in bout_t:
                rtt_net.append((b_in_t[pn]["t"] - p["t"]) + (cov_a[pn] - bout_t[spn]))
        if pn in b_in_t and pn in cov_b:
            hold.append(cov_b[pn] - b_in_t[pn]["t"])
    r["rtt_wire_med_s"] = med(rtt_wire)
    r["rtt_wire_last20_med_s"] = med(rtt_wire[-20:])
    r["rtt_net_med_s"] = med(rtt_net)
    r["rtt_net_last20_med_s"] = med(rtt_net[-20:])
    r["ack_hold_med_s"] = med(hold)
    r["ack_hold_p95_s"] = pct(hold, 0.95)
    r["ack_ranges_med"] = med([p.get("ranges") for p in b_out if p["type"] == "ack"])

    # ── kopyalar: faydalı / gereksiz / kayıp; gereksizin zamanlaması ───────────────────────
    n_cp = n_cp_useful = n_cp_red_arr = n_cp_lost = n_cp_after_arr = n_cp_after_conf = 0
    b_cp_useful = b_cp_red = 0
    for u, lst in txs.items():
        fa = first_arr.get(u, (None, None))[0]
        if fa is None and u in rec_t:
            fa = rec_t[u]
        tconf = min((cov_a[p["pn"]] for p in lst if p["pn"] in cov_a), default=None)
        for p in lst[1:]:
            n_cp += 1
            if p["pn"] not in b_in_t:
                n_cp_lost += 1
            elif p["pn"] in useful_pn:
                n_cp_useful += 1
                b_cp_useful += p["l2"]
            else:
                n_cp_red_arr += 1
                b_cp_red += p["l2"]
            if fa is not None and p["t"] > fa:
                n_cp_after_arr += 1
                if tconf is not None and p["t"] > tconf:
                    n_cp_after_conf += 1
    r.update({"cp_tx": n_cp, "cp_useful": n_cp_useful, "cp_redund_arr": n_cp_red_arr,
              "cp_lost": n_cp_lost, "cp_sent_after_arrival": n_cp_after_arr,
              "cp_sent_after_conf": n_cp_after_conf, "cp_useful_l2B": b_cp_useful,
              "cp_redund_l2B": b_cp_red})
    orig_red = sum(1 for p in b_in if p["type"] in ("data", "data_frag")
                   and p["pn"] in first_tx_pn and p["pn"] not in useful_pn)
    r["orig_redund_arr"] = orig_red

    # ── mesaj zaman ayrıştırması ───────────────────────────────────────────────────────────
    s_data = g["s_data_l2"]
    ser = s_data / R
    ideal_end = -1e18
    ow, sw, rw, so, qx, exc, phys_lb = [], [], [], [], [], [], []
    late_iv = []
    for i in sorted(offer):
        t_off = offer[i]
        ideal_start = max(t_off, ideal_end)
        ideal_end = ideal_start + ser
        lst = txs.get(i)
        if not lst:
            continue
        fa = first_arr.get(i)
        if fa is None:
            continue
        t_arr, upn = fa
        t_dtx = aout_t[upn]
        n_lost_before = sum(1 for p in lst if p["t"] < t_dtx and p["pn"] not in b_in_t)
        lb = (ideal_end - t_off) + d_nom + n_lost_before * rtt_phys
        ow.append(t_arr - t_off)
        sw.append(lst[0]["t"] - t_off)
        rw.append(t_dtx - lst[0]["t"])
        so.append(t_arr - t_dtx)
        qx.append((t_arr - t_dtx) - ser - d_nom)
        phys_lb.append(lb)
        exc.append((t_arr - t_off) - lb)
        a = t_off + lb - ser          # ideal serileştirme başlangıcı (gecikme hattından sonra)
        b = t_arr - ser               # gerçek serileştirme başlangıcı
        if b > a:
            late_iv.append((a, b))
    r["ow_p50_s"], r["ow_p95_s"], r["ow_p99_s"] = pct(ow, .5), pct(ow, .95), pct(ow, .99)
    r["ow_n"] = len(ow)
    r["ow_deliv_D"] = (sum(1 for x in ow if x <= g["deadline_s"]) / len(offer)) if offer else None
    r["fec_recovered_units_cap"] = sum(1 for u, t in rec_t.items()
                                       if u not in first_arr or first_arr[u][0] > t)
    r["sender_wait_p50_s"], r["sender_wait_p95_s"] = pct(sw, .5), pct(sw, .95)
    r["sender_wait_max_s"] = max(sw) if sw else None
    r["recovery_wait_p95_s"] = pct(rw, .95)
    r["sojourn_p50_s"], r["sojourn_p95_s"] = pct(so, .5), pct(so, .95)
    r["queue_extra_p50_s"], r["queue_extra_p95_s"] = pct(qx, .5), pct(qx, .95)
    r["queue_extra_max_s"] = max(qx) if qx else None
    r["excess_p50_s"], r["excess_p95_s"] = pct(exc, .5), pct(exc, .95)
    r["phys_lb_p95_s"] = pct(phys_lb, .95)

    # hat meşgul aralıkları (ileri, varış düzlemi): [B_in − ser_j, B_in], sınıfa göre
    busy = {}
    for p in b_in:
        c = classify_fwd(p)
        c = {"pay_useful_orig": "useful", "pay_useful_copy": "useful",
             "pay_redund_orig": "redund", "pay_redund_copy": "redund",
             "fec_repair": "fec", "fec_source": "useful"}.get(c, "ctrl")
        busy.setdefault(c, []).append((p["t"] - p["l2"] / R, p["t"]))
    busy_m = {c: merge_iv(v) for c, v in busy.items()}
    all_busy = merge_iv([iv for v in busy.values() for iv in v])
    late = merge_iv(late_iv)
    late_tot = sum(b - a for a, b in late)
    r["late_s"] = late_tot
    if late_tot > 0:
        busy_in_late = sum(overlap_total(all_busy, a, b) for a, b in late)
        r["late_idle_frac"] = 1 - busy_in_late / late_tot
        for c, iv in busy_m.items():
            r[f"late_busy_{c}_frac"] = sum(overlap_total(iv, a, b) for a, b in late) / late_tot
    # hat doluluğu (pencere içinde) ve gönderim hızı/hat hızı pencereleri
    r["link_busy_frac"] = sum(b - a for a, b in all_busy) / win
    w = max(1.0, 10 * ser)
    nb = max(1, int(math.ceil(win / w)))
    bins = [0.0] * nb
    for p in a_out:
        k = int((p["t"] - t_lo) / w)
        if 0 <= k < nb:
            bins[k] += p["l2"]
    ratios = [b / (R * w) for b in bins]
    r["emit_ratio_p50"], r["emit_ratio_p95"] = pct(ratios, .5), pct(ratios, .95)
    r["emit_ratio_max"] = max(ratios) if ratios else None
    r["emit_over_cap_frac"] = sum(1 for x in ratios if x > 1.0) / len(ratios)
    r["emit_window_s"] = w
    # teyit gecikmesinin tel ayrıştırması: alıcı tutması + dönüş
    conf_wire = []
    for i, lst in txs.items():
        tc = min((cov_a[p["pn"]] for p in lst if p["pn"] in cov_a), default=None)
        if tc is not None and i in offer:
            conf_wire.append(tc - offer[i])
    r["conf_wire_p95_s"] = pct(conf_wire, .95)
    r["jitter_sigma_s"] = jit
    return r


# ════════════════════════════════════════════════════════════════════════════════════════════
# TCP yakalama analizi
# ════════════════════════════════════════════════════════════════════════════════════════════


def tcp_capture(rep, g, ev):
    cli = read_cap(rep / "capb_cli.csv.gz")
    srv = read_cap(rep / "capb_srv.csv.gz")
    if cli is None or srv is None:
        return {"cap": "yok"}
    port = server_port(rep)
    R = g["rate_Bps"]
    F = 12 + g["payload"]

    def seg(rows, direction, to_server):
        out = []
        for t, dr, pr, ipl, sp, dp, x in rows:
            if pr != 6 or dr != direction or not x:
                continue
            if port is not None and (dp if to_server else sp) != port:
                continue
            try:
                seq, ack, flags, doff, plen = (int(v) for v in x.split(":"))
            except ValueError:
                continue
            out.append({"t": t / 1e9, "seq": seq, "ack": ack, "flags": flags, "doff": doff,
                        "plen": plen, "l2": ipl + 14, "hdr": ipl + 14 - plen})
        return out

    a_out, b_in = seg(cli, "o", True), seg(srv, "i", True)
    b_out, a_in = seg(srv, "o", False), seg(cli, "i", False)
    r = {"cap": "var"}
    syn = next((s for s in a_out if s["flags"] & 0x02), None)
    if syn is None:
        r["cap"] = "syn-yok"
        return r
    isn = syn["seq"]

    def rel(s):
        return (s["seq"] - isn - 1) % (1 << 32)

    def acc(plane, prefix, payload_cls):
        # Sütunların toplamı = toplam tel baytı. Yük taşıyan segmentte başlıklar (Eth+IP+TCP+seçenek)
        # l234'e, yük sınıflarına; yalın ACK ve SYN/FIN segmentinin BÜTÜN baytı kendi sınıfına.
        a = {}
        for s in plane:
            a[prefix + "pkts"] = a.get(prefix + "pkts", 0) + 1
            a[prefix + "total"] = a.get(prefix + "total", 0) + s["l2"]
            if s["plen"] == 0:
                key = "syn_fin" if s["flags"] & 0x07 else "ack"
                a[prefix + key] = a.get(prefix + key, 0) + s["l2"]
                a[prefix + key + "_pkts"] = a.get(prefix + key + "_pkts", 0) + 1
                if key == "ack":
                    a[prefix + "ackpkt_l2"] = a.get(prefix + "ackpkt_l2", 0) + s["l2"]
            else:
                a[prefix + "l234"] = a.get(prefix + "l234", 0) + s["hdr"]
                for k, v in payload_cls(s).items():
                    a[prefix + k] = a.get(prefix + k, 0) + v
        return a

    # ileri: B_in'de ilk kez gelen bayt faydalı, tekrar gelen gereksiz (bit eşlem)
    total_len = F * g["msgs"] + 4096
    got = bytearray(total_len)
    rcv_nxt = 0
    inorder_t = {}

    def fwd_arrival(s):
        nonlocal rcv_nxt
        lo = rel(s)
        hi = lo + s["plen"]
        new = 0
        for k in range(lo, min(hi, total_len)):
            if not got[k]:
                got[k] = 1
                new += 1
        while rcv_nxt < total_len and got[rcv_nxt]:
            rcv_nxt += 1
        m_done = rcv_nxt // F
        for i in range(len(inorder_t), m_done):
            inorder_t[i] = s["t"]
        nm = min(new, s["plen"])
        # çerçeve başlığı payı (12/F) faydalı baytın içinden ayrılır
        fr = nm * 12 / F
        return {"pay_useful": nm - fr, "frame_hdr": fr, "pay_redund": s["plen"] - nm}

    for s in b_in:
        if s["plen"] > 0:
            s["_cls"] = fwd_arrival(s)
    r.update(acc(b_in, "wf_", lambda s: s.get("_cls", {})))
    sent_max = 0
    retx_B = 0
    retx_n = 0

    def fwd_attempt(s):
        nonlocal sent_max, retx_B, retx_n
        lo, hi = rel(s), rel(s) + s["plen"]
        new = max(0, hi - max(lo, sent_max))
        if new < s["plen"]:
            retx_B += s["plen"] - new
            retx_n += 1
        sent_max = max(sent_max, hi)
        return {"pay_new": new, "pay_retx": s["plen"] - new}

    r.update(acc(a_out, "wfa_", fwd_attempt))
    r["tcp_retx_segs"], r["tcp_retx_B"] = retx_n, retx_B
    r.update(acc(a_in, "wr_", lambda s: {"app_ack": s["plen"]}))
    r.update(acc(b_out, "wra_", lambda s: {"app_ack": s["plen"]}))
    # tek yön (sıralı teslim) gecikmesi; sunum zamanı = ilk veri segmenti hizası
    first_data = next((s for s in a_out if s["plen"] > 0), None)
    if first_data is not None and ev and ev[0].get("sent_ns") is not None:
        offset = first_data["t"] - ev[0]["sent_ns"] / 1e9
        ow = [inorder_t[e["msg_id"]] - (offset + e["sent_ns"] / 1e9) for e in ev
              if e["msg_id"] in inorder_t]
        r["ow_p50_s"], r["ow_p95_s"], r["ow_p99_s"] = pct(ow, .5), pct(ow, .95), pct(ow, .99)
        r["ow_n"] = len(ow)
        t_lo = offset
    else:
        t_lo = a_out[0]["t"]
    t_hi = max([s["t"] for s in b_in + a_in] + [t_lo])
    win = max(t_hi - t_lo, 1e-9)
    r["win_s"] = win
    r["wf_util"] = r.get("wf_total", 0) / (R * win)
    r["wr_util"] = r.get("wr_total", 0) / (R * win)
    r["wfa_offered_load"] = r.get("wfa_total", 0) / (R * win)
    r["wf_lost_pkts"] = max(0, len(a_out) - len(b_in))
    r["wr_lost_pkts"] = max(0, len(b_out) - len(a_in))
    return r


# ════════════════════════════════════════════════════════════════════════════════════════════
# QUIC yakalama analizi (şifreli: yalnız paket/bayt ve uzun/kısa başlık ayrımı)
# ════════════════════════════════════════════════════════════════════════════════════════════


def quic_capture(rep, g, ev):
    cli = read_cap(rep / "capb_cli.csv.gz")
    srv = read_cap(rep / "capb_srv.csv.gz")
    if cli is None or srv is None:
        return {"cap": "yok"}
    port = server_port(rep)
    R = g["rate_Bps"]

    def plane(rows, direction, to_server, prefix):
        a = {}
        ts = []
        for t, dr, pr, ipl, sp, dp, x in rows:
            if pr != 17 or dr != direction:
                continue
            if port is not None and (dp if to_server else sp) != port:
                continue
            ts.append(t / 1e9)
            long_hdr = bool(x) and int(x[:2], 16) & 0x80
            a[prefix + "pkts"] = a.get(prefix + "pkts", 0) + 1
            a[prefix + "total"] = a.get(prefix + "total", 0) + ipl + 14
            a[prefix + "l234"] = a.get(prefix + "l234", 0) + 42
            k = prefix + ("quic_long" if long_hdr else "quic_short")
            a[k] = a.get(k, 0) + ipl - 28
        return a, ts

    r = {"cap": "var"}
    for rows, d, ts_, pre in ((srv, "i", True, "wf_"), (cli, "o", True, "wfa_"),
                              (cli, "i", False, "wr_"), (srv, "o", False, "wra_")):
        a, ts = plane(rows, d, ts_, pre)
        r.update(a)
        r[pre + "t"] = ts
    delivered = sum(1 for e in ev if e.get("delivered_ns") is not None)
    r["wf_pay_useful"] = delivered * g["payload"]
    allt = r.pop("wf_t") + r.pop("wr_t") + r.pop("wfa_t") + r.pop("wra_t")
    win = (max(allt) - min(allt)) if allt else 0
    win = max(win, 1e-9)
    r["win_s"] = win
    r["wf_util"] = r.get("wf_total", 0) / (R * win)
    r["wr_util"] = r.get("wr_total", 0) / (R * win)
    r["wfa_offered_load"] = r.get("wfa_total", 0) / (R * win)
    r["wf_lost_pkts"] = r.get("wfa_pkts", 0) - r.get("wf_pkts", 0)
    r["wr_lost_pkts"] = r.get("wra_pkts", 0) - r.get("wr_pkts", 0)
    return r


# ════════════════════════════════════════════════════════════════════════════════════════════
# Koşu
# ════════════════════════════════════════════════════════════════════════════════════════════


def analyze_run(rep, g, arm):
    r = {"cell": g["cell"], "arm": arm, "rep": int(rep.name[3:])}
    rc = read_text(rep / "rc").strip()
    r["rc"] = int(rc) if rc.lstrip("-").isdigit() else None
    ts, te = read_float(rep / "t_start"), read_float(rep / "t_end")
    r["run_wall_s"] = (te - ts) if ts and te else None
    meta, ev = read_events(rep)
    r["has_events"] = bool(ev)
    if not ev:
        return r
    D = g["deadline_s"]
    lat = [(e["delivered_ns"] - e["sent_ns"]) / 1e9 for e in ev if e.get("delivered_ns") is not None]
    n = len(ev)
    r["n_msgs"] = n
    r["deliv_D"] = sum(1 for x in lat if x <= D) / n
    for dd in ALT_DEADLINES_S:
        if dd <= D:
            r[f"deliv_{dd}s"] = sum(1 for x in lat if x <= dd) / n
    r["deliv_any"] = len(lat) / n
    r["conf_p50_s"], r["conf_p95_s"], r["conf_p99_s"] = pct(lat, .5), pct(lat, .95), pct(lat, .99)
    r["conf_max_s"] = max(lat) if lat else None
    if meta:
        r["setup_s"] = meta.get("setup_ns", 0) / 1e9
        r["transport"] = meta.get("transport")
        r["profile_id"] = meta.get("profile_id")
        r["udp_tx_B"], r["udp_rx_B"] = meta.get("udp_tx_bytes"), meta.get("udp_rx_bytes")
        b = meta.get("budget") or {}
        r["cli_inj_ip_B"], r["cli_inj_pkts"] = b.get("injected_bytes"), b.get("injected_pkts")
    r.update(parse_client_err(read_text(rep / "c.err")))
    r.update(parse_server_err(read_text(rep / "s.err")))
    r.update(parse_realized(rep))
    win = r.get("run_wall_s")
    if win:
        for dname in ("fwd", "rev"):
            if f"q_{dname}_bytes" in r:
                r[f"q_{dname}_util_wall"] = r[f"q_{dname}_bytes"] / (g["rate_Bps"] * win)
    # srtt — gerçek RTT kıyası (yalnız Kama kolları sayaç verir)
    if r.get("est_srtt") is not None:
        r["srtt_s"] = r["est_srtt"] / 1e6
        r["srtt_over_rttphys"] = r["srtt_s"] / g["rtt_phys_s"]
        r["srtt_over_rttphys16"] = r["srtt_s"] / g["rtt_phys_r16_s"]
        r["srtt_over_rttfifo"] = r["srtt_s"] / g["rtt_fifo_s"]
    if r.get("est_rtprop") is not None:
        r["rtprop_s"] = r["est_rtprop"] / 1e6
    if r.get("est_pace") is not None:
        # motor bayt birimi = mühürlü UDP yükü (runtime.rs wire_len); hat hızının veri payına çevrimi
        udp_share = (g["s_data_l2"] - 42) / g["s_data_l2"]
        r["pace_over_link"] = r["est_pace"] / (g["rate_Bps"] * udp_share)
        if r.get("est_btlbw") is not None:
            r["btlbw_over_link"] = r["est_btlbw"] / (g["rate_Bps"] * udp_share)
    # kapasite reddi (tanım: bkz. RAPOR-B §4.4)
    rej_keys = ("cli_refused_pkts", "srv_egress_refused_pkts", "srv_delivery_dropped",
                "srv_dropped_new_peers", "srv_frag_chunk_dropped", "srv_msg_asm_evicted",
                "srv_fec_blocks_evicted", "srv_fec_pre_param_dropped", "parity_denied")
    if any(k in r for k in rej_keys):
        r["cap_reject_total"] = sum(int(r.get(k) or 0) for k in rej_keys)
    tr = r.get("transport") or ""
    if tr.startswith("kama"):
        cap = kama_capture(rep, g, meta, ev)
    elif tr.startswith("tcp"):
        cap = tcp_capture(rep, g, ev)
    elif tr.startswith("quinn"):
        cap = quic_capture(rep, g, ev)
    else:
        cap = {"cap": "bilinmeyen-taşıyıcı"}
    r.update(cap)
    if r.get("srtt_s") and r.get("rtt_net_last20_med_s"):
        r["srtt_over_rttnet"] = r["srtt_s"] / r["rtt_net_last20_med_s"]
    # faydalı yük payı (ileri, hattan geçen) — sınıf sütunları + kapasiteye oran
    tot = r.get("wf_total")
    if tot:
        useful = (r.get("wf_pay_useful_orig", 0) + r.get("wf_pay_useful_copy", 0)
                  + r.get("wf_pay_useful", 0))
        r["wf_useful_B"] = useful
        r["wf_useful_share"] = useful / tot
        if r.get("wf_cap_B"):
            r["wf_useful_of_cap"] = useful / r["wf_cap_B"]
        elif r.get("win_s"):
            r["wf_useful_of_cap"] = useful / (g["rate_Bps"] * r["win_s"])
    if r.get("win_s") and r.get("wf_useful_B") is not None:
        r["goodput_Bps"] = r["wf_useful_B"] / r["win_s"]
    return r


# ════════════════════════════════════════════════════════════════════════════════════════════
# Özet
# ════════════════════════════════════════════════════════════════════════════════════════════

OZET_METRICS = [
    ("deliv_D", "teslim oranı (D içinde) [olay]"),
    ("ow_deliv_D", "alıcıda varış oranı (D içinde) [yakalama]"),
    ("conf_p50_s", "gönderen-teyit p50 (s) [olay]"),
    ("conf_p95_s", "gönderen-teyit p95 (s) [olay]"),
    ("conf_p99_s", "gönderen-teyit p99 (s) [olay]"),
    ("ow_p50_s", "tek yön p50 (s) [yakalama]"),
    ("ow_p95_s", "tek yön p95 (s) [yakalama]"),
    ("ow_p99_s", "tek yön p99 (s) [yakalama]"),
    ("wf_useful_share", "ileri hattan geçenin faydalı yük payı [yakalama]"),
    ("wf_useful_of_cap", "ileri kapasitenin faydalı yük payı [yakalama]"),
    ("wf_util", "ileri hat doluluğu [yakalama]"),
    ("wfa_offered_load", "ileri deneme yükü / hat [yakalama]"),
    ("wr_util", "dönüş hat doluluğu [yakalama]"),
    ("goodput_Bps", "goodput (B/s) [yakalama]"),
    ("cp_tx", "kopya (yeniden gönderim + çoğaltma + prob) sayısı [yakalama]"),
    ("cp_useful", "teslime katkı veren kopya [yakalama]"),
    ("cp_redund_arr", "gereksiz varan kopya [yakalama]"),
    ("cp_sent_after_arrival", "alıcıda olan birime gönderilen kopya [yakalama]"),
    ("cp_sent_after_conf", "teyit geldikten sonra gönderilen kopya [yakalama]"),
    ("tcp_retx_B", "TCP yeniden gönderim baytı [yakalama]"),
    ("srtt_s", "motor srtt (s, kapanış) [sayaç]"),
    ("srtt_over_rttphys", "srtt / RTT_fiz (profil) [sayaç+model]"),
    ("srtt_over_rttfifo", "srtt / RTT_fifo (netem sıra-koruyan jitter) [sayaç+model]"),
    ("srtt_over_rttnet", "srtt / ölçülen ağ RTT (son 20) [sayaç+yakalama]"),
    ("rtt_samples", "RTT örneği sayısı [sayaç]"),
    ("rtt_net_last20_med_s", "ölçülen ağ RTT, son 20 orijinal (s) [yakalama]"),
    ("ack_hold_med_s", "alıcı ACK tutması medyanı (s) [yakalama]"),
    ("ack_hold_p95_s", "alıcı ACK tutması p95 (s) [yakalama]"),
    ("ack_ranges_med", "ACK aralık sayısı medyanı [yakalama]"),
    ("pace_over_link", "pacer hızı / hattın veri payı [sayaç]"),
    ("emit_ratio_p95", "gönderim hızı / hat, pencere p95 [yakalama]"),
    ("emit_over_cap_frac", "hattı aşan pencere oranı [yakalama]"),
    ("sender_wait_p95_s", "gönderende ilk iletim bekleme p95 (s) [yakalama]"),
    ("queue_extra_p95_s", "darboğaz kuyruk fazlası p95 (s) [yakalama]"),
    ("excess_p95_s", "ideal-ARQ sınırı üstü fazla p95 (s) [yakalama]"),
    ("late_idle_frac", "geç kalma süresinde hat BOŞ payı (zamanlayıcı) [yakalama]"),
    ("late_busy_useful_frac", "geç kalma süresinde hat faydalı yükle dolu [yakalama]"),
    ("late_busy_redund_frac", "geç kalma süresinde hat gereksiz kopyayla dolu [yakalama]"),
    ("cap_reject_total", "kapasite reddi toplamı [sayaç]"),
    ("tail_probes", "kuyruk probu (motor) [sayaç]"),
    ("establish_tx", "kuruluş probu [sayaç]"),
    ("parity", "FEC parite emisyonu [sayaç]"),
    ("setup_s", "kurulum süresi (s) [olay]"),
]

# Sütun toplamı = "toplam" (ackpkt_l2 hariç: o, ACK paketlerinin bütün tel baytını AYRICA verir).
CLASS_COLS_FWD = ["l234", "env", "hdr", "pay_useful_orig", "pay_useful_copy", "pay_redund_orig",
                  "pay_redund_copy", "fec_repair", "fec_source", "probe", "ctrl", "other",
                  "pay_useful", "frame_hdr", "pay_redund", "ack", "syn_fin", "quic_long", "quic_short",
                  "ackpkt_l2"]
CLASS_COLS_REV = ["l234", "env", "hdr", "ack", "recovered", "ctrl", "probe", "other", "app_ack",
                  "syn_fin", "quic_long", "quic_short", "ackpkt_l2"]


def stats(vals):
    v = sorted(x for x in vals if isinstance(x, (int, float)) and not isinstance(x, bool))
    if not v:
        return None
    return {"n": len(v), "med": pct(v, .5), "q1": pct(v, .25), "q3": pct(v, .75),
            "min": v[0], "max": v[-1]}


def fmt(x, nd=3):
    if x is None:
        return "–"
    if isinstance(x, float):
        if abs(x) >= 1000:
            return f"{x:.0f}"
        return f"{x:.{nd}f}"
    return str(x)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("root")
    ap.add_argument("--out")
    ap.add_argument("--hucre", default="b_")
    a = ap.parse_args()
    root = pathlib.Path(a.root)
    out = pathlib.Path(a.out) if a.out else root / "analiz_b"
    out.mkdir(parents=True, exist_ok=True)
    geoms = fizik.by_cell()
    rows = []
    cells_seen = []
    for cd in sorted(p for p in root.iterdir() if p.is_dir() and p.name.startswith(a.hucre)):
        g = geoms.get(cd.name)
        if g is None:
            print(f"atla {cd.name}: fizik.py ızgarasında yok", file=sys.stderr)
            continue
        cells_seen.append((cd.name, (cd / "DONE").exists()))
        for armd in sorted(p for p in cd.iterdir() if p.is_dir()):
            for rep in sorted(armd.glob("rep*"), key=lambda q: int(q.name[3:])):
                try:
                    rows.append(analyze_run(rep, g, armd.name))
                except Exception as e:  # analiz hatası koşuyu düşürmez, raporlanır
                    rows.append({"cell": cd.name, "arm": armd.name, "rep": int(rep.name[3:]),
                                 "analiz_hatasi": f"{type(e).__name__}: {e}"})
    keys = []
    for r in rows:
        for k in r:
            if k not in keys:
                keys.append(k)
    with open(out / "kosular_b.csv", "w", newline="") as f:
        w = csv.DictWriter(f, fieldnames=keys)
        w.writeheader()
        for r in rows:
            w.writerow({k: (json.dumps(v) if isinstance(v, (list, dict)) else v) for k, v in r.items()})
    # hücre × kol × ölçü
    groups = {}
    for r in rows:
        groups.setdefault((r["cell"], r["arm"]), []).append(r)
    num_keys = [k for k in keys if any(isinstance(r.get(k), (int, float)) and not isinstance(r.get(k), bool)
                                       for r in rows)]
    with open(out / "hucre_b.csv", "w", newline="") as f:
        w = csv.writer(f)
        w.writerow(["cell", "arm", "metric", "n", "median", "q1", "q3", "min", "max"])
        for (c, arm), lst in sorted(groups.items()):
            for k in num_keys:
                s = stats([r.get(k) for r in lst])
                if s:
                    w.writerow([c, arm, k, s["n"], s["med"], s["q1"], s["q3"], s["min"], s["max"]])
    # OZET-B.md
    arms_order = ["taban", "aday", "quinn", "tcp"]
    L = ["# KTP2-F0 / B — ölçüm özeti (otomatik; analiz_b.py)", "",
         f"Kök: `{root}` · koşu satırı: {len(rows)} · hücre: {len(cells_seen)}", "",
         "Her hücrede: medyan [Q1–Q3] (min–maks), N = koşu. N < 30 → betimsel.", ""]
    for c, done in cells_seen:
        g = geoms[c]
        L.append(f"## {c} — {g['prio']} {g['kind']}{'' if done else ' (DONE YOK: yarım)'}")
        L.append(f"`{g['netem']}` · M={g['msgs']} × {g['payload']} B @ {g['ival_ms']} ms · D={g['deadline_s']} s · "
                 f"ρ_ileri={g['rho_fwd']:.3f} · RTT_fiz={g['rtt_phys_s']:.3f}/{g['rtt_phys_r16_s']:.3f} s · "
                 f"RTT_fifo={g['rtt_fifo_s']:.3f} s · "
                 f"tek yön alt sınır={g['oneway_lb_s']:.3f} s · ideal ARQ p95 tek yön={g['ideal_arq_oneway_p95_s']:.2f} s")
        L.append("")
        hdr = "| ölçü | " + " | ".join(arms_order) + " |"
        L += [hdr, "|---|" + "---|" * len(arms_order)]
        for k, label in OZET_METRICS:
            cells_txt = []
            anyv = False
            for arm in arms_order:
                s = stats([r.get(k) for r in groups.get((c, arm), [])])
                if s:
                    anyv = True
                    cells_txt.append(f"{fmt(s['med'])} [{fmt(s['q1'])}–{fmt(s['q3'])}] ({fmt(s['min'])}–{fmt(s['max'])}) N={s['n']}")
                else:
                    cells_txt.append("–")
            if anyv:
                L.append(f"| {label} | " + " | ".join(cells_txt) + " |")
        # sınıf sütunları (medyan bayt, ileri/dönüş)
        for pre, cols, title in (("wf_", CLASS_COLS_FWD, "ileri, hattan geçen (medyan bayt)"),
                                 ("wr_", CLASS_COLS_REV, "dönüş, hattan geçen (medyan bayt)")):
            present = [col for col in cols if any(r.get(pre + col) for arm in arms_order
                                                   for r in groups.get((c, arm), []))]
            if not present:
                continue
            L.append("")
            L.append(f"**Tel sınıfları — {title}**")
            L.append("")
            L.append("| kol | toplam | " + " | ".join(present) + " |")
            L.append("|---|---|" + "---|" * len(present))
            for arm in arms_order:
                lst = groups.get((c, arm), [])
                if not lst:
                    continue
                tot = med([r.get(pre + "total") for r in lst])
                vals = [med([r.get(pre + col, 0) if r.get(pre + "total") else None for r in lst]) for col in present]
                L.append(f"| {arm} | {fmt(tot, 0)} | " + " | ".join(
                    f"{fmt(v, 0)} ({(v / tot * 100) if (v is not None and tot) else 0:.1f}%)" for v in vals) + " |")
        bad = [r for r in rows if r["cell"] == c and (r.get("rc") not in (0,) or not r.get("has_events"))]
        if bad:
            L.append("")
            L.append("Düşen/olaysız koşular: " + ", ".join(f"{r['arm']}/rep{r['rep']} rc={r.get('rc')}" for r in bad))
        L.append("")
    (out / "OZET-B.md").write_text("\n".join(L) + "\n")
    errs = [r for r in rows if "analiz_hatasi" in r]
    print(f"{len(rows)} koşu · {len(cells_seen)} hücre · analiz hatası {len(errs)} → {out}")
    for r in errs[:10]:
        print(f"  {r['cell']}/{r['arm']}/rep{r['rep']}: {r['analiz_hatasi']}", file=sys.stderr)


if __name__ == "__main__":
    main()
