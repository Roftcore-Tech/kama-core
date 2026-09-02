# KAMA ADAPTİF TRANSPORT — BÜTÜN RAPOR
## Cold-Start Liveness + Faithful Wall-Clock FEC Utility + Sensör Observability

> **Bu çalışma ARQ ile FEC'i karşılaştırmaz. ARQ (Automatic Repeat reQuest) bütün deney kollarında
> aktiftir; aynı implementasyonu, aynı ACK/NACK/PTO/retransmit semantiğini kullanır. Ölçülen tek şey,
> ARQ substratının ÜZERİNE eklenen proaktif FEC repair'lerinin (r0=ARQ+0, r1=ARQ+1, r2=ARQ+2, r4=ARQ+4)
> marjinal faydasıdır. FEC, ARQ'nun yerine geçmez; FEC'in kapatamadığı residual eksik daima ARQ ile
> tamamlanır (hibrit). "ARQ FEC'i yendi / FEC ARQ'dan iyi" gibi ifadeler bu çalışmada YASAKTIR ve
> analizi geçersiz kılar.**

**Tarih:** 2026-07-23 · **Branch:** `d1-sprint5c` · **PR:** #44 (draft) · **Commit aralığı:** FO..FV
(`42d4ef0`..`6cc56a1`) + kombinatoryal temel FJ+FK (`501665e`)
**Üretim koduna etki:** SIFIR (tüm değişiklikler feature-gated harness / test / script / rapor).

---

## 0. YÖNETİCİ ÖZETİ

Bu turda üç ayrı fakat bağlı Ar-Ge kapısı kapatıldı. İki nihai hüküm çıktı ve önceki turun üç yanlış/
eksik sonucu düzeltildi.

### İki nihai hüküm

| # | Kapı | Hüküm |
|---|---|---|
| **1** | Cold-start liveness | **COLD-START LIVENESS DEFECT CONFIRMED — PROGRESS DR REQUIRED** |
| **2** | Wall-clock FEC utility + observability | **FEC WALL-CLOCK UTILITY IDENTIFIED — CONTROLLER OBSERVABILITY BLOCKED; SENSOR/MEASUREMENT DR REQUIRED** |

### Üç düzeltme (bu turun faithful-harness'ı sayesinde)

1. **"loss sensörü kör" (önceki tur) YANLIŞTI.** `loss_windowed_ppm=0` ölçümü, netem'in gecikme/kayıp
   **uygulamadığı** bozuk bir harness'ta yapılmıştı. Gerçek kayıpta sensör doğru okuyor.
2. **"Wall-clock FEC faydası ölçülemez" (önceki tur) ÇÖZÜLDÜ.** Faithful harness ile RTT≤250ms'te pozitif
   fayda ilk kez ölçüldü.
3. **Cold-start donması (FK'da gözlenen)** bir harness-artefaktı değil, **gerçek bir transport liveness
   kusuru** olarak sınıflandırıldı.

### Tek cümlelik cevaplar (kullanıcının sorularına)

- **Cold-start problemi nedir?** İlk paket kaybolursa (veya ilk ACK dönmezse), gönderici PreConverge
  durumuna yapısal olarak **kilitlenir**: RTT örneği (ACK) gelene kadar durum ilerlemez, PTO donuktur, ve
  dispatch 31 B/s tabanına iner. Tek kaçış, ~61 saniye sonra (1200B birim için) gelen ACK-bağımsız
  geometrik kopyadır. Bu, hedef kullanım (hostile ağlar; bağlantı başında kayıp SIK) için gerçek bir zaaftır.
- **FEC neden (çoğu yerde) işe yaramadı, nerede yaradı?** Proaktif FEC'in tek gerçek avantajı, kayıp
  paketi bir round-trip beklemeden (proaktif parity ile) kurtarmaktır. Bu avantaj yalnızca **düşük RTT'de
  (≤250ms)** materyalize oldu; orada FEC completion'ı 46-77ms hızlandırdı ve sıkı deadline'ları yakaladı.
  **Yüksek RTT'de (≥1000ms) fayda kayboldu** — çünkü mevcut modelde parity, blok kapanışında (tüm source
  gönderildikten sonra) emit edilir; ACK-clocked source dispatch, yüksek RTT'de blok kapanışını ~1 RTT
  geciktirir, böylece parity ancak ARQ onarımıyla aynı anda varır. Yani FEC'in "round-trip atlama" avantajı,
  blok-kapanış gecikmesiyle silinir.

---

## 1. NEDEN GÜVENEBİLİRİZ: FAİTHFUL ÖLÇÜM ALTYAPISI (Phase-B / FQ)

Önceki turlarda "netem" testleri rootless tek-namespace veth üzerinde koşuluyordu. Bu turda **kanıtlandı ki
bu kurulum netem gecikmesini/kaybını UYGULAMIYOR**: qdisc `delay 600ms` yapılandırılsa bile ping RTT ≈
0.02ms. Aynı veth çifti aynı namespace'te olduğunda çekirdek paketleri hızlı yoldan teslim ediyor, egress
qdisc atlanıyor.

**Çözüm (root'suz, docker-daemon'suz):** iki AYRI network namespace.
- `unshare -rnm` → parent netns (userns içinde CAP_NET_ADMIN).
- `unshare -n sleep <büyük> &` → child netns; `/proc/$CHILD/ns/net` beklenir.
- `ip link set v1 netns $CHILD` → veth'in bir ucu child'a taşınır (PID-referanslı; `/var/run/netns` mount
  gerekmez — bu, root'suz `ip netns add`'in başarısız olduğu yer).
- rx tarafı child'da `nsenter -t $CHILD -n` ile koşar. Forward netem = v0 (parent egress, tx→rx); reverse
  netem = child'daki v1 (rx→tx).

**Acceptance kapısı 6/6 PASS** (`faithful_acceptance.sh`):

| Test | Sonuç |
|---|---|
| RTT 100 / 1000 / 2500ms (simetrik) | ölçülen-min = configured (birebir) |
| Forward-only 500ms (reverse hızlı) | RTT 501ms (asimetrik yön izolasyonu) |
| Reverse-only 500ms | RTT 501ms |
| Blackout+recovery (reverse %100 loss → restore) | pre=2, during=0, post=3 (replies) |
| Host cleanup | veth sızıntısı yok |

Sweep içi ölçülen: rtt250=250.09ms, rtt1000=1000.18ms, rtt2500=2500.33ms (configured-exact).

> **Ders (kalıcı):** Herhangi bir netem ölçümüne güvenmeden önce RTT/loss acceptance pini koştur. Bozuk
> tek-ns netem, geçmişte iki ayrı sonucu sessizce bozdu (aşağıda Bölüm 5).

Kod: `scripts/netem/faithful_harness.sh` (`faithful_setup`/`faithful_netem`/`faithful_rx`) · acceptance:
`faithful_acceptance.sh` · commit **FQ = `28f0f6a`**.

---

## 2. COLD-START LIVENESS PROBLEMİ (Phase-A / FO + FP)

### 2.1 Progress invariant (önceden ilan edildi — koddan, sonuç görülmeden)

Gönderici (`Sender`) saf-reaktif bir state-machine'dir: tek mutasyon girişi `on_event`'tir; **kendini
uyandıran bir timer'ı ve "sonraki uyanış" çıktısı YOKTUR**. Bu, tüm analizin çekirdeğidir.

- **PreConverge çıkışı yapısal olarak ACK-kapılıdır:** durum yalnız `rtt.srtt().is_some()` iken Startup'a
  geçer (sender.rs:1382-1388); `srtt` yalnız **boş olmayan bir ACK'in RTT örneğiyle** set edilir
  (`observe`, 1136; boş ACK erken döner, 987-989). ACK yoksa → PreConverge'de kalıcı kilitli.
- **PreConverge'de PTO donuktur:** time-threshold loss tespiti ve retransmit gate'lenir (1177). Emergency/
  forward-recovery de bypass (ack_confidence FULL-pinli).
- **Tek ACK-bağımsız ilerleme = §3.3-B geometrik kopya:** ilk-uçuş birimi `dup_queue`'ya kuyruklanır;
  ilk kopya `first_delay = (3/2)·(birim_boyutu + 65) / 31` saniyede; sonra ×2 backoff, sınırsız, bir ACK
  iptal edene kadar. `schedule_transmit` her `on_event`'te koşar → **caller-clocked** (gönderici kendisi
  uyanmaz).
- **Dispatch PreConverge'de 1-birim tabanı + 31 B/s refill** ile sınırlıdır (bir sonraki 1200B birim için
  ~38.7s refill bekler). Bu, bilinmeyen kapasiteli yavaş kanalı boğmamak için kasıtlıdır.

**İlan edilen sınır:** ilk uygun gönderimden sonra, ACK-bağımsız bir kopya en geç **2×first_delay** içinde
emit edilmeli (aksi = SilentStall). Durum ilerlemesi ACK yokken yapısal olarak imkansızdır.

### 2.2 Kanıt — iki kol (direct-integration + wire)

**Direct-integration** (`tests/cold_start_liveness.rs`; GERÇEK Sender kod-yolu; simüle-zaman `now_ns` elle
ilerletilir, gerçek uyku yok → ~61s sınırı anında taranır; 4 test yeşil):

| unit_size | first_delay | **gözlenen ilk Duplicate** | 2×sınır içinde | state |
|---|---|---|---|---|
| 65 B | 6.3 s | **6.3 s** ✓ | ✓ | PreConverge-pinli |
| 300 B | 17.7 s | **17.7 s** ✓ | ✓ | PreConverge-pinli |
| 1200 B | 61.2 s | **61.2 s** ✓ | ✓ | PreConverge-pinli |

1200B zaman çizgisi: `t=0 Data(seq0)` → `t=61.25 Duplicate(unit1, taze-seq)` → `t=183.70 Duplicate`
(×2 backoff). Kontrol testi: hayatta kalan bir ACK PreConverge'i açar (Startup'a geçer) → mekanizma ACK
varlığında çalışır; kilit ACK-yokluğuna özgüdür.

**Wire-arm** (UDP-runner faithful harness üzerinde; RTT 100ms; blok-0 ilk-source kaybı; 90s ufuk): blok-0
**90 saniyede tamamlanmadı**; alıcı 8s sessizlik sonrası terk etti (cold-start dispatch aralığı ~38s ≫
alıcı sabrı 8s).

**İki kol uyumlu → bu GERÇEK bir transport özelliğidir, harness artefaktı değil** (direct-integration
zaten gerçek Sender kodunu çalıştırır; wire uç-uca sonucu doğrular).

### 2.3 Neden DEFECT? (PASS-vs-DEFECT değerlendirmesi)

| Kriter | Gözlem | Yön |
|---|---|---|
| Bounded progress (kurtarılabilir senaryo) | Geometrik kopya 2×first_delay içinde | PASS yönü |
| PTO arm/fire | PreConverge'de DONUK | **DEFECT göstergesi** |
| ACK'siz durum ilerlemesi | Yapısal olarak İMKANSIZ | **DEFECT göstergesi** |
| Silent stall (sınır aşımı) | Yok, ama sınır ~61s@1200B | borderline |
| PreConverge dispatch tabanı | 31 B/s (glacial, ama kalıcı değil) | borderline |

İki DEFECT göstergesi (PTO arm yok + ACK'siz durum ilerlemez) **literal olarak** karşılanır. Kalıcı
silent-deadlock değildir, ama recovery zaman ölçeği (~61s@1200B, ~6.3s@65B) + PTO donukluğu + yapısal
ACK-bağımlılık, hedef kullanım için (bağlantı başında first-flight loss / reverse blackout SIK) gerçek bir
liveness zaafıdır.

### 2.4 HÜKÜM + düzeltme seçenekleri (uygulanmadı — ayrı onay)

> **COLD-START LIVENESS DEFECT CONFIRMED — PROGRESS DR REQUIRED.**

Fix bu turda UYGULANMADI (direktif gereği önce sınıflandır + raporla). Seçenekler ve riskleri:

1. **PreConverge'de PTO'yu INITIAL_RTT (100ms) ile aç** → ilk kayıp bounded retransmit alır (~61s yerine
   birkaç×100ms). *Risk:* gerçek RTT ≫ 100ms olan linklerde (2.5s) prematüre retransmit / self-congestion.
2. **PreConverge minimum-progress timer** → first_delay'i bounded tavana kırp. *Risk:* yavaş-kanal koruması
   (31 B/s tabanının varlık nedeni) zayıflar.
3. **Durum çıkışını bounded probe ile aç** (kesin ACK yerine konservatif initial-RTT). *Risk:* RTT örneği
   olmadan ramp → aşırı gönderim.
4. **31 B/s tabanını yükselt.** *Risk:* yavaş-kanal flooding.

**Ortak gerilim:** cold-start liveness ↔ yavaş-kanal koruması. Bu bir tasarım kararıdır (DR), trivial fix
değil. Rapor: **FP = `9e0a4b5`** · test: **FO = `42d4ef0`**.

---

## 3. FEC UTILITY: NEREDE İŞE YARADI, NEREDE YARAMADI (Phase-C/D/E / FS + kombinatoryal FK)

FEC utility'sinin **iki katmanı** vardır ve ikisini ayırmak şarttır:

### 3.1 Kombinatoryal katman — "FEC decode MÜMKÜN MÜ?" (FK; `501665e`; delay-bağımsız)

Bu tamamen matematiksel bir özelliktir (MDS erasure kodu): K=8 source'luk bir blokta `deficit` kadar
source silinirse, blok ancak `(K − deficit) + r ≥ K`, yani **`r ≥ deficit`** iken first-flight'ta (round-
trip'siz) decode edilebilir. Bu, her RTT'de aynıdır ("iso" anchor ile her profilde doğrulandı):

| deficit | r < deficit | r ≥ deficit |
|---|---|---|
| 1 | r0 → PENDING (7<8) | r1 → FEC-COMPLETE (7+1=8) |
| 2 | r1 → PENDING (6+1=7<8) | r2 → FEC-COMPLETE (8) |
| 4 | r2 → PENDING (4+2=6<8) | r4 → FEC-COMPLETE (8) |

Bu **decodability sınırıdır** ve nettir. Ama "decode mümkün" demek "gerçek zaman kazancı var" demek DEĞİL.

### 3.2 Wall-clock katman — "FEC gerçekten ZAMAN kazandırıyor mu?" (FS; faithful harness; 48 koşu/0-fail)

İşte asıl soru burada cevaplanıyor. Ölçülen büyüklükler (blok-ilk-gelişine göreli, tek-yön gecikme iptal):
- **T_fec** = parity'nin varış zamanı (FEC decode'un mümkün olduğu an).
- **T_arq** = silinen source'un substrat onarımının (ARQ retransmit) varış zamanı.
- **completion** = bloğun tamamlanma gecikmesi = min(FEC yolu, ARQ yolu).

**Phase-C — Zamanlama fizibilitesi** (deadline, kazanan görülmeden donduruldu):

| RTT | T_fec p50/p99 (ms) | T_arq p50/p01 (ms) | ayrışma? |
|---|---|---|---|
| **250ms** | 282-287 / **293-296** | 350-368 / **315-328** | **VAR** (T_fec_p99 < T_arq_p01) |
| 1000ms | 1062-2480 / 1914-3183 | 1074-1141 / 1037-1050 | yok/örtüşme |
| 2500ms | 2671-6401 / 4870-8292 | 2647-2875 / 2602-2616 | yok/örtüşme |

RTT 250ms için donmuş deadline = **305ms** (296 ve 315 arasında, önceden ilan).

**Phase-D/E — Marjinal wall-clock utility** (hybrid; r≥d vs r<d; 60 blok/hücre):

| RTT | deficit | r<d completion p50 | **r≥d completion p50** | FEC-path(r≥d) | SINIF |
|---|---|---|---|---|---|
| **250ms** | 1 | 333ms | **287ms** (Δ−46) | **30/30** | **MarginalFecBeneficial** |
| **250ms** | 2 | 352ms | **300ms** (Δ−52) | **30/30** | **MarginalFecBeneficial** |
| **250ms** | 4 | 392ms | **315ms** (Δ−77) | **30/30** | **MarginalFecBeneficial** |
| 1000ms | 1 | 1074ms | 1042ms | 15/30 | NoMarginalFecUtility_thisCondition |
| 1000ms | 2 | 1101ms | 1117ms | 6/30 | NoMarginalFecUtility_thisCondition |
| 1000ms | 4 | 1211ms | 1194ms | 0/30 | NoMarginalFecUtility_thisCondition |
| 2500ms | 1 | 2635ms | 2633ms | 14/30 | NoMarginalFecUtility_thisCondition |
| 2500ms | 2 | 2683ms | 2685ms | 6/30 | NoMarginalFecUtility_thisCondition |
| 2500ms | 4 | 3074ms | 3070ms | 0/30 | NoMarginalFecUtility_thisCondition |

**Donmuş deadline (305ms) @ RTT 250ms:** r<d (completion 333-453ms) → **deadline %100 KAÇIRIR**; r≥d
(completion 287-332ms) → **deadline %100 YAKALAR** → deficit 1/2/4 için **MarginalFecEssential** (ARQ+0
deadline'ı kaçırır, ARQ+r yakalar).

### 3.3 FEC neden yüksek RTT'de işe yaramadı? (mekanizma — sezgiye aykırı ama ölçülü)

**Naif beklenti:** FEC round-trip'ten kaçındığı için, RTT büyüdükçe DAHA faydalı olmalı. **Ölçülen tam
tersi.** Sebep:

Mevcut modelde **parity, blok KAPANIŞINDA emit edilir** (tüm K source dispatch edildikten sonra). Source
dispatch ise ACK-clocked'dır (offered ≤ 0.6C).
- **Düşük RTT'de:** blok hızlı kapanır → parity erken gider → T_fec (~287ms, intra-block yayılım + tek-yön,
  RTT'den görece bağımsız) ARQ round-trip'inden (~350ms) ÖNCE varır → **FEC kazanır**.
- **Yüksek RTT'de:** blok kapanışı ACK-clocking ile ~1 RTT gecikir → parity geç gider → T_fec (~1062ms@1s,
  ~2671ms@2.5s) ARQ onarımına (~1074/2647ms) EŞİTLENİR ya da AŞAR → **FEC avantajını kaybeder**.

Bunu doğrulayan işaret: r büyüdükçe T_fec de büyür (rtt2500 d4: T_fec=6401ms vs T_arq=2875ms — parity ARQ'dan
2.2× yavaş), çünkü daha çok parity blok-kapanış sonrası daha geç emit edilir. FEC-path kullanımı da bununla
düşer (rtt1000: 15/30 → 6/30 → 0/30).

> **Kritik nüans:** Bu sınır **emisyon modeline bağlıdır**, FEC'in doğasına değil. Parity, blok kapanışını
> beklemeden source ile birlikte (first-flight) gönderilseydi, yüksek RTT'de de kazanabilirdi. Bu, gelecek
> bir tasarım girdisidir (bu turda DEĞİŞTİRİLMEDİ).

Rapor: **FS = `d57e8ae`** · runner: **FR = `782fddd`** (arq_repair_ms ledger + dispatch-gated settle).

---

## 4. LOSS-SİNYAL FİDELİTY: BÜYÜK DÜZELTME (Phase-F / FT)

### 4.1 Önceki hüküm neden yanlıştı

Önceki tur (FE/FF): 231k fuzzy event boyunca `loss_windowed_ppm=0` (blackout30 dahil) → "sensör kaybı
görmüyor → observability blocked" hükmü verilmişti. **Bu ölçüm bozuk tek-ns netem'de yapılmıştı** — netem
kaybı da (gecikme gibi) UYGULAMIYORDU. Yani loss=0 DOĞRUYDU (gerçek kayıp yoktu), ama bu sensör körlüğü
değildi; harness'in kaybı uygulamamasıydı.

### 4.2 Kod ne diyor (loss.rs)

`loss_windowed_ppm` = **pre-recovery erasure metresi**: Confirmed durumundaki orijinal-DATA kayıp
birimlerinin toplamı / penceredeki gönderilen DATA birimleri (ppm). Kritik özellikler:
- Retransmit ve duplicate birimleri `units=0` ile kaydolur → ne paya ne paydaya girer.
- **Recovery ÇIKARMAZ:** retransmit taze seq alır → orijinal seq asla yeniden ACK'lenmez; RecoveredControl
  loss detektörüne dokunmaz. Yani başarılı ARQ/FEC, kaybı windowed tahminden silmez.
- Pencere = 8×max(srtt, EAI) kayan pencere; fuzzy okumadan önce güncellenir.

Sonuç: sensör, **FEC-controller'ın tam istediği şeyi** ölçer — recovery ÖNCESİ erasure.

### 4.3 Faithful kayıp deneyi (bu tur; gerçek forward loss)

`run_loss_fidelity.sh` (faithful harness; forward=v0 GERÇEK loss + 50ms delay; reverse temiz; `--emit-fuzzy`;
warm steady-state):

| profil | configured | **warm loss_windowed_ppm (mean)** | true-fraction oranı |
|---|---|---|---|
| loss0 | 0% (0 ppm) | **0** | baseline |
| loss10 | 10% (100000 ppm) | **81 780** | ~%82 |
| loss20 | 20% (200000 ppm) | **167 646** | ~%84 |
| loss30 | 30% (300000 ppm) | **271 945** | ~%91 |

**Sensör gerçek kaybı GÖRÜR ve doğru izler** (monoton, true-fraction'a yakın). Hafif eksik-okuma (~%82-91)
= confirm-age (1.5×EAI) + pencere gecikmesi.

### 4.4 Fidelity caveat'leri (körlük değil, ölçüm nüansları)

- **Gecikme/lag:** taze bir drop bir ufuk boyunca görünmez (confirm-age + pencere); warm steady'de doğru.
  Cold-start'ta time-threshold loss donuk → cold-start kayıp sinyali gecikmeli/eksik.
- **Forward↔reverse ayrımı yok:** reverse ACK kaybı da loss declare eder. (Bu deneyde reverse temiz tutuldu.)
- **ack_confidence forward loss'a duyarsız:** %30 forward loss'ta bile ack_conf=1000 (FULL). Forward kaybı
  YALNIZ loss_ppm taşır.
- **fuzzy_ratio=0 @ %30 loss:** controller kaybı GÖRÜR (loss_ppm=272k) ama proaktif FEC=0 üretir, çünkü
  fuzzy loss-membership HIGH eşiği ~%50 (500k-600k ppm). Bu bir **eşik-kalibrasyon** konusudur (bu turda
  KAPSAM DIŞI), sensör körlüğü değil.

> **HÜKÜM: LOSS SIGNAL FIDELITY DEFECT — RED.** Sensör sağlam; önceki "kör" hükmü harness artefaktıydı.
> Önceki turun "loss-signal-fidelity DR"sinin loss yarısı **KAPANDI**. Rapor: **FT = `33803fa`**.

---

## 5. CONTROLLER OBSERVABILITY (Phase-G / FU)

### 5.1 Soru

FEC-faydalı rejim (RTT≤250ms) karar verilmeden ÖNCE mevcut sinyallerle ayırt edilebilir mi? Yani controller
"şu an proaktif FEC eklemek marjinal fayda sağlar" durumunu görebilir mi?

### 5.2 Mevcut fusion girdileri vs. gereken ayrıştırıcı

`FusionInputs` (fusion.rs): `loss_ppm`, `bandwidth_bps`, `jitter_ppm`, `qd_ratio_permille`,
`ack_confidence_permille`. **RTT / srtt / deadline / time-to-deadline YOK.**

Utility rejiminin ayrıştırıcısı = **RTT** (düşük RTT → faydalı). Ayrıca loss/deficit varlığı ve deadline
sıkılığı.

| Rejim bileşeni | Mevcut girdi | Gözlemlenebilir? |
|---|---|---|
| Loss/deficit varlığı | `loss_ppm` (sağlam; §4) | **EVET** |
| RTT (asıl ayrıştırıcı) | — (fusion girdisi değil) | **HAYIR** (mevcut vektörde) |
| Deadline / time-to-deadline | — (app katmanı) | **HAYIR** |

### 5.3 Aday ölçüm — srtt zaten hesaplanıyor ve RTT'yi temiz ayrıştırıyor

`srtt_us`, `diag_rate()` ile expose edilir ama fusion girdisi değildir. Karar öncesidir (geçmiş ACK'lerden;
sızıntı yok). Ölçüldü:

| configured RTT | srtt_us (warm p50) | ayrışma |
|---|---|---|
| 100ms | **112 769** | — |
| 2500ms | **2 516 564** | 22×; örtüşme yok |

⇒ srtt, faydalı-vs-nötr rejimi **temiz ayrıştırır**. Aday türev: `deadline_slack_ratio = time_to_deadline /
est_arq_repair`, `est_arq_repair ≈ 1.5×srtt` (hepsi karar öncesi; no-leakage doğrulandı).

### 5.4 Observability sınıfları + hüküm

| Boyut | Sınıf |
|---|---|
| Loss/deficit | `ObservablePreDecision` (loss_ppm sağlam) |
| RTT rejimi | `UnobservableWithCurrentFusionInputs` → `ObservableWithCandidateMeasurement` (srtt mevcut) |
| Cold-start rejimi | `Confounded` (srtt=None; ayrı rejim şart) |

> **HÜKÜM: FEC WALL-CLOCK UTILITY IDENTIFIED — CONTROLLER OBSERVABILITY BLOCKED (mevcut girdilerle);
> OBSERVABLE WITH CANDIDATE MEASUREMENT → SENSOR/MEASUREMENT DR REQUIRED.** Utility VAR ama mevcut fusion
> vektörü RTT/deadline boyutunu görmez; ayrıştırıcı ölçüm (srtt) halihazırda hesaplanır → DR = srtt +
> deadline-slack'i karar vektörüne bağla (loss sensörü zaten sağlam; fuzzy eşik-tuning DEĞİL). Rapor:
> **FU = `86a392c`**.

---

## 6. SENTEZ — HER ŞEY NASIL BAĞLANIYOR

1. **Cold-start ve FEC ayrı problemlerdir ama aynı kök mimariyi paylaşır:** her ikisi de ACK-clocked
   dispatch + PreConverge'in ACK-bağımlılığı etrafında döner. Cold-start'ta ACK hiç gelmez (kilit); yüksek
   RTT'de ACK-clocking blok kapanışını geciktirir (FEC avantajını siler).
2. **FEC'in decodability'si (kombinatoryal) her yerde çalışır; wall-clock faydası ise dar bir rejimde
   (RTT≤250ms) çalışır.** İkisini karıştırmak, önceki turun "decodability var → utility var" hatasına yol açar.
3. **Faithful harness olmadan bu ayrımların hiçbiri görülemezdi.** Bozuk netem, hem cold-start'ı gizlemiş
   (kısa koşularda 61s'e ulaşılmıyordu) hem de loss sensörünü haksız yere suçlamıştı.
4. **Controller için tam resim:** proaktif FEC'i faydalı rejimde armak için controller'ın (a) kaybı görmesi
   (loss_ppm — VAR), (b) RTT'yi görmesi (srtt — hesaplanıyor ama fuse edilmemiş), ve (c) cold-start'ta karar
   vermemesi (srtt=None ayrı rejim) gerekir. Ek olarak fuzzy loss eşiği ~%50 çok yüksek (ayrı kalibrasyon).

### Açılan yeni DR'lar (ikisi de ayrı onaya tabi; bu turda uygulanmadı)

1. **PROGRESS DR** — cold-start liveness: bounded cold-start recovery + slow-channel safety birlikte
   optimize (4 seçenek, Bölüm 2.4).
2. **SENSOR/MEASUREMENT DR** — karar vektörüne srtt (RTT) + deadline-slack ekle; cold-start için ayrı rejim.
   (Loss sensörü zaten sağlam — ekleme yok.)

---

## 7. KAPSAM, DÜRÜSTLÜK VE KOŞULMAYAN EKSENLER

**Değişmeyenler (hiçbiri değiştirilmedi/açılmadı):** üretim fuzzy-config · budget policy `LeftoverOnly` ·
üretim byte-layout · F15/P15 · DR-1f / DR-1d / Kesit-2-full / Sprint-6 · PR merge. Tüm harness/enjektör/
emit-fuzzy **feature-gated** (`experimental-fec-budget`, default OFF) ve **üretim yoluna bağlı değil**.

**Koşulmayan / kısmi eksenler (sessiz tavan yok — açıkça işaretli):**
- Pin-4 (deficit=5) **wall-clock faithful** koşulmadı (kombinatoryal FK'da var: iso-pending residual-1 +
  hybrid 31/29 FEC/ARQ split).
- **Reverse-path utility ekseni** (blackout/lossy reverse) utility matrisine eklenmedi (harness yeteneği var).
- **Headroom ekseni** (0.4/0.6/0.8C) taranmadı (sabit ~0.6C).
- **§11 forward-recovery izole utility ekseni** ayrıca taranmadı (FK'da bu rejimde §11-dup=0 gözlendi).
- **MarginalFecHarmful** pozitif ölçülmedi (parity byte-waste metriği ayrı; bu koşulda completion'ı
  kötüleştirmedi).
- **Loss-membership eşiği** (fuzzy_ratio=0 @ %30 loss) ayrı kalibrasyon konusu, KAPSAM DIŞI.

**Test kapıları:** `cargo fmt` · `clippy -D warnings` · `cold_start_liveness` (4) · `udp_runner_smoke` (5)
— hepsi yeşil. Faithful harness acceptance 6/6.

---

## 8. COMMIT HARİTASI + VERİ KONUMLARI

| Commit | Ne |
|---|---|
| `501665e` (FJ+FK) | exact-source-deficit enjektör + MDS decode-fix + kombinatoryal pinler |
| `42d4ef0` (FO) | cold-start liveness direct-integration testi + kanıt |
| `9e0a4b5` (FP) | cold-start liveness sınıflandırma raporu (DEFECT) |
| `28f0f6a` (FQ) | faithful two-netns wall-clock harness + acceptance 6/6 |
| `782fddd` (FR) | wall-clock runner (arq_repair_ms + dispatch-gated settle) + sweep script |
| `d57e8ae` (FS) | wall-clock utility boundary raporu + 48-koşu verisi |
| `33803fa` (FT) | loss-signal fidelity raporu + düzeltme + gerçek-loss verisi |
| `86a392c` (FU) | regime observability raporu |
| `6cc56a1` (FV) | 42-çıktı SON-SUNUM + iki hüküm |

**Alt raporlar:** `reports/d1-exit/FP-cold-start-liveness.md` · `FS-wallclock-fec-utility.md` ·
`FT-loss-signal-fidelity.md` · `FU-regime-observability.md` · `FV-final-review.md`.
**Ham veri:** `reports/d1-exit/data/{cold-start, faithful-harness, wallclock, loss-fidelity, observability}/`.
**Analiz araçları:** `scripts/netem/{faithful_harness,faithful_acceptance,run_wallclock_pins,
run_loss_fidelity}.sh` + `{fec_wallclock,loss_fidelity}_analyze.py`.

---

*Bu rapor, bu turda üretilen FJ/FK/FO/FP/FQ/FR/FS/FT/FU/FV çalışmalarının bütünsel sentezidir. Tüm iddialar
lab-verifiable; ham veri ve scriptler yukarıda listelendi. Üretim davranışı değişmedi; iki DR ayrı onaya tabi.*
