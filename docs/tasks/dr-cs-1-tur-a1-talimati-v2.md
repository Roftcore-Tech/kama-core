# CLAUDE CODE TALİMATI v2 — TUR A1: DR-CS-1 KALAN FAITHFUL HÜCRELER

**Tarih:** 2026-07-26 · **Branch:** `d1-sprint5c` · **PR:** #44 (DRAFT — merge YASAK)
**v1'i HÜKÜMSÜZ kılar** (arşiv deseni). **Dayanak:** F4-CLOSURE + ROADMAP GO + Abkarada
A1-inceleme hükmü: *CONDITIONAL GO — F-A1-0 başlayabilir; F-A1-1 öncesi beş yürütme boşluğu
normatif kapatılmalı.* Bu v2 o beş kapanışı + H1-kapsam düzeltmesini + nedensellik-zinciri
assert'lerini içerir. Hedef: *TUR A1 IS DETERMINISTIC, CAUSALLY ATTRIBUTABLE AND CAPABLE OF
CLOSING THE TWO REMAINING SENDER-SIDE CELLS WITHOUT PRODUCTION CHANGES.*

---

## 0. BAĞLAYICI ÇERÇEVE

### 0.1 MUTLAK SINIR — PRODUCTION CODE DEĞİŞİKLİĞİ SIFIR
Değişebilecek yüzey YALNIZ: test-harness script'leri, harness binary'lerinin test-only
bayrak/hook'ları, test dosyaları, ledger/rapor araçları, doc-commit'ler. Ürün mekanizma kodunda
(`crates/*/src` mekanizma yolları) herhangi bir diff = **DUR-B**. Flag-OFF pinleri yeşil kalır.
[CAL] üretim sabitleri değişmez. Tuning YOK · yeni mekanizma YOK · A2/FEC işi YOK ·
beklenti-gevşetme YOK.

### 0.2 ENSTRÜMAN ZİNCİRİ — İKİ KATMANLI RTT KABULÜ (Gap-4)
Her faithful koşu için ZORUNLU; biri kırmızı → **INVALID RUN — NO EVIDENCE** (FAIL değil;
yeniden koşulur):
1. **Preflight:** faithful acceptance 6/6 @ canonical RTT100 (oturum başına).
2. **Per-cell RTT realization (her koşu satırında):** `configured_rtt ∈ {100,250,1000}` için
   `measured_rtt_p50 / measured_rtt_p95 / rtt_tolerance_result` kaydedilir; forward/reverse
   gecikme AYRI doğrulanır. **[CAL — F-A1-0'da donar]:** `|p50 − configured| ≤ max(5ms, %5)`;
   `p95 ≤ configured + max(20ms, %10)`. Geçmeyen satır INVALID.
3. **Overlap-gate:** `receiver_events > 0`.
4. **Binary-provenance:** koşu öncesi byte-boyut + feature-kimlik (bin_cs ≠ bin_off; raporda).
5. **Argüman-paritesi:** kollar birebir-arg; arg-diff çıktısı rapora.
6. **Pre-registration:** §1-§2 beklenen-zincirler, tüm [CAL]'ler ve eşikler KOŞULARDAN ÖNCE
   hash+timestamp ile doc-commit (F-A1-0); sonuca bakıp değer/beklenti seçmek YASAK.

### 0.3 Clock-domain
Tüm ledger zaman damgaları aynı host `CLOCK_MONOTONIC` domain'indedir (tx/rx süreçleri aynı
kernel → çapraz-süreç sıralama assert'leri bu sayede meşrudur). Karışım YASAK; ledger'a
clock-domain satırı yazılır.

### 0.4 Yürürlük
Spec-otoritesi > kod · sessiz silme YOK · arşiv deseni · DUR-A=rapor-et-devam ·
DUR-B=bekle-onay · DUR-SON=tam rapor+dur.

---

## 1. HÜCRE A1-1 — W-3-65 (restore=61) KAPANIŞI

**Kurulum:** faithful two-netns · 65B × 16 · çift-yön blackout t0→restore=61s → temizle ·
**settle-exit KAPALI** · **OBSERVATION_HORIZON = 120 s** [CAL] · **RX_TEST_PATIENCE = 130 s**
[CAL] (Gap-3 — normatif: `RX_TEST_PATIENCE ≥ OBSERVATION_HORIZON + RTT_bound +
processing_margin`; bu YALNIZ test-harness gözlem kontratıdır — product receiver
implementasyonu/hükmü SAYILMAZ; ürün patience A2 işidir) · 3 tekrar.

**Pre-registered beklenen zincir:**
```text
TimedOut@60s (epoch'ta BİR kez) → restore@61s → sonraki mevcut-PTO deadline
(backoff-serisi RAPORLANIR) → retransmit wire-emission (mevcut ARQ yolu) →
DATA-ACK → PostTimeoutRecovery (ikinci terminal YOK; busy-loop YOK)
```
**Ledger:** `timeout_time · restore_time · pto_deadline_series · retransmit_wire_time ·
ack_time · transport_duplicate_rx_count · application_delivery_count ·
application_duplicate_delivery_count · terminal_class · recovery_class`.
**Kabul:** 3/3 → PostTimeoutRecovery horizon içinde; post-condition (`next_wakeup ∈ {None,
gelecek}`) her batch'te. **Sonuç sınıfları:** `PASS 3/3` **veya** `UNOBSERVED-AGAIN — [EXACT]`
(horizon yetmezse: gözlenen PTO-serisi + gereken ufuk hesabı → DUR-B; yeni ufuk ancak yeni
pre-registration ile).

---

## 2. HÜCRE A1-2 — W-2B FAITHFUL ACK-STARVATION

**Amaç:** `probe → echo → bootstrap-RTT → mevcut-PTO retransmit → receiver-dedup` dalının ve
§2.4-a spurious maliyetinin İLK gerçek-tel kanıtı.

**Kurulum:** çift-yön açık (yalnız delay; loss=0) · rx-tarafı deterministik suppression:
t=0'dan itibaren rx'in ürettiği TÜM matching DATA-ACK/SACK emisyonları bastırılır;
**PROBE-ECHO (0x1A) HER ZAMAN geçer**. Test-only bayrak; üretim yoluna sızmaz. Settle-exit
kapalı.

### 2.1 RELEASE MODELİ — ARRIVAL-TRIGGERED (Gap-1; NORMATİF)
```text
release_condition:
  first matching PTO retransmit ARRIVAL at receiver
  (mekanik tetik: aynı unit-id/seq için İLK transport-duplicate DATA arrival →
   rx dedup'ının seq-already-seen isabeti; test-only gözlem)
  → suppression, ACK üretilmeden HEMEN ÖNCE bırakılır

required evidence:
  sender ledger, bu arrival'ın bir EXISTING-PTO wire-emission'ından
  geldiğini AYNI unit-id ile kanıtlar (pto_wire kaydı ↔ retransmit_rx eşleşmesi)

normatif sıra:
  pto_wire ≤ retransmit_rx ≤ suppression_release ≤ released_ack_wire
```
Receiver'a production-benzeri gizli state EKLENMEZ; sender-rx arası out-of-band koordinasyon
YOK (wire-emission-anında release istenirse ayrı test-only IPC tanımı gerekir ve deney açısından
GEÇEMEZ — bu turda kullanılmıyor).

### 2.2 SINIRLAR [CAL — F-A1-0'da gerekçesiyle donar]
`T_MAX = 30 s` (suppression-başlangıcından; zarf-gerekçesi pre-registration'a yazılır:
`T_FIRST_PROBE(3s) + RTT_MAX_CELL(1s) + PTO_AFTER_BOOTSTRAP_BOUND(≤3×srtt≈3s) +
retransmit_dispatch_bound(≤1s) + cömert-pay → ≤13s < 30s`) →
aşımda: release + hücre `FAIL/PARTIAL — [EXACT]`.
`SUPPRESSED_ACK_CAP = 64` → aşımda: release + FAIL (suppressed sayısı sınırsız OLAMAZ).

### 2.3 ZORUNLU LEDGER — AÇIK İSİMLİ ALAN TABLOSU (Gap-2; slash'lı kısaltma YOK)
| # | Alan | Taraf |
|---|---|---|
| 1 | suppression_start | rx |
| 2 | suppressed_ack_count | rx |
| 3 | probe_tx_count / probe_tx_bytes | tx |
| 4 | probe_rx_count | rx |
| 5 | echo_tx_count / echo_tx_bytes | rx |
| 6 | echo_rx_count | tx |
| 7 | probe_wire (ilk probe wire-emission zamanı) | tx |
| 8 | echo_rx (ilk echo kabul zamanı) | tx |
| 9 | bootstrap_srtt_time | tx |
| 10 | pto_eligible | tx |
| 11 | pto_enqueue | tx |
| 12 | pto_wire (+ unit_id) | tx |
| 13 | retransmit_rx (+ unit_id) | rx |
| 14 | retransmit_rx_count | rx |
| 15 | transport_duplicate_rx_count | rx |
| 16 | application_delivery_count | rx |
| 17 | application_duplicate_delivery_count | rx |
| 18 | suppression_release (release_time) | rx |
| 19 | released_ack_wire | rx |
| 20 | bootstrap_induced_retransmit_bytes (unit-id bazlı) | tx |
| 21 | terminal_class | tx |
| 22 | ack_accepted_before_bootstrap_count | tx |
| 23 | srtt_source | tx |
| 24 | retransmit_path | tx |
| 25 | new_retransmit_path_count | tx |

### 2.4 NEDENSELLİK-ZİNCİRİ ASSERT'LERİ (ledger'da; Abkarada sırası aynen)
```text
suppression_start < probe_wire < echo_rx ≤ bootstrap_srtt_time
≤ pto_eligible ≤ pto_enqueue ≤ pto_wire ≤ retransmit_rx
≤ suppression_release ≤ released_ack_wire
```
Ek assert'ler: `ack_accepted_before_bootstrap_count = 0` · `srtt_source = ProbeEcho` ·
`retransmit_path = ExistingPtoArq` · `new_retransmit_path_count = 0` ·
`bootstrap_induced_retransmit_bytes` YALNIZ pto_wire↔retransmit_rx **unit-id eşleşmesiyle**
ilişkilendirilir (zamansal korelasyon tek başına kanıt DEĞİL).

### 2.5 BEKLENEN DESEN (spurious gerçekleşince)
```text
transport_duplicate_rx_count ≥ 1
application_duplicate_delivery_count = 0
application_delivery_count = beklenen-birim-sayısı (her birim BİR kez)
```
("dup=0" tarzı tek sayaç YASAK — dedup-başarısı ile retransmit-hiç-ulaşmadı ayrışır.)

### 2.6 MATRİS
RTT {100, 250, 1000} × size {65, 300, 1200} × 3 tekrar = **27 koşu**; her satır §0.2
per-cell-realization alanlarıyla. Kabul: **27/27** (bkz. §5 eşik kuralları).

---

## 3. HÜCRE A1-3 — H1-DESEN DENETİMİ (Gap-H1 kapsam düzeltmesiyle)

**Yöntem (yalnız grep DEĞİL):** `rg` ile aday toplama → shell/manuel SEMANTİK inceleme →
mümkünse shellcheck/parser desteği. Desen satırlar-arası olabilir; false-positive ayıklanır.
**Sınıflandırma (her bulgu tam bir sınıf):** `SAFE` · `HISTORICAL-SUSPECT` ·
`CURRENT-EVIDENCE-SUSPECT`.
**DUR-B tetiği YALNIZ:** mevcut A1 harness'ı VEYA yaşayan status-board'da hâlen güvenilen bir
kanıt VEYA mevcut closure hükmünü taşıyan bir script etkileniyorsa. Tamamen arşivsel, hüküm
taşımayan script → kanıt-statüsü düşürülerek KAYDA alınır; A1'i tek başına BLOKLAMAZ.
**Çıktı:** dosya:satır + sınıf + etkilenen-kanıt değerlendirmesi; `H1 current-evidence audit:
CLEAN` veya `[EXACT etki listesi] → DUR-B`.

---

## 4. FAZLAR + DUR NOKTALARI (Abkarada faz-kararıyla)

- **F-A1-0 — GO (doc-only):** roadmap belgesi repo'ya (`docs/roadmap/…`, verildiği gibi;
  değişiklik YOK) + bu talimatın pre-registration bloğu: beklenen-zincirler, [CAL] tablosu
  (HORIZON=120s · RX_TEST_PATIENCE=130s · T_MAX=30s+zarf-gerekçesi · SUPPRESSED_ACK_CAP=64 ·
  RTT-toleransları) + §5 eşikleri → hash+timestamp → **DUR-A**.
- **F-A1-1 — beş düzeltme işlenmiş olarak GO:** harness değişiklikleri (settle-exit anahtarı ·
  horizon · RX_TEST_PATIENCE · arrival-triggered suppression-release + unit-id eşleşme kaydı ·
  sayaç ayrımı · per-cell RTT-realization ölçümü). Her hook İZOLE doğrulanır + flag-OFF pin
  seti yeşil.
- **F-A1-2 — GO (zincir + per-cell pinlerle):** A1-1 (3 rep) + A1-2 (27 koşu).
- **F-A1-3 — GO (sınıflandırmalı):** H1 denetimi.
- **F-A1-4 — Adversarial-verify (≥4 skeptik; şüphede refuted=true):** (i) suppression YALNIZ
  DATA-ACK/SACK'leri mi bastırdı (echo_rx>0 + bootstrap_srtt_time kanıtı) · (ii) release doğru
  olayda mı (retransmit_rx ≤ suppression_release sırası + unit-id eşleşmesi; pto_wire
  sender-kanıtı) · (iii) W-3-65 zinciri gerçek mi (PTO-serisi → retransmit_wire_time;
  ikinci-terminal-yokluğu; RX_TEST_PATIENCE'ın sonucu maskelemediği) · (iv) arg-paritesi +
  binary-provenance + per-cell-RTT ihlali var mı.
- **F-A1-5 — closure:** yalnız §5 eşikleri sağlanınca FULL.

---

## 5. KABUL EŞİĞİ VE HÜKÜM (Gap-5 — kesin)

**FULL için ZORUNLU:**
```text
W-3-65  = 3/3 PASS
W-2B    = 27/27 PASS
H1 current-evidence audit = CLEAN
```
`26/27` OTOMATİK FULL YAPMAZ → `PARTIAL` + exact-failing-cell + **deterministik rerun** +
kök-neden. Enstrüman-zinciri geçersiz koşu FAIL SAYILMAZ → `INVALID — NO EVIDENCE` + yeniden
koşum. Sonuca bakıp eşik/beklenti/[CAL] değiştirmek YASAK.

**Hüküm satırları:**
`DR-CS-1 SENDER-SIDE ACCEPTANCE: FULL — [W-3-65: 3/3 · W-2B: 27/27 · H1: CLEAN]`
**veya** `DR-CS-1 SENDER-SIDE ACCEPTANCE: PARTIAL — [EXACT hücre + neden + rerun-planı]`.
Ürün-alıcı satırı DEĞİŞMEZ (A2); END-TO-END iddiası YASAK; merge/flip YASAK.

**DUR-SON çıktıları:** hücre tabloları + 25-alan W-2B ledger'ları + PTO-backoff-serileri +
per-cell RTT-realization tablosu + H1 sınıflandırma raporu + enstrüman-zinciri kanıtları +
adversarial sonuç bloğu + commit haritası + status-board güncelleme önerisi + açık riskler.

**DUR.**
