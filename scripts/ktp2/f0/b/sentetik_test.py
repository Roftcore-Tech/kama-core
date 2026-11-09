#!/usr/bin/env python3
"""[KTP2-F0 / Ajan B] analiz_b.py'nin elle kurulmuş sentetik koşuyla denetimi (koşu YOK).

Senaryo (hücre b_r1k_d300_l20_p65: R = 1000 B/s, d = 0,3 s, veri paketi 134 B → ser 0,134 s):
  m0 pn0  t=0,001 → B 0,435
  m1 pn1  t=0,451 → KAYIP
  m2 pn2  t=0,901 → B 1,335            ACK#0 {0,2}       B_out 1,336 → A 1,709
  m1 pn3  t=1,200 (yeniden) → B 1,634  ACK#1 {0,2,3}     B_out 1,635 → A 2,008
  m1 pn4  t=1,300 (kopya)   → B 1,768  (gereksiz varış: m1 zaten 1,634'te geldi)
  m3 pn5  t=1,351 → B 1,902 (pn4'ün arkasında kuyruk)    ACK#2 {0,2,3,4,5} B_out 1,903 → A 2,276
Beklenen: kopya 2 (faydalı 1, gereksiz varış 1), ileri kayıp 1, m1 kurtarma bekleme 0,749 s.
Kullanım: python3 -I sentetik_test.py   (geçici dizinde kurar, analiz eder, doğrular)
"""
import csv
import gzip
import json
import pathlib
import subprocess
import sys
import tempfile

HERE = pathlib.Path(__file__).resolve().parent
sys.dont_write_bytecode = True
T0 = 1_800_000_000.0
PORT = 23456
CPORT = 40000


def seal(frame, pn):
    return frame + pn.to_bytes(8, "big") + bytes(8)


def data(pn, unit, pay=65):
    return seal(bytes([0x00]) + (pn & 0xFFFF).to_bytes(2, "big") + unit.to_bytes(8, "big") + bytes([0x5A]) * pay, pn)


def ack(spn, pns, delay_us=0):
    v = sorted(set(pns))
    blocks = []
    lo = hi = v[0]
    for x in v[1:]:
        if x == hi + 1:
            hi = x
        else:
            blocks.append((lo, hi))
            lo = hi = x
    blocks.append((lo, hi))
    b0lo, b0hi = blocks[-1]
    body = bytes([0x14, 0]) + (b0hi & 0xFFFF).to_bytes(2, "big") + (delay_us // 256).to_bytes(2, "big")
    body += (b0hi - b0lo + 1).to_bytes(2, "big")
    ranges = []
    cursor = b0lo
    for lo, hi in reversed(blocks[:-1]):
        ranges.append(((cursor - hi) & 0xFFFF, hi - lo + 1))
        cursor = lo - 1
    body += bytes([len(ranges)])
    for gp, ln in ranges:
        body += gp.to_bytes(2, "big") + ln.to_bytes(2, "big")
    body += (0).to_bytes(4, "big") + (0xFFFF).to_bytes(2, "big")
    return seal(body, spn)


def line(t, d, payload, sport, dport):
    x = payload.hex() if len(payload) <= 200 else payload[:40].hex() + ":" + payload[-16:].hex()
    return f"{int((T0 + t) * 1e9)},{d},17,{len(payload) + 28},{sport},{dport},{x}\n"


def build(root):
    cell = root / "b_r1k_d300_l20_p65"
    rep = cell / "taban" / "rep1"
    rep.mkdir(parents=True)
    (cell / "DONE").write_text("OK\n")
    (rep / "cmd.txt").write_text(f"SRV: x serve --bind 10.10.1.1:{PORT} --transport kama-product\nCLI: y\n")
    (rep / "rc").write_text("0\n")
    (rep / "t_start").write_text(f"{T0 - 0.1:.9f}\n")
    (rep / "t_end").write_text(f"{T0 + 2.5:.9f}\n")
    fwd = [(0.001, 0, 0, 0.435), (0.451, 1, 1, None), (0.901, 2, 2, 1.335),
           (1.200, 3, 1, 1.634), (1.300, 4, 1, 1.768), (1.351, 5, 3, 1.902)]
    acks = [(1.336, 0, [0, 2], 1.709), (1.635, 1, [0, 2, 3], 2.008), (1.903, 2, [0, 2, 3, 4, 5], 2.276)]
    cli, srv = [], []
    for t, pn, u, tb in fwd:
        pkt = data(pn, u)
        cli.append((t, line(t, "o", pkt, CPORT, PORT)))
        if tb is not None:
            srv.append((tb, line(tb, "i", pkt, CPORT, PORT)))
    for tb, spn, pns, ta in acks:
        pkt = ack(spn, pns)
        srv.append((tb, line(tb, "o", pkt, PORT, CPORT)))
        cli.append((ta, line(ta, "i", pkt, PORT, CPORT)))
    for name, rows in (("capb_cli.csv.gz", cli), ("capb_srv.csv.gz", srv)):
        with gzip.open(rep / name, "wt") as f:
            for _, ln in sorted(rows):
                f.write(ln)
    t0_ns = int(T0 * 1e9)
    off = 1_000_000
    offers = [0.0, 0.45, 0.9, 1.35]
    conf = [1.709, 2.008, 1.709, 2.276]
    meta = {"meta": {"transport": "kama-product", "t0_unix_ns": t0_ns, "events_epoch_offset_ns": off,
                     "setup_ns": 1000, "profile_id": "kama-v1-full", "udp_tx_bytes": 0, "udp_rx_bytes": 0,
                     "budget": {"injected_bytes": 0, "injected_pkts": 6}}}
    with open(rep / "out.jsonl", "w") as f:
        f.write(json.dumps(meta) + "\n")
        for i, (o, c) in enumerate(zip(offers, conf)):
            f.write(json.dumps({"event": {"msg_id": i, "sent_ns": off + int(o * 1e9),
                                          "delivered_ns": off + int(c * 1e9), "payload_bytes": 65}}) + "\n")
    (rep / "c.err").write_text(
        "KP-MANIFEST role=client profile=kama-v1-full features=[diag-sched-telemetry=false]\n"
        "KP-CLIENT delivered=4/4 tx=552B(data=368 retx=92 dup=92 fec=0 ctrl=0) rx=93B refused=0B/0pkt "
        "stale=0 establish_tx=0 state=Idle\n"
        "KP-ACTIVATED arq_retx=92B rtt_samples=3 data_denied=0 retx_denied=0 spurious=0 gate_opened=0 "
        "return_path=[bw=10B/s ack_int=300ms ret_ack_loss=0‰ fwd_loss=200‰]\n"
        "KP-ACTIVATION cfg=[tail=true fec=true] fired=[tail_probe=0@0ms tail_cancel=0 parity=0 parity_denied=0 "
        "retx=1 dup=1] reach=[transitions=3 recover_arq=1 recover_fwd=0 probe_up=0 left_preconv=900ms] "
        "est=[btlbw=500B/s srtt=900000us rtprop=800000us qdelay=0us pace=600B/s tokens=0 inflight=0B]\n")
    # alıcı damgası (bench serve --rx-log, şema ktp2-c-rx-v1): birim = mesaj, an = B varışı + 1 ms
    with open(rep / "rx_log.jsonl", "w") as f:
        f.write(json.dumps({"rx_log_schema": "ktp2-c-rx-v1", "dropped": 0}) + "\n")
        for u, tb in ((0, 0.435), (2, 1.335), (1, 1.634), (3, 1.902)):
            f.write(json.dumps({"u": u, "t": int((T0 + tb + 0.001) * 1e9), "n": 65}) + "\n")
    (rep / "s.err").write_text("KP-SERVER delivered=4 t=3.0s stats=ServerStats { delivered: 4, delivery_dropped: 0, "
                               "rx_datagrams: 5, tx_datagrams: 3, tx_bytes: 93 }\n")
    return cell


def main():
    with tempfile.TemporaryDirectory() as td:
        root = pathlib.Path(td)
        build(root)
        subprocess.run([sys.executable, "-I", str(HERE / "analiz_b.py"), str(root)], check=True)
        rows = list(csv.DictReader(open(root / "analiz_b" / "kosular_b.csv")))
        assert len(rows) == 1, rows
        r = rows[0]
        if r.get("analiz_hatasi"):
            raise SystemExit(f"analiz hatası: {r['analiz_hatasi']}")
        f = lambda k: float(r[k])  # noqa: E731
        checks = {
            "cp_tx": 2, "cp_useful": 1, "cp_redund_arr": 1, "cp_lost": 0, "wf_lost_pkts": 1,
            "cp_sent_after_arrival": 0, "wf_pkts": 5, "wr_pkts": 3, "ow_n": 4,
            "wf_pay_useful_orig": 3 * 65, "wf_pay_useful_copy": 65, "wf_pay_redund_copy": 65,
            "wf_l234": 5 * 42, "wf_env": 5 * 16, "wf_hdr": 5 * 11, "wr_ack": 3 * (15 + 4),
            "deliv_D": 1.0,
        }
        bad = []
        for k, v in checks.items():
            got = f(k) if r.get(k) not in (None, "") else None
            if got is None or abs(got - v) > 1e-6:
                bad.append(f"{k}: beklenen {v}, bulunan {r.get(k)}")
        # m1: ilk iletim 0,451, teslim eden kopya 1,200 → kurtarma bekleme 0,749 (p95 4 mesajda)
        approx = {"ow_p50_s": None, "rtt_wire_med_s": None}
        ow = sorted([0.435 - 0.0, 1.634 - 0.45, 1.335 - 0.9, 1.902 - 1.35])
        assert abs(f("ow_p50_s") - (ow[1] + ow[2]) / 2) < 2e-3, (r["ow_p50_s"], ow)
        # orijinal veri RTT: pn0 1,709−0,001; pn2 1,709−0,901; pn5 2,276−1,351 → medyan 0,925
        assert abs(f("rtt_wire_med_s") - 0.925) < 2e-3, r["rtt_wire_med_s"]
        # ağ RTT: pn0 (0,434 + 0,373) = 0,807, pn2 0,807, pn5 (0,551 + 0,373) = 0,924 → medyan 0,807
        assert abs(f("rtt_net_med_s") - 0.807) < 2e-3, r["rtt_net_med_s"]
        # alıcı tutması: pn0 1,336−0,435 = 0,901; pn2 0,001; pn5 0,001 → medyan 0,001
        assert abs(f("ack_hold_med_s") - 0.001) < 2e-3, r["ack_hold_med_s"]
        assert r["data_denied"] in ("", None), "sched kapalıyken data_denied n/a olmalı"
        # rx_log tek yön = yakalama tek yönü + 1 ms
        assert abs(f("rx_ow_p50_s") - (f("ow_p50_s") + 0.001)) < 2e-3, (r["rx_ow_p50_s"], r["ow_p50_s"])
        assert abs(f("rx_deliv_D") - 1.0) < 1e-9 and r["birikim_kaynak"] == "rx", r["birikim_kaynak"]
        del approx
        if bad:
            raise SystemExit("BAŞARISIZ:\n  " + "\n  ".join(bad))
        ozet = (root / "analiz_b" / "OZET-B.md").read_text()
        assert "b_r1k_d300_l20_p65" in ozet
        print("sentetik_test: GEÇTİ (", len(checks) + 7, "denetim )")


if __name__ == "__main__":
    main()
