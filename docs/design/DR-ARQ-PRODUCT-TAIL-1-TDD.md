# DR-ARQ-PRODUCT-TAIL-1 — TDD (readiness audit · deadline-feasibility · versioned INV-CS-4 supersession)

> **YAMA KAYDI — 2026-10-24 [v1.1 · K4G-05b — AMENDMENT-12 işaretçisi; sahip kararları CEVAP PAKETİ 7, 2026-10-18, DUR-4 (b); CEVAP PAKETİ 9, 2026-10-24 (RTT kabulü, devir) — `docs/decisions/K4G-CEVAP-PAKETLERI-4-7.md`]:** §4 karar bloğunun RESET/CANCEL, BOUND, PROBE LOSS ("then give up (bounded failure) — no infinite retransmit") ve FAIL-CLOSED ("never unbounded emission") satırları v1.1'de AMENDMENT-12'ye tabidir (otorite: `specs/transport-layer-tdd.md` §12 [AMENDMENT-12]): MAX_TAIL_PROBES bütçesi tükenince birim düşürülmez, aynı turda kopya kuyruğuna devredilir ve teyide ya da oturum sonuna dek RTO_MAX = 30 s tavanlı aralıkla yeniden gönderilir; prob kopyası da later-ACK kanıtıyla kayıp ilan edilip yeniden planlanır; sıfırlama kuralları AMENDMENT-12 "Sıfırlama" paragrafındadır. §2 "retx RTT-sample safety" satırı ve §4 KARN satırı v1.1'de engine §6 KABUL KURALI'na tabidir: prob kopyası taze seq taşır, örnek o iletimin kendi gönderim anından alınır (Karn dışlaması kalktı). Prob mekanizmasının kendi bütçesi (`max_tail_probes`) DEĞİŞMEDİ. Bu belgenin metni değiştirilmedi; düzeltmesi "INT-02 GEÇTİ" sonrası ayrı doküman PR'ı.

> Versioned transport decision / TDD amendment for a PRODUCT bounded tail-loss recovery mechanism in the actual
> transport core. Supersedes INV-CS-4's "no Pto retransmit path" EXPLICITLY (not silently). Product-baseline scope
> only — NOT production deployment, NOT RFC 9002 compliance. A0-TAIL (harness, 5cfbe37) is a behavioral oracle only.

## §2 Readiness audit (product transport core — mechanical map)
```text
Timer::Pto              produced+consumed in Sender::on_event; current core-path emits NO new retransmit (INV-CS-4).
INV-CS-4 exact scope    "mevcut Timer::Pto core-yolu — YENİ-retransmit-yolu-YOK" (sender.rs ~L126). Cold-start Pto is
                        used for rate/pacing/CS-probe (BootstrapPto), NOT tail retransmission.
ACK progress            Event::AckReceived(AckInfo{acked_seqs, ack_delay}) → inflight removal + pending_lost.
outstanding tracking    inflight: BTreeMap<seq, InflightPacket> + inflight_bytes (ack-eliciting source in flight).
send timestamps         InflightPacket carries original send instant (un-loss original-send anchor).
RTT samples             RttEstimator (rtt.rs): srtt_ns, min_rtt_ns, rttvar_ns; apply_ack_delay(); confidence = ≥1 sample.
delayed-ACK             apply_ack_delay(ack_delay) already separates ACK delay from RTT.
retx RTT-sample safety  existing estimator path; retransmitted-packet samples must be excluded (Karn) — enforced by design below.
timer ownership         Sender owns Pto arm/reset; cs_wakeup queue (BootstrapPto re-arm). Reset on ACK progress exists.
packet lineage          per-seq inflight keys; largest-acked implied by acked_seqs.
existing retransmit      pending_lost → RECOVER_ARQ selective retransmit (§9) — ACK-driven (gap-triggered) ONLY.
sender state mutation    on_event / on_event_core dispatch; inflight/pending_lost/rtt/rate updated there.
VERDICT: ACK metadata STRUCTURALLY SUFFICIENT for a PTO tail probe. Gap is solely the INV-CS-4 Pto→no-retransmit wiring.
         (§11 "ACK metadata insufficient" DUR condition NOT triggered.)
```

## §3 Deadline-feasibility matrix (outcome-independent physical lower bounds; reference t=0 = first source offer)
```text
workload 4848 B · pacer 0.80C=800kbps · bottleneck 1Mbps · src_serialize=48.5ms
RTT   owd   no_loss_LB  1tail_recovery_LB(=serialize+1.5·RTT)  deadline  horizon  class                          bench_deadline(3×LB)
50    25    73.5ms      123.5ms                                199       699      FEASIBLE_NO_LOSS_AND_RECOVERY  370ms
250   125   173.5ms     423.5ms                                599       2499     FEASIBLE_NO_LOSS_AND_RECOVERY  1270ms
1000  500   548.5ms     1548.5ms                               2099      9249     FEASIBLE_NO_LOSS_AND_RECOVERY  4645ms
```
All cells FEASIBLE (deadline > 1-tail recovery LB) → deadlines are NOT grown. R2A's near-0 deadline-hit is a PERFORMANCE
fact (observed recovery > physical LB: pacing + FEC-after-source ordering + multi-loss d2/d4 + conservative 2×RTT prototype),
NOT infeasibility. No authoritative product SLO exists → product validation reports (a) feasible-configured-deadline hit
rate, (b) observation-horizon completion, (c) time-to-completion distribution, (d) feasibility-normalized BENCHMARK deadline
= 3× one-tail recovery LB (370/1270/4645 ms) FROZEN pre-outcome and explicitly labeled a benchmark endpoint, NOT a product SLO.

## §4 Versioned INV-CS-4 supersession — bounded tail-probe behavioral spec (typed constants; product-config-driven)
```text
DECISION ID : DR-ARQ-PRODUCT-TAIL-1 (supersedes INV-CS-4 "no Pto retransmit path"). Additive + gated: default OFF →
              flag-off wire/behavior byte-identical to pre-decision (no cold-start/rate/pacing regression). ON only via
              product transport config `tail_probe_enabled`.
ARM         : Pto/tail timer MAY emit a retransmission probe ONLY when (a) tail_probe_enabled, (b) inflight non-empty
              (outstanding ack-eliciting SOURCE data exists), (c) no ACK progress since the last probe/arm.
TARGET      : the OLDEST outstanding inflight source seq (tail); one probe = one bounded retransmit of that unit.
PTO formula : pto = clamp(srtt + max(4·rttvar, TIMER_GRANULARITY) + max_ack_delay, PTO_MIN, PTO_MAX) · 2^probe_index
              (RFC-9002-inspired; exponential backoff). PTO_MIN/PTO_MAX are typed constants (bounds). If no RTT sample:
              pto = PTO_INITIAL (fixed conservative), NOT derived from a retransmitted sample (Karn).
RESET/CANCEL: any ACK progress (new acked_seq) resets the timer and probe_index to 0. inflight empty → DISARM (no fire).
BOUND       : probe_index ≤ MAX_TAIL_PROBES (hard cap) per outstanding unit; never unbounded. exponential backoff between.
REORDER/DACK: a probe is suppressed while within one PTO of the most recent ACK progress (reordering/delayed-ACK guard).
PROBE LOSS  : if the probe itself is lost (no progress after its PTO), backoff+re-probe up to MAX_TAIL_PROBES, then give up
              (bounded failure) — no infinite retransmit.
KARN        : RTT samples from retransmitted (probed) packets are NOT fed to the estimator (ambiguity).
FEC-INDEP   : the mechanism reads NO FEC controller state and is not gated on FEC activation.
NO-LOSS INV : on a no-loss flow every unit is acked before its PTO → timer never fires → 0 probes, 0 added bytes, wire
              output byte-identical to tail_probe OFF. (Test-enforced.)
OVERFLOW    : all timer arithmetic saturating/checked; deterministic (no RNG; golden-vector stable).
METRICS     : probe_count, probe_bytes, spurious_probe_count/bytes (probe of a unit that was in fact deliverable),
              recovery_cause, timer_fire_count — observation-only.
FAIL-CLOSED : config/parse error or bound overflow → tail probe stays OFF (never unbounded emission).
GATES       : this decision is validated by the SAME typed constants in product code + unit tests + faithful two-netns.
```

## Status / remaining (honest)
```text
DONE (this checkpoint): §2 readiness audit (ACK metadata sufficient) · §7 environment gate (faithful rootless netns+veth+
  netem VERIFIED working here) · §3 deadline-feasibility matrix (all FEASIBLE; benchmark-deadline formula frozen) · §4
  versioned INV-CS-4 supersession TDD (bounded tail-probe behavioral spec, default-OFF additive).
REMAINING (high-stakes production engineering — de-risked, design frozen): §5 product implementation in sender.rs (gated,
  additive, per §4) · §6 unit/deterministic tests + full CI gates (fmt/clippy -D/miri/deny/audit/golden pins/§4.4 benchmark)
  · §7/§8 faithful two-netns/netem harness + typed 36-cell core grid + adversarial slices (N=30 core / ≥10 adversarial;
  no adaptive stopping) · §9 metrics · §10 product baseline GO/NO-GO. §5 changes PRODUCT transport core under KAT-1..5
  frozen gates — must be executed gate-green, not rushed.
```
