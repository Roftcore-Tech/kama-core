# KAMA ADAPTIVE FEC — MARGINAL FEC UTILITY BOUNDARY + CONTROLLER OBSERVABILITY (FJ–FN)

> **Bu deney ARQ ile FEC'i karşılaştırmaz. ARQ bütün kollarda aktiftir; ölçülen şey ARQ üzerine eklenen
> proaktif FEC'in marjinal faydasıdır.** ARQ/substrat (§9-retransmit + §11-forward-recovery) hiçbir kolda
> kapatılmadı. `r0=ARQ+0`, `r1=ARQ+1`, `r2=ARQ+2`, `r4=ARQ+4` — tek değişen, ARQ-substratının ÜSTÜNE
> eklenen proaktif-FEC-parity miktarıdır (`r`). FEC ARQ'yu ASLA değiştirmez; FEC'in kapatamadığı residual
> DAİMA ARQ ile tamamlanır (hibrit). YASAK yorumlar: "ARQ FEC'i yendi", "FEC ARQ'yu yendi", "ARQ tamamlıyor
> → FEC gereksiz", "FEC faydalıysa ARQ kapalı olmalı". Böyle bir yorum analizi geçersiz kılar.

İki-eksenli karar modeli: **Utility ekseni** (proaktif-FEC'in marjinal decode-faydası var mı, hangi
koşulda) × **Observability ekseni** (mevcut fuzzy girdileri o sınırı karar-anında görebilir mi).

---

## 0. YÖNETİCİ ÖZETİ (hüküm)

**HÜKÜM: FEC UTILITY BOUNDARY IDENTIFIED (kombinatoryal) — CONTROLLER OBSERVABILITY BLOCKED;
SENSOR/MEASUREMENT DR REQUIRED.** Ek olarak: wall-clock LATENCY-utility bu harness'ta ölçülemez
(rootless-netem sadık-gecikme-uygulamaz) → ikinci, ayrı bir tooling-DR.

- **Utility sınırı (kombinatoryal, DELAY-BAĞIMSIZ, mekanik-pinli):** proaktif-FEC-parity `r`, deficit `d`
  erasure'ı ancak `r ≥ d` iken first-flight'ta (round-trip'siz) decode-edilebilir kılar — MDS:
  `(K−d)+r ≥ K ⟺ r ≥ d`. 3 mekanik-pin (d=1/2/4) FEC-izolasyonunda **temiz ayrıştı** (r<d → PENDING;
  r≥d → FEC-COMPLETE). Harness GEÇERLİ.
- **Utility'nin wall-clock payoff'u (latency) ölçülemedi:** FEC'in tek gerçek marjinal-faydası
  "substrat-round-trip'ini beklemeden decode" — bu ancak `substrat-onarım-gecikmesi > deadline` rejiminde
  görünür. (i) rootless-netem sadık-gecikme uygulamaz (ping RTT≈0.02ms; qdisc-yapılandırılsa-da), (ii)
  ölçülebilir düşük-RTT rejiminde ARQ-substratı erasure'ı aynı-hızlı tamamlar → marjinal-latency-fayda=0.
  Yani utility'nin VARLIĞI kombinatoryal-kanıtlı; MİKTARI (latency) bu harness'ta ölçülemez.
- **Observability BLOCKED:** karar-öncesi `FusionInputs` FEC-utility'yi belirleyen rejimi (deficit-derinliği,
  RTT-vs-deadline oranı) göremez. `loss_windowed_ppm` = 0 (1497/1497 event, blackout30 dahil — önceki
  FE/FF kanıtı). Ne per-blok-deficit-sinyali ne de RTT/deadline-oranı girdi-vektöründe var. Bu bir
  fuzzy-kalibrasyon sorunu DEĞİL; bir SENSÖR/ÖLÇÜM sorunudur → ayrı DR.

**Sonuç eylemi:** Üretim fuzzy-config DEĞİŞMEZ (zero-biased controller korunur — koşul-kapsamlı,
GLOBAL-DEĞİL). Kalibrasyon İLERLEMEZ; önce sensör/ölçüm-DR (loss-signal-fidelity + regime-observability).

---

## 1. FJ — GERÇEK-YIĞIN ZAMANLAMA FİZİBİLİTE PROBU (TimingNonSeparating)

**Amaç:** `earliest_fec_decode < deadline < earliest_arq_repair` zaman-zincirinin gerçek-yığında
kurulabilirliğini kanıtlamak (yalnız-zamanlama probu; sonra DONMUŞ karşılaştırmalı-deadline).

**Bulgu — kurulaMAZ (harness-kısıtı, FEC-tasarımı-değil):**

| Deneme | Sonuç |
|---|---|
| Tek-namespace veth + `tc netem delay 600ms×2` | ping RTT = **0.020ms** — gecikme UYGULANMADI |
| Cross-namespace netem (gerçek gecikme) | `/var/run/netns` mount-izni-yok → **root'suz-imkansız** |
| Sonuç | Wall-clock `RTT > deadline` rejimi bu harness'ta KURULAMAZ |

Bu, önceki netem-çalışmasının zaten bildiği kısıttır (`run_rootless_netns.sh` başlığı:
"netem = YÖN-doğrulama, packet-birebir-değil"). `ENV.txt`'e RTT-kanıtı yazıldı (0.017–0.024ms).

**Karar (FJ):** Wall-clock zamanlama-ayrışması **TimingNonSeparating** (harness-limitli). Zamanlama
kanıtı yerine mekanik-ayrışma **DELAY-BAĞIMSIZ KOMBİNATORYAL** özellik üzerinden kurulur (FK). Wall-clock
latency-avantajı YAPISAL-COROLLARY olarak belirtilir (FEC=0-round-trip proaktif; substrat-onarımı ≥1
round-trip reaktif/ack-conf-tetikli), ÖLÇÜLMEZ.

---

## 2. FK — EXACT SOURCE-DEFICIT ENJEKSİYONU + MEKANİK PİNLER

**Enjektör (test-only, feature-gated `experimental-fec-budget`):** rx-ingress'te, ağdan-SONRA ama
FEC-decoder/ARQ-işlemesinden-ÖNCE, `b ≥ deficit_from_block` bloklarında ilk-uçuş source-esi `< deficit`
düşürülür (gerçek-erasure; rx'e-hiç-girmez). Üretim-yolu ETKİLENMEZ (yalnız-harness).

**İki mod:**
- **FEC-IZOLASYON (`--drop-repair`):** silinen-esi'nin substrat-onarımı (0x01 §9-ARQ + 0x03 §11-dup) da
  düşer → blok YALNIZ first-flight (kalan-source + parity) ile tamamlanabilir → **saf-FEC-decodability**.
- **HİBRİT (default):** substrat-onarımı geçer → r<deficit'te ARQ/§11 residual'ı tamamlar.

**MDS decode-modeli (DÜZELTİLDİ — bkz. §5 kritik-bulgu):** distinct = |source-esi| + |parity-shard|.
`distinct ≥ K` → recoverable. `decode_via`: `src_full(=K source)` → 0 (substrat), aksi → 1 (FEC-parity
K-distinct'i first-flight'ta tamamladı).

**Warmup gerekçesi (FM-girdisi):** ilk-PAKET-kaybı cold-start'ı (PreConverge; PTO-donuk; 1-birim-floor)
FEC'ten-BAĞIMSIZ deadlock'a sokar (tx ilk-source-ACK'ini alamaz → dispatch durur). Bu yüzden deficit
yalnız warm-steady bloklara uygulanır. **Bu bir gözlemlenebilirlik bulgusudur: FEC-utility rejimi ancak
steady-state'te var/ölçülebilir; cold-start'ta kayıp-türünden-bağımsız yapısal-başarısızlık baskındır.**

### Pin sonuçları (2 rep × 30 deficit-blok = 60 blok/hücre)

**FEC-IZOLASYON — mekanik-ayrışma kapısı:**

| Pin | deficit | ARQ+r (düşük) | ARQ+r (yüksek) | Ayrışma |
|---|---|---|---|---|
| Pin-1 | 1 | r=0 → **60 PENDING** (7<8, FEC-imkansız) | r=1 → **60 FEC-COMPLETE** (7+1=8) | **SEPARATES ✓** |
| Pin-2 | 2 | r=1 → **60 PENDING** (6+1=7<8) | r=2 → **60 FEC-COMPLETE** (6+2=8) | **SEPARATES ✓** |
| Pin-3 | 4 | r=2 → **60 PENDING** (4+2=6<8) | r=4 → **60 FEC-COMPLETE** (4+4=8) | **SEPARATES ✓** |
| Pin-4 | 5 | r=4 → **60 PENDING** izolasyonda (3+4=7<8; residual-1) | — | (hibrit gerekir) |

FEC-COMPLETE bloklarının tümünde `decode_via=1` (FEC) ve `first_repair_ms ≥ 0` (parity-arrival zamanı
yakalandı; 60/60). `repair_kind=0` (izolasyonda substrat-onarımı yok → saf-FEC).

**HİBRİT — substrat residual'ı tamamlar:**

| Pin | deficit,r | Sonuç | Sınıf |
|---|---|---|---|
| Pin-1 | d1,r0 | 60 complete, via0, repair=§9-ARQ | HybridResidualArqRequired (FEC+substrat) |
| Pin-1 | d1,r1 | 60 complete, via0, repair=§9-ARQ | NoMarginalFecUtility_thisCondition (substrat düşük-RTT'de aynı-hızlı tamamlar) |
| Pin-4 | d5,r4 | 60 complete, **via0=31 / via1=29** | Hibrit: r=4-parity ARQ-residual'ı 5→1'e indirir |

**Kapı hükmü:** Pin-1/2/3 mekanik-ayrıştı → **HARNESS GEÇERLİ** ("FEC UTILITY HARNESS INVALID" tetiklenMEDİ).

**Substrat-karakterizasyonu:** tüm hibrit-onarımlar §9-ARQ (repair_kind=1, toplam 420); §11-forward-recovery
(0x03) bu rejimde gözlenMEDİ (rk_dup=0). Substrat bu koşulda ARQ-baskın.

---

## 3. FL — MARGINAL FEC UTILITY BOUNDARY (utility-sınıfları)

**Sınır (kesin, kombinatoryal):** deficit `d` erasure için proaktif-FEC-parity `r`'nin marjinal
decode-faydası **`r ≥ d`** iken vardır (MDS `(K−d)+r ≥ K`). Bu DELAY-BAĞIMSIZ ve mekanik-pinli.

**Utility-sınıfları (koşul-kapsamlı):**

| Sınıf | Koşul | Kanıt |
|---|---|---|
| `MarginalFecDecodable_isolation` | `r ≥ d` VE substrat-onarımı-yok (first-flight-only) | Pin-1/2/3 yüksek-kol: FEC-COMPLETE |
| `FecInsufficient_isolation` | `r < d` | Pin-1/2/3 düşük-kol + Pin-4: PENDING |
| `HybridResidualArqRequired` | `r < d` VE substrat-aktif | Hibrit düşük-kol: substrat tamamlar |
| `NoMarginalFecUtility_thisCondition` | `r ≥ d` AMA düşük-RTT (substrat aynı-hızlı) | Hibrit yüksek-kol: substrat via0 tamamlar |

**Kritik-nüans (dürüstlük):** utility'nin VARLIĞI kombinatoryal-kanıtlı (`r ≥ d` → round-trip'siz-decode).
Ama FEC'in **wall-clock-payoff'u** (bant-genişliği harcayıp substrat yerine FEC-seçmenin gerçek-nedeni)
ancak `substrat-onarım-gecikmesi > deadline` rejiminde materyalize olur. Ölçülebilir düşük-RTT rejiminde
substrat (ARQ) erasure'ı aynı-hızlı tamamlar → `NoMarginalFecUtility_thisCondition`. Yüksek-RTT rejimi
harness'ta kurulamadı (FJ). **Dolayısıyla `MarginalFecEssential` (latency) sınıfı bu harness'ta POZİTİF
ölçülemedi; yalnız kombinatoryal-decodability sınırı kesindir.**

**Ek boyut — §11-forward-recovery:** substrat yalnız reaktif-ARQ değil; ACK-bağımsız proaktif-duplication
(§11) da içerir. §11 tek-erasure'ları round-trip'siz-proaktif kurtarabildiğinden, düşük-deficit'te
proaktif-FEC'in marjinal-faydası ayrıca §11 ile ÖRTÜŞÜR. (Bu rejimde §11 tetiklenmedi ama mekanizma
aktif — gelecekteki utility-analizi bunu izole etmeli.)

---

## 4. FM — CONTROLLER OBSERVABILITY (karar-anı ayrışabilirlik)

**Soru:** karar-öncesi `FusionInputs` MarginalFecDecodable (`r≥d`) rejimini `FecInsufficient`'ten
ayırt edebilir mi? Yani controller "şu an FEC-parity-eklemek marjinal-fayda-sağlar" durumunu görebilir mi?

**Girdi-envanteri (`diag_fusion_input_snapshot`):** `loss_windowed_ppm`, `qd_ratio_permille`, `jitter_ppm`,
`ack_confidence_permille`, `bandwidth_bps`.

**Bulgular:**

1. **Loss-signal fidelity SIFIR:** `loss_windowed_ppm = 0`, 1497/1497 event (blackout30 profili dahil —
   önceki FE/FF kanıtı). Controller kayıp-yükünü göremez. Deficit-derinliği (`d`) hiçbir girdiye yansımaz.
2. **RTT/deadline-oranı girdi-vektöründe YOK:** utility'yi belirleyen "substrat-round-trip > deadline mi"
   sorusunu ayıracak bir girdi yok (`srtt_us` diag'ta var ama fusion-input değil; deadline karar-modelinde
   temsil-edilmiyor).
3. **Cold-start-vs-steady:** utility-rejimi ancak steady-state'te var; cold-start'ta (ilk-paket-kaybı)
   kayıp-türünden-bağımsız yapısal-deadlock baskın (FK-warmup-bulgusu). Controller cold-start'ta ne
   utility'yi ne de deadlock'u FusionInputs'tan ayırt edebilir.

**Observability-sınıfı: `UnobservableWithCurrentFusionInputs` + `Confounded`.** Karar-öncesi girdiler
FEC-utility sınırını (deficit-derinliği × RTT/deadline-oranı) göremez.

**Loss-signal-fidelity kararı (direktif-kuralı):** utility VAR (kombinatoryal-kanıtlı) ama girdiler
ayrıştırMIYOR → **"FEC UTILITY EXISTS — CONTROLLER OBSERVABILITY BLOCKED"**. Bu bir fuzzy-tuning işi
DEĞİL; ayrı bir **SENSÖR/ÖLÇÜM DR**'ı (loss-signal-fidelity onarımı + regime-observability girdisi).

---

## 5. KRİTİK KORREKSİYON — MDS decode-model wire-kind hatası (yakalandı+düzeltildi)

İlk enjektör-koşusunda `d1-r1-izolasyon` yanlışlıkla `decode_via=0` (substrat) etiketledi — silinen-esi ve
onarımı düşükken IMKANSIZ. **Kök-neden:** parity-esi 0-tabanlı (`close_fec_block_manual: for esi in 0..r`)
→ source-esi-uzayı (0..K−1) ile ÇAKIŞIR. rx `e < K` ile sınıflandırdığından parity'yi source-sandı →
sahte-`src_full`. **Düzeltme:** rx artık WIRE-KIND'a göre sınıflandırır (0x02=parity ayrı-küme); MDS-sayım
`|source| + |parity| ≥ K`. Düzeltme sonrası tüm-pinler tutarlı (§2). Ders: kodlanmış-shard'ları esi-
büyüklüğüyle değil kind'la ayır. Ayrıca §9-ARQ(0x01) ve §11-dup(0x03) ayrı wire-kind'a ayrıldı (substrat-
atıf doğruluğu). [Bkz. [[measurement-tool-first]] — çıktı-güvenilmeden-önce-araç-doğrula.]

---

## 6. KAPSAM / DEĞİŞMEYENLER

- **Üretim fuzzy-config DEĞİŞMEDİ.** Zero-biased controller korunur (koşul-kapsamlı; GLOBAL-DEĞİL).
- **LeftoverOnly** default korunur. Enjektör + izolasyon-modu yalnız test/harness (feature-gated).
- **Üretim-yolu byte-layout DEĞİŞMEDİ.** Yalnız harness-runner (udp_runner.rs) + smoke + script + doküman.
- F15/P15 REAÇILMADI; DR-1f/1d/Kesit-2-full/Sprint-6 AÇILMADI.

---

## 7. SON-SUNUM (30 madde)

1. **Çerçeve:** Bu deney ARQ-vs-FEC değil; ARQ bütün-kollarda-aktif; ölçülen marjinal-proaktif-FEC-faydası.
2. İki-eksen: Utility (marjinal-decode-fayda var mı) × Observability (karar-anı görülebilir mi).
3. **FJ:** wall-clock `RTT>deadline` rejimi rootless-harness'ta KURULAMAZ (netem-gecikme-uygulanmaz;
   ping 0.02ms; cross-ns root-gerektirir) → **TimingNonSeparating**.
4. Zamanlama-ayrışması yerine DELAY-BAĞIMSIZ KOMBİNATORYAL-ayrışma kullanıldı (MDS-decodability).
5. **FK-enjektör:** rx-ingress exact-source-erasure (ağdan-sonra, FEC/ARQ-öncesi); test-only; üretim-etkisiz.
6. İki-mod: FEC-IZOLASYON (substrat-onarım-düşük → saf-FEC) + HİBRİT (substrat-residual'ı-tamamlar).
7. Utility sınırı **KESİN:** `r ≥ deficit` (MDS `(K−d)+r ≥ K`).
8. **Pin-1 (d=1):** r0→60 PENDING, r1→60 FEC-COMPLETE. **SEPARATES ✓**.
9. **Pin-2 (d=2):** r1→60 PENDING, r2→60 FEC-COMPLETE. **SEPARATES ✓**.
10. **Pin-3 (d=4):** r2→60 PENDING, r4→60 FEC-COMPLETE. **SEPARATES ✓**.
11. **Pin-4 (d=5):** r4→izolasyonda 60 PENDING (residual-1); hibritte substrat tamamlar (31/29 FEC/ARQ).
12. **Kapı:** Pin-1/2/3 ayrıştı → **HARNESS GEÇERLİ** (INVALID-tetiklenmedi).
13. FEC-COMPLETE'lerin tümü `decode_via=1` + `first_repair_ms≥0` (60/60) — FJ-timing-zinciri çalışıyor.
14. **Hibrit substrat:** tüm-onarım §9-ARQ (repair_kind=1, 420); §11-dup bu-rejimde gözlenmedi.
15. **Kritik-korreksiyon:** parity-esi 0-tabanlı → source-esi-çakışması → rx yanlış-sınıflandırma.
16. Düzeltme: rx WIRE-KIND'la sınıflandırır (0x02=parity); MDS `|source|+|parity|≥K`. Tüm-pinler-tutarlı.
17. §9-ARQ(0x01) ve §11-dup(0x03) ayrı wire-kind'a ayrıldı (substrat-atıf-doğruluğu).
18. **FL utility-sınıfları:** MarginalFecDecodable_isolation / FecInsufficient / HybridResidualArqRequired
    / NoMarginalFecUtility_thisCondition.
19. **Dürüstlük-nüansı:** utility'nin VARLIĞI kombinatoryal-kanıtlı; **wall-clock-payoff (latency) ölçülemedi**.
20. Latency-payoff ancak `substrat-onarım > deadline` rejiminde; düşük-RTT'de substrat aynı-hızlı → payoff=0.
21. `MarginalFecEssential` (latency) sınıfı bu-harness'ta POZİTİF-ölçülemedi (yalnız kombinatoryal-sınır kesin).
22. **§11-örtüşme:** substrat proaktif-duplication da içerir → düşük-deficit'te FEC-marjini §11 ile örtüşür.
23. **Cold-start-bulgusu:** ilk-paket-kaybı FEC'ten-bağımsız deadlock (PreConverge/PTO-donuk/1-birim-floor).
24. → utility-rejimi ancak steady-state'te ölçülebilir; deficit warm-bloklara uygulanır (warmup-knob).
25. **FM loss-fidelity:** `loss_windowed_ppm=0`, 1497/1497 (blackout30-dahil) → controller kaybı-görmez.
26. RTT/deadline-oranı ve per-blok-deficit karar-öncesi FusionInputs'ta YOK.
27. **Observability-sınıfı:** `UnobservableWithCurrentFusionInputs` + `Confounded`.
28. **Loss-fidelity-kararı:** utility-var ama girdiler-ayrıştırmıyor → **OBSERVABILITY BLOCKED** (fuzzy-tuning
    değil; SENSÖR/ÖLÇÜM-DR).
29. **Kapsam:** üretim fuzzy-config + LeftoverOnly + byte-layout DEĞİŞMEDİ; enjektör test-only; F15/P15-reaçılmadı.
30. **HÜKÜM:** **FEC UTILITY BOUNDARY IDENTIFIED (kombinatoryal) — CONTROLLER OBSERVABILITY BLOCKED;
    SENSOR/MEASUREMENT DR REQUIRED.** (+ ikinci DR: wall-clock-latency-utility rootless-harness'ta ölçülemez.)

---

## 8. HÜKÜM

> **FEC UTILITY BOUNDARY IDENTIFIED (kombinatoryal: `r ≥ deficit`, mekanik-pinli, delay-bağımsız,
> CI-kilitli) — CONTROLLER OBSERVABILITY BLOCKED; SENSOR/MEASUREMENT DR REQUIRED.**
>
> İki ayrı DR açılır (kalibrasyon İLERLEMEZ):
> 1. **Loss-signal-fidelity + regime-observability DR:** `loss_windowed_ppm=0` sensör-hatası + FusionInputs'a
>    deficit/RTT-vs-deadline rejim-girdisi. Fuzzy-tuning DEĞİL; sensör/ölçüm-katmanı.
> 2. **Harness wall-clock-timing DR:** sadık-gecikme (gerçek `RTT>deadline`) için root'lu-netem veya
>    cross-namespace-harness — proaktif-FEC'in latency-payoff'unu POZİTİF-ölçmek için.
>
> Üretim fuzzy-config zero-biased-controller korunur (koşul-kapsamlı, GLOBAL-DEĞİL). DUR.

**Veri:** `reports/d1-exit/data/fec-pins/` (28 koşu; `ANALYSIS.txt`; `ENV.txt` RTT-kanıtı).
**Kod:** `crates/adaptive-transport/src/udp_runner.rs` (enjektör + MDS-decode-fix + wire-kind);
`examples/udp_budget_runner.rs` (`--deficit`/`--deficit-from-block`/`--drop-repair`);
`tests/udp_runner_smoke.rs` (5 test, FK-mekanik-pin CI-kilitli); `scripts/netem/run_fec_pins.sh` +
`fec_pins_analyze.py`.
