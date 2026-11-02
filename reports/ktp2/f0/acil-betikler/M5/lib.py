import gzip, json, os
CELL = "/home/ryuzaki/kama-k4g-work/ktp2-f0/runs/tam/b/out/b_r12k_d1000_l30_p1000"

def pct(v, q):
    v = sorted(v)
    if not v: return float('nan')
    k = (len(v)-1)*q
    f = int(k); c = min(f+1, len(v)-1)
    return v[f] + (v[c]-v[f])*(k-f)

def read_out(rep):
    meta = None; ev = []
    with open(os.path.join(rep, "out.jsonl")) as f:
        for line in f:
            o = json.loads(line)
            if "meta" in o: meta = o["meta"]
            elif "event" in o: ev.append(o["event"])
    return meta, ev

def read_rx(rep):
    d = {}
    with open(os.path.join(rep, "rx_log.jsonl")) as f:
        for line in f:
            o = json.loads(line)
            if "u" in o:
                d.setdefault(o["u"], o["t"])
    return d

def read_cap(path):
    rows = []
    with gzip.open(path, "rt") as f:
        for line in f:
            if line.startswith("#"): continue
            p = line.rstrip("\n").split(",")
            if len(p) < 7: continue
            rows.append((int(p[0]), p[1], int(p[2]), int(p[3]), int(p[4]), int(p[5]), p[6]))
    return rows

FT = {0x00: "data", 0x05: "data_frag", 0x01: "fec", 0x02: "recovered", 0x03: "param",
      0x04: "param_ack", 0x06: "cs_probe", 0x07: "cs_echo", 0x10: "ack", 0x14: "ack"}

def kdecode(x):
    if ":" in x:
        h, t = x.split(":")
        head = bytes.fromhex(h); tail = bytes.fromhex(t); frame = head; full = None
    else:
        b = bytes.fromhex(x); tail = b[-16:]; frame = b[:-16]; full = frame
    if len(tail) < 16 or not frame: return None
    d = {"pn": int.from_bytes(tail[:8], "big"), "ft": frame[0], "type": FT.get(frame[0], "other")}
    if d["type"] in ("data", "data_frag") and len(frame) >= 11:
        d["seq"] = int.from_bytes(frame[1:3], "big")
        d["unit"] = int.from_bytes(frame[3:11], "big")
    if d["type"] == "ack" and full is not None:
        fr = full
        d["largest"] = int.from_bytes(fr[2:4], "big")
        d["ack_delay"] = int.from_bytes(fr[4:6], "big")
        b0 = int.from_bytes(fr[6:8], "big")
        n = fr[8]
        rngs = []
        for i in range(n):
            o = 9 + 4*i
            rngs.append((int.from_bytes(fr[o:o+2], "big"), int.from_bytes(fr[o+2:o+4], "big")))
        d["b0"] = b0; d["ranges"] = rngs
        o = 9 + 4*n
        if fr[0] == 0x14 and len(fr) >= o+6:
            d["seen"] = int.from_bytes(fr[o:o+4], "big"); d["arr_rate"] = int.from_bytes(fr[o+4:o+6], "big")
    return d

def ack_low16_set(d):
    # returns list of u16 PN acked (low16), descending reconstruct
    out = []
    L = d["largest"]; b0 = d["b0"]
    if b0 == 0: return out
    cur = (L - b0 + 1)
    for s in range(cur, L+1): out.append(s & 0xFFFF)
    for gap, ln in d["ranges"]:
        cur -= gap
        if ln == 0: continue
        lo = cur - ln + 1
        for s in range(lo, cur+1): out.append(s & 0xFFFF)
        cur -= ln
    return out
