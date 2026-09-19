# KAPSAMA MATRİSİ — spec ↔ kod ↔ ürün ↔ default ↔ aktivasyon ↔ benchmark

Kaynak: 12 küme SPEC→CODE + 12 adversaryal doğrulama + 8 modül CODE→SPEC + bayrak sicili (33 ajan).
Ham veri `phase1-raw/`. Doğrulama: **292 teyit · 71 düzeltme · 29 çürütme · 3 doğrulanamaz**; 67 sınıf
düzeltmesi ve 72 atlanmış mekanizma doğrulama turunda eklendi.

## Özet

| küme | mekanizma | SHIP | WIRE (açık) | KEEP-EXP | DELETE | bu oturumda çözülen |
|---|---:|---:|---:|---:|---:|---:|
| ack-sack | 36 | 23 | 9 | 0 | 3 | 0 |
| arq | 34 | 25 | 5 | 0 | 3 | 1 |
| asymmetry | 35 | 20 | 8 | 6 | 1 | 0 |
| bench-harness | 51 | 16 | 23 | 6 | 4 | 2 |
| burst-random | 29 | 12 | 6 | 3 | 0 | 7 |
| bw-rtt-pacing | 49 | 31 | 9 | 7 | 2 | 1 |
| cold-start | 25 | 13 | 2 | 6 | 1 | 3 |
| delivery-handshake | 62 | 34 | 7 | 18 | 2 | 4 |
| fec-core | 35 | 15 | 12 | 6 | 2 | 1 |
| fec-decision | 30 | 13 | 6 | 6 | 4 | 1 |
| loss-fate | 37 | 25 | 6 | 2 | 4 | 0 |
| tail-probe | 21 | 16 | 0 | 4 | 1 | 0 |
| **TOPLAM** | **444** | **243** | **93** | **64** | **27** | **20** |

`UNCLASSIFIED` 4 kalem: doğrulama ajanının sınıf yerine serbest metin yazdığı kayıtlar — tema tablosunda ilgili başlığa düştüler.

## Bu oturumda kapatılan WIRE kalemleri (20)

| mekanizma | çözen commit |
|---|---|
| [cold-start] Speculative FEC lane (fec_loss_risk, default bounded_v1 = ON) is NOT guarded by [KURULUŞ-GUARD] | `b07d8286 (KURULUŞ-GUARD arbiter kapsamı)` |
| [arq] Segment-block emergency MinimalParity action + Framing::SegmentBlock (§11) | `c9664187 (FEC ürün default'una bağlandı)` |
| [burst-random] Structure-aware FEC gate (structure_gate_fec) — the mechanism that gives burst vs random loss d | `c9664187 (FEC ürün default'una bağlandı)` |
| [burst-random] Single FEC arbiter — final = max(confirmed, structure, speculative), no summation | `c9664187 (FEC ürün default'una bağlandı)` |
| [burst-random] Emergency FEC interlock — parity forced to planned_r = 0 while mode == EmergencyForwardRedundan | `c9664187 (FEC ürün default'una bağlandı)` |
| [cold-start] ACK_FRAME_V2 cold-start measurement extension consumer (seen_count direction-discriminated loss | `c9664187 (FEC ürün default'una bağlandı)` |
| [delivery-handshake] FEC-recovered chunk joins the SAME delivery admission gate (recovery↔reassembly join) | `c9664187 (FEC ürün default'una bağlandı)` |
| [fec-core] Manual / static-r actuation mode (FecActuationMode::Manual + close_fec_block_manual; STATIC_R c | `c9664187 (FEC ürün default'una bağlandı)` |
| [fec-decision] Speculative FEC loss-risk lane — core (Suspected age-weighted risk split, L0/L1/L2 levels, boot | `c9664187 (FEC ürün default'una bağlandı)` |
| [burst-random] FecStructurePolicy::Q18Iid (burst → parity zero) | `c9664187 (FEC ürün default'una bağlandı)` |
| [burst-random] Reorder-pressure guard on speculative FEC (reorder_suppress_permille = 300, C4/G1) and the pers | `c9664187 (FEC ürün default'una bağlandı)` |
| [delivery-handshake] PARAM_SET / PARAM_ACK bootstrap handshake (0x03 / 0x04) — sender side, bounded retry | `c9664187 (FEC ürün default'una bağlandı)` |
| [delivery-handshake] RECOVERED control frame (0x02) — receiver→sender FEC-recovery delivery confirmation | `c9664187 (FEC ürün default'una bağlandı)` |
| [bench-harness] Ürün-default sapması #1: initial_window_bytes (ürün 2400, bench 0) | `c9664187 (bench tek kaynak + manifest)` |
| [bench-harness] Ürün-default sapması #2: structure_gate_level_permille (ürün Some(250), bench None ⇒ 500) | `c9664187 (bench tek kaynak + manifest)` |
| [burst-random] Structure-gate parity level (structure_gate_level_permille / gate_level_permille) | `c9664187 (bench tek kaynak + manifest)` |
| [bw-rtt-pacing] Initial window [IW]: PreConverge first-flight burst budget (burst_lo raised to initial_window.b | `c9664187 (bench tek kaynak + manifest)` |
| [delivery-handshake] Initial-flight window (IW) — PreConverge burst clamp correction | `c9664187 (bench tek kaynak + manifest)` |
| [burst-random] Bench unit test rsweep_caps_flow_to_runtime is arithmetically inconsistent with the uncommitted | `c9664187 (bench tek kaynak + manifest)` |
| [cold-start] Bench arm/product default divergence is not limited to [IW] — the kama-product bench arm cannot | `c9664187 (bench tek kaynak + manifest)` |

## AÇIK WIRE — tema bazında (93; spec-'required' olan 62)


### A. DÖNÜŞ-YOLU / ASİMETRİ — 20 kalem

| küme | mekanizma | spec | güven |
|---|---|---|---|
| ack-sack | Measured return-path bandwidth (return_bw) replacing the 31 B/s prior — §3.3-E FULL (receiver cadenc | required | high |
| ack-sack | ACK decimation (reverse-path impaired → ACK_FREQ↑ / ACK-only budget↓; sparse cumulative ACK in EMERG | required | medium |
| ack-sack | ACK-only return budget fraction Y (RETURN_BUDGET_FRACTION=0.25) / per-profile ACK budget (INV-ACK-1  | required | medium |
| arq | ACK decimation keyed on reverse-path impairment (§2.1/§11/TDD §5.6 sparse ACK) | required | medium |
| asymmetry | E-FULL measured return_bw (§3.3-E FULL / §12 bandwidth-aware ACK cadence from MEASURED return bandwi | required | medium |
| asymmetry | ACK decimation under reverse-path impairment (§2.1/§11/§12: ack_confidence LOW → ACK_FREQ N↑, ACK-on | required | medium |
| asymmetry | emergency::ack_decimation(level) helper predicate | required | medium |
| asymmetry | BtlBw sample validity flags for reverse-path (§7.3 reverse_path_impaired / ack_compression_score / e | required | medium |
| burst-random | Emergency ACK decimation (reverse-path impaired → sparse ACK, ACK_FREQ ↑, ACK budget ↓) | required | high |
| burst-random | Multi-dimensional burst-length signal (burst_p50 / burst_p95 / burst_max) and choose_profile | required | high |
| bw-rtt-pacing | estimator_confidence EWMA + ack_compression_score + reverse_path/ack_delay validity feed (§7.3: bw_e | required | medium |
| fec-core | Emergency-mode minimal parity for segment-block framing (engine §11 / TDD §5.6 FEC branch; RecoveryA | required | high |
| loss-fate | Measured return_bw replacing EAI_RETURN_BW_PRIOR (§3.3-E FULL) — confirm-horizon defect fix | required | medium |
| ack-sack | Extended SACK V2 measurement ext — sender consumer (seen≪sent forward/return-loss split; arrival_rat | experimental | low |
| ack-sack | Return-path budget blindness: three non-SACK return families bypass all ACK accounting | ? | medium |
| asymmetry | INV-ACK-1 'ACK-only budgeted' as an actual return-byte budget (RETURN_BUDGET_FRACTION Y enforced at  | ? | medium |
| bw-rtt-pacing | estimator_confidence EWMA / ack_compression_score / external validity feed | ? | medium |
| bw-rtt-pacing | Duplicated static return-BW prior: 65/31 (computed) vs 2_097_000_000 ns (hardcoded literal) | ? | medium |
| fec-core | R_esi_reserved_unique code-space accounting (fec-core §4 R-ÜÇLÜ, analiz-P0-4) | ? | medium |
| fec-decision | Spec R-triple parity clamp (R_ratio_max / R_wire_max) | ? | medium |

### B. FEC TAMAMLANMAMIŞ PARÇALAR — 25 kalem

| küme | mekanizma | spec | güven |
|---|---|---|---|
| arq | Duplication redundancy-budget clamp (§8.5 / TDD §5.5-§5.6: duplication clamped by available_redundan | required | medium |
| bench-harness | Hizmet-semantiği normalizasyon tablosu (§8.1) — kama-product KAPSAM DIŞI | required | high |
| bench-harness | Sürücü: product_vs_quinn.sh (teşhis-tier ürün-vs-quinn koşumu) | required | high |
| bw-rtt-pacing | reordering_score EWMA (out_of_order/total) → jitter input = max(jitter_score, reordering_score) (§6) | required | low |
| bw-rtt-pacing | §8.5 parity budget clamp f(BtlBw): budget_cap_permille = min(profile_cap, BtlBw×1000/budget_ref) and | required | medium |
| delivery-handshake | IdentitySeam — the only production SecuritySeam implementation (no crypto) | required | high |
| delivery-handshake | Peer identity mapping (SocketAddr ↔ SeamPeer) and peer-slot lifecycle | required | high |
| delivery-handshake | Bidirectional application data plane (server→client DATA / client-side delivery) | required | high |
| fec-core | Sender block accumulation + deterministic block/ESI assignment (unit_id/K) + same-event RS-encode +  | required | high |
| fec-core | Partial/final block coding (Model-A known-zero injection; K_actual < K_shape; final-block padding re | required | high |
| fec-core | §8.5 budget clamp of parity ratio (available_redundancy_budget: parity_final = min(fuzzy, budget_cap | required | high |
| fec-core | Lost-parity accounting (loss-detected FecParity seqs enter pending_lost and are ARQ-retransmitted as | required | high |
| fec-decision | Emergency forward-redundancy — FEC branch (segment-block MinimalParity) and latch emergency interloc | required | medium |
| fec-decision | §8.5 redundancy budget clamp on parity ratio (budget::budget_cap_permille / parity_final) | required | medium |
| arq | available_redundancy_budget parity clamp (§8.5 `parity_final = min(fuzzy_parity, budget_cap, profile | ? | medium |
| arq | Sender::last_decision() has exactly one non-test caller, and it is behind a default-OFF feature | ? | medium |
| burst-random | BurstFusion evidence threshold — prior-as-evidence defect in the shipped branch | not-specified | high |
| burst-random | Unbounded geometric dup_queue reschedule — no COPY_MAX_INTERVAL / RETRY_BUDGET (spec §16.2 BREAK row | ? | medium |
| bw-rtt-pacing | §8.5 upper budget clamp: budget::budget_cap_permille + budget::parity_final + BudgetConfig.budget_re | ? | medium |
| cold-start | Operator `bandwidth_class` prior override for PRE_CONVERGE_FLOOR_BW (§3.3-A) | optional | low |
| cold-start | Confirmed/fuzzy FEC lane also lacks a PreConverge guard (same latch, same reachability) | ? | medium |
| fec-core | On-demand repair from unsent ESI (fec-core §4 'on-demand-repair = önceden-sabit-code-space'teki-GÖND | ? | medium |
| fec-core | Emergency parity interlock (FecSuppressionReason::EmergencyPendingArbitration) | ? | medium |
| fec-core | Golden-vector coverage gap across the profile catalog (§5 GOLDEN-VECTOR-ZORUNLU) | ? | medium |
| fec-decision | Spec R-triple parity clamp (R_ratio_max / R_wire_max) — UNIMPLEMENTED | ? | medium |

### C. TELEMETRİ/GÖZLEM BORCU — 6 kalem

| küme | mekanizma | spec | güven |
|---|---|---|---|
| asymmetry | ACK_FRAME_V2 seen_count (receiver-observed datagram count for forward-loss vs return-loss discrimina | required | medium |
| asymmetry | Path diagnosis PATH_DEAD vs REVERSE_PATH vs CHANNEL_LOSS (TDD §9) and PATH_DEAD_TIMER / T_PATH_DEAD  | required | low |
| burst-random | Emergency forward duplication (Tiny framing) — D× time-spaced copies; dup_spacing as the burst-decor | required | high |
| delivery-handshake | Schema control frames with no product implementation (SACK 0x11, BLOCK_COMPLETE 0x13, BLOCK_ABORT 0x | required | high |
| loss-fate | residual_loss (unrecovered_units/sent_units, post-recovery) — on_unrecovered feed | required | medium |
| loss-fate | diag_loss().2 (pending_lost_now) — third element of the loss-attribution triple never surfaced | ? | medium |

### D. KIYAS METODOLOJİSİ — 22 kalem

| küme | mekanizma | spec | güven |
|---|---|---|---|
| bench-harness | Bench arm: Q-FEC-ISOLATION / Q-FEC-REFERENCE (FAZ-2, FEC atfı için) | required | medium |
| bench-harness | Paketleme asimetrisi: FEC profili 0x03 + 256 B payload → mesaj başına 5 datagram | required | high |
| bench-harness | Reverse-path bozulması + reverse-yön bütçesi/gate-watch | required | medium |
| bench-harness | FEC aktüasyon modu: Manual (statik-r) yapı-gate/arbiter'ı BYPASS eder | required | high |
| bench-harness | Kol etiketleme / provenans (RunMeta.kama_arm + koşu metadata'sı) | required | high |
| bench-harness | Ön-kayıtlı kampanya manifesti (kapi_manifest.py) — kama-product KOL LİSTESİNDE YOK | required | high |
| bench-harness | Birincil metrik EKSİK: blackout survival (ikili) | required | high |
| bench-harness | Birincil metrik EKSİK: operational floor [X_quic, Y_kama] | required | high |
| bench-harness | Mekanizma-kanıtı EKSİK: estimator accuracy + loss/congestion confusion matrix | required | high |
| bench-harness | Kaynak maliyeti metrikleri EKSİK (CPU/byte, bellek, enerji, ARM) | required | high |
| bench-harness | ADR-18 nightly netem kalite kapısı (CI-07) — stub kolu ölçüyor | required | high |
| burst-random | Emergency minimal-parity action (SegmentBlock framing → low-rate forward FEC) | required | high |
| bw-rtt-pacing | Cluster telemetry export to product stats (btlbw, srtt, rtprop, queue_delay, send_rate/pacer rate, p | required | medium |
| fec-decision | Speculative lane — DEADLINE_ASSISTED (deadline/srtt ≤ 2.5 ⇒ L1 uses persistent cap) | required | low |
| bench-harness | Mimari-aktif manifest (`bench --manifest`) — koşu başında etkin config dökümü | not-specified | medium |
| bench-harness | Bench arm: kama security-stub (--transport kama) | ? | medium |
| bench-harness | Wall-clock budget asymmetry: quinn gets a full max_deadline for handshake AND a full per-message dea | ? | medium |
| bench-harness | Idle-timeout / session-lifetime asymmetry (quinn 300 s vs product idle_hold 105 s) | ? | medium |
| bench-harness | Pre-registered analyzer hard-codes the STUB as the primary Kama arm | ? | medium |
| bench-harness | Audit-baseline drift: the flagged defaults were being edited during the audit | ? | medium |
| loss-fate | ledger_evicted_total — SR-AUDIT-001 bounded-ledger eviction audit counter with no product accessor | ? | medium |
| loss-fate | LossConfig / ThresholdLossDetector::with_config — the [CAL] min_delay and 9/8 ratio have no product  | ? | medium |

### E. POST-V1 (S2/kripto katmanı) — 11 kalem

| küme | mekanizma | spec | güven |
|---|---|---|---|
| ack-sack | ack_epoch (rekey/wrap epoch) field + cross-epoch ACK rejection | required | medium |
| ack-sack | Second ACK/SACK codec family — kama-transport-schema Control::Ack (0x10, u64 pn ranges) / Control::S | required | medium |
| bench-harness | Kripto byte-pariteliği: IdentitySeam (+16 B) vs gerçek noise-wire (+26 B) | required | high |
| delivery-handshake | Wire codepoint namespace — v0 shell frame types vs frozen §2.56 inner_kind registry | required | high |
| delivery-handshake | kama_engine::admit_and_decode / ClientSession::admit — architectural decode gate, unwired | required | high |
| fec-core | INV-FEC-1 on the product wire: code-then-encrypt-authenticate-shards / authenticate-before-decode vi | required | high |
| fec-core | Type-gated decode bridge: kama-engine admit_and_decode (OpenedTransportShard → profile-gate → recons | required | high |
| fec-core | Sender/receiver retention, lifecycle and sensitive-symbol hygiene (retention release, TTL, block-com | required | medium |
| ack-sack | PeerSlot::next_pn() — per-peer wire packet number for every sealed return frame | ? | medium |
| asymmetry | ack_epoch cross-epoch ACK rejection (§2 rekey/counter-wrap key domain) — decoded-but-unused, same cl | ? | medium |
| delivery-handshake | Per-peer seam packet-number counter (tx_pn / PeerSlot::next_pn) — INV-NONCE-1 caller-supplied contra | ? | medium |

### F. PACING/CC + DİĞER — 9 kalem

| küme | mekanizma | spec | güven |
|---|---|---|---|
| arq | arq_aggressiveness fuzzy output actuation (§8.4/§5.2: light/med/moderate/reorder-tolerant ARQ) | required | medium |
| bench-harness | Karşı-taraf CC seçimi (`--cc cubic|bbr`) — 'en güçlü mevcut CC' | required | high |
| bench-harness | Kurulum maliyeti (setup cost) muhasebesi — quinn handshake ölçüm penceresi DIŞINDA | required | high |
| bench-harness | İstatistiksel sağlamlık: N≥30 / p95-p99 / %95 CI / git-commit provenansı | required | high |
| bw-rtt-pacing | Inflight byte cap inflight_bytes ≤ cwnd_bytes gate at send (§8.2/§14) and MIN_CWND channel-derived ( | required | medium |
| bw-rtt-pacing | ProbeBW down-leg after probe (pacing_gain = 1.0 − g_dn to drain the probe-built queue) (§8.3) | required | medium |
| fec-decision | Delivery-deadline declaration API (send_with_deadline → declare_delivery_deadline → tightest_deadlin | required | medium |
| loss-fate | reordering_score EWMA (§6 fuzzy jitter input = fuzzify(max(jitter_score, reordering_score))) | required | medium |
| ack-sack | Duplicate-triggered forced ACK is an unbudgeted 1:1 responder (INV-ACK-1 edge) | ? | medium |