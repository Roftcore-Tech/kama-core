# DR-1b-v1 — Parity token/budget politikası [TASLAK — NO IMPLEMENTATION]

> Statü: **TASLAK — NO IMPLEMENTATION** (faz: "DR-1b DESIGN AUTHORIZED — IMPLEMENTATION NOT
> AUTHORIZED"). Kanıt-tabanı: [1b-micro-sweep-1](../d1-exit/1b-micro-sweep-1.md) (MS1) + Commit-N-
> attribution + [rule-coverage-prepass](../d1-exit/rule-coverage-prepass.md) + kesit-1-audit.
> **BAĞLAYICI-SINIR:** 1b pacing-algoritmasını, BtlBw-estimator'ı, queue-ordering'i ve DATA-first-
> sırasını DEĞİŞTİRMEZ-varsayımıyla-tasarlanmıştır; biri-gerekirse-AYRI-explicit-DR.

## A. 30-soru (talimat-§10)

1. **1b'nin-kesin-sorumluluğu:** kuyruklu (emisyon-izinli) parity'nin token-tahsis-POLİTİKASI — ne-zaman/
   ne-kadar-token parity'ye-akar. Planlama (1a-latch) ve emisyon-izni (üyelik) DEĞİL; yalnız-tahsis.
2. **Budget ≠ token:** token = pacer'ın-anlık-byte-bakiyesi (mekanizma); budget = parity'nin-token-
   akışından-alabileceği-pay-politikası (bu-DR'ın-konusu). Bugün budget-politikası = "leftover" (örtük).
3. **Admission ≠ emission — EVET-ayrı:** admission = kuyruğa-kabul (bugün: izin-kapısı-girişte; miktar-
   sınırsız); emission = token-drain. 1b her-ikisine-ayrı-bound-getirir (C-tamamlayıcısı + D-akışı).
4. **Budget-kapsamı:** HİBRİT — admission-cap block-scoped (blok-başına-üst-sınır), akış-tahsisi
   window-scoped (rate-türevi-oran). Salt-block-scoped akışı-kontrol-edemez (MS1-B2: tavan-arz-sınırlı);
   salt-window-scoped blok-adaletini-kaçırır (starve-blok).
5. **DATA-first-korunması:** İNVARIANT — parity-tahsisi DATA-emisyon-sırasını-değiştirmez (plan_fec_parity
   drain_new_data-SONRASI-kalır). Model-D-floor'u sıraya-DOKUNMAZ; yalnız-bounded-token-payı-ayırır
   (DATA o-tick-daha-az-token-görür ama önce-koşar). Kanıt-planı §C.
6. **Work-conserving:** parity-talebi-yokken parity-payı-DATA'ya-kalır (rezervasyon-tembel: yalnız-
   pending_fec-boşken-pay-serbest); leftover-fazlası-parity'ye-akmaya-devam-eder.
7. **Minimum-parity-guarantee:** ÖNERİLEN-EVET — Model-D-floor: blok-close+T_floor-içinde ≥1-parity
   (MS1-counterfactual: starve 0.45-0.59 → 0.00, displacement ≤%4). T_floor [CAL] öneri 2×srtt-taban.
8. **Hard-parity-cap:** kümülatif-parity-byte ≤ H×Σ(rate·dt); öneri H=0.30 [CAL] (MS1-D-modeli).
   Ayrıca-anlık: tick-başına-parity ≤ burst-cap (mevcut-mekanizma-zaten-sınırlıyor).
9. **`pending_fec`-memory-cap:** KESİN-cap [CAL öneri 256-item] — aşımda-yeni-admission-RED+sayaç
   (en-yeni-red; eski-blok-kararı-korunur). SR-AUDIT-001. DR-1f-madde-4'le-ortak.
10. **`manual_r`-clamp:** close_fec_block_manual r ≤ profil-r_code_max (negotiated-profile-bound);
    aşım typed-red-veya-clamp+diag [tasarım-tercihi: clamp+sayaç — legacy-harness-uyumu]. DR-1f-madde-5.
11. **requested-r vs admitted-r AYRIMI — GEREKİR:** admission-cap'le planned≠admitted-olabilir; diag-
    zincirine `admitted_r` eklenir (requested→planned→admitted→enqueued→emitted).
12. **enqueued≠emitted-ayrımı-korunması:** mevcut-sayaç-çifti + Commit-N-attribution AYNEN; 1b-ek-alanı
    yalnız-admitted.
13. **Token-denial-hangi-bloğa:** kuyruk-önü-item'ın-bloğuna (Commit-N-snapshot: block_id+generation —
    UYGULANDI-testli). MS1-ledger: denial=bağlam, kategori-değil.
14. **Underutilized-BtlBw-lock'ta:** 1b-politikası-rate-türevi-kaldığından kilit-düşükse-tüm-paylar-
    küçülür (headroom-körlüğü-SÜRER — bilinçli-sınır). Floor-tabanı [CAL] mutlak-alt-değer (ör. 1-parity/
    T_floor) kilitte-bile-minimum-koruma-verir. BtlBw-düzeltmesi AYRI-DR (MS1-§4-kanıt-dosyası).
15. **Offered>C-overload'da:** leftover→0 (MS1-offered-c1: real-0.01); politika: floor-devrede (bounded-
    displacement), cap-devrede; parity-asla-DATA'yı-sırada-geçmez → FEC-koruması-minimumda-tutulur,
    çöküş-yok. Overload-tespiti-1b-kapsamı-DIŞI (headroom-sensörü kalibrasyon-fazı).
16. **Long-RTT'de-budget-birikimi:** birikim-YOK (window-scoped-akış; token-bucket-burst-cap'li) —
    d1000-tavanı-floor+cap'le-yönetilir; birikimli-borç-modeli-REDDEDİLDİ (queue-şişirir).
17. **Reverse-blackout'ta:** ackconf-düşer; rate-floor'a-iner → parity-payı-da-floor'a; RECOVERED-
    gelmez → cancel-yok → pending-cap-devrede. rev99-MS1-verisi: baseline-real-0.22 → D-modeli-1.00
    (replay-unreliable-şerhiyle; gerçek-doğrulama-implementasyon-fazı-testinde).
18. **Emergency 1b'de-mi-1d'de-mi:** **1d-ile-birlikte-arbitrasyon** (mevcut-interlock-üstte-kalır;
    1b-emergency'de-parity-akıtmaz). Gerekçe: rule-coverage-prepass — emergency∧nonzero-ratio %0.07
    (ölçü-sıfır); ampirik-temelsiz-politika-yazılmaz. Güvenli-tek-taraflı-1b-çözümü-YOK → 1d-şartı.
19. **Dup↔parity-double-spend → 1d'ye-bırakma:** 1b parity-payını-tanımlar; dup (forward-recovery)
    kendi-yolunda-kalır; ortak-ARBITRAJ (aynı-bloğa-dup+parity-harcaması) 1d-DR'ında — 1b-tasarımı
    dup-yolunu-OKUMAZ/değiştirmez (sınır-net).
20. **Manual-mode-korunması:** Manual{true}+floor/cap AYNI-politikadan-geçer (mod-bağımsız-tahsis);
    Manual{false}: talep-zaten-yok. Legacy-desen-testleri (sprint5c/n8/m1) bit-davranış-korunur-şartı
    (flag-OFF-yolunda; §22).
21. **Feature-flag/default:** `BudgetPolicy::LeftoverOnly` (DEFAULT = bugünkü-davranış-birebir) |
    `FloorCap{t_floor, h_cap}` — config-injection (1e-deseni: validated-config). Default-değişimi
    AYRI-onay.
22. **Rollback:** flag→LeftoverOnly tek-adım; kuyruk/karar-state-etkilenmez (yalnız-tahsis-akışı).
23. **Determinism:** sabit-nokta-aritmetik (permille/byte-tam-sayı); RNG-yok; golden-vector (ADR-17)
    politika-fonksiyonuna-eklenir.
24. **Integer-sınırları:** pay-hesabı u64-byte; H×Σrate-birikimi saturating; permille≤1000-clamp;
    overflow-kanıtı-DR-implementasyon-bölümünde (compute_requested_r-emsali).
25. **Queue/memory-safety:** §9-cap + bounded-diag (Commit-N-modeli); unbounded-yapı-YOK.
26. **Telemetry:** admitted_r + floor_granted_total + cap_suppressed_gauge + mevcut-denial-attribution;
    semantik-tablo-güncellemesi-zorunlu.
27. **Property-testler:** (i) Σparity_bytes ≤ H×Σrate+floor-payı; (ii) DATA-önce-sıra-değişmez (decision-
    sırası-karşılaştırma); (iii) work-conserving (talep-yokken-pay-DATA'da); (iv) admission≤cap;
    (v) exhaustive küçük-uzay (rate×talep-grid) determinism.
28. **Adversarial-testler:** starvation-mutasyonu (floor-kapatılırsa d1000-starve-geri-gelir-pin);
    cap-bypass-denemesi; interleaved-blok-floor-adaleti; overload-DATA-koruması; N8-recovered-parity-
    cancel-etkileşimi.
29. **N8/non-regression:** flag-OFF byte-for-byte (mini-DR-protokolü); flag-ON N8-bit-özdeşlik (bw-
    dokunmasız-kanıt) + {A+B+C} + MS1-yeniden-koşusu (before/after-tablo).
30. **Calibration↔policy-sınırı:** 1b SABİTLERİ [CAL]-etiketli (T_floor/H/cap-değerleri) — kesit-2-
    kalibrasyonu-değerleri-ayarlar, MEKANİZMAYI-değiştirmez. Fuzzy-eşikleri-1b-kapsamı-DIŞI.

## B. Model-karşılaştırması ve ÖNERİ (MS1-counterfactual-kanıtlı; onay-değil)

**ÖNERİLEN: Model-D-çekirdeği (minimum-floor + hard-cap) + Model-C-tamamlayıcısı (admission/memory-cap),
mevcut-leftover-akışının-ÜSTÜNE, flag-arkası (default=bugünkü-davranış).** Gerekçe: güvenilir-fidelity-
hücrelerinde ≥baseline-realization; starve-eliminasyonu (0.00, d1000/d2500-yönünde-de); displacement
≤%4-bounded; DATA-first-korunur; determinism-basit. REDDEDİLENLER: **B-micro-bucket** (ölçülen-
parametrelerle real-0.32-0.52/delay-5-8s/starve-0.5-0.8 — bucket-granülaritesi-parity'yi-boğuyor;
farklı-parametre-ayrı-kanıt-ister) · **C-tek-başına** (korumayı-yapısal-yarıya-atar; yalnız-memory-
bound-tamamlayıcısı) · **A-bounded-share** (D'ye-yakın-fayda ama displacement-payı-daha-yüksek-ve-
kontrolsüz-dağılımlı; ikinci-aday) · **E-deadline-aware** (deadline-semantiği/fuzzy-girdisi-tanımsız —
değerlendirilemez; product-policy-olarak-seçilemez). ŞERH: counterfactual contended-hücrelerde
REPLAY-UNRELIABLE — nihai-kanıt implementasyon-fazının-gerçek-motor-testlerinde.

## C. DATA-first-invariant kanıt-planı (§20-talimat-maddesi)

(1) Decision-sıra-testi: aynı-event-akışında flag-ON/OFF DATA-decision-sıraları-birebir (yalnız-parity-
zamanlaması-değişebilir); (2) sıralama-mutasyon-pini (parity-öne-alınırsa-test-düşer); (3) N8-DATA-
bit-özdeşlik (S5-6-emsal); (4) MS1-goodput-DATA-bileşeni before/after.

## D. Açık-riskler

Headroom-körlüğü-sürer (BtlBw-kilidi-ayrı-DR); floor-displacement overload'da-DATA-tail-latency'ye-
ölçülebilir-ama-bounded-etki; counterfactual-fidelity-sınırı (contended-doğrulama-impl-fazına);
emergency-arbitrasyonu-1d-bağımlı (§18).
