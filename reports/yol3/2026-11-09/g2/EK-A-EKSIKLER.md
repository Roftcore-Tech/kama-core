# EK-A §2 eksikleri — G2 teşhisinin ham veriden ayrıştırması (2026-11-10)

Sahip kararı v4 EK-A §2 (KK-M2-EKA-1…5) için. Kaynak: G2 teşhis koşusu (`yol2/o1` 5fdad83b; ikili sha256'ları `KOSU-KAYDI.txt`), ham veri `reports/yol3/2026-11-09/ham/g2/*.tar.zst` (16 hücre, 160 koşu; `ham-veri.sha256` ile doğrulandı). Teşhis manifesti `docs/design/E1-TESHIS-G2.md` (sha256 37bcabe3…) ve okuma (`OKUMA.md`) **değişmez**; bu belge seçim kuralını yeniden işletmez, yalnız EK-A §2'nin istediği ayrıştırmaları yazar.

- Etiketler: [ölçüldü] koşu verisi (sayaç, iz, yakalama) · [kod] kaynakta açık (ölçüm ağacı 5fdad83b, dosya:satır) · [türetilmiş] ölçülen sayılardan hesap · [çıkarım] doğrulanmamış açıklama.
- Birim = koşu; tablolar koşu medyanı (parantezde min–maks). N = 10, betimsel; N < 30, istatistik test yok, yalnız kesin olasılık satırları (§5 sonu).
- Tablolar `eka_eksikler.py` çıktısıdır (ondalık ayırıcı nokta). Tablo sütunlarının etiketi her tablonun altında.
- Etiket: gösterim; kapılı PoC ölçümü Aralık; ARQ yolu, FEC Aralık'ta; N < 30 betimsel. K1 deneysel gösterim profilidir; ürün çözümü ya da quinn'e karşı üstünlük iddiası değildir.

## 0. Özet

- **EKA-1:** bekleniyor (§1).
- **EKA-2:** 4,90 pacer **iznidir**, emisyon değildir. Dosyasız hücrede K2'nin PTT pencerelerinde tele çıkan yeni veri sunulan yükün 1,00 katı (K0 0,62); toplam tel 1,28 katı, bunun 0,26'sı yeniden iletim. Koşu boyunca faydalı teslim / gönderilen 0,61 (1 − p_ileri 0,70). Manşette (tuzaksız) K2 faydalı/gönderilen 0,69, K0 0,68.
- **EKA-3:** eksik koşu: analogda K0 2/10, K0r 3/10, E1 kolları 0/40; dosyasızda 0/30; manşette her kolda 2/10; l0 ve r25'te 0. K0r'nin üç eksiği aynı desen: Startup, BtlBw ısınma değerindeyken (250–324 B/s) bitti, BtlBw ≤ 392 B/s'de kilitlendi.
- **EKA-4:** K2'nin üç tuzaklı koşusunda ilk 42–66 s tel neredeyse boş; 60 s içinde teslim %2–43; sw95 59–69 s. sw95 9. sıra 63,75 s bu koşulardan biridir (K0'ın 9. sırası 34,89 s, tuzaksız bir koşu).
- **EKA-5:** r25'te 20/20 koşuda Startup'tan sonraki hiçbir duruma geçilmedi, ProbeUp 0. (d)'nin değiştirebileceği karar bu hücrede hiç oluşmuyor. Manşette K2'nin 10/10 koşusunda ProbeUp 0.

## 1. EKA-1 — R3 koruması K1'de

Orkestratörün gece koşusu (K1, N = 10, `b_r12k_d1000_l0_p1000` ve `b_r25_d300_l20_p65`) bekleniyor.

## 2. EKA-2 — emisyon/sunulan = 4,90 neyin oranı, pay nelerden oluşuyor

### 2.1 Sayaçlar ve katman

| sayaç | kaynak | katman | doğrulama |
|---|---|---|---|
| `pr` (oku.py (iii) payı) | `ActivationDiag.pacer_rate_bps` ← `Pacer::rate()` | pacer jeton hızı, B/s; jetonlar mühürlü datagram boyuyla düşer (`wire_len`, runtime.rs:2490, :3176) | [kod] |
| `txb` | runtime.rs `seal_send!` `stats.tx_bytes += sd.bytes.len()` (:1951) | mühürlü datagram = UDP yükü; deneme düzlemi | [kod] + [ölçüldü]: her A koşusunun ilk emisyonunda `txb` − birim = 33 B (DATA başlığı 11 + mühür 22) |
| `rtxb` | aynı noktada `TxClass::Retx` (:1954); `TransmitKind::Retransmit` → `Retx` (:2105) | UDP yükü | [kod]; kuyruk sondaları da `Retransmit` olarak çıkar (sender.rs:6034, :6042) → `rtxb` ARQ + sonda |
| `rtx`, `tlp`, `dup`, `par` | motor karar sayaçları (`ActivationDiag`) | adet | [kod]; `tlp` ⊂ `rtx` |
| yeni veri baytı | aynada sayaç **yok**; Transport'un kabul ettiği birimler (`tr` S kaydı `len`) + 33 B | UDP yükü | [türetilmiş]; datagram kapanışı `txd` = ilk iletilen birim + `rtx` + `dup` + `par` bütün A koşularında farksız (0) → CS-probe ya da PARAM tele çıkmadı, `txb − rtxb − yeni veri` = kopya baytı |
| faydalı teslim | bob `tr` D kayıtları (birim başına ilk), `len` + 33 B | UDP yükü katmanında (birim boyu + 33 B) | [türetilmiş] |
| B hücreleri | KP-CLIENT `tx=…B(data retx dup fec ctrl)` (UDP yükü); yakalamadan analiz_b sınıfları (`wfa_*`, L2) | UDP yükü / L2 | [ölçüldü] |
| parite | FEC kapalı (KK-ORK-01) | — | [ölçüldü]: `par` = 0 bütün koşularda |

### 2.2 4,90 nedir

- oku.py (iii) payı, alice `ts` örneklerinde bob oturumunun **pacer hızı** `pr`'dir (Idle örnekleri hariç, koşu medyanı); payda 430 B/s = (40 + 5 + 8 + 11 + 22) B / 0,2 s [kod: oku.py @502fce94 `acil_pacer_med`, `sunulan_ptt_bps`].
- `pr` gönderim izni verir, gönderim saymaz. Uygulamayla sınırlı akışta izin, tele çıkanın üstünde kalır.
- Betik oku.py sayısını yeniden üretiyor: K2 4,90, K2−c 3,69, K0 1,09 [ölçüldü/türetilmiş].
- **K1 ve K2−e dosyasız hücrede koşmadı.** Manifest §2 gereği onların (iii) değeri K2'ninkidir (oku.py `select`: `ratio_m["K1"] = ratio_m["K2me"] = ratio_m["K2"]`) [kod]. Gösterim seti K1 için bu hücrede ölçüm yok.

### 2.3–2.5 Ayrıştırma tabloları

#### a_r12k_d300_l30_dosyasiz — emisyon ayrıştırması (koşu medyanı; parantezde min–maks)

| kol | n | izin/sunulan (oku.py) | PTT penceresi: izin/sunulan | PTT penceresi: tel/sunulan | PTT penceresi: yeni veri/sunulan | PTT penceresi retx payı | koşu: yeni veri payı | koşu: retx payı | retx içinde sonda (tlp/rtx) | kopya adet | parite adet | datagram kapanış farkı | faydalı/gönderilen | 1 − p_ileri |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| K0 | 10 | 1.09 | 0.54 | 0.67 (0.21–1.34) | 0.62 (0.16–1.00) | 0.11 | 0.63 | 0.35 (0.33–0.39) | 0.50 | 4 | 0 | 0–0 | 0.63 (0.57–0.67) | 0.69 |
| K2 | 10 | 4.90 | 3.04 | 1.28 (0.47–1.66) | 1.00 (0.47–1.07) | 0.26 | 0.61 | 0.36 (0.32–0.40) | 0.52 | 2 | 0 | 0–0 | 0.61 (0.56–0.66) | 0.70 |
| K2−c | 10 | 3.69 | 1.91 | 1.32 (0.17–1.46) | 1.00 (0.17–1.00) | 0.24 | 0.59 | 0.36 (0.30–0.45) | 0.55 | 4 | 0 | 0–0 | 0.59 (0.55–0.68) | 0.69 |

#### a_r12k_d300_l30_dosyasiz — PTT pencereleri, mutlak (B/s, mühürlü UDP yükü; koşu × basış medyanı)

| kol | pencere sayısı | sunulan (pencerede kabul edilen birimler + 33 B) | pacer izni `pr` | tele çıkan | bunun yeniden iletimi | yeni veri (+ kopya) | pencerede rtx / tlp / kopya (medyan adet) |
|---|---|---|---|---|---|---|---|
| K0 | 20 | 481 | 264 | 334 | 28 | 296 | 2 / 0 / 0 |
| K2 | 20 | 481 | 1456 | 617 | 167 | 476 | 8 / 1 / 0 |
| K2−c | 20 | 481 | 920 | 639 | 156 | 465 | 9 / 1 / 0 |

#### a_r12k_d150_l20_foto150k — emisyon ayrıştırması (koşu medyanı; parantezde min–maks)

| kol | n | izin/sunulan (oku.py) | koşu: yeni veri payı | koşu: retx payı | retx içinde sonda (tlp/rtx) | kopya adet | parite adet | datagram kapanış farkı | faydalı/gönderilen | 1 − p_ileri |
|---|---|---|---|---|---|---|---|---|---|---|
| K0 | 10 | 9.94 | 0.79 | 0.21 (0.15–0.27) | 0.40 | 1 | 0 | 0–0 | 0.79 (0.73–0.84) | 0.80 |
| K0r | 10 | 6.40 | 0.79 | 0.21 (0.14–0.24) | 0.39 | 1 | 0 | 0–0 | 0.78 (0.75–0.86) | 0.80 |
| K1 | 10 | 19.46 | 0.79 | 0.21 (0.17–0.24) | 0.46 | 0 | 0 | 0–0 | 0.79 (0.76–0.83) | 0.81 |
| K2 | 10 | 12.79 | 0.79 | 0.21 (0.16–0.27) | 0.46 | 2 | 0 | 0–0 | 0.79 (0.72–0.83) | 0.79 |
| K2−c | 10 | 16.33 | 0.78 | 0.22 (0.18–0.23) | 0.42 | 1 | 0 | 0–0 | 0.78 (0.75–0.82) | 0.80 |
| K2−e | 10 | 13.37 | 0.81 | 0.19 (0.17–0.25) | 0.44 | 0 | 0 | 0–0 | 0.81 (0.75–0.82) | 0.80 |

#### b_r12k_d1000_l30_p1000 — emisyon ayrıştırması (KP-CLIENT UDP yükü; yakalama sınıfları L2)

| kol | küme | n | gönderim/gereken [3, 28) s | yeni veri payı | retx payı | kopya payı | retx adet (sonda) | faydalı/UDP gönderilen | faydalı/L2 enjekte | gereksiz kopya / L2 | kayıp orijinal / L2 | p_ileri (yakalama) |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| K0 | bütün | 10 | 0.55 | 0.70 | 0.29 | 0.01 | 41 (0) | 0.68 | 0.65 | 0.10 | 0.19 | 0.29 |
| K0 | tuzaksız | 10 | 0.55 | 0.70 | 0.29 | 0.01 | 41 (0) | 0.68 | 0.65 | 0.10 | 0.19 | 0.29 |
| K2 | bütün | 10 | 1.00 | 0.70 | 0.28 | 0.02 | 40 (0) | 0.69 | 0.66 | 0.08 | 0.20 | 0.28 |
| K2 | tuzaksız | 7 | 1.02 | 0.71 | 0.28 | 0.01 | 39 (0) | 0.69 | 0.66 | 0.08 | 0.19 | 0.28 |
| K2 | tuzaklı | 3 | 0.02 | 0.69 | 0.28 | 0.03 | 41 (1) | 0.64 | 0.62 | 0.10 | 0.21 | 0.32 |
| K2−c | bütün | 10 | 0.53 | 0.71 | 0.27 | 0.02 | 38 (1) | 0.67 | 0.64 | 0.10 | 0.19 | 0.30 |
| K2−c | tuzaksız | 8 | 0.56 | 0.71 | 0.27 | 0.02 | 38 (1) | 0.69 | 0.66 | 0.09 | 0.19 | 0.29 |
| K2−c | tuzaklı | 2 | 0.02 | 0.68 | 0.29 | 0.03 | 40 (0) | 0.62 | 0.60 | 0.12 | 0.22 | 0.34 |

Sütun etiketleri:
- izin/sunulan ve `pr`: [ölçüldü] pay, [türetilmiş] oran.
- tel, retx, kopya, parite, `rtx`/`tlp` adetleri: [ölçüldü].
- yeni veri, faydalı/gönderilen, PTT penceresi oranları: [türetilmiş]. Pencereler `ts` örnekleriyle (200 ms aralık) kesilir; pencere kenarı hatası ≤ 2 × 0,2 s / 5 s = %8 [türetilmiş].
- 1 − p_ileri: [ölçüldü] netem sayaçları; ileri yön (mb0, f0_cell.sh:49), koşu öncesi/sonrası farkı, ileri yöndeki bütün datagramlar.
- Manşet: gönderim/gereken oku.py R1 tanımı; yakalama sınıfları analiz_b `wfa_pay_*` (enjekte düzlem, L2) [ölçüldü].

### 2.6 Okuma

- **Dosyasız hücre (4,90'ın hücresi):**
  - PTT pencerelerinde ([15, 20) s ve [60, 65) s; kol başına 20 pencere) sunulan yük 481 B/s'dir: PTT 430 + aynı penceredeki metin ve sinyal [türetilmiş].
  - K2: izin 1 456 B/s (3,04 ×), tel 617 B/s (1,28 ×), yeniden iletim 167 B/s, yeni veri 476 B/s (1,00 ×) [ölçüldü/türetilmiş]. Gönderim uygulamayla sınırlı; izin fazlası kullanılmıyor.
  - K0: izin 264 B/s (0,54 ×), yeni veri 296 B/s (0,62 ×) [ölçüldü/türetilmiş]. K0 PTT yükünü taşıyamıyor; (iii)'ün ayırdığı fark bu.
  - Koşu boyunca K2'nin gönderdiği baytın 0,61'i yeni veri, 0,36'sı yeniden iletim. Yeniden iletim kararlarının 0,52'si kuyruk sondası. Faydalı teslim / gönderilen 0,61. İleri kayıp 0,30 iken yalnız kaybolan iletimler tekrarlansaydı oran 1 − p = 0,70 olurdu [türetilmiş]. Aradaki 0,09, kayıp telafisini aşan yeniden iletimdir (kaybolmamış birimin sondası ya da ACK'i kaybolan birimin tekrarı) [çıkarım; A hücrelerinde yakalama yok, ikisi ayrılamaz]. K0'da da aynı tablo: 0,63 ↔ 0,69.
- **Gösterim analoğu:** her kolda retx payı 0,19–0,22, faydalı/gönderilen 0,78–0,81, 1 − p 0,79–0,81 [ölçüldü/türetilmiş]. Dosyalı hücrede yeniden iletim kayıp telafisi düzeyinde. Bu hücrede PTT pencereleri dosyayla örtüştüğü için pencere oranları yazılmadı.
- **Manşet (R1'in hücresi):**
  - Tuzaksız K2: gönderim/gereken 1,02. Gönderilen UDP baytının 0,71'i yeni veri, 0,28'i yeniden iletim, 0,01'i kopya [ölçüldü].
  - Faydalı/UDP gönderilen 0,69 (K0 0,68; K2−c 0,69). Gereksiz kopya (alıcıda zaten olan birimin kopyası) L2'nin 0,08'i (K0 0,10). Kayıp orijinaller 0,19 (K0 0,19). Yakalamada ileri kayıp 0,28 [ölçüldü/türetilmiş].
  - K2'nin ek emisyonu kayıp telafisine gidiyor; gereksiz kopya payı K0'dan yüksek değil.
- **Sonuç cümlesi önerisi.** "Hız kilidini (c) açıyor" (EK-A §2) ve BULGULAR §5'te geri çekilen "K1 hız kilidini açıyor" yerine:

  > G2'de K2 (K1 + retx_first) manşetin tuzaksız koşularında gereken gönderim hızının 1,02 katına çıktı (K0 0,55). Gönderilen UDP baytının faydalı teslim payı K0 ile aynı düzeyde kaldı (0,69 ↔ 0,68); ek emisyon kayıp telafisine gitti. Dosyasız PTT hücresindeki 4,90 pacer iznidir: tele çıkan yeni veri sunulan yükün 1,00 katı (K0 0,62); toplam tel 1,28 katı, faydalı/gönderilen 0,61. K1 bu iki hücrede ölçülmedi; değerleri K2'nindir. Gösterim, kapılı ölçüm değil; N = 10.

## 3. EKA-3 — rc = 0 ≠ aktarım tamamlandı

#### A hücreleri — koşu başına tamamlanma

| hücre | kol | n | rc = 0 | alice `complete` | dosya: bob aldı | dosya: alice alındısı (≤ 60 s) | PTT kapandı | metin/sinyal tam | **eksik koşu** | eksik koşular (rep: neden) |
|---|---|---|---|---|---|---|---|---|---|---|
| a_r12k_d150_l20_foto150k | K0 | 10 | 10/10 | 9/10 | 9/10 | 8/10 | 9/10 | 9/10 | **2** | rep1: dosya bob'da 115 s; alice Outgoing(ReceiptTimeout) @ 60.0 s; rep3: bob'da eksik C 0/2, O 0/2, P 0/50, S 0/10, T 0/40; dosya bob'da yok; alice Outgoing(ReceiptTimeout) @ 60.0 s |
| a_r12k_d150_l20_foto150k | K0r | 10 | 10/10 | 7/10 | 7/10 | 7/10 | 7/10 | 7/10 | **3** | rep4: bob'da eksik C 0/2, O 0/2, P 0/50, S 0/10, T 0/40; dosya bob'da yok; alice Outgoing(ReceiptTimeout) @ 60.0 s; rep6: bob'da eksik C 0/2, O 0/2, P 0/50, S 0/10, T 0/40; dosya bob'da yok; alice Outgoing(ReceiptTimeout) @ 60.0 s; bob Incoming(IdleTimeout) @ 277 s; rep8: bob'da eksik C 0/2, O 0/2, P 0/50, S 0/10, T 0/40; dosya bob'da yok; alice Outgoing(ReceiptTimeout) @ 60.0 s |
| a_r12k_d150_l20_foto150k | K1 | 10 | 10/10 | 10/10 | 10/10 | 10/10 | 10/10 | 10/10 | **0** | — |
| a_r12k_d150_l20_foto150k | K2 | 10 | 10/10 | 10/10 | 10/10 | 10/10 | 10/10 | 10/10 | **0** | — |
| a_r12k_d150_l20_foto150k | K2−c | 10 | 10/10 | 10/10 | 10/10 | 10/10 | 10/10 | 10/10 | **0** | — |
| a_r12k_d150_l20_foto150k | K2−e | 10 | 10/10 | 10/10 | 10/10 | 10/10 | 10/10 | 10/10 | **0** | — |
| a_r12k_d300_l30_dosyasiz | K0 | 10 | 10/10 | 10/10 | — | — | 10/10 | 10/10 | **0** | — |
| a_r12k_d300_l30_dosyasiz | K2 | 10 | 10/10 | 10/10 | — | — | 10/10 | 10/10 | **0** | — |
| a_r12k_d300_l30_dosyasiz | K2−c | 10 | 10/10 | 10/10 | — | — | 10/10 | 10/10 | **0** | — |

#### B hücreleri — koşu başına tamamlanma

| hücre | kol | n | rc = 0 | koşu sonunda bütün mesajlar alıcıda | teslim@D = 1 | **eksik koşu** (alıcıda eksik mesaj) | eksik koşular (rep: alıcıda / sunulan) |
|---|---|---|---|---|---|---|---|
| b_r12k_d1000_l30_p1000 | K0 | 10 | 10/10 | 8/10 | 8/10 | **2** | rep3: 99/100; rep10: 97/100 |
| b_r12k_d1000_l30_p1000 | K2 | 10 | 10/10 | 8/10 | 7/10 | **2** | rep6: 96/100; rep7: 93/100 |
| b_r12k_d1000_l30_p1000 | K2−c | 10 | 10/10 | 8/10 | 6/10 | **2** | rep5: 64/100; rep10: 49/100 |
| b_r12k_d1000_l0_p1000 | K0 | 10 | 10/10 | 10/10 | 10/10 | **0** | — |
| b_r12k_d1000_l0_p1000 | K2 | 10 | 10/10 | 10/10 | 10/10 | **0** | — |
| b_r25_d300_l20_p65 | K0 | 10 | 10/10 | 10/10 | 9/10 | **0** | — |
| b_r25_d300_l20_p65 | K2 | 10 | 10/10 | 10/10 | 8/10 | **0** | — |

#### a_r12k_d150_l20_foto150k — eksik koşular: Startup çıkışı ve hız kilidi

| kol | rep | Startup sonrası ilk durum @ s | o andaki BtlBw (B/s) | koşu BtlBw maks | pacer `pr` medyanı [10, 60) s | tel [5, 65) s (B/s) | ProbeUp girişi | rtx (koşu) | bob'da teslim birimi (dosya parçası / toplam) | sıralı serbest bırakma (sonraki) | koşu sonu Transport kuyruğu |
|---|---|---|---|---|---|---|---|---|---|---|---|
| K0 | 1 | Steady @ 5.5 | 260 | 5201 | 1385 | 1065 | 5 | 65 | 261 (152/152) | 261 | 0 |
| K0 | 3 | Drain @ 6.3 | 336 | 554 | 216 | 195 | 10 | 27 | 138 (134/152) | 136 | 122 |
| K0r | 4 | Drain @ 7.8 | 324 | 324 | 324 | 321 | 5 | 19 | 52 (48/152) | 51 | 208 |
| K0r | 6 | Drain @ 10.4 | 259 | 332 | 193 | 246 | 5 | 17 | 64 (60/152) | 64 | 196 |
| K0r | 8 | Steady @ 7.7 | 250 | 392 | 187 | 232 | 5 | 13 | 61 (57/152) | 61 | 200 |

Tamamlanan koşular (karşılaştırma; aralık min–maks):

| kol | n | Startup sonrası ilk durum anı (s) | o andaki BtlBw (B/s) | pacer `pr` medyanı [10, 60) s | tel [5, 65) s (B/s) |
|---|---|---|---|---|---|
| K0 | 8 | 8.1–30.1 | 12015–12022 | 10772–12022 | 3451–3862 |
| K0r | 7 | 10.6–23.4 | 9378–12022 | 9013–12020 | 3425–3779 |
| K1 | 10 | 13.0–28.4 | 12013–12018 | 12014–15021 | 3403–3733 |
| K2 | 10 | 9.9–23.6 | 11854–12284 | 11926–14989 | 3380–3914 |
| K2−c | 10 | 10.2–25.4 | 12010–12019 | 12018–15022 | 3452–3799 |
| K2−e | 10 | 9.8–22.2 | 10427–12087 | 10427–12255 | 3407–3755 |

Sütun etiketleri: bütün sayımlar [ölçüldü] (`rc`, alice/bob özetleri `sent`/`counts`/`file`, `tr` izleri, ayna). Tamamlandı = (dosya varsa bob dosyayı eksiksiz aldı) ∧ (her basışın kapanış iletisi C ve bütün demetler bob'da) ∧ (gönderilen her metin/sinyal bob'da). "Eksik koşu" ayrıca alice'in dosya alındısını 60 s içinde alamadığı koşuyu da sayar (`FILE_RECEIPT_TTL` = 60 s, kama-app file_transfer.rs:267). B hücrelerinde teslim@D: D = 60 s (fizik.py hücre geometrisi `deadline_s`). "Startup sonrası ilk durum" = Startup'tan sonra erişilebilen ilk durum örneği (Drain, Steady, ProbeUp, DrainRtprop).

**K0r'deki 3 eksik koşunun nedeni (tek satır):** dosya 5 s'de başladıktan 2,7–5,4 s sonra Startup, BtlBw ısınma metinlerinden öğrenilmiş değerdeyken (250–324 B/s) bitti. BtlBw koşu boyunca ≤ 392 B/s'de kilitli kaldı, pacer [10, 60) s'de 187–324 B/s. 152 parçalık dosya tek sıralı kuyrukta önde olduğu için bob 404 s'de dosyanın 48–60 parçası ve 3 ısınma metni dışında hiçbir birim almadı. alice dosyayı 60 s'de `ReceiptTimeout` ile kapattı; rc = 0, çünkü süreçler normal çıktı [ölçüldü].

- Ne **değil** [ölçüldü]:
  - ARQ tavanı (HandToForward) değil: koşu başına `rtx` 13–19.
  - HOL boşluğu değil: sıralı serbest bırakma teslimle aynı (51/52, 64/64, 61/61).
  - Alıcı zaman aşımı değil: üç koşuda da alice 60,0 s'de `ReceiptTimeout` gördü. rep6'da bob dosyayı 277 s'de `Incoming(IdleTimeout)` ile düşürdü (60 s yeni parça yok); bu kilidin sonucudur [çıkarım].
- K0 rep3 aynı desen (Drain @ 6,3 s, BtlBw 336). K0 rep1 başka: Startup 5,5 s'de BtlBw 260 B/s'deyken bitti, ama BtlBw sonra 5 201 B/s'ye çıktı (tel [5, 65) s 1 065 B/s). bob dosyayı 115 s'de aldı, alice 60 s'de `ReceiptTimeout` gördü. Bu, file_transfer.rs:102'deki bilinen sınırdır (alıcıda olan dosya için gönderende "başarısız").
- Tamamlanan koşularda Startup sonrası ilk durum 8,1–30,1 s'de ve BtlBw 9 378–12 284 B/s iken geliyor [ölçüldü].
- Kilit deseni E1 kollarının 40 koşusunda yok, K0 + K0r'de 4/20 [ölçüldü]. Olayların hepsinin K0 + K0r'ye düşme olasılığı p = 0,0099 [türetilmiş]. K0r ↔ K0 farkı (3/10 ↔ 1/10) bu veriden ayrılamaz (p = 0,291) [türetilmiş].
- Dört E1 kolunun ortak bayrakları (d) ve DrainRtprop.
- Aday mekanizma (d) [çıkarım; plato sayacı aynada yok, doğrulanmadı]:
  - Kodda `base_is_startup` yalnız platoda düşer (sender.rs:4012-4013). Startup sayacı yalnız gerçek girişte sıfırlanır: `state == Startup ∧ !base_is_startup` (:4187) [kod].
  - Isınmadan sonraki Idle → Startup (dosya, 5 s) bu yüzden sayacı sıfırlamaz. (d) kapalıyken ısınmanın uygulamayla sınırlı turları plato sayacında kalır; dosya başlayınca Startup az turla bitebilir.
- Startup'tan sonra ProbeUp BtlBw'yi yükseltemiyor: p = 0,2'de prob teslimi 1,25 × 0,8 = 1,00 < 1,025 başarı eşiği (KK-M2-12 aritmetiği) [türetilmiş]. Ölçülen ProbeUp girişleri: 5–10.

## 4. EKA-4 — tuzaklı koşular

#### b_r12k_d1000_l30_p1000 — koşu başına (tuzaklı = PreConverge çıkışı yok ya da > 10 s; oku.py R1 tanımı)

| kol | rep | PreConverge çıkışı (s) | tuzak | gönderim/gereken | sw95 (s) | teslim p95 (s) | ≤ 10 s | ≤ 30 s | ≤ 60 s | koşu sonunda alıcıda | süre (s) | son durum | BtlBw son (B/s) |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| K0 | 1 | 2.6 |  | 0.44 | 16.49 | 31.61 | 0.08 | 0.90 | 1.00 | 100/100 | 54.6 | Idle | 4491 |
| K0 | 2 | 2.5 |  | 0.92 | 4.72 | 11.21 | 0.89 | 1.00 | 1.00 | 100/100 | 48.5 | Idle | 4297 |
| K0 | 3 | 2.8 |  | 0.41 | 34.89 | 49.97 | 0.11 | 0.65 | 0.95 | 99/100 | 88.3 | DrainRtprop | 1459 |
| K0 | 4 | 2.5 |  | 0.64 | 8.93 | 23.57 | 0.43 | 1.00 | 1.00 | 100/100 | 39.5 | Idle | 5902 |
| K0 | 5 | 1.8 |  | 0.87 | 5.18 | 15.42 | 0.78 | 1.00 | 1.00 | 100/100 | 38.8 | Idle | 5087 |
| K0 | 6 | 3.2 |  | 0.45 | 18.15 | 29.79 | 0.12 | 0.95 | 1.00 | 100/100 | 65.5 | DrainRtprop | 3698 |
| K0 | 7 | 1.9 |  | 0.79 | 5.20 | 16.91 | 0.78 | 1.00 | 1.00 | 100/100 | 40.4 | Idle | 5430 |
| K0 | 8 | 2.1 |  | 0.68 | 8.06 | 21.60 | 0.57 | 1.00 | 1.00 | 100/100 | 44.9 | Idle | 5261 |
| K0 | 9 | 4.7 |  | 0.45 | 16.56 | 31.71 | 0.04 | 0.92 | 1.00 | 100/100 | 60.9 | Idle | 2922 |
| K0 | 10 | 2.8 |  | 0.40 | 36.22 | 57.96 | 0.12 | 0.55 | 0.90 | 97/100 | 88.3 | DrainRtprop | 1245 |
| K2 | 1 | 2.4 |  | 1.08 | 3.84 | 9.97 | 0.95 | 1.00 | 1.00 | 100/100 | 40.9 | Idle | 6086 |
| K2 | 2 | 56.8 | **tuzaklı** | 0.02 | 59.22 | 65.02 | 0.02 | 0.03 | 0.43 | 100/100 | 85.7 | Startup | 8107 |
| K2 | 3 | 3.8 |  | 0.41 | 17.64 | 22.89 | 0.02 | 1.00 | 1.00 | 100/100 | 45.9 | Idle | 8001 |
| K2 | 4 | 2.1 |  | 1.02 | 3.51 | 8.91 | 0.95 | 1.00 | 1.00 | 100/100 | 44.0 | Idle | 3962 |
| K2 | 5 | 1.8 |  | 1.02 | 4.23 | 10.89 | 0.92 | 1.00 | 1.00 | 100/100 | 35.8 | Idle | 8502 |
| K2 | 6 | 42.2 | **tuzaklı** | 0.02 | 63.75 | 69.66 | 0.01 | 0.01 | 0.14 | 96/100 | 88.3 | Startup | 9369 |
| K2 | 7 | 66.0 | **tuzaklı** | 0.03 | 69.12 | 74.95 | 0.01 | 0.01 | 0.02 | 93/100 | 88.3 | Startup | 10242 |
| K2 | 8 | 2.2 |  | 1.03 | 2.82 | 8.60 | 0.97 | 1.00 | 1.00 | 100/100 | 31.0 | Idle | 4909 |
| K2 | 9 | 2.4 |  | 1.11 | 8.33 | 16.27 | 0.62 | 1.00 | 1.00 | 100/100 | 55.3 | Idle | 6981 |
| K2 | 10 | 1.9 |  | 0.98 | 1.97 | 8.83 | 0.97 | 1.00 | 1.00 | 100/100 | 38.0 | Idle | 3476 |
| K2−c | 1 | 2.3 |  | 0.57 | 20.88 | 25.21 | 0.20 | 0.99 | 1.00 | 100/100 | 54.9 | Idle | 2399 |
| K2−c | 2 | 2.3 |  | 0.55 | 29.89 | 36.72 | 0.18 | 0.82 | 0.99 | 100/100 | 71.1 | Steady | 1623 |
| K2−c | 3 | 29.6 | **tuzaklı** | 0.02 | 31.15 | 40.02 | 0.01 | 0.02 | 1.00 | 100/100 | 69.4 | Idle | 4824 |
| K2−c | 4 | 1.8 |  | 0.60 | 11.46 | 21.40 | 0.26 | 1.00 | 1.00 | 100/100 | 53.2 | Idle | 4325 |
| K2−c | 5 | 62.8 | **tuzaklı** | 0.02 | 68.20 | 74.74 | 0.01 | 0.01 | 0.02 | 64/100 | 88.3 | Startup | 2986 |
| K2−c | 6 | 1.5 |  | 0.50 | 31.66 | 36.87 | 0.24 | 0.79 | 0.99 | 100/100 | 68.7 | Idle | 2099 |
| K2−c | 7 | 2.6 |  | 0.47 | 14.43 | 20.73 | 0.17 | 1.00 | 1.00 | 100/100 | 47.0 | Idle | 6198 |
| K2−c | 8 | 1.6 |  | 0.96 | 3.24 | 9.68 | 0.96 | 1.00 | 1.00 | 100/100 | 37.2 | Idle | 4429 |
| K2−c | 9 | 2.2 |  | 0.85 | 5.16 | 13.38 | 0.80 | 1.00 | 1.00 | 100/100 | 52.0 | Idle | 4690 |
| K2−c | 10 | 5.6 |  | 0.14 | 66.56 | 70.76 | 0.02 | 0.13 | 0.35 | 49/100 | 88.3 | Steady | 967 |

#### Dışlamalı ve dışlamasız medyanlar

| kol | küme | n | gönderim/gereken med | sw95 med | sw95 9. sıra | teslim p95 med | ≤ 60 s med | ≤ 60 s en kötü |
|---|---|---|---|---|---|---|---|---|
| K0 | bütün | 10 | 0.55 | 12.71 | 34.89 | 26.68 | 1.00 | 0.90 |
| K0 | tuzaksız | 10 | 0.55 | 12.71 | 34.89 | 26.68 | 1.00 | 0.90 |
| K2 | bütün | 10 | 1.00 | 6.28 | 63.75 | 13.58 | 1.00 | 0.02 |
| K2 | tuzaksız | 7 | 1.02 | 3.84 | — | 9.97 | 1.00 | 1.00 |
| K2 | tuzaklı | 3 | 0.02 | 63.75 | — | 69.66 | 0.14 | 0.02 |
| K2−c | bütün | 10 | 0.53 | 25.39 | 66.56 | 30.96 | 1.00 | 0.02 |
| K2−c | tuzaksız | 8 | 0.56 | 17.65 | — | 23.30 | 1.00 | 0.35 |
| K2−c | tuzaklı | 2 | 0.02 | 49.68 | — | 57.38 | 0.51 | 0.02 |

Sütun etiketleri:
- PreConverge çıkışı, sw95, teslim p95, ≤ X s teslim payı, alıcıdaki mesaj, son durum, BtlBw: [ölçüldü] (analiz_b).
- gönderim/gereken: [türetilmiş] (oku.py R1 tanımı).
- tuzak: oku.py R1 tanımı (0 < çıkış ≤ 10 s ise tuzaksız).
- Dışlama yalnız R1 medyanı içindir; aşağıdaki okuma ikisini de kullanır.

Okuma:
- **Hangi koşular** [ölçüldü]: K2 rep2, rep6, rep7 (PreConverge çıkışı 56,8 / 42,2 / 66,0 s) ve K2−c rep3, rep5 (29,6 / 62,8 s). K0'da tuzak 0/10. K2 ↔ K0 farkı N = 10'da ayrılamaz: p = 0,105 [türetilmiş].
- **Gerçek kullanıcı ne yaşar** [ölçüldü]:
  - Tuzaklı K2 koşularında [3, 28) s'de gönderim, gerekenin 0,02–0,03'ü; tel neredeyse boş.
  - Mesajların 10 s içinde %1–2'si, 30 s içinde %1–3'ü, 60 s içinde %2–43'ü teslim. Göndericide bekleme p95 59–69 s.
  - PreConverge'den çıkınca birikmiş kuyruk boşalıyor: koşu sonunda (88 s) alıcıda 93–100/100.
  - Kullanıcı yaklaşık bir dakika hiçbir mesajın gitmediğini, sonra toplu teslimi görür.
  - K2−c rep5'te koşu sonunda 64/100. K2−c rep3'te çıkış 29,6 s'de; 60 s içinde %100.
- **Medyanlar** [ölçüldü/türetilmiş]:
  - K2 sw95 medyanı bütün koşularda 6,28 s, tuzaksızlarda 3,84 s (n = 7, 9. sıra yok). Teslim p95 13,58 ↔ 9,97 s. 60 s içinde teslimin en kötüsü 0,02 ↔ 1,00.
  - K0'da tuzak olmadığı için bütün = tuzaksız: sw95 12,71 s, 9. sıra 34,89 s; 60 s içinde en kötü 0,90.
  - EK-A §2.4'teki "34,89 → 63,75 s" K0'ın 9. sırasından K2'nin 9. sırasına geçiştir. K2'nin 9. sırası tuzaklı rep6'dır.
- **Tuzaksız ≠ sorunsuz** [ölçüldü]: K2−c rep10'da PreConverge çıkışı 5,6 s, R1'de tuzaksız sayıldı. Koşu sonunda 49/100; 60 s içinde %35; son BtlBw 967 B/s, ProbeUp 2 giriş. EKA-3'teki hız kilidine benziyor [çıkarım]. K0 rep3 ve rep10'da son BtlBw 1 459 ve 1 245 B/s; 60 s içinde %95 ve %90.
- Gösterim ağacında (E2a açık) manşet ölçülmedi. Manifest R1 metni PC tuzağını E2a m6/m8'in kapattığını söyler; bu ölçümde E2a yok, tuzak oranı gösterim ağacında bilinmiyor.

## 5. EKA-5 — CC-1 girdileri

#### b_r25_d300_l20_p65 — koşu başına durum (N = 10 + 10)

| kol | rep | PreConverge çıkışı (s; 0 = çıkılmadı) | son durum | geçiş | RecoverArq girişi | ProbeUp girişi | açıklanmayan geçiş | teslim@60 | gönderim (B) |
|---|---|---|---|---|---|---|---|---|---|
| K0 | 1 | 78.8 | Startup | 3 | 1 | 0 | 0 | 4 | 2554 |
| K0 | 2 | 46.4 | Startup | 5 | 2 | 0 | 0 | 6 | 2083 |
| K0 | 3 | 44.1 | Idle | 4 | 1 | 0 | 0 | 6 | 1856 |
| K0 | 4 | 62.5 | Startup | 1 | 0 | 0 | 0 | 6 | 2040 |
| K0 | 5 | 0.0 | PreConverge | 0 | 0 | 0 | 0 | 6 | 3479 |
| K0 | 6 | 46.5 | Startup | 1 | 0 | 0 | 0 | 6 | 1791 |
| K0 | 7 | 87.5 | Startup | 1 | 0 | 0 | 0 | 6 | 2397 |
| K0 | 8 | 46.7 | Startup | 1 | 0 | 0 | 0 | 6 | 1791 |
| K0 | 9 | 60.1 | Startup | 3 | 1 | 0 | 0 | 6 | 2067 |
| K0 | 10 | 96.2 | Startup | 3 | 1 | 0 | 0 | 6 | 3084 |
| K2 | 1 | 87.0 | Startup | 1 | 0 | 0 | 0 | 5 | 2727 |
| K2 | 2 | 0.0 | PreConverge | 0 | 0 | 0 | 0 | 5 | 3490 |
| K2 | 3 | 54.8 | Startup | 1 | 0 | 0 | 0 | 6 | 1975 |
| K2 | 4 | 90.5 | Startup | 3 | 1 | 0 | 0 | 6 | 2743 |
| K2 | 5 | 60.8 | Startup | 3 | 1 | 0 | 0 | 6 | 2121 |
| K2 | 6 | 55.0 | Startup | 1 | 0 | 0 | 0 | 6 | 2132 |
| K2 | 7 | 46.7 | Startup | 1 | 0 | 0 | 0 | 6 | 1856 |
| K2 | 8 | 68.8 | Startup | 5 | 2 | 0 | 0 | 6 | 2126 |
| K2 | 9 | 46.9 | Startup | 3 | 1 | 0 | 0 | 6 | 1883 |
| K2 | 10 | 70.6 | Idle | 2 | 0 | 0 | 0 | 6 | 2013 |

#### ProbeUp girişi — hücre × kol (koşu sayısı: ProbeUp'a hiç girmeyen / n; medyan)

| hücre | kol | ProbeUp = 0 olan koşu | medyan | maks |
|---|---|---|---|---|
| b_r12k_d1000_l30_p1000 | K0 | 9/10 | 0 | 1 |
| b_r12k_d1000_l30_p1000 | K2 | 10/10 | 0 | 0 |
| b_r12k_d1000_l30_p1000 | K2−c | 4/10 | 1 | 3 |
| b_r12k_d1000_l0_p1000 | K0 | 0/10 | 1 | 1 |
| b_r12k_d1000_l0_p1000 | K2 | 10/10 | 0 | 0 |
| b_r25_d300_l20_p65 | K0 | 10/10 | 0 | 0 |
| b_r25_d300_l20_p65 | K2 | 10/10 | 0 | 0 |
| a_r12k_d150_l20_foto150k | K0 | 0/10 | 4 | 10 |
| a_r12k_d150_l20_foto150k | K0r | 0/10 | 4 | 5 |
| a_r12k_d150_l20_foto150k | K1 | 0/10 | 1 | 2 |
| a_r12k_d150_l20_foto150k | K2 | 0/10 | 2 | 3 |
| a_r12k_d150_l20_foto150k | K2−c | 0/10 | 2 | 2 |
| a_r12k_d150_l20_foto150k | K2−e | 0/10 | 3 | 5 |
| a_r12k_d300_l30_dosyasiz | K0 | 0/10 | 4 | 5 |
| a_r12k_d300_l30_dosyasiz | K2 | 3/10 | 1 | 3 |
| a_r12k_d300_l30_dosyasiz | K2−c | 1/10 | 1 | 4 |

Sütun etiketleri:
- durum, geçiş, RecoverArq, ProbeUp, teslim, gönderim: [ölçüldü] (KP-ACTIVATION, KP-CLIENT).
- "açıklanmayan geçiş": [türetilmiş] (betikte `resid`). Geçiş sayacından şunlar düşülür: PreConverge → Startup (1); RecoverArq giriş ve çıkışı (2 × giriş; son durum RecoverArq ise 1 eksik); sondaki Idle (1).

**CC-1 girdisi olarak kayıt:**
1. **r25'te (d) devreye girmiyor** [ölçüldü/türetilmiş]:
   - 20/20 koşuda ProbeUp girişi 0.
   - PreConverge çıkışı 44,1–96,2 s; 2 koşuda hiç çıkılmadı (K0 rep5, K2 rep2).
   - Son durum: Startup 16, Idle 2, PreConverge 2.
   - Açıklanmayan geçiş 0/20: Drain, Steady, ProbeUp ya da DrainRtprop'a hiç geçilmedi.
   - (d) yalnız Startup plato kararını değiştirir (sender.rs `startup_plateau`, :4252) [kod]. Bu hücrede plato çıkışı hiç olmadığı için (d)'nin değiştirebileceği karar oluşmuyor; R3-r25 (d) için boş geçer.
   - Her koşunun en az ilk 44 s'si PreConverge'de geçti.
   - CC-1 I.1 (3) için anlamı: bu rejimde ne (d) ne prob tabanlı p̂ girdi alıyor [çıkarım].
2. **Manşette ProbeUp 0** [ölçüldü]:
   - K2: 10/10 koşuda ProbeUp girişi 0. K0: 9/10'da 0 (en çok 1). K2−c: 4/10'da 0 (en çok 3).
   - d1000 l0'da K0'ın her koşusu 1 kez girdi; K2'nin hiçbir koşusu girmedi.
   - (c) içeren K2'nin iki d1000 hücresinde ProbeUp hiç yaşanmıyor. I.1 (1)'in "p̂_taban yalnız güçlü ve başarılı probdan sonra öğrenilir" kuralı bu hücrelerde hiç tetiklenmez [çıkarım].
3. **Ek CC-1 girdisi: prob var, BtlBw yükselmiyor** [ölçüldü + türetilmiş]:
   - Analog K0 rep3: 10 ProbeUp girişi, BtlBw en çok 554 B/s.
   - Analog K0r rep4/6/8: 5'er giriş, BtlBw ≤ 392 B/s.
   - Manşet K2−c rep10: 2 giriş, son BtlBw 967 B/s.
   - Prob 1,25 × BtlBw ile koşarken teslim p = 0,2'de 1,00 ×, p = 0,3'te 0,875 × BtlBw; başarı eşiği 1,025 ×.
   - v4 I.1 (1)'in "kısır döngü" aritmetiğinin ölçülmüş örnekleri: BtlBw düşük bir değere bir kez oturunca prob onu kaldıramıyor.

#### Kesin olasılıklar (tek yönlü hipergeometrik; betimsel)

- Analog kilit deseni (bob yalnız ısınma metinleri, dosya yok): K0+K0r 4/20, E1 kolları 0/40; olayların hepsinin K0+K0r'ye düşme olasılığı p = 0.0099.
- Aynı desen K0r 3/10 ↔ K0 1/10: p = 0.291.
- Manşet PreConverge tuzağı K2 3/10 ↔ K0 0/10: p = 0.105.
- Manşet PreConverge tuzağı K2−c 2/10 ↔ K0 0/10: p = 0.237.

## 6. Açık kalanlar

- EKA-1 (orkestratörün gece koşusu).
- K1 dosyasız hücrede ve manşette koşmadı. Gösterim seti için (iii) ve R1 değerleri K2'nindir (manifest §2). K1'in kendi değerleri ölçülmedi.
- Kilit deseninin mekanizması ((d) adayı, §3) doğrulanmadı; Startup plato sayacı aynada yok. Doğrulama için CC-0 kablolaması (`diag_probe`/`diag_rate`) ya da aynaya tur sayacı gerekir (Aralık; v4 Bölüm V).
- A hücrelerinde kopya baytı ayrı sayaç değil. Datagram kapanışı 0 olduğu için kalan bayt = kopya baytı [türetilmiş].
- Startup dönemi kaybı [0, 20) s ve `diag_probe` sayaçları ölçülmedi (OKUMA.md ile aynı).

## 7. Yeniden üretim

```
python3 -I reports/yol3/2026-11-09/g2/eka_eksikler.py                    # ham/g2/*.tar.zst (Python ≥ 3.14)
python3 -I reports/yol3/2026-11-09/g2/eka_eksikler.py --kok <açılmış koşu kökü>
python3 -I reports/yol3/2026-11-09/g2/eka_eksikler.py --json <çıktı.json>  # koşu satırları
```

#### Ölçüm denetimleri

- İlk emisyonda `txb` − birim uzunluğu (A hücreleri, bütün koşular): [33] B (beklenen 33).
- Datagram kapanışı `txd` − (ilk iletilen birim + rtx + kopya + parite), bütün A koşuları: [0] (0 = başka sınıf datagram yok: CS-probe/PARAM tele çıkmadı).

İki girdi yolu (arşiv ve açılmış koşu kökü) bayt bayt aynı çıktıyı verdi.
