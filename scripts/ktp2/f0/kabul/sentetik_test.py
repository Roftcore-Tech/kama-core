#!/usr/bin/env python3
"""[PoC SON KABUL · Bulut-B] degerlendir.py'nin elle kurulmuş sentetik koşuyla denetimi (koşu YOK).

Taban koşu (senaryo v1 çizelgesi; t0 = 1 800 000 000 s):
* Darboğaz (`cap/MID_mb0.jsonl` out) her an hat hızında dolu: 1 228 B datagramlar, aralık
  1 228 / hız(an). 85,0–85,5 s kısa daralma dahil.
* `lg` ACK'leri 4 s'de bir: 20 s'ye dek tavan 12 500; 22 s'de 8 000, 26 s'de 6 250 (2. ACK, bant
  içi), sonra 6 250; 62 s'de 6 250, 66 s'de 12 500 (2. ACK), sonra 12 500.
* `ts` 200 ms'de bir, yeni veri kuyruğu 5 (veri hazır).
* Metin 2 s'de bir, gecikme 400 ms; alındı Bob teslimden 1,5 s sonra.
* Başlangıç kaydı iki uçta cc-port + bitmap; alice `ack_rx` 0x15 = 12; PTT yok.
* `realized_post.json`: mb0'da netem + tbf (drops 0); `qdisc_series.jsonl` backlog 1 228 B.
Beklenen: kip, CC, hız sınırlama (taban kopyası ile), hızlanma, metin/alındı GEÇTİ; PTT SINANMADI
(basış yok). Basışlı kollarda bant yalnız dört kapı (canlı sayaç, PTT datagramı > 0, birleşik
300 ms, 0x17) doğrulanınca GEÇTİ olabilir.
Bozulmuş kollar her biri tek ölçütü değiştirir (aşağıdaki VAKALAR).

Kullanım: python3 -I sentetik_test.py
"""
import importlib.util
import json
import pathlib
import shutil
import sys
import tempfile

HERE = pathlib.Path(__file__).resolve().parent
sys.dont_write_bytecode = True
spec = importlib.util.spec_from_file_location("degerlendir", HERE / "degerlendir.py")
D = importlib.util.module_from_spec(spec)
spec.loader.exec_module(D)

T0 = 1_800_000_000.0


def ms(t):
    return (T0 + t) * 1000.0


def build(d, *, lg=True, cc2=6250, up2=12500, text_lat=400.0, ack_format="bitmap",
          tbf=True, qdisc=True, starve_after=None, ptt_on_time=None, ifb=False, ifb_cap=True, live_diag=True,
          ptt_bytes=4977, ptt_dgrams=34, coalesced=True, x17=4, ack_rx=True,
          wire_null=False, x15=12, no_alice_json=False, bob_moments=False):
    d.mkdir(parents=True)
    (d / "t_start").write_text(f"{T0}\n")
    for who in ("alice", "bob"):
        (d / f"node-{who}").mkdir()
        (d / f"node-{who}" / "transport-start.txt").write_text(
            f"motor=cc-port\nack_biçimi={ack_format}\nack_gönderici_etkin=evet\nack_alıcı_etkin=evet\n"
            "şeritler=açık\n")
    (d / "cap").mkdir()
    # ifb kolu (Yerel'in düzeneği): darboğaz ifb0 (tbf + pfifo), kayıp SONRA (mb0 netem %20):
    # mb0 çıkışı her beşinci datagramı görmez; değerlendirici mb0'dan TÜRETMEMELİ.
    dev = "ifb0" if ifb else "mb0"
    fb = (d / "cap" / f"MID_{dev}.jsonl").open("w") if (not ifb or ifb_cap) else None
    fm = (d / "cap" / "MID_mb0.jsonl").open("w") if ifb else None
    t, n = 0.0, 0
    while t < D.END_V1:
        if starve_after is None or t < starve_after:
            rec = json.dumps({"t_unix_ns": int((T0 + t) * 1e9), "dir": "out", "ip_len": 1228}) + "\n"
            if fb:
                fb.write(rec)
            if fm and n % 5 != 4:
                fm.write(rec)
            n += 1
        t += 1228 / D.rate_at(D.PLAN_V1, t)
    for f in (fb, fm):
        if f:
            f.close()
    alice, bob = [{"k": "hdr", "alice_addr": "10.10.0.1:24645", "bob_addr": "10.10.1.1:24645"}], []
    # Dönüş: her lg ACK'i Bob'un bağlı portundan alice'in geçici portuna; çıkış lg − 1,6 s, varış
    # lg − 0,01 s (kayıpsız FIFO). Bob'un verisi (alındı) ayrı portlardan: süzgeç onu almamalı.
    with (d / "cap" / "B_vb0.jsonl").open("w") as fb, (d / "cap" / "A_va0.jsonl").open("w") as fa:
        for k in range(1, 25):
            t = 4.0 * k - 2.0
            fb.write(json.dumps({"t_unix_ns": int((T0 + t - 1.6) * 1e9), "dir": "out", "ip_len": 90,
                                 "sport": 24645, "dport": 50599}) + "\n")
            fa.write(json.dumps({"t_unix_ns": int((T0 + t - 0.01) * 1e9), "dir": "in", "ip_len": 90,
                                 "sport": 24645, "dport": 50599}) + "\n")
            fb.write(json.dumps({"t_unix_ns": int((T0 + t - 1.0) * 1e9), "dir": "out", "ip_len": 70,
                                 "sport": 35602, "dport": 24645}) + "\n")
    if lg:
        for k in range(1, 25):
            t = 4.0 * k - 2.0
            if t < 20:
                p = 12500
            elif t < 60:
                p = {22.0: 8000, 26.0: cc2}.get(t, 6250)
            else:
                p = {62.0: 6250, 66.0: up2}.get(t, up2 if t > 66 else 6250)
            alice.append({"k": "lg", "t": ms(t), "pace_before": p, "pace_after": p, "rate_after": None,
                          "raised": False, "probe_failed": False})
    t = 0.0
    while t < D.END_V1:
        alice.append({"k": "ts", "t": ms(t), "s": [{"rec": {"txq": 5}, "pr": 12500}]})
        t += 0.2
    for i in range(50):
        t = 2.0 * i
        lat = text_lat if 30 <= t < 40 else 400.0
        alice.append({"k": "send", "c": "T", "i": i, "t": ms(t)})
        bob.append({"k": "rx", "c": "T", "i": i, "t": ms(t) + lat, "first": True})
        alice.append({"k": "rcpt", "c": "T", "i": i, "t": ms(t) + lat + 1500.0, "first": True})
    if bob_moments:
        # Bob'un ACK başına anları (kesme ACK'i 26 s'de işlenir, telde çıkış 24,4 s):
        # kanıt 22,0 s, hazır 23,1 s, çıkış 24,4 s.
        for k in range(1, 25):
            t = 4.0 * k - 2.0
            bob.append({"k": "ack", "t_kanit": ms(t - 4.0), "t_hazir": ms(t - 2.9), "t_cikis": ms(t - 1.6)})
    for name, rows in (("alice.jsonl", alice), ("bob.jsonl", bob)):
        (d / name).write_text("".join(json.dumps(r) + "\n" for r in rows))
    presses = [] if ptt_on_time is None else [{
        "stream": 1, "bundle_ms": 300, "ptt_started": 1, "ptt_ended": 1, "bundles_planned": 28,
        "bundles_delivered": 25, "frames_planned": 420, "frames_on_time": ptt_on_time,
        "first_audio_ms": 640.0, "creation_to_delivery_ms": {"min": 610.0, "median": 650.0, "max": 800.0}}]
    wire = {"live_diag": live_diag, "ptt_wire_bytes": ptt_bytes if ptt_on_time is not None else 0,
            "ptt_wire_datagrams": ptt_dgrams if ptt_on_time is not None else 0}
    if ack_rx:
        wire["ack_rx"] = {"0x14": 0, "0x15": x15, "0x16": 0, "0x17": 0 if ptt_on_time is None else x17}
    if live_diag is None:
        del wire["live_diag"]
    chosen = [] if ptt_on_time is None else [{"sp": 1, "bundle_ms": 300, "coalesced": coalesced,
                                               "caller_ms": 200}]
    if not no_alice_json:
        (d / "alice.json").write_text(json.dumps({"wire": None if wire_null else wire, "ptt_presses": chosen}))
    (d / "bob.json").write_text(json.dumps({"wire": {}, "ptt_presses": presses}))
    q = [{"kind": "netem", "dev": "mb0", "drops": 40}]
    if tbf:
        q.append({"kind": "tbf", "dev": dev, "drops": 0})
    # `>>` ile art arda eklenmiş iki nesne (kapi3_realized deseni): sonuncusu okunur.
    (d / "realized_post.json").write_text(json.dumps({"tag": "eski", "mid_qdisc": []}) + "\n"
                                          + json.dumps({"tag": "post", "mid_qdisc": q}) + "\n")
    if qdisc:
        # Kuyruk: 20–29 s'de 15 000 B (6 250'de 2,4 s), sonra 1 228 B.
        (d / "qdisc_series.jsonl").write_text("".join(
            json.dumps({"t_unix_ns": int((T0 + t) * 1e9), "dev": dev,
                        "backlog_bytes": 15000 if 20 <= t < 29 else 1228}) + "\n"
            for t in range(0, 100)))


def verdicts(d, baseline=True, base_dir=None):
    _, rows = D.evaluate(d, D.PLAN_V1, None, base_dir if baseline else None)
    return {n: v for n, v, _ in rows}, rows


G, K, S = D.GECTI, D.KALDI, D.SINANMADI
VAKALAR = [
    # (ad, build argümanları, taban kopyası verilsin mi, beklenen {ölçüt: hüküm})
    ("taban", {}, True, {"kip (önkoşul)": G, "1 CC": G, "3 hız sınırlama": G, "4 hızlanma": G,
                         "2 PTT": S, "5 metin/alındı": G}),
    ("lg yok", {"lg": False}, True, {"1 CC": S, "4 hızlanma": S, "3 hız sınırlama": G}),
    ("CC 2. ACK bant dışı", {"cc2": 8000}, True, {"1 CC": K}),
    ("hızlanma 2.–3. ACK 9 000", {"up2": 9000}, True, {"4 hızlanma": K}),
    ("daralmada metin 1,5 s", {"text_lat": 1500.0}, True, {"3 hız sınırlama": K, "5 metin/alındı": K}),
    ("ack V2", {"ack_format": "v2"}, True, {"kip (önkoşul)": K, "1 CC": S, "3 hız sınırlama": S}),
    ("ayrı kuyruk yok", {"tbf": False}, True, {"3 hız sınırlama": S}),
    ("kuyruk serisi yok", {"qdisc": False}, True, {"3 hız sınırlama": S}),
    ("taban kopyası yok", {}, False, {"3 hız sınırlama": S}),
    ("40 s'den sonra tel yok", {"starve_after": 40.0}, True, {"3 hız sınırlama": K}),
    # Darboğaz ifb0, kayıp sonra: ifb0 yakalaması varsa taban gibi; yoksa mb0 alt sınırı (%80) → SINANMADI.
    ("ifb0 darboğaz, yakalama var", {"ifb": True}, True, {"1 CC": G, "3 hız sınırlama": G, "4 hızlanma": G}),
    ("ifb0 darboğaz, yakalama yok", {"ifb": True, "ifb_cap": False}, True,
     {"1 CC": S, "3 hız sınırlama": S, "4 hızlanma": S}),
    # PTT (8,4 s basış, 28 × 300 ms; bant sınırı 5 250 B): dört kapı + bayt.
    ("PTT tam, 4 977 B", {"ptt_on_time": 400}, True, {"2 PTT": G}),
    ("PTT zamanında 252/420", {"ptt_on_time": 252}, True, {"2 PTT": K}),
    ("PTT bant 5 400 B", {"ptt_on_time": 400, "ptt_bytes": 5400}, True, {"2 PTT": K}),
    ("PTT canlı sayaç kapalı", {"ptt_on_time": 400, "live_diag": False}, True,
     {"2 PTT": S, "kip (önkoşul)": S, "1 CC": S, "4 hızlanma": S}),
    ("PTT datagram 0", {"ptt_on_time": 400, "ptt_dgrams": 0}, True, {"2 PTT": S}),
    ("PTT birleşik değil", {"ptt_on_time": 400, "coalesced": False}, True, {"2 PTT": S}),
    ("PTT 0x17 yok", {"ptt_on_time": 400, "x17": 0}, True, {"2 PTT": S}),
    # Bulut-D 7097599958: yanlış kip / kayıp sayaç hiçbir ölçütte GEÇTİ vermez; pilot kipi (yalnız
    # 0x17) kip GEÇTİ alır; kapalı/bilinmeyen ayna KALDI değil SINANMADI.
    ("wire null", {"wire_null": True}, True,
     {"kip (önkoşul)": S, "1 CC": S, "3 hız sınırlama": S, "4 hızlanma": S, "2 PTT": S, "5 metin/alındı": S}),
    ("alice.json yok", {"no_alice_json": True}, True, {"kip (önkoşul)": S, "5 metin/alındı": S}),
    ("V2 kipi, metin", {"ack_format": "v2"}, True, {"kip (önkoşul)": K, "5 metin/alındı": S}),
    ("pilot: yalnız 0x17", {"ptt_on_time": 400, "x15": 0, "x17": 12}, True,
     {"kip (önkoşul)": G, "1 CC": G, "2 PTT": G, "5 metin/alındı": G}),
    ("ayna bilinmiyor (live_diag yok)", {"live_diag": None}, True,
     {"kip (önkoşul)": S, "1 CC": S, "5 metin/alındı": S}),
    ("canlı ayna kapalı, sayaç 0", {"live_diag": False, "x15": 0}, True,
     {"kip (önkoşul)": S, "5 metin/alındı": S}),
    # ACK tür sayacı yoksa CC/hızlanma SINANMADI (lg olsa da).
    ("ACK tür sayacı yok", {"ack_rx": False}, True, {"1 CC": S, "4 hızlanma": S, "kip (önkoşul)": S}),
]


def main():
    fails = 0
    with tempfile.TemporaryDirectory() as tmp:
        tmp = pathlib.Path(tmp)
        build(tmp / "kesmesiz")
        for i, (name, kw, base, want) in enumerate(VAKALAR):
            d = tmp / f"v{i}"
            build(d, **kw)
            got, rows = verdicts(d, base, tmp / "kesmesiz")
            bad = {k: (got.get(k), v) for k, v in want.items() if got.get(k) != v}
            print(f"{'OK  ' if not bad else 'HATA'} {name}: {got}")
            if bad:
                fails += 1
                print(f"     beklenen≠bulunan: {bad}")
                for n, v, notes in rows:
                    if n in bad:
                        for x in notes:
                            print(f"       {n}: {x}")
        # Gecikme ayrıştırması (taban): kesme ACK'i 26 s; çıkış 24,4 → parça 1 4,40 s; parça 2 1,59 s;
        # kuyruk 29 s'de ≤ 0,7 s → parça 3 3,00 s.
        _, rows = verdicts(tmp / "v0", True, tmp / "kesmesiz")
        notes = " | ".join(next(n for name, _, n in rows if name == "gecikme ayrıştırması"))
        want = ["parça 1 düşüş → Bob'dan çıkış: 4.40 s (tek parça", "parça 2 Bob'dan çıkış → alice varışı: 1.59 s",
                "parça 3 kesme → kuyruk ≤ 0.7 s (60 s'ye dek): 3.00 s"]
        miss = [w for w in want if w not in notes]
        # Kanıt anı yokken parça 1 bütçe/yol verme diye etiketlenmez.
        if "bütçe" in notes or "yol ver" in notes:
            miss.append("parça 1 etiketi")
        print(f"{'OK  ' if not miss else 'HATA'} gecikme ayrıştırması: {notes}")
        fails += bool(miss)
        # Beş an (Bob kaydı var): kanıt 22,0 → hazır 23,1 → çıkış 24,4 → varış 25,99 → kesme 26,0.
        d = tmp / "bes_an"
        build(d, bob_moments=True)
        _, rows = verdicts(d, True, tmp / "kesmesiz")
        notes = " | ".join(next(n for name, _, n in rows if name == "gecikme ayrıştırması"))
        want = ["aralık düşüş → Bob ilk geçerli kanıt: 2.000 s", "aralık Bob ilk geçerli kanıt → Bob ACK hazır: 1.100 s",
                "aralık Bob ACK hazır → telde çıkış: 1.300 s", "aralık telde çıkış → Alice varışı: 1.590 s",
                "aralık Alice varışı → LineGuard kesmesi: 0.010 s"]
        miss = [w for w in want if w not in notes]
        print(f"{'OK  ' if not miss else 'HATA'} beş an: {notes}")
        fails += bool(miss)
        # Bob kaydı yokken iki an EKSİK, aralıklar "ayrıştırılamadı".
        _, rows = verdicts(tmp / "v0", True, tmp / "kesmesiz")
        notes = " | ".join(next(n for name, _, n in rows if name == "gecikme ayrıştırması"))
        ok = "Bob ilk geçerli kanıt EKSİK" in notes and "ayrıştırılamadı" in notes
        print(f"{'OK  ' if ok else 'HATA'} beş an, Bob kaydı yok")
        fails += not ok
        # Alan denetimi: tam kayıtta transport + PTT HAZIR; Bob kaydı ve lg yokken HAZIR DEĞİL.
        import contextlib
        import io
        d = tmp / "alan_tam"
        build(d, bob_moments=True, ptt_on_time=400)
        buf = io.StringIO()
        with contextlib.redirect_stdout(buf):
            D.field_check(d)
            D.field_check(tmp / "v1")  # lg yok, Bob kaydı yok
        txt = buf.getvalue().split("ALAN DENETİMİ")
        ok = "transport: HAZIR\n" in txt[1] and "ptt: HAZIR\n" in txt[1] and "transport: HAZIR DEĞİL" in txt[2]
        print(f"{'OK  ' if ok else 'HATA'} alan denetimi")
        fails += not ok
    print("SONUÇ:", "GEÇTİ" if fails == 0 else f"{fails} vaka KALDI")
    return 1 if fails else 0


if __name__ == "__main__":
    sys.exit(main())
