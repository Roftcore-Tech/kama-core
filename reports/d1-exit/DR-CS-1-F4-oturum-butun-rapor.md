# DR-CS-1 F4-CLOSURE — OTURUM BÜTÜN RAPORU

**Tüm deneyler · sonuçlar · bulgular · analizler · düzeltmeler · hükümler**

> **BAĞLAYICI ÇERÇEVE:** Bu deney ARQ ile FEC'i karşılaştırmaz. ARQ bütün deney kollarında aktiftir; aynı
> ACK/NACK/PTO/retransmit semantiğini kullanır. Ölçülen tek şey, ARQ substratının üzerine eklenen proaktif
> yapının marjinal faydasıdır. DR-CS-1 bir **cold-start LIVENESS/bootstrap** araştırma-kararıdır — FEC-marjı
> ölçümü değil. "ARQ FEC'i yendi / FEC ARQ'dan iyi" türü ifadeler bu çalışmada YASAKTIR.

**Tarih:** 2026-07-25/26 · **Branch:** `d1-sprint5c` · **PR:** #44 (DRAFT) · **Commit aralığı:** `595a1d6..0a613a0`
**Feature:** `experimental-cold-start` — **DEFAULT-OFF** · **Üretim-etkisi: SIFIR** (flag-OFF byte-özdeş)
**Ürün-alıcı patience-contract:** UYGULANMADI (hüküm bunu yansıtır)

---

## 0. YÖNETİCİ ÖZETİ

Bu oturumda DR-CS-1 (cold-start liveness) kabul-matrisinin **eksik F4-hücreleri** tamamlandı. Altı hücrenin tümü
kanıtlandı; ayrıca **ölçüm-öncesi üç harness-hatası** bulunup düzeltildi ve **adversarial-doğrulama** bir raporlama
hatasını yakalayarak **daha doğru bir bilimsel bulguya** yol açtı.

### Nihai hüküm
> **SENDER-SIDE COLD-START BOOTSTRAP CORE VALIDATED — ACCEPTANCE PARTIAL — [açık hücre: W-3 restore=61 @ 65B —
> harness-gözlem-penceresi] — PRODUCT RECEIVER PATIENCE CONTRACT UNIMPLEMENTED.**

### Hücre-özeti
| Hücre | Sonuç | Kanıt |
|---|---|---|
| **W-1** (OFF-vs-ON, frozen-bound) | **PASS** | ON blok-0 2.7–5.6s (<30s); deadlock **workload-vs-cwnd-bağımlı** |
| **W-2** (exact-once ACK-drop) | **PASS 27/27** | self-heal, spurious=0, dup=0, ~1×RTT |
| **DI-2B** (reverse-blackout) | **PASS** | probe-reach=4 + echo-lost + TimedOut@60s |
| **W-3** (blackout-heal) | **5/6 PASS** | restore=20 (3) + restore=61 (300/1200B); 65B harness-window |
| **DI-10** (rx-caps + O(1) probe-path) | **TAMAM** | g10–g10d + f4_di10 |
| **Mixed-version** | **TAMAM** | g9 5-satır codec |

### En kritik iki içgörü
1. **BUG-H1** (kök-neden yanlış-teşhisi düzeltilerek bulundu): Tüm önceki wire-koşularının `receiver_events=0`
   vermesinin sebebi CPU-yükü değil, `$(runrx)` komut-ikamesinin rx-ömrü-boyunca bloklamasıydı.
2. **W-1 workload-vs-cwnd bulgusu** (adversarial-verify'ın yakaladığı bir hatadan doğdu): Cold-start deadlock'u
   naif "her boyut" değil, **workload'un initial-cwnd'i aşıp aşmamasına** bağlı.

---

## 1. BAŞLANGIÇ DURUMU (devralınan)

Oturuma FC0 planı (`595a1d6`) + plumbing (`06cd303`) + static-ARP harness (`4bb1381`) hazır girildi. Yapılacaklar:
FC1 (W-2), FC2 (DI-2B), FC3 (W-3), FC4 (DI-10), FC5 (mixed-version), FC6 (T_W1_BOUND), FC7 (closure + hüküm).

**Yasaklar (bu tur):** yeni mekanizma/tasarım/codepoint YOK · 0x19/0x1A layout değişmez · 0x12=RECOVERED
dokunulmaz · FEC/DR-1b/fuzzy dokunulmaz · production-receiver-patience YOK · default-flip YOK · PR-merge YOK ·
kırmızı-test-beklentisi-gevşetme YOK · sonuç-görüp-bound-seçme YOK.

**Önkoşul (her faithful-koşu):** acceptance-RTT-pin (=100ms) + provenance-blok. Geçmezse → **INVALID RUN — NO
EVIDENCE.**

---

## 2. METODOLOJİ — FAITHFUL TWO-NETNS WALL-CLOCK HARNESS

Rootless iki-ayrı-network-namespace: `unshare -rnm` (parent A + userns CAP_NET_ADMIN) → child B
(`unshare -n sleep BIG`, PID-referanslı) → `ip link set v1 netns $CHILD` → cross-namespace veth-forwarding
egress-qdisc'i **sadakatle** uygular. (Tek-namespace-veth netem'i sessizce yutar; kanıt: tek-ns RTT≈0.02ms vs
cross-ns RTT-pin=100ms.) Statik-ARP (her iki yön `permanent`) → reverse-blackout'ta forward-teslimin ARP'a
bağımlı olmaması. IP: tx=10.9.0.1:47001, rx=10.9.0.2:47002. Forward=parent-v0-egress, Reverse=child-v1-egress.

**Overlap-gate (kritik acceptance ölçütü):** her koşuda `receiver_events>0` olmalı — rx ile tx gerçekten
örtüştü demektir. `receiver_events=0` → INVALID (non-overlap; kanıt değil).

---

## 3. KOD ÇALIŞMASI

### 3.1 FC5 — Mixed-version 5-satır matris (g9) — `33558d9`
`crates/kama-noise-wire/tests/dr_cs1_wire.rs::g9_mixed_version_matrix`. Codec-seviyesinde (production-yol) 5-satır
çift-sütun (wire_safety + feature_effectiveness):

| # | Yön | wire_safety | feature_effectiveness |
|---|---|---|---|
| 1 | new-ON → old-rx | 0x19/0x1A = UnknownInnerKind → frame-drop, session-YAŞAR | feature-etkisiz (eski-rx anlamaz) |
| 2 | new-OFF → old-rx | 0x19/0x1A wire'a-çıkmaz → mevcut-frame byte-özdeş | N/A |
| 3 | old → new-rx | eski-frame (RECOVERED 0x12) new-rx'te decode | N/A |
| 4 | new-ON → new-rx | 0x19/0x1A tam-roundtrip + classify-Ok | EFFECTIVE |
| 5 | new-OFF → new-rx | mevcut-frame byte-özdeş (RECOVERED-golden korunur) | N/A |

**Karşılaşılan hata + düzeltme:** ilk yazımda `seal()` "non-monotonic wire_pn (nonce-reuse guard)" ile reddetti
(bir protector-instance'tan monotonik-artan wire_pn zorunlu). Tek monotonik sayaç (`pn()` closure) ile çözüldü.

### 3.2 FC4 — DI-10 receiver resource-cap + O(1) probe-path — `3cb006b`
Ürün-alıcı UYGULANMADI; bu kanıtlar DR-CS-1 WIRE-eklerinin (0x19/0x1A) alıcıya **unbounded durum
DAYATMADIĞINI** production-codec + saf-fonksiyon seviyesinde gösterir:

| Test | Kanıt |
|---|---|
| `g10_di10_wire_additions_are_noretry` | probe/echo NoRetry → sender retry-durumu-biriktirmez (O(1)) |
| `g10b_di10_fixed_width_no_amplification` | body alan-değerinden-bağımsız sabit (12B/11B); length-prefix-YOK → wire unbounded-alloc-isteyemez; u32::MAX → aynı-boyut |
| `g10c_di10_probe_flood_must_not_dedup_unbounded` | **ADVERSARIAL:** 1024-probe `InboundControlDedup.admit()`'e → `seen` LİNEER-büyür (unbounded; Sprint-4-TTL-öncesi) → probe admit()'ten-GEÇMEMELİ; stateless-echo O(1) |
| `g10d_di10_echo_idempotent_regeneration` | echo = (probe_seq, ledger)'in saf-fonksiyonu → byte-özdeş-regen |
| `f4_di10_receiver_cold_block_cap` | `cold_admit_new_block` saf-fn: cap+1-adversarial + monotonluk + cap==0-sınırsız |

**Bounding-disiplini (SR-AUDIT-001):** harness rx `cold_block_cap>0` → yeni-blok-slot cap'te **reddedilir**
(bounds-before-alloc; typed-diag `cs_rxcap_diag`; mevcut-bloklar EVICT-EDİLMEZ → sessiz-eviction-YOK). Önerilen
cap: `RX_COLD_BLOCK_CAP=256` (= kama-engine `MAX_PREPARAM_SHARDS` emsali); byte-cap `256×K(8)×UNIT(1200)=2,457,600 B`.

**Kritik DI-10 bulgusu:** `InboundControlDedup.admit()` her control_id'yi **unbounded** `seen: HashSet`'e ekler
(Sprint-4-deferred, pre-existing). Bu yüzden ürün-alıcı probe/echo'yu **stateless-echo yolu** ile ele-almalı
(harness'in yaptığı gibi: fixed 43B-buf, sıfır-map-insert), `admit()` ile DEĞİL.

---

## 4. ÜÇ HARNESS-HATASI (ölçüm-öncesi bulundu, düzeltildi, izole-doğrulandı)

Bu closure'un **ölçüm-bütünlüğü** açısından en önemli bölümü. Hiçbiri üretim-kodu değil; hepsi test-harness.
Her biri, düzeltilmeseydi, tüm wire-kanıtını geçersiz kılardı.

### 4.1 BUG-H2 — DI-2B küçük-workload tx settle-erken-çıkışı — `33558d9`
**Belirti:** DI-2B'de 300/1200B `TimedOut@60000ms` verirken 65B `cs_terminals=none` (2-probe, disarmed=false).
**Kök-neden:** 65B-birimlerde tüm 8-birim pencere ilk-cwnd'e sığar → `workload_done@t~0` → tx, cold-start
give-up'tan (60s) ÖNCE settle-çıkıyordu.
**Düzeltme:** armed-VE-terminal-olmayan cold-start-epizodu = bekleyen-sender-işi → settle-exit bloklanır → give-up
terminal her boyutta gözlemlenebilir. **Harness-gözlem-kapısı; mekanizma (give-up-timer) değişmez; flag-OFF byte-özdeş.**
**Doğrulama:** 65/300/1200B üçü de `TimedOut@60000ms`.

### 4.2 bin_cs'in yanlışlıkla ezilmesi (ara-bulgu)
"Text file busy" hatasından sonra bir `cp` komutu, son-derlenen target'ı (o an flag-OFF binary) bin_cs'e kopyaladı
→ **bin_cs 759336 byte (flag-OFF) oldu**, oysa cold-start için 811000 byte olmalıydı. Byte-boyut kontrolüyle yakalandı,
iki binary doğru-sırayla yeniden-derlendi (bin_cs=811000 ≠ bin_off=759336), izole-di2b-65 ile cold-start-aktif
(`cs_probe_emitted>0`) + overlap (`receiver_events>0`) doğrulandı.

### 4.3 BUG-H1 — `$(runrx)` komut-ikamesi rx-ömrü-boyunca BLOKLAR (KRİTİK) — `a6d199b`
**Belirti:** TÜM wire-koşuları `receiver_events=0` (di2b/w3/w1 hepsi). Yani rx hiç veri almadı.
**İlk YANLIŞ teşhis:** "concurrent cargo build CPU-load contamination". Bu teşhisle matris yeniden-koşuldu — yine 0.
**Doğru teşhis (izole-tekrar-üretim):** `runrx()` rx'i arka-plana atarken YALNIZ stderr'i yönlendiriyordu; rx'in
**stdout**'u `$(runrx)` komut-ikame BORUSUNA bağlı kalıyordu → komut-ikamesi rx ÇIKANA-KADAR (66s) **bloklamıştı** →
tx ancak rx-öldükten-SONRA başlıyordu → **rx/tx hiç örtüşmüyordu**.
- Kanıt: `runrx(){ sleep 6 & echo $!; }` içinde `$(runrx)` → 6s-bloklar; `sleep 6 >/dev/null 2>&1 &` → 0s-döner.
- İzole binary-testi: yönlendirme öncesi `receiver_events=0`, sonrası `receiver_events=9`.
**Düzeltme:** `runrx()` → rx stdout'u da `>/dev/null 2>&1`. **Kök-neden BUYDU; CPU-yükü/netem/binary DEĞİL.**

**DERS:** İlk teşhisim yanlıştı ("load-contamination"). Doğru mühendislik disiplini, bir hipotezi **izole-tekrar-
üretimle** kanıtlamaktı — bu, gerçek kök-nedeni ortaya çıkardı. Yanlış-teşhis raporda şeffafça korunuyor.

### 4.4 BUG-H3 — W-2 yüksek-RTT inter-run FINISH-marker bleed — `85d6a84`
**Belirti:** W-2 matrisinde RTT100 (9/9) geçti; RTT250-r2'den itibaren `receiver_events=0` (cascade).
**Kök-neden (izole-tekrar-üretim — RTT250 3-rep → [8, 0, 0]):** yüksek-RTT'de bir önceki-koşunun FINISH-marker'ları
(`0xFE`), netem-gecikmesiyle SONRAKİ koşunun rx'i bind-olurken varır → yeni-rx ≥3-stale-FINISH görüp erken-çıkar
(`receiver_events=0`) → tx-ölü-rx'e-gönderir → cascade. RTT100'de bleed yok (kısa gecikme; 14-run testi doğruladı).
**Düzeltme:** koşular-arası DRAIN (çift-yön %100-loss + 1.2s-settle) → in-flight boşalır. İzole-doğrulama fix-sonrası:
RTT250 4/4-PASS.

---

## 5. DENEY SONUÇLARI (temiz veri; tüm koşu `receiver_events>0` → VALID)

### 5.1 W-1 — cold-start OFF vs ON (frozen-bound T_W1_BOUND=30s)
`--deficit 1` (her blok'un ilk-source-esi'si ingress'te silinir), `--fixed-r 0` (proaktif-FEC-yok → kurtarım YALNIZ
ARQ), RTT100, w-units=16.

| size | T_ON blok-0 | ON final | OFF recv | OFF final | OFF-deadlock? | cold-start decisive? |
|---|---|---|---|---|---|---|
| 65 | **2706ms** | 2 | **18** | **2** | HAYIR (cwnd'e-sığar) | hayır (ikisi-de-tamamlar) |
| 300 | **5554ms** | 2 | **1** | **0** | **EVET** | **EVET** |
| 1200 | **5603ms** | 2 | **1** | **0** | **EVET** | **EVET** |

**⚠ ÖNEMLİ DÜZELTME (adversarial-verify §7 bulgusu):** İlk-koşuda OFF-tx `--unit-bytes` **atlanmıştı** → 3-OFF-koşu
aslında 1200B-default'taydı (65/300 yanlış-etiketli, hepsi recv=1 görünüyordu). Script düzeltildi + true-size
yeniden-koşuldu (2-rep tutarlı: OFF-65 recv=18/final=2). Bu **deadlock'un naif-size-bağımsız-DEĞİL, workload-vs-
initial-cwnd-bağımlı** olduğunu ortaya çıkardı.

**DOĞRU mekanizma:** OFF'ta first-source-loss, delivered-rate/BtlBw saatini ilkler-ilklemez'e bağlı.
- **Workload initial-cwnd'e SIĞARSA** (65B: 16×65=1040B) → tüm 16-birim ilk-burst'te gider → ACK-clock gerekmeden
  blok tamamlanır (OFF-65 recv=18/final=2, **deadlock YOK**).
- **Workload cwnd'i AŞARSA** (300B=4800B, 1200B=19200B) → dispatch ACK-clocked → first-loss BtlBw-clock'u-ilklemez
  → pacing→0 → initial-cwnd-sonrası STALL → **deadlock** (recv=1/final=0).
- **ON-kolu (cold-start)** bootstrap-RTT'yle saati ilkler → blok-0 her boyutta 2.7–5.6s'te tamamlanır (<30s-bound).

**HÜKÜM: W-1 PASS** — cold-start deadlock'u çözer (kanıt: OFF-300/1200 deadlock ⇒ ON-300/1200 tamamlar). Değer
workload>cwnd rejiminde DECISIVE; 65B'de OFF-de-tamamlar (bu boyutta fayda-yok).

### 5.2 W-2 — exact-once ACK-drop self-heal (RANDOM-LOSS YASAK)
Çift-yön açık (yalnız delay). rx `--drop-first-sack` (İLK non-boş-SACK exact-once suppress). RTT{100,250,1000} ×
size{65,300,1200} × 3-rep = 27 koşu.

**SONUÇ: 27/27 PASS.** Her koşu ÖZDEŞ desen:

| RTT | (drop) | (heal) | (spurious-B) | (dup) | terminal | rx-ev | self-heal-gecikme |
|---|---|---|---|---|---|---|---|
| 100 | true×9 | 1×9 | **0**×9 | **0**×9 | ResolvedByDataAck×9 | 8×9 | ~186ms (~1 RTT) |
| 250 | true×9 | 1×9 | **0**×9 | **0**×9 | ResolvedByDataAck×9 | 8×9 | ~316ms (~1 RTT) |
| 1000 | true×9 | 1×9 | **0**×9 | **0**×9 | ResolvedByDataAck×9 | 8×9 | ~1086ms (~1 RTT) |

**Yorum:** exact-once İLK-SACK-drop, 2nci-cumulative-SACK'le SELF-HEAL eder (blok tamamlanır) — **SIFIR
spurious-retransmit**, **SIFIR duplicate-delivery**. Bootstrap 2nci-SACK'in DataAck'iyle; gecikme ≈ 1×RTT (RTT'yle
lineer: 186/316/1086ms). **Katman-ayrımı (dürüstlük):** in-process DI-2 testi (tek-ACK-erasure → sender spurious-
retransmit BEKLENEN, alıcı-seq-dedup-atar) ile faithful-W-2 (cumulative-SACK-cadence self-heal) **farklı-katman-
tamamlayıcı, çelişki-YOK**. **W-2 27/27 PASS.**

### 5.3 DI-2B — sustained reverse-blackout (sender-terminal)
Forward-açık (delay 50ms) · reverse-blackout (delay 50ms loss 100%). Probe forward'a ulaşır, echo reverse'te kaybolur.

| size | probe_emit | terminal | rx receiver_events | rx cs_probe_rx | rx cs_echo_tx | echo_bytes |
|---|---|---|---|---|---|---|
| 65 | 4 | TimedOut@60000ms | 25 | **4** | 4 | 172 |
| 300 | 4 | TimedOut@60000ms | 6 | **4** | 4 | 172 |
| 1200 | 4 | TimedOut@60001ms | 2 | **4** | 4 | 172 |

**Yorum:** forward-açık → 4-probe her boyutta rx'e ULAŞIR (`cs_probe_rx=4`); rx 4-echo üretir (172B) ama reverse-
blackout'ta HEPSİ kaybolur → gönderen echo-alamaz → give-up@60s `TimedOut`. Faithful DI-2B: **probe-reach +
echo-generated-but-lost + sender-TimedOut**, üç-boyutta tutarlı. `rx_events` boyutla azalır (25/6/2): küçük-birim →
daha-çok ARQ-retransmit datagram'ı sığar (INV-RATE-1: kayıp send_rate'i düşürmez). **DI-2B PASS.**

### 5.4 W-3 — blackout-heal (restore 20s / 61s)
Çift-yön blackout t0..restore → restore'da temizle → heal. `--deficit 1`.

| size | restore | terminal | finalized(heal) | T_heal | rx_events | Hüküm |
|---|---|---|---|---|---|---|
| 65 | 20 | ResolvedByProbeEcho@21143ms | **1** | 21.1s | 14 | **PASS** |
| 300 | 20 | ResolvedByProbeEcho@21102ms | **1** | 21.1s | 10 | **PASS** |
| 1200 | 20 | ResolvedByProbeEcho@21099ms | **1** | 21.1s | 9 | **PASS** |
| 300 | 61 | TimedOut@60000 → PostTimeoutRecovery@63872ms | **1** | 63.9s | 11 | **PASS** |
| 1200 | 61 | TimedOut@60000 → PostTimeoutRecovery@83225ms | **1** | 83.2s | 10 | **PASS** |
| 65 | 61 | TimedOut@60000ms (PostTimeoutRecovery-YOK) | 0 | — | 2 | **PARTIAL** ⚠ |

**⚠ w3-65-61 analizi (harness-gözlem-penceresi kenarı — MEKANİZMA-DEFEKTİ-DEĞİL):** restore=61s > give-up 60s.
65B tüm-pencere ilk-cwnd'e sığar → `workload_done@t~0`. give-up@60s'te cold-start-disarm → küçük-workload settle-exit
@~64s. restore@61s AMA 60s-blackout-boyunca PTO-backoff exponential-büyür → sonraki-PTO-retransmit 61–64s penceresinde
ateşlemez → exit-öncesi heal-olmaz. 300/1200B settle-ETMEZ (dispatch ACK-clocked, blackout'ta tamamlanmaz) → ext-
horizon'a-kadar yaşar → PostTimeoutRecovery-heal (63.9s / 83.2s).
**Dürüst-etiketleme:** bu "kanıtlı-mekanizmanın harness-gözlem-penceresi-kırpılması"dır — 65B'nin doğrudan faithful-
heal'i ÖLÇÜLMEDİ (çıkarım). Ayrıca gecikmeli-PTO, rx `idle_abandon=105s`'i aşarsa always-on-tx bile ürün-receiver-
patience'a (UYGULANMADI) bağımlı olurdu. give-up/PostTimeoutRecovery-DEFEKTİ-YOK; settle-exit büyük-boyutları
etkileyemez. **Heal-mekanizması KANITLI:** restore=20 (3-boyut) + restore=61 (300/1200B) + in-process F3-D pini.
**W-3: 5/6 PASS + 1 harness-window-partial.**

---

## 6. FC6 — T_W1_BOUND CALIBRATION + FREEZE
`reports/d1-exit/T_W1_BOUND-calibration-freeze.md`. Birinci-ilke bileşen-envelope'u (probe-fire 3s + echo-RTT ≤1s +
ARQ-recovery ≤3×RTT + settle + generous-liveness-payı) → **T_W1_BOUND = 30_000 ms**, temiz-W-1-verisi **görülmeden**
commit edildi ("sonuç-görüp-bound-seçme YOK" disiplini). Prior-gözlem (5.5s) tavana FIT-EDİLMEDİ. Ölçülen T_ON
(2.7–5.6s) ≪ 30s → kabul.

---

## 7. ADVERSARIAL DOĞRULAMA (6 bağımsız-skeptik-ajan)

Hüküm-öncesi, 6-ana-iddia bağımsız-ajanlarla **çürütülmeye** çalışıldı (her ajan: "kanıtla ki YANLIŞ; şüphede
refuted=true"). **Sonuç: 0/6 REFUTED — 6-iddia da AYAKTA.** Ama değerli non-refuting bulgular çıktı:

| İddia | Refuted? | Bulgu / eylem |
|---|---|---|
| W1-deadlock-genuine | HAYIR | **YAKALADI: OFF-tx `--unit-bytes` atlanmıştı** → düzeltme + true-size-rerun → **workload-vs-cwnd-bulgusu** (§5.1). Deadlock-GEÇERLİ (300/1200B decisive). |
| DI2B-interpretation | HAYIR | Nit: di2b-1200 TimedOut@60001ms (1ms-jitter, açık). Faithful. |
| W3-65-61-honest | HAYIR | Caveat: "always-on 65B-heal" çıkarım + idle_abandon=105s'e-bağımlı. Etiketleme-inceltildi. Defekt-YOK. |
| W2-selfheal-clean | HAYIR | 27/27 değer tam-tutar. Caveat: provenance dirty=10. |
| code-proofs-hold | HAYIR | Caveat: retry.rs default-arm CS-explicit'i redundant-kılar; g9 old_classify MODEL (açık). 6-özellik non-vacuous, testler-geçer. |
| no-prohibited-actions | HAYIR | İhlal-YOK. flag-OFF-identity 4-pin+cfg-gating'e dayanır. |

**Adversarial-verify'ın kanıtlanmış değeri:** bir raporlama-hatasını (OFF `--unit-bytes`) yakalayarak daha doğru bir
bilimsel bulguya (workload-vs-cwnd) yol açtı. Skeptiklerin "size-independent" varsayımı bile ölçümle çürütüldü
(OFF-65 tamamlar).

---

## 8. INVARIANT + YASAK-UYUM KONTROLÜ

| Kontrol | Durum |
|---|---|
| INV-CS-5 flag-OFF byte-özdeş | ✅ cold_start_liveness.rs 4-pin + tüm-eklemeler cfg-gated |
| INV-CS-7 probe_seq monoton | ✅ cs_probe_seq_wrapped=0 |
| INV-CS-1 wire-emission ayrı-sayaç | ✅ cs_probe_emitted/tx_bytes |
| INV-CS-3 tek-terminal/epoch | ✅ take_cs_terminal (F3 A/B/C/D pin) |
| 0x12=RECOVERED kilidi | ✅ g1/g8 (dokunulmadı) |
| Yeni-codepoint yok (0x19/0x1A onaylı) | ✅ registry {0x01,0x02}∪{0x10..=0x1A} |
| default-flip yok / merge yok | ✅ DEFAULT-OFF; PR#44 DRAFT |
| production-receiver-patience yok | ✅ ürün-alıcı UYGULANMADI (hüküm yansıtır) |

**Testler yeşil:** dr_cs1_acceptance 22 (+3 ignored) · dr_cs1_wire 12 · cold_start_liveness 4. **fmt/clippy temiz.**

---

## 9. NİHAİ HÜKÜM

> ## **SENDER-SIDE COLD-START BOOTSTRAP CORE VALIDATED — ACCEPTANCE PARTIAL — [AÇIK HÜCRE: W-3 restore=61 @ 65B — harness-gözlem-penceresi (small-workload settle-exit + post-blackout PTO-backoff); heal-mekanizması 5/6-config + in-process F3-D'de KANITLI] — PRODUCT RECEIVER PATIENCE CONTRACT UNIMPLEMENTED.**

**YASAK-hükümler İDDİA-EDİLMEDİ:** ~~"COLD-START PROGRESS FIX FULLY VALIDATED"~~ · ~~"END-TO-END PRODUCT FIX
VALIDATED"~~ · ~~"PRODUCTION DEFAULT-FLIP READY"~~.

**Default-flip önkoşulu:** (i) ürün-alıcı patience-contract (RX_COLD_ABANDON) implementasyonu; (ii) §4.4-benchmark.
Her ikisi bu closure-kapsamı DIŞI. **DUR — Abkarada-kararı bekliyor.**

---

## 10. COMMIT LİSTESİ (bu oturum; `d1-sprint5c` → PR#44)

| Commit | İçerik |
|---|---|
| `33558d9` | DI-2B settle-fix (BUG-H2) + g9 mixed-version (FC5) |
| `3cb006b` | DI-10 receiver-cap + O(1) probe-path (FC4) |
| `a6d199b` | **BUG-H1 fix** — runrx stdout-redirect |
| `01728e1` | T_W1_BOUND freeze + closure-skeleton + W-2/extract scripts (FC6/FC7) |
| `1bd9016` | Wire-hücreler doldur — clean-run (FC2/FC3/FC6) |
| `85d6a84` | **BUG-H3 fix** — W-2 inter-run drain |
| `2d790d9` | BUG-H3 dokümante + H1/H2/H3 sıralama |
| `ee37016` | W-2 doldur — 27/27 PASS |
| `0a613a0` | **FC7 finalize** — verdict + adversarial-verify + W-1 workload-vs-cwnd düzeltmesi + evidence |

Push: `f22e644..0a613a0 → origin/d1-sprint5c`. PR#44-comment: `#issuecomment-5080866237`.

---

## 11. DERSLER (kalıcı)

1. **Yanlış-teşhis izole-tekrar-üretimle düzeltilir.** BUG-H1'de ilk "CPU-load-contamination" hipotezi yanlıştı;
   gerçek kök-neden (komut-ikame-blok) ancak izole-tekrar-üretimle bulundu. → [[measurement-tool-first]]
2. **Netem-harness'ta üç görünmez tuzak:** (a) rx/tx-overlap (H1), (b) small-workload gözlem-penceresi (H2),
   (c) inter-run-isolation/FINISH-bleed (H3). Hepsi `receiver_events`-gate ve izole-tekrar-üretimle yakalandı.
   → [[faithful-netem-harness]]
3. **Argüman-paritesi:** OFF-arm bir CLI-arg'ı (`--unit-bytes`) atlayınca sessizce yanlış-boyut ölçüldü;
   adversarial-verify yakaladı → **daha iyi bilim** (workload-vs-cwnd). Karşılaştırma-kollarında birebir-arg-eşitliği
   kritik. → [[olcum-teshis-dogrulama-ayrimi]]
4. **Binary-provenance:** "Text file busy" sonrası stray-`cp` yanlış-feature'lı binary üretti; byte-boyut-kontrolü
   yakaladı. Her koşu-öncesi binary-kimlik doğrulanmalı.
5. **Frozen-bound disiplini:** T_W1_BOUND, veri-görülmeden commit edildi → "sonuç-görüp-bound-seçme" önlendi.
6. **Dürüst-partial > sahte-full:** w3-65-61 heal-etmedi; "tuning-to-pass" yerine harness-window olarak dürüstçe
   raporlandı → ACCEPTANCE PARTIAL. Hüküm, ürün-receiver-patience'ın uygulanmadığını da açıkça taşır.
