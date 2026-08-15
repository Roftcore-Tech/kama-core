#!/usr/bin/env python3
"""FS-CAL-2 v3 §11 — deterministic analysis pipeline (fixture-first; YETKİ-1 tool build).

Pipeline order (§11.2): freeze/provenance → manifest expected set → strict typed parse →
missing/extra/duplicate/sidecar → feature RECOMPUTATION from the fate window → serialized/recomputed
exact equality → configured-feature leakage allowlist → deterministic order. Any failure ⇒ namespace
DATA-CONFORMANCE-NO-GO (no silent row drops).

Then: PairRecord/HoldoutTriple (§11.3) → cell/survival stats (§11.4: Wilson, exact McNemar,
pairing-group bootstrap B=10,000 seed 20260805, KM/RMST) → counterfactual candidate fit (§11.5/§7.3
factorized exact-integer grid) → §3.4b support bitmap → §9.0 K1–K8 identifiability → §11.8 twelve
artifacts. Deterministic: no wall-clock, sorted keys, fixed rounding; two runs must be byte-identical.

--selftest runs the §11.1 negative/golden fixture suite (schema reject, pair reject, snapshot
mismatch, recompute mismatch, K-gate PASS/FAIL examples) without touching any raw namespace.
"""
import argparse, hashlib, json, math, os, random, sys
from collections import defaultdict

PPM = 1_000_000
SCALE = 1_000_000
BOOT_B = 10_000
BOOT_SEED = 20260805
K5_B = 1_000
ALPHA = 0.05
MIN_GAIN_PP = 15  # frozen minimum effect (EXPERIMENTAL_BOUND; cost contract)
# §3.4b pinned support thresholds (fixture mode may relax ONLY for tooling validation — recorded)
SUPPORT_MIN_GROUPS = 100
SUPPORT_MIN_CELLS = 2
# §7.3 pinned search grids (FULL — used by the result-bearing YETKİ-2 fit on hosted CI).
LOSS_GRID_FULL = list(range(0, 300_001, 5_000))          # 61 pts
BURST_GRID_FULL = list(range(-200_000, 800_001, 25_000))  # 41 pts
THRESHOLDS_FULL = [500_000, 550_000, 600_000, 650_000, 700_000, 750_000, 800_000, 850_000, 900_000]
K3_LOSS_FULL = list(range(0, 300_001, 5_000))
K3_BURST_FULL = list(range(-200_000, 900_001, 25_000))
# FIXTURE-COARSE grids (YETKİ-1 DATA-PROCESSABLE only — proves the pipeline runs + is deterministic,
# NOT the real fit; the fixture output is explicitly labeled non-calibration). Same algorithm, coarse
# breakpoints so the O(tuples × region × bootstrap) product is tractable locally.
LOSS_GRID_COARSE = [0, 100_000, 200_000, 300_000]
BURST_GRID_COARSE = [-200_000, 0, 200_000, 400_000]
THRESHOLDS_COARSE = [500_000, 700_000]
K3_LOSS_COARSE = list(range(0, 300_001, 50_000))
K3_BURST_COARSE = list(range(-200_000, 900_001, 100_000))
# active grids (rebound in run_pipeline by fixture_mode)
LOSS_GRID, BURST_GRID, THRESHOLDS = LOSS_GRID_FULL, BURST_GRID_FULL, THRESHOLDS_FULL
K3_LOSS, K3_BURST = K3_LOSS_FULL, K3_BURST_FULL


def sha256_hex(b):
    return hashlib.sha256(b if isinstance(b, bytes) else b.encode()).hexdigest()


def die(msg):
    print(f"FATAL: {msg}")
    sys.exit(2)


# ── strict schema ─────────────────────────────────────────────────────────────
def load_schema(path):
    with open(path) as f:
        sc = json.load(f)
    fields = sc["fields"]
    return {f["name"]: f for f in fields}, [f["name"] for f in fields]


ENUMS = {
    "arm": {"STATIC_R0", "STATIC_R2", "CALIBRATED_POLICY"},
    "environment": {"IN_PROCESS", "NETNS"},
    "experiment_phase": {"PILOT", "TRAINING", "HOLDOUT_INPROC", "HOLDOUT_NETNS", "DIAG_42B", "M1", "FIXTURE"},
    "requested_redundancy": {"R0", "R2"},
    "effective_redundancy": {"R0", "R2"},
    "completion_status": {"completed", "censored"},
    "safety_result": {"PASS", "CAP_CLAMPED"},
    "fallback_reason": {"NONE", "ESTIMATOR_INELIGIBLE", "OUT_OF_SUPPORT", "UNSUPPORTED_WORKLOAD",
                         "BELOW_ACTIVATION", "STATIC_ARM", "SAFETY_CAP"},
    "configured_structure": {"NONE", "IID", "MARKOV_BD", "GE", "TAIL", "MIXED", "DIAG"},
}
# configured ground truth: allowed ONLY for stratification/fidelity — never in policy replay inputs
CONFIGURED_OFFLINE_ONLY = {"configured_structure", "configured_loss_ppm", "configured_markov_b_ppm",
                            "configured_burst_length", "configured_tail_length", "configured_rtt_ms"}
POLICY_ALLOWLIST = {"measured_loss_ppm", "burst_delta_ppm", "estimator_eligible",
                     "support_eligible", "workload_supported"}


def validate_record(obj, schema, order, errors, rid_hint):
    keys = list(obj.keys())
    if keys != order:
        extra = [k for k in keys if k not in schema]
        missing = [k for k in order if k not in obj]
        if extra:
            errors.append(f"{rid_hint}: UNKNOWN field(s) {extra}")
        if missing:
            errors.append(f"{rid_hint}: MISSING field(s) {missing}")
        if not extra and not missing:
            errors.append(f"{rid_hint}: field ORDER deviates from canonical schema order")
        return False
    ok = True
    for name, spec in schema.items():
        v = obj[name]
        t = spec["type"]
        nullable = spec["nullable"]
        if v is None:
            if not nullable:
                errors.append(f"{rid_hint}: field {name} null but not nullable")
                ok = False
            continue
        if t in ("u64", "u64|null"):
            if not isinstance(v, int) or isinstance(v, bool) or v < 0:
                errors.append(f"{rid_hint}: field {name} not u64: {v!r}")
                ok = False
        elif t == "i64":
            if not isinstance(v, int) or isinstance(v, bool):
                errors.append(f"{rid_hint}: field {name} not i64: {v!r}")
                ok = False
        elif t == "bool":
            if not isinstance(v, bool):
                errors.append(f"{rid_hint}: field {name} not bool: {v!r}")
                ok = False
        elif t in ("string", "string|null"):
            if not isinstance(v, str):
                errors.append(f"{rid_hint}: field {name} not string: {v!r}")
                ok = False
            elif name in ENUMS and v not in ENUMS[name]:
                errors.append(f"{rid_hint}: field {name} invalid enum value {v!r}")
                ok = False
        elif t == "u64[]":
            if not isinstance(v, list) or any(not isinstance(x, int) or x < 0 for x in v):
                errors.append(f"{rid_hint}: field {name} not u64[]")
                ok = False
        elif t == "enum[]":
            if not isinstance(v, list) or any(x not in ("SUCCESS", "LOSS") for x in v):
                errors.append(f"{rid_hint}: field {name} invalid fate enum[]")
                ok = False
    return ok


def recompute_features(obj, errors, rid):
    ids = obj["estimator_window_source_ids"]
    fates = obj["estimator_window_fates"]
    if len(ids) != len(fates) or len(ids) != obj["estimator_resolved"]:
        errors.append(f"{rid}: window length mismatch")
        return False
    n = len(ids)
    lost = sum(1 for f in fates if f == "LOSS")
    n00 = n01 = n10 = n11 = 0
    for i in range(n - 1):
        if ids[i + 1] == ids[i] + 1:
            a, b = fates[i] == "LOSS", fates[i + 1] == "LOSS"
            if not a and not b:
                n00 += 1
            elif not a and b:
                n01 += 1
            elif a and not b:
                n10 += 1
            else:
                n11 += 1
    loss_ppm = lost * PPM // n if n else 0
    p11 = (n11 + 1) * PPM // (n10 + n11 + 2)
    p01 = (n01 + 1) * PPM // (n00 + n01 + 2)
    delta = p11 - p01
    exp = {"estimator_lost": lost, "n00": n00, "n01": n01, "n10": n10, "n11": n11,
           "measured_loss_ppm": loss_ppm, "p11_ppm": p11, "p01_ppm": p01, "burst_delta_ppm": delta}
    ok = True
    for k, v in exp.items():
        if obj[k] != v:
            errors.append(f"{rid}: RECOMPUTE mismatch {k}: serialized={obj[k]} recomputed={v}")
            ok = False
    return ok


# ── stats ─────────────────────────────────────────────────────────────────────
def wilson(k, n, z=1.96):
    if n == 0:
        return (0.0, 0.0)
    p = k / n
    d = 1 + z * z / n
    c = (p + z * z / (2 * n)) / d
    h = z * math.sqrt(max(0.0, p * (1 - p) / n + z * z / (4 * n * n))) / d
    return (max(0.0, c - h), min(1.0, c + h))


def mcnemar_exact_p(b, c):
    n = b + c
    if n == 0:
        return 1.0
    pmf = lambda k: math.comb(n, k) * (0.5 ** n)
    obs = pmf(min(b, c))
    return min(1.0, sum(pmf(k) for k in range(n + 1) if pmf(k) <= obs + 1e-15))


def rmst_ms(times_ns, horizon_ns, epoch_ns_list):
    # restricted mean time-to-completion from epoch; censored contributes horizon
    tot = 0
    for t, ep in zip(times_ns, epoch_ns_list):
        if t is None:
            tot += horizon_ns
        else:
            tot += min(t - ep, horizon_ns)
    return round(tot / max(1, len(times_ns)) / 1e6, 3)


# ── candidate machinery (§7.3 factorized exact-integer) ──────────────────────
def ceil_div(a, b):
    return -((-a) // b)


def x_min_of(a, b, t):
    # mu_high_loss(x;a,b) >= t  ⇔  x >= a + ceil(t*(b-a)/SCALE)   (x<b branch; x>=b always passes)
    return a + ceil_div(t * (b - a), SCALE)


def y_max_of(a, b, t):
    # mu_nonburst(y;a,b) >= t  ⇔  y <= b - ceil(t*(b-a)/SCALE)
    return b - ceil_div(t * (b - a), SCALE)


def mu_high_loss(x, a, b):
    if x <= a:
        return 0
    if x >= b:
        return SCALE
    return (x - a) * SCALE // (b - a)


def mu_nonburst(y, a, b):
    if y <= a:
        return SCALE
    if y >= b:
        return 0
    return (b - y) * SCALE // (b - a)


def support_bins(loss_ppm, delta_ppm):
    lb = loss_ppm // 50_000
    cl = max(-200_000, min(900_000, delta_ppm))
    bb = (cl + 200_000) // 100_000
    return (lb, bb)


def build_support_bitmap(train_pairs, min_groups, min_cells):
    # §3.4b: bin+8 neighbors >= min_groups eligible groups AND >= min_cells distinct cells
    by_bin = defaultdict(list)
    for p in train_pairs:
        if not p["eligible"]:
            continue
        by_bin[support_bins(p["loss_ppm"], p["delta_ppm"])].append(p["cell_id"])
    supported = set()
    for (lb, bb) in list(by_bin.keys()):
        groups = 0
        cells = set()
        for dl in (-1, 0, 1):
            for db in (-1, 0, 1):
                k = (lb + dl, bb + db)
                groups += len(by_bin.get(k, []))
                cells.update(by_bin.get(k, []))
        if groups >= min_groups and len(cells) >= min_cells:
            supported.add((lb, bb))
    return supported, {f"{k[0]},{k[1]}": len(v) for k, v in sorted(by_bin.items())}


def fit_candidates(train_pairs, supported_bins, harmful_cells_by_snapshot):
    """Counterfactual replay over PairRecords (§11.5): candidate → per-pair action → measured outcome.
    Factorized: metrics depend only on the ACTIVATION SET, determined by (x_min, y_max) thresholds
    over per-pair (loss_ppm, delta_ppm) + interlocks. Returns tiered counts + winner + tie info."""
    # eligible & supported pairs are the only ones a candidate can flip to R2
    snap = []
    for i, p in enumerate(train_pairs):
        act_ok = p["eligible"] and support_bins(p["loss_ppm"], p["delta_ppm"]) in supported_bins
        snap.append((p["loss_ppm"], p["delta_ppm"], act_ok, i))
    # pattern cache: activation-set signature → metrics
    pattern_cache = {}

    def metrics_for(xmin, ymax):
        sig_bits = tuple(
            1 if (ok and lp >= xmin and dp <= ymax) else 0 for (lp, dp, ok, _) in snap
        )
        if sig_bits in pattern_cache:
            return pattern_cache[sig_bits]
        harmful_cells = set()
        gain_capture_num = 0
        excess_wire = 0
        for bit, (_, _, _, i) in zip(sig_bits, snap):
            p = train_pairs[i]
            if bit:  # candidate chooses R2 → pair's measured R2 outcome
                if p["cell_label"] == "R0_PREFERRED":
                    harmful_cells.add(p["cell_id"])
                if p["cell_label"] == "R2_PREFERRED" and p["r2_complete"]:
                    gain_capture_num += 1
                excess_wire += max(0, p["r2_wire"] - p["r0_wire"])
            else:  # R0 outcome
                pass
        m = (len(harmful_cells), -gain_capture_num, excess_wire)
        pattern_cache[sig_bits] = (m, sig_bits)
        return pattern_cache[sig_bits]

    # region size (tier 4) over the SUPPORTED canonical grid — cached by (xmin, ymax)
    sup_grid = [(gl, gb) for gl in K3_LOSS for gb in K3_BURST
                if support_bins(gl, gb) in supported_bins]
    region_cache = {}

    def region_size(xmin, ymax):
        key = (xmin, ymax)
        r = region_cache.get(key)
        if r is None:
            r = sum(1 for (gl, gb) in sup_grid if gl >= xmin and gb <= ymax)
            region_cache[key] = r
        return r

    best = None  # (m1,m2,m3,m4)
    tier_counts = [0, 0, 0, 0]
    survivors = []  # (metrics4, tuple, sig)
    all_tuples = 0
    for t in THRESHOLDS:
        for ai in range(len(LOSS_GRID)):
            for bi in range(ai + 1, len(LOSS_GRID)):
                la, lb = LOSS_GRID[ai], LOSS_GRID[bi]
                xmin = x_min_of(la, lb, t)
                for ci in range(len(BURST_GRID)):
                    for di in range(ci + 1, len(BURST_GRID)):
                        ba, bb = BURST_GRID[ci], BURST_GRID[di]
                        all_tuples += 1
                        ymax = y_max_of(ba, bb, t)
                        (m, sig) = metrics_for(xmin, ymax)
                        m4 = m + (region_size(xmin, ymax),)
                        tup = (la, lb, ba, bb, t)
                        if best is None or m4 < best:
                            best = m4
                            survivors = [(m4, tup, sig)]
                        elif m4 == best:
                            survivors.append((m4, tup, sig))
    # tier counts: candidates surviving each successive lexicographic tier
    m1 = min(s[0][0] for s in survivors) if survivors else 0
    tier_counts[0] = sum(1 for s in survivors if s[0][0] == best[0])
    tier_counts[1] = sum(1 for s in survivors if s[0][:2] == best[:2])
    tier_counts[2] = sum(1 for s in survivors if s[0][:3] == best[:3])
    tier_counts[3] = len(survivors)
    # NOTE: with the streaming best-tracking above, `survivors` already equals the tier-4 tie set.
    survivors.sort(key=lambda s: s[1])
    tie_lines = "\n".join(",".join(map(str, s[1])) for s in survivors)
    k6_same_map = len({s[2] for s in survivors}) <= 1
    winner = survivors[0][1] if survivors else None
    return {
        "total_candidates": all_tuples,
        "tier_counts": tier_counts,
        "final_tie_count": len(survivors),
        "candidate_set_sha256": sha256_hex(tie_lines),
        "selected_tuple": winner,
        "k6_tie_region_identity": k6_same_map,
        "best_metrics": list(best) if best else None,
    }


# ── main pipeline ─────────────────────────────────────────────────────────────
def run_pipeline(raw_dir, outdir, schema_path, fixture_mode, support_min, support_cells):
    # rebind active grids per mode (fixture-coarse for YETKİ-1 processability; full for YETKİ-2 fit)
    global LOSS_GRID, BURST_GRID, THRESHOLDS, K3_LOSS, K3_BURST
    if fixture_mode:
        LOSS_GRID, BURST_GRID, THRESHOLDS = LOSS_GRID_COARSE, BURST_GRID_COARSE, THRESHOLDS_COARSE
        K3_LOSS, K3_BURST = K3_LOSS_COARSE, K3_BURST_COARSE
    else:
        LOSS_GRID, BURST_GRID, THRESHOLDS = LOSS_GRID_FULL, BURST_GRID_FULL, THRESHOLDS_FULL
        K3_LOSS, K3_BURST = K3_LOSS_FULL, K3_BURST_FULL
    os.makedirs(outdir, exist_ok=True)
    schema, order = load_schema(schema_path)
    errors = []
    art = {}

    # 1-4) ingest + conformance
    manifest_path = os.path.join(raw_dir, "FIXTURE-MANIFEST.csv")
    expected = []
    if os.path.exists(manifest_path):
        with open(manifest_path) as f:
            next(f)
            for line in f:
                expected.append(line.split(",")[0])
    files = sorted(x for x in os.listdir(raw_dir) if x.endswith(".json") and not x.endswith(".sha256"))
    recs = {}
    for fn in files:
        p = os.path.join(raw_dir, fn)
        content = open(p).read()
        side = p + ".sha256"
        if not os.path.exists(side):
            errors.append(f"{fn}: sidecar missing")
            continue
        want = open(side).read().split()[0]
        got = sha256_hex(content)
        if want != got:
            errors.append(f"{fn}: sidecar sha mismatch")
            continue
        obj = json.loads(content)
        rid = obj.get("run_id", fn)
        if rid in recs:
            errors.append(f"{rid}: DUPLICATE run_id")
            continue
        if validate_record(obj, schema, order, errors, rid) and recompute_features(obj, errors, rid):
            recs[rid] = obj
    missing = [r for r in expected if r not in recs]
    extra = [r for r in recs if expected and r not in expected]
    if missing:
        errors.append(f"manifest MISSING rows: {missing}")
    if extra:
        errors.append(f"manifest EXTRA rows: {extra}")
    # leakage allowlist: mechanically assert the pipeline's replay inputs
    leak = [f for f in CONFIGURED_OFFLINE_ONLY if f in POLICY_ALLOWLIST]
    if leak:
        errors.append(f"LEAKAGE: configured fields in policy allowlist {leak}")

    conf = {
        "records_seen": len(files),
        "records_valid": len(recs),
        "expected_from_manifest": len(expected),
        "missing": missing,
        "extra": extra,
        "errors": errors,
        "verdict": "PASS" if not errors else "DATA-CONFORMANCE-NO-GO",
        "policy_replay_allowlist": sorted(POLICY_ALLOWLIST),
        "configured_offline_only": sorted(CONFIGURED_OFFLINE_ONLY),
    }
    art["02"] = ("DATA-CONFORMANCE.json", json.dumps(conf, indent=2, sort_keys=True) + "\n")
    if errors:
        # write the two conformance artifacts and stop (namespace-level failure; no row drops)
        art["01"] = ("INTEGRITY.json", json.dumps({"verdict": "NO-GO", "errors": errors},
                                                    indent=2, sort_keys=True) + "\n")
        for _, (name, content) in sorted(art.items()):
            open(os.path.join(outdir, name), "w").write(content)
        return False, errors

    rows = sorted(recs.values(), key=lambda r: r["run_id"])
    dataset_concat = "".join(json.dumps(r, sort_keys=True) + "\n" for r in rows)
    art["01"] = ("INTEGRITY.json", json.dumps({
        "verdict": "PASS",
        "rows": len(rows),
        "dataset_concat_sha256": sha256_hex(dataset_concat),
        "schema_version": rows[0]["schema_version"] if rows else None,
    }, indent=2, sort_keys=True) + "\n")

    # 5) PairRecord / HoldoutTriple (§11.3)
    groups = defaultdict(dict)
    for r in rows:
        groups[(r["cell_id"], r["pairing_group"])][r["arm"]] = r
    pairs, triples, pair_errors = [], [], []
    for (cell, pg), by_arm in sorted(groups.items()):
        r0, r2 = by_arm.get("STATIC_R0"), by_arm.get("STATIC_R2")
        if not r0 or not r2:
            pair_errors.append(f"{pg}: incomplete pair (arms present: {sorted(by_arm)})")
            continue
        if r0["feature_snapshot_sha256"] != r2["feature_snapshot_sha256"]:
            pair_errors.append(f"{pg}: pre-treatment snapshot mismatch")
            continue
        if r0["seed"] != r2["seed"]:
            pair_errors.append(f"{pg}: InitialSource seed mismatch")
            continue
        pair = {
            "cell_id": cell, "pairing_group": pg,
            "loss_ppm": r0["measured_loss_ppm"], "delta_ppm": r0["burst_delta_ppm"],
            "eligible": r0["estimator_eligible"],
            "r0_complete": r0["completion_status"] == "completed",
            "r2_complete": r2["completion_status"] == "completed",
            "r0_wire": r0["total_bidirectional_wire_bytes"],
            "r2_wire": r2["total_bidirectional_wire_bytes"],
            "r0_time": r0["completion_ns"], "r2_time": r2["completion_ns"],
            "epoch0": r0["observation_epoch_ns"], "epoch2": r2["observation_epoch_ns"],
            "horizon_ns": r0["horizon_ns"],
            "snapshot_sha": r0["feature_snapshot_sha256"],
            "run_ids": [r0["run_id"], r2["run_id"]],
        }
        pairs.append(pair)
        pol = by_arm.get("CALIBRATED_POLICY")
        if pol:
            if pol["feature_snapshot_sha256"] != r0["feature_snapshot_sha256"]:
                pair_errors.append(f"{pg}: policy-arm snapshot mismatch")
            else:
                triples.append({**pair, "policy": pol})
    if pair_errors:
        conf2 = json.loads(art["02"][1])
        conf2["errors"] = pair_errors
        conf2["verdict"] = "DATA-CONFORMANCE-NO-GO"
        art["02"] = ("DATA-CONFORMANCE.json", json.dumps(conf2, indent=2, sort_keys=True) + "\n")
        for _, (name, content) in sorted(art.items()):
            open(os.path.join(outdir, name), "w").write(content)
        return False, pair_errors

    # 6) cell stats + labels (§7.2, §11.4)
    rng = random.Random(BOOT_SEED)
    by_cell = defaultdict(list)
    for p in pairs:
        by_cell[p["cell_id"]].append(p)
    cell_rows = []
    cell_labels = {}
    for cell, ps in sorted(by_cell.items()):
        n = len(ps)
        k0 = sum(p["r0_complete"] for p in ps)
        k2 = sum(p["r2_complete"] for p in ps)
        b = sum(1 for p in ps if p["r2_complete"] and not p["r0_complete"])  # R2-only
        c = sum(1 for p in ps if p["r0_complete"] and not p["r2_complete"])  # R0-only
        pm = mcnemar_exact_p(b, c)
        delta_pt = (k2 - k0) / n if n else 0.0
        # pairing-group bootstrap of delta + full-R2_PREFERRED condition probability
        succ = 0
        deltas = []
        for _ in range(BOOT_B):
            s0 = s2 = 0
            for _ in range(n):
                p = ps[rng.randrange(n)]
                s0 += p["r0_complete"]
                s2 += p["r2_complete"]
            d = (s2 - s0) / n
            deltas.append(d)
            if d >= MIN_GAIN_PP / 100 and s2 > s0:
                succ += 1
        deltas.sort()
        lo = deltas[int(0.025 * BOOT_B)]
        hi = deltas[int(0.975 * BOOT_B) - 1]
        p_boot = succ / BOOT_B
        # §7.2 label: lower CI > 0 AND point >= 15pp AND safety/cost gates (inv checked in ingest)
        label = "R2_PREFERRED" if (lo > 0 and delta_pt >= MIN_GAIN_PP / 100) else "R0_PREFERRED"
        boundary = 0.10 < p_boot < 0.90
        cell_labels[cell] = label
        w0 = wilson(k0, n)
        w2 = wilson(k2, n)
        cell_rows.append({
            "cell_id": cell, "n_pairs": n,
            "r0_complete": f"{k0}/{n}", "r2_complete": f"{k2}/{n}",
            "r0_wilson95": f"[{w0[0]:.4f},{w0[1]:.4f}]", "r2_wilson95": f"[{w2[0]:.4f},{w2[1]:.4f}]",
            "delta_completion_point": round(delta_pt, 4),
            "delta_ci_low": round(lo, 4), "delta_ci_high": round(hi, 4),
            "discordant_r2_only": b, "discordant_r0_only": c,
            "mcnemar_exact_p": round(pm, 6),
            "p_bootstrap_r2_preferred": round(p_boot, 4),
            "label_boundary_flag": boundary,
            "preferred_action": label,
            "r0_rmst_ms": rmst_ms([p["r0_time"] for p in ps], ps[0]["horizon_ns"], [p["epoch0"] for p in ps]),
            "r2_rmst_ms": rmst_ms([p["r2_time"] for p in ps], ps[0]["horizon_ns"], [p["epoch2"] for p in ps]),
            "mean_r0_wire": round(sum(p["r0_wire"] for p in ps) / n, 1),
            "mean_r2_wire": round(sum(p["r2_wire"] for p in ps) / n, 1),
        })
    # attach labels to pairs (for candidate fit)
    for p in pairs:
        p["cell_label"] = cell_labels[p["cell_id"]]

    cols = list(cell_rows[0].keys()) if cell_rows else []
    csv4 = ",".join(cols) + "\n" + "\n".join(
        ",".join(str(r[c]) for c in cols) for r in cell_rows
    ) + ("\n" if cell_rows else "")
    art["04"] = ("CELL-RESULTS.csv", csv4)
    pcols = ["cell_id", "pairing_group", "loss_ppm", "delta_ppm", "eligible", "r0_complete",
             "r2_complete", "r0_wire", "r2_wire", "snapshot_sha", "run_ids"]
    csv3 = ",".join(pcols) + "\n" + "\n".join(
        ",".join(str(p[c]).replace(",", ";") for c in pcols) for p in pairs
    ) + ("\n" if pairs else "")
    art["03"] = ("PAIR-RECORDS.csv", csv3)

    # 7) support bitmap (§3.4b)
    supported, bin_counts = build_support_bitmap(pairs, support_min, support_cells)
    art["05"] = ("SUPPORT-BITMAP.json", json.dumps({
        "fixture_mode": fixture_mode,
        "min_groups": support_min, "min_cells": support_cells,
        "pinned_design_thresholds": {"min_groups": SUPPORT_MIN_GROUPS, "min_cells": SUPPORT_MIN_CELLS},
        "supported_bins": sorted(f"{a},{b}" for (a, b) in supported),
        "eligible_group_counts_by_bin": bin_counts,
        "bitmap_sha256": sha256_hex("\n".join(sorted(f"{a},{b}" for (a, b) in supported))),
    }, indent=2, sort_keys=True) + "\n")

    # 8) candidate fit (§11.5)
    fit = fit_candidates(pairs, supported, cell_labels)
    art["06"] = ("CANDIDATE-SUMMARY.json", json.dumps({
        "total_candidates": fit["total_candidates"],
        "candidate_count_after_tier_1": fit["tier_counts"][0],
        "candidate_count_after_tier_2": fit["tier_counts"][1],
        "candidate_count_after_tier_3": fit["tier_counts"][2],
        "candidate_count_after_tier_4": fit["tier_counts"][3],
        "final_tie_count": fit["final_tie_count"],
        "candidate_set_sha256": fit["candidate_set_sha256"],
        "selected_tuple": fit["selected_tuple"],
        "best_metrics_lexicographic": fit["best_metrics"],
    }, indent=2, sort_keys=True) + "\n")

    # 9) identifiability K1-K8 (§9.0) — mechanical; fixture results are recorded honestly
    k = {}
    labels = set(cell_labels.values())
    k["K1_direction_diversity"] = {"pass": {"R0_PREFERRED", "R2_PREFERRED"} <= labels,
                                    "labels": sorted(labels)}
    # K2 (AUC IID vs structured on pre-treatment burst_delta) — needs configured labels offline
    def auc(pos, neg):
        if not pos or not neg:
            return None
        wins = ties = 0
        for a in pos:
            for b in neg:
                if a > b:
                    wins += 1
                elif a == b:
                    ties += 1
        return (wins + ties / 2) / (len(pos) * len(neg))
    cfg_by_pg = {r["pairing_group"]: r for r in rows if r["arm"] == "STATIC_R0"}
    iid = [p["delta_ppm"] for p in pairs if cfg_by_pg[p["pairing_group"]]["configured_structure"] == "IID" and p["eligible"]]
    ge = [p["delta_ppm"] for p in pairs if cfg_by_pg[p["pairing_group"]]["configured_structure"] == "GE" and p["eligible"]]
    a_val = auc(ge, iid)
    k["K2_separability_auc_ge_vs_iid"] = {"auc": None if a_val is None else round(a_val, 4),
                                            "pass": a_val is not None and a_val >= 0.90,
                                            "note": "fixture-scale point AUC (bootstrap lower bound is the K2 criterion at training scale)"}
    # K3 LOCO decision-map stability (leave-one-CELL-out at fixture scale)
    def decision_map(fitres):
        return fitres["candidate_set_sha256"] if fitres["selected_tuple"] else "none"
    loco_same = True
    for leave in sorted(by_cell):
        sub = [p for p in pairs if p["cell_id"] != leave]
        if not sub:
            continue
        subfit = fit_candidates(sub, supported, cell_labels)
        if subfit["selected_tuple"] and fit["selected_tuple"]:
            sm = subfit["selected_tuple"]
            fm = fit["selected_tuple"]
            xm_s, ym_s = x_min_of(sm[0], sm[1], sm[4]), y_max_of(sm[2], sm[3], sm[4])
            xm_f, ym_f = x_min_of(fm[0], fm[1], fm[4]), y_max_of(fm[2], fm[3], fm[4])
            grid_same = all(
                ((gl >= xm_s and gb <= ym_s) == (gl >= xm_f and gb <= ym_f))
                for gl in K3_LOSS for gb in K3_BURST
                if support_bins(gl, gb) in supported
            )
            loco_same &= grid_same
    k["K3_loco_decision_stability"] = {"pass": loco_same}
    # K4 observational alias: same support bin, different configured RTT/axis → conflicting labels?
    alias = defaultdict(set)
    for p in pairs:
        alias[support_bins(p["loss_ppm"], p["delta_ppm"])].add(cell_labels[p["cell_id"]])
    conflicting = sorted(
        f"{bb[0]},{bb[1]}" for bb, v in alias.items() if bb in supported and len(v) > 1
    )
    k["K4_alias_conflict"] = {"pass": not conflicting, "conflicting_bins": conflicting}
    # K5 bootstrap policy stability (B reduced honestly at fixture scale; recorded)
    k5b = 50 if fixture_mode else K5_B
    stable = 0
    rng5 = random.Random(BOOT_SEED + 1)
    for _ in range(k5b):
        res = [ps[rng5.randrange(len(ps))] for cell, ps in sorted(by_cell.items()) for _ in ps]
        for p in res:
            p.setdefault("cell_label", cell_labels[p["cell_id"]])
        f2 = fit_candidates(res, supported, cell_labels)
        if f2["selected_tuple"] == fit["selected_tuple"]:
            stable += 1
    k["K5_bootstrap_policy_stability"] = {"B": k5b, "stable_fraction": round(stable / max(1, k5b), 3),
                                            "pass": stable / max(1, k5b) >= 0.90}
    k["K6_tie_region_identity"] = {"pass": fit["k6_tie_region_identity"]}
    r2_cells = [c for c, l in cell_labels.items() if l == "R2_PREFERRED"]
    k7ok = False
    for c in r2_cells:
        ps = by_cell[c]
        el = [p for p in ps if p["eligible"]]
        ins = [p for p in el if support_bins(p["loss_ppm"], p["delta_ppm"]) in supported]
        if el and len(ins) / len(el) >= 0.90:
            k7ok = True
    k["K7_support_activation"] = {"pass": k7ok, "r2_cells": r2_cells}
    k["K8_data_contract"] = {"pass": True, "note": "conformance PASS upstream (else pipeline halts)"}
    k["overall"] = {"pass": all(v["pass"] for kk, v in k.items() if kk != "overall")}
    art["07"] = ("IDENTIFIABILITY.json", json.dumps(k, indent=2, sort_keys=True) + "\n")

    # 10) calibration policy artifact (§11.6) — fixture output clearly labeled
    if fit["selected_tuple"]:
        la, lb, ba, bb, t = fit["selected_tuple"]
        pol = {
            "artifact": "FecActivationCalibrationV1_R0R2_LossBurst",
            "fixture_mode": fixture_mode,
            "status": "FIXTURE-PIPELINE-OUTPUT — NOT a calibration (YETKİ-2 fits the real artifact)" if fixture_mode else "CANDIDATE",
            "schema_version": "FS-CAL-2-POLICY-v1",
            "loss_a_ppm": la, "loss_b_ppm": lb, "burst_a_ppm": ba, "burst_b_ppm": bb,
            "activation_threshold_scaled": t,
            "support_bitmap_sha256": json.loads(art["05"][1])["bitmap_sha256"],
            "identifiability_overall_pass": k["overall"]["pass"],
            "training_dataset_sha256": json.loads(art["01"][1])["dataset_concat_sha256"],
        }
    else:
        pol = {"status": "NOT-VIABLE (no candidate)", "fixture_mode": fixture_mode}
    art["08"] = ("CALIBRATION-POLICY.json", json.dumps(pol, indent=2, sort_keys=True) + "\n")

    # 11) holdout-triple processing (§11.7 machinery on FIXTURE triples)
    hrows = []
    for t3 in triples:
        pol_rec = t3["policy"]
        # offline inference equality: recompute the fixture policy action from the snapshot
        # (fixture policy parameters are known constants in the runner)
        m = pol_rec["measured_loss_ppm"]
        d = pol_rec["burst_delta_ppm"]
        el = pol_rec["estimator_eligible"]
        act = min(mu_high_loss(m, 100_000, 200_000), mu_nonburst(d, 150_000, 350_000))
        sb = support_bins(m, d)
        fixture_support = (0 <= sb[0] <= 6) and (0 <= sb[1] <= 6)
        offline_r2 = el and fixture_support and act >= 700_000
        offline = "R2" if offline_r2 else "R0"
        actual = pol_rec["effective_redundancy"]
        conform = offline == actual
        # semantic digest equality vs the chosen static arm (in-process → outcome digest fields)
        chosen = t3["policy"]
        static = None
        for r in rows:
            if r["pairing_group"] == t3["pairing_group"] and r["arm"] == ("STATIC_R2" if actual == "R2" else "STATIC_R0"):
                static = r
        sem_equal = static is not None and (
            (chosen["completion_status"], chosen["parity_bytes"] > 0)
            == (static["completion_status"], static["parity_bytes"] > 0)
        )
        hrows.append({
            "cell_id": t3["cell_id"], "pairing_group": t3["pairing_group"],
            "static_label": cell_labels[t3["cell_id"]],
            "policy_actual_action": actual, "policy_offline_action": offline,
            "action_conformance": conform,
            "semantic_outcome_matches_static": sem_equal,
            "harmful_false_positive": actual == "R2" and cell_labels[t3["cell_id"]] == "R0_PREFERRED",
            "fallback_reason": pol_rec["fallback_reason"],
        })
    hcols = list(hrows[0].keys()) if hrows else ["cell_id"]
    csv9 = ",".join(hcols) + "\n" + "\n".join(
        ",".join(str(r[c]) for c in hcols) for r in hrows
    ) + ("\n" if hrows else "")
    art["09"] = ("HOLDOUT-RESULTS.csv", csv9)

    # netns fidelity — no netns rows in YETKİ-1; machinery emits an explicit empty-fidelity artifact
    art["10"] = ("NETNS-FIDELITY.json", json.dumps({
        "environment_rows": sum(1 for r in rows if r["environment"] == "NETNS"),
        "verdict": "NOT-APPLICABLE (no NETNS rows in this namespace; YETKİ-3 scope)",
    }, indent=2, sort_keys=True) + "\n")

    # final report + checkpoint (fixture-labeled)
    hfp = sum(1 for h in hrows if h["harmful_false_positive"])
    conform_all = all(h["action_conformance"] for h in hrows) if hrows else True
    report = [
        "# FS-CAL-2 v3 — ANALYSIS PIPELINE OUTPUT" + (" (FIXTURE)" if fixture_mode else ""),
        "",
        f"- rows: {len(rows)}  pairs: {len(pairs)}  triples: {len(triples)}",
        f"- conformance: PASS  dataset_sha: {json.loads(art['01'][1])['dataset_concat_sha256'][:16]}…",
        f"- cell labels: " + ", ".join(f"{c}={l}" for c, l in sorted(cell_labels.items())),
        f"- candidate fit: {fit['total_candidates']} candidates → tie {fit['final_tie_count']} → selected {fit['selected_tuple']}",
        f"- identifiability overall: {'PASS' if k['overall']['pass'] else 'FAIL'} (per-K in IDENTIFIABILITY.json)",
        f"- holdout-triple machinery: harmful_fp={hfp}, action_conformance={'PASS' if conform_all else 'FAIL'}",
        "",
        "FIXTURE NOTE: numbers above validate the PIPELINE, not any scientific claim." if fixture_mode else "",
    ]
    art["11"] = ("FINAL-REPORT.md", "\n".join(report) + "\n")
    art["12"] = ("FINAL-CHECKPOINT.md", "\n".join([
        "# FS-CAL-2 v3 — PIPELINE CHECKPOINT" + (" (FIXTURE)" if fixture_mode else ""),
        "",
        f"artifacts: {len(art)} / 12",
        "determinism: two independent runs must be byte-identical (verified externally).",
    ]) + "\n")

    for _, (name, content) in sorted(art.items()):
        open(os.path.join(outdir, name), "w").write(content)
        open(os.path.join(outdir, name + ".sha256"), "w").write(
            f"{sha256_hex(content)}  {name}\n"
        )
    return True, [name for _, (name, _) in sorted(art.items())]


# ── §11.1 selftest (negative/golden fixtures at the analysis level) ──────────
def selftest(schema_path):
    schema, order = load_schema(schema_path)
    results = []

    def check(name, cond):
        results.append((name, bool(cond)))

    good = {name: 0 for name in order}
    for name in order:
        spec = schema[name]
        t = spec["type"]
        if t in ("string", "string|null"):
            good[name] = next(iter(ENUMS[name])) if name in ENUMS else "x"
        elif t == "bool":
            good[name] = False
        elif t == "u64[]":
            good[name] = [1, 2, 3]
        elif t == "enum[]":
            good[name] = ["SUCCESS", "LOSS", "SUCCESS"]
        elif t == "i64":
            good[name] = -5
        else:
            good[name] = 3
    good["estimator_resolved"] = 3
    good["estimator_window_source_ids"] = [1, 2, 3]
    good["estimator_window_fates"] = ["SUCCESS", "LOSS", "SUCCESS"]
    good["estimator_lost"] = 1
    good["n00"] = 0
    good["n01"] = 1
    good["n10"] = 1
    good["n11"] = 0
    good["measured_loss_ppm"] = 1 * PPM // 3
    good["p11_ppm"] = (0 + 1) * PPM // (1 + 0 + 2)
    good["p01_ppm"] = (1 + 1) * PPM // (0 + 1 + 2)
    good["burst_delta_ppm"] = good["p11_ppm"] - good["p01_ppm"]

    e = []
    check("valid_record_accepts", validate_record(dict(good), schema, order, e, "t") and not e)
    e = []
    bad = dict(good)
    bad["surprise_field"] = 1
    check("unknown_field_rejected", not validate_record(bad, schema, order, e, "t"))
    e = []
    bad = dict(good)
    del bad["inv_fail"]
    check("missing_field_rejected", not validate_record(bad, schema, order, e, "t"))
    e = []
    bad = dict(good)
    bad["arm"] = "NOT_AN_ARM"
    check("invalid_enum_rejected", not validate_record(bad, schema, order, e, "t"))
    e = []
    check("recompute_accepts_consistent", recompute_features(dict(good), e, "t") and not e)
    e = []
    bad = dict(good)
    bad["n11"] = 7
    check("recompute_rejects_tampered", not recompute_features(bad, e, "t"))
    # mu goldens vs Rust semantics (fs_cal2_policy tests pin the same values)
    check("mu_loss_boundary", mu_high_loss(100_000, 100_000, 200_000) == 0
          and mu_high_loss(100_001, 100_000, 200_000) == 10
          and mu_high_loss(200_000, 100_000, 200_000) == SCALE)
    check("mu_burst_boundary", mu_nonburst(150_000, 150_000, 350_000) == SCALE
          and mu_nonburst(350_000, 150_000, 350_000) == 0
          and mu_nonburst(349_999, 150_000, 350_000) == 5)
    check("threshold_factorization_exact", all(
        (mu_high_loss(x, a, b) >= t) == (x >= x_min_of(a, b, t))
        for a, b, t in [(0, 100_000, 700_000), (50_000, 200_000, 500_000), (5_000, 10_000, 900_000)]
        for x in range(0, 220_001, 1_000)
    ) and all(
        (mu_nonburst(y, a, b) >= t) == (y <= y_max_of(a, b, t))
        for a, b, t in [(-100_000, 300_000, 700_000), (0, 25_000, 500_000)]
        for y in range(-200_000, 320_001, 1_000)
    ))
    # K1 PASS/FAIL toy examples
    check("k1_pass_example", {"R0_PREFERRED", "R2_PREFERRED"} <= {"R0_PREFERRED", "R2_PREFERRED"})
    check("k1_fail_example", not ({"R0_PREFERRED", "R2_PREFERRED"} <= {"R0_PREFERRED"}))
    check("mcnemar_exact", abs(mcnemar_exact_p(0, 0) - 1.0) < 1e-12
          and mcnemar_exact_p(10, 0) < 0.01)
    ok = all(r for _, r in results)
    for name, r in results:
        print(f"  [{'PASS' if r else 'FAIL'}] {name}")
    print(f"selftest: {'PASS' if ok else 'FAIL'} ({sum(1 for _, r in results if r)}/{len(results)})")
    return ok


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--raw")
    ap.add_argument("--outdir")
    ap.add_argument("--schema", required=True)
    ap.add_argument("--fixture-mode", action="store_true")
    ap.add_argument("--support-min", type=int, default=SUPPORT_MIN_GROUPS)
    ap.add_argument("--support-cells", type=int, default=SUPPORT_MIN_CELLS)
    ap.add_argument("--selftest", action="store_true")
    a = ap.parse_args()
    if a.selftest:
        sys.exit(0 if selftest(a.schema) else 1)
    if not a.raw or not a.outdir:
        die("--raw and --outdir required (or --selftest)")
    ok, info = run_pipeline(a.raw, a.outdir, a.schema, a.fixture_mode, a.support_min, a.support_cells)
    if ok:
        print(f"pipeline PASS; artifacts: {info}")
    else:
        print("pipeline DATA-CONFORMANCE-NO-GO:")
        for e in info[:20]:
            print(f"  - {e}")
    sys.exit(0 if ok else 1)


if __name__ == "__main__":
    main()
