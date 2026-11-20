#!/usr/bin/env python3
"""kama-target-net-v2 betiklerinin birim testleri — SENTETİK veriyle (ağ, netem, ikili gerekmez).

Koşum: python3 -m unittest scripts/tnet/test_tnet.py   (yalnız stdlib)
"""
import csv
import hashlib
import json
import os
import subprocess
import sys
import tempfile
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import tnet_analyze as ta  # noqa: E402
import tnet_exec as te  # noqa: E402
import tnet_manifest as tm  # noqa: E402

V1_GRAPH_HEAD = ["loss_pct", "rtt_class", "workload", "arm", "completion_rate", "n", "x", "wilson_lo", "wilson_hi",
                 "deadline_rate", "delivered_frac_mean", "scope", "mode", "group"]


class ManifestTests(unittest.TestCase):
    def test_deterministic_and_sized(self):
        a, b = tm.build("R1"), tm.build("R1")
        self.assertEqual(a, b)
        self.assertEqual(len(a), len(tm.cells()) * len(tm.ARMS) * tm.N_PER_REPLICA)
        self.assertEqual(len(tm.cells()), 54)
        with tempfile.TemporaryDirectory() as d:
            p1, p2 = os.path.join(d, "a.csv"), os.path.join(d, "b.csv")
            for p in (p1, p2):
                subprocess.run([sys.executable, os.path.join(HERE, "tnet_manifest.py"), "--replica", "R2", "--out", p],
                               check=True, capture_output=True)
            h = []
            for p in (p1, p2):
                with open(p, "rb") as f:
                    h.append(hashlib.sha256(f.read()).hexdigest())
            self.assertEqual(h[0], h[1])

    def test_horizon_formula(self):
        self.assertEqual(tm.timing("main", "low", "small", "none"), (11, 225))
        self.assertEqual(tm.timing("main", "high", "large", "none")[0], 20)
        # band large med: ser = 100495/12500 = 8.0396 s → 10 + 2.5 + 80.396 = 92.9 → 93
        self.assertEqual(tm.timing("band", "med", "large", "100kbit")[0], 93)
        self.assertEqual(tm.timing("band", "high", "small", "100kbit")[0], 24)
        self.assertEqual(tm.timing("asym", "field", "small", "100kbit")[0], 180)
        self.assertEqual(tm.timing("asym-twin", "field", "large", "100kbit")[0], 180)
        self.assertEqual(tm.timing("control", "none", "large", "none"), (11, 225))

    def test_netem_strings(self):
        self.assertEqual(tm.netem(125, 30, "both", "100kbit", "none"),
                         ("delay 125ms loss 30% rate 100kbit", "delay 125ms loss 30%"))
        self.assertEqual(tm.netem(300, 20, "fwd", "100kbit", "0.5kbit"),
                         ("delay 300ms loss 20% rate 100kbit", "delay 300ms rate 0.5kbit"))
        self.assertEqual(tm.netem(0, 0, "both", "none", "none"), ("delay 0ms", "delay 0ms"))

    def test_seeds_shared_across_arms_and_rotation(self):
        rows = [r for r in tm.build("R1") if r[3] == "iid30-med-large"]
        by_rep = {}
        for r in rows:
            by_rep.setdefault(r[16], []).append(r)
        for rep, rs in by_rep.items():
            self.assertEqual(len({(r[22], r[23]) for r in rs}), 1, "tohum kollar arasında ortak olmalı")
            self.assertEqual(sorted(r[14] for r in rs), sorted(tm.ARMS))
        self.assertNotEqual(by_rep[0][0][14], by_rep[1][0][14], "kol sırası rep ile dönmeli")
        r1 = {r[22] for r in rows}
        r2 = {r[22] for r in tm.build("R2") if r[3] == "iid30-med-large"}
        self.assertFalse(r1 & r2, "replikaların tohum alanı ayrık olmalı")

    def test_blocks_chunk_ten_reps(self):
        rows = tm.build("R1")
        blk = {}
        for r in rows:
            blk.setdefault(r[24], []).append(r)
        self.assertTrue(all(len(v) == tm.REP_CHUNK * len(tm.ARMS) for v in blk.values()))

    def test_pilot_subset(self):
        p = tm.build("P", pilot=True)
        self.assertEqual(len(p), 7 * 2 * len(tm.ARMS))
        self.assertTrue(all(r[24].endswith("/pilot") for r in p))


class NetemCheckTests(unittest.TestCase):
    def q(self, delay=0.125, loss=0.3, rate=None, seed=42, limit=1000):
        o = {"limit": limit, "delay": {"delay": delay, "jitter": 0, "correlation": 0}, "seed": seed}
        if loss:
            o["loss-random"] = {"loss": loss, "correlation": 0}
        if rate is not None:
            o["rate"] = {"rate": rate, "packetoverhead": 0}
        return json.dumps([{"kind": "netem", "handle": "8001:", "options": o}])

    def test_match_and_mismatch(self):
        self.assertTrue(te.check_netem(self.q(), "delay 125ms loss 30%", 42)[0])
        self.assertEqual(te.check_netem(self.q(), "delay 125ms loss 30%", 42)[1], 1000)
        self.assertFalse(te.check_netem(self.q(), "delay 125ms loss 20%", 42)[0])
        self.assertFalse(te.check_netem(self.q(), "delay 125ms loss 30%", 43)[0], "tohum denetlenmeli")
        self.assertTrue(te.check_netem(self.q(rate=12500), "delay 125ms loss 30% rate 100kbit", 42)[0])
        self.assertFalse(te.check_netem(self.q(), "delay 125ms loss 30% rate 100kbit", 42)[0], "hız unutulmamalı")
        # 0,5 kbit = 62,5 B/s → tc 62 B/s (aşağı yuvarlama); ±1 tolerans
        self.assertTrue(te.check_netem(self.q(delay=0.3, loss=0, rate=62), "delay 300ms rate 0.5kbit", 42)[0])
        self.assertFalse(te.check_netem("[]", "delay 0ms", None)[0])


def write_run(d, meta, delivered):
    with open(os.path.join(d, "out.jsonl"), "w") as f:
        f.write(json.dumps({"meta": meta}) + "\n")
        for i, t in enumerate(delivered):
            f.write(json.dumps({"event": {"msg_id": i, "sent_ns": 0, "delivered_ns": t, "payload_bytes": 1}}) + "\n")


def snap(wall=1.0, no_netem=False):
    q = [{"kind": "netem", "packets": 0, "drops": 0}]
    q2 = [{"kind": "netem", "packets": 10, "drops": 2}]
    l0 = [{"stats64": {"tx": {"bytes": 100}, "rx": {"bytes": 50}}}]
    l1 = [{"stats64": {"tx": {"bytes": 1100}, "rx": {"bytes": 250}}}]
    return {"q_fwd_before": q, "q_fwd_after": q2, "q_rev_before": q, "q_rev_after": q2, "l_before": l0, "l_after": l1,
            "rc": 0, "seq_no": 1, "unix_ts": 1, "netem_limit": 1000, "loadavg1": "0.5", "nproc": 12,
            "no_netem": no_netem, "wall_s": wall}


class AssembleTests(unittest.TestCase):
    def man(self, arm, wl="small"):
        rows = [r for r in tm.build("R1") if r[14] == arm and r[11] == wl and r[2] == "main"]
        return dict(zip(tm.COLS, map(str, rows[0])))

    def test_kama_port_ok_and_checks(self):
        m = self.man("kama-port")
        n = int(m["msgs"])
        base = {"profile_id": "kama-v1-full", "profile_delta": [], "engine": "cc-port", "kama_arm":
                "kama-product-tailv1-cc-port-adaptive-p0x05-lane-text", "events_epoch_offset_ns": 1000,
                "tx": {"fec": 77, "retx": 5}, "warmup_residue": 0, "setup_ns": 9}
        with tempfile.TemporaryDirectory() as d:
            write_run(d, base, [1000 + 5_000_000] * n)
            r = te.assemble(m, d, snap())
            self.assertEqual(r["status"], "OK")
            self.assertEqual((r["delivered"], r["complete_horizon"], r["t_complete_ms"]), (n, 1, "5.000"))
            self.assertEqual((r["wire_bytes"], r["wire_tx_bytes"], r["wire_rx_bytes"]), (1200, 1000, 200))
            self.assertEqual((r["fwd_pkts"], r["fwd_drops"], r["kama_fec_bytes"]), (10, 2, 77))
            for patch, want in (({"engine": "legacy"}, "ENGINE-MISMATCH"),
                                ({"kama_arm": "kama-product-tailv1-cc-port-adaptive-p0x05"}, "LANE-MISMATCH"),
                                ({"profile_delta": ["x"]}, "PROFILE-MISMATCH")):
                write_run(d, {**base, **patch}, [1000 + 5_000_000] * n)
                self.assertEqual(te.assemble(m, d, snap())["status"], want)
                self.assertEqual(te.assemble(m, d, snap())["complete_horizon"], 0)

    def test_nofec_expected_delta(self):
        m = self.man("kama-port-nofec", "large")
        n = int(m["msgs"])
        meta = {"profile_id": "kama-custom", "profile_delta": te.NOFEC_DELTA, "engine": "cc-port",
                "kama_arm": "kama-product-tailv1-cc-port-static-r0-p0x05-lane-file", "events_epoch_offset_ns": 0}
        with tempfile.TemporaryDirectory() as d:
            write_run(d, meta, [10 ** 9] * n)
            self.assertEqual(te.assemble(m, d, snap())["status"], "OK")
            write_run(d, {**meta, "profile_delta": []}, [10 ** 9] * n)
            self.assertEqual(te.assemble(m, d, snap())["status"], "PROFILE-MISMATCH")

    def test_missing_output_and_horizon(self):
        m = self.man("tcp")
        with tempfile.TemporaryDirectory() as d:
            r = te.assemble(m, d, snap())
            self.assertEqual((r["status"], r["complete_horizon"]), ("ERROR-NO-RESULT", 0))
            n = int(m["msgs"])
            write_run(d, {"events_epoch_offset_ns": 0}, [10 ** 9] * (n - 1) + [None])
            r = te.assemble(m, d, snap())
            self.assertEqual((r["status"], r["delivered"], r["complete_horizon"]), ("OK", n - 1, 0))
            late = (int(m["horizon_s"]) * 1000 + 1) * 10 ** 6
            write_run(d, {"events_epoch_offset_ns": 0}, [late] * n)
            self.assertEqual(te.assemble(m, d, snap())["complete_horizon"], 0, "horizon sonrası tamamlanma sayılmaz")
            write_run(d, {"events_epoch_offset_ns": 0}, [10 ** 6] * n)
            self.assertEqual(te.assemble(m, d, snap(no_netem=True))["status"], "SMOKE-NO-NETEM")

    def test_append_and_resume(self):
        m = self.man("tcp")
        with tempfile.TemporaryDirectory() as d:
            res = os.path.join(d, "results.csv")
            r = te.assemble(m, d, snap())
            te.append_row(res, r)
            te.append_row(res, r)
            rows = list(csv.DictReader(open(res)))
            self.assertEqual(list(rows[0].keys()), te.COLS)
            self.assertEqual(len(rows), 2)
            self.assertEqual(te.done_ids(res), {m["run_id"]})


# ── analizör: sentetik sonuç tablosu ───────────────────────────────────────────────────────────
def synth_results(path, plan, manifests, extra=(), drop_ids=()):
    """plan[(group, cell, arm, replica)] = tamamlanan rep sayısı (ilk k rep tamam; kalanı başarısız)."""
    with open(path, "w", newline="") as f:
        w = csv.writer(f, lineterminator="\n")
        w.writerow(te.COLS)
        for rep_name, rows in manifests.items():
            for mr in rows:
                m = dict(zip(tm.COLS, map(str, mr)))
                k = plan.get((m["group"], m["cell"], m["arm"], rep_name), None)
                if k is None or m["run_id"] in drop_ids:
                    continue
                ok = int(m["rep"]) < k
                n = int(m["msgs"])
                row = dict.fromkeys(te.COLS, "")
                for c in ("run_id", "replica", "group", "cell", "loss_pct", "loss_dir", "rtt_class", "owd_ms",
                          "rate_fwd", "rate_rev", "workload", "msgs", "payload", "arm", "mode", "rep", "deadline_ms",
                          "block"):
                    row[c] = m[c]
                row.update(horizon_ms=int(m["horizon_s"]) * 1000, status="OK", delivered=n if ok else n // 2,
                           complete_horizon=int(ok), complete_deadline=0, t_complete_ms="1000.000" if ok else "",
                           wire_bytes=3 * n * int(m["payload"]), load_flag=0, warmup_residue=0,
                           kama_fec_bytes=n if m["arm"] == "kama-port" else "")
                w.writerow([row[c] for c in te.COLS])
        for row in extra:
            w.writerow([row.get(c, "") for c in te.COLS])


class AnalyzeTests(unittest.TestCase):
    def run_analyze(self, plan, extra=(), drop_ids=()):
        d = tempfile.mkdtemp()
        man = {"R1": tm.build("R1"), "R2": tm.build("R2")}
        for rep, rows in man.items():
            with open(os.path.join(d, f"manifest-{rep}.csv"), "w", newline="") as f:
                w = csv.writer(f, lineterminator="\n")
                w.writerow(tm.COLS)
                w.writerows(rows)
        res = os.path.join(d, "results.csv")
        synth_results(res, plan, man, extra, drop_ids)
        out = os.path.join(d, "out")
        subprocess.run([sys.executable, os.path.join(HERE, "tnet_analyze.py"), "--manifest",
                        f"R1={d}/manifest-R1.csv", "--manifest", f"R2={d}/manifest-R2.csv", "--result", res,
                        "--out", out], check=True, capture_output=True)
        return out

    def test_wilson_known_values(self):
        lo, hi = ta.wilson(0, 60)
        self.assertAlmostEqual(hi, 0.0602, places=3)
        self.assertEqual(lo, 0.0)
        lo, hi = ta.wilson(60, 60)
        self.assertAlmostEqual(lo, 0.9398, places=3)
        self.assertEqual(ta.pct([5, 1, 3, 2, 4], 0.5), 3)
        self.assertEqual(ta.pct([5, 1, 3, 2, 4], 0.9), 5)

    def test_verdicts_fec_graph_accounting(self):
        C, L = "iid30-med-large", "iid50-low-small"
        plan = {}
        for rep in ("R1", "R2"):
            plan.update({("main", C, "kama-port", rep): 30, ("main", C, "tcp", rep): 0,
                         ("main", C, "quinn", rep): 15, ("main", C, "quinn-bbr", rep): 30,
                         ("main", C, "kama-port-nofec", rep): 5,
                         ("main", L, "kama-port", rep): 0, ("main", L, "tcp", rep): 30,
                         ("main", L, "quinn", rep): 0, ("main", L, "quinn-bbr", rep): 0,
                         ("main", L, "kama-port-nofec", rep): 30})
        smoke = {"run_id": "S-x", "replica": "R1", "group": "main", "cell": C, "arm": "tcp", "status": "SMOKE-NO-NETEM",
                 "msgs": "1", "payload": "1", "load_flag": "0"}
        dropped = {f"R1-main-{C}-warm-tcp-r000"}
        out = self.run_analyze(plan, extra=[smoke], drop_ids=dropped)
        T = open(os.path.join(out, "TABLES.md")).read()
        line = next(x for x in T.splitlines() if x.startswith("| 30% | med | large"))
        self.assertIn("**kama-port ÖNDE**", line)               # 60/60 ↔ 0/59 (tcp)
        self.assertIn("ayırt edilemedi", line)                   # ↔ quinn-bbr 60/60
        self.assertIn("50% | low | small · tcp ÖNDE", T)          # GERİDE listesi aynı görünürlükte
        fec = next(x for x in T.splitlines() if x.startswith("| main | 30% | med | large"))
        self.assertIn("FEC karşılığını verdi", fec)
        fec2 = next(x for x in T.splitlines() if x.startswith("| main | 50% | low | small"))
        self.assertIn("FEC zarar verdi", fec2)
        G = list(csv.reader(open(os.path.join(out, "GRAPH.csv"))))
        self.assertEqual(G[0][:14], V1_GRAPH_HEAD, "GRAPH.csv v1 şeklini korumalı")
        A = open(os.path.join(out, "ACCOUNTING.md")).read()
        self.assertIn("SMOKE-NO-NETEM / pilot (P): 1", A)
        self.assertRegex(A, r"\| R1 \| 8100 \| 299 \| 7801 \| 0 \|")   # 2 hücre × 5 kol × 30 = 300 − 1 düşürülen
        cr = [r for r in csv.DictReader(open(os.path.join(out, "CELL-RESULTS.csv")))
              if r["scope"] == "pooled" and r["cell"] == C and r["arm"] == "kama-port"][0]
        self.assertEqual(cr["n"], "60")
        self.assertEqual(cr["wire_per_delivered_payload_all"], "3.000")   # Σtel = 3 × Σteslim-yük

    def test_status_not_ok_stays_in_denominator_and_banner(self):
        C = "iid10-low-small"
        plan = {("main", C, a, rep): 30 for a in tm.ARMS for rep in ("R1", "R2")}
        rid = f"R1-main-{C}-warm-kama-port-r000"
        common = {"replica": "R1", "group": "main", "cell": C, "arm": "kama-port", "msgs": "48", "payload": "101",
                  "delivered": "48", "complete_horizon": "0", "complete_deadline": "0", "load_flag": "0",
                  "warmup_residue": "0", "wire_bytes": "100", "loss_pct": "10", "loss_dir": "both",
                  "rtt_class": "low", "rate_fwd": "none", "rate_rev": "none", "workload": "small", "mode": "warm"}
        bad = {**common, "run_id": rid, "status": "ENGINE-MISMATCH"}
        foreign = {**common, "run_id": "R1-main-not-in-manifest", "status": "OK", "complete_horizon": "1"}
        out = self.run_analyze(plan, extra=[bad, foreign], drop_ids={rid})
        T = open(os.path.join(out, "TABLES.md")).read()
        self.assertIn("GEÇERSİZLİK UYARISI", T)
        self.assertIn("kama-port/ENGINE-MISMATCH=1", T)
        A = open(os.path.join(out, "ACCOUNTING.md")).read()
        self.assertIn("| R1 | 8100 | 151 | 7950 | 1 | 0 | 1 | 0 |", A)   # fazla satır sayılır, hükme girmez
        cr = [r for r in csv.DictReader(open(os.path.join(out, "CELL-RESULTS.csv")))
              if r["scope"] == "pooled" and r["cell"] == C and r["arm"] == "kama-port"][0]
        self.assertEqual((cr["n"], cr["horizon_x"], cr["status_not_ok"]), ("60", "59", "1"))

    def test_replica_disagreement_detected(self):
        C = "iid20-high-large"
        plan = {("main", C, a, "R1"): 30 for a in tm.ARMS}
        plan.update({("main", C, a, "R2"): 30 for a in tm.ARMS})
        plan[("main", C, "tcp", "R2")] = 0
        out = self.run_analyze(plan)
        T = open(os.path.join(out, "TABLES.md")).read()
        self.assertIn(f"main · {C} · tcp: R1 30/30 ↔ R2 0/30", T)


if __name__ == "__main__":
    unittest.main()
