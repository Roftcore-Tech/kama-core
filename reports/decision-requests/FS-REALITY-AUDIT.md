# FS-AUDIT — REPO REALITY / PROVENANCE (DR-FEC-FS-1 v2 §3)

**Statü:** Sonuç-taşıyan-koddan ÖNCE zorunlu ön-denetim. Kod doğrudan-doğrulandı (2026-07-31; branch
d1-sprint5c @ 3a901fe sonrası). Eski envanter (`fuzzy-controller-envanter.md`, 2026-07-22) bazı
noktalarda **STALE** — DR-1a/1e/1f turları aktüasyon-zincirini uyguladı; aşağıda güncel-gerçek.

## 3.1 COLD-START B-FREEZE DURUMU → **NOT PRESENT / NOT VERIFIED**

Repoda cold-start **B-freeze belgesi YOK**. Grep bulguları:
```text
docs/roadmap/...status-board.md:91  "### B — COLD-START FREEZE (yalnız A1+A2 tamamlandıktan sonra)"  ← PLAN, koşulmadı
reports/.../DR-CS-1b-A2-CAL-freeze-clientreceiver.md   ← A2 CAL-freeze (client/receiver sabitleri; B DEĞİL)
reports/d1-exit/1b-dr1b-final-freeze.md                ← DR-1b policy-freeze (Leftover; B DEĞİL)
reports/d1-exit/T_W1_BOUND-calibration-freeze.md       ← tek-sabit kalibrasyon-freeze (B DEĞİL)
```
**Hüküm:** B (probe-codepoint/ladder/PTO/patience/resource-pin mühürlemesi) **başlamadı** — çünkü ön-koşulu
A1+A2'nin tamamlanması ve A2 hâlâ **PARTIAL** (hücre-iii Sprint-6). A1/A2 regresyon-pinleri test-suite'te
CANLI (cold_start_liveness 4 · dr_cs1_acceptance 22 · dr_cs1_wire 12 · A2 CLOSURE-1B/2) ama **freeze-belgesiyle
MÜHÜRLENMİŞ DEĞİL**. FS programı **B'yi açmaz, cold-start-tuning yapmaz, cold-start'a FEC sokmaz** (§0 sınırı).

## 3.2 GÜNCEL FEC-AKTÜASYON GERÇEĞİ (kenar-kenar; kod-doğrulandı)

Zincir: `FusionController → requested ratio → R → block-authority → eligibility → enqueue → budget/token → scheduler → seal/wire`

| # | Kenar | Durum | Kanıt (dosya:satır) |
|---|---|---|---|
| 1 | Fusion → `fec_parity_ratio_permille` + `arq_aggressiveness_permille` | **LIVE** | fusion.rs:438/440/547-548 (Mamdani 4-kural; R2→fec_high) |
| 1c | ratio → controller_decision_snapshot | **LIVE** | sender.rs:2406-2413 (`last_decision.fec_parity_ratio_permille` okunur) |
| 2 | ratio → **R** (`compute_requested_r`) | **LIVE (mekanizma)** | sender.rs:509 def; **çağrı sender.rs:2475** (latch-içi) |
| 3 | block-ownership (`FecBlockAuthority`, non-Clone tek-tüketim) | **LIVE (mekanizma)** | sender.rs:259/2299/2370; same-event-guard latch |
| 4 | eligibility+enqueue (`plan_fec_parity`→`pending_fec`) | **LIVE (mekanizma)** | sender.rs:2352 push_back; drain pace |
| 5 | budget/token admission (§8.5 `parity_final=min(fuzzy,budget_cap,profile_cap)`) | **EXPERIMENTAL-ONLY** | budget.rs:38/45; üretim-çağıranı YOK (yalnız `experimental-fec-budget`) |
| 6 | scheduler → seal/wire | **LIVE** (DATA-önce sıra korunur) | mevcut pacer; INV-FEC-1 |
| — | **Controller-mode'u ÜRETİMDE açan çağrı** | **MISSING** | `open_fec_block_authority` tek-ÜRETİM-DIŞI-çağranı `udp_runner.rs:759` (experimental-fec-budget); gerisi TEST |
| — | default aktüasyon-modu | `Manual { enabled:false }` (KAPALI) | sender.rs:989 (flag-OFF byte-özdeş) |
| — | dup↔parity çapraz-koordinasyon | **MISSING** | `plan_forward_duplication`↔`plan_fec_parity` çapraz-okuma SIFIR (deferred-item#6) |
| — | ARQ-aggressiveness aktüatörü | **DEAD/DEGENERATE** | plan() yalnız `==0`-gate; seviye-büyüklüğü kullanılmıyor |
| — | `MinimalParity` (§11) | **DEAD** | emergency.rs:194-197 yalnız `Framing::SegmentBlock`; üretim hep `Framing::Tiny` (sender.rs:1947) |
| — | FuzzyConfig injection | **LIVE** | `try_with_fuzzy_config` sender.rs:958 (+ValidatedFuzzyConfig; ctor'a-gömülü-kalibrasyon-yok) |

**ÖZET:** DR-1a/1e ile aktüasyon-zinciri (fusion→snapshot→latch→R→authority→enqueue→seal) **UYGULANDI** ama
**EXPERIMENTAL-ONLY**: default `Manual{false}`, Controller-mode'u yalnız `open_fec_block_authority` açar ve
onun tek üretim-dışı çağıranı `experimental-fec-budget` udp_runner'ıdır. Bu, DR-1a'nın "MECHANISM CLOSED —
PRODUCT INTEGRATION DEFERRED" hükmüyle tutarlı. **Eski "fec_parity_ratio ÖLÜ-ÇIKTI" ifadesi bugün STALE** —
ratio artık latch→R→enqueue yolunda tüketiliyor (ama üretim-default'unda değil). FS runner bu experimental
yolu (Manual{enabled,ratio} / Controller + open_fec_block_authority) kullanarak FEC-parity emisyonunu sürebilir.

## 3.3 OBSERVABILITY GİRDİLERİ (FS-5 ön-tespiti)

`FusionInputs` (fusion.rs) YALNIZ **5 alan**: `loss_ppm · bandwidth_bps · jitter_ppm · qd_ratio_permille ·
ack_confidence_permille`. **YOK:** RTT/SRTT · time-to-deadline · deadline-slack · tahmini-ARQ-repair-gecikmesi
· burst-persistence · reordering · reverse-path-confidence · reverse-bandwidth. **Kritik:** tarihsel tek-fayda-
bölgesinin ayırıcısı **RTT'dir ve fusion-girdisinde YOK** → canlı-controller o bölgeyi ayırt-EDEMEZ
(`FU-regime-observability.md` "CONTROLLER OBSERVABILITY BLOCKED" bulgusu GÜNCEL-DOĞRULANDI). FS-5 bu eksikliği
aday-ölçümlerle (SRTT/deadline-slack/ARQ-repair-tahmini) test edecek.

## 3.4 STALE / SUPERSEDED İŞARETLERİ
```text
STALE      fuzzy-controller-envanter.md §"fec_parity_ratio ÖLÜ-ÇIKTI"  → SUPERSEDED-by: DR-1a latch/R-wiring (bu-audit §3.2)
GEÇERLİ    envanter §senkron-mod=SPEC-NİYETİ, dup↔parity=SIFIR, MinimalParity=ÖLÜ, budget_cap=kablosuz  → HÂLÂ-DOĞRU
GEÇERLİ    FU-regime-observability CONTROLLER-OBSERVABILITY-BLOCKED  → HÂLÂ-DOĞRU (FusionInputs RTT-içermiyor)
```
Eski belgeler SESSİZCE düzeltilmedi; yukarıdaki işaretlerle referanslandı.
