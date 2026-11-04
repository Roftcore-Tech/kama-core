# Kama Transport v1 — ÜRÜN PROFİLİ SÖZLEŞMESİ (`KamaV1Profile`)

> **YAMA KAYDI — 2026-10-24 [v1.1 · K4G-05b — AMENDMENT-12 işaretçisi; sahip kararı CEVAP PAKETİ 7, 2026-10-18, DUR-4 (b) — `docs/decisions/K4G-CEVAP-PAKETLERI-4-7.md`]:** Bu sözleşme v1'i (`kama-v1-full`) tanımlar; v1.1'de KURAL satırındaki ve "Kimlik" tablosundaki kimlik `kama-v1.1-full` / `transport-v1.1`'dir (`TRANSPORT_V1_PROFILE_ID` / `TRANSPORT_V1_ARCHITECTURE`). `RuntimeConfig` alanları ve tablolardaki değerler değişmez; davranışı değişen satırlar AMENDMENT-12'ye tabidir (otorite: `specs/transport-layer-tdd.md` §12 [AMENDMENT-12]): Tail-recovery (`max_tail_probes 3` bütçesi bitince kopya kuyruğuna devir; "backoff tavanı 30 s" = normatif RTO_MAX, aynı zamanda kopya aralığı tavanı) · ARQ ("cap aşımı → HandToForward" düşürmesiz, kalıcı kopya sahipliğine devir). Bu belgenin metni değiştirilmedi; düzeltmesi "INT-02 GEÇTİ" sonrası ayrı doküman PR'ı.

Kaynak: `crates/client-core/src/runtime.rs` (`KamaV1Profile::runtime_config()`; `RuntimeConfig::default()`
buna devreder). Değerler koddan okunmuştur (2026-09-11), tahmin değildir. Pin testleri:
`crates/client-core/tests/{product_profile_v1,transport_v1_freeze}.rs`; bench kimlik testleri
`crates/bench/src/{manifest,kama_product}.rs`.

> **KURAL:** Bu profilden herhangi bir alanda sapan runtime yapılandırması Kama Transport v1
> DEĞİLDİR ve `kama-v1-full` etiketi alamaz; benchmark bunu `kama-custom` + sapma listesi olarak
> üretir (INV-008/009).

## Kimlik

| | değer | kaynak |
|---|---|---|
| profil kimliği (benchmark `arm`) | `kama-v1-full` | `TRANSPORT_V1_PROFILE_ID` |
| mimari kimliği | `transport-v1` | `TRANSPORT_V1_ARCHITECTURE` |
| manifest şeması | `kama-bench-manifest/v1` | `bench::manifest::MANIFEST_SCHEMA` |

## `RuntimeConfig` alanları

| alan | donmuş v1 değeri | sınıf | neden | ADR / spec | doğrulama |
|---|---|---|---|---|---|
| `fec` | `Some{profile_id: 0x05, manual_r: None}` — K_shape 8, R_max 4, E 1024, adaptif (Controller) | **SHIP** | denetim B-1: `None` FEC alt-sistemini yok bırakıyordu; 0x05 birimi 1013 B'de tutar (0x03/E=64 → 53 B) | ADR-fec-v1-role; engine §8–§9 | T2a/T2b/T6/T7'de parity ateşledi; `fec_v1_profile_is_symbol_aligned` |
| `max_unit_bytes` | `1024 − SOURCE_PREFIX_LEN(11) = 1013` | SHIP | birim tam bir FEC source-symbol'üne oturur (§2.6 symbol_len) | engine §2.6 | `fec_v1_profile_is_symbol_aligned` |
| `max_message_bytes` | `None` (fragmenter KAPALI, açık opt-in) | SHIP | açmak her DATA'ya +11 B prefix = tel görünür değişiklik; düşük-bant hedefinde yanlış takas | audit DEFAULT-OFF | T0–T7 256 B yükle (< 1013) |
| `initial_window_bytes` | `2400` (= `DEFAULT_BURST_CAP` = 2×MTU 1200) | SHIP | eski `0` bir CLAMP hatasıydı; ilk uçuş tek paket → kuruluş yazı-tura | audit B-6, `[IW]` yorumu | T1a/T1c/T2b kuruluş; `product_v1_profile_is_pinned` |
| `structure_gate_level_permille` | `Some(250)` (r=2 @ K=8) | SHIP | r-sweep: 500‰ 15–20 % kayıpta israflı | `reports/fec-loss-risk-v1/rsweep` | T2a gate açıldı |
| `fec_loss_risk` | `FecLossRiskConfig::bounded_v1()`: enabled, L1/L2 cap 250/250‰, `persist_mode = LearnedSpuriousAge`, persist_srtt 500‰, ρ 1250‰, Q 950‰, `reorder_suppress = 300‰` (C4 guard G1) | SHIP | FEC v1 kapanışı (aday A + P2 + G1) | `reports/fec-loss-risk-v1/closure` | T7 `spurious = 1`; pin testi |
| `fec_structure_policy` | `Q18Iid` | SHIP (`BurstFusion` KEEP-EXPERIMENTAL) | BurstFusion kalibre edilmemiş | audit DEFAULT-OFF | T2a/T2b davranışı Q18Iid ile |
| `loss_confirm_horizon` | `CadenceLegacy` | SHIP (`ChannelDerived` ölçüm kolu) | düzeltme = ölçülen return_bw (spec ön-yetkili), k×srtt DEĞİL | `reports/loss-ledger-confirm-defect` | Faz-1 ölçülen return_bw kablolandı |
| `fusion_policy` | `None` | KEEP-EXPERIMENTAL | opt-in ölçüm kapısı (DR-FUSION-POLICY-1); açıkken yapı-gate susar | audit | — |
| `bootstrap_rtt` | `false` | KEEP-EXPERIMENTAL (belgelenmiş sınır) | DR-CS-3 merdiveni kalibre edilmemiş | **ADR-zero-ack-bootstrap** | T1b BULGU T1b-1 |
| `bootstrap_fast_ladder` | `false` | KEEP-EXPERIMENTAL | `bootstrap_rtt` bağımlı | ADR-zero-ack-bootstrap | — |
| `quota_r` | `4` | SHIP | §16.1: R=8 dar dönüşte kota doyumu 13/20 | engine §16.1 | T6 |
| `max_inflight_units` | `4096` | SHIP | unit-store sınırı (SR-AUDIT-001 bounded) | — | — |
| `offer_queue` / `delivery_queue` | `1024` / `1024` | SHIP | bounded mpsc | — | — |
| `max_peers` | `64` | SHIP | SESSION_COUNT_CAP hizası (fail-closed) | — | — |
| `drain_cap` | `3 s` | SHIP | motor birim terk etmez → drain CAP'siz sonlanmayabilir | — | — |
| `idle_hold` | `105 s` | SHIP | SESSION_IDLE_HOLD hizası | — | — |
| `ts_sample_period_ns` | `None` | diagnostic (salt gözlem) | handoff enstrümanı | — | INV-007 |
| `live_diag` | `false` | diagnostic (salt gözlem) | K4G-05 canlı teşhis aynası ve yazıcısı (`ClientLiveDiag`); çalışma-zamanı bayrağı, feature değil; kapalıyken yazıcı çalışmaz, açıkken motor kararları değişmez, maliyeti ölçülmedi; INT-02 düzeneği açar (2026-10-24 eklendi) | DUR-19 (CEVAP PAKETİ 9) | `product_v1_profile_is_pinned` |

## `ClientSession`'ın kurduğu, `RuntimeConfig` DIŞI sevk ayarları

| mekanizma | donmuş v1 değeri | sınıf | doğrulama |
|---|---|---|---|
| Tail-recovery | `TailRecoveryConfig::validated_v1()`: enabled, PTO 10 ms..10 s, initial 1 s, backoff tavanı 30 s, max_ack_delay 25 ms, **max_tail_probes 3** | SHIP | T0: 33 probe/100 msg, toplam tel quinn +%4 (patolojik değil) |
| Kuruluş probu (DR-EST-1) | ON, PreConverge'de, token-muaf | SHIP | audit ACTIVATION-PROOF |
| ARQ | `RetxConfig::fixture()`: retx cap 3, reorder_k 3, ρ 2/1, cap aşımı → HandToForward | SHIP `[CAL]` | T1a retx 95, T2a 143 |
| §8.5 bütçe | `BudgetConfig::fixture()`: profil tavanı 900‰, min_rate 31 B/s, budget_ref 125 kB/s; sıfırlama-reddi | SHIP `[CAL]` | ADR-minimal-parity-exception; clamp 4 300–13 000/hücre |
| §11 emergency | `HysteresisConfig::fixture()` 300/700‰, dwell 1 s; `EmergencyConfig::fixture()` Dup 2×/3×, 20 ms, MinimalParity 100/200‰ | SHIP (wired) — **saha-doğrulanmamış** | ~~`recover_fwd_entries = 0` (330/330)~~ **[ERRATUM-2 (2026-10-18)]** `recover_fwd` > 0 15/330 koşuda (T1a 2, T1b 7, T6 6), §11 MinimalParity emisyonu ayrı sayaçla ölçülmedi; LIM-006 |
| Alıcı ACK/SACK | `ACK_FREQ_N = 2`, SACK 65 B, `MAX_ACK_RANGES = 16`, R-kota 4, return-bw önseli 31 B/s | SHIP | T6 dönüş-yolu |
| Dönüş-yolu gözlemcisi | EWMA α 250‰, min yön kanıtı 8 datagram, min ACK örneği 4, Degraded ×3 / Dead ×12 boşluk | SHIP | T6: fwd 45.5‰ ↔ ret 793‰ |

## Cargo feature'ları (hepsi KAPALI; ürün derlemesi feature'sız)

`experimental-fec-budget` (budget_kernel, kalibre değil) · `experimental-cold-start` (F2A; DR-CS-3
ile duplicate) · `diag-sched-telemetry` (salt gözlem, `KP-ACTIVATED tail_probe=` alanı) ·
`experimental-fs-harness` (kama-integration). Faz-5 her-zaman-açık aktivasyon sayaçları
(`ActivationDiag`) feature'sızdır.

## Benchmark tarafında karşılığı

`bench client --transport kama-product` bayraksız = bu profil. `--fec-profile` varsayılanı
`KamaV1Profile::FEC_PROFILE_ID`'ye bağlıdır (literal değil). `profile_delta(&rt_cfg)` alan alan
kıyaslar; boş değilse `arm = kama-custom`.
