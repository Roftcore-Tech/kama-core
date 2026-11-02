# KTP2-F0 / Ajan B — Düşük bant ve yüksek RTT: rapor taslağı

**TAM SONUÇ bu dosyanın sonundadır (`# TAM SONUÇ`, TS.1–TS.7); hüküm yalnız oradadır.** §0–§9.0c tur notlarıdır ve geçmiş kayıt olarak değiştirilmedi. §4'ün "beklenen" sayıları ile ölçüm arasındaki sapmalar TS.3.1'de.

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

### 9.1 Teslim ve gecikme → TS.2.1
### 9.2 Hat kapasitesinin faydalı veriye giden payı (ileri / dönüş, sınıf sütunları) → TS.2.2
### 9.3 srtt ↔ gerçek RTT (taban / aday) → TS.2.3
### 9.4 Yeniden gönderim: faydalı / gereksiz → TS.2.4
### 9.5 Fiziksel kapasite ↔ zamanlayıcı → TS.2.2
### 9.6 Tahminlerle kıyas (T-1…T-17) → TS.3.1

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

---

# TAM SONUÇ

Kök: `~/kama-k4g-work/ktp2-f0/runs/tam/b/out/` (56 hücre, 4920 koşu). İkililer: bench taban-2cd9f68d (`3311d8e0…`), aday-bf18dc35 (`c893e840…`). Satır numaraları origin'deki taban `a858f3fc`'ye göredir; adayda (`49864510`) farklıysa "/ aday" ile verilir. `adaptive-transport` ve `client-core/src/runtime.rs` kaynakları ölçülen ikililerle bu iki SHA'da aynıdır.

Bütün sayılar `reports/ktp2/f0/b/tam/TAM-OTOMATIK.md` çıktısındandır (`tam_b.py`; aşağıda "T§n"). Etiketler: [ölçüldü] · [kod] · [türetilmiş] · [çıkarım] · [betimsel] · [tahmin]. [tahmin] beklenen etkidir, sonuç değildir.

Kısaltma: bir satırdaki `x / y` = taban / aday. Hücre adı `r12k_d300_l30_p65` = 12,5 kB/s, 300 ms tek yön, %30 iki yönlü kayıp, 65 B. "Havuz" birden çok hücrenin toplamıdır ve hücre adı değildir.

## TS.1 Geçerlilik

**Toplam [T§1]:** N = 4920; rc = 0 4920; kirli başlangıç 0; alıcı kayıt düşüşü 0; eksik dosya 0. El sıkışamayan koşu 85 (tümü quinn / tcp; Kama kolunda ayrı el sıkışma yok, D-9). Yeniden koşu yok.

| hücre | N | rc ≠ 0 | el sıkışma yok quinn · tcp | ileri kayıp | dönüş kayıp |
|---|---|---|---|---|---|
| r25_d300_l0_p65 | 30 · 30 · 30 · 5 | 0 | 5 · 0 | 0,0 (0,0–0,0) | 0,0 (0,0–0,0) |
| r25_d300_l10_p65 | 30 · 30 · 30 · 5 | 0 | 5 · 0 | 14,6 (2,8–23,9) | 9,0 (2,4–25,0) |
| r25_d300_l20_p65 | 30 · 30 · 30 · 5 | 0 | 5 · 0 | 24,7 (14,6–41,4) | 20,8 (4,7–35,7) |
| r25_d300_l30_p65 | 30 · 30 · 30 · 5 | 0 | 5 · 0 | 32,6 (17,1–45,9) | 30,4 (15,8–46,3) |
| r25_d1000_l0_p65 | 30 · 30 · 30 · 5 | 0 | 5 · 0 | 0,0 (0,0–0,0) | 0,0 (0,0–0,0) |
| r25_d1000_l10_p65 | 30 · 30 · 30 · 5 | 0 | 5 · 0 | 12,5 (0,0–20,9) | 9,3 (0,0–19,0) |
| r25_d1000_l20_p65 | 30 · 30 · 30 · 5 | 0 | 5 · 0 | 24,2 (8,1–39,3) | 20,7 (7,0–40,0) |
| r25_d1000_l30_p65 | 30 · 30 · 30 · 5 | 0 | 5 · 0 | 34,0 (20,9–43,1) | 30,1 (14,0–53,8) |
| r1k_d300_l0_p65 | 30 · 30 · 30 · 30 | 0 | 0 · 0 | 0,0 (0,0–0,0) | 0,0 (0,0–0,0) |
| r1k_d300_l0_p1000 | 30 · 30 · 30 · 30 | 0 | 0 · 0 | 0,0 (0,0–0,0) | 0,0 (0,0–0,0) |
| r1k_d300_l10_p65 | 30 · 30 · 30 · 30 | 0 | 0 · 0 | 10,0 (4,8–14,5) | 9,2 (3,0–18,8) |
| r1k_d300_l10_p1000 | 30 · 30 · 30 · 30 | 0 | 0 · 0 | 10,0 (2,6–26,8) | 10,2 (0,0–26,9) |
| r1k_d300_l20_p65 | 30 · 30 · 30 · 30 | 0 | 0 · 0 | 20,1 (10,5–30,3) | 20,1 (9,1–30,9) |
| r1k_d300_l20_p1000 | 30 · 30 · 30 · 30 | 0 | 0 · 0 | 21,6 (7,9–35,4) | 20,0 (3,4–43,5) |
| r1k_d300_l30_p65 | 30 · 30 · 30 · 30 | 0 | 0 · 0 | 29,9 (20,3–40,7) | 31,0 (16,1–42,4) |
| r1k_d300_l30_p1000 | 30 · 30 · 30 · 30 | 0 | 0 · 0 | 30,0 (16,3–44,7) | 28,0 (12,5–48,0) |
| r1k_d300_l50_p65 † | 10 · 10 · 10 · 10 | 0 | 3 · 0 | 51,5 (45,4–54,5) | 49,4 (32,9–60,3) |
| r1k_d300_l50_p1000 † | 10 · 10 · 10 · 10 | 0 | 0 · 0 | 51,6 (41,0–63,6) | 46,7 (33,3–68,3) |
| r1k_d300_l80_p65 † | 10 · 10 · 10 · 10 | 0 | 8 · 4 | 79,2 (77,1–83,6) | 81,7 (73,1–86,8) |
| r1k_d300_l80_p1000 † | 10 · 10 · 10 · 10 | 0 | 3 · 4 | 80,9 (72,4–96,7) | 78,9 (0,0–100) |
| r1k_d1000_l0_p65 | 30 · 30 · 30 · 30 | 0 | 0 · 0 | 0,0 (0,0–0,0) | 0,0 (0,0–0,0) |
| r1k_d1000_l0_p1000 | 30 · 30 · 30 · 30 | 0 | 0 · 0 | 0,0 (0,0–0,0) | 0,0 (0,0–0,0) |
| r1k_d1000_l10_p65 | 30 · 30 · 30 · 30 | 0 | 0 · 0 | 10,2 (4,0–15,7) | 9,7 (3,6–16,3) |
| r1k_d1000_l10_p1000 | 30 · 30 · 30 · 30 | 0 | 0 · 0 | 10,6 (2,3–20,0) | 9,9 (0,0–24,1) |
| r1k_d1000_l20_p65 | 30 · 30 · 30 · 30 | 0 | 0 · 0 | 20,8 (12,6–29,4) | 20,0 (12,4–31,8) |
| r1k_d1000_l20_p1000 | 30 · 30 · 30 · 30 | 0 | 0 · 0 | 20,9 (5,3–33,3) | 20,6 (9,1–42,9) |
| r1k_d1000_l30_p65 | 30 · 30 · 30 · 30 | 0 | 0 · 0 | 29,8 (21,1–36,3) | 30,7 (20,7–41,1) |
| r1k_d1000_l30_p1000 | 30 · 30 · 30 · 30 | 0 | 0 · 0 | 30,4 (15,0–58,3) | 30,6 (15,4–46,9) |
| r12k_d300_l0_p65 | 30 · 30 · 30 · 30 | 0 | 0 · 0 | 0,0 (0,0–0,0) | 0,0 (0,0–0,0) |
| r12k_d300_l0_p1000 | 30 · 30 · 30 · 30 | 0 | 0 · 0 | 0,0 (0,0–0,0) | 0,0 (0,0–0,0) |
| r12k_d300_l10_p65 | 30 · 30 · 30 · 30 | 0 | 0 · 0 | 10,1 (7,2–11,9) | 10,0 (7,3–12,6) |
| r12k_d300_l10_p1000 | 30 · 30 · 30 · 30 | 0 | 0 · 0 | 9,8 (4,3–17,2) | 10,3 (3,3–18,2) |
| r12k_d300_l20_p65 | 30 · 30 · 30 · 30 | 0 | 0 · 0 | 19,9 (16,7–24,7) | 19,6 (16,9–24,3) |
| r12k_d300_l20_p1000 | 30 · 30 · 30 · 30 | 0 | 0 · 0 | 19,5 (12,9–47,4) | 20,5 (10,8–30,0) |
| r12k_d300_l30_p65 | 30 · 30 · 30 · 30 | 0 | 0 · 0 | 29,7 (26,1–34,3) | 30,2 (26,1–34,1) |
| r12k_d300_l30_p1000 | 30 · 30 · 30 · 30 | 0 | 0 · 0 | 30,1 (23,2–38,6) | 29,4 (18,7–40,9) |
| r12k_d300_l50_p65 † | 10 · 10 · 10 · 10 | 0 | 0 · 0 | 49,6 (47,7–51,8) | 50,2 (47,2–53,4) |
| r12k_d300_l50_p1000 † | 10 · 10 · 10 · 10 | 0 | 1 · 0 | 48,9 (42,3–66,7) | 49,4 (42,9–60,7) |
| r12k_d300_l80_p65 † | 10 · 10 · 10 · 10 | 0 | 9 · 3 | 79,9 (76,2–82,2) | 79,8 (73,7–89,0) |
| r12k_d300_l80_p1000 † | 10 · 10 · 10 · 10 | 0 | 4 · 4 | 79,9 (74,4–87,9) | 83,3 (64,7–94,1) |
| r12k_d1000_l0_p65 | 30 · 30 · 30 · 30 | 0 | 0 · 0 | 0,0 (0,0–0,0) | 0,0 (0,0–0,0) |
| r12k_d1000_l0_p1000 | 30 · 30 · 30 · 30 | 0 | 0 · 0 | 0,0 (0,0–0,0) | 0,0 (0,0–0,0) |
| r12k_d1000_l10_p65 | 30 · 30 · 30 · 30 | 0 | 0 · 0 | 9,5 (7,3–12,0) | 10,1 (7,6–12,1) |
| r12k_d1000_l10_p1000 | 30 · 30 · 30 · 30 | 0 | 0 · 0 | 10,0 (5,9–14,0) | 10,0 (3,8–17,4) |
| r12k_d1000_l20_p65 | 30 · 30 · 30 · 30 | 0 | 0 · 0 | 20,4 (16,5–23,0) | 20,2 (16,5–23,1) |
| r12k_d1000_l20_p1000 | 30 · 30 · 30 · 30 | 0 | 0 · 0 | 20,9 (10,3–27,3) | 19,9 (12,6–29,1) |
| r12k_d1000_l30_p65 | 30 · 30 · 30 · 30 | 0 | 2 · 0 | 29,6 (26,8–33,3) | 29,6 (25,6–34,0) |
| r12k_d1000_l30_p1000 | 30 · 30 · 30 · 30 | 0 | 0 · 0 | 30,4 (22,7–40,2) | 30,2 (19,3–41,0) |

**Okuma notları:**
- Kayıp, Kama kollarının netem sayacıdır. tcp ve quinn dahil kol başına satırlar T§1'de. tcp'de `kayıp` skb/segment karışıktır (D-13).
- r25'te kayıp aralığı geniştir ve medyan nominalin üstünde kalabilir (l10: 12,5–14,6). Koşuda ≈ 60 paket var. Koşu bittiğinde kuyrukta kalan paket "geçen" sayılmıyor, düşen ise kuyruğa girişte sayılıyor [çıkarım]. Hücre kaybı olarak nominal değer, koşu kaybı olarak T§1 kullanılır.
- Yakalama / qdisc oranı: hücre medyanı 0,97–1,00; 128/2560 Kama koşusunda < 0,95. Bunlar kısa koşularda birkaç paketlik farktır (örnek 108 / 114 paket): yakalama qdisc'ten önce bitiyor [T§1]. Stres hücrelerinde en düşük değerler 3–9 paketlik koşulardan gelir.
- r25 quinn N = 5 (Ajan D kararı), l50 / l80 N = 10: betimsel; hüküm cümlesine girmez.

**Ölçülemedi:**
- **`b_r25_d300_l0_p1000` ve `_quinn`:** koşulmadı (P6, bütçe dışı). 25 B/s'de 1 KB mesaj bu şeritte ölçülemedi.
- **Zamanlayıcı ret sayaçları (`data_denied`, `retx_denied`):** ikili `diag-sched-telemetry` olmadan derlendi; kosular_b.csv'de boş, ts.csv'de 0. Jeton yokluğu yalnız `jeton_ac_payi` ile dolaylı okunur.
- **Durum ve BtlBw zaman serisi B ızgarasında yok:** ts.csv yalnız FEC tanı sütunları taşır. A-10 durum payları Ajan A'nın ürün yolu `alice.jsonl` serisinden; B'de yalnız `pending_lost` ve yakalamadan emisyon ölçüldü.
- **Kopyanın türü telde ayrılmaz:** ARQ / kuyruk probu / kuruluş probu / geometrik kopya aynı DATA görünür. Tür ayrımı sayaç düzlemindedir; hattan geçen baytta yalnız "kopya" var.
- **IW patlama yükü:** ızgarada patlama yok (ρ = 0,3, düzenli aralık).
- **Bozuk dönüş yolu ve 3,125 B/s:** bu ızgarada hücre yok (K-5 Ajan D; K-3 stresi hiç koşulmadı).
- **Onarım (FEC) maliyeti:** parite koşuların çoğunda 0 (T§18). B-K5'in tahmini sınanamadı.

## TS.2 Sonuç tabloları

**Yöntem.** Birim = koşu. Hücre içi medyan [Q1–Q3] (min–maks); min–maks ve bütün ölçüler T§2.1'de. Taban–aday farkı = medyan(aday) − medyan(taban), %95 yüzdelik bootstrap, koşu birimli, her kol kendi içinde yerine koyarak, B = 2000, tohum sha256(hücre|ölçü). Oranlar için Fisher kesin (iki yanlı). Çoklu karşılaştırma düzeltmesi yok: T§2.2'de 238 karşılaştırmada %5 ile ≈ 12 yanlış pozitif beklenir, 61 aralık 0'ı dışlıyor. Kilit tablosunda 48 hücre + 12 havuz testi var; Bonferroni eşiği ≈ 0,0008.

### TS.2.1 Teslim ve gecikme [ölçüldü]

Alıcı damgası (`rx_log`), dört kolda aynı şema. D = 60 s (1 KB ve r25) ya da 30 s (r1k / r12k 65 B). † betimsel.

| hücre | N | D taban | D aday | D tcp | D quinn | p95 taban | p95 aday | p95 tcp | p95 quinn |
|---|---|---|---|---|---|---|---|---|---|
| r25_d300_l0_p65 | 30 | 0,167 | 0,167 | 1,000 | – † | 118 [118–118] | 118 [118–118] | 26,9 [26,8–26,9] | – |
| r25_d300_l10_p65 | 30 | 0,167 | 0,167 | 1,000 | – † | 109 [106–114] | 112 [106–118] | 26,8 [23,8–31,4] | – |
| r25_d300_l20_p65 | 30 | 0,167 | 0,167 | 1,000 | – † | 98,0 [93,4–103] | 98,2 [91,3–104] | 29,9 [20,8–49,7] | – |
| r25_d300_l30_p65 | 30 | 0,167 | 0,417 | 1,000 | – † | 82,1 [75,4–89,4] | 89,6 [78,6–95,4] | 30,6 [22,6–33,5] | – |
| r25_d1000_l0_p65 | 30 | 0,167 | 0,167 | 1,000 | – † | 118 [118–119] | 118 [118–118] | 29,7 [29,5–29,8] | – |
| r25_d1000_l10_p65 | 30 | 0,167 | 0,167 | 1,000 | – † | 111 [108–114] | 112 [106–114] | 28,1 [25,0–33,5] | – |
| r25_d1000_l20_p65 | 30 | 0,167 | 0,167 | 1,000 | – † | 97,8 [94,1–105] | 97,0 [94,1–108] | 36,2 [22,9–50,3] | – |
| r25_d1000_l30_p65 | 30 | 0,167 | 0,500 | 1,000 | – † | 87,5 [77,8–93,1] | 85,7 [80,1–91,2] | 46,5 [26,9–53,0] | – |
| r1k_d300_l0_p65 | 30 | 1,000 | 1,000 | 1,000 | 1,000 | 0,596 [0,546–0,681] | 0,580 [0,552–0,834] | 0,539 [0,533–0,547] | 3,473 [3,425–3,541] |
| r1k_d300_l0_p1000 | 30 | 1,000 | 1,000 | 1,000 | 1,000 | 2,036 [1,480–18,3] | 2,026 [1,744–2,347] | 1,476 [1,462–1,493] | 3,648 [3,593–3,683] |
| r1k_d300_l10_p65 | 30 | 1,000 | 1,000 | 1,000 | 1,000 | 2,959 [2,565–3,438] | 2,705 [2,543–3,123] | 1,988 [1,811–2,701] | 3,271 [2,099–3,798] |
| r1k_d300_l10_p1000 | 30 | 1,000 | 1,000 | 1,000 | 1,000 | 6,910 [2,598–18,3] | 3,964 [2,950–7,655] | 4,119 [2,324–4,822] | 6,696 [5,413–6,959] |
| r1k_d300_l20_p65 | 30 | 1,000 | 1,000 | 1,000 | 1,000 | 4,481 [3,453–5,454] | 4,701 [3,392–6,453] | 6,282 [4,399–10,3] | 3,333 [2,973–4,444] |
| r1k_d300_l20_p1000 | 30 | 1,000 | 1,000 | 1,000 | 1,000 | 20,2 [13,1–26,5] | 8,625 [6,898–11,9] | 10,3 [5,447–18,9] | 10,7 [7,368–14,9] |
| r1k_d300_l30_p65 | 30 | 1,000 | 1,000 | 0,910 | 1,000 | 6,762 [5,158–9,128] | 6,232 [5,545–12,9] | 19,1 [11,3–33,2] | 5,691 [4,854–37,6] |
| r1k_d300_l30_p1000 | 30 | 1,000 | 1,000 | 0,875 | 1,000 | 23,6 [16,7–47,3] | 14,3 [10,8–18,1] | 29,4 [15,7–43,5] | 25,3 [15,2–45,2] |
| r1k_d300_l50_p65 † | 10 | 0,890 | 0,990 | 0,055 | 0,940 | 34,4 [18,7–42,2] | 13,5 [10,5–23,9] | 31,9 [12,9–36,8] | 8,103 [7,043–9,276] |
| r1k_d300_l50_p1000 † | 10 | 0,925 | 1,000 | 0,175 | 0,950 | 61,3 [49,6–73,5] | 33,5 [28,9–43,5] | 33,8 [14,7–58,6] | 38,2 [30,2–53,8] |
| r1k_d300_l80_p65 † | 10 | 0,670 | 0,710 | 0,000 | 0,000 | 36,2 [30,9–41,1] | 36,7 [31,8–44,7] | 20,4 [14,1–31,9] | – |
| r1k_d300_l80_p1000 † | 10 | 0,225 | 0,400 | 0,050 | 0,000 | 74,1 [54,9–83,8] | 80,5 [66,6–81,8] | 1,406 [1,378–1,443] | 1,371 [1,317–1,426] |
| r1k_d1000_l0_p65 | 30 | 1,000 | 1,000 | 1,000 | 1,000 | 1,485 [1,451–1,498] | 1,480 [1,457–1,514] | 1,490 [1,462–1,501] | 2,790 [2,685–2,918] |
| r1k_d1000_l0_p1000 | 30 | 1,000 | 1,000 | 1,000 | 1,000 | 3,582 [2,569–19,9] | 2,905 [2,538–3,136] | 2,375 [2,338–2,412] | 2,974 [2,867–3,090] |
| r1k_d1000_l10_p65 | 30 | 1,000 | 1,000 | 1,000 | 1,000 | 5,092 [4,800–5,338] | 4,965 [4,622–5,656] | 4,898 [4,485–5,594] | 4,252 [4,003–4,386] |
| r1k_d1000_l10_p1000 | 30 | 1,000 | 1,000 | 1,000 | 1,000 | 11,1 [9,567–20,3] | 7,540 [3,603–10,0] | 11,9 [9,099–13,8] | 9,203 [5,932–12,2] |
| r1k_d1000_l20_p65 | 30 | 1,000 | 1,000 | 1,000 | 1,000 | 7,555 [6,203–9,644] | 7,519 [6,188–8,835] | 10,2 [9,095–13,9] | 5,463 [5,062–6,755] |
| r1k_d1000_l20_p1000 | 30 | 1,000 | 1,000 | 1,000 | 1,000 | 19,2 [12,6–22,2] | 12,6 [10,7–16,0] | 13,6 [12,2–21,6] | 14,6 [9,859–51,4] |
| r1k_d1000_l30_p65 | 30 | 1,000 | 1,000 | 0,855 | 1,000 | 9,981 [8,844–13,2] | 10,0 [8,792–13,1] | 17,3 [12,6–22,3] | 8,424 [6,948–10,3] |
| r1k_d1000_l30_p1000 | 30 | 1,000 | 1,000 | 0,750 | 1,000 | 26,1 [19,5–36,8] | 17,7 [13,5–23,2] | 36,1 [22,6–49,8] | 21,6 [17,1–48,7] |
| r12k_d300_l0_p65 | 30 | 1,000 | 1,000 | 1,000 | 1,000 | 0,678 [0,477–1,084] | 0,667 [0,434–0,782] | 0,429 [0,423–0,431] | 0,425 [0,421–0,429] |
| r12k_d300_l0_p1000 | 30 | 1,000 | 1,000 | 1,000 | 1,000 | 0,644 [0,512–1,721] | 0,553 [0,509–0,704] | 0,482 [0,476–0,491] | 0,485 [0,479–0,496] |
| r12k_d300_l10_p65 | 30 | 1,000 | 1,000 | 1,000 | 1,000 | 1,278 [1,208–1,448] | 1,269 [1,210–1,421] | 1,737 [1,610–2,141] | 1,109 [1,098–1,123] |
| r12k_d300_l10_p1000 | 30 | 1,000 | 1,000 | 1,000 | 1,000 | 2,281 [2,053–2,537] | 2,249 [2,145–2,520] | 1,523 [1,415–2,299] | 1,410 [1,366–1,449] |
| r12k_d300_l20_p65 | 30 | 1,000 | 1,000 | 1,000 | 1,000 | 2,107 [1,766–2,898] | 1,993 [1,710–2,788] | 7,856 [4,528–12,8] | 1,242 [1,200–1,789] |
| r12k_d300_l20_p1000 | 30 | 1,000 | 1,000 | 1,000 | 1,000 | 3,255 [2,705–6,108] | 3,595 [2,853–3,990] | 12,1 [4,389–25,0] | 2,279 [1,893–3,281] |
| r12k_d300_l30_p65 | 30 | 1,000 | 1,000 | 0,873 | 1,000 | 3,948 [2,608–5,712] | 3,776 [3,148–4,655] | 16,8 [11,4–25,6] | 1,915 [1,888–1,951] |
| r12k_d300_l30_p1000 | 30 | 1,000 | 1,000 | 0,605 | 1,000 | 8,961 [5,558–15,8] | 4,730 [4,164–5,662] | 34,7 [21,7–56,5] | 3,234 [2,841–4,108] |
| r12k_d300_l50_p65 † | 10 | 0,999 | 0,998 | 0,089 | 1,000 | 13,2 [12,1–16,4] | 13,8 [10,2–14,9] | 10,0 [4,611–29,9] | 4,085 [3,717–7,353] |
| r12k_d300_l50_p1000 † | 10 | 0,535 | 0,995 | 0,085 | 1,000 | 61,9 [44,8–72,5] | 32,1 [17,7–44,5] | 28,7 [16,2–40,0] | 9,391 [9,026–9,810] |
| r12k_d300_l80_p65 † | 10 | 0,617 | 0,804 | 0,000 | 0,645 | 36,4 [30,6–37,7] | 29,0 [27,5–36,1] | 0,317 [0,316–14,2] | 17,3 [17,3–17,3] |
| r12k_d300_l80_p1000 † | 10 | 0,035 | 0,460 | 0,000 | 0,120 | 54,8 [46,6–58,7] | 62,9 [56,5–67,8] | 67,6 [62,1–73,2] | 27,3 [25,1–43,6] |
| r12k_d1000_l0_p65 | 30 | 1,000 | 1,000 | 1,000 | 1,000 | 3,582 [3,344–3,758] | 3,432 [2,983–3,811] | 2,408 [2,126–2,621] | 1,447 [1,422–1,468] |
| r12k_d1000_l0_p1000 | 30 | 1,000 | 1,000 | 1,000 | 1,000 | 4,546 [3,991–5,104] | 4,954 [3,800–5,439] | 1,421 [1,397–1,439] | 1,409 [1,385–1,440] |
| r12k_d1000_l10_p65 | 30 | 1,000 | 1,000 | 1,000 | 1,000 | 4,795 [4,431–5,160] | 4,864 [4,382–5,665] | 5,335 [4,803–6,162] | 3,752 [3,678–3,791] |
| r12k_d1000_l10_p1000 | 30 | 1,000 | 1,000 | 1,000 | 1,000 | 6,455 [4,776–8,048] | 6,018 [4,357–8,501] | 5,960 [4,807–9,145] | 3,719 [3,607–3,871] |
| r12k_d1000_l20_p65 | 30 | 1,000 | 1,000 | 1,000 | 1,000 | 6,870 [6,557–7,532] | 6,507 [6,213–7,293] | 9,882 [7,703–18,3] | 4,037 [3,987–4,364] |
| r12k_d1000_l20_p1000 | 30 | 1,000 | 1,000 | 1,000 | 1,000 | 15,1 [9,165–33,3] | 11,3 [8,785–13,6] | 15,7 [9,451–31,8] | 5,029 [4,396–6,507] |
| r12k_d1000_l30_p65 | 30 | 1,000 | 1,000 | 0,466 | 1,000 | 12,2 [10,2–13,3] | 10,6 [9,556–11,8] | 17,7 [11,7–28,8] | 6,410 [6,221–15,1] |
| r12k_d1000_l30_p1000 | 30 | 1,000 | 1,000 | 0,465 | 1,000 | 31,3 [24,0–53,9] | 24,6 [15,6–42,8] | 50,1 [29,8–61,7] | 7,157 [6,573–12,5] |

**Taban–aday [T§2.2, T§2.3]:**

| ölçü | n | aday büyük | aday küçük |
|---|---|---|---|
| alıcı teslim D | 40 | 2 | 0 |
| alıcı p50 s | 40 | 3 | 5 |
| alıcı p95 s | 40 | 1 | 5 |
| ileri faydalı pay | 40 | 5 | 6 |
| srtt/RTT_fifo | 38 | 0 | 23 |
| gereksiz kopya baytı | 40 | 3 | 8 |

Alıcı p95 ve alıcı teslim D'de aralığı 0'ı dışlayan hücreler:

| hücre | ölçü | aday − taban | %95 aralık |
|---|---|---|---|
| r25_d300_l30_p65 | alıcı teslim D | 0,250 | [0,167; 0,333] |
| r25_d300_l30_p65 | alıcı p95 s | 7,509 | [0,291; 12,5] |
| r25_d1000_l30_p65 | alıcı teslim D | 0,333 | [0,167; 0,333] |
| r1k_d300_l20_p1000 | alıcı p95 s | -11,6 | [-16,9; -8,178] |
| r1k_d300_l30_p1000 | alıcı p95 s | -9,323 | [-21,9; -4,969] |
| r1k_d1000_l10_p1000 | alıcı p95 s | -3,604 | [-10,0; -1,590] |
| r1k_d1000_l30_p1000 | alıcı p95 s | -8,417 | [-12,4; -3,493] |
| r12k_d300_l30_p1000 | alıcı p95 s | -4,231 | [-7,771; -1,236] |

- r1k ve r12k hüküm hücrelerinde alıcı teslim D medyanı iki kolda 1,000 (32/32 hücre). Bu grupta fark gösterilmedi.
- Aday farkı 1 KB kayıplı hücrelerde p95'te kısalma, r25 l30'da teslim artışıdır. r25_d300_l30_p65 p95 +7,5 [+0,3; +12,5] sınırdadır; aday daha çok mesaj teslim ettiği için p95 daha geç mesajları da içerir.
- 65 B hücrelerinde alıcı p95'te fark gösterilmedi (aralık 0'ı kapsıyor).

**Kama ↔ rakip [T§15]** (Kama − rakip; hücre tablosu orada). Bench Kama kolu üst sınırdır (D-9). tcp D-13 sınırlamasını taşır.
- aday − quinn alıcı p95: 32 hücrede Kama uzun 18, Kama kısa 6, fark gösterilmedi 8.
- taban − quinn alıcı p95: 32 hücrede Kama uzun 21, Kama kısa 2, fark gösterilmedi 9.
- aday − tcp alıcı p95: 40 hücrede Kama uzun 17 (8'i r25), Kama kısa 14, fark gösterilmedi 9.
- aday − tcp teslim D: 40 hücrede Kama yüksek 4, Kama düşük 8 (8'i de r25).
- Teslim D'de aday − quinn: 32 hücrede aralık 0'ı dışlayan yok. Medyan iki kolda 1,000.

### TS.2.2 S2: hat kapasitesinin payları [ölçüldü]

İleri yön, hattan geçen bayt (L2 dahil) üzerinden %. Hat, pencere = ilk sunum → son paket. Her sütun taban / aday. r25'te koşu sonunda kuyrukta paket kalır; paylar pencere içindir. Kol başına tam tablo (tcp, quinn dahil) T§3.

| hücre | doluluk % | faydalı/geçen | faydalı/kapasite | L2-4 | başlık | gereksiz yük | onarım | kontrol | ACK ileri | kopya F tel | kopya G tel |
|---|---|---|---|---|---|---|---|---|---|---|---|
| r25_d300_l0_p65 | 97,8 / 98,0 | 3,5 / 3,5 | 3,5 / 3,5 | 40,1 / 40,1 | 18,3 / 18,3 | 14,2 / 14,2 | 0,0 / 0,0 | 23,9 / 23,9 | 0,0 / 0,0 | 0,0 / 0,0 | 29,2 / 29,2 |
| r25_d300_l10_p65 | 98,0 / 98,0 | 5,2 / 5,4 | 5,2 / 5,2 | 39,3 / 39,7 | 18,5 / 18,4 | 16,0 / 14,0 | 0,0 / 0,0 | 21,5 / 22,7 | 0,0 / 0,0 | 0,0 / 0,0 | 33,0 / 28,9 |
| r25_d300_l20_p65 | 97,1 / 97,3 | 5,5 / 8,4 | 5,3 / 7,9 | 39,1 / 38,6 | 19,1 / 18,6 | 16,6 / 15,4 | 0,0 / 0,0 | 19,4 / 19,9 | 0,0 / 0,0 | 3,6 / 0,0 | 36,8 / 31,8 |
| r25_d300_l30_p65 | 96,2 / 94,9 | 8,9 / 10,7 | 8,7 / 10,4 | 38,3 / 38,0 | 19,4 / 18,7 | 17,0 / 14,6 | 0,0 / 0,0 | 16,0 / 18,1 | 0,0 / 0,0 | 3,8 / 3,9 | 38,0 / 30,2 |
| r25_d1000_l0_p65 | 98,7 / 98,6 | 3,5 / 3,5 | 3,5 / 3,5 | 40,1 / 40,1 | 18,3 / 18,3 | 14,2 / 14,2 | 0,0 / 0,0 | 23,9 / 23,9 | 0,0 / 0,0 | 0,0 / 0,0 | 29,2 / 29,2 |
| r25_d1000_l10_p65 | 97,5 / 97,7 | 3,6 / 5,4 | 3,5 / 5,2 | 39,5 / 39,4 | 18,4 / 18,4 | 16,1 / 14,3 | 0,0 / 0,0 | 22,3 / 22,1 | 0,0 / 0,0 | 0,0 / 0,0 | 33,4 / 29,5 |
| r25_d1000_l20_p65 | 96,8 / 97,6 | 7,0 / 7,2 | 6,9 / 7,0 | 38,8 / 38,6 | 18,8 / 18,6 | 16,4 / 15,2 | 0,0 / 0,0 | 19,1 / 19,9 | 0,0 / 0,0 | 0,0 / 0,0 | 36,7 / 31,4 |
| r25_d1000_l30_p65 | 96,7 / 94,0 | 7,5 / 10,6 | 7,0 / 8,7 | 38,4 / 38,0 | 19,5 / 18,7 | 17,5 / 15,2 | 0,0 / 0,0 | 17,0 / 18,3 | 0,0 / 0,0 | 3,8 / 4,0 | 38,2 / 31,4 |
| r1k_d300_l0_p65 | 31,3 / 31,4 | 45,0 / 44,7 | 14,2 / 14,1 | 31,7 / 31,7 | 20,1 / 20,1 | 2,2 / 2,3 | 0,0 / 0,0 | 1,0 / 1,0 | 0,0 / 0,0 | 0,0 / 0,0 | 4,6 / 4,6 |
| r1k_d300_l0_p1000 | 49,9 / 49,6 | 56,0 / 56,2 | 28,1 / 27,8 | 4,8 / 4,7 | 2,8 / 2,8 | 35,0 / 36,0 | 0,0 / 0,0 | 0,7 / 0,7 | 0,0 / 0,0 | 0,0 / 0,0 | 37,6 / 38,5 |
| r1k_d300_l10_p65 | 30,6 / 30,6 | 45,5 / 45,4 | 13,9 / 13,9 | 31,7 / 31,7 | 20,1 / 20,1 | 1,8 / 1,8 | 0,0 / 0,0 | 1,0 / 1,0 | 0,0 / 0,0 | 8,8 / 8,4 | 3,8 / 3,7 |
| r1k_d300_l10_p1000 | 42,7 / 44,8 | 64,3 / 60,8 | 27,7 / 27,8 | 4,7 / 4,4 | 2,9 / 2,7 | 27,2 / 30,8 | 0,0 / 0,0 | 0,6 / 0,5 | 0,0 / 0,0 | 6,2 / 6,4 | 29,4 / 33,0 |
| r1k_d300_l20_p65 | 28,6 / 29,5 | 45,9 / 45,7 | 13,0 / 13,2 | 31,8 / 31,6 | 20,1 / 20,1 | 1,4 / 1,8 | 0,0 / 0,0 | 1,0 / 0,8 | 0,0 / 0,0 | 18,2 / 19,0 | 3,3 / 3,8 |
| r1k_d300_l20_p1000 | 34,6 / 41,4 | 73,7 / 63,7 | 25,1 / 26,3 | 4,9 / 4,6 | 3,0 / 2,7 | 18,2 / 28,3 | 0,0 / 0,0 | 0,6 / 0,6 | 0,0 / 0,0 | 17,2 / 14,5 | 19,8 / 30,2 |
| r1k_d300_l30_p65 | 26,5 / 25,9 | 46,1 / 46,1 | 12,2 / 11,8 | 31,8 / 31,6 | 20,2 / 20,1 | 0,9 / 1,4 | 0,0 / 0,0 | 0,8 / 0,7 | 0,0 / 0,0 | 28,2 / 29,0 | 2,9 / 2,9 |
| r1k_d300_l30_p1000 | 31,1 / 36,4 | 78,4 / 69,7 | 23,5 / 25,5 | 5,1 / 4,6 | 3,0 / 2,8 | 12,0 / 22,5 | 0,0 / 0,0 | 0,6 / 0,6 | 0,0 / 0,0 | 26,5 / 23,0 | 13,6 / 24,1 |
| r1k_d1000_l0_p65 | 31,5 / 31,1 | 43,6 / 44,1 | 13,8 / 13,8 | 32,2 / 32,2 | 20,0 / 20,0 | 2,2 / 1,3 | 0,0 / 0,0 | 2,3 / 2,4 | 0,0 / 0,0 | 0,0 / 0,0 | 4,5 / 2,7 |
| r1k_d1000_l0_p1000 | 39,1 / 41,8 | 69,0 / 64,6 | 27,2 / 27,3 | 5,6 / 5,5 | 3,1 / 3,1 | 20,8 / 25,6 | 0,0 / 0,0 | 1,5 / 1,4 | 0,0 / 0,0 | 0,0 / 0,0 | 22,2 / 27,3 |
| r1k_d1000_l10_p65 | 28,9 / 29,1 | 44,3 / 44,4 | 12,8 / 13,0 | 32,1 / 32,1 | 20,0 / 20,0 | 1,3 / 1,3 | 0,0 / 0,0 | 1,9 / 2,2 | 0,0 / 0,0 | 9,9 / 8,4 | 2,8 / 2,7 |
| r1k_d1000_l10_p1000 | 35,5 / 39,0 | 71,4 / 69,1 | 25,8 / 26,5 | 5,7 / 5,2 | 3,2 / 3,0 | 18,0 / 21,0 | 0,0 / 0,0 | 1,5 / 1,2 | 0,0 / 0,0 | 8,4 / 7,6 | 19,5 / 22,5 |
| r1k_d1000_l20_p65 | 26,7 / 26,8 | 44,4 / 44,7 | 11,7 / 11,9 | 32,2 / 32,1 | 20,1 / 20,0 | 1,7 / 1,3 | 0,0 / 0,0 | 1,9 / 2,0 | 0,0 / 0,0 | 19,5 / 18,3 | 3,7 / 2,7 |
| r1k_d1000_l20_p1000 | 32,2 / 34,6 | 74,5 / 72,4 | 25,1 / 24,1 | 5,6 / 5,2 | 3,1 / 3,0 | 15,0 / 18,2 | 0,0 / 0,0 | 1,4 / 1,2 | 0,0 / 0,0 | 15,5 / 15,8 | 16,2 / 19,4 |
| r1k_d1000_l30_p65 | 24,3 / 24,0 | 44,6 / 44,5 | 10,9 / 10,4 | 32,2 / 32,0 | 20,1 / 20,0 | 1,3 / 1,1 | 0,0 / 0,0 | 1,7 / 1,9 | 0,0 / 0,0 | 27,4 / 27,6 | 3,2 / 2,2 |
| r1k_d1000_l30_p1000 | 28,2 / 30,9 | 76,8 / 75,8 | 21,7 / 23,2 | 5,8 / 5,3 | 3,2 / 3,0 | 11,9 / 15,0 | 0,0 / 0,0 | 1,4 / 1,3 | 0,0 / 0,0 | 25,9 / 25,0 | 13,1 / 16,0 |
| r12k_d300_l0_p65 | 28,9 / 28,4 | 48,3 / 48,3 | 14,0 / 13,7 | 31,4 / 31,4 | 20,1 / 20,1 | 0,1 / 0,1 | 0,0 / 0,0 | 0,1 / 0,1 | 0,0 / 0,0 | 0,0 / 0,0 | 0,2 / 0,2 |
| r12k_d300_l0_p1000 | 29,8 / 29,9 | 92,4 / 92,4 | 27,5 / 27,6 | 4,0 / 4,0 | 2,6 / 2,6 | 0,9 / 0,9 | 0,0 / 0,0 | 0,1 / 0,1 | 0,0 / 0,0 | 0,0 / 0,0 | 1,0 / 1,0 |
| r12k_d300_l10_p65 | 27,5 / 27,3 | 48,3 / 48,3 | 13,3 / 13,2 | 31,4 / 31,4 | 20,1 / 20,1 | 0,1 / 0,1 | 0,0 / 0,0 | 0,1 / 0,1 | 0,0 / 0,0 | 10,3 / 9,9 | 0,2 / 0,2 |
| r12k_d300_l10_p1000 | 29,0 / 29,3 | 91,4 / 92,3 | 26,3 / 26,7 | 4,0 / 4,0 | 2,6 / 2,6 | 1,8 / 0,9 | 0,0 / 0,0 | 0,1 / 0,1 | 0,0 / 0,0 | 9,4 / 9,8 | 2,0 / 1,0 |
| r12k_d300_l20_p65 | 25,8 / 25,3 | 48,3 / 48,3 | 12,5 / 12,2 | 31,4 / 31,4 | 20,1 / 20,1 | 0,1 / 0,1 | 0,0 / 0,0 | 0,1 / 0,1 | 0,0 / 0,0 | 20,2 / 19,2 | 0,2 / 0,2 |
| r12k_d300_l20_p1000 | 27,6 / 26,6 | 91,5 / 91,5 | 25,3 / 24,1 | 4,1 / 4,0 | 2,6 / 2,6 | 1,8 / 1,8 | 0,0 / 0,0 | 0,1 / 0,1 | 0,0 / 0,0 | 18,7 / 19,6 | 2,0 / 2,0 |
| r12k_d300_l30_p65 | 22,8 / 21,2 | 48,3 / 48,2 | 11,0 / 10,2 | 31,4 / 31,4 | 20,1 / 20,1 | 0,1 / 0,1 | 0,0 / 0,0 | 0,1 / 0,1 | 0,0 / 0,0 | 29,3 / 29,9 | 0,2 / 0,2 |
| r12k_d300_l30_p1000 | 21,4 / 25,1 | 91,6 / 90,6 | 19,5 / 22,9 | 4,1 / 4,0 | 2,6 / 2,6 | 1,8 / 2,7 | 0,0 / 0,0 | 0,1 / 0,1 | 0,0 / 0,0 | 30,8 / 28,8 | 1,9 / 2,9 |
| r12k_d1000_l0_p65 | 27,0 / 27,0 | 47,8 / 47,8 | 12,9 / 12,9 | 31,5 / 31,5 | 20,1 / 20,1 | 0,2 / 0,2 | 0,0 / 0,0 | 0,4 / 0,4 | 0,0 / 0,0 | 0,0 / 0,0 | 0,3 / 0,3 |
| r12k_d1000_l0_p1000 | 28,7 / 28,7 | 91,0 / 91,0 | 26,1 / 26,2 | 4,2 / 4,2 | 2,6 / 2,6 | 1,8 / 1,8 | 0,0 / 0,0 | 0,3 / 0,3 | 0,0 / 0,0 | 0,0 / 0,0 | 1,9 / 1,9 |
| r12k_d1000_l10_p65 | 23,8 / 23,4 | 47,9 / 47,8 | 11,4 / 11,2 | 31,5 / 31,5 | 20,1 / 20,1 | 0,2 / 0,2 | 0,0 / 0,0 | 0,3 / 0,4 | 0,0 / 0,0 | 9,2 / 9,9 | 0,3 / 0,3 |
| r12k_d1000_l10_p1000 | 26,1 / 27,1 | 91,1 / 91,2 | 23,8 / 24,7 | 4,2 / 4,2 | 2,6 / 2,6 | 1,8 / 1,8 | 0,0 / 0,0 | 0,3 / 0,3 | 0,0 / 0,0 | 11,1 / 9,8 | 1,9 / 1,9 |
| r12k_d1000_l20_p65 | 20,2 / 22,1 | 47,9 / 48,0 | 9,7 / 10,6 | 31,5 / 31,4 | 20,1 / 20,1 | 0,2 / 0,1 | 0,0 / 0,0 | 0,3 / 0,3 | 0,0 / 0,0 | 20,3 / 20,0 | 0,3 / 0,2 |
| r12k_d1000_l20_p1000 | 21,8 / 22,7 | 91,1 / 91,2 | 20,0 / 20,9 | 4,2 / 4,2 | 2,6 / 2,6 | 1,8 / 1,8 | 0,0 / 0,0 | 0,2 / 0,3 | 0,0 / 0,0 | 20,4 / 21,4 | 1,9 / 1,9 |
| r12k_d1000_l30_p65 | 16,5 / 16,2 | 48,0 / 47,9 | 7,9 / 7,8 | 31,5 / 31,5 | 20,1 / 20,1 | 0,2 / 0,1 | 0,0 / 0,0 | 0,3 / 0,3 | 0,0 / 0,0 | 28,8 / 29,7 | 0,3 / 0,2 |
| r12k_d1000_l30_p1000 | 12,7 / 14,4 | 91,1 / 91,4 | 11,6 / 13,3 | 4,2 / 4,2 | 2,6 / 2,6 | 1,8 / 1,8 | 0,0 / 0,0 | 0,2 / 0,3 | 0,0 / 0,0 | 28,9 / 32,1 | 1,9 / 1,9 |
| hücre | ρ_ARQ | dönüş doluluk % | dönüşte ACK % | dönüş kontrol % | fazla mesaj·s | gönderen % | kurtarma % | kuyruk % | geç mesaj / M | geçte hat boş | baskın kaynak |
|---|---|---|---|---|---|---|---|---|---|---|---|
| r25_d300_l0_p65 | 0,298 | 92,4 / 93,9 | 38,8 / 39,1 | 22,2 / 22,1 | 122 / 118 | 0 / 0 | 0 / 0 | 100,0 / 100,0 | 2 / 1 / 6 | 0,000 / 0,000 | hat dolu (kontrol) / hat dolu (kontrol) |
| r25_d300_l10_p65 | 0,331 | 90,6 / 90,0 | 42,7 / 43,3 | 20,8 / 20,5 | 196 / 221 | 0 / 0 | 0 / 0 | 101 / 101 | 2 / 3 / 6 | 0,000 / 0,000 | hat dolu (kontrol) / hat dolu (kontrol) |
| r25_d300_l20_p65 | 0,372 | 74,9 / 77,5 | 48,1 / 45,7 | 18,8 / 19,7 | 196 / 286 | 0 / 0 | -3 / 0 | 106 / 98,0 | 2 / 4 / 6 | 0,021 / 0,000 | hat dolu (kontrol) / hat dolu (kontrol) |
| r25_d300_l30_p65 | 0,425 | 58,8 / 61,5 | 50,1 / 49,9 | 18,1 / 18,2 | 254 / 263 | 0 / 0 | -4 / 4 | 105 / 95,2 | 4 / 5 / 6 | 0,023 / 0,012 | hat dolu (kontrol) / hat dolu (kontrol) |
| r25_d1000_l0_p65 | 0,298 | 91,5 / 94,5 | 38,8 / 39,4 | 22,2 / 22,0 | 118 / 121 | 0 / 0 | 0 / 0 | 100,0 / 100,0 | 1 / 2 / 6 | 0,000 / 0,000 | hat dolu (kontrol) / hat dolu (kontrol) |
| r25_d1000_l10_p65 | 0,331 | 89,6 / 89,7 | 44,2 / 44,3 | 20,2 / 20,2 | 114 / 210 | 0 / 0 | 0 / 0 | 101 / 104 | 2 / 2 / 6 | 0,000 / 0,000 | hat dolu (kontrol) / hat dolu (kontrol) |
| r25_d1000_l20_p65 | 0,372 | 74,5 / 77,3 | 49,7 / 45,9 | 18,2 / 19,6 | 222 / 270 | 0 / 0 | 0 / 0 | 106 / 98,4 | 3 / 4 / 6 | 0,022 / 0,001 | hat dolu (kontrol) / hat dolu (kontrol) |
| r25_d1000_l30_p65 | 0,425 | 59,7 / 60,6 | 50,7 / 51,7 | 17,9 / 17,5 | 233 / 250 | 0 / 0 | -5 / 0 | 105 / 102 | 4 / 5 / 6 | 0,000 / 0,000 | hat dolu (kontrol) / hat dolu (kontrol) |
| r1k_d300_l0_p65 | 0,298 | 10,6 / 10,7 | 92,4 / 92,6 | 2,8 / 2,7 | 2 / 2 | 95,7 / 91,8 | 0 / 0 | 8 / 18,6 | 14,0 / 13,5 / 100 | 0,909 / 0,864 | gönderen / gönderen |
| r1k_d300_l0_p1000 | 0,297 | 3,5 / 3,5 | 75,1 / 73,0 | 9,0 / 9,8 | 2 / 3 | 35,3 / 31,3 | 0 / 0 | 65,1 / 69,3 | 6 / 5 / 20 | 0,604 / 0,139 | kuyruk / kuyruk |
| r1k_d300_l10_p65 | 0,331 | 11,5 / 12,2 | 94,4 / 94,0 | 2,0 / 2,2 | 36,5 / 30,0 | 40,0 / 44,9 | 43,9 / 49,7 | 1 / 1 | 48,5 / 49,0 / 100 | 0,730 / 0,726 | kurtarma / kurtarma |
| r1k_d300_l10_p1000 | 0,330 | 3,0 / 2,7 | 80,1 / 82,5 | 7,2 / 6,4 | 12,3 / 7 | 15,5 / 30,1 | 29,8 / 46,5 | 11,5 / 19,2 | 8 / 7 / 20 | 0,638 / 0,566 | kurtarma / kurtarma |
| r1k_d300_l20_p65 | 0,372 | 11,8 / 12,0 | 95,6 / 95,2 | 1,6 / 1,7 | 55,1 / 73,4 | 23,4 / 26,6 | 76,9 / 54,3 | 1 / 0 | 53,0 / 59,0 / 100 | 0,732 / 0,725 | kurtarma / kurtarma |
| r1k_d300_l20_p1000 | 0,371 | 2,4 / 2,4 | 84,1 / 79,1 | 5,8 / 7,6 | 98,8 / 21,0 | 69,8 / 11,2 | 24,8 / 82,7 | 4 / 7 | 12,5 / 8 / 20 | 0,711 / 0,640 | gönderen / kurtarma |
| r1k_d300_l30_p65 | 0,425 | 11,3 / 11,3 | 96,1 / 96,5 | 1,4 / 1,2 | 103 / 152 | 7 / 38,0 | 87,2 / 49,8 | 1 / 1 | 55,5 / 69,5 / 100 | 0,741 / 0,737 | kurtarma / kurtarma |
| r1k_d300_l30_p1000 | 0,424 | 2,2 / 2,1 | 83,8 / 83,4 | 5,9 / 6,0 | 117 / 42,9 | 59,5 / 10 | 42,3 / 88,9 | 2 / 1 | 12,5 / 11,0 / 20 | 0,727 / 0,708 | gönderen / kurtarma |
| r1k_d1000_l0_p65 | 0,298 | 16,8 / 16,6 | 89,0 / 88,7 | 4,0 / 4,1 | 3 / 2 | 0 / 1 | 0 / 0 | 93,3 / 66,2 | 6 / 6 / 100 | 0,953 / 0,901 | kuyruk / kuyruk |
| r1k_d1000_l0_p1000 | 0,297 | 3,7 / 3,7 | 58,3 / 58,2 | 15,1 / 15,2 | 8 / 2 | 82,1 / 21,7 | 0 / 0 | 32,4 / 76,1 | 8 / 3 / 20 | 0,625 / 0,129 | gönderen / kuyruk |
| r1k_d1000_l10_p65 | 0,331 | 17,3 / 16,1 | 92,1 / 90,4 | 2,9 / 3,5 | 27,3 / 33,2 | 25,1 / 33,4 | 66,5 / 44,9 | 7 / 7 | 25,5 / 31,0 / 100 | 0,743 / 0,744 | kurtarma / kurtarma |
| r1k_d1000_l10_p1000 | 0,330 | 3,1 / 3,1 | 63,3 / 64,8 | 13,3 / 12,8 | 21,1 / 11,3 | 23,4 / 10 | 52,3 / 70,8 | 6 / 11,6 | 8 / 5 / 20 | 0,661 / 0,634 | kurtarma / kurtarma |
| r1k_d1000_l20_p65 | 0,372 | 15,5 / 14,7 | 93,1 / 91,6 | 2,5 / 3,1 | 71,6 / 108 | 25,3 / 69,6 | 65,5 / 34,0 | 4 / 3 | 46,0 / 56,0 / 100 | 0,759 / 0,750 | kurtarma / gönderen |
| r1k_d1000_l20_p1000 | 0,371 | 2,7 / 2,4 | 65,1 / 66,2 | 12,7 / 12,3 | 45,4 / 28,3 | 11,4 / 12,8 | 57,5 / 89,8 | 4 / 3 | 9 / 8 / 20 | 0,664 / 0,693 | kurtarma / kurtarma |
| r1k_d1000_l30_p65 | 0,425 | 13,5 / 12,6 | 94,0 / 93,9 | 2,2 / 2,2 | 129 / 135 | 13,6 / 33,2 | 68,3 / 56,3 | 4 / 2 | 51,0 / 54,0 / 100 | 0,754 / 0,782 | kurtarma / kurtarma |
| r1k_d1000_l30_p1000 | 0,424 | 2,2 / 2,1 | 74,3 / 69,2 | 9,3 / 11,2 | 126 / 56,6 | 65,9 / 10,6 | 29,9 / 82,8 | 1 / 2 | 12,0 / 10,0 / 20 | 0,761 / 0,699 | gönderen / kurtarma |
| r12k_d300_l0_p65 | 0,298 | 13,5 / 13,6 | 99,3 / 99,3 | 0,3 / 0,3 | 24,6 / 24,1 | 104 / 99,1 | 0 / 0 | 3 / 2 | 110 / 108 / 600 | 0,748 / 0,755 | gönderen / gönderen |
| r12k_d300_l0_p1000 | 0,300 | 1,3 / 1,3 | 94,2 / 94,4 | 2,1 / 2,0 | 5 / 3 | 101 / 102 | 0 / 0 | -4 / 4 | 20,5 / 15,0 / 100 | 0,782 / 0,817 | gönderen / gönderen |
| r12k_d300_l10_p65 | 0,331 | 18,5 / 18,4 | 99,5 / 99,5 | 0,2 / 0,2 | 69,1 / 90,5 | 70,4 / 85,9 | 22,8 / 11,4 | 2 / 1 | 280 / 304 / 600 | 0,725 / 0,733 | gönderen / gönderen |
| r12k_d300_l10_p1000 | 0,333 | 1,5 / 1,5 | 96,4 / 95,8 | 1,3 / 1,5 | 21,0 / 25,1 | 36,0 / 56,2 | 60,9 / 48,2 | 1 / -0 | 38,5 / 46,0 / 100 | 0,729 / 0,722 | kurtarma / gönderen |
| r12k_d300_l20_p65 | 0,372 | 16,0 / 15,8 | 99,6 / 99,6 | 0,1 / 0,1 | 176 / 216 | 55,2 / 91,3 | 34,7 / 11,6 | 1 / 0 | 392 / 436 / 600 | 0,742 / 0,745 | gönderen / gönderen |
| r12k_d300_l20_p1000 | 0,375 | 1,5 / 1,4 | 96,0 / 96,5 | 1,4 / 1,3 | 51,2 / 48,6 | 36,5 / 45,9 | 56,5 / 58,2 | 1 / 1 | 55,5 / 55,0 / 100 | 0,724 / 0,736 | kurtarma / kurtarma |
| r12k_d300_l30_p65 | 0,425 | 13,0 / 12,6 | 99,6 / 99,6 | 0,1 / 0,1 | 421 / 692 | 35,5 / 91,9 | 63,0 / 9 | 0 / -0 | 436 / 484 / 600 | 0,769 / 0,783 | kurtarma / gönderen |
| r12k_d300_l30_p1000 | 0,429 | 1,2 / 1,4 | 96,9 / 96,8 | 1,1 / 1,2 | 212 / 92,5 | 31,8 / 41,2 | 45,5 / 55,3 | 0 / 0 | 77,0 / 70,0 / 100 | 0,780 / 0,744 | kurtarma / kurtarma |
| r12k_d1000_l0_p65 | 0,298 | 14,2 / 14,5 | 98,1 / 98,3 | 0,7 / 0,6 | 522 / 474 | 98,0 / 96,3 | 0 / 0 | 2 / 3 | 320 / 304 / 600 | 0,701 / 0,702 | gönderen / gönderen |
| r12k_d1000_l0_p1000 | 0,300 | 1,9 / 1,9 | 88,6 / 88,9 | 4,1 / 4,0 | 133 / 140 | 98,3 / 98,0 | 0 / 0 | 1 / 1 | 53,0 / 54,5 / 100 | 0,695 / 0,696 | gönderen / gönderen |
| r12k_d1000_l10_p65 | 0,331 | 17,3 / 16,5 | 98,9 / 98,7 | 0,4 / 0,5 | 616 / 602 | 96,1 / 96,7 | 2 / 2 | 1 / 1 | 362 / 370 / 600 | 0,712 / 0,714 | gönderen / gönderen |
| r12k_d1000_l10_p1000 | 0,333 | 2,0 / 1,9 | 92,0 / 91,5 | 2,9 / 3,1 | 194 / 165 | 90,4 / 93,0 | 7 / 6 | 1 / 2 | 69,0 / 64,5 / 100 | 0,711 / 0,716 | gönderen / gönderen |
| r12k_d1000_l20_p65 | 0,372 | 13,8 / 14,1 | 99,0 / 99,1 | 0,4 / 0,3 | 988 / 916 | 93,5 / 96,1 | 6 / 3 | 1 / 0 | 478 / 488 / 600 | 0,750 / 0,742 | gönderen / gönderen |
| r12k_d1000_l20_p1000 | 0,375 | 1,7 / 1,7 | 93,1 / 93,5 | 2,5 / 2,4 | 676 / 543 | 88,3 / 93,9 | 7 / 6 | 0 / 0 | 98,0 / 98,0 / 100 | 0,760 / 0,756 | gönderen / gönderen |
| r12k_d1000_l30_p65 | 0,425 | 10,8 / 11,0 | 99,2 / 99,1 | 0,3 / 0,3 | 2 257 / 2 433 | 83,7 / 98,1 | 13,9 / 3 | -0 / -0 | 586 / 587 / 600 | 0,818 / 0,823 | gönderen / gönderen |
| r12k_d1000_l30_p1000 | 0,429 | 0,9 / 1,1 | 94,1 / 94,3 | 2,1 / 2,1 | 1 637 / 1 229 | 84,5 / 92,6 | 14,5 / 8 | -0 / -0 | 98,0 / 99,0 / 100 | 0,857 / 0,845 | gönderen / gönderen |

Dönüş yolu ve fazlanın ayrımı. **Fiziksel yetersizlik ile gönderen kaynaklı gecikme burada ayrılıyor.** Bütün hüküm hücrelerinde ρ_ARQ ≤ 0,43: tek kopyalı iş koruyan bir gönderen için hat yeterlidir.

| hücre | ρ_ARQ | dönüş doluluk % | dönüşte ACK % | dönüş kontrol % | fazla mesaj·s | gönderen % | kurtarma % | kuyruk % | geç mesaj / M | geçte hat boş | baskın kaynak |
|---|---|---|---|---|---|---|---|---|---|---|---|
| r25_d300_l0_p65 | 0,298 | 92,4 / 93,9 | 38,8 / 39,1 | 22,2 / 22,1 | 122 / 118 | 0 / 0 | 0 / 0 | 100,0 / 100,0 | 2 / 1 / 6 | 0,000 / 0,000 | hat dolu (kontrol) / hat dolu (kontrol) |
| r25_d300_l10_p65 | 0,331 | 90,6 / 90,0 | 42,7 / 43,3 | 20,8 / 20,5 | 196 / 221 | 0 / 0 | 0 / 0 | 101 / 101 | 2 / 3 / 6 | 0,000 / 0,000 | hat dolu (kontrol) / hat dolu (kontrol) |
| r25_d300_l20_p65 | 0,372 | 74,9 / 77,5 | 48,1 / 45,7 | 18,8 / 19,7 | 196 / 286 | 0 / 0 | -3 / 0 | 106 / 98,0 | 2 / 4 / 6 | 0,021 / 0,000 | hat dolu (kontrol) / hat dolu (kontrol) |
| r25_d300_l30_p65 | 0,425 | 58,8 / 61,5 | 50,1 / 49,9 | 18,1 / 18,2 | 254 / 263 | 0 / 0 | -4 / 4 | 105 / 95,2 | 4 / 5 / 6 | 0,023 / 0,012 | hat dolu (kontrol) / hat dolu (kontrol) |
| r25_d1000_l0_p65 | 0,298 | 91,5 / 94,5 | 38,8 / 39,4 | 22,2 / 22,0 | 118 / 121 | 0 / 0 | 0 / 0 | 100,0 / 100,0 | 1 / 2 / 6 | 0,000 / 0,000 | hat dolu (kontrol) / hat dolu (kontrol) |
| r25_d1000_l10_p65 | 0,331 | 89,6 / 89,7 | 44,2 / 44,3 | 20,2 / 20,2 | 114 / 210 | 0 / 0 | 0 / 0 | 101 / 104 | 2 / 2 / 6 | 0,000 / 0,000 | hat dolu (kontrol) / hat dolu (kontrol) |
| r25_d1000_l20_p65 | 0,372 | 74,5 / 77,3 | 49,7 / 45,9 | 18,2 / 19,6 | 222 / 270 | 0 / 0 | 0 / 0 | 106 / 98,4 | 3 / 4 / 6 | 0,022 / 0,001 | hat dolu (kontrol) / hat dolu (kontrol) |
| r25_d1000_l30_p65 | 0,425 | 59,7 / 60,6 | 50,7 / 51,7 | 17,9 / 17,5 | 233 / 250 | 0 / 0 | -5 / 0 | 105 / 102 | 4 / 5 / 6 | 0,000 / 0,000 | hat dolu (kontrol) / hat dolu (kontrol) |
| r1k_d300_l0_p65 | 0,298 | 10,6 / 10,7 | 92,4 / 92,6 | 2,8 / 2,7 | 2 / 2 | 95,7 / 91,8 | 0 / 0 | 8 / 18,6 | 14,0 / 13,5 / 100 | 0,909 / 0,864 | gönderen / gönderen |
| r1k_d300_l0_p1000 | 0,297 | 3,5 / 3,5 | 75,1 / 73,0 | 9,0 / 9,8 | 2 / 3 | 35,3 / 31,3 | 0 / 0 | 65,1 / 69,3 | 6 / 5 / 20 | 0,604 / 0,139 | kuyruk / kuyruk |
| r1k_d300_l10_p65 | 0,331 | 11,5 / 12,2 | 94,4 / 94,0 | 2,0 / 2,2 | 36,5 / 30,0 | 40,0 / 44,9 | 43,9 / 49,7 | 1 / 1 | 48,5 / 49,0 / 100 | 0,730 / 0,726 | kurtarma / kurtarma |
| r1k_d300_l10_p1000 | 0,330 | 3,0 / 2,7 | 80,1 / 82,5 | 7,2 / 6,4 | 12,3 / 7 | 15,5 / 30,1 | 29,8 / 46,5 | 11,5 / 19,2 | 8 / 7 / 20 | 0,638 / 0,566 | kurtarma / kurtarma |
| r1k_d300_l20_p65 | 0,372 | 11,8 / 12,0 | 95,6 / 95,2 | 1,6 / 1,7 | 55,1 / 73,4 | 23,4 / 26,6 | 76,9 / 54,3 | 1 / 0 | 53,0 / 59,0 / 100 | 0,732 / 0,725 | kurtarma / kurtarma |
| r1k_d300_l20_p1000 | 0,371 | 2,4 / 2,4 | 84,1 / 79,1 | 5,8 / 7,6 | 98,8 / 21,0 | 69,8 / 11,2 | 24,8 / 82,7 | 4 / 7 | 12,5 / 8 / 20 | 0,711 / 0,640 | gönderen / kurtarma |
| r1k_d300_l30_p65 | 0,425 | 11,3 / 11,3 | 96,1 / 96,5 | 1,4 / 1,2 | 103 / 152 | 7 / 38,0 | 87,2 / 49,8 | 1 / 1 | 55,5 / 69,5 / 100 | 0,741 / 0,737 | kurtarma / kurtarma |
| r1k_d300_l30_p1000 | 0,424 | 2,2 / 2,1 | 83,8 / 83,4 | 5,9 / 6,0 | 117 / 42,9 | 59,5 / 10 | 42,3 / 88,9 | 2 / 1 | 12,5 / 11,0 / 20 | 0,727 / 0,708 | gönderen / kurtarma |
| r1k_d1000_l0_p65 | 0,298 | 16,8 / 16,6 | 89,0 / 88,7 | 4,0 / 4,1 | 3 / 2 | 0 / 1 | 0 / 0 | 93,3 / 66,2 | 6 / 6 / 100 | 0,953 / 0,901 | kuyruk / kuyruk |
| r1k_d1000_l0_p1000 | 0,297 | 3,7 / 3,7 | 58,3 / 58,2 | 15,1 / 15,2 | 8 / 2 | 82,1 / 21,7 | 0 / 0 | 32,4 / 76,1 | 8 / 3 / 20 | 0,625 / 0,129 | gönderen / kuyruk |
| r1k_d1000_l10_p65 | 0,331 | 17,3 / 16,1 | 92,1 / 90,4 | 2,9 / 3,5 | 27,3 / 33,2 | 25,1 / 33,4 | 66,5 / 44,9 | 7 / 7 | 25,5 / 31,0 / 100 | 0,743 / 0,744 | kurtarma / kurtarma |
| r1k_d1000_l10_p1000 | 0,330 | 3,1 / 3,1 | 63,3 / 64,8 | 13,3 / 12,8 | 21,1 / 11,3 | 23,4 / 10 | 52,3 / 70,8 | 6 / 11,6 | 8 / 5 / 20 | 0,661 / 0,634 | kurtarma / kurtarma |
| r1k_d1000_l20_p65 | 0,372 | 15,5 / 14,7 | 93,1 / 91,6 | 2,5 / 3,1 | 71,6 / 108 | 25,3 / 69,6 | 65,5 / 34,0 | 4 / 3 | 46,0 / 56,0 / 100 | 0,759 / 0,750 | kurtarma / gönderen |
| r1k_d1000_l20_p1000 | 0,371 | 2,7 / 2,4 | 65,1 / 66,2 | 12,7 / 12,3 | 45,4 / 28,3 | 11,4 / 12,8 | 57,5 / 89,8 | 4 / 3 | 9 / 8 / 20 | 0,664 / 0,693 | kurtarma / kurtarma |
| r1k_d1000_l30_p65 | 0,425 | 13,5 / 12,6 | 94,0 / 93,9 | 2,2 / 2,2 | 129 / 135 | 13,6 / 33,2 | 68,3 / 56,3 | 4 / 2 | 51,0 / 54,0 / 100 | 0,754 / 0,782 | kurtarma / kurtarma |
| r1k_d1000_l30_p1000 | 0,424 | 2,2 / 2,1 | 74,3 / 69,2 | 9,3 / 11,2 | 126 / 56,6 | 65,9 / 10,6 | 29,9 / 82,8 | 1 / 2 | 12,0 / 10,0 / 20 | 0,761 / 0,699 | gönderen / kurtarma |
| r12k_d300_l0_p65 | 0,298 | 13,5 / 13,6 | 99,3 / 99,3 | 0,3 / 0,3 | 24,6 / 24,1 | 104 / 99,1 | 0 / 0 | 3 / 2 | 110 / 108 / 600 | 0,748 / 0,755 | gönderen / gönderen |
| r12k_d300_l0_p1000 | 0,300 | 1,3 / 1,3 | 94,2 / 94,4 | 2,1 / 2,0 | 5 / 3 | 101 / 102 | 0 / 0 | -4 / 4 | 20,5 / 15,0 / 100 | 0,782 / 0,817 | gönderen / gönderen |
| r12k_d300_l10_p65 | 0,331 | 18,5 / 18,4 | 99,5 / 99,5 | 0,2 / 0,2 | 69,1 / 90,5 | 70,4 / 85,9 | 22,8 / 11,4 | 2 / 1 | 280 / 304 / 600 | 0,725 / 0,733 | gönderen / gönderen |
| r12k_d300_l10_p1000 | 0,333 | 1,5 / 1,5 | 96,4 / 95,8 | 1,3 / 1,5 | 21,0 / 25,1 | 36,0 / 56,2 | 60,9 / 48,2 | 1 / -0 | 38,5 / 46,0 / 100 | 0,729 / 0,722 | kurtarma / gönderen |
| r12k_d300_l20_p65 | 0,372 | 16,0 / 15,8 | 99,6 / 99,6 | 0,1 / 0,1 | 176 / 216 | 55,2 / 91,3 | 34,7 / 11,6 | 1 / 0 | 392 / 436 / 600 | 0,742 / 0,745 | gönderen / gönderen |
| r12k_d300_l20_p1000 | 0,375 | 1,5 / 1,4 | 96,0 / 96,5 | 1,4 / 1,3 | 51,2 / 48,6 | 36,5 / 45,9 | 56,5 / 58,2 | 1 / 1 | 55,5 / 55,0 / 100 | 0,724 / 0,736 | kurtarma / kurtarma |
| r12k_d300_l30_p65 | 0,425 | 13,0 / 12,6 | 99,6 / 99,6 | 0,1 / 0,1 | 421 / 692 | 35,5 / 91,9 | 63,0 / 9 | 0 / -0 | 436 / 484 / 600 | 0,769 / 0,783 | kurtarma / gönderen |
| r12k_d300_l30_p1000 | 0,429 | 1,2 / 1,4 | 96,9 / 96,8 | 1,1 / 1,2 | 212 / 92,5 | 31,8 / 41,2 | 45,5 / 55,3 | 0 / 0 | 77,0 / 70,0 / 100 | 0,780 / 0,744 | kurtarma / kurtarma |
| r12k_d1000_l0_p65 | 0,298 | 14,2 / 14,5 | 98,1 / 98,3 | 0,7 / 0,6 | 522 / 474 | 98,0 / 96,3 | 0 / 0 | 2 / 3 | 320 / 304 / 600 | 0,701 / 0,702 | gönderen / gönderen |
| r12k_d1000_l0_p1000 | 0,300 | 1,9 / 1,9 | 88,6 / 88,9 | 4,1 / 4,0 | 133 / 140 | 98,3 / 98,0 | 0 / 0 | 1 / 1 | 53,0 / 54,5 / 100 | 0,695 / 0,696 | gönderen / gönderen |
| r12k_d1000_l10_p65 | 0,331 | 17,3 / 16,5 | 98,9 / 98,7 | 0,4 / 0,5 | 616 / 602 | 96,1 / 96,7 | 2 / 2 | 1 / 1 | 362 / 370 / 600 | 0,712 / 0,714 | gönderen / gönderen |
| r12k_d1000_l10_p1000 | 0,333 | 2,0 / 1,9 | 92,0 / 91,5 | 2,9 / 3,1 | 194 / 165 | 90,4 / 93,0 | 7 / 6 | 1 / 2 | 69,0 / 64,5 / 100 | 0,711 / 0,716 | gönderen / gönderen |
| r12k_d1000_l20_p65 | 0,372 | 13,8 / 14,1 | 99,0 / 99,1 | 0,4 / 0,3 | 988 / 916 | 93,5 / 96,1 | 6 / 3 | 1 / 0 | 478 / 488 / 600 | 0,750 / 0,742 | gönderen / gönderen |
| r12k_d1000_l20_p1000 | 0,375 | 1,7 / 1,7 | 93,1 / 93,5 | 2,5 / 2,4 | 676 / 543 | 88,3 / 93,9 | 7 / 6 | 0 / 0 | 98,0 / 98,0 / 100 | 0,760 / 0,756 | gönderen / gönderen |
| r12k_d1000_l30_p65 | 0,425 | 10,8 / 11,0 | 99,2 / 99,1 | 0,3 / 0,3 | 2 257 / 2 433 | 83,7 / 98,1 | 13,9 / 3 | -0 / -0 | 586 / 587 / 600 | 0,818 / 0,823 | gönderen / gönderen |
| r12k_d1000_l30_p1000 | 0,429 | 0,9 / 1,1 | 94,1 / 94,3 | 2,1 / 2,1 | 1 637 / 1 229 | 84,5 / 92,6 | 14,5 / 8 | -0 / -0 | 98,0 / 99,0 / 100 | 0,857 / 0,845 | gönderen / gönderen |

**Okuma:**
- **r25:** Hat dolu (%94–98,7), faydalı pay %3,5–10,7. Doluluğu sunulan yük değil, kontrol ve teslim etmeyen kopyalar yaratıyor. Fazlanın tamamı darboğaz kuyruğunda; geç kalma süresinde hattı en çok kontrol tutuyor (`late_busy_ctrl` 0,44–0,73 [T§6]). Bu gecikme fiziksel kapasite yetersizliği değildir: tcp aynı hatta D = 1,000 ve p95 26,8–46,5 s ölçtü (T§14).
- **r12k:** Hat doluluğu %12,7–29,9, geç kalma süresinde hat %70–86 boş. Fazlanın %84–98'i r12k_d1000'de gönderen beklemesi. Gecikme hat yetmediği için değil, gönderen tuttuğu için oluşuyor. d300'de l10–l30 1 KB'de kurtarma payı yükselir.
- **r1k:** 65 B'de fazla kurtarmada (kopyanın ilk varışı), 1 KB'de taban d300 l20–l30 gönderende (PreConverge), adayda kurtarmada.
- **Stres l80 †:** ρ_ARQ 1,49 > 1, fiziksel tavan aşılır ama hat doluluğu 0,005–0,19 kaldı (T-13). Gönderen tavana hiç gelmiyor.
- **Bayt payı:** başlık + L2-4 65 B'de %51–52, 1 KB'de %6,6–9,0 (r25 hariç). Bu, mesaj boyunun fiziksel sonucudur; iki kolda aynı.
- **Onarım:** hüküm hücrelerinde ileri baytta %0,0 (medyan). ACK ileri yönde taşınmıyor (%0,0); dönüş yolunda %39–99,6.

### TS.2.3 Gerçek RTT ↔ motor srtt [ölçüldü]

RTT_fiz ve RTT_fifo tanımı §2.2. srtt koşu sonu değeridir.

| hücre | RTT_fiz | RTT_fifo | srtt taban | srtt aday | oran taban | oran aday | ağ RTT taban | ağ RTT aday | n taban | n aday |
|---|---|---|---|---|---|---|---|---|---|---|
| r25_d300_l0_p65 | 8,880 | 8,879 | – | 84,2 | – | 9,488 | 12,5 | 8,955 | 0 | 30 |
| r25_d300_l10_p65 | 8,880 | 8,879 | 121 | 108 | 13,7 | 12,1 | 61,9 | 67,7 | 11 | 30 |
| r25_d300_l20_p65 | 8,880 | 8,879 | 104 | 89,6 | 11,7 | 10,1 | 82,7 | 80,4 | 20 | 30 |
| r25_d300_l30_p65 | 8,880 | 8,879 | 82,1 | 61,6 | 9,242 | 6,936 | 76,2 | 60,9 | 25 | 30 |
| r25_d1000_l0_p65 | 10,3 | 10,3 | – | 84,2 | – | 8,192 | 10,6 | 13,6 | 0 | 30 |
| r25_d1000_l10_p65 | 10,3 | 10,3 | 125 | 114 | 12,1 | 11,0 | 62,0 | 70,9 | 9 | 30 |
| r25_d1000_l20_p65 | 10,3 | 10,3 | 104 | 92,0 | 10,1 | 8,954 | 92,2 | 83,7 | 21 | 30 |
| r25_d1000_l30_p65 | 10,3 | 10,3 | 86,5 | 62,7 | 8,414 | 6,100 | 73,3 | 67,1 | 24 | 30 |
| r1k_d300_l0_p65 | 0,807 | 0,806 | 0,831 | 0,826 | 1,030 | 1,025 | 0,817 | 0,815 | 30 | 30 |
| r1k_d300_l0_p1000 | 1,742 | 1,741 | 1,759 | 1,810 | 1,010 | 1,039 | 1,778 | 1,775 | 30 | 30 |
| r1k_d300_l10_p65 | 0,807 | 0,806 | 0,882 | 0,876 | 1,093 | 1,086 | 0,853 | 0,860 | 30 | 30 |
| r1k_d300_l10_p1000 | 1,742 | 1,741 | 2,042 | 1,798 | 1,173 | 1,033 | 1,796 | 1,780 | 30 | 30 |
| r1k_d300_l20_p65 | 0,807 | 0,806 | 0,987 | 0,865 | 1,225 | 1,073 | 0,873 | 0,871 | 30 | 30 |
| r1k_d300_l20_p1000 | 1,742 | 1,741 | 2,897 | 1,828 | 1,664 | 1,050 | 1,808 | 1,781 | 30 | 30 |
| r1k_d300_l30_p65 | 0,807 | 0,806 | 1,099 | 0,893 | 1,363 | 1,107 | 0,884 | 0,878 | 30 | 30 |
| r1k_d300_l30_p1000 | 1,742 | 1,741 | 2,976 | 1,829 | 1,709 | 1,050 | 1,799 | 1,780 | 30 | 30 |
| r1k_d1000_l0_p65 | 2,207 | 2,224 | 2,312 | 2,276 | 1,039 | 1,023 | 2,274 | 2,245 | 30 | 30 |
| r1k_d1000_l0_p1000 | 3,142 | 3,140 | 3,256 | 3,158 | 1,037 | 1,006 | 3,229 | 3,191 | 30 | 30 |
| r1k_d1000_l10_p65 | 2,207 | 2,224 | 2,406 | 2,331 | 1,082 | 1,048 | 2,318 | 2,348 | 30 | 30 |
| r1k_d1000_l10_p1000 | 3,142 | 3,140 | 3,748 | 3,318 | 1,194 | 1,057 | 3,196 | 3,226 | 30 | 30 |
| r1k_d1000_l20_p65 | 2,207 | 2,224 | 2,418 | 2,339 | 1,087 | 1,052 | 2,291 | 2,332 | 30 | 30 |
| r1k_d1000_l20_p1000 | 3,142 | 3,140 | 3,618 | 3,280 | 1,152 | 1,045 | 3,200 | 3,169 | 30 | 30 |
| r1k_d1000_l30_p65 | 2,207 | 2,224 | 2,476 | 2,349 | 1,113 | 1,056 | 2,286 | 2,302 | 30 | 30 |
| r1k_d1000_l30_p1000 | 3,142 | 3,140 | 4,594 | 3,336 | 1,463 | 1,063 | 3,256 | 3,178 | 29 | 30 |
| r12k_d300_l0_p65 | 0,617 | 0,662 | 0,670 | 0,681 | 1,013 | 1,029 | 0,669 | 0,685 | 30 | 30 |
| r12k_d300_l0_p1000 | 0,691 | 0,691 | 0,704 | 0,695 | 1,019 | 1,006 | 0,686 | 0,697 | 30 | 30 |
| r12k_d300_l10_p65 | 0,617 | 0,662 | 0,686 | 0,677 | 1,037 | 1,023 | 0,680 | 0,687 | 30 | 30 |
| r12k_d300_l10_p1000 | 0,691 | 0,691 | 0,721 | 0,705 | 1,044 | 1,020 | 0,691 | 0,685 | 30 | 30 |
| r12k_d300_l20_p65 | 0,617 | 0,662 | 0,689 | 0,647 | 1,041 | 0,978 | 0,679 | 0,672 | 30 | 30 |
| r12k_d300_l20_p1000 | 0,691 | 0,691 | 0,775 | 0,703 | 1,122 | 1,017 | 0,710 | 0,706 | 29 | 30 |
| r12k_d300_l30_p65 | 0,617 | 0,662 | 0,680 | 0,643 | 1,028 | 0,972 | 0,658 | 0,655 | 30 | 30 |
| r12k_d300_l30_p1000 | 0,691 | 0,691 | 0,838 | 0,704 | 1,213 | 1,018 | 0,712 | 0,696 | 30 | 30 |
| r12k_d1000_l0_p65 | 2,017 | 2,371 | 2,425 | 2,433 | 1,023 | 1,026 | 2,435 | 2,418 | 30 | 30 |
| r12k_d1000_l0_p1000 | 2,091 | 2,137 | 2,200 | 2,174 | 1,030 | 1,017 | 2,172 | 2,153 | 30 | 30 |
| r12k_d1000_l10_p65 | 2,017 | 2,371 | 2,448 | 2,283 | 1,033 | 0,963 | 2,458 | 2,416 | 30 | 30 |
| r12k_d1000_l10_p1000 | 2,091 | 2,137 | 2,250 | 2,174 | 1,053 | 1,017 | 2,224 | 2,163 | 30 | 30 |
| r12k_d1000_l20_p65 | 2,017 | 2,371 | 2,469 | 2,157 | 1,041 | 0,910 | 2,467 | 2,419 | 30 | 30 |
| r12k_d1000_l20_p1000 | 2,091 | 2,137 | 2,265 | 2,170 | 1,060 | 1,015 | 2,209 | 2,206 | 30 | 30 |
| r12k_d1000_l30_p65 | 2,017 | 2,371 | 2,479 | 2,160 | 1,045 | 0,911 | 2,472 | 2,450 | 30 | 30 |
| r12k_d1000_l30_p1000 | 2,091 | 2,137 | 2,306 | 2,118 | 1,079 | 0,991 | 2,146 | 2,165 | 30 | 30 |

- **r12k:** oran taban 1,01–1,21, aday 0,91–1,03. l20 / l30 65 B d1000'de aday srtt 2,16 s, RTT_fifo'nun altında (0,91); ağ RTT 2,42–2,45 s. Aday örnek kuralı küçük örneği de alıyor [çıkarım].
- **r1k 1 KB:** taban d300 l20 / l30 ve d1000 l30'da 1,46–1,71; aynı hücrelerde aday 1,05–1,06.
- **Taban ≥ aday:** r1k / r12k l20 / l30'da 16/16 hücre. srtt/RTT_fifo'da aralığı 0'ı dışlayan 23 hücrenin hepsinde aday küçük, aday büyük 0 [T§2.3].
- **r25:** oran 6,1–13,7. Bu şişme değil, gerçek öz kuyruktur: ağ RTT 8,96–92 s. Taban l0'da hiç örnek yok (srtt > 0 0/30, l10 9–11/30).

### TS.2.4 Yeniden gönderim: teslime katkı veren ↔ gereksiz [ölçüldü]

Hücre başına iki kol: TS.2.2 `kopya F tel` / `kopya G tel`; adet ve bayt T§5.
- **r25:** F %0–4, G %29–38. Bu hattan geçen baytın üçte biridir ve teslime katkısı neredeyse yoktur.
- **r1k 1 KB l0:** F 0, G %37,6 / %38,5 (d300), %22,2 / %27,3 (d1000). Kayıp yokken kopyanın tamamı gereksiz.
- **r1k 1 KB l30 d300:** F 26,5 / 23,0, G 13,6 / 24,1. Aday gereksiz payı artırıyor.
- **r1k 65 B ve r12k:** G ≤ %4,6; kayıplı hücrelerde kopyalar çoğunlukla faydalı (F %8–32).

## TS.3 Bulgular (ölçüm · dosya:işlev:satır · kök neden · en dar düzeltme)

1. **B-K13 PARAM_SET r25 hattını dolduruyor.**
   - Ölçüm [T§9, N = 30/kol]:
     - `param_sends` medyanı 25, 16/16 kol×hücre.
     - Hattan geçen baytın PARAM payı: l0 %63,4 (iki kol, iki gecikme), l10 %58–59, l20 %51–54, l30 %44–49.
     - r25_d300_l0_p65: hat 3665 B = orijinal 268 B (%7,3) + kopya 1072 B (%29,2) + PARAM 2325 B (%63,4).
     - Teslim 60 s: taban 1/6 → 30/30 koşu (l0, l10, l20), 22/30 (l30). Aday 30/30 (l0, l10), 22/30 (l20), 9/30 ve 5/30 (l30).
     - Dönüş doluluğu l0 %92–95; dönüş baytının %61–62'si ACK dışı, yani PARAM_ACK [T§9.5, türetilmiş].
     - Ters oynatım [türetilmiş, geri beslemesiz, doğrulama hatası p50 0,03–0,18 s]: D S0 0,167–0,50 → yalnız ilk PARAM 0,83–1,00; PARAM katlamalı (6 deneme) 0,67–1,00.
   - Dosya:işlev:satır: `client-core/src/runtime.rs:2554-2569` `client_task` rtprop kolu (`:2558` / aday `:2639`). `seal_send!` doğrudan: motor kararı değil, pacer'a girmez.
   - Kök neden:
     - Aralık sabit 200 ms, RTT'den türemiyor [kod].
     - 25 × 93 B = 2325 B = 93 s hat süresi, 5 s içinde kuyruğa girer [ölçüldü + türetilmiş].
     - Bunun üstüne ilk mesajın 8 kopyası: 3397 B = 136 s. Taban ve aday l0'da ikinci mesajın tek yönü 124 s (T§9.4). 18 s'de sunulan mesaj D içinde kalamaz.
     - RTT 8,9 s olduğundan PARAM_ACK dönmeden 25 deneme biter.
   - En dar düzeltme: aralığı 200 ms × 2^k yapmak, deneme tavanı 25 kalır (F1-B1). Spec aralığı sabitlemiyor (`specs/kama-noise-wire-tdd-v2.0.md:378` yalnız "gelene kadar retry").

2. **B-K2 / B-K1 / B-K8 r25 kopyaları.**
   - Ölçüm [T§9.1–9.2, r25_d300_l0_p65]:
     - Sayaç (UDP deneme): taban 4392 B = data 552 + retx 357 + dup 2208 + ctrl 1275; aday 3023 B = 552 + 828 + 368 + 1275.
     - Tel deneme (L2): taban 6954 B, aday 4871 B. Hat kapasitesi 150 s'de 3750 B: deneme yükü 1,86 / 1,30.
     - Kuruluş probu 21 / 4, geometrik 3 / 0, kuyruk probu 6 / 9.
     - Kopya: gönderilen 30 / 13, teslime katkı 0 / 0, gereksiz 8 / 8, kuyrukta/kayıp 22 / 5.
     - Ajan D'nin gözlemi doğrulandı, ama düzlem karışık: 3665 B hat baytı (L2), 552 / 1275 / 2208 UDP deneme sayacı (toplam 4035 B). Aynı düzlemde ayrım yukarıda.
     - 31 B/s tabanı: pacer yolu (orijinal + ARQ/prob + geometrik) 7,9–9,7 B/s UDP. Taban hız bu yolu bağlamıyor. PARAM ve kuruluş probu tabanın dışında.
     - Ters oynatım, teslim etmeyen kopyalar yok (S2): D 0,50–1,00. PARAM katlamalı + kopyasız (S5): D 0,83–1,00, p50 5,7–7,5 s, p95 11,8–39,5 s; l20'de p95 11,8–14,7 s [türetilmiş; kâhin, üst sınır].
   - Dosya:işlev:satır: `sender.rs:1490-1491` / `:1677-1678`, `:5995-6049` `drain_establish_probe` / `:7213`, `:4306` / `:4796`; `tail_recovery.rs:72-82`, `:155-165` `base_pto_ns`; `sender.rs:5926` `retire_inflight_to_tombstone`; `:1485`, `budget.rs:29`.
   - Kök neden:
     - Taban hiç RTT örneği alamıyor (l0 `rtt_samples` 0, srtt > 0 0/30): kuyruk probu ilk ACK'ten önce orijinali tombstone yapıyor, tombstone örnek vermiyor [ölçüldü + kod]. PreConverge koşu boyu sürer (30/30, l0).
     - Kuruluş probu 0,5 / 1,5 / 3,5 / 7,5 s, sonra 8 s'de bir: 4 + (150 − 7,5)/8 ≈ 21 [türetilmiş]. Ölçülen 21.
     - Örnek yokken PTO 1 s (`initial_pto`), RTT 8,9 s. Bulgu metnimdeki 10 s kıskacı değil, bu 1 s ilk etkendir [sapma].
     - Aday RTT örneğini kabul ediyor ve 9–14 s'de PreConverge'den çıkıyor; prob 4'e iniyor. Yine de kuyruk ilk 7,5 s'de yüklenmiş olduğu için l0–l20'de D değişmiyor.
   - En dar düzeltme: F1-B2. Tek başına yetmez; F1-B1 ile anlamlı.

3. **A-10 + B-K14 Kayıp, hızı dolaylı düşürüyor (iki kol).**
   - Ölçüm:
     - A-10 yeniden üretildi [T§10; Ajan A ürün yolu, rinf hariç payda koşu]: çöküş 12/225 ve 14/225 (Fisher p = 0,84). A'nın 235 paydası rinf dahildir.
     - Backlog örneklerinin durum payı: RecoverArq taban 0,83–0,985, aday 0,376–0,453. ProbeUp taban 0,000 (tasma_ptt 0,010), aday 0,02–0,08. ProbeUp girişi taban 0 (tasma_ptt 3), aday 3–27.
     - BtlBw pencere başına çarpan: 0,928 [0,898–0,954] (n = 139 koşu) / 0,908 [0,878–0,932] (n = 142). Aday ProbeUp'a girdiği halde BtlBw düşüyor.
     - Çöküş fazları: düşük-tepe 6 / 5 (tepe BtlBw 270 / 226 B/s), azalma 6 / 9 (tepe 4344 / 3050 B/s → son 315 / 363 B/s).
     - Gönderen kilidi `kilit_gönderen`, payda koşu [T§7]: havuz r12k l30 78/120 / 92/120 (p = 0,064), r12k l20 30/120 / 44/120 (p = 0,069), r1k l30 36/120 / 56/120 (p = 0,011), r1k l20 23/120 / 38/120 (p = 0,037), r25 0.
     - Hücre: r12k_d300_l30_p65 10/30 / 27/30 (p = 1,1e-05), r1k_d300_l30_p65 3/30 / 16/30 (p = 6,3e-04), r12k_d300_l30_p1000 16/30 / 7/30 (p = 0,033, ters yön), r12k_d1000_l30_p1000 30/30 / 30/30.
     - Yalnız bu ilk ikisi Bonferroni eşiğinin (≈ 0,0008) altında. Havuz p değerleri düzeltmesiz; r1k l30 (0,011) düzeltme sonrası geçmez.
     - `btlbw × (1 − p) / sunulan` < 1 koşullarında kilit 0,512 / 0,736; ≥ 1'de 0,144 / 0,167 (r1k/r12k l10–l30; taban 240 + 480, aday 235 + 485 koşu). btlbw koşu sonu değeri; ters nedensellik olabilir.
     - `pending_lost > 0` payı [T§20, ts.csv]: r12k_d300_l30_p65 0,711 / 0,329, r12k_d1000_l30_p1000 0,672 / 0,315.
     - Kilit fiziksel değil: hat doluluğu r12k_d300_l30_p65 %22,8 / %21,2; r25'teki C tanımı `kilitli` 30/30 ise darboğaz kuyruğudur, gönderen kilidi 0.
   - Dosya:işlev:satır: `sender.rs:3993` `update_state` (`pending_lost` → RecoverArq) / `:4464`; `:4252-4255` `compute_send_rate` (taban Startup değilse Steady kazancı) / `:4723`; `:4076` ProbeUp iptali / `:4547`; `:4104` backlog koşulu / `:4575`; `:3046-3052` pencere / `:3429-3435`; `bw.rs:221` `BtlBwFilter::update`; `rate.rs:164-180` `effective_send_rate`.
   - Kök neden:
     - Pacer = kazanç × BtlBw; BtlBw = teslim hızı örneklerinin pencereli maksimumu [kod].
     - Örnek kaybolan baytı saymaz. Hat tıkalı değilken örnek ≈ pacer × (1 − p) [çıkarım; Ajan C B-C9 ile aynı].
     - `pending_lost` boş değilken durum RecoverArq, kazanç 1. ProbeUp yalnız Steady'den başlar. Taban backlog'da neredeyse hiç Steady görmüyor [ölçüldü].
     - ProbeUp girse de 1,25 × 0,7 = 0,875 < 1: p = 0,3'te ProbeUp BtlBw'yi yükseltemez [türetilmiş].
     - Sonuç: rastgele kayıp BtlBw üzerinden hızı düşürüyor. Formül kayıp kör ama girdi değil. `rate.rs` yorumu "dört girdi loss-kör" der; INV-RATE-1'in amacı ölçülen hücrelerde sağlanmıyor [ölçüldü + kod].
     - İdeal çarpan g(1 − p) = 0,7 iken ölçülen 0,91–0,93. Fark, maksimum filtrenin ACK kümelenmesi ve yeniden gönderim patlamalarında yüksek örnek tutmasından gelebilir [çıkarım; ayırt edici ölçüm yok].
     - Aday kilidi 65 B l30'da artırıyor: fazla bileşeni r12k_d300_l30_p65 gönderen %36 → %92, kurtarma %63 → %9 (T§13.1). Aday baş boşluğu kurtarmaya veriyor, bekleme yeni veriye kayıyor [çıkarım; Ajan A A-3 ile tutarlı].
   - En dar düzeltme: F1-B4. Kayıp girdi olamaz (INV-RATE-1); yukarı yönlü işaret yalnız kuyruk gecikmesidir. `compute_send_rate` tek satır değildir, BtlBw filtresi de değişir.

4. **B-K15 (yeni) Başlangıç uçuş tavanı: MIN_CWND, d1000'de sunulan yükün altında.**
   - Ölçüm [T§8, T§19, N = 30/kol]:
     - r12k_d1000_l0_p65: PreConverge 2,2 s; ilk 5 s emisyon 1596 / 1617 B/s, sunulan 2556 B/s. Sunumdan > 0,2 s sonra çıkan 298 / 286 mesaj (M = 600), sonuncusu 11,4 / 10,9 s'de. Alıcı p95 3,58 / 3,43 s; quinn 1,45, tcp 2,41.
     - Aynı profil d300: 58 mesaj, p95 0,68 / 0,67 s. 4800 / 0,7 s = 6,9 kB/s, sunulanın üstünde.
     - r12k_d1000_l30_p1000: emisyon 5 s aralıkla 1956 / 2259 / 2259 / 2670 / 3081 / 2978 B/s (taban), sunulan 3604 B/s, quinn 5,1–5,3 kB/s. Kilit 30/30 iki kol, hat doluluğu %12,7 / %14,4, alıcı p50 17,1 / 13,4 s, quinn 1,28 s.
     - Ajan A ts serisi: backlog örneklerinin yalnız %3–7'sinde pacer × srtt = 4800 B ± %3 [T§10]. Tavan bir başlangıç olgusudur, backlog boyunca sürmez.
   - Dosya:işlev:satır: `rate.rs:101` `min_cwnd: 4 * DEFAULT_MTU`; `rate.rs:164-180` `effective_send_rate` (`cwnd/srtt` sert tavan); `sender.rs:4208` Startup.
   - Kök neden:
     - İlk RTT'de hız tavanı 4800 B / 2,1 s ≈ 2,3 kB/s < sunulan [kod + ölçüldü].
     - Rampa çarpanı RTT başına 2(1 − p) × rtprop/srtt: l0 1,53–1,67, l30 1,04–1,18 (`g_cwnd`, T§19). l30'da rampa neredeyse yok [türetilmiş].
     - Hâlâ biriken kuyruk bitmeden Startup biterse BtlBw altta kalır ve A-10 mekanizması devralır.
   - **r12k_d1000 oran çöküşü A-10 ile aynı kök mü?** Tetik farklı, sürdürme aynı. Tetik: başlangıç tavanı (yalnız d1000). Sürdürme: `pending_lost > 0` payı 0,672 / 0,315 ve BtlBw ≈ teslim hızı. Sunulan yük 28,5 s'de bittiği için B'deki düşüş A'nın sonsuz backlog'uyla bire bir kıyaslanmaz: 30 s sonrası emisyon düşüşünde gönderen kuyruğu da boşalıyor (`hiz_seri.csv`).
   - En dar düzeltme: F1-B5 (`min_cwnd` 10 × MTU). Tam etkisi F1-B4'e bağlı.

5. **A-9 Startup, uygulama sınırlı turda bitiyor.**
   - Ölçüm: A-10 düşük-tepe fazı 6 / 5 koşu; backlog başında BtlBw 232 / 188 B/s, tepe 270 / 226 B/s [T§10]. A-9: r12k dosyasızda pacer hattın %2,9 / %2,5'i (RAPOR-A). B'de r12k l0 btlbw/sunulan 1,29–1,34 (d300) [T§7].
   - Dosya:işlev:satır: `sender.rs:4208-4232` `startup_plateau` / `:4679`.
   - Kök neden: plato sayımı yalnız "yeni örnek geldi mi"ye bakıyor; tur uygulama sınırlıysa BtlBw büyümez ve tur plato sayılır [kod]. Sonradan gelen dosya yükü Startup kazancını bulamıyor [çıkarım].
   - En dar düzeltme: uygulama sınırlı turu plato saymamak (F1-B6).

6. **B-K4 PTO, alıcının ACK tutmasını görmüyor.**
   - Ölçüm [T§16, T§5]:
     - r1k_d300_l0_p1000: kayıp yokken kuyruk probu 11 / 11 (M = 20), gereksiz kopya 13 / 13, gereksiz yük %35,0 / %36,0, ileri doluluk %49,9 / %49,6 (ρ 0,30). ACK tutma p95 2,65 / 3,51 s.
     - tel/teslim 1,78 / 1,78; tcp 1,14, quinn 1,49 [T§21].
     - Aday artırıyor: r1k_d300_l20_p1000 gereksiz yük %18,2 → %28,3, gereksiz bayt +4207 [+2931; +5672]; kuyruk probu 6 → 12.
     - T§21 K-7 oranı r1k 1 KB'de 0,30–0,95, sınır 0–0,64. 40 hücrede oran ≤ sınır 17 / 18.
   - Dosya:işlev:satır: `tail_recovery.rs:79` `max_ack_delay_ns` 25 ms, `:155-165`; `receiver.rs:76` `ACK_FREQ_N`, `:268-324` `on_ack_timer`, `:434-446` `tick_ack`.
   - Kök neden: seyrek trafikte tek paketin ACK'i ikinci pakete dek bekliyor (p95 ≈ I = 3,6 s). PTO ≈ 1,8–2,5 s; ACK ≈ RTT + tutma. Prob ACK'ten önce ateşlenir [kod + ölçüldü]. Taban şişik srtt yüzünden daha az prob atar; aday srtt'yi düzeltince sorun görünür hale geliyor [çıkarım].
   - En dar düzeltme: PTO'ya pencereli azami `ack_delay` eklemek (F1-B3); tel değişmez.

7. **B-K11 + B-K8 PreConverge: 05b'nin ölçülen kazancı.**
   - Ölçüm [T§16, T§2.3]:
     - r1k_d300_l20_p1000: PreConverge çıkışı 23,7 → 1,9 s; gönderen bekleme p95 16,6 → 0,67 s; srtt/RTT_fifo 1,66 → 1,05; alıcı p95 −11,6 [−16,9; −8,2] s.
     - r1k_d1000_l30_p1000: PreConverge 24,9 → 3,7 s; alıcı p95 −8,4 [−12,4; −3,5] s.
     - Ters taraf: aynı hücrelerde gereksiz bayt artıyor (bulgu 6), ileri faydalı pay r1k_d300_l20_p1000 −0,101 [−0,132; −0,065].
   - Dosya:işlev:satır: taban `sender.rs:3066-3092`, `:3094-3127`; aday `:3449-3460`, `:3609-3620`; `:1485` / `:1672` 31 B/s; `:4239-4244` / `:4710-4715`.
   - Kök neden: PreConverge'de hız 31 B/s, 1027 B birim ≈ 33 s jeton. Tabanda çıkış için orijinalin ACK'i gerekir; Duplicate/kopya ACK'i sayılmaz [kod]. Aday kabul kuralı bunu kaldırıyor.
   - En dar düzeltme: 05b madde 7 (F1-B7). Ölçülen fark bütün adaya aittir; madde 7 tek başına ölçülmedi.

8. **B-K3 SACK delikleri: ölçüldü, engelleyici değil.**
   - Ölçüm [T§17]: 16 aralık r12k 65 B l10+ ve r1k 65 B l30'da (T-6). Bu hücrelerde ACK baytının %47'si aralık [türetilmiş, T§17]. Dönüş doluluğu r1k / r12k en çok %18,5. r25'te aralık 3, aralık payı %14.
   - Dosya:işlev:satır: `runtime.rs:3580`, `receiver.rs:394-420`, `ack.rs:36`, `:92-149`.
   - Kök neden: yeniden gönderim yeni PN alıyor [kod]. Ama aralık dolu hücrelerde dönüş hattı boş, r25'te ise delik az. §4'teki "ACK başına +2,56 s" ölçülen hücrelerde tetiklenmedi [sapma].
   - En dar düzeltme: F1'e alınmadı.

9. **B-K5 onarım maliyeti: ölçülemedi.**
   - Ölçüm [T§18, payda koşu]: hüküm havuzlarında parite > 0 koşu r1k/r12k 0–8/60. r25 l30'da 30/60 / 50/60, en çok 1 parite. Kurtarılan birim havuz başına ≤ 16.
   - Dosya:işlev:satır: `kama-fec-codec/src/profile.rs:85-90`; arbiter Ajan C.
   - Kök neden: bu yükte blok kapanmıyor / planlanmıyor; ayrıntı Ajan C (B-C4). TASLAK-NOTLARI H-15.
   - En dar düzeltme: Ajan C.

10. **B-K12 quinn r25 [betimsel, N = 5].** 40/40 koşu 60 s'de el sıkışamadı; `setup_s` 60,0. RFC 9000 §14.1 fiziksel alt sınır ≥ 99 s. Kama için düzeltme yok.

**Kapananlar:**
- T-12: kapasite reddi 0, 80/80 kol×hücre; alıcı teslim düşüşü 2560/2560 Kama koşusunda 0.
- B-K9: yakalama yeterli, İSTEK-K1 gerekmedi.
- B-K10: `offer_qfull` ve `unaccepted` 2560/2560 koşuda 0.
- B-K6, B-K7: ölçülmedi; kayıt olarak kalır. 22 B zarf 65 B veri paketini 134 → 140 B yapar [türetilmiş].

### TS.3.1 Tahmin–ölçüm kıyası [T§11, T§12]

Tutan = medyan ön-kayıtlı aralıkta, kol×hücre sayısı.

| T | sonuç | ölçüm | sapma dekompozisyonu |
|---|---|---|---|
| T-1 | **kısmen** | teslim 16/16; tek yön 14/16; teyit 10/16; 65 B pay 7/8; 1 KB pay 4/4 | Aşağıda |
| T-2 | **kısmen** | prob ≥ 0,4 M 6/16; gereksiz ≥ 0,4 M 6/16; ACK tutma 15/16; çürütme eşiği geçildi 16/16 | d300 l0–l10 ve aday l20–l30 tutuyor. d1000'de 4–7 prob: ACK tutması srtt/rttvar'a girip PTO'yu uzatıyor [çıkarım]. Taban l20–l30 6: şişik srtt (1,66–1,71) PTO'yu uzatıyor |
| T-3 | **tuttu** | 8/8, 8/8 | – |
| T-4 | **kısmen** | aday 4 (8/8); taban 17,5–21 (0/8); eşik 16/16; 2. mesaj 80–124 s (≥ 12) | Tahmin PreConverge'in ilk RTT'de bittiğini varsaydı. Tabanda bitmiyor: +17 prob = (150 − 7,5)/8 [türetilmiş] |
| T-5 | **tuttu** | 6/6; tcp dahil p95 > 12 s | – |
| T-6 | **kısmen** | = 16 16/24; ≥ 12 20/24 | r1k l10–l20'de koşu başına delik < 16 (M = 100, p·M = 10–20). Tahmin bu üst sınırı unutmuştu |
| T-7 | **kısmen** | aday 32/40; taban ≥ aday 16/16 | Tutmayan 8 hücre r25: oran 6,1–12,1, gerçek öz kuyruk (ağ RTT ölçüldü). RTT_fifo boş hat varsayar; r25'te hat dolu |
| T-8 | **tuttu** | 8/8 (N = 5, betimsel), 8/8 | – |
| T-9 | **tuttu** | 8/8 | – |
| T-10 | **kısmen** [betimsel] | tcp faydalı pay 0,185–0,255 (tahmin 0,31); Kama 0,036–0,107 (≤ 0,3) | Kama sınırın çok altında: PARAM paydaya girmiyordu. tcp: yeniden gönderim + ACK ileri tel |
| T-11 | **kısmen** | l0 parite 0 20/20. İkinci kısım ölçülemedi (parite ≈ 0) | – |
| T-12 | **tuttu** | 80/80, 80/80 | – |
| T-13 | **kısmen** [betimsel] | teslim < 1 8/8; doluluk ≥ 0,9 **0/8** (0,005–0,19) | Tahmin gönderenin ρ_ARQ 1,49'u sürdüğünü varsaydı. Pacer teslim hızına bağlı olduğu için yük hiç oluşmuyor (bulgu 3) [çıkarım] |
| T-14 | **tuttu** | 16/16, 16/16 | – |
| T-15 | **kısmen** | ≥ yarı 4/8; eşik ≥ %20 6/8; kuyruk < 0,2 s 4/8 | Kilit kayba ve kola bağlı: taban l20 %0–27, aday l30 %90–93. Taban d1000'de `queue_extra` p95 0,21–0,23 s: jitter payı [çıkarım] |
| T-16 | **kısmen** | 6/8; eşik 3/4 | Taban medyan 23,7 / 8,1 / 3,8 / 24,9 s: dağılım iki tepeli (ilk orijinalin ACK'ine bağlı) [çıkarım]. Aday 1,9–3,7 s, tuttu |
| T-17 | **tuttu** | 4920/4920 | – |

**Ön-kayıtlı çürütme ölçütünün geçildiği yerler (kol×hücre):** T-1 9/60 parça (ölçüt: tek hücre dışı yeter), T-6 4/24 (r1k l10), T-7 8/40 (r25), T-15 2/8 (d1000 l20), T-16 1/4 (r1k_d1000_l20_p1000). T-13'ün hat doluluğu parçası ölçütsüzdü ve 0/8 ile tamamen çürüdü. T-2, T-3, T-4, T-5, T-8, T-9, T-12, T-14, T-17 ölçütünü geçmedi. "Kısmen", kapsamın bir bölümünde tuttu, bir bölümünde çürüdü demektir.

**T-1 dekompozisyonu [T§12]:**
- r12k_d300_l0_p65 teyit: 0,703 / 0,705 s, üst 0,698 s. Sapma +5 / +7 ms (%0,7). Sınır ACK tutmasını I = 36 ms aldı; önemsiz.
- r12k_d1000_l0_p65 teyit 2,85 / 2,75 s, üst 2,41 s; sapma +0,44 / +0,34 s. Gönderen bekleme p50 0,18 / 0,00 s; kalan artık 0,21 / 0,28 s. Medyanlar toplanmadığı için artık, ilk 11 s birikiminin (B-K15) dağılım etkisidir [çıkarım].
- r12k_d1000_l0_p1000 tek yön p50 1,71 / 1,58 s; RTT_fifo tek yön 1,13 s; oran 1,52 / 1,40 (> 1,3). Sapma +0,58 / +0,45 s: gönderen 0,40 / 0,28, yolculuk +0,02, artık 0,16 / 0,15.
- Sonuç: üç ıskanın ikisi B-K15, biri ölçü sınırı.

**Bulgu sapmaları (§4 ↔ ölçüm):**
- B-K1: etken 10 s kıskacı değil, örnek yokken 1 s PTO.
- B-K3: "+2,56 s" tetiklenmedi.
- B-K8: r25'te hız tabanı bayt payı açısından bağlamıyor; asıl etkisi r1k 1 KB PreConverge.
- B-K14: `btlbw_sunulan` tek başına açıklamıyor, `× (1 − p)` ve kol gerekiyor.
- Yeni: B-K15. Önceki hipotezim (rtprop/srtt oranı d1000'i açıklar) tutmadı: oran iki gecikmede 0,75–0,84. Fark 4800/srtt'nin sunulan yükle ilişkisinden.

## TS.4 Sınıflandırma (ISEMRI §5)

Sınıf: **dar** = tek crate, motor dışı. **sözleşme** = S1, AMP ya da tel değişir. **motor çekirdeği** = adaptive-transport gönderen zamanlayıcısı, hız ya da kurtarma kuralı.

| davranış | (i) 05b düzeltiyor mu | (ii) sınıf | (iii) KTP-2 0.3 | (iv) korunacak avantaj |
|---|---|---|---|---|
| B-K13 PARAM r25 | **hayır**: 25 deneme ve PARAM payı iki kolda aynı; l0–l10'da D 1/6 30/30 | **dar** (client-core `runtime.rs`, motor dışı; tel değişmez) | §3.6 kontrol sınıfı jetonlu; PARAM'ın o yola gireceği yazılı değil | – |
| B-K2 kuruluş probu | **kısmen**: prob 21 → 4, tel deneme 6954 → 4871 B; D l0–l20'de fark gösterilmedi, l30 +0,250 [0,167; 0,333] ve +0,333 [0,167; 0,333] | **motor çekirdeği** (`sender.rs`) | §3.4 PTO + §3.5 uçuş tavanı; TASLAK-NOTLARI V-22, D-K3 | – |
| B-K1 PTO | **kısmen**: taban örneksiz kalıyordu (0/30), aday 30/30; prob 6 → 9 | **motor çekirdeği** (`tail_recovery.rs`) | §3.4; D-K4 | – |
| B-K4 PTO ↔ ACK tutma | **hayır**: 05b'de aynı kod, r1k_d300_l20_p1000 gereksiz bayt +4207 [+2931; +5672] | **motor çekirdeği**; tel değişmez | §3.4, §3.11; V-18 | – |
| B-K11 srtt | **evet**: 23 hücrede küçük, büyük 0; r1k 1 KB 1,46–1,71 → 1,05–1,06 | **motor çekirdeği** (05b'de yazılı) | §3.4 | – |
| B-K8 PreConverge r1k 1 KB | **evet**, ölçülen 4 hücrede: p95 −11,6 [−16,9; −8,2], −9,3 [−21,9; −5,0], −8,4 [−12,4; −3,5], −3,6 [−10,0; −1,6]; r1k 1 KB'nin kalan 4 hücresinde fark gösterilmedi | **motor çekirdeği** (05b'de yazılı) | §3.5 taban hız; V-22 | – |
| A-10 / B-K14 hız ve kilit | **hayır**: çöküş 12/225 ↔ 14/225 (p = 0,84); pencere çarpanı 0,93 ↔ 0,91; kilit r1k l30 havuzu 36/120 → 56/120 (p = 0,011), r12k l30 78/120 → 92/120 (p = 0,064); r12k_d300_l30_p1000 ters yön 16/30 → 7/30 (p = 0,033) | **motor çekirdeği** (`sender.rs`, `bw.rs`) | §3.5 "bant filtresi aynen kullanılır": mekanizmayı devralır. TASLAK-NOTLARI'nda yok | Yeni tasarım kayıp körlüğünü bozmamalı: tcp karşısında l30 kazancı bundan gelir [çıkarım] |
| A-9 Startup | **hayır**: `startup_plateau` aynı; düşük-tepe 6 ↔ 5 | **motor çekirdeği** | §3.5; karşılığı yok | – |
| B-K15 MIN_CWND | **hayır**: p95 r12k_d1000_l0 fark gösterilmedi; emisyon 2259 B/s iki kolda | **motor çekirdeği** (tek sabit) | §3.5 uçuş tavanı max(2 paket, …) **daha dar**: 2 × 1100 B / 2,1 s ≈ 1 kB/s < 3,6 kB/s [türetilmiş]. TASLAK-NOTLARI'nda yok; V-22 yalnız r25 yönünü yazıyor | – |
| B-K3 SACK | **hayır**, engelleyici de değil | sınıflanmadı | §3.4 "her ACK son aralıkların tamamını taşır"; V-18 | – |

**(iv) Ölçülen avantajlar:**
- **Yüksek rastgele kayıp, tcp karşısında (N = 30, hüküm):** r12k l30'da teslim D 1,000 ↔ tcp 0,605–0,873 (d300), 0,465–0,466 (d1000). aday − tcp p95 −13,0 [−18,1; −9,0], −29,9 [−42,7; −20,3], −7,1 [−15,8; −1,8], −25,5 [−34,9; −7,3]; r1k l30 dört hücrede −7,3…−18,4. tcp D-13 taşır.
- **Stres [betimsel, N = 10]:** l80 r1k 65 B teslim 0,67 / 0,71 ↔ quinn 0,000, tcp 0,000; 1 KB 0,225 / 0,400 ↔ 0,000, 0,050. l80 r12k 65 B 0,617 / 0,804 ↔ quinn 0,645, tcp 0,000; 1 KB 0,035 / 0,460 ↔ 0,120, 0,000. Bu dört hücrede quinn 35/40 koşu hiç teslim edemedi. Bench'te Kama el sıkışması yok (D-9); kıyas üst sınırdır.
- **quinn karşısı:** kayıpsız r1k'da p95 kısa (−2,9 [−2,9; −2,7], −1,6, −1,3 s) ama fark ilk mesajların el sıkışmayı beklemesinden gelir ve D-9'a tam duyarlıdır: avantaj olarak yazılmaz. r12k'da 16 hücrenin 15'inde quinn p95 kısa, birinde fark gösterilmedi.
- **Oturum:** Kama koşularında rc = 0 2560/2560.
- **Bozuk dönüş yolu:** bu ızgarada ölçülmedi.

**Korunmayan iddia:** r25'te Kama avantajı yok. tcp aynı hücrelerde D = 1,000 ve p95 26,8–46,5 s, Kama 0,167–0,500 ve 82–118 s.

## TS.5 F1 girdisi (uygulama listesi)

Beklenen etkiler [tahmin]dir; F1'de ölçülecek. Süre ajan-gün, ölçüm hariç. Sıra: bağımlılık ve fayda / süre.

| # | crate / dosya | değişiklik | sınıf, bağımlılık | ölçü ve beklenen etki [tahmin]; dayanak | süre |
|---|---|---|---|---|---|
| F1-B1 | client-core `runtime.rs:2554-2569` | PARAM_SET yeniden deneme aralığı 200 ms × 2^k; tavan 25 kalır | **dar**; bağımlılık yok | K-3: r25 D 0,167 → 0,67–1,00; p50 61–127 s → 15–56 s. Dayanak: PARAM hat %44–63; S4 ters oynatım (geri beslemesiz, kaybolan PARAM'ın yeniden deneme zamanı modellenmez) | 0,5 |
| F1-B5 | adaptive-transport `rate.rs:101` | `min_cwnd` 4 → 10 × MTU | **motor çekirdeği**; Steady sonrası tam etki F1-B4'e bağlı | K-2, K-6: r12k_d1000 geç çıkan mesaj 286–298 → ≈ 60/600; l0 p95 3,4–3,6 → ≤ 2 s. Dayanak: 4800/srtt ≈ 2,0–2,3 kB/s, d300 aynı hücre 58 mesaj | 0,5 |
| F1-B7 | adaptive-transport `sender.rs` | 05b madde 7 RTT örnek kuralını tek başına almak (aday `:3449-3460`, `:3609-3620`) | **motor çekirdeği**; 05b'nin merge kararı sahipte | K-2: r1k 1 KB p95 −8…−12 s. Dayanak: bütün adayın farkı; madde 7 ayrı ölçülmedi. Yan etki: B-K4 görünür olur, F1-B3 ile birlikte | 0,5 |
| F1-B3 | adaptive-transport `tail_recovery.rs:155-169` (`base_pto_ns`, `current_pto_ns`), `sender.rs:3188` | PTO'ya sabit 25 ms yerine pencereli azami `ack_delay` (`:3188` bugün yalnız gözlem) | **motor çekirdeği**; tel değişmez; bağımlılık yok | K-7: r1k_d300_l0_p1000 oran 0,65 → ≤ 0,1, gereksiz yük %35 → ≤ %5, tel/teslim 1,78 → ≈ 1,2. Dayanak: 11/20 prob, ACK tutma p95 2,6–3,5 s. Risk: gerçek kuyruk kaybında kurtarma ≈ I gecikir | 1 |
| F1-B2 | adaptive-transport `sender.rs` `drain_establish_probe`, `tail_recovery.rs` `base_pto_ns` | Kuruluş probu ilk aralığı ve örneksiz PTO = max(bugünkü, geometrik kopyanın `rtt_floor`'u); üst kıskaç max(10 s, 2 × srtt) | **motor çekirdeği**; F1-B1 ile anlamlı | K-3: S5 üst sınırı p50 5,7–7,5 s, l20 p95 11,8–14,7 s (kâhin kopya). Ölçüt 12 s; ideal ARQ 14,5–16,6 s (V-4): K-3 bu düzeltmelerle de sınırda kalır | 1,5 |
| F1-B6 | adaptive-transport `sender.rs:4208` | Uygulama sınırlı turu Startup platosu saymamak | **motor çekirdeği**; bağımlılık yok | K-1: düşük-tepe çöküş 6 / 5 → 0. Dayanak: T§10 tepe 226–270 B/s. Ajan A A-9 / F1-A4 buna bağlı | 1 |
| F1-B4 | adaptive-transport `sender.rs` (`update_state`, `compute_send_rate`), `bw.rs` | Backlog + düşük kuyruk gecikmesinde RecoverArq'ta ProbeUp kazancı ve BtlBw düşüşünün durması (kayıp kör; işaret qd) | **motor çekirdeği**; qd duyarlılığı gerekir; F1-B5, F1-B6 sonrası | K-1, K-2: r12k l30 kilit havuzu %65–77 → l10 düzeyi %8–11; çöküş 12–14/225 → 0; r12k_d300_l30_p65 p95 3,8–3,9 → ≈ 2 s. Dayanak: quinn aynı hücrede 1,9 s; r12k_d1000_l30_p1000'de quinn emisyonu 5,1–5,3 kB/s, Kama 2,0–3,8 kB/s | 3 |

**Listede olmayanlar:**
- **B-K3:** ölçülen hücrelerde engelleyici değil (bulgu 8).
- **B-K5:** ölçülemedi; Ajan C.
- **B-K6 / B-K7:** ürün kararı / ölçüm notu.

**TASLAK-NOTLARI adayı (Ajan E):** §3.5 uçuş tavanı 2 paket ve "bant filtresi aynen" cümlesi; bulgu 3 ve 4.

## TS.6 TABAN girdisi (S2 + S5 katkısı)

**S2 — Düşük bantta hat kapasitesinin ne kadarı gerçek veriye gidiyor? 25 B/s'de %3,5–10,4; 1 kB/s'de %10–28; 12,5 kB/s'de %8–28.** Hüküm hücreleri, N = 30/kol, faydalı yük / hat kapasitesi; sunulan yük hattın %30'u (`b_` ızgarası).

- **25 B/s (65 B):** hat %94–98,7 dolu, faydalı/geçen %3,5–10,7. Hattan geçen baytın %44–63'ü PARAM_SET, %29–45'i kopya, %7–16'sı orijinal. Kopyaların teslime katkısı neredeyse yok. 60 s teslim 0,167 (l0–l20 medyan; l30 aday 0,42–0,50). tcp aynı hatta 1,000; quinn 40/40 el sıkışamadı (N = 5, betimsel).
- **Sebep kapasite değil:** ρ_ARQ ≤ 0,43. 25 PARAM × 93 B, 5 s içinde 93 s hat süresi yüklüyor (`runtime.rs:2558`, motor dışı). 31 B/s tabanı bayt payı açısından bağlamıyor: pacer yolu 7,9–9,7 B/s. Ters oynatım [türetilmiş]: PARAM katlamalı D 0,67–1,00.
- **1 kB/s:** 65 B faydalı/geçen %44–46, kayıp arttıkça kurtarma; 1 KB %56–78, gereksiz yük %12–36. Kayıpsız 1 KB'de bile %35–36 gereksiz: PTO alıcının ACK tutmasını görmüyor (`tail_recovery.rs:79`); 05b bunu artırıyor.
- **12,5 kB/s:** faydalı/geçen 65 B %48, 1 KB %91–92; bu başlık payıdır. Hat %13–30 dolu, geç kalma süresinde %70–86 boş. Gecikme gönderende: pacer ≈ teslim hızı, kayıpta RecoverArq'ta kazanç 1 (`sender.rs:3993`, `:4252`).
- **Yüksek RTT:** 12,5 kB/s, 1 s tek yönde başlangıç tavanı 4800 B/srtt ≈ 2,3 kB/s, sunulanın altında (`rate.rs:101`): 600 mesajın ≈ 290'ı geç çıkıyor, p95 3,4–3,6 s, quinn 1,45 s.
- **Başlık:** 65 B'de başlık + L2-4 %51–52; fiziksel, iki kolda aynı. Onarım %0,0. ACK dönüşte; dönüş hattı r1k/r12k'da en çok %18,5.
- **05b (aday − taban):** srtt düzeldi (23 hücre, ters 0). r1k/r12k 1 KB'de 5 hücrede p95 −3,6…−11,6 s. Aynı hücrelerde gereksiz bayt arttı. 65 B kilit: r12k_d300_l30 10/30 → 27/30. r25 l0–l20 teslimde fark gösterilmedi. Hız çöküşü: fark gösterilmedi (12/225 ↔ 14/225, p = 0,84).

**S5 katkısı — somut değişiklikler (ayrıntı TS.5):**
- Dar: PARAM_SET aralığı (F1-B1, 0,5 gün).
- Motor çekirdeği, tek sabit / tek kural: `min_cwnd` (F1-B5), 05b madde 7 (F1-B7), PTO `ack_delay` (F1-B3), kuruluş probu + örneksiz PTO (F1-B2), Startup platosu (F1-B6).
- Motor çekirdeği, gerçek değişiklik: RecoverArq kazancı + BtlBw düşüşü (F1-B4, 3 gün). KTP-2 §3.5 aynı filtreyi devraldığından bu iş KTP-2'de de gerekir.

## TS.7 Yeniden üretim

Worktree kökünden:

```
R=~/kama-k4g-work/ktp2-f0/runs/tam/b/out
A=~/kama-k4g-work/ktp2-f0/runs/tam/a/out
G=$(mktemp -d)
python3 -I scripts/ktp2/f0/b/analiz_b.py $R --out $G
gzip -c -n -9 $G/kosular_b.csv > reports/ktp2/f0/b/tam/kosular_b.csv.gz
python3 -I scripts/ktp2/f0/b/r25_kok.py $R reports/ktp2/f0/b/tam/r25_kosular.csv
python3 -I scripts/ktp2/f0/b/a10_kok.py $A reports/ktp2/f0/b/tam/a10_kosular.csv
python3 -I scripts/ktp2/f0/b/hiz_seri.py $R reports/ktp2/f0/b/tam/hiz_seri.csv 5
python3 -I scripts/ktp2/f0/b/tam_b.py --tam reports/ktp2/f0/b/tam --kok $R
(cd reports/ktp2/f0/b/tam && sha256sum -c SHA256SUMS)
```

- Ham kökler git dışıdır: `runs/tam/b/out/`, `runs/tam/a/out/` (A-10 girdisi `alice.jsonl`).
- Çıktılar `reports/ktp2/f0/b/tam/`: `TAM-OTOMATIK.md` (T§1–T§23), `kosular_b.csv.gz` (4920 satır), `r25_kosular.csv`, `a10_kosular.csv`, `hiz_seri.csv`, `farklar.csv`, `s2_paylar.csv`, `tahmin.csv`, `SHA256SUMS`.
- Bu bölümdeki tablolar `TAM-OTOMATIK.md`'den aynen alınmıştır.
- sha256:
  - `TAM-OTOMATIK.md` `0990bfb65006048c08c768c5b0b842b41d95128e39fec2c3bdf63f7ebbe577ef`
  - `a10_kosular.csv` `505dbc9196fed9395b0795ede36ec20ac4ee4a7330e26fed73741012ec2a8e48`
  - `farklar.csv` `a15de087f5993a3767e1e8e01c8cfcbcf3cdd40f8336d1d8b61932cba12d6bef`
  - `hiz_seri.csv` `bbf791bcb6bfbf844949e4ebf0ae2072c66ad5f76944757cf42848d825fbd8ee`
  - `kosular_b.csv.gz` `17bfd8b4e85d81c942c7586d43cabf956a40bae3c9d54a1ed3db623a68b33106`
  - `r25_kosular.csv` `e1bff79c02909a9f984c448c389d1a3be82f1d3de606a1046ed2ff381d6337a9`
  - `s2_paylar.csv` `8878be25b4e06ed4dc0d5079507d221c9fed6afa8f92c5b0c18841dbd57630c7`
  - `tahmin.csv` `374a18e0257c6a3ab6c73992e4f31fba37c9228b9947215e34296d5dcf829838`
