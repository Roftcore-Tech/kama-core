#!/usr/bin/env python3
"""[KAPI-1 W8] Kampanya manifest üretici — profil şeması + tohum türetimi + dengeli randomizasyon.

Deterministik: aynı girdi → byte-eş manifest (RNG yok; tüm tohumlar SHA-256 türetimi).

Tohum uzayları (Model A kör-ayrımı, r5 J):
  calibration: seed = SHA256("KAMA-V2-CAL" ‖ cell_id ‖ rep)[:8]  — drand GEREKMEZ.
  evaluation:  seed = SHA256(drand_randomness ‖ "KAMA-V2-EVAL" ‖ cell_id ‖ rep)[:8]
               — --drand-randomness HEX verilmeden evaluation manifesti ÜRETİLMEZ (yapısal kilit:
               configs-freeze'den önce evaluation tohumu var OLAMAZ).

Dengeli randomizasyon (r5 I): her 10-rep blokta 5/5 kama-önce/karşılaştırıcı-önce; sıra cell-tohumundan.

Bütçe türetimi (r5 D, KAPI-1 §2; kanal-türevli, transport-BAĞIMSIZ):
  burst: B = C×D×u/8 · paced: refill=C×u/8, bucket=max(3×1500, 2s×refill), ceiling=C×W×u/8
  (u primary=0.75, karar D2; W = (n_msg−1)×interval + D).

Kullanım:
  kapi_manifest.py calibration --out cal.jsonl
  kapi_manifest.py evaluation  --out eval.jsonl --drand-randomness <hex>
  kapi_manifest.py profiles                       (tam şema dökümü, KAPI-2 eki)
"""

import argparse
import hashlib
import json
import sys

U_PRIMARY = 0.75  # karar D2
MTU = 1500

# ---------------------------------------------------------------------------
# Profil kataloğu — KAPI-1 §9 tablolarının kod hali (r5 F tam şema).
# Ortak alanlar aşağıda `full_schema`'da doldurulur; burada profil-özgü olanlar.
# rate birimleri: netem `rate` argümanı; capacity_bps = bit/s (bütçe türetimi için).
# loss_model: {"random": p_pct} | {"gemodel": [p,r,1-h,1-k]} | {"trace": plan_adı}
# ---------------------------------------------------------------------------
PROFILES = {
    # Grup A (v1 mirası — harness/netem-profiles.sh değerleri)
    "p1_loss50": dict(grp="A", cls="I", fwd="loss 50% rate 100kbit", rev="loss 50% rate 100kbit",
                      cap_fwd=100_000, cap_rev=100_000, loss={"random": 50}, delay_ms=0, jitter_ms=0),
    "p2_loss80": dict(grp="A", cls="I", fwd="loss 80% rate 200kbit", rev="loss 80% rate 200kbit",
                      cap_fwd=200_000, cap_rev=200_000, loss={"random": 80}, delay_ms=0, jitter_ms=0),
    "p3_loss95": dict(grp="A", cls="I", fwd="loss 95% rate 100kbit", rev="loss 95% rate 100kbit",
                      cap_fwd=100_000, cap_rev=100_000, loss={"random": 95}, delay_ms=0, jitter_ms=0),
    "p4_jitter": dict(grp="A", cls="I", fwd="delay 200ms 100ms distribution normal rate 100kbit",
                      rev="delay 200ms 100ms distribution normal rate 100kbit",
                      cap_fwd=100_000, cap_rev=100_000, loss={"random": 0}, delay_ms=200, jitter_ms=100),
    "p5_revpath": dict(grp="A", cls="I", fwd="loss 5% rate 100kbit", rev="loss 80% rate 100kbit",
                       cap_fwd=100_000, cap_rev=100_000, loss={"random": 5}, delay_ms=0, jitter_ms=0,
                       loss_rev={"random": 80}),
    "p6_blackout": dict(grp="A", cls="I", fwd="trace", rev="trace", trace="p6_blackout",
                        cap_fwd=100_000, cap_rev=100_000, loss={"trace": "p6_blackout"}, delay_ms=0, jitter_ms=0),
    "p7_bbr": dict(grp="A", cls="I", fwd="loss 2% rate 50kbit", rev="loss 2% rate 50kbit",
                   cap_fwd=50_000, cap_rev=50_000, loss={"random": 2}, delay_ms=0, jitter_ms=0),
    # Grup B (r5 F aynen; h5 DÜZLEM-B → ertelendi, karar D5 — katalogda yok)
    "h1_ulra": dict(grp="B", cls="III", fwd="delay 1200ms 400ms distribution normal loss 40% rate 0.2kbit",
                    rev="delay 1200ms 400ms distribution normal loss 40% rate 0.2kbit",
                    cap_fwd=200, cap_rev=200, loss={"random": 40}, delay_ms=1200, jitter_ms=400),
    "h2_cell": dict(grp="B", cls="II", fwd="delay 300ms 200ms distribution normal loss 25% rate 8kbit",
                    rev="delay 300ms 200ms distribution normal loss 25% rate 8kbit",
                    cap_fwd=8_000, cap_rev=8_000, loss={"random": 25}, delay_ms=300, jitter_ms=200),
    "h3a_burst40": dict(grp="B", cls="I", fwd="loss gemodel 40% 25% 90% 10% rate 250kbit",
                        rev="rate 250kbit", cap_fwd=250_000, cap_rev=250_000,
                        loss={"gemodel": [40, 25, 90, 10]}, delay_ms=0, jitter_ms=0),
    "h3b_burst50": dict(grp="B", cls="I", fwd="loss gemodel 50% 25% 90% 10% rate 250kbit",
                        rev="rate 250kbit", cap_fwd=250_000, cap_rev=250_000,
                        loss={"gemodel": [50, 25, 90, 10]}, delay_ms=0, jitter_ms=0),
    "h3c_burst60": dict(grp="B", cls="I", fwd="loss gemodel 60% 25% 90% 10% rate 250kbit",
                        rev="rate 250kbit", cap_fwd=250_000, cap_rev=250_000,
                        loss={"gemodel": [60, 25, 90, 10]}, delay_ms=0, jitter_ms=0),
    "h4p_periodic": dict(grp="B", cls="II", fwd="trace", rev="trace", trace="h4p_1on2off",
                         cap_fwd=100_000, cap_rev=100_000, loss={"trace": "h4p_1on2off"}, delay_ms=0, jitter_ms=0),
    "h4s_sustained": dict(grp="B", cls="II", fwd="loss 85% rate 100kbit", rev="loss 85% rate 100kbit",
                          cap_fwd=100_000, cap_rev=100_000, loss={"random": 85}, delay_ms=0, jitter_ms=0),
    "h4a_revsevere": dict(grp="B", cls="II", fwd="loss 10% rate 100kbit", rev="loss 90% rate 100kbit",
                          cap_fwd=100_000, cap_rev=100_000, loss={"random": 10}, delay_ms=0, jitter_ms=0,
                          loss_rev={"random": 90}),
    "h6_rtt20": dict(grp="B", cls="I", fwd="delay 10ms loss 1% rate 10mbit", rev="delay 10ms loss 1% rate 10mbit",
                     cap_fwd=10_000_000, cap_rev=10_000_000, loss={"random": 1}, delay_ms=10, jitter_ms=0),
    "h6_rtt200": dict(grp="B", cls="I", fwd="delay 100ms loss 1% rate 10mbit", rev="delay 100ms loss 1% rate 10mbit",
                      cap_fwd=10_000_000, cap_rev=10_000_000, loss={"random": 1}, delay_ms=100, jitter_ms=0),
}
# Grup C (sub-kbps): p8/p9/p10 × {0,20,40}
for rate_kbps, pid in [(1.0, "p8"), (0.5, "p9"), (0.25, "p10")]:
    for lp in (0, 20, 40):
        spec = f"rate {rate_kbps}kbit" if lp == 0 else f"loss {lp}% rate {rate_kbps}kbit"
        PROFILES[f"{pid}_l{lp}"] = dict(
            grp="C", cls="III", fwd=spec, rev=spec,
            cap_fwd=int(rate_kbps * 1000), cap_rev=int(rate_kbps * 1000),
            loss={"random": lp}, delay_ms=0, jitter_ms=0)

# Mesaj planları + deadline setleri (KAPI-1 §9.3; paced aralıklar sınıf-başına).
CLASS_PLAN = {
    "I": dict(msgs=100, payload=256, paced_interval_ms=1_000, deadlines_s=[30, 60, 120, 300]),
    "II": dict(msgs=30, payload=64, paced_interval_ms=5_000, deadlines_s=[30, 60, 120, 300]),
    "III": dict(msgs=10, payload=32, paced_interval_ms=30_000, deadlines_s=[120, 300, 600]),
}

ARMS = ["kama-arq", "kama-tail-v1", "q-stream", "q-dgram-arq"]  # FAZ-1
ARM_CLI = {
    "kama-arq": dict(transport="kama", kama_arm="arq"),
    "kama-tail-v1": dict(transport="kama", kama_arm="tail-v1"),
    "q-stream": dict(transport="quinn", kama_arm="arq"),
    "q-dgram-arq": dict(transport="qdgram", kama_arm="arq"),
}

# Zaman-pencere planları (W7 kapi3_trace formatı; commit'li = trace-replay, r5 J).
TRACE_PLANS = {
    "p6_blackout": "0 none|none\n30 loss 100%|loss 100%\n60 none|none\n",
    "h4p_1on2off": "".join(
        f"{t} loss 100%|loss 100%\n{t + 1} none|none\n" for t in range(0, 120, 3)
    ),
}


def h8(*parts: bytes) -> int:
    """SHA-256(parçalar) → ilk 8 byte BE u64 (deterministik tohum)."""
    d = hashlib.sha256(b"\x1f".join(parts)).digest()
    return int.from_bytes(d[:8], "big")


def budgets(prof: dict, deadline_s: int, workload: str, msgs: int, interval_ms: int) -> dict:
    """Kanal-türevli bütçe (yön-başına; aktarım YOK). Değerler IP-byte."""
    c_fwd, c_rev = prof["cap_fwd"], prof["cap_rev"]
    window_s = deadline_s + (msgs - 1) * interval_ms / 1000.0 if workload == "paced" else deadline_s
    out = {"u": U_PRIMARY, "window_s": window_s}
    if workload == "burst":
        out["fwd"] = {"burst_bytes": int(c_fwd * deadline_s * U_PRIMARY / 8)}
        out["rev"] = {"burst_bytes": int(c_rev * deadline_s * U_PRIMARY / 8)}
    else:
        for k, c in (("fwd", c_fwd), ("rev", c_rev)):
            refill = int(c * U_PRIMARY / 8)
            out[k] = {
                "refill_bytes_per_s": refill,
                "bucket_bytes": max(3 * MTU, 2 * refill),  # D6: 2 s × refill
                "ceiling_bytes": int(c * window_s * U_PRIMARY / 8),
            }
    return out


def full_schema(pid: str, prof: dict) -> dict:
    """r5 F tam profil şeması (eksik alanlı profil KAPI-1'den geçemez)."""
    return {
        "profile_id": pid,
        "external_label": pid,
        "group": prof["grp"],
        "class": prof["cls"],
        "duplex_model": "full",  # netem çift-yön-ayrı; half-duplex telsiz TEMSİL EDİLMİYOR (raporda).
        "forward_capacity_bps": prof["cap_fwd"],
        "reverse_capacity_bps": prof["cap_rev"],
        "one_way_base_delay_ms": prof["delay_ms"],
        "jitter_ms": prof["jitter_ms"],
        "delay_distribution": "normal" if prof["jitter_ms"] else "none",
        "correlation": 0,
        "fwd_rev_rng": "independent",
        "loss_model": prof["loss"],
        "loss_model_rev": prof.get("loss_rev", prof["loss"]),
        "queue": {"discipline": "netem", "limit_packets": 1000, "cross_traffic": None},
        "qdisc_order": "netem(delay,loss,rate) tek kural",
        "ip_version": 4,
        "path_mtu": MTU,
        "max_udp_payload": 1452,
        "fragmentation_policy": "none (mesaj tek datagram)",
        "pmtud_policy": "off (sabit MTU)",
        "minimum_frame_size": "quinn Initial 1200B padding nötr raporlanır",
        "message_coalescing_policy": "off (GSO off; wrapper max_transmit_segments=1)",
        "netem_fwd": prof["fwd"],
        "netem_rev": prof["rev"],
        "trace_plan": prof.get("trace"),
    }


def gen_rows(phase: str, drand_hex: str | None):
    """Manifest satırları. calibration: primary aile ×4 kol ×2 workload ×n=10, D=120 (KAPI-1 §10)."""
    if phase == "calibration":
        cells = [(p, w, 10, 120) for p in ("p2_loss80", "p3_loss95", "p5_revpath")
                 for w in ("paced", "burst")]
        ns_tag, salt = b"KAMA-V2-CAL", b""
    elif phase == "evaluation":
        if not drand_hex:
            sys.exit("HATA: evaluation manifesti drand-randomness OLMADAN üretilemez (Model A kilidi)")
        # FAZ-1 headline: primary aile × paced × A2 × n=30 (p2/p3 n=50) + sekonder eksenler KAPI-2'de.
        cells = [(p, "paced", 50 if p in ("p2_loss80", "p3_loss95") else 30, 120)
                 for p in ("p2_loss80", "p3_loss95", "p5_revpath")]
        ns_tag, salt = b"KAMA-V2-EVAL", bytes.fromhex(drand_hex)
    else:
        sys.exit(f"bilinmeyen faz: {phase}")

    for pid, workload, n, deadline_s in cells:
        prof = PROFILES[pid]
        plan = CLASS_PLAN[prof["cls"]]
        interval = plan["paced_interval_ms"] if workload == "paced" else 0
        cell_id = f"{phase}:{pid}:{workload}:d{deadline_s}"
        # Dengeli randomizasyon: 10-blokta 5/5 kama-önce; sıra cell-tohumundan (deterministik).
        order_seed = h8(salt, ns_tag, cell_id.encode(), b"order")
        for rep in range(n):
            block, pos = rep // 10, rep % 10
            # blok içi 10 pozisyonun 5'i kama-önce: pozisyon permütasyonu tohumdan.
            perm = sorted(range(10), key=lambda i, b=block: h8(salt, ns_tag, cell_id.encode(),
                                                              b"perm", bytes([b, i])))
            kama_first = pos in perm[:5]
            seed = h8(salt, ns_tag, cell_id.encode(), rep.to_bytes(4, "big"))
            yield {
                "cell_id": cell_id, "phase": phase, "rep": rep, "seed": seed,
                "kama_first": kama_first,
                "profile": full_schema(pid, prof),
                "arms": {a: ARM_CLI[a] for a in ARMS},
                "workload": workload, "submit_interval_ms": interval,
                "msgs": plan["msgs"], "payload": plan["payload"],
                "deadline_s": deadline_s,
                "budget": budgets(prof, deadline_s, workload, plan["msgs"], interval),
                "plane": "A2", "quota_r": 4,
            }


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("mode", choices=["calibration", "evaluation", "profiles"])
    ap.add_argument("--out", default="-")
    ap.add_argument("--drand-randomness", default=None)
    a = ap.parse_args()

    out = sys.stdout if a.out == "-" else open(a.out, "w")
    if a.mode == "profiles":
        for pid, prof in PROFILES.items():
            out.write(json.dumps(full_schema(pid, prof), ensure_ascii=False) + "\n")
        for name, plan in TRACE_PLANS.items():
            out.write(json.dumps({"trace_plan": name, "windows": plan}) + "\n")
        return 0
    rows = list(gen_rows(a.mode, a.drand_randomness))
    for r in rows:
        out.write(json.dumps(r, ensure_ascii=False) + "\n")
    digest = hashlib.sha256("".join(json.dumps(r, ensure_ascii=False) for r in rows).encode()).hexdigest()
    print(f"{a.mode}: {len(rows)} satır, manifest-sha256={digest}", file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.exit(main())
