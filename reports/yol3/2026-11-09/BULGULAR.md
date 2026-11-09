# YOL3 — G2 teşhisi, E2a kıyası ve quinn kolu (2026-11-09)

Bu klasör üç ölçüm koşusunun ham verisini, okumalarını ve bulgularını tek yerde toplar; amacı bağımsız incelemedir. Her sayı bu klasördeki dosyalardan yeniden üretilebilir (§8).

Etiketler: [ölçüldü] koşulmuş veri · [kod] kaynakta açık (dosya:satır) · [türetilmiş] ölçülen sayılardan hesap · [çıkarım] doğrulanmamış açıklama.
Kapsam: N = 10 (hücre başına koşu), betimsel; N < 30 olduğu için istatistik test yok. Etiket: gösterim; kapılı PoC ölçümü Aralık; ARQ yolu, FEC kapalı (KK-ORK-01).

## 1. Ne ölçüldü

Gösterim analoğu hücresi `a_r12k_d150_l20_foto150k`:
- İki yön `delay 150ms loss 20% rate 100kbit` (12,5 kB/s), jitter yok.
- Sahne: 3 ısınma TEXT, 5 s'de 150 480 B dosya, 6 s'den 2,5 s aralıkla TEXT, 15 s'de PTT (25 demet × 80 B, 200 ms aralık). Tam parametreler `g2/KOSU-KAYDI.txt` ve ham verideki `alice.jsonl` başlığında.

| koşu | ağaç (origin) | kollar | ikili sha256 |
|---|---|---|---|
| G2 teşhisi | `yol2/o1` 5fdad83b = main dbea1ef3 + E1 bayrakları bd5492c5 + ölçüm aracı | K0, K0r, K1, K2, K2−c, K2−e (16 hücre; gösterim analoğu 6 kol) | `g2/KOSU-KAYDI.txt` |
| E2a kıyası | `yol2/o1-e2a` b3511591 = 5fdad83b + `yol3/m1-e2a-goster` 46c0d30d + 502fce94 | K1, K1a, K0a | `e2a/KOSU-KAYDI.txt` |
| quinn kolu | `yol3/quinn-kol` d93aef10 = 5fdad83b + f0-quinn | Q-bbr, Q-bbr-rfc, Q-cubic | `quinn/KOSU-KAYDI.txt` |

Kol tanımları:
- **K0:** E1 bayrakları kapalı (main davranışı).
- **K1:** E1 (c) başlangıç cwnd + (d) uygulamayla-sınırlı plato + DrainRtprop kapısı + (e-dar) probe_keep.
- **K2:** K1 + retx_first.
- **K0r:** yalnız retx_first.
- **K1a:** K1 + E2a (RFC 9002 zaman eşiği `tt_rfc` g = 1 ms, `rtt_rule_v11`, `iw_idle_hold`, `pto_probes = 2`).
- **K0a:** yalnız E2a.
- **Q-bbr:** quinn 0.11.11 / quinn-proto 0.11.18, BBR, ilk pencere 240 000 B.
- **Q-bbr-rfc:** aynı, ilk pencere 14 520 B (RFC 9002).
- **Q-cubic:** Cubic, ilk pencere 12 000 B.
- Ortak quinn ayarları: mesaj başına uni-stream, öncelik yok, MTU 1452.

Bütün Kama ağaçlarında şerit (T1) yok: PTT, dosya ile aynı sıralı kuyrukta.

## 2. Sonuçlar — gösterim analoğu hücresi [ölçüldü]

Tanımlar `oku.py` (i) ve (ii); koşu medyanları.

| kol | koşu | tamam | dosya bitiş med (s) | PTT p95 med (s) | kurulum med (s) |
|---|---|---|---|---|---|
| K0 | G2 | 9/10 | 23,9 | 15,29 | 0,021 |
| K0r | G2 | 7/10 | 26,7 | 16,37 | 0,021 |
| K1 | G2 | 10/10 | 19,2 | 8,63 | 0,021 |
| K2 | G2 | 10/10 | 17,5 | 9,24 | 0,021 |
| K2−c | G2 | 10/10 | 20,8 | 10,22 | 0,021 |
| K2−e | G2 | 10/10 | 19,7 | 9,21 | 0,021 |
| K1 | E2a | 10/10 | 20,4 | 9,82 | — |
| K1a | E2a | 10/10 | 21,3 | 10,69 | — |
| K0a | E2a | 10/10 | 23,3 | 12,73 | — |
| Q-bbr | quinn | 10/10 | 13,6 | 2,16 | 0,502 |
| Q-bbr-rfc | quinn | 10/10 | 14,8 | 1,51 | 0,502 |
| Q-cubic | quinn | 10/10 | 26,8 | 1,79 | 0,502 |

- Aynı K1 iki koşuda (G2 ↔ E2a; E2a kapalı, davranış aynı): PTT p95 8,63 ↔ 9,82 s, dosya 19,2 ↔ 20,4 s. Koşular arası aralık bu büyüklükte.
- G2 seçim kuralının (S) sonucu: "Gösterim seti: K1. E1 (manşet, tuzaksız): K2 1.02, K2−c 0.56." (`g2/OKUMA.md`).

## 3. Ham veriden ayrıştırma (`ayristirma_util.py`) [ölçüldü / türetilmiş]

Pencere: alice `file.sent → file.delivered`. Kaynak: gönderici ayna anlıkları (`k = "ts"`), `txb`/`rtxb`/`rtx` farkları.

| kol | koşu | n | tel hızı med (B/s) | retx payı rtxb/txb med | retx adet med |
|---|---|---|---|---|---|
| K0 | G2 | 8 | 9 185 | 0,206 | 50 |
| K0r | G2 | 7 | 8 011 | 0,199 | 46 |
| K1 | G2 | 10 | 10 318 | 0,198 | 46 |
| K2 | G2 | 10 | 11 617 | 0,197 | 50 |
| K1 | E2a | 10 | 10 033 | 0,202 | 48 |
| K1a | E2a | 10 | 9 686 | 0,221 | 58 |
| K0a | E2a | 10 | 8 853 | 0,200 | 48 |

`txb`'nin katmanı (mühürlü UDP yükü mü) doğrulanmadı. Pacer hedefi (`pr` medyanı, `e2a/OKUMA.md` R4): K1 12 644, K1a 11 826 B/s.

Faydalı dosya debisi = 150 480 B / dosya bitiş [türetilmiş]:

| kol | faydalı debi (kB/s) |
|---|---|
| K1 | 7,84 |
| K2 | 8,60 |
| Q-bbr | 11,06 |
| Q-bbr-rfc | 10,17 |
| Q-cubic | 5,61 |

## 4. Bulgular

**B1 — PTT farkı sıralamadan.**
- quinn aynı öncelikli akışları paket paket sırayla döndürüyor (quinn-proto 0.11.18 `connection/streams/mod.rs:393-426`, send_fairness varsayılan açık) [kod].
- Her PTT demeti ayrı akış olduğu için dosyanın arkasında beklemiyor. quinn öncelik kapalıyken PTT p95 1,51–2,16 s; Kama şeritsiz ağaçta 8,63–16,37 s [ölçüldü].
- Kama'da bunun karşılığı şeritlerdir (T1, `yol3/t1-seritler` 4c1b96cf); bu klasörde ölçülmedi.

**B2 — Kama hattı doldurmuyor.**
- Q-bbr'nin faydalı debisi 11,06 kB/s, 12,5 × (1 − 0,2) = 10 kB/s'nin üstünde. Bu, netem'in kaybı hız sınırlayıcıdan önce düşürdüğünü, kaybın hat kapasitesi harcamadığını gösteriyor [türetilmiş]. Faydalı tavan ≈ 12,5 kB/s, dosya alt sınırı ≈ 12 s (başlık hariç).
- Kama K1'in faydalı debisi 7,84 kB/s. İki kat görünüyor:
  - (a) Pacer hedefi teslim hızında (≈ 12,6 kB/s). Hattı doldurmak için ≈ 15,6 kB/s gerekir. Bu, teslim-hızı (1 − p) yanlılığı [türetilmiş].
  - (b) Tel bu hedefin de altında (≈ 10 kB/s) [ölçüldü]. Nedeni ayrıştırılmadı.

**B3 — Kayıp kuralı tek başına değiştirmedi.**
- K1a ↔ K1: PTT p95 10,69 ↔ 9,82 s, dosya 21,3 ↔ 20,4 s. Fark, K1'in koşular arası aralığının (8,63–9,82 s) içinde [ölçüldü].
- retx payı her kolda ≈ kayıp oranı (0,20). Yeniden gönderim israfı görünmüyor [ölçüldü].
- Şeritsiz ağaçta PTT, dosya boşalmasına bağlı olduğu için bu sonuç beklenebilir [çıkarım].

**B4 — İlk pencere quinn farkını açıklamıyor.** RFC ilk pencereli quinn (Q-bbr-rfc) dosya 14,8 s, PTT 1,51 s [ölçüldü].

**B5 — quinn-BBR'nin pacing_rate'i pacer'a bağlı değil.** quinn pacer'ı pencere ve srtt ile çalışıyor (`connection/pacing.rs`, `connection/mod.rs:625`); BBR'nin `pacing_rate` alanı yalnız metrik [kod]. Kama ↔ quinn "BBR eşdeğer" eşlemeleri bu yüzden düzeltilmeli.

**B6 — G2 seçimi K1; retx_first bu ağaçta yararsız.**
- K0r ↔ K0: PTT 16,37 ↔ 15,29 s; K0r'de 3/10 eksik [ölçüldü].
- Açıklama adayı: Kama kaybı geç ilan ediyor. Zaman eşiği tabanı 1,5·EAI, EAI = 2·birim/pace + 65 B / 31 B/s; bu hücrede ≈ 3,4 s [türetilmiş, `oku.py` formülü; `sender.rs` `expected_ack_interval`]. Bu yüzden retx_first'ün öne alacağı yeniden gönderim geç oluşuyor [çıkarım].
- (c) PTT'yi iyileştirdi (K1 8,63 ↔ K2−c 10,22 s). Şeritsiz ağaçta dosya erken bittiği için PTT erken çıkıyor [çıkarım].

## 5. Düzeltmeler (aynı gün)

- "Fiziksel alt sınır ≈ 15 s" yanlış; doğrusu ≈ 12 s (B2).
- "Öncelik olmadan hiçbir ARQ PTT'yi 3 s'ye indiremez" yanlış (B1).
- "K1 hız kilidini açıyor" ifadesi geri çekildi. Emisyon/sunulan 4,90; yeni veri, retx, prob ve parite paylarına ayrılmadan kullanılmaz.
- `oku.py` R4 "kayıp eşiği" sütunu formüldür; `tt_rfc` açık kollarda (K1a, K0a) geçersiz.

## 6. Adillik ve sınırlar (`seritler/QK-QUINN-KOL.md`)

1. El sıkışma: quinn'inki netem içinde, Kama zarfları Unix soketinden (netem dışı). İkisinde de pencere dışında; kurulum ayrı sütunda.
2. Öncelik: iki tarafta da yok.
3. Sıralama eşit değil. quinn akış başına bağımsız teslim yapıyor, Kama oturum geneli S1 sırası kullanıyor. Bu kol quinn için üst sınır.
4. Uygulama katmanı: quinn'de çerçeve en az, dosya tek akış.
5. quinn için motor sondası yok; (iii) ve R4 boş.

## 7. Açık kalemler ve kodlanmış ama ölçülmemiş işler

Açık kalemler:
- B2(b): tel hızının pacer hedefinin altında kalma nedeni.
- Şeritli ağaçta K1 ↔ quinn.

Kodlanmış, ölçülmemiş (origin dalları, bayraklar varsayılan kapalı):
- `yol3/q1-quinn-ack` 507417b7:
  - A-1 boşlukta anında ACK (`gap_ack`)
  - A-2 31 B/s önseli 1 kB/s altına kapılı (`return_prior_gated`)
  - A-5 PTO çapa son gönderim, tur tavanı yok (`pto_rfc`)
  - Rapor: `seritler/Q1-KAYIP-ACK.md`
- `yol3/q2-quinn-pacer-prob` c8450809:
  - A-3 kova tabanı 2·MTU (`a3_burst_floor`)
  - A-4 prob abort yalnız qd, periyot 4·srtt (`a4_probe_srtt`)
  - Rapor: `seritler/Q2-PACER-PROB.md`
- Bu maddeler bir karşılaştırma belgesinin §3-A paketinden alındı. O belge depoda değil.

## 8. Yeniden üretme

- Ham veri `ham/<koşu>/<hücre>.tar.zst`; sha256 listesi `ham-veri.sha256`.
- Açma: `zstd -dc ham/g2/a_r12k_d150_l20_foto150k__K1.tar.zst | tar -x`.
- Okuma: ilgili ağacın `scripts/yol3/tesis/oku.py --kok <kök> --manifest docs/design/E1-TESHIS-G2.md` (manifest sha256 37bcabe3…; `KOSU-KAYDI.txt`). Bu klasördeki `OKUMA.md` ve `okuma.json` o çıktının kendisi; sha256'ları `KOSU-KAYDI.txt` sonunda.
- Ayrıştırma: `python3 -I ayristirma_util.py <kök> K1 K1a K0a`.
- G2'nin ilk denemesinde 5 hücre oturum kesintisiyle yarım kaldı. Bu 5 hücre aynı ağaç ve ikiliyle baştan koşuldu. Yarım koşular okumaya girmedi ve bu klasörde yok (`g2/KOSU-KAYDI.txt`).
