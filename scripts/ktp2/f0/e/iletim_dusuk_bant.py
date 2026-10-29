#!/usr/bin/env python3
"""KTP-2 taslak 0.3 denetimi (Ajan E) — K-3 (düşük bant), K-2 (iki yönlü kayıp), §9, Ek A.4.

Kullanım: python3 -I scripts/ktp2/f0/e/iletim_dusuk_bant.py

Ortak model:
  - Tel boyu = yük + KTP-2 başlığı (zarf 22 + AKIŞ 9 = 31 B) + alt katman başlığı L.
    L ∈ {0 (taslak varsayımı: UDP yükü), 28 (IPv4 20 + UDP 8), 42 (Ethernet 14 + IPv4 20 + UDP 8;
    netem'in veth üzerinde saydığı çerçeve)}.
  - Serileştirme = tel boyu / hız; yayılım gecikmesi 0 (K-3 profilinde yazılı değil).
  - Kayıp bağımsız Bernoulli; "iletim sayısı" ileri yönde ilk başarılı alıma kadar (geometrik).
  - "n iletimin süresi" = n × serileştirme (art arda gönderim; ARQ beklemesi YOK) — taslağın
    kullandığı model budur; ARQ'lu yol ayrıca hesaplanır (K3-06).
"""
from fractions import Fraction as F
from math import ceil, log

ZARF = 22
AKIS = 9
KTP2 = ZARF + AKIS  # 31
ONARIM_BASLIK = 12
ACK_MIN = ZARF + 8 + 4  # ACK çerçevesi n=1 aralık: 22 + 8 + 4 = 34 B
L_SECENEK = [(0, "L yok"), (28, "IPv4+UDP 28"), (42, "Eth+IPv4+UDP 42")]


def n_guven(p, hedef):
    """P(n iletimde en az bir alım) >= hedef olan en küçük n (kesin karşılaştırma)."""
    n = 1
    while 1 - p**n < hedef:
        n += 1
    return n


def parca_boyu(hiz, t_ser):
    """§3.9: S = sınırla(hız × T_ser − başlık, 64, 1100)."""
    return max(64, min(1100, hiz * t_ser - KTP2))


def main():
    print("== K3-01  K-3 alt sınırlar: 65 B yük + 31 B = 96 B (taslak: 3,8 s / 30,7 s) ==")
    for L, ad in L_SECENEK:
        tel = 65 + KTP2 + L
        print(f"  {ad}: tel={tel} B → 25 B/s: {tel / 25:.3f} s ; 3,125 B/s: {tel / 3.125:.3f} s")

    print("== K3-02  K-3 '1 KB → 25 B/s ≈ 42 s' (tek paket mi, §3.9 parçalaması mı) ==")
    for yuk in (1024, 1000):
        tek = yuk + KTP2
        print(f"  yük {yuk} B tek paket: {tek} B → {tek / 25:.2f} s (L yok)")
        for t_ser, ad in ((8, "ses yok, T_ser 8 s"), (2, "ses etkin, T_ser 2 s")):
            S = parca_boyu(25, t_ser)
            n = ceil(yuk / S)
            for L, lad in L_SECENEK:
                tel = yuk + n * (KTP2 + L)
                print(f"    §3.9 ({ad}): S={S} B, {n} parça, {lad}: tel={tel} B → {tel / 25:.2f} s")
        for L, lad in L_SECENEK[1:]:
            print(f"    tek paket {lad}: {(tek + L) / 25:.2f} s")

    print("== K3-03  K-3 not: %80 kayıpta iletim sayısı (taslak: ortalama 5, %90 güven ≈ 11) ==")
    p = F(8, 10)
    ort = 1 / (1 - p)
    n90 = n_guven(p, F(9, 10))
    n95 = n_guven(p, F(95, 100))
    print(f"  ortalama 1/(1−p) = {ort} ; %90: n={n90} (ln0,1/ln0,8 = {log(0.1) / log(0.8):.3f}) ; %95: n={n95}")

    print("== K3-04  K-3 not: süreler (taslak: ortalama ≈ 2,6 dk, %90 ≈ 5,6 dk; spec: 1,5–2 dk) ==")
    print("  (a) taslağın okuması: 65 B YÜK + 31 B")
    print("  (b) spec'in yazdığı 'SESSION_DATA ~65B' = UDP yükü (ccr §35.6: 22 B + yük) → tel 65 B")
    for etiket, tel0 in (("(a) 96 B", 96), ("(b) 65 B", 65)):
        for L, ad in L_SECENEK:
            tel = tel0 + L
            t = tel / 3.125
            print(
                f"  {etiket} {ad}: tel={tel} B, iletim={t:.2f} s → ortalama 5×={5 * t / 60:.2f} dk,"
                f" %90 11×={11 * t / 60:.2f} dk, %95 14×={14 * t / 60:.2f} dk"
            )

    print("== K3-05  K-3 ölçüt '25 B/s %20 kayıpta p95 ≤ 3 × alt sınır (≈ 12 s)' ==")
    p = F(2, 10)
    print(f"  3 × 3,84 = {3 * 3.84:.2f} s ; %95 için gereken iletim n = {n_guven(p, F(95, 100))}")
    for L, ad in L_SECENEK:
        ser = (65 + KTP2 + L) / 25
        ser_on = (65 + ZARF + ONARIM_BASLIK + L) / 25
        ack = (ACK_MIN + L) / 25
        # (i) ileri onarım (§3.8 güvenilir akış kuralı, K=1: R = ⌈1·0,25·1,25⌉ = 1): kaynak + onarım art arda
        on = ser + ser_on
        # (ii) yalnız ARQ, tek mesaj: kayıp yalnız PTO ile anlaşılır. PTO zamanlayıcısı gönderim
        #      anından (sokete verildiği an) başlar; srtt ≥ ileri ser + ACK ser (yayılım 0).
        #      P(ilk kayıp)=0,2 > 0,05 → p95 ikinci iletimdedir: p95 = PTO + ser.
        #      QUIC biçimi PTO = srtt + 4·rttvar + max_ack_delay(25 ms).
        srtt = ser + ack
        arq_min = srtt + 0.025 + ser  # rttvar = 0 (sabit serileştirme, titreşim yok)
        rttvar_tavan = (3 * ser - ser - srtt - 0.025) / 4  # p95 ≤ 3·ser için izin verilen rttvar
        arq_ilk = 3 * srtt + 0.025 + ser  # ilk örnekte rttvar = srtt/2 (RFC 9002 §5.3)
        print(
            f"  {ad}: sınır 3×{ser:.2f}={3 * ser:.2f} s | ileri onarımla p95 ≤ {on:.2f} s"
            f" (P(ikisi de kayıp)=0,04) | yalnız ARQ: srtt≥{srtt:.2f} s, p95 ≥ {arq_min:.2f} s (rttvar=0),"
            f" {arq_ilk:.2f} s (rttvar=srtt/2); sınır için rttvar ≤ {rttvar_tavan:.2f} s (yayılım 0)"
        )

    print("== K2-01  K-2 '%80 iki yönlü kayıpta 1 KB için ideal ARQ ≈ 25 iletim' ==")
    qf = qr = F(2, 10)
    print(f"  teslim (alıcıya ilk varış) ortalaması: 1/q_ileri = {1 / qf}")
    print(f"  gönderenin teyit alması (her alınan kopyaya 1 ACK): 1/(q_ileri·q_dönüş) = {1 / (qf * qr)}")
    for r in (2, 3, 4):
        qa = 1 - (1 - qr) ** r
        print(f"  ACK her alımda {r} kez tekrarlanırsa: 1/(q_ileri·(1−0,8^{r})) = {float(1 / (qf * qa)):.2f}")
    for hiz in (1000, 12500):
        print(f"  1 KB'ın parça sayısı ({hiz} B/s, T_ser 8 s): S={parca_boyu(hiz, 8)} → {ceil(1024 / parca_boyu(hiz, 8))}")

    print("== S9-01  §9 '1,2 kbit/s ≈ 150 B/s' ve '%95'te ≈ 20 iletim' ==")
    print(f"  1200/8 = {1200 / 8} B/s")
    p = F(95, 100)
    print(f"  tek yön ortalama 1/(1−p) = {1 / (1 - p)} ; iki yön (her kopyaya 1 ACK) 1/(1−p)² = {1 / (1 - p) ** 2}")
    print(f"  %90 teslim için n = {n_guven(p, F(9, 10))}")
    print("  '§9 FEC'le 2–10 katı': yalnız FEC çarpanı 1,25 / 1,67 / 2 / 3,25 / 9 (tablo);")
    for T, yuk in ((200, 30), (40, 6)):
        tel = (yuk + KTP2) * 1000 // T
        print(
            f"    başlık dahil {T} ms demet: {tel} B/s = {tel / 150:.2f}×150 → FEC'le "
            + ", ".join(f"{tel * m / 150:.2f}×" for m in (1.25, 5 / 3, 2, 3.25, 9))
        )

    print("== A4-01  Ek A.4 '31 B/s'de tek ilk paket (1200 B) ≈ 39 s' ==")
    for L, ad in ((0, "UDP yükü"), (28, "+IPv4/UDP"), (42, "+Eth/IPv4/UDP")):
        print(f"  {ad}: {(1200 + L) / 31:.2f} s")


if __name__ == "__main__":
    main()
