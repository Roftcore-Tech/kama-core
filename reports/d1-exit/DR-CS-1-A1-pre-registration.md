> ## ⛔ SUPERSEDED — ARŞİV (Abkarada DUR-B kararı, 2026-07-26)
> Bu belgenin **W-2B** bölümü (§3, `retransmit_path=ExistingPtoArq` varsayımı) hükümsüzdür:
> ```
> REJECTED AS MECHANISTICALLY INAPPLICABLE —
> FULL DATA-ACK/SACK SUPPRESSION PREVENTS §9 ARQ FROM FIRING.
> SUPERSEDED BY W-2B-R2.
> ```
> W-2B → **W-2B-R2** (yeni pre-registration: `DR-CS-1-A1-R2-pre-registration.md`). **W-1/W-3/DI-2B/DI-10/mixed
> bölümleri + §1 [CAL] tablosu (HORIZON/RX_PATIENCE/T_MAX/CAP/RTT-tol) GEÇERLİ kalır.** W-3-65 kabul-kriteri
> B1-kararıyla güncellendi (bkz. R2 pre-reg). Bu belge arşiv-deseniyle KORUNUR; içeriği/sonucu DEĞİŞTİRİLMEZ.

# DR-CS-1 TUR-A1 — PRE-REGISTRATION (F-A1-0 artefaktı)

> **BAĞLAYICI ÇERÇEVE:** Bu deney ARQ ile FEC'i karşılaştırmaz; ARQ tüm kollarda aktiftir. DR-CS-1 bir
> cold-start LIVENESS/bootstrap DR'ıdır. **PRODUCTION CODE DEĞİŞİKLİĞİ SIFIR** (yalnız harness/test-only).

**Amaç (talimat §0.2/6):** W-3-65 ve W-2B hücrelerinin beklenen-zincirlerini, tüm [CAL] değerlerini ve
§5 kabul-eşiklerini **HERHANGİ BİR KOŞU YAPILMADAN ÖNCE** dondurmak. Bu belgenin git-commit'i (hash +
author-date) pre-registration anchor'ıdır. **Sonuca bakıp değer/beklenti/[CAL] seçmek YASAK.**

**Dayanak talimat:** `docs/tasks/dr-cs-1-tur-a1-talimati-v2.md` · **Roadmap:** `docs/roadmap/kama-transport-roadmap-status-board.md`

---

## 1. DONDURULMUŞ [CAL] TABLOSU

| Parametre | Donmuş değer | Gerekçe |
|---|---|---|
| `OBSERVATION_HORIZON` | **120_000 ms** (120 s) | restore=61s sonrası post-blackout PTO-backoff serisinin tamamlanabilmesi (gözlenen 1200B-heal@83.2s referans; cömert pay) |
| `RX_TEST_PATIENCE` | **130_000 ms** (130 s) | Gap-3 normatif: `≥ OBSERVATION_HORIZON + RTT_bound(≤1s) + processing_margin(≥9s)`. YALNIZ test-harness gözlem kontratı — ürün patience DEĞİL (A2 işi) |
| `T_MAX` (W-2B) | **30_000 ms** (30 s) | suppression-başlangıcından; zarf ≤13s (aşağıda) ≪ 30s cömert-pay |
| `SUPPRESSED_ACK_CAP` (W-2B) | **64** | suppressed_ack sınırsız OLAMAZ; aşımda release + FAIL |
| RTT-tolerans p50 | **`\|measured_p50 − configured\| ≤ max(5ms, %5)`** | per-cell RTT-realization kabul (talimat §0.2) |
| RTT-tolerans p95 | **`measured_p95 ≤ configured + max(20ms, %10)`** | per-cell RTT-realization kabul (talimat §0.2) |

**T_MAX zarf-gerekçesi (donmuş):**
```text
T_FIRST_PROBE               = 3 s   (ladder ilk-fire; INV-CS-7)
RTT_MAX_CELL                = 1 s   (matris üst-RTT = 1000ms)
PTO_AFTER_BOOTSTRAP_BOUND   ≤ 3 s   (≤ 3×srtt bootstrap-sonrası)
retransmit_dispatch_bound   ≤ 1 s
cömert-pay                  ≤ 5 s
────────────────────────────────────
TOPLAM                      ≤ 13 s  <  T_MAX = 30 s
```

---

## 2. W-3-65 (restore=61) — DONMUŞ BEKLENEN ZİNCİR

**Kurulum (donmuş):** faithful two-netns · 65B × 16 · çift-yön blackout t0→restore=61s → temizle ·
**settle-exit KAPALI** · OBSERVATION_HORIZON=120s · RX_TEST_PATIENCE=130s · 3 tekrar.

```text
TimedOut@60s (epoch'ta BİR kez) → restore@61s → sonraki mevcut-PTO deadline
(backoff-serisi RAPORLANIR) → retransmit wire-emission (mevcut ARQ yolu) →
DATA-ACK → PostTimeoutRecovery (ikinci terminal YOK; busy-loop YOK)
```

**Zorunlu ledger alanları:** `timeout_time · restore_time · pto_deadline_series · retransmit_wire_time ·
ack_time · transport_duplicate_rx_count · application_delivery_count · application_duplicate_delivery_count ·
terminal_class · recovery_class`.

**Donmuş kabul:** 3/3 → `PostTimeoutRecovery` horizon içinde; post-condition (`next_wakeup ∈ {None, gelecek}`)
her batch'te. **Sonuç sınıfları:** `PASS 3/3` **veya** `UNOBSERVED-AGAIN — [EXACT]` (horizon yetmezse:
gözlenen PTO-serisi + gereken-ufuk hesabı → DUR-B; yeni ufuk ancak YENİ pre-registration ile).

---

## 3. W-2B (faithful ACK-starvation) — DONMUŞ ZİNCİR + ASSERT'LER

**Kurulum (donmuş):** çift-yön açık (yalnız delay; loss=0) · rx-tarafı deterministik suppression (t=0'dan
tüm matching DATA-ACK/SACK bastırılır; **PROBE-ECHO 0x1A HER ZAMAN geçer**) · settle-exit KAPALI.

**Release modeli (donmuş — ARRIVAL-TRIGGERED):** ilk matching PTO retransmit ARRIVAL at receiver (aynı
unit-id/seq için ilk transport-duplicate DATA arrival → rx dedup seq-already-seen isabeti) → suppression,
ACK üretilmeden HEMEN ÖNCE bırakılır. Sender-ledger, arrival'ın bir EXISTING-PTO wire-emission'ından
geldiğini AYNI unit-id ile kanıtlar.

**Donmuş nedensellik-zinciri (ledger'da assert):**
```text
suppression_start < probe_wire < echo_rx ≤ bootstrap_srtt_time
≤ pto_eligible ≤ pto_enqueue ≤ pto_wire ≤ retransmit_rx
≤ suppression_release ≤ released_ack_wire
```
**Ek donmuş assert'ler:** `ack_accepted_before_bootstrap_count = 0` · `srtt_source = ProbeEcho` ·
`retransmit_path = ExistingPtoArq` · `new_retransmit_path_count = 0` · `bootstrap_induced_retransmit_bytes`
YALNIZ pto_wire↔retransmit_rx **unit-id eşleşmesiyle** ilişkilendirilir (zamansal korelasyon tek başına
kanıt DEĞİL).

**Donmuş beklenen desen (spurious gerçekleşince):**
```text
transport_duplicate_rx_count        ≥ 1
application_duplicate_delivery_count = 0
application_delivery_count           = beklenen-birim-sayısı (her birim BİR kez)
```
("dup=0" tarzı tek-sayaç YASAK — dedup-başarısı ile retransmit-hiç-ulaşmadı AYRIŞIR.)

**Donmuş 25-alan ledger:** talimat §2.3 tablosu (1..25) aynen — slash'lı kısaltma YOK.

**Donmuş matris:** RTT {100, 250, 1000} × size {65, 300, 1200} × 3 tekrar = **27 koşu**; her satır per-cell
RTT-realization alanlarıyla.

---

## 4. DONMUŞ §5 KABUL EŞİĞİ

```text
FULL için ZORUNLU:
  W-3-65  = 3/3 PASS
  W-2B    = 27/27 PASS
  H1 current-evidence audit = CLEAN
```
- `26/27` OTOMATİK FULL YAPMAZ → PARTIAL + exact-failing-cell + deterministik rerun + kök-neden.
- Enstrüman-zinciri geçersiz koşu FAIL SAYILMAZ → `INVALID — NO EVIDENCE` + yeniden koşum.
- **Hüküm satırları:** `DR-CS-1 SENDER-SIDE ACCEPTANCE: FULL — [W-3-65: 3/3 · W-2B: 27/27 · H1: CLEAN]`
  **veya** `DR-CS-1 SENDER-SIDE ACCEPTANCE: PARTIAL — [EXACT hücre + neden + rerun-planı]`.
- Ürün-alıcı satırı DEĞİŞMEZ (A2); END-TO-END iddiası YASAK; merge/flip YASAK.

---

## 5. ENSTRÜMAN ZİNCİRİ (her faithful-koşuda ZORUNLU; biri kırmızı → INVALID — NO EVIDENCE)
1. Preflight faithful acceptance 6/6 @ RTT100 (oturum başına).
2. Per-cell RTT-realization: measured_rtt_p50 / p95 / tolerance_result (forward/reverse AYRI).
3. Overlap-gate: `receiver_events > 0`.
4. Binary-provenance: byte-boyut + feature-kimlik (bin_cs ≠ bin_off).
5. Argüman-paritesi: kollar birebir-arg; arg-diff rapora.
6. Clock-domain: tüm ledger tek host CLOCK_MONOTONIC; ledger'a clock-domain satırı.

---

## FREEZE-BEYANI
Bu belge, W-3-65 ve W-2B koşuları YAPILMADAN ÖNCE commit edilmiştir. Bu belgenin git-commit-hash'i ve
author-date'i pre-registration anchor'ıdır. Yukarıdaki hiçbir [CAL] değeri, beklenen-zincir veya eşik,
koşu-sonucuna bakılarak değiştirilemez (talimat §0.2/6 + §5). Değişiklik gerekirse → YENİ pre-registration
(yeni hash+timestamp) + DUR-B.
