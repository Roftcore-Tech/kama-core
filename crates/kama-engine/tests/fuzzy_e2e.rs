//! Commit-J §4 — GERÇEK FusionInputs → decide → EMISSION uçtan-uca (ENJEKSİYON-YOK).
//! Yasaklara-uyum: snapshot-alanı-yazılmaz; ratio-hook'u-yok; latch fuzzy-katmanını-atlamaz
//! (snapshot = gerçek `controller_decision_snapshot()`); gerçek-on_event-yolu; validated-fixture-config.

mod common;

use adaptive_transport::fusion::FuzzyConfig;
use adaptive_transport::sender::{
    AckInfo, Event, FecActuationMode, FecSuppressionReason, OpaqueUnit, Sender, Timer, TransmitKind,
};
use kama_engine::admit_and_decode;
use kama_fec_codec::{by_id, FecCodec, RsSimdCodec, SourceSymbol};
use kama_noise_wire::{OpenedTransportShard, SealArgs};
use kama_transport_schema::inner_kind::ShardKind;
use kama_transport_schema::prefix::SourcePrefix;
use kama_transport_schema::{LogicalShardHeader, UnsealedTransportPacket};
use std::time::Duration;

const MS: u64 = 1_000_000;
const PID: u8 = 0x03;

fn ack(seqs: Vec<u64>) -> Event {
    Event::AckReceived(AckInfo {
        acked_seqs: seqs,
        ack_delay: Duration::ZERO,
    })
}

/// Rejim-sürücüsü: her-round `emit_n`-paket `gap_ms`-arayla (in-round-burst); yalnız-son-`ack_n`
/// batch-ACK (öncekiler packet-threshold'la kayıp-ilan); round'lar `round_gap_ms`-aralıklı.
/// ZAMAN-ÖLÇEĞİ-KRİTİK: ledger confirm_age≈3.15s ve W_loss≈16.8s (EAI return-önseli 65B/31Bps
/// ≈2.1s tabanlı) — Suspected→Confirmed-in-window için pump-süresi ≥ W_loss olmalı; kısa-pump
/// yapısal-olarak ppm=0 okur (denominatör=pencere-içi-TÜM-gönderim, pay=yalnız-yaşlanmış-Confirmed).
fn drive_regime(
    s: &mut Sender,
    rounds: u64,
    emit_n: u64,
    ack_last_n: usize,
    gap_ms: u64,
    round_gap_ms: u64,
    t0: u64,
) -> u64 {
    let mut t = t0;
    let mut next_id = 10_000 + t0; // t0-ayrık id-uzayı (fazlar-çakışmasın)
    let mut tot_emit = 0u64;
    let mut tot_ack = 0u64;
    for _ in 0..rounds {
        let mut seqs: Vec<u64> = Vec::new();
        for _ in 0..emit_n {
            let out = s.on_event(
                Event::AppData(OpaqueUnit {
                    id: next_id,
                    size: 1200,
                }),
                t * MS,
            );
            next_id += 1;
            seqs.extend(out.iter().map(|d| d.seq));
            t += gap_ms;
        }
        tot_emit += seqs.len() as u64;
        let keep: Vec<u64> = seqs.iter().rev().take(ack_last_n).copied().collect();
        if !keep.is_empty() {
            tot_ack += keep.len() as u64;
            s.on_event(ack(keep), t * MS + MS / 2);
        }
        t += round_gap_ms;
    }
    let (dl_pkt, dl_time, dl_pend) = s.diag_loss();
    let (unloss, ppm, spur) = s.diag_ledger();
    eprintln!(
        "[drive] emit={tot_emit} ack={tot_ack} decl_pkt={dl_pkt} decl_time={dl_time} pend={dl_pend} unloss={unloss} ppm={ppm} spur={spur}"
    );
    t
}

/// Gerçek-latch + gerçek-emisyon + seal + decode (blok-DATA'sı harness-canonical; k_actual-parametreli).
/// Döner (decode, snapshot-ratio, planned, emitted).
fn real_latch_emit_decode(
    s: &mut Sender,
    h_ext: u32,
    block_id: u32,
    k_units: usize,
    t_ms: u64,
) -> (Result<Vec<u8>, kama_engine::EngineError>, u32, u16, u64) {
    let (mut tx, mut rx) = common::pair(h_ext);
    let p = by_id(PID).unwrap();
    let e = p.e_usize();
    let k_shape = p.k_shape as usize;
    let app = vec![0x33u8; common::PAYLOAD_CAP * k_units];
    let chunks: Vec<&[u8]> = app.chunks(common::PAYLOAD_CAP).collect();
    let k_actual = chunks.len() as u8;
    let mut block: Vec<Vec<u8>> = Vec::new();
    for (i, c) in chunks.iter().enumerate() {
        let pfx = SourcePrefix {
            message_id: 0x5000 + block_id,
            chunk_offset: (i * common::PAYLOAD_CAP) as u32,
            actual_payload_len: c.len() as u16,
            source_flags: u8::from(i == chunks.len() - 1),
        };
        let mut sym = vec![0u8; e];
        pfx.serialize_symbol(c, &mut sym).unwrap();
        block.push(sym);
    }
    while block.len() < k_shape {
        block.push(vec![0u8; e]);
    }
    let srcs: Vec<SourceSymbol> = block
        .iter()
        .enumerate()
        .map(|(i, b)| SourceSymbol {
            esi: i as u16,
            bytes: b,
        })
        .collect();
    let repairs = RsSimdCodec.encode(&p, &srcs).unwrap();

    // GERÇEK-zincir: witness → GERÇEK-snapshot (yazılmaz!) → latch (otorite-tüketimi + staging-dahili
    // — Commit-K) → gerçek-emisyon.
    let authority = s.open_fec_block_authority();
    let snap = s.controller_decision_snapshot().expect("gerçek-decision");
    let latch = s.fec_controller_latch(authority, Some(snap), k_actual, p.r_code_max, block_id, 64);
    // Emisyon-drenajı: her-tick emitted-DATA'yı batch-ACK'le (retransmit-backlog'u GERÇEKTEN
    // boşalt — ACK'siz-drenaj PTO'yla re-loss/re-queue döngüsüne girer, burst-cap≤2×MTU'yla
    // parity'ye hiç-sıra-gelmez); parity-esi dedup (re-emisyon çift-sayılmasın).
    let mut emitted: Vec<u16> = Vec::new();
    // Emisyonlar HER on_event dönüşünden toplanır — Pto-tick'ten DE ACK'ten DE. Eskiden yalnız
    // Pto-tick'in çıktısı sayılıyordu ve ACK çağrısının dönüşü ATILIYORDU; parity o çağrıda
    // emit edildiğinde gerçek emisyon görünmez oluyordu (ölçüm kör noktası, ürün kusuru DEĞİL —
    // bkz. reports/dr-rate-ramp-2b/FEC-PARITY-OBSERVATION-CORRIGENDUM.md).
    let collect = |out: &[adaptive_transport::sender::TransmitDecision],
                       emitted: &mut Vec<u16>,
                       data_seqs: &mut Vec<u64>| {
        for d in out {
            if d.kind == TransmitKind::FecParity {
                if let Some(h) = d.send_record {
                    let adaptive_transport::sender::SrLogicalRef::Fec { block_id: b, esi } =
                        h.logical_ref;
                    if b == block_id && !emitted.contains(&esi) {
                        emitted.push(esi);
                    }
                }
            } else {
                data_seqs.push(d.seq);
            }
        }
    };
    for t in 0..900u64 {
        let now = (t_ms + 10 + t * 10) * MS;
        let out = s.on_event(Event::TimerFired(Timer::Pto), now);
        let mut data_seqs: Vec<u64> = Vec::new();
        collect(&out, &mut emitted, &mut data_seqs);
        if !data_seqs.is_empty() {
            let ack_out = s.on_event(ack(data_seqs), now + 5 * MS);
            let mut ignored: Vec<u64> = Vec::new();
            collect(&ack_out, &mut emitted, &mut ignored);
        }
        if emitted.len() as u16 >= latch.planned_r {
            break;
        }
    }
    // Seal: 1-source-düşür (recovery-fırsatı; planned=0-ise-kayıpsız-source-only).
    let drop_esi: &[u16] = if latch.planned_r > 0 { &[1] } else { &[] };
    let mut pn = 0u64;
    let mut dgs: Vec<Vec<u8>> = Vec::new();
    for (i, sym) in block.iter().enumerate().take(usize::from(k_actual)) {
        if !drop_esi.contains(&(i as u16)) {
            let pkt = UnsealedTransportPacket::FecShard {
                header: LogicalShardHeader {
                    kind: ShardKind::Source,
                    profile_id: PID,
                    block_id,
                    esi: i as u16,
                    k_actual,
                },
                symbol: sym.clone(),
            };
            dgs.push(
                tx.seal(
                    &pkt,
                    SealArgs {
                        wire_pn: pn,
                        dh_epoch: 0,
                        send_generation: 0,
                    },
                )
                .unwrap()
                .bytes,
            );
        }
        pn += 1;
    }
    for esi in &emitted {
        let rep = repairs.iter().find(|r| r.esi == *esi).unwrap();
        let pkt = UnsealedTransportPacket::FecShard {
            header: LogicalShardHeader {
                kind: ShardKind::Repair,
                profile_id: PID,
                block_id,
                esi: rep.esi,
                k_actual,
            },
            symbol: rep.bytes.clone(),
        };
        dgs.push(
            tx.seal(
                &pkt,
                SealArgs {
                    wire_pn: pn,
                    dh_epoch: 0,
                    send_generation: 0,
                },
            )
            .unwrap()
            .bytes,
        );
        pn += 1;
    }
    let opened: Vec<OpenedTransportShard> = dgs.iter().map(|d| rx.open(d).expect("auth")).collect();
    (
        admit_and_decode(opened, PID, k_actual),
        snap.ratio_permille,
        latch.planned_r,
        emitted.len() as u64,
    )
}

fn controller_sender() -> Sender {
    let mut s = Sender::try_with_fuzzy_config(FuzzyConfig::fixture()).unwrap(); // validated-yol
    s.set_fec_actuation_mode(FecActuationMode::Controller)
        .unwrap();
    s
}

/// 4.1/4.5 R2-rejimi (loss-HIGH ∧ bw-HIGH ∧ jitter-LOW): warm + (pump ↔ clean-burst)×2.
/// Clean-burst'ler btlbw-MAX-filtresini (W_BTLBW≈16.8s) tazeler — ardışık-round batch-ACK'ler
/// Δt≈9ms'de 9600B teslim ≈ 1MB/s örneği verir; pump'ın 150ms-round-gap örnekleri (~16kB/s)
/// max'ı taşıyamaz. Pump loss-HIGH'ı taşır (8-emit/2-ACK ≈ 750k frac × pencere-faktörü ≈ 610k).
fn drive_r2(s: &mut Sender) -> u64 {
    let mut t = drive_regime(s, 20, 4, 4, 1, 1, 0); // warm (srtt+btlbw-kur)
    for _ in 0..2 {
        t = drive_regime(s, 55, 8, 2, 1, 150, t); // loss-pompalama (~8.3s)
        t = drive_regime(s, 3, 8, 8, 1, 1, t); // clean-burst (btlbw-tazele; ~30ms)
    }
    t
}

// ── 4.1 NON-ZERO (R2: loss-HIGH ∧ bw-HIGH ∧ jitter-LOW) ──
#[test]
fn e2e_real_fuzzy_nonzero_r2_to_emission() {
    let mut s = controller_sender();
    let t = drive_r2(&mut s);
    let snap = s.diag_fusion_input_snapshot().unwrap();
    eprintln!(
        "[E2E-4.1] inputs: loss={} bw={} jitter={} qd={} ackconf={} no_rule={}",
        snap.loss_windowed_ppm,
        snap.bandwidth_bps,
        snap.jitter_ppm,
        snap.qd_ratio_permille,
        snap.ack_confidence_permille,
        snap.no_rule
    );
    // Üyelik-sınıfı-assert'leri (R2-koşulu; fixture-eşikleri): loss-HIGH>0 ∧ bw-HIGH-bölgesi ∧ jitter-LOW.
    let cfg = FuzzyConfig::fixture();
    let dl = cfg.loss.fuzzify(snap.loss_windowed_ppm);
    let db = cfg
        .bandwidth
        .fuzzify(adaptive_transport::fusion::log2_fp(snap.bandwidth_bps));
    let dj = cfg.jitter.fuzzify(snap.jitter_ppm);
    eprintln!(
        "[E2E-4.1] üyelik: loss(H)={} bw(H)={} jit(L)={} → R2-ağırlık={}",
        dl.high,
        db.high,
        dj.low,
        dl.high.min(db.high).min(dj.low)
    );
    assert!(
        dl.high > 0 && db.high > 0 && dj.low > 0,
        "R2-öncülleri gerçek-inputs'ta"
    );
    // GERÇEK-zincir (enjeksiyonsuz):
    let (out, ratio, planned, emitted) = real_latch_emit_decode(&mut s, 0x0A00_0001, 1, 8, t);
    eprintln!("[E2E-4.1] ratio={ratio} planned={planned} emitted={emitted}");
    assert!(
        ratio > 0,
        "fuzzy-motor non-zero-ratio ÜRETTİ (enjeksiyon-yok)"
    );
    assert!(
        planned > 0 && emitted == u64::from(planned),
        "gerçek-emisyona-ULAŞTI"
    );
    assert_eq!(
        out.unwrap(),
        vec![0x33u8; common::PAYLOAD_CAP * 8],
        "1-kayıp-recovered"
    );
}

// ── 4.2 ZERO-FEC (R1: loss-LOW) ──
#[test]
fn e2e_real_fuzzy_zero_r1_source_only() {
    let mut s = controller_sender();
    let t = drive_regime(&mut s, 30, 4, 4, 1, 1, 0); // temiz: hepsi-ACK → loss≈0
    let snap = s.diag_fusion_input_snapshot().unwrap();
    assert!(snap.loss_windowed_ppm < 200_000, "loss-LOW-bölgesi");
    let (out, ratio, planned, emitted) = real_latch_emit_decode(&mut s, 0x0A00_0002, 1, 8, t);
    eprintln!("[E2E-4.2] ratio={ratio} planned={planned} emitted={emitted}");
    assert_eq!(
        (ratio, planned, emitted),
        (0, 0, 0),
        "R1: literal/configured-zero → emisyon-yok"
    );
    assert_eq!(
        out.unwrap(),
        vec![0x33u8; common::PAYLOAD_CAP * 8],
        "source-only-tamamlama"
    );
}

// ── 4.3 NO-RULE (loss-HIGH ∧ bw-düşük → 4-kural-sıfır) ──
#[test]
fn e2e_real_fuzzy_no_rule_literal_zero() {
    let mut s = controller_sender();
    // Düşük-hacim (10ms-ara, tek-ACK→app-limited→btlbw-düşük) + ağır-kayıp ≥W_loss-süresi →
    // ppm ≈ 833k×0.81 ≈ 670k ≥ 600k (loss.med=0, HIGH-plateau) ∧ bw.high=0 → 4-kural-sıfır.
    let t = drive_regime(&mut s, 10, 2, 2, 10, 10, 0); // hafif-warm (srtt-kur)
    let t = drive_regime(&mut s, 60, 6, 1, 10, 230, t); // %83-kayıp; teslim-seyrek (~17.4s)
    let snap = s.diag_fusion_input_snapshot().unwrap();
    eprintln!(
        "[E2E-4.3] inputs: loss={} bw={} no_rule={}",
        snap.loss_windowed_ppm, snap.bandwidth_bps, snap.no_rule
    );
    assert!(
        snap.no_rule,
        "GERÇEK-inputs no_rule-sınıfında (zero-weight)"
    );
    let (out, _ratio, planned, emitted) = real_latch_emit_decode(&mut s, 0x0A00_0003, 1, 8, t);
    assert_eq!(
        (planned, emitted),
        (0, 0),
        "no_rule → latch-LITERAL-0 (consequent-bağımsız)"
    );
    let d = s.diag_fec_actuation();
    assert_eq!(
        d.last_latch.unwrap().suppression,
        Some(FecSuppressionReason::NoRule)
    );
    assert!(out.is_ok(), "kayıpsız-source-only");
}

// ── 4.5 PARTIAL (gerçek-fuzzy + k_actual=5) ──
#[test]
fn e2e_real_fuzzy_partial_block() {
    let mut s = controller_sender();
    let t = drive_r2(&mut s); // 4.1-rejimi (loss-HIGH∧bw-HIGH∧jit-LOW)
    let (out, ratio, planned, emitted) = real_latch_emit_decode(&mut s, 0x0A00_0005, 1, 5, t);
    eprintln!("[E2E-4.5] partial: ratio={ratio} planned={planned} emitted={emitted}");
    assert!(ratio > 0);
    // k_actual=5-formülü (nominal-K-değil): planned=(ratio×5+500)/1000-clamp.
    let expect = ((u64::from(ratio) * 5 + 500) / 1000).min(4) as u16;
    assert_eq!(planned, expect, "ratio×k_actual (K-değil)");
    assert_eq!(emitted, u64::from(planned));
    assert_eq!(out.unwrap(), vec![0x33u8; common::PAYLOAD_CAP * 5]);
}

// ── 4.4 EMERGENCY gerçek-yol-DENEMESİ (loss-HIGH ∧ bw-LOW ∧ ackconf-LOW) ──
#[test]
fn e2e_real_fuzzy_emergency_attempt() {
    let mut s = controller_sender();
    let t = drive_regime(&mut s, 10, 2, 2, 30, 30, 0); // yavaş-warm (2400B/90ms≈27kB/s örnek)
    let t = drive_regime(&mut s, 60, 6, 1, 30, 110, t); // loss-HIGH; yavaş-kadans → btlbw<75k (bw.low>500)
                                                        // TEK-TİCK-SESSİZLİK (11s): ackconf = threshold/silence < 400 (threshold=max(3srtt, K×EAI)≈4.2s);
                                                        // tek-event → burst-cap(≤2×MTU) emisyonu-sınırlar → loss-denominatörü-dilüe-olmaz; pencere
                                                        // (16.8s) pump-kuyruğu declares'ını hâlâ-kapsar (Suspected→Confirmed tick'te yaşlanmış).
    let t2 = t + 11_000;
    s.on_event(Event::TimerFired(Timer::Pto), t2 * MS);
    let snap = s.diag_fusion_input_snapshot().unwrap();
    let dec_mode = s.last_decision().mode;
    eprintln!(
        "[E2E-4.4] inputs: loss={} bw={} ackconf={} state={:?} → mode={:?}",
        snap.loss_windowed_ppm,
        snap.bandwidth_bps,
        snap.ack_confidence_permille,
        s.state(),
        dec_mode
    );
    if dec_mode == adaptive_transport::fusion::Mode::EmergencyForwardRedundancy {
        // GERÇEK-YOL-emergency ulaşıldı: latch-interlock planned=0. GERÇEK-BULGU (fixture-geometrisi):
        // emergency-bölgesi (bw.low>500 ⇔ bw<75kB/s) R2'nin bw.high-öncülüyle AYRIK ve loss-HIGH-
        // plateau'da (≥600k) loss.med=0 → R3-de-sıfır → gerçek-sürülen-emergency-noktasında çoğunlukla
        // no_rule∧emergency BİRLİKTE. Guard-önceliği (Commit-G-mühürlü-sıra) no_rule'u-önce-raporlar;
        // her-iki-dal-da planned=0-interlock'tur (EmergencyPendingArbitration-dalı enjeksiyonlu-unit
        // t27/dr_1a_latch_guards'da kanıtlı).
        let authority = s.open_fec_block_authority();
        let real = s.controller_decision_snapshot().unwrap();
        assert!(real.emergency, "gerçek-snapshot emergency-işaretli");
        let latch = s.fec_controller_latch(authority, Some(real), 8, 4, 0x44, 64);
        assert_eq!(latch.planned_r, 0, "interlock: emisyon-yok");
        let expect = if real.no_rule {
            FecSuppressionReason::NoRule
        } else {
            FecSuppressionReason::EmergencyPendingArbitration
        };
        assert_eq!(latch.suppression, Some(expect), "guard-önceliği-sırası");
        eprintln!(
            "[E2E-4.4] GERÇEK-YOL-interlock: no_rule={} suppression={:?}",
            real.no_rule, latch.suppression
        );
    } else {
        // Deterministik-gerçek-yol-tetiği bu-sürüşte-oluşmadı → §4.4-izinli-sınırlama: neden =
        // ackconf-sessizlik-eşiği srtt/EAI-çarpanlı (sürüş-parametrelerine-duyarlı); emergency-
        // interlock-davranışı unit+engine-testlerde kanıtlı (dr_1a_latch_guards / t27 / engine_partial).
        eprintln!("[E2E-4.4] emergency-tetiklenmedi (izinli-sınırlama; unit-testler-kapsıyor)");
    }
}
