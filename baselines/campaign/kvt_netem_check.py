#!/usr/bin/env python3
"""Fail-closed netem doğrulaması: `tc -j qdisc show dev X` çıktısı BEKLENEN spec'i taşıyor mu?

Kullanım: kvt_netem_check.py '<tc -j json>' '<spec>' [seed]   (spec örn. "delay 25ms loss 10% rate 1mbit")
Çıkış 0 = eşleşti; 1 = eşleşmedi (satır ÜRETİLMEZ). stdout: netem `limit` değeri.
"""
import json, sys

def parse_spec(spec):
    t = spec.split(); want = {"delay": 0.0, "loss": 0.0, "rate": 0}
    i = 0
    while i < len(t):
        k = t[i]
        if k == "delay":
            want["delay"] = float(t[i + 1].rstrip("ms")) / 1000.0; i += 2
        elif k == "loss":
            want["loss"] = float(t[i + 1].rstrip("%")) / 100.0; i += 2
        elif k == "rate":
            v = t[i + 1].lower()
            mul = 1
            for suf, m in (("kbit", 1000), ("mbit", 1000_000), ("bit", 1)):
                if v.endswith(suf):
                    mul = m; v = v[: -len(suf)]; break
            want["rate"] = int(float(v) * mul / 8); i += 2   # tc JSON: bayt/s
        else:
            raise SystemExit(f"bilinmeyen netem belirteci: {k}")
    return want

def main():
    q = [x for x in json.loads(sys.argv[1]) if x.get("kind") == "netem"]
    if len(q) != 1:
        print("netem qdisc yok/çoklu", file=sys.stderr); sys.exit(1)
    o = q[0]["options"]; want = parse_spec(sys.argv[2])
    got = {
        "delay": float((o.get("delay") or {}).get("delay", 0.0)),
        "loss": float((o.get("loss-random") or {}).get("loss", 0.0)),
        "rate": int((o.get("rate") or {}).get("rate", 0)),
    }
    ok = (abs(got["delay"] - want["delay"]) < 1e-6 and abs(got["loss"] - want["loss"]) < 1e-4
          and abs(got["rate"] - want["rate"]) <= max(1, want["rate"] // 1000))
    if len(sys.argv) > 3 and str(o.get("seed")) != sys.argv[3]:
        print(f"netem seed uyuşmazlığı: beklenen={sys.argv[3]} gerçekleşen={o.get('seed')}", file=sys.stderr); sys.exit(1)
    if not ok:
        print(f"netem uyuşmazlığı: beklenen={want} gerçekleşen={got}", file=sys.stderr); sys.exit(1)
    print(o.get("limit", -1))

main()
