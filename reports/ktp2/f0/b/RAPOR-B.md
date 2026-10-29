# KTP2-F0 / Ajan B — Düşük bant ve yüksek RTT: rapor taslağı

**Tur 1 — koşu yok.** Bu taslakta ölçüm sonucu yoktur. Sayıların tamamı [kod] (kaynak okuması, dosya:satır) ya da [çıkarım] (ölçülmemiş akıl yürütme; `scripts/ktp2/f0/b/fizik.py` ile hesaplandı). §6'daki tek tablo 05b arşivinden [betimsel, sonradan] türetildi. Koşu sonrası §9 doldurulur; §5'teki tahminler o zaman sapma dekompozisyonuyla kıyaslanır.

Dayanak: `ISEMRI-KTP2-F0-2026-10-29` §3 "Ajan B"; `scripts/ktp2/f0/KATALOG.md`; `ORTAK-KURALLAR.md`.
Kollar: `taban` = main `a858f3fc` (+ ölçüm kodu), `aday` = 05b `49864510` (+ aynı ölçüm kodu), `quinn` / `tcp` = taban ikilisinin rakip kolları.
Dal: `ktp2/f0-b`. Bu turda ürün kodu değişmedi.

Kod farkı denetimi [kod]: B'nin dayandığı dosyalardan `receiver.rs`, `ack.rs`, `tail_recovery.rs`, `pacer.rs`, `rtt.rs` (adaptive-transport) ile `seam.rs`, `wire.rs` (client-core) iki kolda bayt bayt aynıdır. Kollar `sender.rs`, `arq.rs`, `loss.rs` ve `client-core/src/runtime.rs`'de ayrışır (`git diff --stat a858f3fc 49864510 -- crates/`). Satır numaraları taban içindir; adayda farklıysa ayrıca yazılır.

---

## 0. Bu turun özeti

1. **Izgara:** 49 hücre. Bütçe içi P1–P4 için **≈ 92,9 koşu-saati** beklenir (en kötü 141,8). Dört kol ve `-j3` ile ≈ 31 saat duvar süresi tutar. Bütçe dışı ve budanan hücreler P5–P6'dadır. Ayrıntı §1 ve Ek B'de.
2. **Sunulan yük:** ileri hat kapasitesinin %30'u. Ölçü, tek iletimli Kama veri paketinin L2 dahil boyudur. Mesaj boyları açık: 65 B ve 1000 B. "1 KB" için 1000 B seçildi, çünkü 1024 B ürün v1 profilinde tek birim olarak gönderilemez (B-K6).
3. **Koddan en güçlü dört bulgu** (§4; hepsi [kod] + [çıkarım], ölçümle doğrulanacak):
   - **B-K1:** Kuyruk probu PTO'su 10 s'de kırpılıyor. 25 B/s'de RTT 8,9–12,8 s olduğundan, d1000'de her mesaj için gereksiz bir tam kopya bekleniyor.
   - **B-K2:** Kuruluş probu jetonsuz ve 0,5 s'den başlıyor. 25 B/s'de ilk RTT içinde ilk mesajın 4 kopyasını, artı 1 geometrik kopyasını gönderiyor. Bu ≈ 27 s ileri hat süresi eder.
   - **B-K4:** Alıcı ACK'i ikinci veriye kadar tutuyor (`ACK_FREQ_N = 2`, tutma ≤ 8,39 s). Gönderenin PTO'su ise 25 ms tutma varsayıyor. Bu çelişki 1 kB/s – 1 KB hücrelerinde mesajların %50–100'ü için gereksiz kuyruk probu üretir.
   - **B-K3:** Yeniden gönderim yeni PN alıyor, bu yüzden SACK delikleri kalıcı. 16 kayıptan sonra her ACK 137 B olur (yalın ACK 73 B). 25 B/s'de bu, ACK başına +2,56 s demektir.
4. **K-3 notu (Ajan E'ye):** Taslağın K-3 alt sınırı (96 B → 3,8 s) L2/L3/L4'ün 42 B'sini saymıyor. Bench'te 65 B'lik mesaj telde 134 B'dir; spec zarfıyla 140 B. 25 B/s'de tek yön alt sınır 5,66 s'dir (d300).
   %20 kayıpta ideal ARQ'nun p95'i 14,5 s'dir; bu, K-3 hedefi olan 12 s'nin üstündedir. Hedefi yalnız eşzamanlı 2 kopya karşılar (11,0 s) [çıkarım, fizik.py].
5. **Ölçüm araçları:**
   - Uç yakalama (`cap_b.py` + `cap_wrap.sh`): çerçeve türü, birim kimliği ve PN kaydedilir. Yeniden gönderimin faydalı/gereksiz ayrımı ve gerçek RTT ürün koduna dokunmadan ölçülür.
   - `analiz_b.py`: sentetik denetimden geçti (22 denetim, `sentetik_test.py`).
   - netem'in sıra koruyan jitter'ı ortalama tek yön gecikmesini profil değerinin üstüne çıkarır. Model `fifo_owd`'dir; srtt ayrıca bu değerle de kıyaslanır (§2.2).

---

## 1. Izgara

### 1.1 Eksenler ve geometri

Profil dizeleri KATALOG §2'den aynen alındı. Örnek: `delay 1000ms 200ms distribution normal loss 20% rate 8kbit`. Kayıp iki yönde simetriktir; `limit` varsayılan değerinde bırakıldı (1000).

| hız | yük | aralık I | mesaj M | son tarih D | veri paketi (tel) | ρ ileri (tek iletim) | gerekçe |
|---|---|---|---|---|---|---|---|
| r25 (25 B/s) | 65 B | 18 000 ms | 6 | 60 s | 134 B (5,36 s) | 0,298 | K-3 profili; bütçe M'yi 6'da tutar |
| r25 | 1000 B | 143 000 ms | 3 | 60 s | 1069 B (42,8 s) | 0,299 | yalnız P6, betimsel (bütçe dışı) |
| r1k (1 kB/s) | 65 B | 450 ms | 100 | 30 s | 134 B (0,134 s) | 0,298 | D ≥ 13 × RTT_fiz |
| r1k | 1000 B | 3 600 ms | 20 | 60 s | 1069 B (1,069 s) | 0,297 | K-2: 1 KB, 60 s |
| r12k (12,5 kB/s) | 65 B | 36 ms | 600 | 30 s | 134 B (10,7 ms) | 0,298 | D ≥ 12 × RTT_fifo |
| r12k | 1000 B | 285 ms | 100 | 60 s | 1069 B (85,5 ms) | 0,300 | K-2: 1 KB, 60 s |

- Aralık formülü: I = veri paketi / (0,30 × hız), yukarı yuvarlanır.
- Veri paketinin tel boyu = 42 (Eth 14 + IPv4 20 + UDP 8) + 16 (bench zarfı) + 11 (DATA başlığı) + yük.
- %30 kayıpta ideal ARQ'nun ileri yükü 0,43'tür. Kalan pay, ACK dışı protokol trafiğine (kopya, prob, FEC) bırakılan alandır. Bu trafik ölçülen sütundur.
- Stres hücrelerinde ideal ARQ yükü: l50'de ρ = 0,60, l80'de **1,49**. l80'de ideal ARQ bile hattı aşar; teslim oranı kapasiteyle sınırlıdır [çıkarım]. Geometri birincil hücrelerle karşılaştırılabilsin diye değiştirilmedi.

### 1.2 Öncelik, budama, bütçe

| öncelik | hücreler | N | beklenen koşu-saati (kümülatif) | en kötü (kümülatif) |
|---|---|---|---|---|
| **P1** | r1k/r12k × d300/d1000 × l0/l20/l30 × 65 B / 1000 B (24 hücre) | 30 | 50,3 (50,3) | 85,2 (85,2) |
| **P2** | r25 × d300/d1000 × l0/l20 × yalnız 65 B (4 hücre) | 30 | 17,3 (67,7) | 18,8 (104,0) |
| **P3** | r1k/r12k × d300/d1000 × l10 × 65 B / 1000 B (8 hücre) | 30 | 17,1 (84,8) | 28,4 (132,4) |
| **P4** | stres: r1k/r12k × d300 × l50/l80 × 65 B / 1000 B (8 hücre) | 10 | 8,1 (92,9) | 9,5 (141,8) |
| P5 (budandı) | r25 × d300/d1000 × l10/l30 × 65 B (4 hücre) | 30 | 18,4 (111,3) | 18,8 (160,7) |
| P6 (budandı) | r25 × d300 × l0 × 1000 B (1 hücre) | 10 | 3,5 (114,8) | 3,5 (164,2) |

Budama açıktır:
- r25'te yalnız 65 B ve l0/l20 hükümdedir. l10/l30 P5'tedir.
- r25 – 1 KB P6'dadır: tek mesajın serileştirmesi 42,8 s; ρ = 0,3'te aralık 143 s; 10 tekrar ≈ 3,5 saat.
- Stres hücreleri yalnız d300'dedir.

Koşu süresi modeli (`fizik.run_time`) [çıkarım]:
- **Kama:** sunum + kuyruk süresi. Kuyruk, kayıpsızda RTT + I; kayıpta min(D, 4·RTT + I); streste D. İstemci, bütün teyitler gelince ya da son sunum + D'de biter (`kama_product.rs:757-781`).
- **quinn:** el sıkışma + sunum + kuyruk. El sıkışma D ile sınırlıdır (`quic.rs:256`). r25'te başarısız el sıkışma sonrası mesaj sunulmaz.
- **TCP:** her koşuda **sabit** min(D, 60 s) bekler (`tcp.rs:164`). Bu bekleme P1 bütçesinin ≈ %18'idir (9,0 / 50,3 sa); TCP kolu toplamda P1'in ≈ %37'si. Bkz. İSTEK-D1.

Hücre zaman aşımı `BUDGET_S`, en kötü kol süresinin 30 s üstüdür.

### 1.3 Duman seti (önce bu)

`cells/b/DUMAN.txt`, `F0_REPS=1`:

| hücre | neden |
|---|---|
| b_r12k_d300_l30_p65 | 600 mesaj, en yoğun yakalama |
| b_r12k_d1000_l10_p1000 | 1 KB, yüksek jitter |
| b_r1k_d300_l0_p65 | temel kayıpsız durum |
| b_r1k_d1000_l20_p1000 | B-K4 (ACK tutma ↔ PTO) |
| b_r25_d1000_l20_p65 | B-K1/B-K2, quinn el sıkışma zaman aşımı |
| b_r12k_d300_l80_p65 | stres |

24 koşu sürer: beklenen ≈ 30 dk, en kötü ≈ 42 dk. Dumanda denetlenecekler:
- `capb_*.csv.gz` iki uçta da var, `capb_*.FAIL` yok;
- `analiz_b.py` analiz hatası 0;
- dört kolda `cmd.txt` doğru;
- quinn ve tcp'de `F0_RAKIP_TANIM=gecici-B` görünüyor (D'nin tanımı gelene kadar).

### 1.4 Kollar ve yakalama

- **taban, aday:** `scripts/ktp2/f0/arms_kama.sh` → `kama_arms`. İki kolda komut bayt bayt aynıdır, fark yalnız ikilidir:
  - `serve --bind @SRV@ --transport kama-product --payload P`
  - `client … --msgs M --payload P --submit-interval-ms I --max-deadline-s D --seed @REP@ --repeat @REP@ --fwd-impair '<FWD>' --rev-impair '<REV>' --manifest @D@/manifest.json --out @D@/out.jsonl`

  Ürün v1 profili bench varsayılanlarıdır (fec 0x05 adaptive, tail validated_v1, IW 2400 B).
- **quinn, tcp (GEÇİCİ):** `b/hucre_b.sh` → `b_rakip_gecici`.
  - quinn: `--transport quinn --cc bbr`. tcp: `--transport tcp`.
  - `cmd.txt`'de `env F0_RAKIP_TANIM=gecici-B` ile işaretlidir.
  - `scripts/ktp2/f0/arms_rakip.sh` gelir ve `rakip_arms` işlevini sağlarsa hücreler kendiliğinden onu kullanır (İSTEK-D3).
- **Yakalama:** `b/cap_wrap.sh` her kolun SRV/CLI komutunu sarar. Komut anlamı, çıkış kodu ve SIGTERM iletimi korunur; bu, sahte komutla denendi.
  - Ad alanı içindeki uç arayüzler: istemci tarafı `va0`, sunucu tarafı `vb0`.
  - netem yalnız MID'dedir. Uç veth kayıpsızdır: `vb0`'a giren = darboğazdan geçen ileri trafik; `va0`'a giren = darboğazdan geçen dönüş trafiği.
  - Bench mühür katmanı (IdentitySeam) şifrelemez. Çerçeve düz metindir; PN etiketin ilk 8 baytındadır (`seam.rs:105-108, 166`).
  - KATALOG'un `CAPTURE=1` mekanizması (`pkt_capture.py`) yalnız ip_len/port yazdığı için kullanılmadı (`CAPTURE=0`).

---

## 2. Fiziksel model

### 2.1 Tel boyları [kod]

| paket | tel boyu (L2 dahil) | kaynak |
|---|---|---|
| Kama DATA | 42 + 16 + 11 + yük → 134 B (65 B), 1069 B (1000 B) | `seam.rs:105` (16 B etiket), `wire.rs:17` (11 B) |
| Kama ACK V2 | 42 + 16 + 15 + 4 × aralık → 73 B … 137 B (16 aralık) | `ack.rs:36`, `ack.rs:204-232` |
| Kama FEC onarımı (0x05) | 42 + 16 + 12 + 1024 = 1094 B (65 B verinin 8,2 katı) | `profile.rs:85-90`, `wire.rs:105, 126` |
| Kama PARAM / PARAM_ACK | 93 / 91 B | `wire.rs` LEN_PARAM 35, LEN_PARAM_ACK 33 |
| Spec zarfı | bench + 6 B/paket (22 B: hub_handle 4 + seq 2 + etiket 16) | `specs/transport-layer-tdd.md:149` |
| TCP veri | 14 + 20 + 32 (TS seçeneği) + 12 (çerçeve) + yük → 143 B (65 B) | `tcp.rs:41`; Linux varsayılanı |
| TCP uygulama ACK'i / yalın ACK | 74 B / 66 B | `tcp.rs:43` |
| QUIC Initial | ≥ 1200 B UDP yükü → ≥ 1242 B | RFC 9000 §14.1; `quinn-proto-0.11.18/src/lib.rs:330` |

### 2.2 Gerçek RTT'nin iki tanımı

- **RTT_fiz** (iş emrinin tanımı): 2 × eklenmiş gecikme + ser(veri) + ser(ACK). 0 aralıklı ve 16 aralıklı ACK için ayrı verilir.
- **RTT_fifo** [çıkarım, model]: netem `rate` kolunda paket, kuyruktaki son paketten önce çıkamaz: time_to_send = max(son, şimdi + delay) + ser (KATALOG §2: "jitter yeniden sıralamaz").
  - Bu yüzden σ, paket aralığına yaklaşınca ya da onu aşınca ortalama tek yön gecikmesi profil değerinin **üstüne** çıkar.
  - `fizik.fifo_owd` bunu sabit tohumlu Monte Carlo ile hesaplar: ileri aralık I, dönüş aralığı ≈ 2I (ACK_FREQ_N = 2).
  - Örnek: r12k/d1000/65 B'de (I = 36 ms) tek yön 1,000 → 1,221 s; RTT 2,017 → 2,371 s.
  - 05b p4_jitter'da (200 ± 100 ms, 50 ms aralık) RTT_fiz 431,8 ms, RTT_fifo 537,4 ms. İki kolun srtt medyanı 580 ve 568 ms (§6). Profil RTT'sine göre ≈ 1,33 oranının büyük kısmını bu etki açıklıyor [betimsel].

srtt üç referansla kıyaslanır: RTT_fiz, RTT_fifo ve **ölçülen ağ RTT'si**. Ağ RTT'si yakalamadan çıkar: orijinal paketin ileri yolculuğu + onu ilk kapsayan ACK'in dönüş yolculuğu. Alıcı tutması hariçtir.

### 2.3 Hücre başına alt sınırlar

Tam tablo Ek A'dadır (`python3 -I scripts/ktp2/f0/b/fizik.py tablo`). İdeal ARQ varsayımları:
- kayıp bağımsız q;
- yeniden deneme aralığı = RTT_fiz;
- ACK kaybı yok;
- tek yön varış, k. denemede tek yön alt sınır + (k−1) × RTT_fiz.

Seçilmiş değerler [çıkarım]:

| hücre | tek yön alt sınır | ideal ARQ p95 tek yön | eşzamanlı 2 kopya p95 | faydalı yük payı üst sınırı (ileri, tek iletim) |
|---|---|---|---|---|
| r25 d300 l20 65 B | 5,66 s | 14,54 s | 11,02 s | 0,485 |
| r25 d1000 l20 65 B | 6,36 s | 16,64 s | 11,72 s | 0,485 |
| r1k d300 l20 1000 B | 1,37 s | 3,11 s | 2,44 s | 0,935 |
| r1k d1000 l30 65 B | 1,13 s | 5,55 s | – | 0,485 |
| r12k d1000 l30 1000 B | 1,09 s | 5,27 s | – | 0,935 |

65 B'de faydalı yük payının fiziksel üst sınırı 65/134 = %48,5'tir. Bu sınıra dönüş yönündeki ACK baytları dahil değildir.

---

## 3. Ölçüm tanımları (koşu başına, kol başına)

| ölçü | tanım | kaynak |
|---|---|---|
| teslim oranı (D içinde) | gönderen teyidi ≤ D olan mesaj / M; ayrıca 5/10/20/30/60 s | out.jsonl [olay] |
| alıcıda varış oranı | birimin B'ye ilk varışı ≤ sunum + D | yakalama |
| gecikme p50/p95/p99 | (a) gönderen teyidi (bench'in tanımı); (b) tek yön: sunum → B'ye ilk varış. TCP'de sıralı teslim, HOL dahil. quinn'de (b) ölçülemez (şifreli; İSTEK-D2) | olay, yakalama |
| hızlandırıcı | motorun kapanış pacer hızı ve btlbw değeri / hattın veri payı. Yakalamada, 10 × ser(veri) (≥ 1 s) pencerelerde gönderim hızı / hat: p50, p95, maks ve hattı aşan pencere oranı | KP-ACTIVATION, yakalama |
| kuyruk yaşı | gönderende ilk iletime kadar bekleme (sunum → A'dan ilk çıkış; p50/p95/maks); darboğaz kuyruk fazlası (yolculuk − ser − RTT_fifo tek yön); netem son backlog | yakalama, qdisc |
| kapasite reddi | (a) istemci egress reddi; (b) sunucu delivery_dropped, frag_chunk_dropped, msg_asm_evicted, fec_blocks_evicted, fec_pre_param_dropped, dropped_new_peers, egress reddi. Toplam `cap_reject_total`. Ayrıca, toplama girmeden: (c) parite jeton reddi (`parity_denied`); (d) zamanlayıcı veri/yeniden gönderim jeton reddi (yalnız `diag-sched-telemetry` derlemesinde; varsayılan derlemede n/a); (e) uygulama sunumunun QueueFull reddi (sayaç yok, İSTEK-D4) | c.err/s.err |
| yeniden gönderim | aynı birimin ilk iletim dışındaki her DATA iletimi: ARQ, kuyruk probu, §11 kopyası, kuruluş probu ve geometrik kopya. Tel bunları birbirinden ayırmaz; ayrım için istemci sayaçlarına bakılır (retx/dup baytı, tail_probes, establish_tx) | yakalama + sayaç |
| — faydalı | B'ye **ilk** varan kopya, orijinal değilse | yakalama |
| — gereksiz | birim B'de zaten varken gelen kopya. Alt sınıflar: alıcıda olan birime gönderilen; teyit geldikten sonra gönderilen | yakalama |
| — kayıp | netem'de düşen kopya. Hat kapasitesi harcamaz: netem kaybı serileştirmeden önce uygulanır | yakalama |
| goodput | ileri faydalı yük baytı / pencere (ilk sunum → son varış) | yakalama |
| hat payları | ileri ve dönüş ayrı, hattan geçen düzlemde. Sütunlar: L2/L3/L4 (42 B/paket), zarf (16 B/paket), çerçeve başlığı, faydalı yük (orijinal / kopya), gereksiz yük (orijinal / kopya), FEC onarımı, prob, kontrol, ACK, ayrıca ACK paketlerinin bütün baytı. Hat kapasitesine (hız × pencere) ve hattan geçen toplama oranı verilir. Deneme düzlemi (A'dan çıkan) ayrıca verilir | yakalama, qdisc ile çapraz denetim |
| fiziksel / zamanlayıcı ayrımı | §3.1 | yakalama |

### 3.1 Fiziksel kapasite yetersizliğini zamanlayıcı kaynaklı gecikmeden ayırma

Mesaj i için **ideal sınır**: tek kopyalı, iş koruyan bir FIFO gönderici, mesajı hat boşalır boşalmaz gönderir. Hesap:
- ideal_başlangıç_i = max(sunum_i, ideal_bitiş_{i−1});
- ideal sınır = ideal_bitiş_i − sunum_i + RTT_fifo tek yön + (önceki kayıp deneme sayısı × RTT).

Bu sınırı aşan fazla üç bileşene ayrılır:
- gönderende bekleme fazlası;
- kurtarma beklemesi fazlası;
- darboğaz kuyruk fazlası.

**Geç kalma aralığı** = [ideal serileştirme başlangıcı, gerçek serileştirme başlangıcı]. Bu aralıkta hat ya **meşguldür** ya **boştur**.
- Meşgulse, serileştirilen paketin sınıfı payı belirler: faydalı yük = fiziksel kapasite; gereksiz kopya, FEC ya da kontrol = protokol trafiği.
- Boşsa gecikme zamanlayıcı ya da hızlandırıcı kaynaklıdır: gönderen tutuyordu ya da geç gönderilen kopya yolda ilerliyordu.

Çıktı: `late_idle_frac`, `late_busy_useful_frac`, `late_busy_redund_frac`, `late_busy_fec_frac`, `late_busy_ctrl_frac`. Ayrıştırma yaklaşıktır: jitter gürültüsü ±σ'dır ve gönderilmemiş kopyanın kuyruğu sayılmaz. Bu nedenle [betimsel ayrıştırma] olarak etiketlenir.

---

## 4. Koddan bulgular

Her bulgu dört parçalıdır. Ölçüm sütunu bu turda **beklenen** ölçümdür; koşu sonrası ölçülen değer yazılır.

### B-K1 — Kuyruk probu PTO'su 10 s'de kırpılıyor, RTT ise 25 B/s'de 8,9–12,8 s

- **Ölçüm (beklenen):**
  - r25/d1000: `tail_probes` ≈ mesaj sayısı (6/koşu), iki kolda. `cp_redund_arr` ≥ 6/koşu. `wfa_offered_load` ≈ 0,6: ρ'nun iki katı, kuruluş kopyaları hariç.
  - r25/d300 l20: ACK'e aralık eklendikçe (B-K3) RTT 8,9 s'den 11,4 s'ye doğru büyür; 10 s'yi aştığı mesajlarda aynı prob beklenir.
- **Dosya/işlev:**
  - `adaptive-transport/src/tail_recovery.rs:72-82` `validated_v1`: `max_pto_ns` 10 s, `max_ack_delay_ns` 25 ms, `max_tail_probes` 3.
  - `:155-165` `base_pto_ns`: `raw.clamp(min_pto, max_pto)`.
  - `sender.rs:5890-5938` `emit_tail_probe`. Hedefi tombstone'a çeker ve "superseded" işaretler (`:5926`).
  - İki kolda aynı dosya; adayda `emit_tail_probe` `:7097`.
- **Kök neden:** taban PTO = srtt + 4·rttvar + 25 ms, üstten 10 s'de kırpılır. 25 B/s'de tek mesajın RTT'si 10 s'yi aşar:
  - d1000'de 0 aralıklı ACK ile 10,28 s;
  - d300'de 16 aralıklı ACK ile 11,44 s.

  Bu durumda prob, ACK gelmeden **her mesajda** ateşlenir. Prob tam veri kopyasıdır (25 B/s'de 5,36 s hat süresi). Ayrıca orijinali "superseded" yaptığından tabanda o mesajdan RTT örneği alınmaz (B-K11).
- **En dar düzeltme:** `base_pto_ns`'te üst kıskacı srtt'nin altına inmeyecek biçimde değiştirmek. Örnek: `raw.clamp(min_pto, max(max_pto, 2 × srtt))` (tek satır).
  - Geri dönüş (backoff) tavanı 30 s kalır.
  - Tel biçimi değişmez.

### B-K2 — Kuruluş probu (PreConverge) jetonsuz; düşük bantta ilk mesaj çoğaltılıyor

- **Ölçüm (beklenen):**
  - r25: `establish_tx` = 4 (d300 ve d1000), artı 1 geometrik kopya. İlk mesaj için 6 paket × 134 B = 804 B, yani ≈ 32 s ileri hat süresi.
  - İkinci mesaj (t = 18 s) bu kuyruğun arkasında kalır. Tek yön gecikmesi ≈ 19–20 s olur (alt sınır 5,66 s). `sender_wait` küçük, `queue_extra` büyük çıkar.
  - r1k / r12k: 1–2 kuruluş probu. Etki 1 KB'de 1–2 × 1069 B'dir.
- **Dosya/işlev:**
  - `sender.rs:1490-1491` `ESTABLISH_PROBE_INITIAL_NS` 0,5 s, `ESTABLISH_PROBE_CAP_NS` 8 s.
  - `:5995-6049` `drain_establish_probe` (jetonsuz, aralık ×2).
  - `:4306` çağrı.
  - `:6151-6158` §3.3-B geometrik kopya (ilk gecikme = 1,5 × (birim + 65)/31 s = 7,6 s, 65 B için).
  - `:1485` `PRE_CONVERGE_FLOOR_BW` 31 B/s. Bu değer r25 hattından (25 B/s) büyüktür.
  - Aday: `:1677-1678`, `:4796`, `:7213`, `:7606`.
- **Kök neden:** kuruluş probunun ilk vadesi ve aralığı hat hızından türemiyor; probe jetonsuz. Geometrik kopyanın kendisi kanaldan türetilmiş bir RTT tabanı kullanıyor (`rtt_floor`), prob kullanmıyor. 25 B/s'de ilk RTT ≈ 8,9–10,3 s içinde 0,5 / 1,5 / 3,5 / 7,5 s'de 4 kopya çıkar.
- **En dar düzeltme:** `drain_establish_probe`'ta ilk aralığı `max(ESTABLISH_PROBE_INITIAL_NS, rtt_floor)` yapmak. Burada `rtt_floor` = (birim + PC_SACK_WIRE)/PRE_CONVERGE_FLOOR_BW, geometrik kopyayla aynı türetim.
  - 65 B'de ilk vade 5,1 s olur; ilk RTT içinde 1 prob kalır.
  - Tel biçimi değişmez.

### B-K3 — SACK delikleri kalıcı: yeniden gönderim yeni PN alıyor, ACK en büyük boyda kalıyor

- **Ölçüm (beklenen):**
  - `ack_ranges_med` = 16: r1k/r12k 65 B'de l10+ ve r12k 1 KB'de l20+ (koşu başına delik ≥ 16).
  - r25'te ≈ delik sayısı (≤ 5).
  - Dönüş ACK paketi 137 B. 25 B/s'de 5,48 s; 1 kB/s'de 0,137 s.
- **Dosya/işlev:**
  - `runtime.rs:3580`: sunucu alıcısına `seq: pn` (her mühür yeni PN).
  - `receiver.rs:394-420` `record_seq`: alınanlar kümesi, delik doldurulmaz. 4096'lık pencere.
  - `ack.rs:92-149` `from_received`: en yüksekten aşağı en çok 16 aralık (`:36`, `:130`).
  - İki kolda aynı dosyalar.
- **Kök neden:** Kama'da yeniden gönderim aynı PN'i değil yeni PN'i taşır. Kaybolan PN'in deliği alıcıda hiç kapanmaz, ACK her seferinde en yeni 16 deliği yeniden anlatır. Gönderen o deliklerin kaderini çoktan bilir: kayıp ilan etmiş, yeniden göndermiştir. Bu bayt bilgi taşımaz.
  - 25 B/s'de ACK başına +2,56 s dönüş süresi; RTT'ye eklenir ve B-K1'i tetikler.
- **En dar düzeltme:** alıcı, aralıkları yalnız son W PN içinde yazsın. Örnek: `largest − 32`. Bu, `from_received`'a bir kesme koşuludur; kayıp tespiti yalnız paket ve zaman eşiklerinin içindeki PN'lere bakar.
  - Tel biçimi değişmez (`range_count` zaten değişkendir).
  - Altın vektörler codec'e aittir; alıcı davranış testleri gözden geçirilmelidir.

### B-K4 — Alıcının ACK tutması ile gönderenin PTO varsayımı çelişiyor

- **Ölçüm (beklenen):**
  - r1k/1 KB (I = 3,6 s), dört kayıp düzeyinde: `tail_probes` ≈ 0,5–1 × M.
  - `cp_redund_arr` ≈ 0,4–1,0 × M.
  - `ack_hold_p95_s` ≈ I (3,6 s).
  - `wf_useful_share` 0,935'ten 0,47–0,65'e iner.
  - Gönderen-teyit p50, RTT_fiz'in (1,74 / 3,14 s) üstünde çıkar.
  - Diğer hücrelerde etki küçük. ACK ilerlemesi PTO'dan sık gelir; prob muhafızı (`tail_recovery.rs:227`) korur.
- **Dosya/işlev:**
  - `receiver.rs:76` `ACK_FREQ_N = 2`.
  - `:268-324` `on_ack_timer`: tail muafiyeti = sessizlik ≥ 2 × son varış aralığı; cadence = clamp(2 × aralık, 2,097 s, 8,39 s) (`:93`, `:115`).
  - `:434-446` `tick_ack`.
  - Gönderende `tail_recovery.rs:79` `max_ack_delay_ns` 25 ms.
  - İki kolda aynı.
- **Kök neden:** seyrek trafikte tek gelen verinin ACK'i ikinci veriye kadar, yani I kadar, ya da cadence süresince bekler. Gönderenin PTO'su alıcı tutmasını 25 ms sayar. Örnek, r1k/1 KB/d300:
  - PTO ≈ 1,74 + 4·rttvar + 0,025 ≈ 2,2 s. Tutulan ACK'in gelişi ≈ 3,1–5,3 s.
  - Prob ateşlenir (1069 B = 1,07 s hat süresi).
  - Probun varışı alıcıdaki çifti tamamlar ve sonraki mesajın tek kalmasına yol açar. d300'de etki mesajların neredeyse tamamına yayılabilir, d1000'de ≈ yarısına [çıkarım].
- **En dar düzeltme (gönderen, tel değişmez):** PTO'ya sabit 25 ms yerine son ACK'lerde gözlenen en büyük `ack_delay` eklensin. Motor bu değeri zaten okuyor (`sender.rs:3188` `diag_ack_delay_max_us`, yalnız gözlem); düzeltme bunu pencereli hâle getirip `current_pto_ns`'e verir.

  Alternatif (alıcı): `since_last_ack > 0` iken tutma ≤ 25 ms olsun. Bu, düşük hızda mesaj başına ACK demektir: 25 B/s'de dönüş yükü artar. Bu yüzden gönderen tarafı daha dardır.

### B-K5 — FEC onarım sembolü E = 1024 sabit; kısa mesajda onarım veriden 8,2 kat pahalı

- **Ölçüm (beklenen):**
  - l0'da `parity` = 0.
  - l20/l30'da parite ateşlenirse, 65 B hücrelerinde `wf_fec_repair` ileri baytın büyük payını alır. Örnek: r1k/65 B'de 8'lik blok başına 2 onarım, +2188 B / 1072 B veri; ileri yük 0,3 → ≈ 0,9 [çıkarım]. Bu durumda `queue_extra` ve `late_busy_fec_frac` artar.
- **Dosya/işlev:**
  - `kama-fec-codec/src/profile.rs:85-90` (0x05: K8/R4/E1024).
  - `client-core/src/runtime.rs:234-245` (`FEC_PROFILE_ID`, `MAX_UNIT_BYTES` 1013).
  - `wire.rs:126` `encode_fec` (sembol = E). Ayrıca `runtime.rs` RS onarımı E boyunda.
- **Kök neden:** sembol boyu profil sabiti. Kaynak simgeler kısa gönderilir; alıcı sıfırla doldurur. Onarım simgesi ise E'nin tamamıdır. 65 B mesajda onarım paketi 1094 B, veri paketi 134 B.
- **En dar düzeltme:** ürün davranışında "onarım bayt maliyeti / blok kaynak baytı" eşiği. Örnek: E / ortalama kaynak boyu > 4 ise bu blok için parite yok. Bu, arbiter'da tek koşuldur. Kalıcı çözüm (blok başına sembol boyu) tel değişikliğidir, dar değildir.

  Ajan C ile ortak bulgu. C'nin hücrelerinde ölçülür; B yalnız düşük bant maliyetini raporlar.

### B-K6 — "1 KB" (1024 B) mesaj ürün v1 profilinde tek birim olarak gönderilemez

- **Ölçüm:** ölçülmez; ızgara 1000 B kullanır. Kayıt amacıyla beklenen davranış:
  - Ürün profili `max_message_bytes = None` ile 1024 B'yi `UnitTooLarge` ile reddeder.
  - Bench fragmenter'ı açar: 2 paket (1013 + 11 B), parça başına +11 B önek, kimlik `kama-custom`.
- **Dosya/işlev:**
  - `runtime.rs:245` (1013 B tavanı), `:266` (`max_message_bytes: None`).
  - `bench/src/kama_product.rs:156` (bench fragmenter'ı açar), `:256-263`, `:661` (kama-custom).
- **Kök neden:** birim tavanı FEC sembolüne bağlı: E − 11 önek.
- **En dar düzeltme:** ürün kararı. Uygulama katmanı mesajı ≤ 1013 B tutar ya da fragmenter varsayılan açılır (her parçaya +11 B). F1 planına girer; F0'da ölçüm yok.

### B-K7 — Bench zarfı 16 B, spec zarfı 22 B

- **Ölçüm:** analizde `s_data_spec22` sütunu, yani paket başına +6 B düzeltilmiş tel. 65 B'de veri paketi +%4,5, ACK +%8. 25 B/s'de veri başına +0,24 s.
- **Dosya/işlev:** `client-core/src/seam.rs:105` (IdentitySeam 16 B); `specs/transport-layer-tdd.md:149` (22 B).
- **Kök neden:** IdentitySeam yalnız AEAD etiketini taklit ediyor; hub_handle 4 + seq 2 yok.
- **En dar düzeltme:** ölçüm tarafında düzeltme sütunu yeterli. Bench'te +6 B taklit bütün şeritlerin telini değiştirir; bu turda yapılmaz.

### B-K8 — PreConverge pacer tabanı 25 B/s hattından büyük; ilk pencere 2400 B

- **Ölçüm:** bu ızgarada sınırlı (ρ = 0,3; PreConverge'de ≤ 1 mesaj). Patlama yükü olmadığı için IW etkisi ölçülemedi.
- **Dosya/işlev:** `sender.rs:1485` (31 B/s); `pacer.rs:9` (2 × 1200 B); `runtime.rs:248` (`INITIAL_WINDOW_BYTES`).
- **Kök neden:** taban hız sabit, hattan türetilmemiş. IW 2400 B, 25 B/s'de 96 s'lik kuyruktur.
- **En dar düzeltme:** ölçülmeden önerilmez. Not olarak kalır (ölçülemedi: patlama yükü yok).

### B-K9 — Alıcıda birim mükerrer sayacı yok

- **Ölçüm:** gereksiz varış yakalamadan ölçülüyor (`cp_redund_arr`, `orig_redund_arr`).
- **Dosya/işlev:** `runtime.rs:3055-3056` `admit_chunk` (aday `:3136-3137`): mükerrer birim sessizce `return`.
- **Kök neden:** teslim kapısı mükerreri yalnız düşürüyor, saymıyor.
- **En dar düzeltme:** İSTEK-K1 (koşullu). Yakalama yeterliyse gerek yok.

### B-K10 — Bench'te gönderen tarafı kapasite reddi görünmüyor

- **Ölçüm:** `offer()` QueueFull dönüşünde 1 ms uyuyup yeniden dener ve bunu saymaz. KP-CLIENT satırında `tx_datagrams`, `rx_datagrams`, `unaccepted_offers` yok; bu alanlar `ClientStats`'ta var.
- **Dosya/işlev:** `bench/src/kama_product.rs:482-494` (`offer`), `:813` (KP-CLIENT); `runtime.rs:572-573`, `:585`.
- **Kök neden:** teşhis satırı eksik.
- **En dar düzeltme:** İSTEK-D4 (bench, D'nin dosyası).

### B-K11 — RTT örnek kuralı iki kolda farklı (05b madde 7)

- **Ölçüm (beklenen):**
  - r25/d1000: taban `rtt_samples` ≤ 3/koşu. Her orijinal, ACK'inden önce prob tarafından "superseded" yapılır (B-K1) ve tombstone kol-2 örnek vermez. Taban srtt'si ilk örneklerde donar.
  - Aday `rtt_samples` ≈ M (kol-2 kabul).
  - r1k/1 KB'de benzer yönde fark.
  - Birincil hücrelerde aday srtt / RTT_fifo ∈ [0,9; 1,4]; taban ≥ aday (yeniden gönderim olan hücrelerde) [çıkarım].
- **Dosya/işlev:**
  - Taban `sender.rs:3066-3092`: yalnız en yüksek yeni teyitli **orijinal** (Data); tombstone kol-2'de örnek yok (`:3094-3127`).
  - Aday `sender.rs:3449-3460`: çerçevenin en büyük seq'i; Data/Retransmit/Duplicate; uçuş, tombstone kol-1/kol-2 ya da un-loss. Ret kuralları `:3609-3620`: doygun ack_delay; ack_delay ≥ rtprop iken düzeltilemeyen ham.
- **Kök neden:** 05b'nin kendi düzeltmesi (README §5.5). Arşivde tabanın srtt şişmesi kayıplı ve yüksek hızlı profillerde görülüyor (§6).
- **En dar düzeltme:** adayın madde 7'si. F0'da ölçülür; kod değişmez.

### B-K12 — quinn 25 B/s'de el sıkışmayı D = 60 s içinde tamamlayamaz

- **Ölçüm (beklenen):** r25 hücrelerinde quinn `deliv_D` = 0, `setup_s` ≈ 60 s (handshake_failed çıktısı); yakalamada iki yönde ≥ 1242 B'lik uzun başlıklı paketler. r1k'de kurulum ≈ 3–10 s.
- **Dosya/işlev:** RFC 9000 §14.1 (istemci ve sunucunun ack isteyen Initial datagram'ı ≥ 1200 B); `quinn-proto-0.11.18/src/lib.rs:330`; `bench/src/quic.rs:256` (el sıkışma D ile sınırlı).
- **Kök neden:** fiziksel. İlk uçuş her yönde ≥ 49,7 s; el sıkışma ≥ 99,4 s + 2d. Başlangıç RTT'si 333 ms (`quinn-proto-0.11.18/src/config/transport.rs:383`) olduğundan PTO Initial'ı yeniden gönderir ve kuyruğu büyütür.
- **En dar düzeltme:** yok (rakip kol). Rapor cümlesi: "r25'te quinn, D = 60 s içinde bağlanamadı" — ölçülürse.

---

## 5. Ön-kayıtlı tahminler [çıkarım]

Bunlar koşudan önce yazıldı. Sapma, §9'da bileşenlerine ayrılarak raporlanır. "Tutmadı" da sonuçtur.

| # | hücre sınıfı | tahmin | çürütme ölçütü |
|---|---|---|---|
| T-1 | r1k/r12k l0, taban ve aday | `deliv_D` = 1,0. Tek yön p50 ∈ [1,0; 1,3] × RTT_fifo tek yön. Gönderen-teyit p50 ∈ RTT_fifo + [0; I] (ACK tutması). `wf_useful_share` 65 B'de ∈ [0,44; 0,485], r12k 1 KB'de ∈ [0,85; 0,935] | herhangi bir hücrede medyan aralık dışında |
| T-2 | r1k 1 KB, tüm kayıplar | `tail_probes` medyanı ≥ 0,4 × M; `cp_redund_arr` medyanı ≥ 0,4 × M; `ack_hold_p95_s` ≥ 0,8 × I (B-K4) | `tail_probes` medyanı < 0,2 × M |
| T-3 | r25 d1000 | `tail_probes` medyanı ≥ M − 1; taban `rtt_samples` medyanı ≤ 3, aday ≥ M − 2 (B-K1, B-K11) | `tail_probes` medyanı < M / 2 |
| T-4 | r25, iki gecikme | `establish_tx` = 4; ilk mesajın kopyası ≥ 4; 2. mesajın tek yönü ≥ 12 s (B-K2) | `establish_tx` medyanı ≤ 2 |
| T-5 | r25 l20 | K-3 ölçütü (65 B tek yön p95 ≤ 12 s) iki kolda da sağlanmaz; ideal ARQ bile 14,5/16,6 s | Kama p95 medyanı ≤ 12 s |
| T-6 | l10+ r1k/r12k 65 B | `ack_ranges_med` = 16 (B-K3) | < 12 |
| T-7 | srtt (birincil) | aday srtt / RTT_fifo ∈ [0,9; 1,4]; taban medyanı ≥ aday medyanı (l20/l30) | aday aralık dışında ya da taban < aday |
| T-8 | quinn r25 | `deliv_D` = 0 (B-K12); quinn r1k/r12k l0 `deliv_D` = 1,0 | r25'te quinn teslimi > 0 |
| T-9 | TCP l20/l30 r12k 65 B | TCP tek yön p95 > Kama tek yön p95 (tek bağlantıda HOL; 28 mesaj/s'de bir kayıp sonraki mesajları ≥ 1 RTO bekletir) | TCP p95 ≤ Kama p95 |
| T-10 | TCP r25 | TCP ileri faydalı yük payı ≈ 0,31 (143 B veri + 66 B yalın ACK, mesaj başına); Kama'nın kopyasız payı 0,485, ama B-K1/B-K2 kopyalarıyla ≤ 0,3 | – (betimsel) |
| T-11 | parite | l0'da 0; l20/l30'da ateşlenirse 65 B hücrelerinde `wf_fec_repair` ≥ faydalı yük baytı (B-K5) | – (betimsel) |
| T-12 | kapasite reddi | `cap_reject_total` = 0 bütün birincil hücrelerde (ρ = 0,3; alıcı kuyrukları 1024) | > 0 |
| T-13 | stres l80 | ρ_ARQ = 1,49 > 1: Kama teslim oranı < 1; hat doluluğu ≥ 0,9; betimsel | – |

---

## 6. 05b bağlamı: srtt şişmesi, iki kolda [betimsel, sonradan]

Kaynak: `reports/ktp2/f0/05b-netem/oturum-2-49864510/run-49864510.tar.xz`. Komut: `python3 -I scripts/ktp2/f0/b/arsiv_srtt.py`.
- Geometri: 100 × 256 B, 50 ms aralık, N = 20.
- srtt koşu sonu anlık değeridir.
- RTT_fiz = 2·gecikme + ser(325 B) + ser(73 B).

| profil | RTT_fiz (ms) | RTT_fifo (ms) | kol | srtt medyan (ms) | min–maks (ms) | srtt>0 / N | srtt/RTT_fiz | rtt_samples medyan | tail_probe medyan |
|---|---|---|---|---|---|---|---|---|---|
| p1_loss50 | 31,8 | 31,8 | taban | 112,9 | 37,9–1247,6 | 19/20 | 3,55 | 20 | 12,5 |
| p1_loss50 | 31,8 | 31,8 | aday | 37,7 | 37,3–99,7 | 20/20 | 1,19 | 33 | 11,5 |
| p2_loss80 | 15,9 | 15,9 | taban | 655,0 | 19,2–5031,4 | 13/20 | 41,14 | 4 | 8,5 |
| p2_loss80 | 15,9 | 15,9 | aday | 19,1 | 18,9–77,7 | 17/20 | 1,20 | 15 | 18,5 |
| p4_jitter | 431,8 | 537,4 | taban | 579,6 | 498,8–650,5 | 20/20 | 1,34 | 62 | 0,0 |
| p4_jitter | 431,8 | 537,4 | aday | 568,4 | 460,2–672,1 | 20/20 | 1,32 | 63 | 0,0 |
| p5_revpath | 31,8 | 31,8 | taban | 163,3 | 35,9–16636,7 | 20/20 | 5,13 | 12 | 14,5 |
| p5_revpath | 31,8 | 31,8 | aday | 38,2 | 34,4–994,5 | 20/20 | 1,20 | 15 | 11,5 |
| p7_bbr | 63,7 | 63,7 | taban | 98,2 | 66,3–224,5 | 20/20 | 1,54 | 50 | 1,0 |
| p7_bbr | 63,7 | 63,7 | aday | 95,3 | 65,5–265,6 | 20/20 | 1,50 | 50 | 1,0 |

Okuma sınırları:
- Bu profillerde sunulan yük hattın büyük kesridir: p7'de 325 B / 50 ms = 6,5 kB/s, hat 6,25 kB/s, yani ρ ≈ 1,04. p1'de ARQ ile ≈ 1,0. Bu yüzden gerçek kuyruk srtt'ye girer. p7'de iki kolun eşit oranı (≈ 1,5) bu kuyrukla tutarlıdır.
- Kayıplı profillerde (p1, p2, p5) tabanın srtt'si aday ve RTT_fiz'den büyük. Fark ayrıca aday hesabındaki kabul kuralıyla (madde 7) tutarlıdır; ayırt edici deney yapılmadı (ARSIV-NOTU).
- **B ızgarası bu ölçüyü ρ = 0,3'te, gecikmeli profillerde, iki kolda yapar.** Kuyruk karışıklığı ayrıca ölçülür (`queue_extra`, `rtt_net`). srtt, ölçülen ağ RTT'siyle kıyaslanır (`srtt_over_rttnet`).

---

## 7. İSTEKLER

| kod | kime | dosya / işlev / alan | ne | gerekçe |
|---|---|---|---|---|
| İSTEK-D1 | Ajan D | `crates/bench/src/tcp.rs:164` (`let wait = cfg.max_deadline.min(60 s)`) | TCP istemcisi bütün uygulama ACK'leri gelince ya da D dolunca bitsin (Kama/quinn ile aynı kural) | P1 bütçesinin ≈ %18'i (9,0 sa) TCP'nin sabit beklemesi; ölçüm anlamı değişmez |
| İSTEK-D2 | Ajan D | `crates/bench/src/quic.rs` `run_server_loop` (`:481-526`; `read_to_end` `:516`) + istemci yükü | quinn sunucusu mesaj başına alım anını (unix ns) ve mesaj kimliğini yazsın. Kimlik yükün ilk 8 baytı olsun; TCP çerçevesindeki gibi | quinn için tek yön gecikme ölçülemiyor (şifreli tel) |
| İSTEK-D3 | Ajan D | `scripts/ktp2/f0/arms_rakip.sh` | `rakip_arms` işlevi: girdi `MSGS PAY IVAL_MS DEADLINE_S FWD REV` → çıktı `ARM_quinn_SRV/CLI`, `ARM_tcp_SRV/CLI`. `--manifest @D@/manifest.json --out @D@/out.jsonl` korunsun | B hücreleri bu işlevi varsa kendiliğinden kullanır, yoksa GEÇİCİ tanım (`b/hucre_b.sh`) |
| İSTEK-D4 | Ajan D | `crates/bench/src/kama_product.rs:482-494` `offer()`; `:813` KP-CLIENT | KP-CLIENT'a `tx_dgrams`, `rx_dgrams`, `unaccepted` (ClientStats `tx_datagrams`, `rx_datagrams`, `unaccepted_offers`) ve `offer_qfull` (QueueFull yeniden deneme sayısı), `offer_qfull_wait_ms` eklensin. Salt gözlem | gönderen tarafı kapasite reddi ölçülemiyor (B-K10) |
| İSTEK-K1 (koşullu) | orkestratör | `crates/client-core/src/runtime.rs` `admit_chunk` (taban `:3055-3056`, aday `:3136-3137`); `ServerStats` | `dup_units_rx` ve `dup_unit_bytes_rx` alanları; mükerrer dönüşünde artar. **Yalnız yakalama yetersiz çıkarsa.** İki kolda aynı; commit `[salt-okunur sayaç]`. `tests/api_surface_v0.rs:208-245` alan kümesini pinliyor, güncellenmesi gerekir | gereksiz varışın sayaç düzlemi (B-K9) |
| İSTEK-O1 | orkestratör | `scripts/ktp2/f0/KATALOG.md` §2 | "Bilinen netem davranışları"na ekleme: rate + jitter'da ortalama tek yön gecikmesi profil değerinin üstüne çıkar (sıra koruyan FIFO; `b/fizik.py fifo_owd`) | srtt ve gecikme yorumları bütün şeritlerde |
| İSTEK-E1 | Ajan E | `KTP-2-PROTOKOL-TASLAGI` §6 K-3 | alt sınır 96 B → 134–140 B tel (L2/L3/L4 42 B); %20 kayıpta ideal ARQ p95 14,5 s > 12 s hedefi (fizik.py) | TASLAK-NOTLARI girdisi |

---

## 8. KOŞU İSTEĞİ

1. **Duman (önce):** `F0_REPS=1 scripts/ktp2/f0/f0_grid.sh -j3 <KÖK-DUMAN> $(cat scripts/ktp2/f0/cells/b/DUMAN.txt)`. 6 hücre, 24 koşu; beklenen ≈ 30 dk, en kötü ≈ 42 dk. Ortam: `BIN_TABAN`, `BIN_ADAY`.
2. **Tam set (duman temizse), öncelik sırasıyla:**
   - `cells/b/LISTE-P1.txt` — 24 hücre, 2880 koşu, ≈ 50,3 sa;
   - `LISTE-P2.txt` — 4 hücre, 480 koşu, ≈ 17,3 sa;
   - `LISTE-P3.txt` — 8 hücre, 960 koşu, ≈ 17,1 sa;
   - `LISTE-P4.txt` — 8 hücre, 320 koşu, ≈ 8,1 sa.

   Toplam ≈ 92,9 koşu-saati (en kötü 141,8). `-j3` ile ≈ 31 saat duvar süresi.
   P5/P6 bütçe dışıdır; ancak bütçe kalırsa koşulur.
3. Hücre başına süre (koşu sayısı × koşu süresi): Ek B.
4. Analiz: `python3 -I scripts/ktp2/f0/b/analiz_b.py <KÖK>` → `<KÖK>/analiz_b/{kosular_b.csv, hucre_b.csv, OZET-B.md}`.

Derleme isteği: yok. B şeridi bu turda Rust kodu değiştirmedi; mevcut `bench` ikilileri yeterli.

---

## 9. Sonuçlar

Koşu sonrası doldurulacak. Kurallar:
- N < 30 hücre betimseldir.
- Her hücrede medyan, IQR ve min–maks verilir.
- Ölçülemeyen hücre tek satırdır: "ölçülemedi: neden".

### 9.1 Teslim ve gecikme — (boş)
### 9.2 Hat kapasitesinin faydalı veriye giden payı (ileri / dönüş, sınıf sütunları) — (boş)
### 9.3 srtt ↔ gerçek RTT (taban / aday) — (boş)
### 9.4 Yeniden gönderim: faydalı / gereksiz — (boş)
### 9.5 Fiziksel kapasite ↔ zamanlayıcı — (boş)
### 9.6 Tahminlerle kıyas (T-1…T-13) — (boş)

---

## 10. Dosyalar ve komutlar

| dosya | iş |
|---|---|
| `scripts/ktp2/f0/arms_kama.sh` | taban/aday kol şablonu (`kama_arms`) |
| `scripts/ktp2/f0/b/fizik.py` | tel boyları, RTT_fiz/RTT_fifo, ideal ARQ, geometri, bütçe (tek kaynak) |
| `scripts/ktp2/f0/b/gen_cells.py` | `cells/b/*.cell` + `LISTE-P*.txt` + `DUMAN.txt` üretir |
| `scripts/ktp2/f0/b/hucre_b.sh` | hücre gövdesi: kollar, GEÇİCİ rakip tanımı, yakalama sarması |
| `scripts/ktp2/f0/b/cap_b.py`, `cap_wrap.sh` | uç yakalama (çerçeve türü, birim, PN; TCP seq) |
| `scripts/ktp2/f0/b/analiz_b.py` | analiz (salt okur) |
| `scripts/ktp2/f0/b/sentetik_test.py` | analizin sentetik denetimi (`python3 -I …`, koşu yok) |
| `scripts/ktp2/f0/b/arsiv_srtt.py` | §6 tablosu |
| `scripts/ktp2/f0/cells/b/` | 49 hücre, öncelik listeleri, duman listesi |

<!-- EK-A-BASLA -->
## Ek A — Hücre başına fiziksel ve ideal sınırlar [kod boylar + çıkarım]

Üretim: `python3 -I scripts/ktp2/f0/b/fizik.py tablo` (bu ek o çıktıdır; elle düzenlenmez). Sütunlar: ρ = tek iletim Kama veri paketi / ileri hat; RTT_fiz 0/16 = 0 ve 16 aralıklı ACK; RTT_fifo = netem sıra-koruyan jitter modeli; ideal ARQ = bağımsız kayıp, yeniden deneme aralığı RTT_fiz, ACK kaybı yok.

| hücre | öncelik | N | I (ms) | M | D (s) | ρ ileri | ρ ileri (ideal ARQ) | veri paketi (B) | ser. veri (s) | ser. ACK 0/16 (s) | RTT_fiz 0/16 (s) | RTT_fifo (s) | tek yön alt sınır / fifo (s) | ideal ARQ p95 tek yön (s) | ideal ARQ D içinde | faydalı yük payı ≤ |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| b_r12k_d1000_l0_p1000 | P1 | 30 | 285 | 100 | 60 | 0.300 | 0.300 | 1069 | 0.086 | 0.006/0.011 | 2.091/2.096 | 2.137 | 1.086/1.130 | 1.09 | 1.0000 | 0.935 |
| b_r12k_d1000_l0_p65 | P1 | 30 | 36 | 600 | 30 | 0.298 | 0.298 | 134 | 0.011 | 0.006/0.011 | 2.017/2.022 | 2.371 | 1.011/1.231 | 1.01 | 1.0000 | 0.485 |
| b_r12k_d1000_l20_p1000 | P1 | 30 | 285 | 100 | 60 | 0.300 | 0.375 | 1069 | 0.086 | 0.006/0.011 | 2.091/2.096 | 2.137 | 1.086/1.130 | 3.18 | 1.0000 | 0.935 |
| b_r12k_d1000_l20_p65 | P1 | 30 | 36 | 600 | 30 | 0.298 | 0.372 | 134 | 0.011 | 0.006/0.011 | 2.017/2.022 | 2.371 | 1.011/1.231 | 3.03 | 1.0000 | 0.485 |
| b_r12k_d1000_l30_p1000 | P1 | 30 | 285 | 100 | 60 | 0.300 | 0.429 | 1069 | 0.086 | 0.006/0.011 | 2.091/2.096 | 2.137 | 1.086/1.130 | 5.27 | 1.0000 | 0.935 |
| b_r12k_d1000_l30_p65 | P1 | 30 | 36 | 600 | 30 | 0.298 | 0.425 | 134 | 0.011 | 0.006/0.011 | 2.017/2.022 | 2.371 | 1.011/1.231 | 5.04 | 1.0000 | 0.485 |
| b_r12k_d300_l0_p1000 | P1 | 30 | 285 | 100 | 60 | 0.300 | 0.300 | 1069 | 0.086 | 0.006/0.011 | 0.691/0.696 | 0.691 | 0.386/0.385 | 0.39 | 1.0000 | 0.935 |
| b_r12k_d300_l0_p65 | P1 | 30 | 36 | 600 | 30 | 0.298 | 0.298 | 134 | 0.011 | 0.006/0.011 | 0.617/0.622 | 0.662 | 0.311/0.345 | 0.31 | 1.0000 | 0.485 |
| b_r12k_d300_l20_p1000 | P1 | 30 | 285 | 100 | 60 | 0.300 | 0.375 | 1069 | 0.086 | 0.006/0.011 | 0.691/0.696 | 0.691 | 0.386/0.385 | 1.08 | 1.0000 | 0.935 |
| b_r12k_d300_l20_p65 | P1 | 30 | 36 | 600 | 30 | 0.298 | 0.372 | 134 | 0.011 | 0.006/0.011 | 0.617/0.622 | 0.662 | 0.311/0.345 | 0.93 | 1.0000 | 0.485 |
| b_r12k_d300_l30_p1000 | P1 | 30 | 285 | 100 | 60 | 0.300 | 0.429 | 1069 | 0.086 | 0.006/0.011 | 0.691/0.696 | 0.691 | 0.386/0.385 | 1.77 | 1.0000 | 0.935 |
| b_r12k_d300_l30_p65 | P1 | 30 | 36 | 600 | 30 | 0.298 | 0.425 | 134 | 0.011 | 0.006/0.011 | 0.617/0.622 | 0.662 | 0.311/0.345 | 1.54 | 1.0000 | 0.485 |
| b_r1k_d1000_l0_p1000 | P1 | 30 | 3600 | 20 | 60 | 0.297 | 0.297 | 1069 | 1.069 | 0.073/0.137 | 3.142/3.206 | 3.140 | 2.069/2.068 | 2.07 | 1.0000 | 0.935 |
| b_r1k_d1000_l0_p65 | P1 | 30 | 450 | 100 | 30 | 0.298 | 0.298 | 134 | 0.134 | 0.073/0.137 | 2.207/2.271 | 2.224 | 1.134/1.152 | 1.13 | 1.0000 | 0.485 |
| b_r1k_d1000_l20_p1000 | P1 | 30 | 3600 | 20 | 60 | 0.297 | 0.371 | 1069 | 1.069 | 0.073/0.137 | 3.142/3.206 | 3.140 | 2.069/2.068 | 5.21 | 1.0000 | 0.935 |
| b_r1k_d1000_l20_p65 | P1 | 30 | 450 | 100 | 30 | 0.298 | 0.372 | 134 | 0.134 | 0.073/0.137 | 2.207/2.271 | 2.224 | 1.134/1.152 | 3.34 | 1.0000 | 0.485 |
| b_r1k_d1000_l30_p1000 | P1 | 30 | 3600 | 20 | 60 | 0.297 | 0.424 | 1069 | 1.069 | 0.073/0.137 | 3.142/3.206 | 3.140 | 2.069/2.068 | 8.35 | 1.0000 | 0.935 |
| b_r1k_d1000_l30_p65 | P1 | 30 | 450 | 100 | 30 | 0.298 | 0.425 | 134 | 0.134 | 0.073/0.137 | 2.207/2.271 | 2.224 | 1.134/1.152 | 5.55 | 1.0000 | 0.485 |
| b_r1k_d300_l0_p1000 | P1 | 30 | 3600 | 20 | 60 | 0.297 | 0.297 | 1069 | 1.069 | 0.073/0.137 | 1.742/1.806 | 1.741 | 1.369/1.369 | 1.37 | 1.0000 | 0.935 |
| b_r1k_d300_l0_p65 | P1 | 30 | 450 | 100 | 30 | 0.298 | 0.298 | 134 | 0.134 | 0.073/0.137 | 0.807/0.871 | 0.806 | 0.434/0.434 | 0.43 | 1.0000 | 0.485 |
| b_r1k_d300_l20_p1000 | P1 | 30 | 3600 | 20 | 60 | 0.297 | 0.371 | 1069 | 1.069 | 0.073/0.137 | 1.742/1.806 | 1.741 | 1.369/1.369 | 3.11 | 1.0000 | 0.935 |
| b_r1k_d300_l20_p65 | P1 | 30 | 450 | 100 | 30 | 0.298 | 0.372 | 134 | 0.134 | 0.073/0.137 | 0.807/0.871 | 0.806 | 0.434/0.434 | 1.24 | 1.0000 | 0.485 |
| b_r1k_d300_l30_p1000 | P1 | 30 | 3600 | 20 | 60 | 0.297 | 0.424 | 1069 | 1.069 | 0.073/0.137 | 1.742/1.806 | 1.741 | 1.369/1.369 | 4.85 | 1.0000 | 0.935 |
| b_r1k_d300_l30_p65 | P1 | 30 | 450 | 100 | 30 | 0.298 | 0.425 | 134 | 0.134 | 0.073/0.137 | 0.807/0.871 | 0.806 | 0.434/0.434 | 2.05 | 1.0000 | 0.485 |
| b_r25_d1000_l0_p65 | P2 | 30 | 18000 | 6 | 60 | 0.298 | 0.298 | 134 | 5.360 | 2.920/5.480 | 10.280/12.840 | 10.278 | 6.360/6.359 | 6.36 | 1.0000 | 0.485 |
| b_r25_d1000_l20_p65 | P2 | 30 | 18000 | 6 | 60 | 0.298 | 0.372 | 134 | 5.360 | 2.920/5.480 | 10.280/12.840 | 10.278 | 6.360/6.359 | 16.64 | 0.9999 | 0.485 |
| b_r25_d300_l0_p65 | P2 | 30 | 18000 | 6 | 60 | 0.298 | 0.298 | 134 | 5.360 | 2.920/5.480 | 8.880/11.440 | 8.879 | 5.660/5.660 | 5.66 | 1.0000 | 0.485 |
| b_r25_d300_l20_p65 | P2 | 30 | 18000 | 6 | 60 | 0.298 | 0.372 | 134 | 5.360 | 2.920/5.480 | 8.880/11.440 | 8.879 | 5.660/5.660 | 14.54 | 1.0000 | 0.485 |
| b_r12k_d1000_l10_p1000 | P3 | 30 | 285 | 100 | 60 | 0.300 | 0.333 | 1069 | 0.086 | 0.006/0.011 | 2.091/2.096 | 2.137 | 1.086/1.130 | 3.18 | 1.0000 | 0.935 |
| b_r12k_d1000_l10_p65 | P3 | 30 | 36 | 600 | 30 | 0.298 | 0.331 | 134 | 0.011 | 0.006/0.011 | 2.017/2.022 | 2.371 | 1.011/1.231 | 3.03 | 1.0000 | 0.485 |
| b_r12k_d300_l10_p1000 | P3 | 30 | 285 | 100 | 60 | 0.300 | 0.333 | 1069 | 0.086 | 0.006/0.011 | 0.691/0.696 | 0.691 | 0.386/0.385 | 1.08 | 1.0000 | 0.935 |
| b_r12k_d300_l10_p65 | P3 | 30 | 36 | 600 | 30 | 0.298 | 0.331 | 134 | 0.011 | 0.006/0.011 | 0.617/0.622 | 0.662 | 0.311/0.345 | 0.93 | 1.0000 | 0.485 |
| b_r1k_d1000_l10_p1000 | P3 | 30 | 3600 | 20 | 60 | 0.297 | 0.330 | 1069 | 1.069 | 0.073/0.137 | 3.142/3.206 | 3.140 | 2.069/2.068 | 5.21 | 1.0000 | 0.935 |
| b_r1k_d1000_l10_p65 | P3 | 30 | 450 | 100 | 30 | 0.298 | 0.331 | 134 | 0.134 | 0.073/0.137 | 2.207/2.271 | 2.224 | 1.134/1.152 | 3.34 | 1.0000 | 0.485 |
| b_r1k_d300_l10_p1000 | P3 | 30 | 3600 | 20 | 60 | 0.297 | 0.330 | 1069 | 1.069 | 0.073/0.137 | 1.742/1.806 | 1.741 | 1.369/1.369 | 3.11 | 1.0000 | 0.935 |
| b_r1k_d300_l10_p65 | P3 | 30 | 450 | 100 | 30 | 0.298 | 0.331 | 134 | 0.134 | 0.073/0.137 | 0.807/0.871 | 0.806 | 0.434/0.434 | 1.24 | 1.0000 | 0.485 |
| b_r12k_d300_l50_p1000 | P4 | 10 | 285 | 100 | 60 | 0.300 | 0.600 | 1069 | 0.086 | 0.006/0.011 | 0.691/0.696 | 0.691 | 0.386/0.385 | 3.15 | 1.0000 | 0.935 |
| b_r12k_d300_l50_p65 | P4 | 10 | 36 | 600 | 30 | 0.298 | 0.596 | 134 | 0.011 | 0.006/0.011 | 0.617/0.622 | 0.662 | 0.311/0.345 | 2.78 | 1.0000 | 0.485 |
| b_r12k_d300_l80_p1000 | P4 | 10 | 285 | 100 | 60 | 0.300 | 1.500 | 1069 | 0.086 | 0.006/0.011 | 0.691/0.696 | 0.691 | 0.386/0.385 | 9.37 | 1.0000 | 0.935 |
| b_r12k_d300_l80_p65 | P4 | 10 | 36 | 600 | 30 | 0.298 | 1.489 | 134 | 0.011 | 0.006/0.011 | 0.617/0.622 | 0.662 | 0.311/0.345 | 8.33 | 1.0000 | 0.485 |
| b_r1k_d300_l50_p1000 | P4 | 10 | 3600 | 20 | 60 | 0.297 | 0.594 | 1069 | 1.069 | 0.073/0.137 | 1.742/1.806 | 1.741 | 1.369/1.369 | 8.34 | 1.0000 | 0.935 |
| b_r1k_d300_l50_p65 | P4 | 10 | 450 | 100 | 30 | 0.298 | 0.596 | 134 | 0.134 | 0.073/0.137 | 0.807/0.871 | 0.806 | 0.434/0.434 | 3.66 | 1.0000 | 0.485 |
| b_r1k_d300_l80_p1000 | P4 | 10 | 3600 | 20 | 60 | 0.297 | 1.485 | 1069 | 1.069 | 0.073/0.137 | 1.742/1.806 | 1.741 | 1.369/1.369 | 24.02 | 0.9995 | 0.935 |
| b_r1k_d300_l80_p65 | P4 | 10 | 450 | 100 | 30 | 0.298 | 1.489 | 134 | 0.134 | 0.073/0.137 | 0.807/0.871 | 0.806 | 0.434/0.434 | 10.92 | 0.9997 | 0.485 |
| b_r25_d1000_l10_p65 | P5 | 30 | 18000 | 6 | 60 | 0.298 | 0.331 | 134 | 5.360 | 2.920/5.480 | 10.280/12.840 | 10.278 | 6.360/6.359 | 16.64 | 1.0000 | 0.485 |
| b_r25_d1000_l30_p65 | P5 | 30 | 18000 | 6 | 60 | 0.298 | 0.425 | 134 | 5.360 | 2.920/5.480 | 10.280/12.840 | 10.278 | 6.360/6.359 | 26.92 | 0.9993 | 0.485 |
| b_r25_d300_l10_p65 | P5 | 30 | 18000 | 6 | 60 | 0.298 | 0.331 | 134 | 5.360 | 2.920/5.480 | 8.880/11.440 | 8.879 | 5.660/5.660 | 14.54 | 1.0000 | 0.485 |
| b_r25_d300_l30_p65 | P5 | 30 | 18000 | 6 | 60 | 0.298 | 0.425 | 134 | 5.360 | 2.920/5.480 | 8.880/11.440 | 8.879 | 5.660/5.660 | 23.42 | 0.9998 | 0.485 |
| b_r25_d300_l0_p1000 | P6 | 10 | 143000 | 3 | 60 | 0.299 | 0.299 | 1069 | 42.760 | 2.920/5.480 | 46.280/48.840 | 46.279 | 43.060/43.060 | 43.06 | 1.0000 | 0.935 |
<!-- EK-A-BITIR -->

<!-- EK-B-BASLA -->
## Ek B — Hücre başına koşu süresi ve koşu-saati [çıkarım]

Üretim: `python3 -I scripts/ktp2/f0/b/fizik.py butce`. Kol süreleri koşu başına saniyedir (beklenen); hücre koşu-saati = N × (taban + aday + quinn + tcp) / 3600.

| hücre | öncelik | N | taban/aday (s, beklenen) | quinn (s) | tcp (s) | hücre koşu-saati (beklenen) | en kötü (saat) | BUDGET_S |
|---|---|---|---|---|---|---|---|---|
| b_r12k_d1000_l0_p1000 | P1 | 30 | 34 | 43 | 96 | 1.71 | 3.75 | 185 |
| b_r12k_d1000_l0_p65 | P1 | 30 | 27 | 36 | 59 | 1.23 | 2.28 | 118 |
| b_r12k_d1000_l20_p1000 | P1 | 30 | 40 | 49 | 96 | 1.87 | 3.75 | 185 |
| b_r12k_d1000_l20_p65 | P1 | 30 | 33 | 42 | 59 | 1.38 | 2.28 | 118 |
| b_r12k_d1000_l30_p1000 | P1 | 30 | 40 | 49 | 96 | 1.87 | 3.75 | 185 |
| b_r12k_d1000_l30_p65 | P1 | 30 | 33 | 42 | 59 | 1.38 | 2.28 | 118 |
| b_r12k_d300_l0_p1000 | P1 | 30 | 32 | 37 | 94 | 1.63 | 3.75 | 185 |
| b_r12k_d300_l0_p65 | P1 | 30 | 25 | 30 | 57 | 1.15 | 2.28 | 118 |
| b_r12k_d300_l20_p1000 | P1 | 30 | 34 | 39 | 94 | 1.68 | 3.75 | 185 |
| b_r12k_d300_l20_p65 | P1 | 30 | 27 | 32 | 57 | 1.20 | 2.28 | 118 |
| b_r12k_d300_l30_p1000 | P1 | 30 | 34 | 39 | 94 | 1.68 | 3.75 | 185 |
| b_r12k_d300_l30_p65 | P1 | 30 | 27 | 32 | 57 | 1.20 | 2.28 | 118 |
| b_r1k_d1000_l0_p1000 | P1 | 30 | 78 | 91 | 139 | 3.22 | 5.12 | 225 |
| b_r1k_d1000_l0_p65 | P1 | 30 | 50 | 63 | 82 | 2.05 | 3.05 | 141 |
| b_r1k_d1000_l20_p1000 | P1 | 30 | 88 | 101 | 139 | 3.46 | 5.12 | 225 |
| b_r1k_d1000_l20_p65 | P1 | 30 | 57 | 70 | 82 | 2.22 | 3.05 | 141 |
| b_r1k_d1000_l30_p1000 | P1 | 30 | 88 | 101 | 139 | 3.46 | 5.12 | 225 |
| b_r1k_d1000_l30_p65 | P1 | 30 | 57 | 70 | 82 | 2.22 | 3.05 | 141 |
| b_r1k_d300_l0_p1000 | P1 | 30 | 77 | 85 | 138 | 3.14 | 5.12 | 225 |
| b_r1k_d300_l0_p65 | P1 | 30 | 49 | 57 | 81 | 1.96 | 3.05 | 141 |
| b_r1k_d300_l20_p1000 | P1 | 30 | 82 | 91 | 138 | 3.27 | 5.12 | 225 |
| b_r1k_d300_l20_p65 | P1 | 30 | 51 | 60 | 81 | 2.03 | 3.05 | 141 |
| b_r1k_d300_l30_p1000 | P1 | 30 | 82 | 91 | 138 | 3.27 | 5.12 | 225 |
| b_r1k_d300_l30_p65 | P1 | 30 | 51 | 60 | 81 | 2.03 | 3.05 | 141 |
| b_r25_d1000_l0_p65 | P2 | 30 | 121 | 66 | 181 | 4.08 | 4.71 | 223 |
| b_r25_d1000_l20_p65 | P2 | 30 | 153 | 66 | 181 | 4.61 | 4.71 | 223 |
| b_r25_d300_l0_p65 | P2 | 30 | 120 | 66 | 180 | 4.04 | 4.71 | 223 |
| b_r25_d300_l20_p65 | P2 | 30 | 153 | 66 | 180 | 4.60 | 4.71 | 223 |
| b_r12k_d1000_l10_p1000 | P3 | 30 | 40 | 49 | 96 | 1.87 | 3.75 | 185 |
| b_r12k_d1000_l10_p65 | P3 | 30 | 33 | 42 | 59 | 1.38 | 2.28 | 118 |
| b_r12k_d300_l10_p1000 | P3 | 30 | 34 | 39 | 94 | 1.68 | 3.75 | 185 |
| b_r12k_d300_l10_p65 | P3 | 30 | 27 | 32 | 57 | 1.20 | 2.28 | 118 |
| b_r1k_d1000_l10_p1000 | P3 | 30 | 88 | 101 | 139 | 3.46 | 5.12 | 225 |
| b_r1k_d1000_l10_p65 | P3 | 30 | 57 | 70 | 82 | 2.22 | 3.05 | 141 |
| b_r1k_d300_l10_p1000 | P3 | 30 | 82 | 91 | 138 | 3.27 | 5.12 | 225 |
| b_r1k_d300_l10_p65 | P3 | 30 | 51 | 60 | 81 | 2.03 | 3.05 | 141 |
| b_r12k_d300_l50_p1000 | P4 | 10 | 91 | 96 | 94 | 1.04 | 1.25 | 185 |
| b_r12k_d300_l50_p65 | P4 | 10 | 55 | 60 | 57 | 0.63 | 0.76 | 118 |
| b_r12k_d300_l80_p1000 | P4 | 10 | 91 | 96 | 94 | 1.04 | 1.25 | 185 |
| b_r12k_d300_l80_p65 | P4 | 10 | 55 | 60 | 57 | 0.63 | 0.76 | 118 |
| b_r1k_d300_l50_p1000 | P4 | 10 | 131 | 140 | 138 | 1.50 | 1.71 | 225 |
| b_r1k_d300_l50_p65 | P4 | 10 | 78 | 86 | 81 | 0.89 | 1.02 | 141 |
| b_r1k_d300_l80_p1000 | P4 | 10 | 131 | 140 | 138 | 1.50 | 1.71 | 225 |
| b_r1k_d300_l80_p65 | P4 | 10 | 78 | 86 | 81 | 0.89 | 1.02 | 141 |
| b_r25_d1000_l10_p65 | P5 | 30 | 153 | 66 | 181 | 4.61 | 4.71 | 223 |
| b_r25_d1000_l30_p65 | P5 | 30 | 153 | 66 | 181 | 4.61 | 4.71 | 223 |
| b_r25_d300_l10_p65 | P5 | 30 | 153 | 66 | 180 | 4.60 | 4.71 | 223 |
| b_r25_d300_l30_p65 | P5 | 30 | 153 | 66 | 180 | 4.60 | 4.71 | 223 |
| b_r25_d300_l0_p1000 | P6 | 10 | 349 | 66 | 501 | 3.51 | 3.55 | 544 |

P1: beklenen 50.3 sa (kümülatif 50.3) · en kötü 85.2 sa (kümülatif 85.2)
P2: beklenen 17.3 sa (kümülatif 67.7) · en kötü 18.8 sa (kümülatif 104.0)
P3: beklenen 17.1 sa (kümülatif 84.8) · en kötü 28.4 sa (kümülatif 132.4)
P4: beklenen 8.1 sa (kümülatif 92.9) · en kötü 9.5 sa (kümülatif 141.8)
P5: beklenen 18.4 sa (kümülatif 111.3) · en kötü 18.8 sa (kümülatif 160.7)
P6: beklenen 3.5 sa (kümülatif 114.8) · en kötü 3.5 sa (kümülatif 164.2)
<!-- EK-B-BITIR -->
