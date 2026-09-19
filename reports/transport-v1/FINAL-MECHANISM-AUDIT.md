# Transport v1 — NİHAİ MEKANİZMA DENETİMİ (SHIP / KEEP-EXPERIMENTAL / DELETE)

Faz-4 denetiminin (`reports/transport-v1-audit/`, 2026-09-08) yeniden ziyareti, Faz 1–5
sonrasında. Her mekanizma tam bir sınıfa düşer. **v1-gerekli hiçbir mekanizmada açık WIRE
kalmamıştır; her default-OFF mekanizmanın ADR/sınır kararı vardır.**

## 17. Mekanizma tablosu

| Mekanizma | v1 durumu | Ürün default | Runtime ulaşılabilir | Kanıt | ADR / karar |
|---|---|---|---|---|---|
| ARQ + selective retransmit (`RetxConfig::fixture`) | **SHIP** | ON | evet | T1a retx 95, T2a 143; `tv_arq`, `sender_loss_path` | freeze |
| Kayıp defteri Suspected/Confirmed/Spurious, `CadenceLegacy` ufku | **SHIP** | ON | evet | T7 `spurious=1`; `tv_loss` | INV-001 |
| Tail-recovery `validated_v1` (3 probe) | **SHIP** | ON (ClientSession) | evet | T0 33 probe, tel quinn +%4; `fs_tailprobe_pins` | freeze; ARCH §4.6 |
| Kuruluş probu DR-EST-1 | **SHIP** | ON | evet | audit ACTIVATION-PROOF; `establish_probe.rs` | freeze |
| İlk-uçuş penceresi [IW] 2 400 B | **SHIP** | ON | evet | T1a/T2b kuruluş; `initial_window.rs` | audit B-6 → freeze |
| Batched ACK/SACK V2 (`seen_count`, ≤16 aralık, R-kota 4) | **SHIP** | ON | evet | T6; `tv_ack`, `receiver_ack_policy` | INV-003 |
| Dönüş-yolu gözlemcisi (Faz-1) | **SHIP** | ON | evet | T6 fwd 45.5‰ ↔ ret 793‰; `return_path` testleri | INV-002 |
| FEC blok kodlama profil 0x05 (K8/R4/E1024), Model-A kısmi blok | **SHIP** | ON (adaptif) | evet | T2a 19 parity, Faz-2 POC; `tv_fec`, `fec_recovery_cell` | ADR-fec-v1-role; INV-013 |
| Tek FEC arbiter `max(confirmed, structure, speculative)` | **SHIP** | ON | evet | manifest `arbiter=[…]`; `fec_loss_risk.rs` | INV-006 |
| Yapı-gate `Q18Iid` @250‰ | **SHIP** | ON | evet | T2a gate açıldı | freeze |
| Spekülatif kol `bounded_v1` (L1/L2 250, P2, G1 rp 300‰) | **SHIP** | ON | evet | T7 reorder bastırma; `product_profile_v1` | FEC v1 kapanışı |
| §8.5 bütçe kısıtı (sıfırlama-reddi) | **SHIP** `[CAL]` | ON | evet | clamp 4 300–13 000/hücre, `would_zero=0` | **ADR-minimal-parity-exception** |
| KURULUŞ-GUARD (PreConverge'de parity 0) | **SHIP** | ON | evet | `preconv_parity_blocked` | audit B-7 |
| Repair kaybı ARQ'ya girmez | **SHIP** | ON | evet | Faz-2 `repair_not_arqed`; T7 | INV-005 |
| §11 Emergency MinimalParity / duplication + hysteresis | **SHIP (wired, birim-testli)** | ON | evet — **T0–T7'de ateşlemedi** | `recover_fwd_entries=0` (330/330); `tv_emergency` | ADR-minimal-parity-exception; LIM-006 |
| Aktivasyon telemetrisi `ActivationDiag` + `rounded_to_zero_count` (Faz-6) | **SHIP** (salt gözlem) | ON | evet | 990 manifest; `sender_diag_contract` | INV-007/015 |
| Pacer `TokenBucket` (BtlBw + queue_delay, burst 2 400 B) | **SHIP** | ON | evet | T3a btlbw 651 B/s; `tv_pace`, `tv_bw` | INV-RATE-1 |
| Kimlik sabitleri `TRANSPORT_V1_PROFILE_ID` / `_ARCHITECTURE` (Faz-6) | **SHIP** | — | — | `transport_v1_freeze.rs`; bench sabitten okur | INV-008 |
| DR-CS-3 bootstrap-RTT merdiveni (`bootstrap_rtt`, `bootstrap_fast_ladder`) | **KEEP-EXPERIMENTAL** | OFF | evet (bayrakla) | T1b BULGU T1b-1; `dr_cs3_*` testleri | **ADR-zero-ack-bootstrap** (LIM-001) |
| Kalibre füzyon politikası (`fusion_policy`) | KEEP-EXPERIMENTAL | OFF | evet (bayrakla) | DR-FUSION-POLICY-1 | audit DEFAULT-OFF |
| `FecStructurePolicy::BurstFusion` | KEEP-EXPERIMENTAL | OFF (`Q18Iid`) | evet | kalibre değil | audit |
| Fragmenter (`max_message_bytes`) | KEEP-EXPERIMENTAL (opt-in) | OFF | evet | S1B testleri | PRODUCT-PROFILE |
| Kanal-türevli confirm ufku (`ChannelDerived`) | KEEP-EXPERIMENTAL (ölçüm kolu) | OFF | evet | `loss-ledger-confirm-defect` | audit |
| Zaman-serisi örnekleyici (`ts_sample_period_ns`) | KEEP-EXPERIMENTAL (diag) | OFF | evet | handoff enstrümanı | INV-007 |
| cargo `experimental-fec-budget` (budget_kernel) | KEEP-EXPERIMENTAL | OFF | feature | kalibre değil | audit |
| cargo `experimental-cold-start` (F2A) | KEEP-EXPERIMENTAL | OFF | feature | DR-CS-3 ile duplicate | audit |
| cargo `diag-sched-telemetry` | KEEP-EXPERIMENTAL (diag) | OFF | feature | vakum-sayaç uyarısı | Faz-3 |
| cargo `experimental-fs-harness` | KEEP-EXPERIMENTAL (harness) | OFF | feature | — | audit |
| `FusionDecision.compress` fuzzy çıktısı (fusion.rs:444) | **DELETE (gelecek temizlik, v1.x)** — kalıntı; tüketicisiz | üretilir, tüketilmez | evet (alan) | zstd yok; TDD §8 AMENDMENT-4 | EVOLUTION §9.8 |
| bench `kama` stub kolu (`mod kama`, `KamaArm`) | **DELETE (gelecek temizlik)** | — | bench | FEC'siz, yanıltıcı; Faz-5'te kullanılmadı | audit UNWIRED |
| bench `serve --ts-out` / `ServerCfg::ts_out` | DELETE (gelecek temizlik) | — | bench | tüketicisiz | audit UNWIRED |
| `EngineError::{FeatureNotImplemented, ParamNotConfirmed, ParamNeverArrived}`, `ParamSetPlan::start_fec_immediately`, `PreParamHold`, `RecoveredMap` (kama-engine) | DELETE (gelecek temizlik) — **tema E, post-v1** | — | kablosuz | audit UNWIRED | Sprint-6/kripto |
| `fate_estimator` modülü (`reordering_guard_ns`, `features_from_fate_bits`, …) | DELETE (gelecek temizlik) | — | test-only | audit UNWIRED | — |
| `rate::evaluate + SrcDecision`, `BtlBwFilter::diag_accept`, `LossDetector::loss_pre_fec_ppm` vb. test-only/none accessor'lar | DELETE (gelecek temizlik) | — | test-only | audit UNWIRED | — |
| `Framing::SegmentBlock`, `RecoveryAction::MinimalParity`, `EmergencyConfig.block_parity_*` | ~~DELETE~~ → **SHIP** | ON | evet | Faz-2 kabloladı (`fec_controller_latch`) | audit kaydı SUPERSEDED |

**DELETE disiplini:** Faz-6'da hiçbir silme YAPILMADI — doğrulanmış ağacı (`e3c1e8bb`)
davranışsal belirsizlikle değiştirmemek için. Silme adayları "gelecek temizlik" olarak
işaretlidir; her biri `git rm` öncesi test-etkisi kontrolü ister. Faz-6 kod değişikliği yalnız:
salt-gözlem sayacı (1) + kimlik sabitleri (2) + bench'in sabiti okuması + 2 test dosyası.

## Faz-4 audit WIRE defteri (93 açık kalem) — kapanış

| tema | kalem | kapatan | kalan |
|---|---|---|---|
| A. dönüş-yolu / asimetri | 20 | Faz-1 (`return_path.rs`, `seen_count` tüketimi, health) | 0 v1-gerekli; `ack_epoch` tüketimi → tema E |
| B. FEC tamamlanmamış parçalar | 25 | Faz-2 (kısmi blok, §8.5, repair fate, MinimalParity) | 0 v1-gerekli; §8.5 send-rate üst sınırı (`u64::MAX`) → EVOLUTION (send_rate zaten BtlBw-güdümlü, loss-kör) |
| C. telemetri | 6 | Faz-3 (`ActivationDiag`) + Faz-6 (`rounded_to_zero_count`) | 0; manifest şeması v2 → EVOLUTION §9.7 |
| D. kıyas metodolojisi | 22 | Faz-4 (TCP kolu, manifest, kurulum, istatistik, fairness) | 0 |
| E. post-v1 / kripto katmanı | 11 | **kapsam dışı (v1-gerekli DEĞİL):** INV-FEC-1 ürün teli, admit_and_decode, `ack_epoch`, `PeerSlot::next_pn`, IdentitySeam↔noise-wire byte paritesi | 11 → `TRANSPORT-CRYPTO-HANDOFF.md` + Sprint-6 |
| F. pacing/CC + diğer | 9 | 3'ü Faz-4 (CC seçimi, kurulum maliyeti, N≥30/CI) | 6 **KEEP-EXPERIMENTAL / EVOLUTION** (aşağıda) |

**Tema F kalan 6 (hiçbiri v1 blocker'ı — gerekçe):** `arq_aggressiveness` fuzzy aktüasyonu
(ARQ bugün `RetxConfig` sabitleriyle çalışır ve T1a/T2a'da yeterli — EVOLUTION) · inflight
`cwnd_bytes` kapısı (pacer BtlBw-güdümlü; kayıp rate'e girmez, INV-RATE-1 sağlam — EVOLUTION) ·
ProbeBW down-leg (§8.3; T4/T5 parite, kuyruk şişmesi gözlenmedi — EVOLUTION) ·
`declare_delivery_deadline` zinciri (`send_with_deadline` API'si var; deadline yalnız
`fusion_policy` açıkken tüketilir, o KEEP-EXPERIMENTAL) · `reordering_score` EWMA
(reorder bastırma `rp_suppress` ile T7'de çalıştı — EVOLUTION) · duplicate-tetikli zorunlu ACK
bütçesizliği (INV-ACK-1 kenarı; T6'da ACK yolu ölçülü sağlıklı — EVOLUTION).

## 18. Sahip karar tablosu (altı Faz-5 kapanış kalemi)

| Kalem | Karar | v1 etkisi | Gelecek eylem |
|---|---|---|---|
| sıfır-ACK kuruluşu | **DOCUMENTED LIMIT** — bootstrap açılmadı | zarf dışı (LIM-001); ADR-zero-ack-bootstrap | v1.1/v2: saha kanıtı → DR-CS-3 kalibrasyonu + T1/T6 yeniden doğrulama |
| FEC maddeleşmesi | **SHIP + DOCUMENTED BEHAVIOR** — profil aynen | FEC açık, nedensellik iddiası yok (LIM-002); ADR-fec-v1-role | yalnız karşı-olgusal kanıtla (EVOLUTION §9.2) |
| telemetri boşluğu | **KAPATILDI** (salt-gözlem `rounded_to_zero_count`) | diag API'de; manifest şeması v1 değişmedi (LIM-003 daraltıldı) | manifest v2'ye taşıma (EVOLUTION §9.7) |
| T5 kapsamı | **VALIDATION LIMITATION** | asimetri ekseni kısmen doğrulanmış (LIM-004); yönlü-gözlem şartı T6 ile | ACK-ağır/doygun-dönüş hücresi (EVOLUTION §9.3) |
| §8.5 / §11 | **SPEC RESOLVED** — `[AMENDMENT-3]` + ADR-minimal-parity-exception; uygulama DEĞİŞMEDİ | çelişki kalktı; §11 saha-doğrulanmamış (LIM-006) | §9.6: ack_confidence köşe ölçümü |
| gecelik CI | **INFRA WAIVER** (`NIGHTLY-INFRA-WAIVER.md`) | kanıt yerel doğrulanmış harness'tan (LIM-007) | sahip: merge + `NETEM_RUNNER=true` + bir kez `run_hostile` |

Hiçbir kalem "TBD" değildir.
