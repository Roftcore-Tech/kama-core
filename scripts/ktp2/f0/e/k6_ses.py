#!/usr/bin/env python3
"""KTP-2 taslak 0.3 denetimi (Ajan E) — K-6 ses tel hızı, FEC'li ses hızı, §3.6 ses tavanı,
K-6 gecikme bütçesi, §3.9 T_ser ile kesilemeyen paket beklemesi, §3.2 ses kredisi.

Kullanım: python3 -I scripts/ktp2/f0/e/k6_ses.py

Model:
  - Kodlayıcı 1200 bit/s = 150 B/s; demet T ms → yük 150·T/1000 B, saniyede 1000/T paket.
  - Kaynak paket = yük + 31 (zarf 22 + AKIŞ 9); onarım paketi = yük + 34 (zarf 22 + ONARIM 12).
  - L = alt katman başlığı (0 / 28 IPv4+UDP / 42 Eth+IPv4+UDP), paket başına.
  - "Aynı boy" modeli: onarım paketi kaynakla aynı boyda (taslağın çarpan modeli).
  - Gecikme bütçesi (taslak K-6): D = K·T + ser + RTT/2 + titreşim.
"""
from fractions import Fraction as F
from math import ceil, floor

KTP2 = 31
ONARIM = 34
L_SECENEK = [(0, "L yok"), (28, "IPv4+UDP"), (42, "Eth+IPv4+UDP")]
TABLO = [(F(1, 10), 4, 1), (F(2, 10), 3, 2), (F(3, 10), 4, 4), (F(5, 10), 4, 9), (F(8, 10), 4, 32)]


def yuk(T):
    return F(150 * T, 1000)


def hiz_fec_siz(T, L=0):
    return (yuk(T) + KTP2 + L) * F(1000, T)


def hiz_fec(T, K, R, L=0, ayni_boy=False):
    src = yuk(T) + KTP2 + L
    rep = src if ayni_boy else yuk(T) + ONARIM + L
    return F(1000, T) / K * (K * src + R * rep)


def main():
    print("== K6-01  ses tel hızı, FEC'siz (taslak: 925 / 667 / 408 / 305 B/s; başlık payı %84 … %51) ==")
    for T, t in ((40, "925"), (60, "667"), (120, "408"), (200, "305")):
        pay = F(KTP2, yuk(T) + KTP2)
        satir = f"  T={T} ms: yük={float(yuk(T)):g} B, {float(hiz_fec_siz(T)):.2f} B/s (taslak {t}), başlık payı {float(pay) * 100:.1f}%"
        for L, ad in L_SECENEK[1:]:
            satir += f" | {ad}: {float(hiz_fec_siz(T, L)):.1f} B/s (pay {float(F(KTP2 + L, yuk(T) + KTP2 + L)) * 100:.1f}%)"
        print(satir)

    print("== K6-02  '%20 kayıpta 200 ms demetle ≈ 510 B/s ≈ 4 kbit/s' ==")
    for L, ad in L_SECENEK:
        a = hiz_fec(200, 3, 2, L, ayni_boy=True)
        b = hiz_fec(200, 3, 2, L)
        print(f"  {ad}: aynı boy {float(a):.2f} B/s ({float(a) * 8 / 1000:.2f} kbit/s) ; ONARIM 12 B başlıkla {float(b):.2f} B/s ({float(b) * 8 / 1000:.2f} kbit/s)")

    print("== K6-03  tablodaki (K,R) ile 200 ms demette ses hızı ve '4–8 kbit/s' ==")
    for p, K, R in TABLO:
        satir = f"  p={float(p):.2f} ({K},{R}):"
        for L, ad in L_SECENEK:
            v = hiz_fec(200, K, R, L)
            satir += f" {ad} {float(v):.1f} B/s = {float(v) * 8 / 1000:.2f} kbit/s;"
        print(satir)

    print("== K6-04  §3.6 ses tavanı = 1,5 × ses tel hızı (FEC'siz, K-6 tanımı) ile tablo çarpanı ==")
    for T in (60, 120, 200):
        tavan = F(3, 2) * hiz_fec_siz(T)
        satir = f"  T={T} ms: tavan {float(tavan):.1f} B/s →"
        for p, K, R in TABLO:
            gerek = hiz_fec(T, K, R)
            satir += f" p={float(p):.1f}: gerek {float(gerek):.1f} ({'SIĞAR' if gerek <= tavan else 'SIĞMAZ'});"
        print(satir)

    print("== K6-05  K-6 bütçe örneği: '200 + 200 + ser + 300 + titreşim → ≈ 750 ms + ser + titreşim' ==")
    T, K, RTT = 200, 2, 600
    print(f"  T_demet + (K−1)·T_demet + RTT/2 = {T} + {(K - 1) * T} + {RTT // 2} = {T + (K - 1) * T + RTT // 2} ms (taslak ≈ 750)")
    print("  tablodaki K ile (200 ms demet, RTT 600 ms; ser ve titreşim hariç):")
    for p, K, R in TABLO[:3]:
        print(f"    p={float(p):.2f} K={K}: K·T + RTT/2 = {K * T + RTT // 2} ms")
    print("  serileştirme terimi (K+R)·paket/hız örnekleri (200 ms demet, (3,2)):")
    for r in (500, 1000, 12500):
        for L, ad in L_SECENEK[::2]:
            pkt = float(yuk(200)) + KTP2 + L
            print(f"    hız {r} B/s {ad}: {5 * pkt / r * 1000:.0f} ms")

    print("== K6-06  [betimsel] D ≤ son tarih için azami demet T ≤ (D − RTT/2)/K (ser = titreşim = 0) ==")
    for D in (400, 800):
        for RTT in (600, 2000):
            kalan = D - RTT / 2
            if kalan <= 0:
                print(f"  D={D} RTT={RTT}: D − RTT/2 = {kalan:.0f} ms ≤ 0 → hiçbir demet/K ile olanaksız")
                continue
            print(f"  D={D} RTT={RTT}: K=3 → T ≤ {kalan / 3:.1f} ms ; K=4 → T ≤ {kalan / 4:.1f} ms")

    print("== K6-07  §3.9 T_ser (ses etkinken 2 s) — kesilemeyen arka plan paketinin ses beklemesi ==")
    for r in (500, 1000, 12500):
        S = max(64, min(1100, r * 2 - KTP2))
        for L, ad in L_SECENEK[::2]:
            print(f"  hız {r} B/s {ad}: S={S} B → paket {S + KTP2 + L} B → bekleme ≤ {(S + KTP2 + L) / r:.3f} s")

    print("== K6-08  §3.2 ses kredisi 'son tarih penceresi kadar parça' (W = ⌊D/T⌋) ile kredi döngüsü ==")
    print("  model: kredi = alıcıya varan (ya da süresi dolan) en yüksek sıra + W; kredi ACK'le döner →")
    print("  döngü ≈ RTT (+ser). Sürekli ses için W·T ≥ RTT; gönderilebilen çerçeve oranı ≤ min(1, W·T/RTT).")
    print("  (Yer, oynatma anında boşalıyorsa koşul W·T ≥ D + RTT/2 olur: W = D/T ile hiçbir RTT'de sağlanmaz.)")
    for D in (400, 800):
        for T in (60, 120, 200):
            W = floor(F(D, T))
            satir = f"  D={D} T={T}: W={W} parça →"
            for RTT in (600, 2000):
                oran = min(F(1), F(W * T, RTT))
                satir += f" RTT {RTT}: oran ≤ {float(oran) * 100:.0f}% (gerek W ≥ {ceil(F(RTT, T))});"
            print(satir)


if __name__ == "__main__":
    main()
