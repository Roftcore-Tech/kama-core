#!/usr/bin/env python3
"""[KTP2-F0 / Ajan B] TAM SONUÇ tabloları (TAM-ANALIZ-BRIEF §2). Salt okur; koşu başlatmaz.

Girdi (`--tam` dizini = reports/ktp2/f0/b/tam):
  kosular_b.csv.gz   analiz_b.py koşu satırları (4920)
  r25_kosular.csv    r25_kok.py
  a10_kosular.csv    a10_kok.py (Ajan A'nın ürün yolu zaman serisi)
  hiz_seri.csv       hiz_seri.py
Ham kök (`--kok`): eksik dosya sayımı için. Yoksa o sütun "–".

Çıktı: TAM-OTOMATIK.md, hucre_ozet.csv, farklar.csv, tahmin.csv.

Yöntem:
* Birim = koşu. Hücre içi: medyan [Q1–Q3] (min–maks).
* Taban–aday farkı: medyan(aday) − medyan(taban); %95 yüzdelik bootstrap, koşu birimli, B = 2000,
  tohum sha256(hücre|ölçü) (Ajan D'nin `tam_analiz.py` yöntemi). Çoklu karşılaştırma düzeltmesi YOK;
  sayı ve beklenen yanlış pozitif tabloda yazılır.
* Kilit oranları: payda koşu. Fisher kesin (iki yanlı). Hücre adı ile havuz ayrı satırdadır.
* N < 30, stres (l50/l80) → betimsel; hüküm satırına girmez.

Kullanım: python3 -I tam_b.py --tam <dizin> [--kok <runs/tam/b/out>]
"""
import argparse
import collections
import csv
import gzip
import hashlib
import io
import math
import pathlib
import random
import statistics
import sys

HERE = pathlib.Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.dont_write_bytecode = True
import fizik  # noqa: E402

ARMS = ("taban", "aday", "quinn", "tcp")
KAMA = ("taban", "aday")


def pct(v, q):
    v = sorted(v)
    if not v:
        return None
    if len(v) == 1:
        return v[0]
    k = q * (len(v) - 1)
    lo = math.floor(k)
    hi = min(lo + 1, len(v) - 1)
    return v[lo] + (v[hi] - v[lo]) * (k - lo)


def tr(x, nd=3):
    """Ondalık virgüllü kısa sayı."""
    if x is None:
        return "–"
    a = abs(x)
    if a >= 1000:
        s = f"{x:,.0f}".replace(",", " ")
    elif a >= 100:
        s = f"{x:.0f}"
    elif a >= 10:
        s = f"{x:.1f}"
    else:
        s = f"{x:.{nd}f}" if nd else f"{x:.0f}"
    return s.replace(".", ",")


class Veri:
    def __init__(self, tam):
        with gzip.open(tam / "kosular_b.csv.gz", "rt") as f:
            self.rows = list(csv.DictReader(f))
        self.geo = fizik.by_cell()
        self.by = collections.defaultdict(list)
        for r in self.rows:
            self.by[(r["cell"], r["arm"])].append(r)

    def vals(self, c, a, k):
        out = []
        for r in self.by.get((c, a), []):
            v = r.get(k, "")
            if v in ("", None):
                continue
            try:
                out.append(float(v))
            except ValueError:
                pass
        return out

    def med(self, c, a, k):
        v = self.vals(c, a, k)
        return statistics.median(v) if v else None

    def dist(self, c, a, k, nd=3):
        v = self.vals(c, a, k)
        if not v:
            return "–"
        return f"{tr(pct(v, .5), nd)} [{tr(pct(v, .25), nd)}–{tr(pct(v, .75), nd)}] ({tr(min(v), nd)}–{tr(max(v), nd)})"

    def med_all(self, c, k):
        return {a: self.med(c, a, k) for a in ARMS}


def boot_diff(a, b, key, B=2000):
    """medyan(a) − medyan(b), %95 yüzdelik bootstrap."""
    if not a or not b:
        return None
    rng = random.Random(int(hashlib.sha256(key.encode()).hexdigest()[:16], 16))
    d0 = statistics.median(a) - statistics.median(b)
    na, nb = len(a), len(b)
    ds = []
    for _ in range(B):
        ds.append(statistics.median(rng.choices(a, k=na)) - statistics.median(rng.choices(b, k=nb)))
    ds.sort()
    return d0, ds[int(0.025 * B)], ds[int(0.975 * B) - 1]


def fisher(a, b, c, d):
    """2×2 Fisher kesin, iki yanlı."""
    n = a + b + c + d
    r1, c1 = a + b, a + c
    lc = math.lgamma

    def p(x):
        return math.exp(lc(r1 + 1) + lc(n - r1 + 1) + lc(c1 + 1) + lc(n - c1 + 1) - lc(n + 1) - lc(x + 1)
                        - lc(r1 - x + 1) - lc(c1 - x + 1) - lc(n - r1 - c1 + x + 1))
    p0 = p(a)
    lo, hi = max(0, r1 + c1 - n), min(r1, c1)
    return min(1.0, sum(p(x) for x in range(lo, hi + 1) if p(x) <= p0 * (1 + 1e-9)))


def order(cells):
    """Birincil önce: hız, gecikme, kayıp, yük."""
    rid = {"r25": 0, "r1k": 1, "r12k": 2}
    def key(c):
        g = c if isinstance(c, dict) else None
        return (rid[g["rate_id"]], g["delay_ms"], g["loss"], g["payload"], g["cell"])
    return sorted(cells, key=key)


def etiket(c):
    return c["cell"][2:] + ("" if c["kind"] == "hukum" else " †")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--tam", required=True)
    ap.add_argument("--kok")
    a = ap.parse_args()
    tam = pathlib.Path(a.tam)
    kok = pathlib.Path(a.kok) if a.kok else None
    V = Veri(tam)
    cells = order([c for c in fizik.grid() if (c["cell"], c["arms"][0]) in V.by])
    hukum = [c for c in cells if c["kind"] == "hukum"]
    L = ["# KTP2-F0 / B — TAM-OTOMATIK (tam_b.py çıktısı; elle düzenlenmez)", "",
         f"Koşu satırı: {len(V.rows)} · hücre: {len(cells)} (hüküm {len(hukum)}, betimsel {len(cells) - len(hukum)}). "
         "† = betimsel (N < 30 ya da stres l50/l80); hüküm cümlesinde kullanılmaz.",
         "Medyan [Q1–Q3] (min–maks); birim = koşu. Bench Kama kolu üst sınırdır (Ajan D, D-9). tcp kolları D-13 "
         "sınırlamasını taşır (veth TSO açık, kayıplar kısmen ikişerli; `kayıp` sütunu tcp'de skb/segment karışık).", ""]

    # ── §1 Geçerlilik ──────────────────────────────────────────────────────────────────────────
    L += ["## §1 Geçerlilik", "",
          "| hücre | kol | N | rc=0 | el sıkışma yok | kirli başlangıç | rx düşen | eksik dosya | kayıp ileri | kayıp dönüş | yakalama/qdisc ileri |",
          "|---|---|---|---|---|---|---|---|---|---|---|"]
    need = ("rc", "c.err", "s.err", "out.jsonl", "manifest.json", "realized_pre.json", "realized_post.json",
            "capb_cli.csv.gz", "capb_srv.csv.gz", "rx_log.jsonl", "cmd.txt")
    top = collections.Counter()
    for c in cells:
        for arm in c["arms"]:
            rows = V.by[(c["cell"], arm)]
            rc0 = sum(1 for r in rows if r["rc"] == "0")
            hs = sum(1 for r in rows if arm in ("quinn", "tcp") and r.get("deliv_any") in ("0", "0.0")
                     and float(r.get("setup_s") or 0) >= 0.95 * c["deadline_s"])
            kirli = sum(1 for r in rows if r.get("kirli_baslangic") == "1")
            rxd = sum(1 for r in rows if r.get("rx_dropped") not in ("", None, "0"))
            eks = "–"
            if kok:
                n = 0
                for rep in (kok / c["cell"] / arm).glob("rep*"):
                    n += any(not (rep / f).exists() for f in need)
                eks = str(n)
            top.update({"N": len(rows), "rc0": rc0, "hs": hs, "kirli": kirli, "rxd": rxd,
                        "eks": 0 if eks == "–" else int(eks)})
            L.append(f"| {etiket(c)} | {arm} | {len(rows)} | {rc0} | {hs} | {kirli} | {rxd} | {eks} | "
                     f"{V.dist(c['cell'], arm, 'kayip_ileri')} | {V.dist(c['cell'], arm, 'kayip_donus')} | "
                     f"{V.dist(c['cell'], arm, 'yakalama_qdisc_ileri')} |")
    ym, ydus, yn = [], 0, 0
    for c in cells:
        for arm in KAMA:
            v = V.vals(c["cell"], arm, "yakalama_qdisc_ileri")
            if v:
                ym.append(statistics.median(v))
                yn += len(v)
                ydus += sum(1 for x in v if x < 0.95)
    L += ["", f"Toplam: N = {top['N']}, rc = 0 {top['rc0']}, el sıkışma yok {top['hs']}, kirli başlangıç {top['kirli']}, "
          f"rx düşen {top['rxd']}, eksik dosya {top['eks']}.",
          "Yakalama / qdisc oranı (ileri, Kama kolları): hücre medyanı "
          f"{tr(min(ym))}–{tr(max(ym))}; koşu < 0,95: {ydus}/{yn} (kısa koşularda birkaç paketlik fark: yakalama, qdisc'ten önce bitiyor).", ""]

    # ── §2 Sonuç ───────────────────────────────────────────────────────────────────────────────
    L += ["## §2 Sonuç tabloları", "", "### §2.1 Teslim ve gecikme", "",
          "`rx_*` = alıcı teslim damgası (rx_log; dört kolda aynı şema). teyit = gönderen teyidi (out.jsonl).", "",
          "| hücre | kol | N | teslim D (teyit) | teslim D (alıcı) | alıcı p50 s | alıcı p95 s | alıcı p99 s | teyit p95 s | kurulum s |",
          "|---|---|---|---|---|---|---|---|---|---|"]
    for c in cells:
        for arm in c["arms"]:
            n = len(V.by[(c["cell"], arm)])
            L.append(f"| {etiket(c)} | {arm} | {n} | " + " | ".join(
                V.dist(c["cell"], arm, k) for k in ("deliv_D", "rx_deliv_D", "rx_ow_p50_s", "rx_ow_p95_s", "rx_ow_p99_s", "conf_p95_s", "setup_s")) + " |")
    L.append("")

    # taban–aday farkları
    fark_keys = [("rx_deliv_D", "alıcı teslim D"), ("rx_ow_p50_s", "alıcı p50 s"), ("rx_ow_p95_s", "alıcı p95 s"),
                 ("wf_useful_share", "ileri faydalı pay"), ("srtt_over_rttfifo", "srtt/RTT_fifo"),
                 ("cp_redund_l2B", "gereksiz kopya baytı")]
    frows = []
    L += ["### §2.2 Taban–aday farkı (aday − taban, %95 bootstrap)", "",
          "| hücre | " + " | ".join(k[1] for k in fark_keys) + " |", "|---|" + "---|" * len(fark_keys)]
    n_cmp = n_sig = 0
    for c in hukum:
        cells_txt = []
        for k, _ in fark_keys:
            x, y = V.vals(c["cell"], "aday", k), V.vals(c["cell"], "taban", k)
            r = boot_diff(x, y, f"{c['cell']}|{k}")
            if r is None:
                cells_txt.append("–")
                continue
            sig = r[1] > 0 or r[2] < 0
            n_cmp += 1
            n_sig += sig
            frows.append({"hucre": c["cell"], "olcu": k, "fark": r[0], "alt": r[1], "ust": r[2], "aralik_0_disinda": int(sig)})
            cells_txt.append(f"{'**' if sig else ''}{tr(r[0])} [{tr(r[1])}; {tr(r[2])}]{'**' if sig else ''}")
        L.append(f"| {etiket(c)} | " + " | ".join(cells_txt) + " |")
    L += ["", f"Kalın = aralık 0'ı dışlıyor. {n_cmp} karşılaştırma, düzeltme yok; %5'te beklenen yanlış pozitif ≈ {n_cmp * 0.05:.0f}, "
          f"gözlenen aralık dışı {n_sig}.", ""]

    L += ["### §2.3 Taban–aday özeti", "", "Aralığı 0'ı dışlayan hücre sayısı (aday − taban); n = karşılaştırılan hüküm hücresi. Düzeltmesiz.", "",
          "| ölçü | n | aday büyük | aday küçük |", "|---|---|---|---|"]
    for k, ad in fark_keys:
        rr = [x for x in frows if x["olcu"] == k]
        L.append(f"| {ad} | {len(rr)} | {sum(1 for x in rr if x['aralik_0_disinda'] and x['fark'] > 0)} | {sum(1 for x in rr if x['aralik_0_disinda'] and x['fark'] < 0)} |")
    L += ["", "Alıcı p95 ve alıcı teslim D'de aralığı 0'ı dışlayan hücreler:", "", "| hücre | ölçü | aday − taban | %95 aralık |", "|---|---|---|---|"]
    for x in frows:
        if x["aralik_0_disinda"] and x["olcu"] in ("rx_ow_p95_s", "rx_deliv_D"):
            L.append(f"| {x['hucre'][2:]} | {'alıcı p95 s' if x['olcu'] == 'rx_ow_p95_s' else 'alıcı teslim D'} | {tr(x['fark'])} | [{tr(x['alt'])}; {tr(x['ust'])}] |")
    L.append("")

    # ── §3 S2: hat payları ─────────────────────────────────────────────────────────────────────
    L += ["## §3 S2: hat kapasitesinin payları", "",
          "İleri yön, hattan geçen bayt (L2 dahil) üzerinden. `faydalı` = alıcıya ilk varan DATA yükü (orijinal ya da kopya). "
          "`kapasite` = hız × pencere (ilk sunum → son paket). Sütun toplamı 100 %; `ACK` ve `dönüş` ayrı yön. "
          "Her değer koşular arası medyandır; ayrı hesaplandığı için toplam 100'den birkaç puan sapabilir. "
          "tcp'de `başlık` = uygulama çerçevesi, L2/L3/L4 = Eth+IP+TCP+seçenek; quinn'de yük dışı bayt = QUIC başlık/çerçeve (şifreli).", "",
          "| hücre | kol | ileri doluluk | deneme yükü/hat | faydalı / geçen % | faydalı / kapasite % | L2/L3/L4 % | başlık % | gereksiz yük % | onarım % | kontrol % | yalın ACK ileri % | dönüş doluluk | dönüşte ACK % | dönüş kontrol % |",
          "|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|"]
    srows = []
    for c in cells:
        cid = c["cell"]
        for arm in c["arms"]:
            rows = V.by[(cid, arm)]

            def share(keys, pre="wf_"):
                v = []
                for r in rows:
                    try:
                        t = float(r.get(pre + "total") or 0)
                    except ValueError:
                        continue
                    if t <= 0:
                        continue
                    s = 0.0
                    for k in keys:
                        try:
                            s += float(r.get(pre + k) or 0)
                        except ValueError:
                            pass
                    v.append(100 * s / t)
                return statistics.median(v) if v else None
            use = share(("pay_useful_orig", "pay_useful_copy", "pay_useful"))
            l234 = share(("l234",))
            hdr = share(("env", "hdr", "frame_hdr"))
            red = share(("pay_redund_orig", "pay_redund_copy", "pay_redund"))
            fec = share(("fec_repair", "fec_source"))
            ctl = share(("ctrl", "probe", "other", "syn_fin"))
            quic = share(("quic_long", "quic_short"))
            ackf = share(("ack",))
            r_ack = share(("ackpkt_l2", "app_ack"), "wr_") if arm != "quinn" else None
            kop_f = share(("pay_useful_copy",))
            tel = []
            for r in rows:
                try:
                    tt = float(r.get("wf_total") or 0)
                    tel.append((100 * float(r.get("cp_useful_l2B") or 0) / tt, 100 * float(r.get("cp_redund_l2B") or 0) / tt))
                except (ValueError, ZeroDivisionError):
                    continue
            kop_tel_f = statistics.median(x[0] for x in tel) if tel else None
            kop_tel_g = statistics.median(x[1] for x in tel) if tel else None
            r_ctl = share(("ctrl", "recovered", "probe", "other", "syn_fin"), "wr_")
            row = {"hucre": cid, "kol": arm, "wf_util": V.med(cid, arm, "wf_util"), "wfa_yuk": V.med(cid, arm, "wfa_offered_load"),
                   "faydali_gecen": use, "faydali_kap": (100 * V.med(cid, arm, "wf_useful_of_cap")) if V.med(cid, arm, "wf_useful_of_cap") is not None else None,
                   "l234": l234, "baslik": hdr if arm != "quinn" else quic, "gereksiz": red, "onarim": fec, "kontrol": ctl, "ack_ileri": ackf,
                   "wr_util": V.med(cid, arm, "wr_util"), "ack_donus": r_ack, "kontrol_donus": r_ctl,
                   "kopya_faydali_yuk": kop_f, "kopya_faydali_tel": kop_tel_f, "kopya_gereksiz_tel": kop_tel_g}
            srows.append(row)
            L.append(f"| {etiket(c)} | {arm} | {tr(row['wf_util'])} | {tr(row['wfa_yuk'])} | {tr(use, 1)} | {tr(row['faydali_kap'], 1)} | "
                     f"{tr(l234, 1)} | {tr(row['baslik'], 1)} | {tr(red, 1)} | {tr(fec, 1)} | {tr(ctl, 1)} | {tr(ackf, 1)} | {tr(row['wr_util'])} | {tr(r_ack, 1)} | {tr(r_ctl, 1)} |")
    L.append("")

    # ── §4 srtt ────────────────────────────────────────────────────────────────────────────────
    L += ["## §4 Gerçek RTT ↔ motor srtt", "",
          "RTT_fiz = 2·gecikme + ser(veri) + ser(ACK 0 aralık). RTT_fifo = sıra koruyan jitter modeli (fizik.fifo_owd). "
          "srtt koşu sonu anlık değeri (KP-ACTIVATION). ağ RTT = son 20 orijinal, alıcı tutması hariç (yakalama). srtt = 0 (örnek yok) dışlanır; "
          "`srtt>0` sütunu kaç koşuda örnek olduğunu verir.", "",
          "| hücre | RTT_fiz | RTT_fifo | kol | srtt>0 | srtt s | srtt/RTT_fiz | srtt/RTT_fifo | ağ RTT s | srtt/ağ RTT | RTT örneği |",
          "|---|---|---|---|---|---|---|---|---|---|---|"]
    for c in cells:
        for arm in KAMA:
            cid = c["cell"]
            if (cid, arm) not in V.by:
                continue
            nz = len(V.vals(cid, arm, "srtt_s"))
            L.append(f"| {etiket(c)} | {tr(c['rtt_phys_s'])} | {tr(c['rtt_fifo_s'])} | {arm} | {nz}/{len(V.by[(cid, arm)])} | "
                     f"{V.dist(cid, arm, 'srtt_s')} | {tr(V.med(cid, arm, 'srtt_over_rttphys'))} | {V.dist(cid, arm, 'srtt_over_rttfifo')} | "
                     f"{tr(V.med(cid, arm, 'rtt_net_last20_med_s'))} | {tr(V.med(cid, arm, 'srtt_over_rttnet'))} | {tr(V.med(cid, arm, 'rtt_samples'))} |")
    L.append("")

    # ── §5 yeniden gönderim ────────────────────────────────────────────────────────────────────
    L += ["## §5 Yeniden gönderim: teslime katkı veren ↔ gereksiz", "",
          "Kopya = aynı birimin ilk iletim dışı her DATA iletimi (ARQ, kuyruk probu, §11, kuruluş probu, geometrik); tel bunları ayırmaz. "
          "Faydalı = B'ye ilk varan kopya. Gereksiz = birim B'de varken gelen. Kayıp = netem düşürdü (hat harcamaz). "
          "`gereksiz pay` = gereksiz / (faydalı + gereksiz), koşu başına, hattan geçen bayt.", "",
          "| hücre | kol | kopya n | faydalı n | gereksiz n | kayıp n | faydalı B | gereksiz B | kayıp B | gereksiz pay | alıcıda varken gönderilen n | teyitten sonra n | retx B (sayaç) | dup B (sayaç) | kuyruk probu | kuruluş probu |",
          "|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|"]
    for c in cells:
        cid = c["cell"]
        for arm in KAMA:
            rows = V.by.get((cid, arm))
            if not rows:
                continue
            gp = []
            for r in rows:
                try:
                    u, x = float(r.get("cp_useful_l2B") or 0), float(r.get("cp_redund_l2B") or 0)
                except ValueError:
                    continue
                if u + x > 0:
                    gp.append(x / (u + x))
            L.append(f"| {etiket(c)} | {arm} | " + " | ".join(tr(V.med(cid, arm, k), 0 if k.endswith('n') else 1) for k in
                     ("cp_tx", "cp_useful", "cp_redund_arr", "cp_lost", "cp_useful_l2B", "cp_redund_l2B", "cp_lost_l2B")) +
                     f" | {tr(statistics.median(gp) if gp else None)} | " + " | ".join(tr(V.med(cid, arm, k), 1) for k in
                     ("cp_sent_after_arrival", "cp_sent_after_conf", "tx_retx_B", "tx_dup_B", "tail_probes", "establish_tx")) + " |")
    L.append("")

    # ── §6 fiziksel ↔ zamanlayıcı ──────────────────────────────────────────────────────────────
    L += ["## §6 Fiziksel kapasite ↔ gönderen/zamanlayıcı", "",
          "İdeal sınır: tek kopyalı iş koruyan FIFO + RTT_fifo tek yön + kayıp deneme başına bir RTT. Fazla (mesaj·s, koşu toplamı) üç bileşene ayrılır "
          "(toplamları = fazla): gönderen (sunum → ilk iletim), kurtarma (ilk iletim → teslim eden kopya, ideal RTO üstü), kuyruk (darboğaz yolculuğu, ideal üstü). "
          "`geç` = teslim, ideal + jitter p95'ten geç. Geç kalma süresi içinde hattın durumu: boş / faydalı / gereksiz / kontrol / onarım. "
          "`kaynak`: fazlanın en büyük bileşeni; hat doluluğu ≥ 0,9 ise ayrıca geç kalmada hattı neyin doldurduğu.", "",
          "| hücre | kol | ρ_ARQ | ileri doluluk | fazla mesaj·s | gönderen % | kurtarma % | kuyruk % | geç mesaj | hat boş | faydalı | gereksiz | kontrol | onarım | kaynak |",
          "|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|"]
    for c in cells:
        cid = c["cell"]
        for arm in KAMA:
            if (cid, arm) not in V.by:
                continue
            m = lambda k: V.med(cid, arm, k)  # noqa: E731
            top_e = m("exc_top_s")
            bil = {}
            for k, n in (("exc_gonderen_s", "gönderen"), ("exc_kurtarma_s", "kurtarma"), ("exc_kuyruk_s", "kuyruk")):
                bil[n] = (100 * m(k) / top_e) if (top_e and m(k) is not None) else None
            kay = max((n for n in bil if bil[n] is not None), key=lambda n: bil[n], default="–")
            if (m("wf_util") or 0) >= 0.9:
                kay = "hat dolu: " + max((("kontrol", m("late_busy_ctrl_frac") or 0), ("gereksiz", m("late_busy_redund_frac") or 0),
                                          ("faydalı", m("late_busy_useful_frac") or 0)), key=lambda x: x[1])[0]
            L.append(f"| {etiket(c)} | {arm} | {tr(c['rho_fwd_arq'])} | {tr(m('wf_util'))} | {tr(top_e, 0)} | {tr(bil['gönderen'], 0)} | {tr(bil['kurtarma'], 0)} | {tr(bil['kuyruk'], 0)} | "
                     f"{tr(m('late_msgs'), 0)}/{c['msgs']} | {tr(m('late_idle_frac'))} | {tr(m('late_busy_useful_frac'))} | {tr(m('late_busy_redund_frac'))} | {tr(m('late_busy_ctrl_frac'))} | {tr(m('late_busy_fec_frac'))} | {kay} |")
    L.append("")

    # ── §7 kilit ───────────────────────────────────────────────────────────────────────────────
    L += ["## §7 Gönderen kilidi (B-C9 / B-K14)", "",
          "Payda = o hücredeki Kama koşusu. `kilitli` = C tanımı (birikim ≥ 1 s ya da ilk iletim payı < 0,95; birikim kaynağı rx_log). "
          "`kilit_gönderen` = sunum → ilk iletim beklemesinin 12 pencereli birikimi ≥ 1 s (B ayrımı). Fisher: `kilit_gönderen` taban ↔ aday.", "",
          "| hücre | kilitli taban | kilitli aday | kilit_gönderen taban | kilit_gönderen aday | Fisher p | btlbw/sunulan taban | aday | jeton aç taban | aday | gönderen birikim ms taban | aday | queue_extra p95 s taban |",
          "|---|---|---|---|---|---|---|---|---|---|---|---|---|"]
    pool = collections.defaultdict(lambda: [0, 0, 0, 0])
    p2 = collections.Counter()

    def cnt(cid, arm, k):
        v = V.vals(cid, arm, k)
        return int(sum(v)), len(v)

    for c in cells:
        cid = c["cell"]
        if (cid, "taban") not in V.by:
            continue
        kt, kta = cnt(cid, "taban", "kilit_gonderen")
        ka, kaa = cnt(cid, "aday", "kilit_gonderen")
        ct, cta = cnt(cid, "taban", "kilitli")
        ca, caa = cnt(cid, "aday", "kilitli")
        pf = fisher(kt, kta - kt, ka, kaa - ka) if kta and kaa else None
        L.append(f"| {etiket(c)} | {ct}/{cta} | {ca}/{caa} | {kt}/{kta} | {ka}/{kaa} | {(f'{pf:.1e}'.replace('.', ',') if pf is not None and pf < 0.01 else tr(pf))} | {tr(V.med(cid, 'taban', 'btlbw_sunulan'))} | {tr(V.med(cid, 'aday', 'btlbw_sunulan'))} | "
                 f"{tr(V.med(cid, 'taban', 'jeton_ac_payi'))} | {tr(V.med(cid, 'aday', 'jeton_ac_payi'))} | {tr(V.med(cid, 'taban', 'gonderen_birikim_ms'), 0)} | {tr(V.med(cid, 'aday', 'gonderen_birikim_ms'), 0)} | {tr(V.med(cid, 'taban', 'queue_extra_p95_s'))} |")
        if c["kind"] == "hukum":
            key = f"{c['rate_id']} l{c['loss']}"
            pool[key][0] += kt
            pool[key][1] += kta
            pool[key][2] += ka
            pool[key][3] += kaa
            for arm in KAMA:
                for r in V.by[(cid, arm)]:
                    try:
                        b = float(r.get("btlbw_sunulan"))
                        kg = r.get("kilit_gonderen")
                    except (TypeError, ValueError):
                        continue
                    if kg in ("0", "1") and c["rate_id"] != "r25" and c["loss"] > 0:
                        p2[(arm, b * (1 - c["loss"] / 100.0) < 1.0, kg == "1")] += 1
    L += ["", "**Havuzlar (hüküm hücreleri; hücre adı değildir, satır başına birden çok hücre):**", "",
          "| havuz | taban kilit_gönderen | aday | Fisher p |", "|---|---|---|---|"]
    for k in sorted(pool):
        x = pool[k]
        L.append(f"| {k} | {x[0]}/{x[1]} | {x[2]}/{x[3]} | {tr(fisher(x[0], x[1] - x[0], x[2], x[3] - x[2]) if x[1] and x[3] else None)} |")
    L += ["", "**Kilit koşulu (r1k/r12k, l10–l30, payda koşu):** btlbw/sunulan × (1 − p) < 1 ise pacer, kayıp sonrası teslim edilebilen hızda sunulan yükün altında. "
          "btlbw koşu sonu değeridir; ters nedensellik olabilir (kilitli koşuda btlbw düşer).", "",
          "| kol | koşul | kilit_gönderen = 1 | = 0 | oran |", "|---|---|---|---|---|"]
    for arm in KAMA:
        for cond in (True, False):
            y, n = p2[(arm, cond, True)], p2[(arm, cond, False)]
            L.append(f"| {arm} | {'< 1' if cond else '≥ 1'} | {y} | {n} | {tr(y / (y + n) if y + n else None)} |")
    L.append("")

    # ── §8 r12k başlangıç ──────────────────────────────────────────────────────────────────────
    hiz = list(csv.DictReader(open(tam / "hiz_seri.csv")))
    L += ["## §8 r12k başlangıç birikimi ve hız", "",
          "`sw200 n` = ilk iletimi sunumdan > 0,2 s sonra çıkan mesaj; `son` = sonuncusunun sunum anı (s). `4800/srtt` = MIN_CWND tavanı (srtt = koşu sonu medyanı). "
          "Sunulan = (16 + 11 + yük) / I, UDP yükü B/s. İlk üç 5 s'lik pencere: A çıkışı toplam UDP B/s (koşular arası medyan).", "",
          "| hücre | kol | PreConverge çıkışı ms | PreConverge'de ilk iletilen msj | sw200 n | son s | gönderen bekleme p95 s | alıcı p95 s | sunulan B/s | 4800/srtt B/s | 0–5 s | 5–10 s | 10–15 s | 15–20 s | koşu sonu btlbw B/s |",
          "|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|"]
    for c in cells:
        cid = c["cell"]
        if c["rate_id"] != "r12k":
            continue
        for arm in ("taban", "aday", "quinn"):
            if (cid, arm) not in V.by:
                continue
            ser = {float(r["t_s"]): float(r["top_Bps"]) for r in hiz if r["hucre"] == cid and r["kol"] == arm}
            sr = V.med(cid, arm, "srtt_s")
            sun = (16 + 11 + c["payload"]) / (c["ival_ms"] / 1000.0)
            L.append(f"| {etiket(c)} | {arm} | {tr(V.med(cid, arm, 'left_preconv'), 0)} | {tr(V.med(cid, arm, 'preconv_msj'), 0)} | {tr(V.med(cid, arm, 'sw200_n'), 0)} | {tr(V.med(cid, arm, 'sw200_son_s'), 1)} | "
                     f"{tr(V.med(cid, arm, 'sender_wait_p95_s'))} | {tr(V.med(cid, arm, 'rx_ow_p95_s'))} | {tr(sun, 0)} | {tr(4800 / sr if sr else None, 0)} | "
                     + " | ".join(tr(ser.get(t), 0) for t in (0.0, 5.0, 10.0, 15.0)) + f" | {tr(V.med(cid, arm, 'est_btlbw'), 0)} |")
    L.append("")

    # ── §9 r25 ─────────────────────────────────────────────────────────────────────────────────
    r25 = list(csv.DictReader(open(tam / "r25_kosular.csv")))

    def rm(c, arm, k):
        v = []
        for r in r25:
            if r["hucre"] == c and r["kol"] == arm and r.get(k) not in ("", None):
                try:
                    v.append(float(r[k]))
                except ValueError:
                    pass
        return statistics.median(v) if v else None

    L += ["## §9 r25 kök ayrıştırması", "",
          "### §9.1 Bayt payları (medyan, koşu başına). Düzlemler karıştırılmaz", "",
          "**Sayaç düzlemi** (UDP yükü, deneme; c.err). **Tel düzlemi** (L2 dahil; yakalama): deneme = va0 çıkışı, hat = vb0 girişi.", "",
          "| hücre | kol | UDP deneme toplam | data | retx | dup | ctrl | tel deneme toplam | orijinal | kopya | PARAM | hat toplam | orijinal | kopya | PARAM | PARAM % (hat) | kopya % (hat) | orijinal % (hat) |",
          "|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|"]
    r25c = sorted({r["hucre"] for r in r25}, key=lambda x: (int(x.split("_")[2][1:]), int(x.split("_")[3][1:])))
    for c in r25c:
        for arm in KAMA:
            d = [rm(c, arm, f"den_{k}_B") for k in ("orijinal", "kopya", "param")]
            h = [rm(c, arm, f"hat_{k}_B") for k in ("orijinal", "kopya", "param")]
            ht = sum(x or 0 for x in h)
            L.append(f"| {c[2:]} | {arm} | " + " | ".join(tr(rm(c, arm, k), 0) for k in ("tx_total_B", "tx_data_B", "tx_retx_B", "tx_dup_B", "tx_ctrl_B")) +
                     f" | {tr(sum(x or 0 for x in d), 0)} | " + " | ".join(tr(x, 0) for x in d) + f" | {tr(ht, 0)} | " + " | ".join(tr(x, 0) for x in h) +
                     f" | {tr(100 * (h[2] or 0) / ht, 1)} | {tr(100 * (h[1] or 0) / ht, 1)} | {tr(100 * (h[0] or 0) / ht, 1)} |")
    L += ["", "### §9.2 Kopyanın türü ve yolu (sayaç, medyan adet; birim 134 B tel)", "",
          "Yol: PARAM motor dışı (`runtime.rs:2558`, pacer yok) · kuruluş probu jetonsuz (`sender.rs:5995`) · kuyruk probu, geometrik, ARQ ve orijinal pacer'dan geçer (31 B/s tabanı bu yolu bağlar).", "",
          "| hücre | kol | PARAM | kuruluş probu | geometrik (dup − kuruluş) | kuyruk probu | ARQ (retx − kuyruk) | pacer yolu UDP B/s (150 s) | PreConverge çıkışı ms | durum sonu |",
          "|---|---|---|---|---|---|---|---|---|---|"]
    for c in r25c:
        for arm in KAMA:
            rows = [r for r in r25 if r["hucre"] == c and r["kol"] == arm]
            est, dup, tail, retx = rm(c, arm, "establish_tx"), rm(c, arm, "dup"), rm(c, arm, "tail_probes"), rm(c, arm, "retx")
            se = collections.Counter(r.get("state_end") for r in rows).most_common(2)
            pacer = ((rm(c, arm, "tx_data_B") or 0) + (rm(c, arm, "tx_retx_B") or 0) + max(0, (dup or 0) - (est or 0)) * 92) / 150.0
            L.append(f"| {c[2:]} | {arm} | {tr((rm(c, arm, 'tx_ctrl_B') or 0) / 51, 0)} | {tr(est, 0)} | {tr(max(0, (dup or 0) - (est or 0)), 0)} | {tr(tail, 0)} | {tr(max(0, (retx or 0) - (tail or 0)), 0)} | {tr(pacer, 1)} | {tr(rm(c, arm, 'left_preconv'), 0)} | {se} |")
    L += ["", "### §9.3 Ters oynatım [türetilmiş, geri beslemesiz]", "",
          "A çıkış dizisi sabit, netem FIFO'su yeniden hesaplanır. S0 hepsi · S1 yalnız ilk PARAM · S2 teslim etmeyen kopyalar yok · S3 S1+S2 · "
          "S4 PARAM katlamalı (ölçülen 25 denemeden 6'sı kalır) · S5 S4+S2. "
          "`hata` = S0'da gerçek vb0 varışına mutlak hata (p50 / p95, s). Kaldırılan paketin geri besleme etkisi modellenmez.", "",
          "| hücre | kol | hata p50 | hata p95 | D içinde S0 | S1 | S2 | S3 | S4 | S5 | p50 S0 | S1 | S2 | S3 | S4 | S5 | p95 S0 | S1 | S2 | S3 | S4 | S5 |",
          "|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|"]
    for c in r25c:
        for arm in KAMA:
            L.append(f"| {c[2:]} | {arm} | {tr(rm(c, arm, 'dogrulama_hata_p50_s'))} | {tr(rm(c, arm, 'dogrulama_hata_p95_s'))} | " +
                     " | ".join(tr(rm(c, arm, f"S{i}_D")) for i in range(6)) + " | " + " | ".join(tr(rm(c, arm, f"S{i}_p50"), 1) for i in range(6)) +
                     " | " + " | ".join(tr(rm(c, arm, f"S{i}_p95"), 1) for i in range(6)) + " |")
    L += ["", "### §9.4 Teslim sayımı (payda koşu; 60 s içinde, alıcı damgası)", "",
          "`2. mesaj` = S0 oynatımında ikinci mesajın tek yönü (s, medyan; doğrulama hatası §9.3).", "",
          "| hücre | kol | D = 1/6 koşu | D > 1/6 | D < 1/6 | D medyan | 2. mesaj tek yön s | rtt_samples medyan |", "|---|---|---|---|---|---|---|---|"]
    for c in r25c:
        for arm in KAMA:
            v = V.vals(c, arm, "rx_deliv_D")
            L.append(f"| {c[2:]} | {arm} | {sum(abs(x - 1 / 6) < 1e-6 for x in v)}/{len(v)} | {sum(x > 1 / 6 + 1e-6 for x in v)} | {sum(x < 1 / 6 - 1e-6 for x in v)} | {tr(statistics.median(v))} | {tr(rm(c, arm, 'm2_tek_yon_s'), 1)} | {tr(rm(c, arm, 'rtt_samples'), 1)} |")
    L += ["", "### §9.5 r25 dönüş yolu (medyan; hattan geçen L2 bayt)", "",
          "`ACK dışı` = dönüş toplam − ACK paketi baytı; l0'da bu PARAM_ACK'tir [türetilmiş].", "",
          "| hücre | kol | dönüş doluluk | dönüş toplam B | ACK paketi B | ACK dışı % |", "|---|---|---|---|---|---|"]
    for c in r25c:
        for arm in KAMA:
            to, ak = V.med(c, arm, "wr_total"), V.med(c, arm, "wr_ackpkt_l2")
            L.append(f"| {c[2:]} | {arm} | {tr(V.med(c, arm, 'wr_util'))} | {tr(to, 0)} | {tr(ak, 0)} | {tr(100 * (to - ak) / to if to else None, 1)} |")
    L.append("")

    # ── §10 A-10 ───────────────────────────────────────────────────────────────────────────────
    a10 = list(csv.DictReader(open(tam / "a10_kosular.csv")))

    def am(rows, k):
        v = []
        for r in rows:
            if r.get(k) not in ("", None):
                try:
                    v.append(float(r[k]))
                except ValueError:
                    pass
        return statistics.median(v) if v else None

    L += ["## §10 A-9 / A-10 hız çöküşü (Ajan A ürün yolu zaman serisi; 200 ms örnek)", "",
          "Girdi: `runs/tam/a/out/*/alice.jsonl` (a10_kok.py). Çöküş tanımı A'nınki: son örnekte txq ≥ 50 ve pacer < hattın %5'i. "
          "Payda: rinf hariç koşu (rinf'te hat hızı yok). Backlog = txq ≥ 50. `w oranı` = BtlBw tepesinden sona pencere başına çarpan.", "",
          "| hücre | kol | backlog koşu | çöküş | düşük-tepe | azalma | RecoverArq payı | DrainRtprop | ProbeUp | Startup | probe girişi | tepe BtlBw | son BtlBw | w oranı | pr = BtlBw payı |",
          "|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|"]
    a10c = sorted({r["hucre"] for r in a10 if r.get("backlog") == "1"})
    for c in a10c:
        for arm in KAMA:
            l = [r for r in a10 if r["hucre"] == c and r["kol"] == arm and r.get("backlog") == "1"]
            if not l:
                continue
            L.append(f"| {c[2:]} | {arm} | {len(l)} | {sum(r['cokus'] == '1' for r in l)} | {sum(r.get('faz') == 'dusuk-tepe' and r['cokus'] == '1' for r in l)} | {sum(r.get('faz') == 'azalma' for r in l)} | " +
                     " | ".join(tr(am(l, k)) for k in ("pay_RecoverArq", "pay_DrainRtprop", "pay_ProbeUp", "pay_Startup")) +
                     f" | {tr(am(l, 'probe_n'), 0)} | {tr(am(l, 'bw_peak'), 0)} | {tr(am(l, 'bw_end'), 0)} | {tr(am(l, 'w_oran'))} | {tr(am(l, 'pr_eq_bw_pay'))} |")
    tc = collections.Counter(r["kol"] for r in a10 if r["cokus"] == "1")
    tn = collections.Counter(r["kol"] for r in a10)
    fz = collections.Counter((r["kol"], r.get("faz")) for r in a10 if r["cokus"] == "1")
    L += ["", f"Toplam çöküş: taban {tc['taban']}/{tn['taban']}, aday {tc['aday']}/{tn['aday']} (Fisher p = {tr(fisher(tc['taban'], tn['taban'] - tc['taban'], tc['aday'], tn['aday'] - tc['aday']))}); "
          f"A'nın paydası 235 (rinf dahil). Faz: taban düşük-tepe {fz[('taban', 'dusuk-tepe')]}, azalma {fz[('taban', 'azalma')]}; aday düşük-tepe {fz[('aday', 'dusuk-tepe')]}, azalma {fz[('aday', 'azalma')]}.",
          "", "Pencere başına düşüş çarpanı, bütün backlog'lu koşular:"]
    for arm in KAMA:
        v = [float(r["w_oran"]) for r in a10 if r["kol"] == arm and r.get("w_oran") not in ("", None)]
        L.append(f"- {arm}: medyan {tr(statistics.median(v))} [{tr(pct(v, .25))}–{tr(pct(v, .75))}] (n = {len(v)} koşu)")
    L += ["", "MIN_CWND tavanı: backlog örneklerinde pacer × srtt = 4800 B ± %3 payı (`cap4800_pay`, koşu medyanı):"]
    for cc in ("a_r12k_d1000_l30_dosyali_dur", "a_r12k_d300_l30_dosyali"):
        for arm in KAMA:
            l = [r for r in a10 if r["hucre"] == cc and r["kol"] == arm and r.get("backlog") == "1"]
            L.append(f"- {cc[2:]} {arm}: {tr(am(l, 'cap4800_pay'))} (n = {len(l)} koşu)")
    L += ["", "Çöküş koşuları, faza göre (medyan):", "", "| kol | faz | koşu | backlog başı BtlBw | tepe BtlBw | son BtlBw | son pacer | son txq | backlog başı s |", "|---|---|---|---|---|---|---|---|---|"]
    for arm in KAMA:
        for fzn in ("dusuk-tepe", "azalma"):
            l = [r for r in a10 if r["kol"] == arm and r["cokus"] == "1" and r.get("faz") == fzn]
            if l:
                L.append(f"| {arm} | {fzn} | {len(l)} | {tr(am(l, 'bw_on'), 0)} | {tr(am(l, 'bw_peak'), 0)} | {tr(am(l, 'bw_end'), 0)} | {tr(am(l, 'pr_end'), 0)} | {tr(am(l, 'txq_end'), 0)} | {tr(am(l, 't_on'), 1)} |")
    L.append("")

    # ── §11 tahminler ──────────────────────────────────────────────────────────────────────────
    T = []

    def hucreler(rate=None, delay=None, loss=None, pay=None, kind=("hukum",)):
        out = []
        for c in cells:
            if rate and c["rate_id"] not in rate:
                continue
            if delay and c["delay_id"] not in delay:
                continue
            if loss is not None and c["loss"] not in loss:
                continue
            if pay and c["payload"] not in pay:
                continue
            if kind and c["kind"] not in kind:
                continue
            out.append(c)
        return out

    def t(no, parca, kapsam, sonuc, ayrinti):
        T.append({"T": no, "parca": parca, "kapsam": kapsam, "tutan": sonuc[0], "toplam": sonuc[1], "ayrinti": ayrinti})

    def say(cs, pred, arms=KAMA):
        ok = n = 0
        bad = []
        for c in cs:
            for arm in arms:
                if (c["cell"], arm) not in V.by:
                    continue
                r = pred(c, arm)
                if r is None:
                    continue
                n += 1
                if r:
                    ok += 1
                else:
                    bad.append(f"{c['cell'][2:]}/{arm}")
        return (ok, n), "; ".join(bad[:12]) + (" …" if len(bad) > 12 else "")

    m = V.med
    p1 = hucreler(rate=("r1k", "r12k"), loss=(0,))
    r, b = say(p1, lambda c, a: (m(c["cell"], a, "deliv_D") or 0) >= 0.999)
    t("T-1", "teslim D = 1", "r1k/r12k l0", r, b)
    r, b = say(p1, lambda c, a: 1.0 <= m(c["cell"], a, "rx_ow_p50_s") / c["oneway_fifo_s"] <= 1.3)
    t("T-1", "tek yön p50 / RTT_fifo tek yön ∈ [1,0; 1,3]", "r1k/r12k l0", r, b)
    r, b = say(p1, lambda c, a: 0 <= m(c["cell"], a, "conf_p50_s") - c["rtt_fifo_s"] <= c["ival_ms"] / 1000.0)
    t("T-1", "teyit p50 ∈ RTT_fifo + [0; I]", "r1k/r12k l0", r, b)
    r, b = say(hucreler(rate=("r1k", "r12k"), loss=(0,), pay=(65,)), lambda c, a: 0.44 <= m(c["cell"], a, "wf_useful_share") <= 0.485)
    t("T-1", "faydalı pay 65 B ∈ [0,44; 0,485]", "r1k/r12k l0 65 B", r, b)
    r, b = say(hucreler(rate=("r12k",), loss=(0,), pay=(1000,)), lambda c, a: 0.85 <= m(c["cell"], a, "wf_useful_share") <= 0.935)
    t("T-1", "faydalı pay 1 KB ∈ [0,85; 0,935]", "r12k l0 1 KB", r, b)
    r1k1 = hucreler(rate=("r1k",), pay=(1000,))
    r, b = say(r1k1, lambda c, a: m(c["cell"], a, "tail_probes") >= 0.4 * c["msgs"])
    t("T-2", "kuyruk probu ≥ 0,4 M", "r1k 1 KB", r, b)
    r, b = say(r1k1, lambda c, a: m(c["cell"], a, "cp_redund_arr") >= 0.4 * c["msgs"])
    t("T-2", "gereksiz kopya ≥ 0,4 M", "r1k 1 KB", r, b)
    r, b = say(r1k1, lambda c, a: m(c["cell"], a, "ack_hold_p95_s") >= 0.8 * c["ival_ms"] / 1000.0)
    t("T-2", "ACK tutma p95 ≥ 0,8 I", "r1k 1 KB", r, b)
    r, b = say(r1k1, lambda c, a: m(c["cell"], a, "tail_probes") >= 0.2 * c["msgs"])
    t("T-2", "çürütme eşiği: kuyruk probu ≥ 0,2 M", "r1k 1 KB", r, b)
    r25d = hucreler(rate=("r25",), delay=("d1000",))
    r, b = say(r25d, lambda c, a: m(c["cell"], a, "tail_probes") >= c["msgs"] - 1)
    t("T-3", "kuyruk probu ≥ M − 1", "r25 d1000", r, b)
    r, b = say(r25d, lambda c, a: (m(c["cell"], a, "rtt_samples") <= 3) if a == "taban" else (m(c["cell"], a, "rtt_samples") >= c["msgs"] - 2))
    t("T-3", "RTT örneği taban ≤ 3, aday ≥ M − 2", "r25 d1000", r, b)
    r25a = hucreler(rate=("r25",))
    r, b = say(r25a, lambda c, a: m(c["cell"], a, "establish_tx") == 4)
    t("T-4", "kuruluş probu = 4", "r25", r, b)
    r, b = say(r25a, lambda c, a: m(c["cell"], a, "establish_tx") > 2)
    t("T-4", "çürütme eşiği: kuruluş probu > 2", "r25", r, b)
    r25l20 = hucreler(rate=("r25",), loss=(20,))
    r, b = say(r25l20, lambda c, a: (m(c["cell"], a, "rx_ow_p95_s") or 1e9) > 12.0, ARMS)
    t("T-5", "K-3 (tek yön p95 ≤ 12 s) sağlanmaz", "r25 l20 (tcp dahil)", r, b)
    t6 = hucreler(rate=("r1k", "r12k"), loss=(10, 20, 30), pay=(65,))
    r, b = say(t6, lambda c, a: m(c["cell"], a, "ack_ranges_med") == 16)
    t("T-6", "ACK aralık medyanı = 16", "r1k/r12k 65 B l10–l30", r, b)
    r, b = say(t6, lambda c, a: m(c["cell"], a, "ack_ranges_med") >= 12)
    t("T-6", "çürütme eşiği: ≥ 12", "r1k/r12k 65 B l10–l30", r, b)
    t7 = hucreler()
    r, b = say(t7, lambda c, a: (m(c["cell"], a, "srtt_over_rttfifo") is not None and 0.9 <= m(c["cell"], a, "srtt_over_rttfifo") <= 1.4) if a == "aday" else None)
    t("T-7", "aday srtt/RTT_fifo ∈ [0,9; 1,4]", "bütün hüküm hücreleri", r, b)
    t7b = hucreler(rate=("r1k", "r12k"), loss=(20, 30))
    r, b = say(t7b, lambda c, a: (m(c["cell"], "taban", "srtt_over_rttfifo") >= m(c["cell"], "aday", "srtt_over_rttfifo")) if a == "aday" else None)
    t("T-7", "taban ≥ aday (medyan)", "r1k/r12k l20/l30", r, b)
    r, b = say([c for c in cells if c["rate_id"] == "r25" and c["cell"].endswith("_quinn")], lambda c, a: m(c["cell"], a, "deliv_D") == 0, ("quinn",))
    t("T-8", "quinn r25 teslim = 0", "r25 (N = 5, betimsel)", r, b)
    r, b = say(p1, lambda c, a: m(c["cell"], a, "deliv_D") >= 0.999, ("quinn",))
    t("T-8", "quinn l0 teslim = 1", "r1k/r12k l0", r, b)
    t9 = hucreler(rate=("r12k",), loss=(20, 30), pay=(65,))
    r, b = say(t9, lambda c, a: (m(c["cell"], "tcp", "rx_ow_p95_s") or 0) > m(c["cell"], a, "rx_ow_p95_s"))
    t("T-9", "tcp p95 > Kama p95", "r12k 65 B l20/l30", r, b)
    r, b = say(hucreler(loss=(0,)), lambda c, a: m(c["cell"], a, "parity") == 0)
    t("T-11", "l0'da parite = 0", "l0", r, b)
    r, b = say(hucreler(), lambda c, a: m(c["cell"], a, "cap_reject_total") == 0)
    t("T-12", "kapasite reddi = 0 (medyan)", "hüküm hücreleri", r, b)
    r, b = say(hucreler(), lambda c, a: max(V.vals(c["cell"], a, "cap_reject_total") or [0]) == 0)
    t("T-12", "kapasite reddi = 0 (bütün koşular)", "hüküm hücreleri", r, b)
    t13 = hucreler(loss=(80,), kind=("stres",))
    r, b = say(t13, lambda c, a: (m(c["cell"], a, "deliv_D") or 0) < 1)
    t("T-13", "teslim < 1", "l80 †", r, b)
    r, b = say(t13, lambda c, a: (m(c["cell"], a, "wf_util") or 0) >= 0.9)
    t("T-13", "hat doluluğu ≥ 0,9", "l80 †", r, b)
    r, b = say(r25a, lambda c, a: m(c["cell"], a, "param_sends") == 25)
    t("T-14", "PARAM = 25", "r25", r, b)
    r, b = say(r25a, lambda c, a: (m(c["cell"], a, "late_busy_ctrl_frac") or 0) >= 0.3)
    t("T-14", "geç kalmada hat PARAM ile ≥ 0,3", "r25", r, b)
    t15 = hucreler(rate=("r12k",), loss=(20, 30), pay=(65,))
    r, b = say(t15, lambda c, a: (sum(V.vals(c["cell"], a, "kilit_gonderen")) / len(V.vals(c["cell"], a, "kilit_gonderen"))) >= 0.5)
    t("T-15", "kilit_gönderen ≥ yarı", "r12k 65 B l20/l30", r, b)
    r, b = say(t15, lambda c, a: (sum(V.vals(c["cell"], a, "kilit_gonderen")) / len(V.vals(c["cell"], a, "kilit_gonderen"))) >= 0.2)
    t("T-15", "çürütme eşiği: ≥ %20", "r12k 65 B l20/l30", r, b)
    r, b = say(t15, lambda c, a: m(c["cell"], a, "queue_extra_p95_s") < 0.2)
    t("T-15", "kuyruk fazlası p95 < 0,2 s", "r12k 65 B l20/l30", r, b)
    t16 = hucreler(rate=("r1k",), loss=(20, 30), pay=(1000,))
    r, b = say(t16, lambda c, a: (m(c["cell"], a, "left_preconv") >= 10000) if a == "taban" else (m(c["cell"], a, "left_preconv") <= 5000))
    t("T-16", "PreConverge taban ≥ 10 s, aday ≤ 5 s", "r1k 1 KB l20/l30", r, b)
    r, b = say(t16, lambda c, a: (m(c["cell"], a, "left_preconv") >= 5000) if a == "taban" else None)
    t("T-16", "çürütme eşiği: taban ≥ 5 s", "r1k 1 KB l20/l30", r, b)
    kl = sum(1 for r_ in V.rows if r_.get("kirli_baslangic") == "1")
    t("T-17", "kirli başlangıç = 0", "4920 koşu", (len(V.rows) - kl, len(V.rows)), f"{kl} koşu")
    L += ["## §11 Tahmin tablosu (ölçüm, otomatik; yorum RAPOR-B TS.3)", "", "| T | parça | kapsam | tutan / kol×hücre | tutmayanlar (ilk 12) |", "|---|---|---|---|---|"]
    for x in T:
        L.append(f"| {x['T']} | {x['parca']} | {x['kapsam']} | {x['tutan']}/{x['toplam']} | {x['ayrinti']} |")
    L.append("")


    # ── §12 T-1 teyit dekompozisyonu ───────────────────────────────────────────────────────────
    L += ["## §12 T-1 sapma dekompozisyonu (l0, r1k/r12k)", "",
          "`teyit` = gönderen teyidi p50. `üst` = tahminin üst sınırı RTT_fifo + I. Bileşenler koşu medyanlarının medyanı: gönderen bekleme p50 · yolculuk p50 (A → B tel) · "
          "ACK tutma medyanı · dönüş yolculuğu medyanı. Medyanlar toplanmaz; `artık` = teyit − bileşen toplamı.", "",
          "| hücre | kol | teyit p50 | RTT_fifo | I | üst | sapma (teyit − üst) | gönderen | yolculuk | ACK tutma | dönüş | artık | tek yön p50 | tek yön / fifo |",
          "|---|---|---|---|---|---|---|---|---|---|---|---|---|---|"]
    for c in hucreler(rate=("r1k", "r12k"), loss=(0,)):
        cid = c["cell"]
        for arm in KAMA:
            mm = lambda k: V.med(cid, arm, k) or 0.0  # noqa: E731
            I = c["ival_ms"] / 1000.0
            bil = [mm("sender_wait_p50_s"), mm("sojourn_p50_s"), mm("ack_hold_med_s"), mm("rev_soj_med_s")]
            L.append(f"| {etiket(c)} | {arm} | {tr(mm('conf_p50_s'))} | {tr(c['rtt_fifo_s'])} | {tr(I)} | {tr(c['rtt_fifo_s'] + I)} | {tr(mm('conf_p50_s') - c['rtt_fifo_s'] - I)} | "
                     + " | ".join(tr(x) for x in bil) + f" | {tr(mm('conf_p50_s') - sum(bil))} | {tr(mm('rx_ow_p50_s'))} | {tr(mm('rx_ow_p50_s') / c['oneway_fifo_s'])} |")
    L.append("")

    # ── §13 S2 yan yana ────────────────────────────────────────────────────────────────────────
    sb = {(x["hucre"], x["kol"]): x for x in srows}

    def ta(cid, k, nd=1, k100=False):
        o = []
        for arm in KAMA:
            v = sb.get((cid, arm), {}).get(k)
            o.append(tr(v * 100 if (k100 and v is not None) else v, nd))
        return " / ".join(o)

    L += ["## §13 S2 yan yana (hüküm hücreleri; her sütun taban / aday; ileri yön, hattan geçen bayt üzerinden %)", "",
          "`kopya F tel` / `kopya G tel` = faydalı / gereksiz kopyanın L2 dahil payı (`başlık` ve `L2-4` sütunlarıyla kısmen örtüşür; toplamaya girmez). "
          "`faydalı` orijinal + ilk varan kopya yükü. Satır toplamı = faydalı + L2-4 + başlık + gereksiz + onarım + kontrol (medyanlar ayrı alındığından ±birkaç puan).", "",
          "| hücre | doluluk % | faydalı/geçen | faydalı/kapasite | L2-4 | başlık | gereksiz yük | onarım | kontrol | ACK ileri | kopya F tel | kopya G tel |",
          "|---|---|---|---|---|---|---|---|---|---|---|---|"]
    for c in hukum:
        cid = c["cell"]
        L.append(f"| {cid[2:]} | {ta(cid, 'wf_util', 1, True)} | {ta(cid, 'faydali_gecen')} | {ta(cid, 'faydali_kap')} | {ta(cid, 'l234')} | {ta(cid, 'baslik')} | {ta(cid, 'gereksiz')} | {ta(cid, 'onarim')} | {ta(cid, 'kontrol')} | {ta(cid, 'ack_ileri')} | {ta(cid, 'kopya_faydali_tel')} | {ta(cid, 'kopya_gereksiz_tel')} |")
    L += ["", "### §13.1 Dönüş yolu ve fiziksel ↔ gönderen ayrımı (taban / aday)", "",
          "`fazla` = ideal sınır üstü bekleme, mesaj·s; pay = fazla içinde gönderen / kurtarma / darboğaz kuyruğu. `tavan?` = ρ_ARQ ≥ 1 ise fiziksel tavan; bütün hüküm hücrelerinde ρ_ARQ < 1.", "",
          "| hücre | ρ_ARQ | dönüş doluluk % | dönüşte ACK % | dönüş kontrol % | fazla mesaj·s | gönderen % | kurtarma % | kuyruk % | geç mesaj / M | geçte hat boş | baskın kaynak |",
          "|---|---|---|---|---|---|---|---|---|---|---|---|"]

    def bil_str(cid, k):
        o = []
        for arm in KAMA:
            te, v = V.med(cid, arm, "exc_top_s"), V.med(cid, arm, k)
            o.append(tr(100 * v / te, 0) if te and v is not None else "–")
        return " / ".join(o)

    for c in hukum:
        cid = c["cell"]
        kay = []
        for arm in KAMA:
            mm = lambda k: V.med(cid, arm, k) or 0.0  # noqa: E731
            if mm("wf_util") >= 0.9:
                kay.append("hat dolu (" + max((("kontrol", mm("late_busy_ctrl_frac")), ("gereksiz", mm("late_busy_redund_frac")), ("faydalı", mm("late_busy_useful_frac"))), key=lambda x: x[1])[0] + ")")
            else:
                kay.append(max((("gönderen", mm("exc_gonderen_s")), ("kurtarma", mm("exc_kurtarma_s")), ("kuyruk", mm("exc_kuyruk_s"))), key=lambda x: x[1])[0])
        L.append(f"| {cid[2:]} | {tr(c['rho_fwd_arq'])} | {ta(cid, 'wr_util', 1, True)} | {ta(cid, 'ack_donus')} | {ta(cid, 'kontrol_donus')} | "
                 f"{tr(V.med(cid, 'taban', 'exc_top_s'), 0)} / {tr(V.med(cid, 'aday', 'exc_top_s'), 0)} | {bil_str(cid, 'exc_gonderen_s')} | {bil_str(cid, 'exc_kurtarma_s')} | {bil_str(cid, 'exc_kuyruk_s')} | "
                 f"{tr(V.med(cid, 'taban', 'late_msgs'), 0)} / {tr(V.med(cid, 'aday', 'late_msgs'), 0)} / {c['msgs']} | {tr(V.med(cid, 'taban', 'late_idle_frac'))} / {tr(V.med(cid, 'aday', 'late_idle_frac'))} | {kay[0]} / {kay[1]} |")
    L.append("")

    # ── §14 teslim + gecikme yan yana ──────────────────────────────────────────────────────────
    L += ["## §14 Teslim ve alıcı p95 yan yana", "",
          "Alıcı damgası (rx_log). p95 medyan [Q1–Q3]; (min–maks) §2.1'de. † betimsel.", "",
          "| hücre | N | D taban | D aday | D tcp | D quinn | p95 taban | p95 aday | p95 tcp | p95 quinn |", "|---|---|---|---|---|---|---|---|---|---|"]

    def q(cid, arm, k="rx_ow_p95_s"):
        v = V.vals(cid, arm, k)
        return f"{tr(pct(v, .5))} [{tr(pct(v, .25))}–{tr(pct(v, .75))}]" if v else "–"

    for c in cells:
        cid = c["cell"]
        if cid.endswith("_quinn") or (cid, "taban") not in V.by:
            continue
        qc = cid if (cid, "quinn") in V.by else cid + "_quinn"
        L.append(f"| {etiket(c)} | {len(V.by[(cid, 'taban')])} | " + " | ".join(tr(V.med(cid, x, "rx_deliv_D")) for x in ("taban", "aday", "tcp")) + f" | {tr(V.med(qc, 'quinn', 'rx_deliv_D'))}{'' if qc == cid else ' †'} | "
                 + " | ".join(q(cid, x) for x in ("taban", "aday", "tcp")) + f" | {q(qc, 'quinn')} |")
    L.append("")

    # ── §15 Kama ↔ rakip ───────────────────────────────────────────────────────────────────────
    L += ["## §15 Kama ↔ rakip farkı (hüküm hücreleri, N = 30/kol; %95 bootstrap, düzeltmesiz)", "",
          "p95: alıcı p95 farkı (Kama − rakip, s; eksi = Kama kısa). D: alıcı teslim D farkı. r25 quinn N = 5 olduğundan bu tabloda yok. "
          "`(n = …)`: rakipte p95 tanımsız koşu var (el sıkışamadı / çok az mesaj); fark yalnız tanımlı koşulardan, rakip lehine yanlı. Bench Kama kolu üst sınırdır (D-9: Noise el sıkışması yok); ilk mesaj kıyası buna duyarlıdır.", "",
          "| hücre | aday − quinn p95 | aday − quinn D | taban − quinn p95 | aday − tcp p95 | aday − tcp D | taban − tcp p95 |", "|---|---|---|---|---|---|---|"]
    rk = collections.Counter()

    def bd(cid, a1, a2, k):
        x, y = V.vals(cid, a1, k), V.vals(cid, a2, k)
        if len(x) < 20 or len(y) < 20:
            return "–"
        r = boot_diff(x, y, f"{cid}|{a1}|{a2}|{k}")
        sig = r[1] > 0 or r[2] < 0
        rk[(a1, a2, k, "n")] += 1
        if sig:
            rk[(a1, a2, k, "+" if r[0] > 0 else "-")] += 1
        ek_n = "" if min(len(x), len(y)) >= 30 else f" (n = {min(len(x), len(y))})"
        return f"{'**' if sig else ''}{tr(r[0])} [{tr(r[1])}; {tr(r[2])}]{'**' if sig else ''}{ek_n}"

    for c in hukum:
        cid = c["cell"]
        if cid.endswith("_quinn"):
            continue
        L.append(f"| {cid[2:]} | {bd(cid, 'aday', 'quinn', 'rx_ow_p95_s')} | {bd(cid, 'aday', 'quinn', 'rx_deliv_D')} | {bd(cid, 'taban', 'quinn', 'rx_ow_p95_s')} | {bd(cid, 'aday', 'tcp', 'rx_ow_p95_s')} | {bd(cid, 'aday', 'tcp', 'rx_deliv_D')} | {bd(cid, 'taban', 'tcp', 'rx_ow_p95_s')} |")
    L += ["", "Özet (aralık 0'ı dışlayan hücre, işaret; n = karşılaştırılan):"]
    for a1, a2 in (("aday", "quinn"), ("taban", "quinn"), ("aday", "tcp"), ("taban", "tcp")):
        for k in ("rx_ow_p95_s", "rx_deliv_D"):
            if rk[(a1, a2, k, "n")]:
                L.append(f"- {a1} − {a2} {k}: n = {rk[(a1, a2, k, 'n')]}; Kama büyük {rk[(a1, a2, k, '+')]}, Kama küçük {rk[(a1, a2, k, '-')]}")
    L.append("")

    # ── §16 r1k 1 KB ───────────────────────────────────────────────────────────────────────────
    L += ["## §16 r1k 1 KB: kuyruk probu, ACK tutma, PreConverge (B-K4, B-K8, B-K11)", "",
          "| hücre | kol | M | kuyruk probu | gereksiz kopya n | ACK tutma p95 s | kuruluş probu | PreConverge çıkışı s | gönderen bekleme p95 s | srtt/RTT_fifo | RTT örneği | alıcı p95 s | faydalı/geçen |",
          "|---|---|---|---|---|---|---|---|---|---|---|---|---|"]
    for c in hucreler(rate=("r1k",), pay=(1000,)):
        cid = c["cell"]
        for arm in KAMA:
            mm = lambda k: V.med(cid, arm, k)  # noqa: E731
            lp = mm("left_preconv")
            L.append(f"| {cid[2:]} | {arm} | {c['msgs']} | {tr(mm('tail_probes'), 1)} | {tr(mm('cp_redund_arr'), 1)} | {tr(mm('ack_hold_p95_s'))} | {tr(mm('establish_tx'), 1)} | {tr(lp / 1000 if lp is not None else None)} | {tr(mm('sender_wait_p95_s'))} | {tr(mm('srtt_over_rttfifo'))} | {tr(mm('rtt_samples'), 1)} | {tr(mm('rx_ow_p95_s'))} | {tr(mm('wf_useful_share'))} |")
    L.append("")

    # ── §17 ACK boyu ───────────────────────────────────────────────────────────────────────────
    L += ["## §17 ACK aralıkları ve dönüş baytı (B-K3)", "",
          "`aralık payı` [türetilmiş] = 4R / (73 + 4R); R = ACK aralık medyanı, ACK L2 = 73 + 4R B [kod: ack.rs]. `dönüş B/paket` = dönüş toplam bayt / paket (ACK + kontrol karışık).", "",
          "| hücre | kol | ACK aralık med | aralık payı | dönüş B/paket | dönüş doluluk | dönüşte ACK % |", "|---|---|---|---|---|---|---|"]
    for c in hukum:
        cid = c["cell"]
        if c["loss"] not in (0, 30):
            continue
        for arm in KAMA:
            al = []
            for r in V.by[(cid, arm)]:
                try:
                    n_, b_ = float(r.get("wr_pkts") or 0), float(r.get("wr_total") or 0)
                except ValueError:
                    continue
                if n_ > 0:
                    al.append(b_ / n_)
            rr = V.med(cid, arm, "ack_ranges_med") or 0.0
            L.append(f"| {cid[2:]} | {arm} | {tr(rr, 1)} | {tr(4 * rr / (73 + 4 * rr))} | {tr(statistics.median(al) if al else None, 1)} | {tr(V.med(cid, arm, 'wr_util'))} | {tr(sb[(cid, arm)]['ack_donus'], 1)} |")
    L.append("")

    # ── §18 FEC ────────────────────────────────────────────────────────────────────────────────
    L += ["## §18 Onarım (FEC) kullanımı", "",
          "Koşu başına toplam parite; havuz (hücre adı değil). Payda koşu.", "",
          "| havuz | kol | koşu | parite > 0 koşu | parite medyan (hepsi) | maks | tx_fec_B medyan | kurtarılan birim toplam | parite_denied medyan |", "|---|---|---|---|---|---|---|---|---|"]
    fp = collections.defaultdict(list)
    for c in cells:
        for arm in KAMA:
            for r in V.by.get((c["cell"], arm), []):
                fp[(c["kind"], c["rate_id"], c["loss"], c["payload"], arm)].append(r)

    def fl(r, k):
        try:
            return float(r.get(k) or 0)
        except ValueError:
            return 0.0

    for k in sorted(fp, key=lambda x: (x[0], {"r25": 0, "r1k": 1, "r12k": 2}[x[1]], x[2], x[3], x[4])):
        rs = fp[k]
        v = [fl(r, "parity") for r in rs]
        L.append(f"| {k[1]} l{k[2]} {k[3]} B{' †' if k[0] != 'hukum' else ''} | {k[4]} | {len(rs)} | {sum(x > 0 for x in v)} | {tr(statistics.median(v), 1)} | {tr(max(v), 0)} | {tr(statistics.median(fl(r, 'tx_fec_B') for r in rs), 0)} | {tr(sum(fl(r, 'srv_fec_recovered_units') for r in rs), 0)} | {tr(statistics.median(fl(r, 'parity_denied') for r in rs), 0)} |")
    L.append("")

    # ── §19 r12k_d1000 oran ────────────────────────────────────────────────────────────────────
    L += ["## §19 Pencere tavanı ve btlbw (r12k_d1000)", "",
          "`g_cwnd` = 2(1−p)·rtprop/srtt; < 1 ise pencere tavanı pacer'ı bağlıyor. `btlbw × (1−p) / sunulan`: < 1 ise pacer sunulan yükün altında.", "",
          "| hücre | kol | g_cwnd | rtprop/srtt | btlbw/sunulan | btlbw(1−p)/sunulan | kilit_gönderen | ileri doluluk | alıcı p50 s | alıcı p95 s |", "|---|---|---|---|---|---|---|---|---|---|"]
    for c in hucreler(rate=("r12k",), delay=("d1000",)):
        cid = c["cell"]
        for arm in ("taban", "aday"):
            bs = V.med(cid, arm, "btlbw_sunulan")
            kg = V.vals(cid, arm, "kilit_gonderen")
            L.append(f"| {cid[2:]} | {arm} | {tr(V.med(cid, arm, 'g_cwnd'))} | {tr(V.med(cid, arm, 'rtprop_srtt'))} | {tr(bs)} | {tr(bs * (1 - c['loss'] / 100.0) if bs else None)} | {int(sum(kg))}/{len(kg)} | {tr(V.med(cid, arm, 'wf_util'))} | {tr(V.med(cid, arm, 'rx_ow_p50_s'))} | {tr(V.med(cid, arm, 'rx_ow_p95_s'))} |")
    L.append("")


    # ── §20 pending_lost payı (ts.csv) ─────────────────────────────────────────────────────────
    L += ["## §20 Kurtarma baskısı: pending_lost > 0 payı (B ızgarası ts.csv, ≈ 50 ms örnek)", "",
          "Pay = t = 0 ile pending_lost > 0 olan son örnek arasındaki örneklerde pending_lost > 0 oranı (koşu medyanı). pending_lost > 0 iken `update_state` RecoverArq'a geçer (`sender.rs:3993`), kazanç 1 olur (`:4252`) ve ProbeUp başlamaz. "
          "ts.csv'de durum ve BtlBw sütunu yok; bu tablo durum payını değil, onu tetikleyen koşulu ölçer. Payda koşu.", "",
          "| hücre | kol | ts'li koşu | pending_lost > 0 payı | Q1–Q3 | tepe | son pending_lost anı s |", "|---|---|---|---|---|---|---|"]
    if kok:
        for c in hucreler(rate=("r12k", "r1k"), loss=(10, 20, 30)):
            if c["rate_id"] == "r1k" and c["loss"] != 30:
                continue
            cid = c["cell"]
            for arm in KAMA:
                pays, tep, son = [], [], []
                for rep in (kok / cid / arm).glob("rep*"):
                    f = rep / "ts.csv"
                    if not f.exists():
                        continue
                    with open(f) as h:
                        rr = [(int(x["t_ms"]), int(x["pending_lost"])) for x in csv.DictReader(h)]
                    i1 = max((i for i, x in enumerate(rr) if x[1] > 0), default=None)
                    if i1 is None:
                        pays.append(0.0)
                        tep.append(0)
                        continue
                    w = rr[:i1 + 1]
                    pays.append(sum(x[1] > 0 for x in w) / len(w))
                    tep.append(max(x[1] for x in w))
                    son.append(rr[i1][0] / 1000.0)
                if pays:
                    L.append(f"| {cid[2:]} | {arm} | {len(pays)} | {tr(statistics.median(pays))} | {tr(pct(pays, .25))}–{tr(pct(pays, .75))} | {tr(statistics.median(tep), 0)} | {tr(statistics.median(son) if son else None, 1)} |")
    L.append("")

    # ── §21 K-7 ────────────────────────────────────────────────────────────────────────────────
    L += ["## §21 K-7 israf ölçütleri (hüküm hücreleri)", "",
          "`oran` = kopya iletim sayısı / M (deneme düzlemi, koşu medyanı; hat kaybına uğrayan dahil). Sınır = 1,5 × p / (1 − p) (taslak §6 K-7). "
          "`tel/teslim` = ileri hattan geçen bayt / faydalı yük baytı. K-7 koşulu K-1 + K-2'dir; bu tablo B ızgarasında aynı ölçütü okur, K-7 hükmü değildir.", "",
          "| hücre | sınır | oran taban | oran aday | tel/teslim taban | aday | tcp | quinn |", "|---|---|---|---|---|---|---|---|"]
    k7 = collections.Counter()
    for c in hukum:
        cid = c["cell"]
        lim = 1.5 * (c["loss"] / 100.0) / (1 - c["loss"] / 100.0)
        ot = (V.med(cid, "taban", "cp_tx") or 0) / c["msgs"]
        oa = (V.med(cid, "aday", "cp_tx") or 0) / c["msgs"]
        k7["n"] += 1
        k7["t"] += ot <= lim
        k7["a"] += oa <= lim

        def tt(arm):
            v = []
            for r in V.by.get((cid, arm), []):
                try:
                    u = float(r.get("wf_useful_B") or 0)
                    if u > 0:
                        v.append(float(r.get("wf_total") or 0) / u)
                except ValueError:
                    pass
            return tr(statistics.median(v) if v else None, 2)

        L.append(f"| {cid[2:]} | {tr(lim)} | {tr(ot)} | {tr(oa)} | {tt('taban')} | {tt('aday')} | {tt('tcp')} | {tt('quinn')} |")
    L += ["", f"Oran ≤ sınır: taban {k7['t']}/{k7['n']}, aday {k7['a']}/{k7['n']} hücre.", ""]

    # ── §22 geçerlilik özeti ───────────────────────────────────────────────────────────────────
    L += ["## §22 Geçerlilik özeti (hücre başına)", "",
          "`N` = taban · aday · tcp · quinn. `el sıkışma yok` = o kolda hiç teslim yok ve kurulum D'ye dayandı (quinn · tcp). Kayıp: netem sayacı, ileri / dönüş, medyan (min–maks), iki Kama kolunun havuzu. rc ≠ 0: bütün tabloda 0.", "",
          "| hücre | N | rc ≠ 0 | el sıkışma yok quinn · tcp | ileri kayıp | dönüş kayıp |", "|---|---|---|---|---|---|"]

    def yuzde(v):
        return f"{tr(100 * pct(v, .5), 1)} ({tr(100 * min(v), 1)}–{tr(100 * max(v), 1)})" if v else "–"

    for c in cells:
        cid = c["cell"]
        if cid.endswith("_quinn"):
            continue
        qc = cid if (cid, "quinn") in V.by else cid + "_quinn"
        ns = [len(V.by.get((cid, a), [])) for a in ("taban", "aday", "tcp")] + [len(V.by.get((qc, "quinn"), []))]
        rcbad = sum(1 for a in ARMS for r in V.by.get((cid, a), []) if r["rc"] != "0") + sum(1 for r in V.by.get((qc, "quinn"), []) if r["rc"] != "0")

        def hs(cc, arm):
            return sum(1 for r in V.by.get((cc, arm), []) if r.get("deliv_any") in ("0", "0.0") and float(r.get("setup_s") or 0) >= 0.95 * c["deadline_s"])

        fw = V.vals(cid, "taban", "kayip_ileri") + V.vals(cid, "aday", "kayip_ileri")
        rv = V.vals(cid, "taban", "kayip_donus") + V.vals(cid, "aday", "kayip_donus")
        L.append(f"| {etiket(c)} | {' · '.join(map(str, ns))} | {rcbad} | {hs(qc, 'quinn')} · {hs(cid, 'tcp')} | {yuzde(fw)} | {yuzde(rv)} |")
    L.append("")

    # ── §23 srtt yan yana ──────────────────────────────────────────────────────────────────────
    L += ["## §23 Gerçek RTT ↔ motor srtt yan yana (hüküm hücreleri)", "",
          "s; srtt koşu sonu değeri, srtt = 0 (örnek yok) dışlanır. `oran` = srtt / RTT_fifo. `ağ RTT` = son 20 orijinal, alıcı tutması hariç. `n` = srtt > 0 koşu.", "",
          "| hücre | RTT_fiz | RTT_fifo | srtt taban | srtt aday | oran taban | oran aday | ağ RTT taban | ağ RTT aday | n taban | n aday |", "|---|---|---|---|---|---|---|---|---|---|---|"]
    for c in hukum:
        cid = c["cell"]
        L.append(f"| {cid[2:]} | {tr(c['rtt_phys_s'])} | {tr(c['rtt_fifo_s'])} | {tr(V.med(cid, 'taban', 'srtt_s'))} | {tr(V.med(cid, 'aday', 'srtt_s'))} | {tr(V.med(cid, 'taban', 'srtt_over_rttfifo'))} | {tr(V.med(cid, 'aday', 'srtt_over_rttfifo'))} | "
                 f"{tr(V.med(cid, 'taban', 'rtt_net_last20_med_s'))} | {tr(V.med(cid, 'aday', 'rtt_net_last20_med_s'))} | {len(V.vals(cid, 'taban', 'srtt_s'))} | {len(V.vals(cid, 'aday', 'srtt_s'))} |")
    L.append("")

    (tam / "TAM-OTOMATIK.md").write_text("\n".join(L) + "\n")
    with open(tam / "farklar.csv", "w", newline="") as f:
        w = csv.DictWriter(f, fieldnames=list(frows[0].keys()))
        w.writeheader()
        w.writerows(frows)
    with open(tam / "s2_paylar.csv", "w", newline="") as f:
        w = csv.DictWriter(f, fieldnames=list(srows[0].keys()))
        w.writeheader()
        w.writerows(srows)
    with open(tam / "tahmin.csv", "w", newline="") as f:
        w = csv.DictWriter(f, fieldnames=list(T[0].keys()))
        w.writeheader()
        w.writerows(T)
    print(f"{len(V.rows)} koşu · {len(cells)} hücre → {tam}/TAM-OTOMATIK.md")


if __name__ == "__main__":
    main()
