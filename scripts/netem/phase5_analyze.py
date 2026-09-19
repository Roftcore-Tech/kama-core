#!/usr/bin/env python3
"""[FAZ-5] Hücre-başına toplama.

İSTATİSTİK BURADA YENİDEN TÜRETİLMEZ: medyan/IQR/CI, blackout, stall, sonuç sınıfları ve
hüküm sınıfı DONMUŞ `bench metrics` uygulamasından okunur (tek doğruluk kaynağı, Faz-4).
Bu betik yalnız (a) o çıktıyı ayrıştırır, (b) manifestlerden aktivasyon/kimlik toplar,
(c) `bench fairness` ile kıyas düzeyini belirler, (d) markdown tablo üretir.
"""
import json, pathlib, re, subprocess, sys, statistics

REPO = pathlib.Path(__file__).resolve().parents[2]
BENCH = REPO / "target/release/bench"
ARMS = ["kama-product", "quinn", "tcp"]
LABEL = {"kama-product": "kama-v1-full", "quinn": "quinn-reliable-stream", "tcp": "tcp-default"}


def metrics(runs, deadlines, setup_view="steady-state"):
    if not runs:
        return {}
    cmd = [str(BENCH), "metrics", "--runs", *[str(r) for r in runs],
           "--deadlines-s", deadlines, "--setup-view", setup_view]
    out = subprocess.run(cmd, capture_output=True, text=True).stdout
    d = {"raw": out, "deadline": {}, "wire": []}
    for ln in out.splitlines():
        m = re.match(r"deadline=\s*(\d+)s\s+median=([\d.]+)\s+IQR=\[([\d.]+),([\d.]+)\]"
                     r"\s+min=([\d.]+)\s+max=([\d.]+)\s+\(n=(\d+)\)", ln)
        if m:
            d["deadline"][int(m.group(1))] = dict(
                median=float(m.group(2)), q1=float(m.group(3)), q3=float(m.group(4)),
                lo=float(m.group(5)), hi=float(m.group(6)), n=int(m.group(7)))
        if ln.startswith("# outcomes"):
            d["outcomes"] = ln.split(":", 1)[1].strip()
        if ln.startswith("# blackout"):
            d["blackout"] = ln.split(":", 1)[1].strip()
        if ln.startswith("# all-runs"):
            d["all_runs"] = ln.split(":", 1)[1].strip()
        if ln.startswith("# stall-free"):
            d["stall_free"] = ln.split(":", 1)[1].strip()
        if ln.startswith("# stall incidence"):
            d["stall"] = ln.split(":", 1)[1].strip()
        if ln.startswith("# claim class"):
            d["claim"] = ln.split(":", 1)[1].strip()
        if ln.startswith("# wire "):
            d["wire"].append(ln)
    return d


def collect(cell_dir):
    """Bir hücrenin kol-başına koşularını ve manifestlerini topla."""
    res = {}
    for arm in ARMS:
        adir = cell_dir / arm
        if not adir.is_dir():
            continue
        runs, mans, cerrs, rcs = [], [], [], []
        for rep in sorted(adir.glob("rep*"), key=lambda p: int(p.name[3:])):
            j, mf, ce, rc = rep/"out.jsonl", rep/"manifest.json", rep/"c.err", rep/"rc"
            if j.exists():
                runs.append(j)
            if mf.exists():
                try:
                    mans.append(json.loads(mf.read_text()))
                except Exception:
                    pass
            if ce.exists():
                cerrs.append(ce.read_text(errors="replace"))
            if rc.exists():
                rcs.append(rc.read_text().strip())
        res[arm] = dict(runs=runs, manifests=mans, cerrs=cerrs, rcs=rcs)
    return res


def activation(mans):
    """Aktivasyon sayaçlarının kol-içi medyanı (kütle değil, TİPİK koşu)."""
    if not mans:
        return {}
    keys = ["parity_emitted", "parity_token_denied", "retransmits", "dups", "tail_probes",
            "state_transitions", "recover_arq_entries", "btlbw_bps", "srtt_us", "rtprop_us",
            "return_forward_loss_permille", "return_ack_loss_permille",
            "bytes_data", "bytes_retx", "bytes_parity"]
    out = {}
    for k in keys:
        vals = [m["activation"].get(k, 0) for m in mans if "activation" in m]
        out[k] = int(statistics.median(vals)) if vals else 0
    out["identity"] = sorted({m.get("arm", "?") for m in mans})
    out["delta_nonempty"] = sum(1 for m in mans if m.get("profile_delta"))
    return out


def fec2(cerrs):
    """stderr-only sayaçlar: clamp / would_zero / repair_not_arqed / partial_blocks."""
    agg = {"clamp": [], "would_zero": [], "repair_not_arqed": [], "partial_blocks": []}
    for t in cerrs:
        m = re.search(r"fec2=\[repair_not_arqed=(\d+) clamp=(\d+) would_zero=(\d+) partial_blocks=(\d+)\]", t)
        if m:
            agg["repair_not_arqed"].append(int(m.group(1)))
            agg["clamp"].append(int(m.group(2)))
            agg["would_zero"].append(int(m.group(3)))
            agg["partial_blocks"].append(int(m.group(4)))
    return {k: (int(statistics.median(v)) if v else 0, max(v) if v else 0) for k, v in agg.items()}


def fairness(cell_dir):
    mans = []
    for arm in ARMS:
        p = sorted((cell_dir/arm).glob("rep*/manifest.json")) if (cell_dir/arm).is_dir() else []
        if p:
            mans.append(str(p[0]))
    if len(mans) < 2:
        return "n/a", []
    r = subprocess.run([str(BENCH), "fairness", "--manifests", *mans], capture_output=True, text=True)
    lvl = {0: "NORMALIZED", 1: "NON-NORMALIZED", 2: "BLOCKING"}.get(r.returncode, f"rc={r.returncode}")
    notes = [l.strip() for l in r.stdout.splitlines() if l.strip().startswith(("BLOCKING", "semantics"))]
    return lvl, notes


def main():
    root = pathlib.Path(sys.argv[1])
    deadlines = sys.argv[2] if len(sys.argv) > 2 else "5,10,20"
    primary = int(deadlines.split(",")[-1])
    cells = sorted([p for p in root.iterdir() if p.is_dir() and (p/"cell.json").exists()],
                   key=lambda p: p.name)
    print(f"# FAZ-5 TOPLAMA — {root}\n")
    for cd in cells:
        cfg = json.loads((cd/"cell.json").read_text())
        done = (cd/"DONE").exists()
        print(f"\n## {cfg['cell']}  {'' if done else '(SHARD TAMAMLANMADI)'}")
        print(f"fwd=`{cfg['fwd']}`  rev=`{cfg['rev']}`  "
              f"geo={cfg['msgs']}x{cfg['payload']}B@{cfg['submit_interval_ms']}ms  D={cfg['deadline_s']}s")
        data = collect(cd)
        lvl, notes = fairness(cd)
        print(f"kıyas düzeyi: **{lvl}**")
        print(f"\n| kol | n | claim | completion(D={primary}) medyan | IQR | outcome | blackout | stall |")
        print("|---|---|---|---|---|---|---|---|")
        for arm in ARMS:
            if arm not in data or not data[arm]["runs"]:
                continue
            m = metrics(data[arm]["runs"], deadlines)
            dl = m.get("deadline", {}).get(primary, {})
            print(f"| {LABEL[arm]} | {dl.get('n','?')} | {m.get('claim','?')} | "
                  f"{dl.get('median',float('nan')):.3f} | "
                  f"[{dl.get('q1',float('nan')):.3f},{dl.get('q3',float('nan')):.3f}] | "
                  f"{m.get('outcomes','?')} | {m.get('blackout','?')} | {m.get('stall','?')} |")
        print()
        for arm in ARMS:
            if arm not in data or not data[arm]["manifests"]:
                continue
            a = activation(data[arm]["manifests"])
            if arm == "kama-product":
                f2 = fec2(data[arm]["cerrs"])
                print(f"- **{LABEL[arm]}** kimlik={a['identity']} sapmalı_koşu={a['delta_nonempty']} | "
                      f"parity={a['parity_emitted']} denied={a['parity_token_denied']} "
                      f"retx={a['retransmits']} dup={a['dups']} probes={a['tail_probes']} | "
                      f"btlbw={a['btlbw_bps']}B/s srtt={a['srtt_us']}us | "
                      f"fwd_loss={a['return_forward_loss_permille']}‰ "
                      f"ret_ack_loss={a['return_ack_loss_permille']}‰ | "
                      f"bytes data/retx/parity={a['bytes_data']}/{a['bytes_retx']}/{a['bytes_parity']} | "
                      f"fec2 clamp={f2['clamp']} would_zero={f2['would_zero']} (medyan,max)")
            else:
                print(f"- **{LABEL[arm]}** kimlik={a['identity']}")
        for n in notes[:4]:
            print(f"  - {n}")


if __name__ == "__main__":
    main()
