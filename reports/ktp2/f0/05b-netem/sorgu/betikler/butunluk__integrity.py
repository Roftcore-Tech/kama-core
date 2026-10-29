#!/usr/bin/env python3
"""[betimsel, sonradan] extract.py CSV'si üstünde bütünlük denetimi. Kullanım: integrity.py CSV"""
import collections
import csv
import sys

EXP = {
    "p1_loss50": ("loss 50% rate 100kbit", "loss 50% rate 100kbit"),
    "p2_loss80": ("loss 80% rate 200kbit", "loss 80% rate 200kbit"),
    "p3_loss95": ("loss 95% rate 100kbit", "loss 95% rate 100kbit"),
    "p4_jitter": ("delay 200ms 100ms distribution normal rate 100kbit",) * 2,
    "p5_revpath": ("loss 5% rate 100kbit", "loss 80% rate 100kbit"),
    "p6_blackout": ("trace:p6_blackout", "trace:p6_blackout"),
    "p7_bbr": ("loss 2% rate 50kbit", "loss 2% rate 50kbit"),
}
rows = list(csv.DictReader(open(sys.argv[1])))
prob = collections.Counter()
ex = []
cnt = collections.Counter((r["profile"], r["arm"]) for r in rows)
for r in rows:
    k = (r["profile"], r["arm"], r["rep"])
    checks = {
        "rc!=0": r["rc"] != "0",
        "arm!=kama-v1-full": r["m_arm"] != "kama-v1-full",
        "delta!=[]": r["m_delta"] != "[]",
        "meta_delta!=[]": r["meta_delta"] != "[]",
        "meta_profile_id": r["meta_profile_id"] != "kama-v1-full",
        "fwd_mismatch": r["m_fwd"] != EXP[r["profile"]][0],
        "rev_mismatch": r["m_rev"] != EXP[r["profile"]][1],
        "seed!=rep": r["m_seed"] != r["rep"],
        "repeat!=rep": r["m_repeat"] != r["rep"],
        "n_ev!=100": r["n_ev"] != "100",
        "cerr_extra": r["cerr_extra"] != "",
        "serr_extra": r["serr_extra"] != "",
        "seal_fail": r["seal_fail"] not in ("0", ""),
        "send_err": r["send_err"] not in ("0", ""),
        "s_seal_fail": r["s_seal_fail"] not in ("0", ""),
        "s_open_fail": r["s_open_fail"] not in ("0", ""),
        "s_dropped_new": r["s_dropped_new"] not in ("0", ""),
        "setup_in_window": r["setup_in_window"] not in ("False",),
    }
    for c, bad in checks.items():
        if bad:
            prob[c] += 1
            ex.append((c, k, r.get(c.split("!")[0], ""), r["cerr_extra"][:150], r["serr_extra"][:150]))
print("kol-profil sayımları:", dict(cnt))
print("sorun sayaçları:", dict(prob))
for e in ex[:60]:
    print("  ", e)
# p6 trace log
p6 = [r for r in rows if r["profile"] == "p6_blackout"]
tl = collections.Counter(r["trace_log"] for r in p6)
print("p6 trace.plan.log biçimleri:")
for t, c in tl.items():
    print(f"   {c}x  {t}")
# kol sırası
oc = collections.Counter((r["arm"], int(r["rep"]) % 2, r["order"]) for r in rows)
print("kol sırası (arm, rep%2, order):", dict(oc))
