# m45-kanal-durumu-v1 — Stale-btlbw Reconciliation (TUR-D1) + Kanal-Durumu-Sensing (TUR-S) [TASLAK r3 — ONAY BEKLİYOR]

> **Statü:** D0-TASARIM-NOTU. Mimari-çekirdek GO (Abkarada r3-talimatı); TUR-D1-kod bu-r3-onayına
> bağlı. **ROL-DARALTMASI (talimat, belge-geneli):** TUR-D1 = **"persistent-underdelivery
> karşısında stale-high-btlbw demotion/reconciliation"** — "kesin-kanal-daralması-tespiti" DEĞİL.
> Gerekçe: R_sample ≈ min(C_path, R_offer, R_cwnd, R_pacing, R_ack-clock) — düşük-delivery ALTI
> nedenden gelebilir; iki-kapı (app_limited+gain) yalnız İKİSİNİ eler; kendi-aritmetiğimizde bile
> min_cwnd-bağlayıcı (Little-kısıtlı-sender). Mekanizma "kanal-daraldı" DEMEZ; "btlbw birden-fazla
> güvenilir-teslim-olayında gerçekleştirilebilen-delivery'den ciddi-yüksek + eski-max kontrol-
> sistemine zararlı → konservatif-aşağı-al" der. Bu daraltma M4-beklememezliği GÜÇLENDİRİR
> (kanalın-NEDEN-yavaşladığını çözmüyor; eski-max'ın sonsuz-kontrol-zehirlemesini kesiyor).
> Yeni-kapı (offered-load) EKLENMEDİ — iddia daraltıldı (iki-review hemfikir).
>
> **r2→r3:** HIGH-1 atomik-event-verdict + HIGH-2 generation-izolasyonu (kod-öncesi-zorunlu-ikili);
> "ardışık"→"ardışık-eligible-evidence-olayı"; S1-S5 kararları işli; §6-temizlik (A:plato-kalıntısı-
> silindi, B:P5-GATE-{100×12}-tek-resmi, C:DESEN-2-iki-metrik); 7-ek-vektör; EK-1-aritmetik-teyit
> (dört-sayı yeniden-türetildi, ikinci-hata YOK) + EK-2-S5-örtüşme-ölçümü + EK-3-borç-kaydı.

## ROL-TABLOSU (talimat)

| mekanizma | gerçek-rol |
|---|---|
| TUR-D1 | stale-btlbw estimator-hijyeni / underdelivery-reconciliation |
| TUR-S / M4+M5 | gerçek-kanal-rejimi + kopma↔yavaşlık ayrımı |
| M3 | yanlış-estimator-bilgisinin motor-geneli hasar-sınırlaması |

## 0. İKİ-TUR AYRIŞIMI [kısıt-b; r2'den — GO]

TUR-D1 (reconciliation, M4-BEKLEMEZ, önce) · TUR-S (kopma-sensing, E-FULL'lu, sonra). Dört
kod-gerekçe korunur (kanıt-altyapısı-hazır; B2-meta-kök-bulaşmaz — örnek-varlığı-vs-yokluğu;
öz-referans-DÜZELTİCİ-anti-ratchet; mevcut-savunmalar-DESEN-1'e-kör: Emergency-sessizlik-anahtarlı/
conf=1000, fuzzy-tabanı-0.5×-yetmez, Little-freni-aşağıda). Etiket: TUR-D1 §0.30-madde-1'in
dar-önalımı; madde-2 (kopma-ayrımı) TUR-S'de; **"sensing çözüldü" iddiası YASAK.**

## 1. NEDEN-ZORUNLU [kısıt-a; r3 EK-1-TEYİTLİ aritmetik]

**EK-1 yeniden-türetme (traj_dv3_3A2, t=38-70, n=63 — dört-sayı bağımsız-teyit, ikinci-hata YOK):**
```
rate×srtt = 4781-4799 B ≈ min_cwnd=4800  → bağlayıcı-terim min_cwnd (2×BDP=631B < 4800)  ✓
rtprop    = 76574 µs SABİT (flood-boyunca; 1.0-1.7s değerleri t≥122 çökme-sonrası-reseed) ✓
srtt*     = min_cwnd/bw_yeni = 4800/375 = 12.8s (teorik-denge)                            ✓
srtt-eğim = 0.92 s/s lineer-fit (ölümsüzlük-eşiği 1/K_BW=0.125; 7.4×) → cutoff GERİLER    ✓
```
Ölçülen srtt MONOTON-TIRMANIR (plato yok) → denge-bile-kurulamayan kaçak-tırmanış; kırılma yalnız
dış-olayla (t=120 loss). **Reconciliation-zorunluluğu ölçülü.** Asimetri epistemik (yüksek-örnek =
kapasite-kanıtı, 0-örnek-gecikmeli girer; düşük-örnek = kanıt-değil, dizi-düzeyi-çıkarım gerekir;
`delivery≫btlbw` yapısal-boş) → tek-yönlü mekanizma; genişleme-KOD-yolu (bw.rs:170-194 + ProbeUp
sender.rs:920-976 + rate.rs:37) **BİT-DEĞİŞMEZ (diff-stat-doğrulama zorunlu).**

## 2. KISIT-C MATRİSİ [r2'den — GO]

UNDERDELIVERY/daralma → yerel (bw-domain boyutsuz-oran; sebat = eligible-evidence-OLAY-sayısı) ·
KOPMA → E-FULL-GEREKLİ (tüm-canlı-saat-adayları srtt-türevli: 3×srtt-b2-kanıtlı-körlük, EAI∝srtt-
zinciri, ack_epoch-iki-dalı-kirli; sınır-bölge-ayrılamaz + return_bw-dürüst-saat + yön-atfı) ·
GENİŞLEME → DOKUNMA-listesi (ProbeUp ölçülü-mükemmel).

## 3. TUR-D1 MEKANİZMASI [r3 — HIGH-1 + HIGH-2 işli]

```
── ÖRNEK-KAPILARI (geçemeyen = INELIGIBLE → ATLA, sayaç-korunur [F4]) ──
  !sample.is_app_limited                                   (bw.rs:30 kablolu)
  ∧ gönderim-ANI-damgalı gain ≥ 1                          [F3: SentPacket faz-alanı; tombstone-kol-1 taşır]
  ∧ sent_ns ≥ olay-BAŞI last_raise_ns-snapshot'ı            [F2 yaş-kapısı; HIGH-2-kural-v]
  ∧ TAZE-SNAPSHOT (delivered_time bu-olaydan-önce tazelenmiş) [F1: kopma-yayılımlı örnek → TUR-S-teşhis-sayacına]

── ATOMİK-EVENT-VERDICT [HIGH-1 — loop-order-bağımsız; kod-öncesi-ZORUNLU] ──
  B_ref = olay-BAŞI btlbw-snapshot;  G_ref = olay-başı generation
  if olay-içinde btlbw yükseldi:            verdict = BASELINE_RAISED  → count=0, generation++
  elif ∃ eligible: sample ≥ B_ref/K_RATIO:  verdict = COUNTER_EVIDENCE → count=0
  elif ∃ eligible (hepsi düşük):            verdict = LOW_EVIDENCE     → count += 1  (olay-başına-max-1)
  else:                                     verdict = SKIP             → count-korunur
  (yüksek-örnek düşüğe ÜSTÜN: batch eski-eşik-üstü-delivery ürettiyse aynı-batch-düşükleri
   daralmayı-kanıtlamaz — pozitif-kapasite-kanıtı-dominance)

── GENERATION-İZOLASYONU [HIGH-2 — kod-öncesi-ZORUNLU] ──
  btlbw_generation += 1  her gerçek-max-yükselişte VE her reset/reseed'de
  (i) her btlbw-yükseliş sayacı KOŞULSUZ-sıfırlar; (ii) reset/reseed = yeni-generation;
  (iii) sayaç yalnız AYNI-generation olaylarından birikir; (iv) expansion olay-içinde ÖNCE-atomik
  belirlenir (aynı-olay hem-raise-hem-shrink OLAMAZ); (v) yaş-kapısı olay-başı-ÖNCEKİ
  last_raise_ns-snapshot'ına göre (yükselişi-yapan-örnek kendi-olayında-geçersizleşmez).

── ATEŞLEME ──
  K_SHRINK [CAL {3,5}] **ardışık-eligible-evidence-olayı** (LOW_EVIDENCE) — sayılan ARDIŞIK-ACK-
  olayı DEĞİL: arada app-limited/gain<1/bayat/SKIP olabilir, sayaç-korunur. İki-sonuç beyanlı:
  sayaç-uzun-yaşar + tek-tek-blackout-dönüşleri epizotlar-boyunca-birikebilir → rol reconciliation
  olduğundan KABUL (uzun-underdelivery sonunda-demotion'a-yol-açabilir); "kesintisiz-daralma-
  kanıtı" ADLANDIRILMAZ. Her-olayda ≥1 eligible-örnek < B_ref/K_RATIO [CAL {2,4}].

── AKTÜATÖR (yalnız-düşürür) ──
  btlbw-filter RESET → pencere := AYNI-generation ∧ verdict=LOW_EVIDENCE olaylarının
  eligible-düşük-örneklerinin MAX'ı [S2-daraltılmış: tek-örnek-reseed-yok, outlier'a-çökme-yok]
  generation++ · diag_shrink_resets++ · TRAJ "shrink="
  [S4-kesin-semantik] reset-sonrası next_probe_eligible_epoch = current_epoch + 1
```
Kopma-dönüş-analizi (F1+verdict-sonrası): dönüş-SACK'i en-fazla-1-LOW sayar (bayat-snapshot'lılar
zaten INELIGIBLE→TUR-S-sayacına); sonraki-olaylar taze-örneklerle kanal-gerçeğini ölçer — aynı-hız-
dönüşte COUNTER_EVIDENCE→sıfırlanır (p6b-tekrarlı-kopma K_SHRINK'e ulaşamaz); gerçek-daralma-üstü-
dönüşte ateşleme DOĞRU (reconciliation-rolü).

## 4. S-KARARLARI (r3-talimat-birleşik)

- **S1 — bu-turda-GİRMEZ + EK-3-BORÇ-KAYDI:** W_BTLBW=8×ack_epoch'un srtt-dalı daralma-rejiminde
  anti-fix (ölümsüzlük-kökü). Dedektör-reset SEMPTOMU-keser, KÖK-DURUR → **dedektör-reset'e
  kalıcı-bağımlılık-riski AÇIK-BORÇ** (M3-decoupling veya W-türetim-turunda kapanır; "dedektör-var-
  diye-W-fix-gereksiz" SANILMAZ). Ölçüm-sonrası-soru: explicit-shrink-reset varken dinamik-W hâlâ
  gerekli mi, normal-eskime için sabit-epoch yeter mi? [r2'deki "§4-S2" atıfı yanlıştı → S1; düzeltildi]
- **S2 — recent-max ONAY, daraltılmış** (§3-aktüatöre işlendi).
- **S3 — TUR-S ayrı-D0 ONAY** (E-FULL-return_bw + ACK-sessizlik + cadence + yön-atfı + durum-
  makinesi); "sensing-çözüldü" TUR-D1-sonrası İDDİA-EDİLMEZ.
- **S4 — koşullu-EVET, kesin-semantik** (§3'e işlendi: next_probe_eligible_epoch=current+1).
  **DARALTMA:** expansion-estimator-FORMÜLÜ bit-aynı AMA ProbeUp-GİRİŞ-zamanlaması bilerek-değişir
  → davranış-bütünü bit-identical-DEĞİL. **KIRMIZI: DESEN-2 estimator-ramp 13×/~1s bozulursa S4-geri-al.**
- **S5 — KABUL, kapsam-GENİŞLETİLMİŞ:** düşük-delivery path-capacity-düşüşü-dışında sender-offer/
  cwnd/rate-sınırından-da gelebilir; TUR-D1 bunları epistemik-ayırma-iddiasında-DEĞİL; persistent-
  underdelivery karşısında stale-high-btlbw'yi konservatif-reconcile-eder; gerçek-neden DV-3/TUR-S-
  sınıflandırmasına bırakılır. EK-2 (§5) örtüşme-maliyetini sayısallaştırdı.

## 5. EK-2 — S5-KÖR-NOKTASI × FLOOD-PENCERESİ ÖRTÜŞME (kod-beklet-şartı; SONUÇ)

- **Ampirik (DESEN-1-traj):** bol-backlog'da S5-vakası OLUŞMADI — gönderim-duraksaması ≥0.5s-üstü
  0-adet/en-uzun-0.5s (sent_u sürekli-arttı → app_limited=false-baskın). Ölçülen-63×-flood
  app_limited=false altında oluştu → r3-dedektörü o-pencerede eligible-örnek görür (yakalanabilir-sınıf).
- **Analitik-tavan (arz-sınırlı-vaka; kanal=375B/s, min_cwnd=4800, SACK-kadansı≈6.4s, K_SHRINK=3):**
  susma = backlog-kurulum + 3-olay-kadansı; flood-büyümesi ARZ-SINIRLI (pacing-hedefi-değil):
  `arz=600 → ~33s / ~27-birim · arz=1000 → ~22s / ~52-birim · arz=2000 → ~20s / ~120-birim`
  (ÜST-sınır — Little-decay gerçek-büyümeyi düşürür). Mertebe ölçülü-63-birimle AYNI-SINIF →
  **kalıntı ihmal-edilemez ama TAVANLI ve arz-oranlı.** Hüküm-önerisi: yedek-tetik bu-turda GEREKMEZ
  (rol-daraltmasıyla tutarlı: reconciliation gecikebilir, motor-yaşar; KIRMIZI-şartlar korunur).
- **Ölçüm-aracı-eksiği (beyan):** tam-ampirik-örtüşme-ölçümü **paced-arrival bench-modu** ister —
  bench'te YOK (client upfront-gönderir). Kod-GO-kapsamına motor-dışı ölçüm-aracı-kalemi önerisi:
  `bench client --paced-arrival <B/s>` → §6-planındaki paced-vektör gerçek-koşuyla doğrulanır.
  Abkarada-onayına.

## 6. ÖLÇÜM-PLANI [r3 — temizlik A/B/C + 7-ek-vektör]

**Birim (7-ek-vektör):** (1) mixed-batch-dominance (düşük+yüksek-eligible aynı-ACK → yüksek-kazanır,
count=0); (2) same-event-expansion (raise-kazanır, generation++, count=0); (3) generation-isolation
(LOW,LOW→raise→LOW → trigger-YOK); (4) skip-persistence (LOW,LOW→uzun-SKIP→LOW → beklenti-AÇIK-
pinli: reconciliation-rolünde koruma-kabul, belgeli); (5) **B2-only-artifact** (kapasite-sabit,
yalnız-srtt-şişer → D1-tetiklenmezse temiz; tetiklenirse false-demotion-recovery ≤S4-epoch —
B2-artefaktının-daralma-sanılma-maliyeti); (6) ACK-split/batch-invariance (aynı-teslim-dizisi
tek-cumulative / üç-ayrı / gecikmeli → karar makul-sınırda-değişmez); (7) salınımlı-kanal 40k↔3k
(shrink→1-epoch-probe→shrink döngü-salınımı + gereksiz-reset-sayısı). + r2-seti: daralma-ateşler /
app-limited-ATLAR / gain<1-ATLAR (**anti-vacuous-kurgu: Drain'de-gönder-Steady'de-ack'le** — faz-
SINIRI-geçen-ack) / kopma-sessizlik / p5-kalıcı-kayıp-ateşlemez.
**Sistem:** DV-3A2/3B2-yeniden — ana-sınav [A: plato-ifadesi-silindi]: **srtt-runaway-eğimi
0.84-0.92 s/s-sınıfından-çıkar; shrink-reset-sonrası srtt/rate-kapalı-döngü-kaçağı kesilir veya
ölçülü-üst-sınıra-iner.** **DESEN-2 İKİ-METRİK [C]:** estimator-ramp (ProbeUp-girişinden 13×/~1s
DEĞİŞMEZ) + end-to-end-recovery (shrink-reset→eski-kapasite; S4-ile ≤~1-epoch-hedefi) — salt-ramp
S4'ün-asıl-etkisini-ölçmez. + paced-arrival-vektörü (§5, araç-onaylıysa).
**KIRMIZI (tek-resmi-set [B]):** **P5-GATE {100×12}** (iki-bağımsız-6'lık-seri; p5×6-tek-seri
ifadesi kaldırıldı) · DESEN-2-ramp-13×/~1s · temiz-1k=45.
**Kapsam:** bw.rs/sender.rs-daralma-yolu + rate.rs + testler; genişleme-yolu BİT-DEĞİŞMEZ
(**diff-stat-doğrulama zorunlu**).
