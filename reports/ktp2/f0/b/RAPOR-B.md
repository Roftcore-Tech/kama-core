# KTP2-F0 / Ajan B — Düşük bant ve yüksek RTT: rapor taslağı

**Tur 2 — duman-1 sonrası, tam set öncesi.** Hüküm verecek ölçüm yok. Kaynaklar:
- [kod] kaynak okuması (dosya:satır);
- [çıkarım] `scripts/ktp2/f0/b/fizik.py` hesabı;
- §6: 05b arşivi [betimsel, sonradan];
- §9.0: duman-1 [ölçüldü, N = 1, betimsel]. Duman, koşucu düzeltmesi `aa9935ac`'den önce koştu; kirli koşular işaretlidir.

Tam set sonrası §9 doldurulur. §5'teki tahminler o zaman sapma dekompozisyonuyla kıyaslanır.

Dayanak: `ISEMRI-KTP2-F0-2026-10-29` §3 "Ajan B"; `scripts/ktp2/f0/KATALOG.md`; `ORTAK-KURALLAR.md`.
Kollar: `taban` = main `a858f3fc` (+ ölçüm kodu), `aday` = 05b `49864510` (+ aynı ölçüm kodu), `quinn` / `tcp` = taban ikilisinin rakip kolları.
Dal: `ktp2/f0-b`. Bu turda ürün kodu değişmedi.

Kod farkı denetimi [kod]: B'nin dayandığı dosyalardan `receiver.rs`, `ack.rs`, `tail_recovery.rs`, `pacer.rs`, `rtt.rs` (adaptive-transport) ile `seam.rs`, `wire.rs` (client-core) iki kolda bayt bayt aynıdır. Kollar `sender.rs`, `arq.rs`, `loss.rs` ve `client-core/src/runtime.rs`'de ayrışır (`git diff --stat a858f3fc 49864510 -- crates/`). Satır numaraları taban içindir; adayda farklıysa ayrıca yazılır.

---

## 0. Bu turun özeti

1. **Izgara:** 58 hücre. 49 ana hücre var; r25'te quinn için 9 ayrı betimsel hücre ayrıldı (N = 5).
   - P1–P5 için **≈ 94,6 koşu-saati** beklenir (en kötü 164,7). Bütçe dışı yalnız P6'dır (≈ 3,0 sa).
   - Tur 1'de r25 l10/l30 (P5) budanmıştı. TCP kolu artık erken bittiği için (Ajan D, transport-ack) P5 bütçeye girdi.
   - Ayrıntı §1 ve Ek B'de.
2. **Sunulan yük:** ileri hat kapasitesinin %30'u. Ölçü, tek iletimli Kama veri paketinin L2 dahil boyudur. Mesaj boyları açık: 65 B ve 1000 B. "1 KB" için 1000 B seçildi, çünkü 1024 B ürün v1 profilinde tek birim olarak gönderilemez (B-K6).
3. **Koddan en güçlü dört bulgu** (§4; hepsi [kod] + [çıkarım], ölçümle doğrulanacak):
   - **B-K1:** Kuyruk probu PTO'su 10 s'de kırpılıyor. 25 B/s'de RTT 8,9–12,8 s olduğundan, d1000'de her mesaj için gereksiz bir tam kopya bekleniyor.
   - **B-K2:** Kuruluş probu jetonsuz ve 0,5 s'den başlıyor. 25 B/s'de ilk RTT içinde ilk mesajın 4 kopyasını, artı 1 geometrik kopyasını gönderiyor. Bu ≈ 27 s ileri hat süresi eder.
   - **B-K4:** Alıcı ACK'i ikinci veriye kadar tutuyor (`ACK_FREQ_N = 2`, tutma ≤ 8,39 s). Gönderenin PTO'su ise 25 ms tutma varsayıyor. Bu çelişki 1 kB/s – 1 KB hücrelerinde mesajların %50–100'ü için gereksiz kuyruk probu üretir.
   - **B-K3:** Yeniden gönderim yeni PN alıyor, bu yüzden SACK delikleri kalıcı. 16 kayıptan sonra her ACK 137 B olur (yalın ACK 73 B). 25 B/s'de bu, ACK başına +2,56 s demektir.
4. **K-3 notu (Ajan E'ye):** Taslağın K-3 alt sınırı (96 B → 3,8 s) L2/L3/L4'ün 42 B'sini saymıyor. Bench'te 65 B'lik mesaj telde 134 B'dir; spec zarfıyla 140 B. 25 B/s'de tek yön alt sınır 5,66 s'dir (d300).
   %20 kayıpta ideal ARQ'nun p95'i 14,5 s'dir; bu, K-3 hedefi olan 12 s'nin üstündedir. Hedefi yalnız eşzamanlı 2 kopya karşılar (11,0 s) [çıkarım, fizik.py].
5. **Duman-1 [ölçüldü, N = 1, betimsel; §9.0]:**
   - **B-K13, yeni:** PARAM_SET 200 ms'de bir, 25 kez yeniden gönderiliyor (`runtime.rs:2554-2569`). 25 B/s'de bu 25 × 93 B = 2325 B, yani ≈ 93 s ileri hat süresi eder. Temiz r25 koşusunda aday için ölçülenler:
     - ileri hattın geç kalma süresinin %57'si PARAM ile dolu;
     - PreConverge 33 s sürdü, srtt 94,6 s;
     - tek yön p50 74,8 s; 6 mesajın 1'i D içinde.
   - **B-K8, ölçüldü:** r1k/d1000/l20/1 KB'de taban PreConverge'de 60,6 s kaldı. 31 B/s tabanıyla gönderende bekleme p95 48 s oldu (adayda 0,27 s).
   - **B-K14 (= Ajan C B-C9), B ızgarasında da görüldü:** r12k/d300/l30/65 B'de iki kolda gecikme 2–3,5 s birikti. Birikimin tamamı gönderen kuyruğunda; darboğaz kuyruk fazlası p95 0,08 s, hat doluluğu ≤ 0,21. Fiziksel kapasite değil.
   - **Kirli koşular:** r25 hücresinde tcp, taban ve quinn koşuları önceki koşunun netem artığıyla başladı (15,4 KB = 10 dk). `aa9935ac` bunu düzeltti; bu koşular yorumlanmaz.
6. **Ölçüm araçları:**
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
| **P1** | r1k/r12k × d300/d1000 × l0/l20/l30 × 65 B / 1000 B (24 hücre, 4 kol) | 30 | 41,9 (41,9) | 89,5 (89,5) |
| **P2** | r25 × d300/d1000 × l0/l20 × 65 B (4 hücre: taban, aday, tcp) + quinn (4 hücre) | 30 / quinn 5 | 14,3 (56,2) | 17,7 (107,2) |
| **P3** | r1k/r12k × d300/d1000 × l10 × 65 B / 1000 B (8 hücre) | 30 | 14,4 (70,7) | 29,8 (137,1) |
| **P4** | stres: r1k/r12k × d300 × l50/l80 × 65 B / 1000 B (8 hücre) | 10 | 8,0 (78,7) | 9,9 (147,0) |
| **P5** | r25 × d300/d1000 × l10/l30 × 65 B (4 + 4 quinn hücre) | 30 / quinn 5 | 15,9 (94,6) | 17,7 (164,7) |
| P6 (bütçe dışı) | r25 × d300 × l0 × 1000 B (1 + 1 quinn hücre) | 10 / quinn 5 | 3,0 (97,6) | 3,2 (167,8) |

Budama ve kararlar:
- r25'te 1 KB P6'dadır, betimseldir: tek mesajın serileştirmesi 42,8 s; ρ = 0,3'te aralık 143 s.
- **r25'te quinn ayrı hücrede, N = 5, betimsel.** İlk uçuş her yönde ≥ 1242 B olduğundan el sıkışma ≥ 99,4 s + 2d sürer; bu HS_TIMEOUT_S = D = 60 s'yi aşar (RFC 9000 §14.1; Ajan D'nin hesabı ≥ ≈ 100 s). Sonuç fizikle belirli: 0 teslim. 30 tekrar bütçe harcar, bilgi eklemez. Ana r25 hücreleri `taban aday tcp` koşar. Rapor cümlesi: "r25'te quinn D içinde bağlanamadı (N = 5, betimsel)".
- Stres hücreleri yalnız d300'dedir.

Koşu süresi modeli (`fizik.run_time`) [çıkarım]:
- **Kama:** sunum + kuyruk süresi. Kuyruk, kayıpsızda RTT + I; kayıpta min(D, 4·RTT + I); streste D. İstemci, bütün teyitler gelince ya da son sunum + D'de biter (`kama_product.rs:757-781`).
- **quinn:** el sıkışma + sunum + kuyruk. El sıkışma D ile sınırlıdır (`quic.rs:256`). r25'te başarısız el sıkışma sonrası mesaj sunulmaz.
- **TCP:** birincil kol artık `--rival-delivery transport-ack` (Ajan D, `run_client_tack`). Teslim `snd_una` ile ölçülür; bütün mesajlar teslim edilince ya da son plan + D'de biter. Bağlanma tavanı HS_TIMEOUT_S = D'dir. Tur 1'deki sabit bekleme (eski `tcp.rs:164`, İSTEK-D1) birincil kolda yok.

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
- duman-1'de quinn ve tcp geçici tanımla koştu (`F0_RAKIP_TANIM=gecici-B`). Tam sette Ajan D'nin `rakip_arms`'ı kullanılır.

### 1.4 Kollar ve yakalama

- **taban, aday:** `scripts/ktp2/f0/arms_kama.sh` → `kama_arms`. İki kolda komut bayt bayt aynıdır, fark yalnız ikilidir:
  - `serve --bind @SRV@ --transport kama-product --payload P`
  - `client … --msgs M --payload P --submit-interval-ms I --max-deadline-s D --seed @REP@ --repeat @REP@ --fwd-impair '<FWD>' --rev-impair '<REV>' --manifest @D@/manifest.json --out @D@/out.jsonl`

  Ürün v1 profili bench varsayılanlarıdır (fec 0x05 adaptive, tail validated_v1, IW 2400 B).
  Kama sunucusu `--rx-log @D@/rx_log.jsonl` yazar: alıcı teslim damgası, şema ktp2-c-rx-v1, dört kolda aynı (İSTEK-B-D). İstemci `--ts-out @D@/ts.csv` yazar: 50 ms motor zaman serisi, salt gözlem, Ajan C ile aynı. Bunlar `KAMA_RXLOG=1` ve `KAMA_TS=1` varsayılanlarıdır.
- **quinn, tcp:** Ajan D'nin `scripts/ktp2/f0/arms_rakip.sh` → `rakip_arms`. Hücreler bunu kendiliğinden kullanır; `b/hucre_b.sh`'deki geçici tanım yalnız yedektir.
  - Birincil kollar taşıma-ACK teslim tanımlıdır.
    - quinn: mesaj başına uni-stream; teslim `SendStream::stopped`; BBR; `--payload-id` ile alıcı damgası.
    - tcp: `snd_una`, erken bitiş.
  - Eski uygulama yanıtlı kollar (`quinn_rpl`, `tcp_rpl`) B ızgarasında koşmaz. Ajan D'nin hücrelerindedir.
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
| alıcı teslim | sunum → sunucunun uygulamaya teslimi; dört kolda aynı şema. Kama ve quinn'de sunum anı `t0_unix_ns`'ten; tcp'de yakalamadaki ilk veri segmentinden hizalanır. Çıktılar: `rx_deliv_D`, `rx_ow_p50/p95/p99_s` | rx_log |
| gönderen kilidi (Ajan C B-C9 ile aynı tanım) | `btlbw_sunulan` = motor btlbw / (16 + 11 + yük)/I; `ilk_iletim_payi` = DATA baytı / (M × (16 + 11 + yük)); `birikim_ms` = mesaj sırasıyla 12 pencerede pencere-minimum gecikmenin son − ilk farkı (kaynak sırası: rx_log, yakalama, teyit); `jeton_ac_payi` = ts.csv'nin son 2/3'ünde jeton < tek DATA; `kilitli` = birikim ≥ 1 s ya da ilk iletim payı < 0,95 | sayaç, yakalama, ts.csv |
| — B ayrımı | `gonderen_birikim_ms`: aynı pencere yöntemiyle sunum → A'dan ilk çıkış. `kilit_gonderen` = bu değer ≥ 1 s. Birikimin gönderen kuyruğunda mı, darboğazda mı (`queue_extra`) olduğunu ayırır. C'nin `kilitli` göstergesi darboğaz kuyruğunu da (örn. PARAM fırtınası) kilit sayar | yakalama |
| kirli başlangıç | koşu başında MID netem kuyruğunda kalan bayt > 200 B. Bağlantı açılışının IPv6 ND/MLD paketleri (≈ 86–90 B) eşiğin altında kalır. `aa9935ac` sonrası 0 beklenir | qdisc |

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

### B-K8 — 31 B/s sabit hız tabanı: PreConverge'de 1 KB mesajı bekletiyor; yakınsamadan sonra da 25 B/s hattının üstünde

- **Ölçüm [duman-1, N = 1, betimsel]:**
  - r1k/d1000/l20/1 KB taban'da `left_preconv` 60,6 s (aday 3,3 s). Bu süre boyunca hız 31 B/s idi. 1027 B'lik birim ≈ 33 s jeton bekler.
    - gönderende bekleme p95 48 s (aday 0,27 s);
    - tek yön p50 32,8 s (aday 2,2 s); `gonderen_birikim_ms` 6440.
  - Uzun PreConverge'in nedeni [çıkarım]: tabanda kuruluş probu (Duplicate) ve kopyaların ACK'leri RTT örneği vermez (B-K11). Yeni orijinal 31 B/s'de seyrek çıkar; ilk orijinalin ACK'i kaybolursa (%20) çıkış uzar.
  - r25 aday: pacer 50 B/s, yani hattın veri payının 2,91 katı; gönderim / hat pencere p95 = 2,8.
  - IW etkisi bu ızgarada ölçülemedi: patlama yükü yok.
- **Dosya/işlev:**
  - `sender.rs:1485` `PRE_CONVERGE_FLOOR_BW` 31 (PreConverge'de `compute_send_rate` `:4239-4244`).
  - Yakınsamadan sonra kalıcı taban: `budget.rs:29` `min_rate_bps: 31` → `send_rate_final` (`sender.rs:4268`). Bu bulguyu Ajan E D-K3 olarak da kaydetti.
  - Motor UDP yükü birimiyle sayar: 31 B/s, 65 B'de L2 dahil 45 B/s eder; 25 B/s hattın 1,8 katı.
  - `pacer.rs:9` (2 × 1200 B IW); `runtime.rs:248`.
- **Kök neden:** taban hız sabit ve hattan türetilmemiş.
  - Düşük hatta (r25) taban hız hat hızının üstündedir. Gönderen kendi kuyruğunu netem'de büyütür; kopyalar ve PARAM birikir.
  - Yüksek hatta (r1k, 1 KB) PreConverge'de hız 31 B/s'de kalır. Ölçüm yokken hat kapasitesinin %3'üyle gönderir.
- **En dar düzeltme:** iki dar değişiklik.
  1. PreConverge çıkışını Duplicate/kuruluş probu ACK'inden de mümkün kılmak. Bu, adayın madde 7 kabul kuralıdır; ölçülen davranış adayda düzelmiş [duman, N = 1].
  2. `min_rate_bps`'yi ölçülen teslim hızının altına inebilir yapmak: `send_rate_final(…, min(min_rate_bps, btlbw), …)`. Örnek yoksa taban 31 B/s kalır.

  Tel biçimi değişmez.

### B-K9 — Alıcıda birim mükerrer sayacı yok

- **Ölçüm:** gereksiz varış yakalamadan ölçülüyor (`cp_redund_arr`, `orig_redund_arr`).
- **Dosya/işlev:** `runtime.rs:3055-3056` `admit_chunk` (aday `:3136-3137`): mükerrer birim sessizce `return`.
- **Kök neden:** teslim kapısı mükerreri yalnız düşürüyor, saymıyor.
- **En dar düzeltme:** İSTEK-K1 (koşullu). Yakalama yeterliyse gerek yok.

### B-K10 — Bench'te gönderen tarafı kapasite reddi görünmüyor

- **Ölçüm:** tur 1'de `offer()` QueueFull dönüşünde 1 ms uyuyup yeniden deniyor ve bunu saymıyordu. KP-CLIENT satırında `tx_datagrams`, `rx_datagrams`, `unaccepted_offers` yok; bu alanlar `ClientStats`'ta var.
- **Dosya/işlev:** `bench/src/kama_product.rs:482-494` (`offer`), `:813` (KP-CLIENT); `runtime.rs:572-573`, `:585`.
- **Kök neden:** teşhis satırı eksik.
- **En dar düzeltme:** İSTEK-D4, karşılandı. `KP-CLIENT-OFFER tx_dgrams rx_dgrams unaccepted offer_qfull offer_qfull_wait_ms` satırı geldi (`c2f230b5`); `analiz_b.py` okuyor.

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

### B-K13 — PARAM_SET 200 ms'de bir, 25 kez: düşük bantta ilk dakikayı dolduruyor [kod + duman]

- **Ölçüm:**
  - [duman-1, N = 1] Bütün Kama koşularında `tx_ctrl_B` = 1275 B (25 × 51 B) gözlendi: r25, r12k/l80 ve r1k/d1000. PARAM_ACK RTT içinde dönmezse 25 deneme ≈ 5 s'de biter.
  - r25'te bunun telde karşılığı 25 × 93 B = 2325 B, yani **93 s** ileri hat süresi; 6 mesajlık koşunun penceresi 150 s.
  - Temiz r25 aday koşusu: geç kalma süresinde hattın %57'si PARAM ile dolu (`late_busy_ctrl_frac` 0,574); `queue_extra_p95` 116 s; `left_preconv` 33 s; srtt 94,6 s.
  - r1k/d300'de 3 deneme (`tx_ctrl_B` 153): ACK çabuk dönüyor.
- **Dosya/işlev:** `client-core/src/runtime.rs:2554-2569` (rtprop tick'inde `param_tries < 25 && now ≥ son + 200 ms`). İki kolda aynı.
- **Kök neden:** yeniden deneme aralığı sabit ve RTT'den türetilmemiş. 25 B/s'de RTT ≥ 10 s. Bu yüzden 25 deneme PARAM_ACK dönmeden biter ve hepsi FIFO darboğazda veriden önce kuyruğa girer.
- **En dar düzeltme:** aralığı katlamak (200 ms × 2^k, deneme tavanı 25 korunur) ya da `max(200 ms, srtt, rtt_floor)` yapmak. Burada `rtt_floor` = (35 + 16 + 65)/31 B/s ≈ 3,7 s; B-K2 ile aynı türetim. Tel biçimi değişmez. Katlamayla 25 B/s'nin ilk 10 s'sinde ≈ 6 deneme kalır (≈ 22 s hat süresi).

### B-K14 — Gönderen kilidi (Ajan C B-C9) B ızgarasında da var; birikim gönderen kuyruğunda [duman]

- **Ölçüm [duman-1, N = 1, betimsel]:** r12k/d300/l30/65 B (600 mesaj, 36 ms aralık).

  | | taban | aday |
  |---|---|---|
  | `birikim_ms` | 2060 | 3500 |
  | `gonderen_birikim_ms` | 2020 | 3490 |
  | `btlbw_sunulan` | 1,00 | 1,07 |
  | gönderende bekleme p95 | 2,2 s | 3,7 s |
  | darboğaz kuyruk fazlası p95 | 0,08 s | 0,07 s |
  | hat doluluğu | 0,17 | 0,21 |
  | geç kalma süresinde hat boş | 0,80 | 0,76 |

  Gecikme hat dolu olduğu için değil, gönderen tuttuğu için büyüyor. Benzer görünüm r12k/d1000/l10/1 KB taban'da var (`gonderen_birikim_ms` 2140, `btlbw_sunulan` 2,5). Bu ikinci hücrede btlbw sunulanın üstünde olduğu için kilit tek başına btlbw ile açıklanmaz; tam sette ayrıştırılır.
- **Dosya/işlev:** Ajan C'nin izi [çıkarım]: `bw.rs:220-245` (kuyruk doluyken teslim örneği ≈ pacer × (1 − p)); `rate.rs:164-180` ve `sender.rs:4239-4268` (Steady kazancı 1); seyrek ProbeUp (`sender.rs:1537`).
- **Kök neden [çıkarım]:** pacer sunulan hıza kilitlenir, yeniden gönderim bandı ayrılmaz. Kayıplar yeni veriyle aynı jetonları paylaşınca gönderen kuyruğu doğrusal büyür.
- **En dar düzeltme:** Ajan C'nin RAPOR-C B-C9 önerisi esas. B tam sette `kilit_gonderen` ile hangi hücrelerde ve ne kadar olduğunu ölçer.

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

Duman-1'den sonra, tam setten önce eklenenler:
- Bunlar ön-kayıttır ama duman verisini gördükten sonra yazıldı.
- Duman-1'de T-13'ün "hat doluluğu ≥ 0,9" kısmı tutmadı: r12k/l80'de doluluk 0,09–0,11 [N = 1].

| # | hücre sınıfı | tahmin | çürütme ölçütü |
|---|---|---|---|
| T-14 | r25 Kama, iki kol | `param_sends` medyanı = 25; `late_busy_ctrl_frac` medyanı ≥ 0,3 (B-K13) | `param_sends` medyanı < 10 |
| T-15 | r12k 65 B l20/l30 | `kilit_gonderen` = 1 koşuların ≥ yarısında, iki kolda; `queue_extra_p95` < 0,2 s (B-K14) | `kilit_gonderen` = 1 koşuların < %20'sinde |
| T-16 | r1k 1 KB l20/l30 | taban `left_preconv` medyanı ≥ 10 s, aday ≤ 5 s (B-K8, B-K11) | taban medyanı < 5 s |
| T-17 | kirli başlangıç | `kirli_baslangic` = 0 bütün koşularda (`aa9935ac`) | > 0 |

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

Durum (2026-10-29, duman-1 sonrası): D1–D4 Ajan D'de karşılandı (`c2f230b5`, `arms_rakip.sh`). Birincil tcp kolu erken biter; quinn `--rx-log` ve `--payload-id` aldı; `KP-CLIENT-OFFER` satırı geldi. O1 KATALOG §2'ye işlendi (`aa9935ac`). Koşular arası netem boşaltması `aa9935ac`'de (Ajan D İSTEK-O-D1; duman-1'in r25 kirliliği bu). E1 TASLAK-NOTLARI V-1/V-4'te. K1 koşullu kalır.

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

1. **Duman-1:** koştu (2026-10-29, 24/24 rc = 0). Sonuç §9.0'da.
2. **Tam set, öncelik sırasıyla:**

   | liste | hücre | koşu | ≈ koşu-saati |
   |---|---|---|---|
   | `cells/b/LISTE-P1.txt` | 24 | 2880 | 41,9 |
   | `LISTE-P2.txt` | 4 + 4 quinn | 360 + 20 | 14,3 |
   | `LISTE-P3.txt` | 8 | 960 | 14,4 |
   | `LISTE-P4.txt` | 8 | 320 | 8,0 |
   | `LISTE-P5.txt` | 4 + 4 quinn | 360 + 20 | 15,9 |

   - Toplam ≈ 94,6 koşu-saati (en kötü 164,7). P6 bütçe dışıdır (≈ 3,0 sa); ancak bütçe kalırsa koşulur.
   - Ön koşul: bench iki kolda `ktp2/f0-taban`'dan (D birleşmesi `6cf74c69` sonrası) yeniden derlenmiş olmalı. `--rx-log`, `--ts-out`, `--rival-delivery` ve `--payload-id` bu derlemede gerekir; eski ikili `--rx-log`'u tanımaz.
3. Hücre başına süre (koşu sayısı × koşu süresi): Ek B.
4. Analiz: `python3 -I scripts/ktp2/f0/b/analiz_b.py <KÖK>` → `<KÖK>/analiz_b/{kosular_b.csv, hucre_b.csv, OZET-B.md}`.

Derleme isteği: yok. B şeridi bu turda Rust kodu değiştirmedi; mevcut `bench` ikilileri yeterli.

---

## 9. Sonuçlar

Koşu sonrası doldurulacak. Kurallar:
- N < 30 hücre betimseldir.
- Her hücrede medyan, IQR ve min–maks verilir.
- Ölçülemeyen hücre tek satırdır: "ölçülemedi: neden".

### 9.0 Duman-1 [ölçüldü, N = 1, betimsel; hüküm yok]

Kaynak: `/home/ryuzaki/kama-k4g-work/ktp2-f0/runs/duman-b/out`. Analiz: `reports/ktp2/f0/b/duman-1/` (`kosular_b.csv`, `OZET-B.md`, `KAYNAK.txt`).
- İkililer: taban `14b5f7d3…` (a858f3fc), aday `fd3089b1…` (49864510). D birleşmesinden önceki bench; rx_log ve ts.csv yok.
- Koşu `-j 6`, C'nin 4 duman hücresiyle eşzamanlı.
- 24/24 koşu rc = 0, yakalama 48/48 dosya. Analiz hatası 0.

**Araç denetimi.**
- Ayrıştırma, yakalama çözümü, ACK kapsamı ve sınıf sütunları çalıştı. Örnek: r1k/d300/l0 tel sınıfları qdisc baytıyla tutarlı; ileri doluluk 0,31–0,32, beklenen ρ 0,30.
- Duman sonrası araçta düzeltilenler (`e236f6e4`):
  - srtt = 0 (örnek yok) oranı tanımsız yazıldı;
  - B'ye varış yoksa pencere sıfırlanıyordu;
  - geç kalma payına jitter p95 eklendi; l0'da jitter gecikmeyi "geç" sayıyordu;
  - FEC birleştirici tahliyesi kapasite reddinden ayrıldı (r12k/l80'de 85/134; kullanıcı verisini düşürmez);
  - kirli başlangıç sütunu eklendi.

**Kirli başlangıç (koşucu kusuru, `aa9935ac` ile giderildi).** r25 hücresinde koşular önceki koşunun netem kuyruğuyla başladı:

| kol | ileri artık |
|---|---|
| quinn | 1614 B / 13 paket |
| tcp | 14 904 B / 12 paket (quinn'in 1242 B'lik Initial yeniden gönderimleri) |
| taban | 15 422 B / 19 paket |

- 25 B/s'de 15 KB ≈ 10 dk hat süresidir. Taban koşusunda sunucuya 150 s boyunca hiç Kama paketi ulaşmadı: `srv rx_datagrams` 0, vb0'da yalnız eski QUIC paketleri ve ICMP yanıtları görüldü.
- Bu üç r25 koşusu yorumlanmaz. tcp'nin r25'te bağlanamaması (`setup_s` 20,0) da bu kirliliğin altındadır; kanıt sayılmaz.
- Diğer hücrelerde artık ≤ 352 B. İlk koşudaki 86–90 B'lik paketler IPv6 ND/MLD'dir.

**Koşu başına özet** (yakalamadan tek yön; sunum → B'ye ilk varış):

| hücre | kol | kirli | teslim D | teyit p50 s | tek yön p50 s | tek yön p95 s | srtt/RTT_fifo | RTT örn. | kuyruk probu | kuruluş probu | PARAM | gereksiz kopya | faydalı pay (ileri) | gönd. birikim ms | geç kalmada hat boş | kurulum s |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| r12k_d1000_l10_p1000 | taban | 0 | 1 | 9,37 | 8,03 | 13,3 | 1,1 | 53 | 0 | 2 | 12 | 2 | 0,909 | 2143 | 0,722 | 0,000107 |
| r12k_d1000_l10_p1000 | aday | 1 | 1 | 4,41 | 3,27 | 5,46 | 1,09 | 66 | 0 | 2 | 10 | 2 | 0,91 | -0,208 | 0,7 | 0,000314 |
| r12k_d1000_l10_p1000 | quinn | 0 | 1 | 2,29 | – | – | – | – | – | – | – | – | 0,815 | – | – | 1,89 |
| r12k_d1000_l10_p1000 | tcp | 0 | 1 | 11,6 | 6,5 | 16,5 | – | – | – | – | – | – | 0,909 | – | – | 1,81 |
| r12k_d300_l30_p65 | taban | 0 | 0,998 | 1,92 | 1,55 | 7,16 | 1,01 | 244 | 5 | 1 | 4 | 1 | 0,483 | 2020 | 0,8 | 0,000657 |
| r12k_d300_l30_p65 | aday | 0 | 1 | 2,3 | 1,93 | 4,52 | 1,01 | 299 | 2 | 1 | 3 | 3 | 0,481 | 3492 | 0,764 | 0,000246 |
| r12k_d300_l30_p65 | quinn | 0 | 1 | 1,41 | – | – | – | – | – | – | – | – | 0,275 | – | – | 0,944 |
| r12k_d300_l30_p65 | tcp | 0 | 0,988 | 14,9 | 12,8 | 17,3 | – | – | – | – | – | – | 0,77 | – | – | 0,499 |
| r12k_d300_l80_p65 | taban | 0 | 0,677 | 3,63 | 2,99 | 28,1 | 0,949 | 47 | 13 | 1 | 25 | 3 | 0,478 | 1,18 | 0,909 | 0,000204 |
| r12k_d300_l80_p65 | aday | 0 | 0,763 | 6,81 | 6,68 | 30,4 | 0,866 | 84 | 2 | 2 | 25 | 12 | 0,472 | 17,4 | 0,892 | 0,000357 |
| r12k_d300_l80_p65 | quinn | 0 | 0 | – | – | – | – | – | – | – | – | – | 0 | – | – | 30 |
| r12k_d300_l80_p65 | tcp | 0 | 0,0217 | 7,49 | 0,531 | 11,3 | – | – | – | – | – | – | 0,639 | – | – | 5,77 |
| r1k_d1000_l20_p1000 | taban | 0 | 0,95 | 33,9 | 32,8 | 52,2 | 1,25 | 9 | 1 | 5 | 12 | 6 | 0,726 | 6440 | 0,725 | 0,000344 |
| r1k_d1000_l20_p1000 | aday | 0 | 1 | 6,97 | 2,2 | 12,6 | 1,08 | 11 | 8 | 2 | 13 | 7 | 0,676 | 0,0832 | 0,586 | 0,000303 |
| r1k_d1000_l20_p1000 | quinn | 0 | 1 | 3,44 | – | – | – | – | – | – | – | – | 0,656 | – | – | 7,17 |
| r1k_d1000_l20_p1000 | tcp | 0 | 0,3 | 60 | 55,1 | 60,4 | – | – | – | – | – | – | 0,862 | – | – | 2,08 |
| r1k_d300_l0_p65 | taban | 0 | 1 | 1,13 | 0,471 | 0,672 | 1,06 | 55 | 4 | 1 | 3 | 5 | 0,453 | 0,0274 | 0,934 | 9,56e-05 |
| r1k_d300_l0_p65 | aday | 0 | 1 | 1,13 | 0,44 | 0,58 | 0,943 | 54 | 4 | 1 | 3 | 5 | 0,453 | -0,00191 | 1 | 0,00059 |
| r1k_d300_l0_p65 | quinn | 0 | 1 | 0,897 | – | – | – | – | – | – | – | – | 0,208 | – | – | 3,24 |
| r1k_d300_l0_p65 | tcp | 0 | 1 | 0,83 | 0,46 | 0,55 | – | – | – | – | – | – | 0,308 | – | – | 0,737 |
| r25_d1000_l20_p65 | taban | 1 | 0 | – | – | – | – | 0 | 3 | 21 | 25 | 0 | – | – | – | 0,000151 |
| r25_d1000_l20_p65 | aday | 0 | 0,167 | 33 | 74,8 | 122 | 9,21 | 4 | 7 | 7 | 25 | 7 | 0,0412 | -0,116 | 0,132 | 0,000262 |
| r25_d1000_l20_p65 | quinn | 1 | 0 | – | – | – | – | – | – | – | – | – | – | – | – | 60 |
| r25_d1000_l20_p65 | tcp | 1 | 0 | – | – | – | – | – | – | – | – | – | – | – | – | 20 |

Okuma:
- **r25/d1000/l20, aday (temiz koşu):** 6 mesajın 1'i D içinde.
  - Tek yön p50 74,8 s; alt sınır 6,36 s.
  - Geç kalmada hat %13 boş, %57 PARAM, %28 gereksiz kopya ile dolu. Gecikme fiziksel kapasiteden değil, gönderenin kendi trafiğinden geliyor (B-K13, B-K2, B-K1).
  - quinn: `setup_s` 60,0, yani el sıkışma zaman aşımı. B-K12 ile tutarlı; bu koşu da kirli başladı.
- **r1k/d1000/l20/1 KB:** taban PreConverge'de 60,6 s kaldı; tek yön p50 32,8 s, teslim D içinde 0,95 (B-K8). Aday 3,3 s'de çıktı, tek yön p50 2,2 s. Adayda kuyruk probu 8, gereksiz kopya 7; faydalı pay 0,68 (B-K4 yönünde).
- **r12k/d300/l30/65 B:** iki kolda gönderen kilidi (B-K14). srtt / RTT_fifo 1,01; ölçülen ağ RTT'si 0,64 s; ACK aralığı medyanı 16 (B-K3).
- **r1k/d300/l0/65 B:**
  - Kayıpsızken de 5 gereksiz kopya: 4 kuyruk probu + 1 kuruluş probu.
  - ACK tutması medyanı 0,17–0,26 s, p95 0,54 s; I = 0,45 s (B-K4'ün küçük hâli).
  - Tek yön p50 0,44–0,47 s; RTT_fifo tek yön alt sınırı 0,434 s.
- **Stres l80 (r12k):** taban teslim 0,68, aday 0,76; quinn 0 (el sıkışma 30 s'de bitmedi); tcp 0,02.

### 9.0b Duman-2 [ölçüldü, N = 1, betimsel; araç denetimi]

Kaynak: `/home/ryuzaki/kama-k4g-work/ktp2-f0/runs/duman-2/out`; analiz `reports/ktp2/f0/b/duman-2/`.
- İkililer: taban `3311d8e0…` (`2cd9f68d`), aday `c893e840…` (`bf18dc35`). D birleşmesinden sonraki bench.
- 4 hücre, 12/12 koşu rc = 0; `-j 12`, A/C/D ile eşzamanlı.

**Denetim (hepsi tamam):**
- `kirli_baslangic` = 0 ve koşu başı netem artığı 0 B, 12/12 koşuda.
- `rx_log.jsonl`: 12/12, `dropped` = 0. Kama'da alıcı damgasının tek yön gecikmesi, yakalamanın B-varışıyla aynı (p50 / p95 eşit). tcp'de rx_log, yakalamadan türetilen sıralı teslimle aynı. quinn'de `t0_unix_ns` hizalamasıyla ölçüldü (r12k/l30 p50 0,47 s).
- `ts.csv`: Kama 6/6; `jeton_ac_payi` hesaplandı.
- `snmp_*` (D): rakip 6/6. tcp çekirdek sayacıyla çapraz denetim:
  - OutSegs + RetransSegs, yakalamadaki A çıkışıyla ±3 tutuyor (r12k: 77 + 33 = 110 / 107).
  - Yakalamanın `tcp_retx_segs`'i yalnız veri taşıyan yeniden gönderimleri sayar. SNMP RetransSegs SYN/FIN'i de sayar: r25'te 3 / 9, r1k'de 7 / 10. Fark SYN yeniden gönderimidir (ilk RTO 1 s < RTT).
- `KP-CLIENT-OFFER`: `offer_qfull` 0 ve `unaccepted` 0 bütün Kama koşularında. `QUINN-STATS` ve `TCP-TACK-CLIENT` ayrıştırıldı.
- quinn r25: `setup_s` 60,0, yani el sıkışma zaman aşımı; rx_log boş (B-K12, T-8 yönünde).

**Duman-1 bulgularının tekrarı (temiz kuyrukla, N = 1):**
- B-K13: r25'te iki kolda `param_sends` 25. Tek yön p50 taban 93,5 s, aday 66,8 s, tcp 17,9 s. D içinde teslim Kama 1/6, tcp 2/6.
- B-K14: r12k/d300/l30/65 B'de `kilit_gonderen` = 1 iki kolda (`gonderen_birikim_ms` 2091 / 3739; `btlbw_sunulan` 1,05 / 1,02; `jeton_ac_payi` 0,87 / 0,82). r1k/1 KB'de adayda da (1392 ms).
- r1k/d1000/l20/1 KB tek yön p50: taban 6,75 s, aday 2,59 s, quinn 2,35 s, tcp 7,68 s.

### 9.0c Tam set ara denetimi: P1'den ilk 4 hücre [ölçüldü, N = 30/kol; ızgara eksik, hüküm tam analizde]

Hücreler: `b_r12k_d300_l0_p65`, `b_r12k_d1000_l0_p65`, `b_r12k_d300_l20_p65`, `b_r12k_d300_l0_p1000`. Koşu başına CSV: `reports/ktp2/f0/b/ara-1/`.
- **Geçerlilik:** 480/480 koşu temiz.
  - rc = 0, olaylar ve yakalama var;
  - `kirli_baslangic` = 0, rx_log `dropped` = 0;
  - eksik dosya ve analiz hatası yok.

**İlk kıyas (medyan [Q1–Q3]):**

| tahmin | sonuç |
|---|---|
| T-1, tek yön p50 | tuttu: oran 1,03 (d300/65 B), 1,04 (d300/1 KB), taban 1,28 / aday 1,19 (d1000/65 B) |
| T-1, gönderen teyidi p50 | **tutmadı** (aşağıda) |
| T-1, faydalı pay | tuttu: 65 B'de 0,478–0,483, 1 KB'de 0,924 |
| T-6 | tuttu: r12k/d300/l20/65 B ACK aralığı 16 [16–16] |
| T-7 | tuttu: aday srtt / RTT_fifo 0,98 [0,94–1,02], taban 1,04 [1,03–1,06]; srtt / ölçülen ağ RTT 0,97–1,01. Bu hücrelerde srtt şişmesi yok |
| T-9 | tuttu: l20'de TCP tek yön p95 7,86 s [4,53–12,8], Kama 2,0–2,1 s |
| T-12 | tuttu: kapasite reddi 0 |
| T-15 | **kısmen**: `kilit_gonderen` taban 8/30, aday 15/30. "İki kolda ≥ yarı" tutmadı; çürütme eşiği (< %20) de aşılmadı |
| T-17 | tuttu: kirli başlangıç 0 |

**T-1, gönderen teyidi p50 tutmadı:**

| hücre | taban | aday | tahmin edilen üst sınır (RTT_fifo + I) |
|---|---|---|---|
| r12k/d1000/l0/65 B | 2,85 s | 2,75 s | 2,41 s |
| r12k/d300/l0/65 B | 0,703 s | 0,705 s | 0,698 s (5–7 ms aştı) |

**Yeni gözlem, kayıpsız ve yüksek RTT'de başlangıç birikimi (B-K8'in r12k yüzü):** r12k/d1000/l0/65 B.
- Tek yön p95:

  | taban | aday | quinn | tcp |
  |---|---|---|---|
  | 3,58 s [3,34–3,76] | 3,43 s [2,98–3,81] | 1,45 s [1,42–1,47] | 2,41 s |

- Mesaj başına: koşunun ilk ≈ 13 mesajı tek yön 1,2–1,5 s ile geçiyor; sonrakiler 2,7–3,7 s'ye çıkıyor.
  - Tek yön > 2 s olan mesaj: taban 270/600 [45–476], aday 257/600 [51–339], quinn 0.
  - Geç mesajlar koşunun ilk ≈ 10–11 s'sinde.
- Gönderende bekleme p95: taban 2,36 s, aday 2,20 s. PreConverge çıkışı 2,2 s.
- Geç kalma süresinde hat %70 boş, darboğaz kuyruk fazlası p95 0,22–0,24 s. Gecikme fiziksel kapasiteden değil, gönderen hızından geliyor.
- Açıklama [çıkarım]: ilk RTT içinde IW (2400 B) ≈ 13 DATA + 9 PARAM + 2 kuruluş probunda tükeniyor. Sonra 31 B/s tabanı geçerli (B-K8). Birikim PreConverge çıkışından sonra ≈ 8 s'de eriyor.
- B-K13 bu hücrede de var: d1000'de `param_sends` 9,5 [8–10].

**Diğer gözlemler:**
- r12k/d300/l20/65 B tek yön p50:

  | taban | aday | quinn | tcp |
  |---|---|---|---|
  | 0,54 s | 0,72 s | 0,36 s | 2,37 s |

  p95: Kama 2,0–2,1 s, quinn 1,24 s.
- r12k/d300/l0/1 KB'de kayıpsızken `kilit_gonderen` taban 6/30, aday 2/30.

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
| `scripts/ktp2/f0/cells/b/` | 58 hücre (49 + 9 r25 quinn), öncelik listeleri, duman listesi |
| `reports/ktp2/f0/b/duman-1/` | duman-1 analizi (koşu başına CSV, özet, kaynak) |

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
| b_r25_d1000_l0_p65_quinn | P2 | 5 | 18000 | 6 | 60 | 0.298 | 0.298 | 134 | 5.360 | 2.920/5.480 | 10.280/12.840 | 10.278 | 6.360/6.359 | 6.36 | 1.0000 | 0.485 |
| b_r25_d1000_l20_p65 | P2 | 30 | 18000 | 6 | 60 | 0.298 | 0.372 | 134 | 5.360 | 2.920/5.480 | 10.280/12.840 | 10.278 | 6.360/6.359 | 16.64 | 0.9999 | 0.485 |
| b_r25_d1000_l20_p65_quinn | P2 | 5 | 18000 | 6 | 60 | 0.298 | 0.372 | 134 | 5.360 | 2.920/5.480 | 10.280/12.840 | 10.278 | 6.360/6.359 | 16.64 | 0.9999 | 0.485 |
| b_r25_d300_l0_p65 | P2 | 30 | 18000 | 6 | 60 | 0.298 | 0.298 | 134 | 5.360 | 2.920/5.480 | 8.880/11.440 | 8.879 | 5.660/5.660 | 5.66 | 1.0000 | 0.485 |
| b_r25_d300_l0_p65_quinn | P2 | 5 | 18000 | 6 | 60 | 0.298 | 0.298 | 134 | 5.360 | 2.920/5.480 | 8.880/11.440 | 8.879 | 5.660/5.660 | 5.66 | 1.0000 | 0.485 |
| b_r25_d300_l20_p65 | P2 | 30 | 18000 | 6 | 60 | 0.298 | 0.372 | 134 | 5.360 | 2.920/5.480 | 8.880/11.440 | 8.879 | 5.660/5.660 | 14.54 | 1.0000 | 0.485 |
| b_r25_d300_l20_p65_quinn | P2 | 5 | 18000 | 6 | 60 | 0.298 | 0.372 | 134 | 5.360 | 2.920/5.480 | 8.880/11.440 | 8.879 | 5.660/5.660 | 14.54 | 1.0000 | 0.485 |
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
| b_r25_d1000_l10_p65_quinn | P5 | 5 | 18000 | 6 | 60 | 0.298 | 0.331 | 134 | 5.360 | 2.920/5.480 | 10.280/12.840 | 10.278 | 6.360/6.359 | 16.64 | 1.0000 | 0.485 |
| b_r25_d1000_l30_p65 | P5 | 30 | 18000 | 6 | 60 | 0.298 | 0.425 | 134 | 5.360 | 2.920/5.480 | 10.280/12.840 | 10.278 | 6.360/6.359 | 26.92 | 0.9993 | 0.485 |
| b_r25_d1000_l30_p65_quinn | P5 | 5 | 18000 | 6 | 60 | 0.298 | 0.425 | 134 | 5.360 | 2.920/5.480 | 10.280/12.840 | 10.278 | 6.360/6.359 | 26.92 | 0.9993 | 0.485 |
| b_r25_d300_l10_p65 | P5 | 30 | 18000 | 6 | 60 | 0.298 | 0.331 | 134 | 5.360 | 2.920/5.480 | 8.880/11.440 | 8.879 | 5.660/5.660 | 14.54 | 1.0000 | 0.485 |
| b_r25_d300_l10_p65_quinn | P5 | 5 | 18000 | 6 | 60 | 0.298 | 0.331 | 134 | 5.360 | 2.920/5.480 | 8.880/11.440 | 8.879 | 5.660/5.660 | 14.54 | 1.0000 | 0.485 |
| b_r25_d300_l30_p65 | P5 | 30 | 18000 | 6 | 60 | 0.298 | 0.425 | 134 | 5.360 | 2.920/5.480 | 8.880/11.440 | 8.879 | 5.660/5.660 | 23.42 | 0.9998 | 0.485 |
| b_r25_d300_l30_p65_quinn | P5 | 5 | 18000 | 6 | 60 | 0.298 | 0.425 | 134 | 5.360 | 2.920/5.480 | 8.880/11.440 | 8.879 | 5.660/5.660 | 23.42 | 0.9998 | 0.485 |
| b_r25_d300_l0_p1000 | P6 | 10 | 143000 | 3 | 60 | 0.299 | 0.299 | 1069 | 42.760 | 2.920/5.480 | 46.280/48.840 | 46.279 | 43.060/43.060 | 43.06 | 1.0000 | 0.935 |
| b_r25_d300_l0_p1000_quinn | P6 | 5 | 143000 | 3 | 60 | 0.299 | 0.299 | 1069 | 42.760 | 2.920/5.480 | 46.280/48.840 | 46.279 | 43.060/43.060 | 43.06 | 1.0000 | 0.935 |
<!-- EK-A-BITIR -->

<!-- EK-B-BASLA -->
## Ek B — Hücre başına koşu süresi ve koşu-saati [çıkarım]

Üretim: `python3 -I scripts/ktp2/f0/b/fizik.py butce`. Kol süreleri koşu başına saniyedir (beklenen; "–" = kol o hücrede yok); hücre koşu-saati = N × (hücredeki kolların toplamı) / 3600. tcp birincil kolu erken biter (transport-ack).

| hücre | öncelik | N | taban/aday (s, beklenen) | quinn (s) | tcp (s) | hücre koşu-saati (beklenen) | en kötü (saat) | BUDGET_S |
|---|---|---|---|---|---|---|---|---|
| b_r12k_d1000_l0_p1000 | P1 | 30 | 34 | 43 | 36 | 1.21 | 4.07 | 185 |
| b_r12k_d1000_l0_p65 | P1 | 30 | 27 | 36 | 29 | 0.98 | 2.34 | 118 |
| b_r12k_d1000_l20_p1000 | P1 | 30 | 40 | 49 | 42 | 1.42 | 4.07 | 185 |
| b_r12k_d1000_l20_p65 | P1 | 30 | 33 | 42 | 35 | 1.18 | 2.34 | 118 |
| b_r12k_d1000_l30_p1000 | P1 | 30 | 40 | 49 | 42 | 1.42 | 4.07 | 185 |
| b_r12k_d1000_l30_p65 | P1 | 30 | 33 | 42 | 35 | 1.18 | 2.34 | 118 |
| b_r12k_d300_l0_p1000 | P1 | 30 | 32 | 37 | 33 | 1.12 | 4.07 | 185 |
| b_r12k_d300_l0_p65 | P1 | 30 | 25 | 30 | 26 | 0.89 | 2.34 | 118 |
| b_r12k_d300_l20_p1000 | P1 | 30 | 34 | 39 | 35 | 1.19 | 4.07 | 185 |
| b_r12k_d300_l20_p65 | P1 | 30 | 27 | 32 | 28 | 0.95 | 2.34 | 118 |
| b_r12k_d300_l30_p1000 | P1 | 30 | 34 | 39 | 35 | 1.19 | 4.07 | 185 |
| b_r12k_d300_l30_p65 | P1 | 30 | 27 | 32 | 28 | 0.95 | 2.34 | 118 |
| b_r1k_d1000_l0_p1000 | P1 | 30 | 78 | 91 | 80 | 2.73 | 5.41 | 225 |
| b_r1k_d1000_l0_p65 | P1 | 30 | 50 | 63 | 52 | 1.80 | 3.11 | 141 |
| b_r1k_d1000_l20_p1000 | P1 | 30 | 88 | 101 | 90 | 3.05 | 5.41 | 225 |
| b_r1k_d1000_l20_p65 | P1 | 30 | 57 | 70 | 59 | 2.03 | 3.11 | 141 |
| b_r1k_d1000_l30_p1000 | P1 | 30 | 88 | 101 | 90 | 3.05 | 5.41 | 225 |
| b_r1k_d1000_l30_p65 | P1 | 30 | 57 | 70 | 59 | 2.03 | 3.11 | 141 |
| b_r1k_d300_l0_p1000 | P1 | 30 | 77 | 85 | 77 | 2.64 | 5.41 | 225 |
| b_r1k_d300_l0_p65 | P1 | 30 | 49 | 57 | 50 | 1.70 | 3.11 | 141 |
| b_r1k_d300_l20_p1000 | P1 | 30 | 82 | 91 | 83 | 2.82 | 5.41 | 225 |
| b_r1k_d300_l20_p65 | P1 | 30 | 51 | 60 | 52 | 1.79 | 3.11 | 141 |
| b_r1k_d300_l30_p1000 | P1 | 30 | 82 | 91 | 83 | 2.82 | 5.41 | 225 |
| b_r1k_d300_l30_p65 | P1 | 30 | 51 | 60 | 52 | 1.79 | 3.11 | 141 |
| b_r25_d1000_l0_p65 | P2 | 30 | 121 | – | 129 | 3.10 | 4.33 | 243 |
| b_r25_d1000_l0_p65_quinn | P2 | 5 | – | 66 | – | 0.09 | 0.09 | 96 |
| b_r25_d1000_l20_p65 | P2 | 30 | 153 | – | 161 | 3.89 | 4.33 | 243 |
| b_r25_d1000_l20_p65_quinn | P2 | 5 | – | 66 | – | 0.09 | 0.09 | 96 |
| b_r25_d300_l0_p65 | P2 | 30 | 120 | – | 126 | 3.05 | 4.33 | 243 |
| b_r25_d300_l0_p65_quinn | P2 | 5 | – | 66 | – | 0.09 | 0.09 | 96 |
| b_r25_d300_l20_p65 | P2 | 30 | 153 | – | 160 | 3.88 | 4.33 | 243 |
| b_r25_d300_l20_p65_quinn | P2 | 5 | – | 66 | – | 0.09 | 0.09 | 96 |
| b_r12k_d1000_l10_p1000 | P3 | 30 | 40 | 49 | 42 | 1.42 | 4.07 | 185 |
| b_r12k_d1000_l10_p65 | P3 | 30 | 33 | 42 | 35 | 1.18 | 2.34 | 118 |
| b_r12k_d300_l10_p1000 | P3 | 30 | 34 | 39 | 35 | 1.19 | 4.07 | 185 |
| b_r12k_d300_l10_p65 | P3 | 30 | 27 | 32 | 28 | 0.95 | 2.34 | 118 |
| b_r1k_d1000_l10_p1000 | P3 | 30 | 88 | 101 | 90 | 3.05 | 5.41 | 225 |
| b_r1k_d1000_l10_p65 | P3 | 30 | 57 | 70 | 59 | 2.03 | 3.11 | 141 |
| b_r1k_d300_l10_p1000 | P3 | 30 | 82 | 91 | 83 | 2.82 | 5.41 | 225 |
| b_r1k_d300_l10_p65 | P3 | 30 | 51 | 60 | 52 | 1.79 | 3.11 | 141 |
| b_r12k_d300_l50_p1000 | P4 | 10 | 91 | 96 | 92 | 1.03 | 1.36 | 185 |
| b_r12k_d300_l50_p65 | P4 | 10 | 55 | 60 | 55 | 0.62 | 0.78 | 118 |
| b_r12k_d300_l80_p1000 | P4 | 10 | 91 | 96 | 92 | 1.03 | 1.36 | 185 |
| b_r12k_d300_l80_p65 | P4 | 10 | 55 | 60 | 55 | 0.62 | 0.78 | 118 |
| b_r1k_d300_l50_p1000 | P4 | 10 | 131 | 140 | 132 | 1.49 | 1.80 | 225 |
| b_r1k_d300_l50_p65 | P4 | 10 | 78 | 86 | 78 | 0.89 | 1.04 | 141 |
| b_r1k_d300_l80_p1000 | P4 | 10 | 131 | 140 | 132 | 1.49 | 1.80 | 225 |
| b_r1k_d300_l80_p65 | P4 | 10 | 78 | 86 | 78 | 0.89 | 1.04 | 141 |
| b_r25_d1000_l10_p65 | P5 | 30 | 153 | – | 161 | 3.89 | 4.33 | 243 |
| b_r25_d1000_l10_p65_quinn | P5 | 5 | – | 66 | – | 0.09 | 0.09 | 96 |
| b_r25_d1000_l30_p65 | P5 | 30 | 153 | – | 161 | 3.89 | 4.33 | 243 |
| b_r25_d1000_l30_p65_quinn | P5 | 5 | – | 66 | – | 0.09 | 0.09 | 96 |
| b_r25_d300_l10_p65 | P5 | 30 | 153 | – | 160 | 3.88 | 4.33 | 243 |
| b_r25_d300_l10_p65_quinn | P5 | 5 | – | 66 | – | 0.09 | 0.09 | 96 |
| b_r25_d300_l30_p65 | P5 | 30 | 153 | – | 160 | 3.88 | 4.33 | 243 |
| b_r25_d300_l30_p65_quinn | P5 | 5 | – | 66 | – | 0.09 | 0.09 | 96 |
| b_r25_d300_l0_p1000 | P6 | 10 | 349 | – | 356 | 2.93 | 3.08 | 439 |
| b_r25_d300_l0_p1000_quinn | P6 | 5 | – | 66 | – | 0.09 | 0.09 | 96 |

P1: beklenen 41.9 sa (kümülatif 41.9) · en kötü 89.5 sa (kümülatif 89.5)
P2: beklenen 14.3 sa (kümülatif 56.2) · en kötü 17.7 sa (kümülatif 107.2)
P3: beklenen 14.4 sa (kümülatif 70.7) · en kötü 29.8 sa (kümülatif 137.1)
P4: beklenen 8.0 sa (kümülatif 78.7) · en kötü 9.9 sa (kümülatif 147.0)
P5: beklenen 15.9 sa (kümülatif 94.6) · en kötü 17.7 sa (kümülatif 164.7)
P6: beklenen 3.0 sa (kümülatif 97.6) · en kötü 3.2 sa (kümülatif 167.8)
<!-- EK-B-BITIR -->
