#!/usr/bin/env python3
"""[KTP2-F0 / Ajan B] r25 kök ayrıştırması: bayt payları ve ileri kuyruk yeniden oynatımı (salt okur).

Her r25 Kama koşusu için:
  1. Düzlem başına bayt (L2): deneme (va0 çıkış) ve hattan geçen (vb0 giriş). Sınıflar: orijinal DATA,
     kopya DATA (ilk iletim dışı her DATA), PARAM_SET.
  2. Sayaç düzlemi (UDP yükü, c.err): data/retx/dup/ctrl; kopyanın türü: kuruluş probu (establish_tx),
     kuyruk probu (tail_probes), geometrik/§3.3-B (dup − establish), ARQ (retx − tail_probes).
     Yol: PARAM motor dışı (pacer'a girmez), kuruluş probu jetonsuz, kalanlar pacer'dan geçer.
  3. Yeniden oynatım [türetilmiş, geri beslemesiz]: A çıkış dizisi sabit tutulur, netem FIFO'su
       tts_j = max(tts_önceki, t_j + d) + boy_j / R,  d = profil tek yön gecikmesi
     ile yeniden hesaplanır. Paketin netem kaderi ölçülendir (vb0'a vardıysa geçti; son varan
     paketten önce gönderilip varmadıysa düştü). Koşu bittiğinde kuyrukta kalan paketlerin kaderi
     bilinmez: onlara tohumlu Bernoulli(1 − p) atanır (hücre+kol+tekrar tohumu). Senaryolar:
       S0 hepsi (doğrulama: gerçek varışa mutlak hata) · S1 yalnız ilk PARAM_SET ·
       S2 teslim etmeyen kopyalar yok · S3 S1+S2 ·
       S4 PARAM_SET katlamalı (ölçülen 25 denemeden 0, 1, 3, 7, 15, 24. sıradakiler kalır: ≈ 0 / 0,2 / 0,6 /
       1,4 / 3,0 / 4,8 s; 200 ms × 2^k'nın ilk 5 s yaklaşığı) · S5 S4+S2.
     Çıktı: D içinde varış oranı, tek yön p50/p95. Kaldırılan paketin yol açacağı geri besleme
     değişimi (daha erken ACK, daha az prob) modellenmez.

Kullanım: python3 -I r25_kok.py <runs/tam/b/out> <çıktı.csv>
"""
import csv
import pathlib
import random
import sys

HERE = pathlib.Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.dont_write_bytecode = True
import analiz_b as A  # noqa: E402
import fizik  # noqa: E402

D_S = 60.0
R = 25.0


def replay(pkts, d):
    """pkts: zaman sıralı (t, l2, ok) → {indeks: varış}. ok=False netem'de düşen (hat harcamaz)."""
    tts = -1e18
    arr = {}
    for i, (t, l2, ok) in enumerate(pkts):
        if not ok:
            continue
        start = max(tts, t + d) if tts > t else t + d
        tts = start + l2 / R
        arr[i] = tts
    return arr


def run(rep, g, arm):
    cli, srv = A.read_cap(rep / "capb_cli.csv.gz"), A.read_cap(rep / "capb_srv.csv.gz")
    if cli is None or srv is None:
        return None
    meta, ev = A.read_events(rep)
    port = A.server_port(rep)
    t0, off = meta["t0_unix_ns"] / 1e9, meta.get("events_epoch_offset_ns", 0)
    offer = {e["msg_id"]: t0 + (e["sent_ns"] - off) / 1e9 for e in ev}

    def udp(rows, dr):
        out = []
        for t, d, pr, ipl, sp, dp, x in rows:
            if pr != 17 or d != dr or not x or dp != port:
                continue
            k = A.kama_decode(x, ipl - 28)
            if k is not None:
                k["t"], k["l2"] = t / 1e9, ipl + 14
                out.append(k)
        return out

    a_out, b_in = udp(cli, "o"), udp(srv, "i")
    b_t = {p["pn"]: p["t"] for p in b_in}
    last_ok = max((i for i, p in enumerate(a_out) if p["pn"] in b_t), default=-1)
    rng = random.Random(f"{g['cell']}/{arm}/{rep.name}")
    q = g["loss"] / 100.0
    fate = {}
    for i, p in enumerate(a_out):
        fate[p["pn"]] = (p["pn"] in b_t) if i <= last_ok else (rng.random() >= q)
    seen = set()
    for p in a_out:
        if p["type"] == "data":
            p["kls"] = "kopya" if p["unit"] in seen else "orijinal"
            seen.add(p["unit"])
        elif p["type"] == "param":
            p["kls"] = "param"
        else:
            p["kls"] = "diger"
    r = {"hucre": g["cell"], "kol": arm, "rep": int(rep.name[3:])}
    for pl, name in ((a_out, "den"), ([p for p in a_out if p["pn"] in b_t], "hat")):
        for kls in ("orijinal", "kopya", "param", "diger"):
            v = [p["l2"] for p in pl if p["kls"] == kls]
            r[f"{name}_{kls}_B"], r[f"{name}_{kls}_n"] = sum(v), len(v)
    c = A.parse_client_err(A.read_text(rep / "c.err"))
    r.update({k: c.get(k) for k in ("tx_total_B", "tx_data_B", "tx_retx_B", "tx_dup_B", "tx_ctrl_B", "establish_tx",
                                     "tail_probes", "retx", "dup", "left_preconv", "state_end", "rtt_samples")})
    # hangi kopya ilk varan?
    first = {}
    for p in a_out:
        if p["type"] == "data" and p["pn"] in b_t:
            u = p["unit"]
            if u not in first or b_t[p["pn"]] < b_t[first[u]]:
                first[u] = p["pn"]
    useful = set(first.values())
    for p in a_out:
        if p["type"] == "data" and p["unit"] not in first and fate[p["pn"]]:
            first[p["unit"]] = p["pn"]
            useful.add(p["pn"])
    first_param = next((p["pn"] for p in a_out if p["type"] == "param"), None)
    params = [p["pn"] for p in a_out if p["type"] == "param"]
    param_katla = {params[i] for i in (0, 1, 3, 7, 15, 24) if i < len(params)}
    d = g["delay_ms"] / 1000.0

    def scen(keep):
        pl = [p for p in a_out if keep(p)]
        arr = replay([(p["t"], p["l2"], fate[p["pn"]]) for p in pl], d)
        by = {}
        for i, p in enumerate(pl):
            if p["type"] == "data" and i in arr:
                u = p["unit"]
                by[u] = min(by.get(u, 1e18), arr[i])
        ow = [by[u] - offer[u] for u in offer if u in by]
        scen.by = by
        return arr, pl, ow

    arr, pl, ow = scen(lambda p: True)
    errs = [abs(arr[i] - b_t[p["pn"]]) for i, p in enumerate(pl) if i in arr and p["pn"] in b_t]
    sira = sorted(offer, key=offer.get)
    r["m2_tek_yon_s"] = (scen.by[sira[1]] - offer[sira[1]]) if len(sira) > 1 and sira[1] in scen.by else None
    r["dogrulama_hata_p50_s"] = A.pct(errs, .5)
    r["dogrulama_hata_p95_s"] = A.pct(errs, .95)
    n = len(offer)
    keepers = {
        "S0": lambda p: True,
        "S1": lambda p: p["type"] != "param" or p["pn"] == first_param,
        "S2": lambda p: p["kls"] != "kopya" or p["pn"] in useful,
        "S3": lambda p: (p["type"] != "param" or p["pn"] == first_param) and (p["kls"] != "kopya" or p["pn"] in useful),
        "S4": lambda p: p["type"] != "param" or p["pn"] in param_katla,
        "S5": lambda p: (p["type"] != "param" or p["pn"] in param_katla) and (p["kls"] != "kopya" or p["pn"] in useful),
    }
    for name, kp in keepers.items():
        _, _, ow = scen(kp)
        r[f"{name}_D"] = sum(1 for x in ow if x <= D_S) / n
        r[f"{name}_p50"], r[f"{name}_p95"] = A.pct(ow, .5), A.pct(ow, .95)
        r[f"{name}_n"] = len(ow)
    return r


def main():
    root = pathlib.Path(sys.argv[1])
    geoms = fizik.by_cell()
    rows = []
    for cd in sorted(p for p in root.iterdir() if p.is_dir() and p.name.startswith("b_r25") and not p.name.endswith("_quinn")):
        g = geoms[cd.name]
        for arm in ("taban", "aday"):
            for rep in sorted((cd / arm).glob("rep*"), key=lambda q: int(q.name[3:])):
                x = run(rep, g, arm)
                if x:
                    rows.append(x)
    keys = []
    for r in rows:
        for k in r:
            if k not in keys:
                keys.append(k)
    with open(sys.argv[2], "w", newline="") as f:
        w = csv.DictWriter(f, fieldnames=keys)
        w.writeheader()
        w.writerows(rows)
    print(len(rows), "koşu")


if __name__ == "__main__":
    main()
