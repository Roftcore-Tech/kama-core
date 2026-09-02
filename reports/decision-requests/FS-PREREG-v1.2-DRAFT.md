# FS-PREREG-v1.2 — HARNESS-A AMENDMENT **DRAFT** (DONMAMIŞ)

> **⛔ SUPERSEDED-BY FS-PREREG-v1.2.md (FINAL, FROZEN sha256 `da4af3c4…`).** Bu DRAFT, FINAL'e
> INCORPORATED (A1.0–A1.11 detaylı-türetme kaynağı). Bağlayıcı-otorite FS-PREREG-v1.2.md'dir.

> **STATÜ: DRAFT — FREEZE-EDİLMEDİ · HASH-YOK.** Bu belge FS-HARNESS-A inşası sırasında ortaya çıkan
> ve Abkarada tarafından bağlayıcı-olarak-verilen netleştirmeleri BİRİKTİRİR. **Tek bir konsolide
> re-hash FS-HARNESS-B tamamlandıktan SONRA, FS-1 result-bearing-run'lardan ÖNCE yapılır** (v1.2-FINAL).
> O ana kadar bu belge OKUNUR-referans; hiçbir sayı buradan freeze-değildir.
>
> **Base:** FS-PREREG-v1.1 (sha `60353e99…`). v1.1'in donmuş nicelikleri AYNEN geçerli; aşağıda YALNIZ
> v1.1-SONRASI delta. Çelişkide sıra (v1.2-FINAL freeze olana dek): v2-prompt > v1.1 > **bu v1.2-DRAFT** >
> coverage-v1.1 > v5-icra. (v1.2-FINAL freeze olunca: v2-prompt > v1.2-FINAL > v1.1 > …)

---

## A1.0 — TIMING: `t_repair_usable` (v1.1 §timing-chain AMENDMAN)
```text
YENİ ledger-alanı BlockTiming.t_repair_usable — redundancy'nin GERÇEKTEN-kullanılabilir olduğu an:
  F* (FEC): = reconstruct-TAMAMLANDI + recovered-byte-hazır  (codec-decode-latency DAHİL).
            t_parity_arrival'DAN FARKLI (parity tel'e-vardı ≠ decode-bitti-byte-hazır).
  A1 (dup): = duplicate-arrival (decode-latency yok → t_redundancy_arrival ile çakışır).

BAĞLAYICI: ParityDelayClass::UsableBeforeArq artık **t_repair_usable < t_arq_arrival**'dan türetilir
(ESKİ: t_parity_arrival). Gerekçe: "coding mı timing mi" sorusu ancak decode-latency'yi İÇEREN
kullanılabilirlik-anıyla dürüst yanıtlanır; parity'nin tel'e-varması FEC-faydasını OLDURMAZ.
Etki: C-P3 (reverse-blackout) redundancy-lead-time yorumları t_repair_usable üstünden okunur.
Smoke-kanıt: F0 D2/R2 → t_parity_arrival=21.4ms iken t_repair_usable=94-115ms (decode+block-complete).
```

## A1.1 — A1-EARLY CARDINALITY: BLOCK-TOTAL bütçe (v1.1 C-P1/C-P4 AMENDMAN)
```text
A1-EARLY "configured R" = **BLOK-TOPLAM redundancy bütçesi**, FEC R-sınıfına eşdeğer:
  configured_r ADET seçili-source, HER-BİRİ BİR-KEZ duplicate edilir (blok-total = r ekstra-paket).
  KESİNLİKLE "her-source başına R-duplicate" (K×R) DEĞİL — bu ~K× FEC-bütçesi olurdu (kardinalite-hatası).
Target-set: a1_duplicate_target_set(block_id, dup_seed, r, k) — lowest-hash-rank, **deficit-BAĞIMSIZ**
  (blind; loss-trace'i GÖRMEZ). dup_seed = H(master,"A1_DUP_TARGET") ⟂ loss_seed = H(master,"LOSS").
Etki: C-P4 budget-parity artık gerçek-eşdeğer (A1 blok-total-r ↔ F0 r-parity), K×R-şişme YOK.
Smoke-kanıt: A1 r=2,k=8,seed=2026 → target={2,3}, duplicates_emitted=2 (blok-total), payload=128B.
```

## A1.2 — C-P4 FAIRNESS: realized-sealed-wire (v1.1 C-P4 SIKILAŞTIRMA + A1.11 DÜZELTMESİ)
```text
C-P4 coding-vs-dup fairness'ı configured_r-eşitliği TEK-BAŞINA-YETMEZ. Nihai metrik:
  **realized_sealed_wire_bytes** (payload + framing), A1/F0 AYNI physical-accounting-domain'inde
  (ortak BottleneckChannel egress + ortak RedundancyCost). Otomatik BUDGET_MISMATCH-guard:
  |wire_A1 − wire_F*| toleransı-aşarsa → BUDGET_MISMATCH-NO-CAUSAL-EVIDENCE (coding-claim GEÇERSİZ).
DÜZELTİLDİ (A1.11): source-shard ve repair-shard sealed-wire boyu AYNI (INV-ENGINE-SOC; transport
  ikisini boyutla ayırt-edemez). Bu yüzden A1-dup framing = F0-parity framing = FS_SHARD_FRAMING_BYTES(37).
  A1 r=2 realized-wire == F0 r=2 realized-wire (matched-r → matched-wire) → C-P4 GERÇEK budget-eşitliği.
Smoke-kanıt (DÜZELTİLMİŞ, grounded fs_sealed_len): A1 dup r=2 = 202B (128 payload + 74 framing) ==
  F0 parity r=2 = 202B (128 payload + 74 framing). Eski uydurma "128 vs 216" domain-hatasıydı (A1.11).
```

## A1.3 — BLIND-DUP HONESTY + causal ledger (v1.1 C-P1/C-P3 NETLEŞTİRME)
```text
A1-EARLY yalnız (target ∩ D) kesişiminde recover eder; aksi halde ARQ-fallback (dup boşa-gitti).
Causal ledger: a1_dup_hits (dropped-initial ∈ target) · a1_dup_misses (dropped-initial ∉ target).
BAĞLAYICI dürüstlük-pini: A1-MISS bir deficit'te A0 ile **byte-özdeş sonuç** vermeli (dup FAYDA-SAĞLAMAZ,
  yalnız wire-maliyeti öder). Bu blind-emisyonun gerçekçiliğini kanıtlar (receiver-feedback'siz proaktif-dup).
Smoke-kanıt: A1-MISS(drop=4) ≡ A0(drop=4): retransmits=1, t_arq_arrival=151.5ms, completion=152ms (İKİSİ).
  A1-HIT(drop=2∈target): retransmits=0, cause=duplicate, UsableBeforeArq.
```

## A1.4 — PIN-A / PIN-E: emission-neutrality & egress-fairness (v1.1 instrument-chain EK)
```text
PIN-A (DIRECT emission-side neutrality): A1 duplicate-emisyonu Sender-recovery-STATE'ini BYPASS-edebilir
  (runner-level dup; Sender-ref hiç-tutulmaz) — AMA emission-anında A0 ile aynı source-emission-trajesi.
  DÜZELTME (Abkarada): post-feedback A0/A1 Sender-state EŞİTLİĞİ ŞART-DEĞİL (gerçek-ACK sonrası doğal-ayrışma
  meşru-faydayı yasaklamamalı). Nötrlük YALNIZ direkt-emisyon-tarafında.
PIN-E (egress fairness): runner-level dup Sender-STATE'i bypass-etse de deneyin FİZİKSEL-egress-maliyetini
  BYPASS-EDEMEZ. Her paket (düşürülen dahil) BottleneckChannel'da serialize → dup BEDAVA-LANE-ALMAZ.
```

## A1.5 — F0: RECOVERED-scope + ESTIMATOR-PURITY (6. sanity-pin; v1.1 §snapshot-contamination GÜÇLENDİRME)
```text
F0 kapanışı GERÇEK RsSimdCodec-reconstruct + Event::RecoveredControl (fake-ACK DEĞİL). 6. pin:
  decode→RecoveredControl bir RTT-gözlemi SENTEZLEMEZ. estimator_pure = (path_timing_snapshot()+diag_wbtlbw())
  RECOVERED-öncesi/sonrası ÖZDEŞ (srtt/rtprop sample-count DEĞİŞMEZ). İhlal = fake-ACK-kontaminasyonu = INVALID.
HYBRID: FEC-yetersizse (D>R) residual-ARQ tamamlar — F0 ARQ-KAPALI-DEĞİL (F0_EXACT_D2_R1: D2/R1 → ARQ-completes).
Smoke-kanıt: F0 tüm-hücrelerde estimator_pure=true; D2/R1 → retransmits>0 + delivered=8/8.
```

## A1.6 — CRATE PLACEMENT / PRODUCTION-GRAPH (v1.1 §B0.1 provenance EK)
```text
INV-ENGINE-SOC: adaptive-transport FEC-AGNOSTIK (kama-fec-codec dep-i YOK). Bu yüzden:
  A0/A1 closed-loop → adaptive-transport::fs_harness (kod-tabanı FEC-bilmez; A1-dup source-byte-kopyası).
  F0 closed-loop   → kama-integration (kama-fec-codec zaten-prod-dep). Ortak modül tests/common/fs_f0.rs;
                     hem test hem examples/fs_runner (#[path]-include) yeniden-kullanır.
feature `experimental-fs-harness` = YALNIZ test-derlemesi (dev-dep adaptive-transport + prod-dep kama-fec-codec).
  Production lib-grafiği GENİŞLEMEZ (INV-CS-5 flag-OFF byte-özdeş korunur).
```

## A1.7 — F2 (interleaved parity) DROPPED (v1.1 arm-set AMENDMAN)
```text
F2 (interleaved/pre-DATA parity) SEALED "DATA-before-parity ordering"i (sender.rs:2287) İHLAL eder →
  bu kampanyadan DÜŞÜRÜLDÜ, PENDING (gelecek DR-FEC-INT-1). FS-1 hiçbir hücrede F2 taşımaz.
  C-P2 arm-seti 5 KALIR (A0,A1,F0,F1,F3) — F2 bunlarda YOKTU. Toplam FS-1 = 540 (6-arm-grid; F2-hariç).
Ref: FS-SEAL-COLLISION-MAPS.md (F2 DUR-B).
```

## A1.8 — TAIL-LOSS = B_REAL_TAIL_ERASURE (SINIFLANDIRILDI; horizon-uzatmayla ÇÖZÜLMEZ) → FS-HARNESS-B
```text
SINIFLANDIRMA (reviewer talebi "önce sınıflandır A vs B"): **B_REAL_TAIL_ERASURE** — harness-defekti DEĞİL.
KANIT (kod-yolu, workflow + doğrudan-okuma teyit): run_arm PTO'yu SADIK servis-eder (Timer::Pto her 1ms-tick,
  koşulsuz, 20s-horizon boyunca; fs_harness.rs:612). Sender YAPISAL-olarak tail-loss'u kurtaramaz:
  detect_time_losses "seq < largest_acked" ÖN-KOŞULUNA bağlı (loss.rs:383; RFC9002 §6.1 later-ACK; temiz-
  kanal false-loss'unu önlemek için EKLENDİ). SON-unit düşünce largest_acked=k−2'de takılır → seq(son) HİÇBİR
  ZAMAN < largest_acked olmaz → sonsuza-dek loss-ilan-edilemez. Timer-kümesi {Pto, RtpropProbe} — GERÇEK
  RTO/TLP YOK. Belgelenmiş-caveat: loss.rs:277 (tail>largest_acked "F0-tespit-(iii)"), loss.rs:364 (tail-
  recovery §5.1 ack_confidence→RECOVER_FWD liveness'e ERTELENDİ, A0'da aktif-DEĞİL). affects_slice_a=false.
SONUÇ: **horizon-uzatma ÇÖZMEZ** (timeout-sorunu değil, seq<largest_acked yapısal-önkoşulu). Bu GERÇEK bir
  tail-erasure rejimidir ve doğrudan FEC-lehine kanıttır (FEC tail-kaybını deterministik-kurtarır, ARQ
  yapısal-olarak KURTARAMAZ). C-P3'te KORUNUR (maskeleme-YOK).
FS-HARNESS-B [B2] YAPILACAKLAR (reviewer B-dalı talimatı): (1) tail-loss'u AÇIK ölçülen-boyut-olarak işaretle
  (source-position/tail-erasure metadata); tail-pozisyonları DIŞLAMA; frozen-horizon'a göre censor/raporla.
  (2) GERÇEK-fix Sender'da (harness-DEĞİL): RTO/TLP VEYA §5.1 ack_confidence→RECOVER_FWD tail-liveness —
  bu transport-crate işi, FS-HARNESS-A kapsamı-DIŞI. KARAR Abkarada (C-P3 bağımlılığı).
```

## A1.9 — INVALID ⟂ INCOMPLETE ortogonalliği (v1.1 §censoring NETLEŞTİRME)
```text
İki-eksen KARIŞMAZ:
  admission-gate (faithful ∧ overlap) → run VALID mı? (harness-bütünlüğü). İhlal = INVALID — NO EVIDENCE.
  completion_ns=None (tail-loss/horizon-aşımı) → LEGITIMATE-INCOMPLETE veri-noktası (P(complete≤horizon)
    endpoint'ine girer; §B0.3 censoring). INVALID-DEĞİL.
Yani "faithful+overlap ama delivered<k" bir GEÇERLİ-ama-tamamlanmamış gözlemdir (extreme-cell primary-endpoint
  bunu bekler), harness-hatası değil.
```

## A1.10 — FS-5 OBSERVABILITY: PLUMBING_GAP çözüldü (v5 §FS-5 reality-audit sonucu)
```text
Reality-audit: RTT/BtlBw bilgisi transport-engine'de ZATEN-MEVCUT (RttEstimator srtt/rtprop/rttvar;
  delivered-rate BtlBw). path_timing_snapshot() bunu READ-ONLY sızdırır (PathTimingLedger). FS-5 riski
  PLUMBING_GAP→çözüldü: ledger-only, karar-tüketmez (§B0.6 snapshot-contamination), production-değişiklik-YOK.
```

## A1.11 — WIRE-ACCOUNTING GROUNDING (review-blocking-fix; A1.2 domain-düzeltmesi) ✅
```text
REVIEW-BULGUSU: eski run_arm A1-dup raw-payload (framing=0, wire=d.size=E) sayıyordu; F0-parity uydurma
  parity_frame=108 (framing=44). İki-sorun: (i) A1/F0 FARKLI-domain (payload-only vs framed) → C-P4 sahte;
  (ii) source DATA da payload-only serialize → completion-timing domain-tutarsız.
GROUNDED-DÜZELTME (uydurma-YOK; §8): TEK sealed-datagram modeli — fs_sealed_len(E) = E + FS_SHARD_FRAMING_BYTES.
  FS_SHARD_FRAMING_BYTES = 37 = EXTENDED_OUTER_OVERHEAD(26: h_ext4+wfmt1+wflags1+pn4+tag16) +
  FEC_INNER_HDR_LEN(11: iv1+kind1+flags1+pid1+block4+esi2+k1). Otorite: kama-noise-wire protector.rs
  sealed_len(FecShard) = 26+11+E ("N7: 101B @ E=64"). SOURCE ve REPAIR shard BOYUTU AYNI (tek FecShard;
  fark yalnız sealed-payload İÇİNDEKİ 1-byte inner_kind 0x01/0x02 — wire-boyu-değil). Eski "108/44" UYDURMA
  (gerçek 101/37; +7B over-count; "44" cold-start CTRL_CS_PROBE frame'iyle rastlantısal-çakışma).
UYGULAMA: source|retransmit|A1-dup|F0-parity HEPSİ fs_sealed_len'den serialize + sayılır (OpaqueUnit.size =
  sealed wire-boyu; Sender wire-reality'de pace-eder). Pin: A1_WIRE_ACCOUNTING_REAL_DATAGRAM — bir dup için
  payload + framing == realized == channel-serialization, HEPSİ AGREE. Sonuç: A1 r=2 = F0 r=2 = 202B (matched).
```

---

## DEĞİŞMEYENLER (v1.1'den AYNEN)
```text
arm-set-çekirdek · statistics (N=10 paired · exact hits/N · ≈%-YASAK) · deadline-formülleri (§B0.2) ·
horizon/censoring (§B0.3) · faithfulness-tolerances (§B0.4) · oracle-ordering (§B0.5) ·
snapshot-contamination (§B0.6; A1.5/A1.10 GÜÇLENDİRİR) · verdict-path (FULL-GO|NICHE-GO|RESEARCH-ONLY|
K4-RETREAT; agent-önerir/Abkarada-onaylar) · cost/pareto (§B0.8).
```

## PROVENANCE (DRAFT)
```text
base       : FS-PREREG-v1.1 sha256=60353e99715cd4e51a42c53a94b6c1ad6963d8c3b3ef017ebc1bf2b9c3aaadf7
statü      : DRAFT — HASH-YOK, FREEZE-YOK. v1.2-FINAL re-hash = FS-HARNESS-B-sonrası, FS-1-öncesi (TEK konsolide).
kaynak     : FS-HARNESS-A inşa-turu netleştirmeleri (A1.0–A1.11). Smoke-kanıtları scripts/netem/fs_runner_smoke.sh.
açık-item  : A1.8 TAIL-LOSS = B_REAL_TAIL_ERASURE → FS-HARNESS-B [B2] (Sender RTO/TLP/§5.1-liveness + tail-metadata;
             harness-DEĞİL Sender-fix; horizon-uzatma ÇÖZMEZ). C-P3 bağımlılığı → Abkarada.
```
