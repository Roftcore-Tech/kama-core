//! [ADAY · eski motor canlılığı] Teslim kanalı dolu alıcı (veriyi motora vermez, ACK'lemez)
//! birkaç PTO turundan uzun durduğunda eski motor oturumu durdurmamalı — deterministik benzetim
//! (gerçek saat, soket, tokio YOK).
//!
//! Ne benzetiliyor: `ack_after_admit.rs` / `legacy_stalled_consumer_loses_no_acked_unit`
//! düzeneği. Gönderici `ClientSession::new()` (eski motor) + çalışma zamanının ürün profilinde
//! göndericiye uyguladığı ayarlar; alıcı eski `Receiver` (`quota_r` 4, sunucu yuvasıyla aynı
//! kurulum). Çalışma-zamanı yapıştırıcısının kuralları birebir: (1) teslim kanalı (4 yer)
//! doluyken datagram alıcı motoruna VERİLMEZ, ACK üretilmez (`runtime.rs` sunucu görevi,
//! `data_not_admitted_full`); (2) zamanlayıcılar çalışma zamanının aralıklarında: gönderici PTO
//! 1 ms, RtpropProbe 200 ms, alıcı ACK zamanlayıcısı 20 ms. Bağlantı kayıpsız, tek yön gecikme
//! bir tik (RTT ≈ 1 ms, geri döngü gibi). Tüketici 1,5 s durur, sonra tik başına BİR birim alır
//! (bir patlamanın kanala ancak kanal yeri kadar girdiği hâl; CI'da yük altında görülen).
//!
//! İki kusur, iki senaryo (ikisi de CI dökümlerinde görüldü):
//!
//! (A) Kuyruk probu turla tavanlıydı (`validated_v1`, 3). Reddedilen birimlerin hiçbirinin
//! PreConverge kopyası yokken (ilk ACK birimlerden önce işlendi; döküm: `tx_dup_bytes: 0`) üç prob
//! duraklamanın ilk ≈ 200 ms'sinde reddedilir; kuyruk birimleri `largest_acked`'in üstünde
//! olduğundan paket/zaman eşiği kayıp ilan edemez; alıcının eski durumu yineleyen ACK'leri
//! `ack_confidence`'ı tam tutar (RecoverFwd yok) → 60 s'de 60 birim eksik. Aday:
//! `TailRecoveryConfig::validated_v1_uncapped` (prob üstel geri çekilmeyle sürer).
//!
//! (B) Bütün birimler PreConverge'de gidince her birinin §3.3-B kopyası aynı anlarda kurulur
//! (31 B/s tabanından ≈ 5 / 15 / 35 / 75 s). Duraklama sonrası ARQ patlamaları kanala ancak yer
//! kadar girer; tavana (3) ulaşan birimin HandToForward kopyası da reddedilince eski kural yeni
//! kopya kurmaz ("planlı kopya var" — ama o kopya onlarca saniye sonra), kopya kaybı ARQ'ya
//! dönmez, uçuşu boş göndericide kuyruk probu söner → birim yalnız merdivene kalır (döküm:
//! birimin uçuş/ARQ kaydı yok, kopya vadesi 35,6 s / 76,2 s; 60 s'de 5–17 birim eksik). Aday:
//! `Sender::queue_losses` kopya kaybında, birimin bir taban PTO içinde başka denemesi yoksa onu
//! ARQ kuyruğuna döndürür.

use std::collections::{BTreeSet, VecDeque};
use std::time::Duration;

use adaptive_transport::receiver::{Receiver, RxDatagram, RxKind};
use adaptive_transport::sender::{
    AckInfo, Event, FecActuationMode, InitialWindowConfig, OpaqueUnit, SenderState, Timer,
    TransmitKind,
};
use client_core::runtime::{RuntimeConfig, TransportEngine};
use client_core::{ClientSession, TransmitDecision};

const MS: u64 = 1_000_000;
const UNITS: u64 = 64;
const QUEUE: usize = 4;
/// Birimin tel boyu (≈ "admit-unit-N" + DATA başlığı + mühür).
const UNIT_WIRE: u64 = 40;
const STALL_END_NS: u64 = 1_500 * MS;
const HORIZON_NS: u64 = 60_000 * MS;
const ACK_TIMER_NS: u64 = 20 * MS;
const RTPROP_TICK_NS: u64 = 200 * MS;

/// Benzetimin sonucu ve duraklama sonundaki ön koşul görüntüsü.
#[derive(Debug)]
struct Outcome {
    /// Uygulamaya teslim edilen (ilk-teslim) birimler.
    delivered: BTreeSet<u64>,
    /// Göndericinin teyit ettiği birimler (`take_confirmed_units`).
    confirmed: BTreeSet<u64>,
    /// Son birimin hem teslim hem teyit edildiği an (ns); bitmediyse `None`.
    done_ns: Option<u64>,
    /// Duraklama sonunda: tele çıkan kuyruk probu ve kopya (Duplicate), reddedilen datagram,
    /// kopya kuyruğundaki kayıt ve ilk reddedilen birimin (4) kopya vadesi (ns).
    tail_probes_at_stall_end: u64,
    dups_at_stall_end: u64,
    refused_at_stall_end: u64,
    dup_queue_at_stall_end: u32,
    first_refused_dup_due_ns: Option<u64>,
    /// Benzetim boyunca gönderici RecoverFwd'e girdi mi.
    entered_recover_fwd: bool,
}

/// Çalışma zamanının (`runtime.rs` istemci görevi) ürün profilinde göndericiye uyguladığı,
/// varsayılandan farklı ayarlar: ilk uçuş penceresi, spekülatif FEC kolu, yapı-gate seviyesi,
/// FEC Controller kipi (port motorunda ilk pencere tabanı 4·MTU).
fn product_session(engine: TransportEngine) -> ClientSession {
    let cfg = RuntimeConfig {
        engine,
        ..RuntimeConfig::default()
    };
    let mut s = match engine {
        TransportEngine::Legacy => ClientSession::new(),
        TransportEngine::CcPort => ClientSession::with_cc_port(),
    };
    let tx = s.sender_mut();
    tx.set_fec_structure_policy(cfg.fec_structure_policy);
    tx.set_loss_confirm_horizon(cfg.loss_confirm_horizon);
    tx.set_fec_loss_risk(cfg.fec_loss_risk);
    tx.set_structure_gate_level(cfg.structure_gate_level_permille);
    if engine == TransportEngine::Legacy {
        tx.set_e2a_flags(cfg.e2a_flags);
    }
    tx.set_initial_window(InitialWindowConfig {
        bytes: cfg.initial_window_bytes,
    });
    if engine == TransportEngine::CcPort {
        tx.set_cc_initial_window(
            cfg.initial_window_bytes
                .max(4 * adaptive_transport::rate::DEFAULT_MTU),
        );
    }
    tx.set_e1_flags(cfg.e1_flags);
    tx.set_fec_actuation_mode(FecActuationMode::Controller)
        .expect("taze Sender kip ayarını kabul eder");
    s
}

/// `first` birim t = 0'da sunulur (PreConverge'de gider); kalanı ilk ACK işlendikten sonra.
fn simulate(engine: TransportEngine, first: u64) -> Outcome {
    let mut s = product_session(engine);
    let mut rx = match engine {
        TransportEngine::Legacy => Receiver::new(),
        TransportEngine::CcPort => Receiver::with_cc_port(),
    };
    rx.set_quota_r(4);
    // Gönderici → alıcı: (seq, birim); alıcı → gönderici: (onaylanan seq'ler, ack_delay).
    let mut to_rx: VecDeque<(u64, u64)> = VecDeque::new();
    let mut to_tx: VecDeque<(Vec<u64>, Duration)> = VecDeque::new();
    let mut channel: VecDeque<u64> = VecDeque::new();
    let mut seen: BTreeSet<u64> = BTreeSet::new();
    let mut delivered: BTreeSet<u64> = BTreeSet::new();
    let mut confirmed: BTreeSet<u64> = BTreeSet::new();
    let mut next_seq: u64 = 0;
    let mut refused: u64 = 0;
    let mut rest_offered = first >= UNITS;
    let mut entered_recover_fwd = false;
    let mut at_stall_end = None;
    let mut done_ns = None;

    let emit = |out: Vec<TransmitDecision>, to_rx: &mut VecDeque<(u64, u64)>, next: &mut u64| {
        for d in out {
            if matches!(
                d.kind,
                TransmitKind::Data | TransmitKind::Retransmit | TransmitKind::Duplicate
            ) {
                to_rx.push_back((d.seq, d.unit_id));
                *next = (*next).max(d.seq + 1);
            }
        }
    };
    let offer = |s: &mut ClientSession, id: u64, t: u64| {
        s.on_event(
            Event::AppData(OpaqueUnit {
                id,
                size: UNIT_WIRE,
            }),
            t,
        )
    };

    for id in 0..first.min(UNITS) {
        let out = offer(&mut s, id, 0);
        emit(out, &mut to_rx, &mut next_seq);
    }
    let mut t = 0u64;
    while t <= HORIZON_NS {
        // Alıcı: teslim kanalında yer yoksa datagram motora verilmez (ACK yok).
        while let Some((seq, unit)) = to_rx.pop_front() {
            if channel.len() >= QUEUE {
                refused += 1;
                continue;
            }
            let out = rx.on_datagram(
                RxDatagram {
                    seq,
                    unit_id: unit,
                    kind: RxKind::Data,
                },
                t,
            );
            if out.delivered.contains(&unit) && seen.insert(unit) {
                channel.push_back(unit);
            }
            if let Some(a) = out.ack {
                to_tx.push_back((a.acked_full(next_seq), a.ack_delay_duration()));
            }
        }
        if t % ACK_TIMER_NS == 0 {
            if let Some(a) = rx.on_ack_timer(t) {
                to_tx.push_back((a.acked_full(next_seq), a.ack_delay_duration()));
            }
        }
        // Tüketici duraklama bitince tik başına bir birim alır.
        if t >= STALL_END_NS {
            if let Some(u) = channel.pop_front() {
                delivered.insert(u);
            }
        }
        if t == STALL_END_NS {
            let a = s.sender().diag_activation();
            let q = s.sender().diag_recovery_timers();
            let first_refused = s.sender().diag_unit_recovery(QUEUE as u64);
            at_stall_end = Some((
                a.tail_probes,
                a.dups,
                refused,
                q.dup_queue_len,
                first_refused.dup_due_ns,
            ));
        }
        t += MS;
        // Gönderici: ACK'ler bir tik sonra varır.
        for (acked_seqs, ack_delay) in std::mem::take(&mut to_tx) {
            let out = s.on_event(
                Event::AckReceived(AckInfo {
                    acked_seqs,
                    ack_delay,
                }),
                t,
            );
            emit(out, &mut to_rx, &mut next_seq);
            confirmed.extend(s.sender_mut().take_confirmed_units());
            if !rest_offered {
                // Kalan birimler ilk ACK işlendikten sonra (PreConverge'den çıkılmış).
                rest_offered = true;
                assert_ne!(s.sender().state(), SenderState::PreConverge);
                for id in first..UNITS {
                    let out = offer(&mut s, id, t);
                    emit(out, &mut to_rx, &mut next_seq);
                }
            }
        }
        let out = s.on_event(Event::TimerFired(Timer::Pto), t);
        emit(out, &mut to_rx, &mut next_seq);
        if t % RTPROP_TICK_NS == 0 {
            let out = s.on_event(Event::TimerFired(Timer::RtpropProbe), t);
            emit(out, &mut to_rx, &mut next_seq);
        }
        confirmed.extend(s.sender_mut().take_confirmed_units());
        entered_recover_fwd |= s.sender().state() == SenderState::RecoverFwd;
        if delivered.len() as u64 == UNITS && confirmed.len() as u64 == UNITS {
            done_ns = Some(t);
            break;
        }
    }
    let (tail_probes, dups, refused_at, dup_queue, first_due) =
        at_stall_end.expect("benzetim duraklama sonuna ulaştı");
    Outcome {
        delivered,
        confirmed,
        done_ns,
        tail_probes_at_stall_end: tail_probes,
        dups_at_stall_end: dups,
        refused_at_stall_end: refused_at,
        dup_queue_at_stall_end: dup_queue,
        first_refused_dup_due_ns: first_due,
        entered_recover_fwd,
    }
}

/// Ortak iddia: oturum durmaz — her birim tam bir kez teslim edilir ve teyit edilir; duraklama
/// bittikten sonra en çok bir duraklama süresi içinde biter (3 s = 2 × duraklama).
fn assert_session_survives(o: &Outcome) {
    let all: BTreeSet<u64> = (0..UNITS).collect();
    assert_eq!(
        o.delivered,
        all,
        "60 s içinde hepsi teslim edilmeli — eksik {} birim: {o:?}",
        UNITS - o.delivered.len() as u64
    );
    assert_eq!(o.confirmed, all, "her birim teyit edilmeli: {o:?}");
    let done = o.done_ns.expect("bitti");
    assert!(
        done <= 2 * STALL_END_NS,
        "duraklama bitince oturum bir duraklama süresi içinde sürmeli: {} ms; {o:?}",
        done / MS
    );
}

/// (A) Eski motor, reddedilen birimlerin hiçbirinde PreConverge kopyası yok. Tavanlı kuyruk
/// probuyla 60 s sonunda 4 teslim / 4 teyit kalıyordu.
#[test]
fn legacy_full_receiver_without_cold_start_copies_does_not_stall() {
    let o = simulate(TransportEngine::Legacy, 4);
    // Ön koşul — düzenek kusurun şeklini kuruyor: duraklama içinde en az üç prob tele çıktı ve
    // reddedildi, hiçbir §3.3-B kopyası yok (kurtarma rastlantıya kalmıyor), RecoverFwd yok.
    assert!(
        o.tail_probes_at_stall_end >= 3,
        "duraklama içinde en az üç kuyruk probu (eski tavan) beklenir: {o:?}"
    );
    assert_eq!(
        (o.dups_at_stall_end, o.dup_queue_at_stall_end),
        (0, 0),
        "PreConverge kopyası olmamalı: {o:?}"
    );
    assert!(
        o.refused_at_stall_end >= UNITS - 4 + 3,
        "60 birim + 3 prob reddedilmiş olmalı: {o:?}"
    );
    assert!(
        !o.entered_recover_fwd,
        "kurtarma RecoverFwd'den gelmemeli: {o:?}"
    );
    assert_session_survives(&o);
}

/// (B) Eski motor, bütün birimler PreConverge'de gitti: her reddedilen birimin §3.3-B kopyası
/// aynı uzak anlarda. Eski kuralla 60 s sonunda 46 teslim kalıyordu (kalanlar 35 s / 76 s
/// basamaklarını bekliyordu).
#[test]
fn legacy_full_receiver_with_synchronised_cold_start_copies_does_not_stall() {
    let o = simulate(TransportEngine::Legacy, UNITS);
    // Ön koşul: ilk uçuş penceresine (2400 B) sığan birimler PreConverge'de gitti; reddedilenlerin
    // kopyası kuyrukta ve vadesi duraklamanın çok ötesinde (31 B/s tabanı: ≈ 5 s); RecoverFwd yok.
    assert!(
        o.dup_queue_at_stall_end >= 50,
        "reddedilen birimlerin PreConverge kopyası kuyrukta olmalı: {o:?}"
    );
    assert!(
        o.first_refused_dup_due_ns
            .is_some_and(|d| d > 2 * STALL_END_NS),
        "PreConverge kopyasının vadesi duraklamanın çok ötesinde olmalı: {o:?}"
    );
    assert!(
        !o.entered_recover_fwd,
        "kurtarma RecoverFwd'den gelmemeli: {o:?}"
    );
    assert_session_survives(&o);
}

/// Port motoru (ürün varsayılanı) aynı benzetimde, iki senaryo: prob turu tavanı yok
/// (`rfc9002_cc_port`), ARQ tavanı 255 (HandToForward'a düşmez). Eşlik kanıtı.
#[test]
fn port_full_receiver_does_not_stall_in_either_scenario() {
    for first in [4, UNITS] {
        let o = simulate(TransportEngine::CcPort, first);
        assert_session_survives(&o);
    }
}
