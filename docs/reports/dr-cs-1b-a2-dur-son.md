# DR-CS-1b / A2 — DUR-SON: KABUL-MATRİSİ + HÜKÜM

**Statü:** A2 full-scale production execution kapanışı (Abkarada-direktif; lead-engineer-modu).
Faz-zinciri F1-C → F2 → F-CID-0/CAL-10 → F3 → F4 → F5 tamamlandı. **Üretim-kodu flag-arkası
(`experimental-cold-start` DEFAULT-OFF); flag-OFF byte-özdeş (INV-CS-5); merge/default-flip YAPILMADI.**

## 1. TAMAMLANAN ÜRÜN DAVRANIŞI (commit zinciri)

| Faz | Commit | Çalışan ürün davranışı |
|---|---|---|
| F1-C | 1bbb234 | PersistentReplayGuard REPLAY_PAIR_CAP=128 + RECENT=16; high-water correctness; per-pair ≤3328B ölçüm |
| F2 | 8249f81 | **Gerçek product ingest**: h_ext-demux → AEAD-open → iki-seviye cold-admission → block-assembly → K-decode → app-delivery → release; non-renewable patience-deadline + S-B |
| F-CID-0 | d0dd791 | ControlIdIssueGate (monoton+burst+wrap); CAL-10 CONTROL_DEDUP_WINDOW=4096 SEALED (addendum a82efd24) |
| F3 | 4ebe4b9 | Fixed-window control dedup (session-scoped; probe-dışı; Closing-drop); g5 DUR-B-4a-amendman |
| F4 | 8ad718d | PostTimeoutNotice (sender Tetik-2a/2b; first-valid-proof; N8 bit-özdeş); RECOVERED-emit (receiver, ControlIdIssueGate) |
| F5 | 28af75f | Pre-registered plateau (F-A/B/C) + product W-3-65 rerun (3× repeatable) |

## 2. KABUL-MATRİSİ (roadmap-A2 kabul-kriteri: DI-10-ürün + W-3-65-ürün-rerun + plateau)

| Kabul kalemi | Durum | Kanıt |
|---|---|---|
| DI-10 ürün-varyantları (cap-admission, stateless-echo, probe-dedup-dışı) | ✅ PASS | F2 cold_admit + F3 probe-dedup-girmez + cap-typed-reject |
| İki-seviye ATOMİK admission (K-9) | ✅ PASS | F1-A ledger + F2 gerçek-ingest |
| Non-renewable patience + resource release | ✅ PASS | F2 deadline-evict + release; F5 plateau |
| Tombstone reserve-before-release + grace | ✅ PASS | F1-B token-RAII + F3.5 Closing-drop |
| A-4 send-retirement + A-5 symbol-zeroize | ✅ PASS | F1-B 5-pin + A-5 5-pin |
| Fixed-window dedup (CAL-10=4096) | ✅ PASS | F3 4-birim + F5 F-C-flood |
| Recovery-notification (sender+receiver mekanizma) | ✅ PASS | F4 sender-4 + receiver-1 |
| Plateau F-A/F-B/F-C (deterministik defter) | ✅ PASS | F5 3-flood |
| Product W-3-65 delivery + cap-stress | ✅ PASS | F5 3× repeatable |
| **Birleşik uçtan-uca 65B-heal (sender+receiver TEK-akışta)** | ✅ PASS (CLOSURE-1B) | AYRI directional-key tek-akış; sender bootstrap→TimedOut→receiver decode/delivery→RECOVERED-emit→gerçek open/decode/RecoveredMap-bridge→sender PostTimeoutNotice; 3-tekrar + 6-negatif-pin + 300B-DataAckClocked-smoke (a2_c1b_unified_heal; commit c710cff) |
| **Gerçek RSS-plateau ölçümü** | ✅ PASS (CLOSURE-2) | gerçek ReceiverHost product-path + /proc/self/statm RSS + FAITHFUL two-netns netem 7-profil×3 (21/21 HARD-boundedness; retained max=%0.21-cap; cleanup→0; typed-reject evrensel; commit 8a1616b) |
| **Gerçek handshake-Split key (hücre-iii)** | ⚠️ AÇIK (Sprint-6) | CLOSURE-1B directional-key kullandı (TEST-ONLY; "handshake-derived" İDDİA-EDİLMEDİ); gerçek-Noise-handshake-Split SEALED Sprint-6 kripto — Abkarada'nın-bağlayıcı-kararıyla ERTELENDİ |

Kapılar (her faz): fmt + clippy(-D, iki-config) + FLAG-OFF **380/0** + ALL-FEATURES **491/0** +
A1-pinleri (cold_start_liveness 4 · dr_cs1_acceptance 22 · dr_cs1_wire 12 [g5-amendmanlı]) + unsafe-YOK.

## 3. HÜKÜM

> ## **PRODUCT COLD-START PARTIAL — real Noise handshake-derived Split key EKSİK (Sprint-6 kripto)**
>
> **CLOSURE-TURU-SONRASI GÜNCELLEME (Abkarada FINAL VALIDATION CLOSURE DIRECTIVE):** hücre-(i)
> birleşik-uçtan-uca-65B-heal → **CLOSURE-1B PASS** (AYRI directional-key; commit c710cff) · hücre-(ii)
> gerçek-RSS/netem → **CLOSURE-2 PASS** (faithful two-netns 7-profil; commit 8a1616b). Kalan-TEK-hücre
> hücre-(iii) gerçek-Noise-handshake-derived-Split-key'dir (SEALED Sprint-6). Abkarada BAĞLAYICI-kararı:
> "Closure-2 PASS olsa bile PARTIAL — real handshake-derived Split key eksik". Hüküm PARTIAL korunur.

**Gerekçe (dürüst-ölçüm-disiplini):** Ürün-alıcı MEKANİZMA-BÜTÜNLÜĞÜ TAM ve gerçek `ReceiverHost` üzerinde
ÇALIŞIR (ingest→admission→decode→delivery→release; patience; tombstone; dedup; recovery-notification;
491-test yeşil). Ancak "END-TO-END PRODUCT COLD-START FIX VALIDATED" tam-hükmü ŞU üç hücre olmadan
kurulamaz: (i) sender-bootstrap→TimedOut→receiver-decode→RECOVERED→sender-PostTimeoutNotice zincirinin
TEK birleşik-akışta doğrulanması (şu an mekanizma-yarımları ayrı-crate-testlerinde kanıtlı); (ii) gerçek-RSS
plateau + netem-hostile-profil (self-hosted-nightly §4.4; şu an deterministik-defter-model); (iii) gerçek
Noise-handshake-Split-key (şu an mock-symmetric). Bunlar mekanizma-EKSİĞİ DEĞİL, VALIDATION-DERİNLİĞİ
eksiğidir — kanıtsız-tam-iddia yerine dürüst-kısmi tercih edildi.

**KORUNAN SINIRLAR:** A1 SEALED (yeniden-açılmadı). 1200B-yasak-iddialar korundu. ARQ-vs-FEC-karşılaştırma
kurulmadı. Merge/default-flip YAPILMADI (ayrı-karar). Tüm [CAL]-SEALED değerler gevşetilmedi.

## 4. DEFAULT-FLIP (AYRI KARAR)
Her durumda ayrı: coordinated-rollout + min-peer-version + federation-policy + §4.4-benchmark sonrası.
Bu DUR-SON default-flip ÖNERMEZ.

## 5. KAPANMAMIŞ HÜCRELER → SONRAKİ (Abkarada-kararına)
```text
(i)   Birleşik uçtan-uca 65B-heal integration-testi   → KAPANDI (CLOSURE-1B; c710cff)
(ii)  Gerçek-RSS/allocator + netem 7-hostile-profil    → KAPANDI (CLOSURE-2; 8a1616b; faithful two-netns)
(iii) Gerçek Noise-handshake-derived Split key         → AÇIK (SEALED Sprint-6 kripto; Abkarada-ertelemesi)
(iv)  DUR-B-4b: roadmap:61-63 SUPERSEDED-by-B1 üst-not (bu DUR-SON-yetkisiyle status-board)
```

## 6. CLOSURE-TURU KANIT-ZİNCİRİ
```text
CLOSURE-1B (c710cff): a2_c1b_unified_heal — AYRI directional-key birleşik-65B-heal (3-tekrar) +
  6-negatif-pin (wrong-key/reversed-direction/stale-epoch/duplicate/same-epoch-diff-block/no-proof) +
  300B-DataAckClocked-smoke. Ürün-yüzeyi: install_session_with_directional_keys. NEG-5-ifade-sapması:
  direktif "InvariantViolation" der; sealed-uygulama first-valid-proof-SUPPRESSION (üretim-değişmedi).
  Kapılar: FLAG-OFF 380/0 (INV-CS-5) · ALL-FEATURES 491→499 · A1-pin sağlam.
CLOSURE-2 (8a1616b): a2_rx_netem_runner (gerçek ReceiverHost + /proc/self/statm) × FAITHFUL two-netns
  netem 7-profil×3. HÜKÜM PASS. Ölçüm-geçerlik: tek-netns-veth netem-inert (SESSİZCE-yok-sayar) →
  YAKALANDI (faithful-netem-harness) → two-netns'e-taşındı (delay 200ms→200.27ms doğrulandı).
  Pre-reg v2 sha256=d7b9aea0 (koşu-öncesi FROZEN). Rapor: reports/d1-exit/A2-CLOSURE-2-results.md.
```
