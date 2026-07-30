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
| **Birleşik uçtan-uca 65B-heal (sender+receiver TEK-akışta)** | ⚠️ KISMİ | sender-PostTimeoutNotice (F4) ve receiver-delivery (F5) AYRI-testlerde; birleşik-tek-akış YOK |
| **Gerçek RSS-plateau ölçümü** | ⚠️ KISMİ | deterministik-defter/model-assert; gerçek-RSS + netem-hostile self-hosted-nightly (§4.4) DIŞI |
| **Gerçek handshake-Split key** | ⚠️ KISMİ | mock-symmetric-key; gerçek-Noise-handshake ayrı |

Kapılar (her faz): fmt + clippy(-D, iki-config) + FLAG-OFF **380/0** + ALL-FEATURES **491/0** +
A1-pinleri (cold_start_liveness 4 · dr_cs1_acceptance 22 · dr_cs1_wire 12 [g5-amendmanlı]) + unsafe-YOK.

## 3. HÜKÜM

> ## **PRODUCT COLD-START PARTIAL — birleşik-uçtan-uca-65B-heal-akışı + gerçek-RSS/netem-ölçümü + gerçek-handshake-key EKSİK**

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
(i)   Birleşik uçtan-uca 65B-heal integration-testi (sender+receiver tek-akış; kama-integration)
(ii)  Gerçek-RSS/allocator plateau + netem 7-hostile-profil (self-hosted-nightly §4.4)
(iii) Gerçek Noise-handshake-Split key (mock→gerçek)
(iv)  DUR-B-4b: roadmap:61-63 SUPERSEDED-by-B1 üst-not (bu DUR-SON-yetkisiyle status-board)
```
