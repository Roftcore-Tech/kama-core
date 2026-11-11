//! §5.1 `ack_confidence` — boşta geçen süre ACK SESSİZLİĞİ sayılmaz (port motoru).
//!
//! Spec `specs/adaptive-transport-engine-spec.md` §5.1 (NORMATİF):
//!
//! ```text
//! active DEĞİLSE → ack_confidence = 1000 (FULL)      # outstanding iş yok, sessizlik anlamsız
//! aksi: silence = now − max(last_ack_time, first_send_time)
//! ```
//!
//! "yeni gönderilmiş paket 'düşük güven' DEĞİLDİR — henüz ACK'lenme zamanı gelmemiştir".
//! `max` ancak `first_send_time` AKTİF DÖNEMİN ilk gönderimi olduğunda bir şey seçer: oturumun ilk
//! gönderimi her ACK'ten önce geldiğinden oturum-başına tek damga ile `max` hep `last_ack` olur ve
//! boşta geçen süre sessizliğe eklenir.
//!
//! Senaryo (iş emri §5 (iii)): ACK t = 1 s → 20 s boşta (iş yok) → yeni `AppData` t = 21 s.
//! * Port motoru (`Sender::with_cc_port`): o tikte ve sonraki tikte RECOVER_FWD yok, emergency
//!   `Arq`'ta kalır, güven 1000.
//! * Koruma (yanlış negatif yok): uçuşta veri varken ACK gelmezse güven yine düşer ve
//!   RECOVER_FWD'ye girilir; PTO probları da, dönem içinde gelen yeni `AppData` da sessizliği
//!   sıfırlamaz (aktif dönem içinde damga sabit).
//! * Çapanın `last_ack` yarısı: dönem içinde ACK'ler geldikçe sessizlik son ACK'ten sayılır.
//! * Eski motor (`Sender::new`): bugünkü davranış pinli — düzeltme `cc_on()` ile kapılı, eski
//!   motorda resume'un ilk mesajı RECOVER_FWD'ye girer (Dup2, sonraki tik Dup3).
//!
//! Koş: `cargo test -p adaptive-transport --test ack_confidence_idle`

use adaptive_transport::emergency::EmergencyLevel;
use adaptive_transport::sender::{
    AckInfo, Event, OpaqueUnit, Sender, SenderState, Timer, TransmitKind,
};
use std::time::Duration;

const MS: u64 = 1_000_000;
const SEC: u64 = 1_000_000_000;
/// Isınma gönderimi; ACK `T_ACK`'te gelir (srtt ≈ 50 ms).
const T_WARM_SEND: u64 = 950 * MS;
const T_ACK: u64 = SEC;
/// 20 s boşta sonrası yeni mesaj.
const T_RESUME: u64 = 21 * SEC;

fn unit(id: u64) -> OpaqueUnit {
    OpaqueUnit { id, size: 1200 }
}

fn ack(seqs: &[u64]) -> Event {
    Event::AckReceived(AckInfo {
        acked_seqs: seqs.to_vec(),
        ack_delay: Duration::ZERO,
    })
}

fn ack_conf(s: &Sender) -> u64 {
    s.diag_fusion_input_snapshot()
        .expect("her olay fuzzy değerlendirir")
        .ack_confidence_permille
}

/// Isınma: tek birim gönder, t = 1 s'de tamamını ACK'le → RTT ölçüldü, iş yok (Idle).
fn warm_then_idle(mut s: Sender) -> Sender {
    let out = s.on_event(Event::AppData(unit(0)), T_WARM_SEND);
    let seqs: Vec<u64> = out
        .iter()
        .filter(|d| d.kind != TransmitKind::FecParity)
        .map(|d| d.seq)
        .collect();
    assert!(!seqs.is_empty(), "ısınma birimi tele çıkmalı");
    s.on_event(ack(&seqs), T_ACK);
    assert_eq!(s.state(), SenderState::Idle, "ısınma sonrası iş kalmamalı");
    assert!(s.inflight_bytes() == 0, "ısınma sonrası uçuş boş");
    s
}

/// Port motoru: 20 s boşta sonrası ilk mesaj ve sonraki tik RECOVER_FWD'ye girmez.
#[test]
fn cc_port_resume_after_idle_is_not_recover_fwd() {
    let mut s = warm_then_idle(Sender::with_cc_port());
    assert!(s.cc_on());

    let out = s.on_event(Event::AppData(unit(1)), T_RESUME);
    assert!(
        out.iter().any(|d| d.kind == TransmitKind::Data),
        "resume birimi aynı tikte tele çıkmalı"
    );
    assert_ne!(
        s.state(),
        SenderState::RecoverFwd,
        "resume tikinde RECOVER_FWD yok"
    );
    assert_eq!(
        s.emergency_level(),
        EmergencyLevel::Arq,
        "resume tikinde emergency yok"
    );
    assert_eq!(ack_conf(&s), 1000, "resume tikinde güven FULL");

    // Sonraki tik: yeni birim uçuşta, ACK henüz gelmedi (taze gönderim LOW değil).
    s.on_event(Event::TimerFired(Timer::Pto), T_RESUME + MS);
    assert_ne!(
        s.state(),
        SenderState::RecoverFwd,
        "sonraki tikte RECOVER_FWD yok"
    );
    assert_eq!(
        s.emergency_level(),
        EmergencyLevel::Arq,
        "sonraki tikte emergency yok"
    );
    assert_eq!(ack_conf(&s), 1000, "sonraki tikte güven FULL");
}

/// Koruma: port motorunda resume sonrası ACK gelmezse (gerçek dönüş yolu sessizliği) güven
/// düşer ve RECOVER_FWD'ye girilir. Sessizlik aktif dönemin ilk gönderiminden sayılır: PTO probları
/// sessizliği sıfırlamaz (güven bu dönemde artmaz).
#[test]
fn cc_port_true_silence_after_resume_still_escalates() {
    let mut s = warm_then_idle(Sender::with_cc_port());
    s.on_event(Event::AppData(unit(1)), T_RESUME);
    s.on_event(Event::AppData(unit(2)), T_RESUME + MS);

    let mut saw_fwd = false;
    let mut first_escalate_ns: Option<u64> = None;
    let mut prev_conf = ack_conf(&s);
    for k in 1..=40u64 {
        let now = T_RESUME + k * SEC;
        s.on_event(Event::TimerFired(Timer::Pto), now);
        let conf = ack_conf(&s);
        assert!(
            conf <= prev_conf,
            "sessizlik boyunca güven artmamalı (t={now} ns: {prev_conf} → {conf})"
        );
        prev_conf = conf;
        if s.state() == SenderState::RecoverFwd {
            saw_fwd = true;
        }
        if s.emergency_level() > EmergencyLevel::Arq && first_escalate_ns.is_none() {
            first_escalate_ns = Some(now);
        }
    }
    assert!(saw_fwd, "ACK yok → RECOVER_FWD'e girmeli (§11)");
    let first = first_escalate_ns.expect("emergency bir noktada escalate etmeli");
    // Düzeltmesiz port resume tikinin kendisinde escalate eder (güven 210, Dup2). Düzeltmeyle güven
    // ancak sessizlik `max(3·srtt, 2·EAI)` eşiğini aşınca düşer ve escalation güven < 300 ister
    // (ölçülen: güven < 1000 ≈ +4.2 s, escalation ≈ +14 s). 3 s bu iki durumu ayıran gevşek sınırdır.
    assert!(
        first > T_RESUME + 3 * SEC,
        "escalation resume'dan en az 3 s sonra olmalı (ilk escalation t={first} ns)"
    );
}

/// Koruma (aktif dönem kapısı): uygulama sessizlik boyunca yeni `AppData` vermeye devam ederse
/// dönem KAPANMAZ ve sessizlik dönemin ilk gönderiminden sayılmaya devam eder. Aktif dönem içinde
/// gelen `AppData` damgayı yenilerse dönüş yolu ölürken uygulama gönderdikçe motor hiç
/// escalate etmez — bu test o yanlış negatifi yakalar.
#[test]
fn cc_port_app_keeps_sending_during_silence_still_escalates() {
    let mut s = warm_then_idle(Sender::with_cc_port());
    let mut saw_fwd = false;
    let mut first_escalate_ns: Option<u64> = None;
    let mut prev_conf = 1000;
    // 60 s boyunca her 200 ms'de bir yeni birim; hiç ACK yok.
    for k in 0..300u64 {
        let now = T_RESUME + k * 200 * MS;
        s.on_event(Event::AppData(unit(1 + k)), now);
        assert!(
            s.inflight_bytes() > 0,
            "ACK yokken dönem kapanmamalı (t={now} ns)"
        );
        let conf = ack_conf(&s);
        assert!(
            conf <= prev_conf,
            "sessizlik boyunca yeni AppData güveni artırmamalı (t={now} ns: {prev_conf} → {conf})"
        );
        prev_conf = conf;
        if s.state() == SenderState::RecoverFwd {
            saw_fwd = true;
        }
        if s.emergency_level() > EmergencyLevel::Arq && first_escalate_ns.is_none() {
            first_escalate_ns = Some(now);
        }
    }
    assert!(
        prev_conf < 300,
        "60 s ACK sessizliği sonunda güven escalate eşiğinin altında olmalı (güven {prev_conf})"
    );
    assert!(
        saw_fwd,
        "uygulama gönderse de ACK yok → RECOVER_FWD'e girmeli"
    );
    let first = first_escalate_ns.expect("uygulama gönderse de emergency escalate etmeli");
    assert!(
        first > T_RESUME + 3 * SEC,
        "escalation resume'dan en az 3 s sonra olmalı (ilk escalation t={first} ns)"
    );
}

/// `max(last_ack, first_send)` çapasının `last_ack` yarısı: aktif dönem içinde ACK'ler geldikçe
/// sessizlik son ACK'ten sayılır, dönem başından değil. Her adımda yalnız önceki adımların
/// birimleri ACK'lenir, en yeni birim uçuşta kalır → dönem 30 s boyunca hiç kapanmaz.
#[test]
fn cc_port_acks_inside_active_period_keep_confidence_full() {
    let mut s = warm_then_idle(Sender::with_cc_port());
    let mut unacked: Vec<u64> = Vec::new();
    for k in 0..150u64 {
        let t_send = T_RESUME + k * 200 * MS;
        let out = s.on_event(Event::AppData(unit(1 + k)), t_send);
        let fresh: Vec<u64> = out
            .iter()
            .filter(|d| d.kind != TransmitKind::FecParity)
            .map(|d| d.seq)
            .collect();
        assert!(!fresh.is_empty(), "birim aynı tikte tele çıkmalı (k={k})");
        assert_eq!(ack_conf(&s), 1000, "gönderim tikinde güven FULL (k={k})");

        if !unacked.is_empty() {
            let t_ack = t_send + 100 * MS;
            s.on_event(ack(&unacked), t_ack);
            assert!(
                s.inflight_bytes() > 0,
                "en yeni birim uçuşta: dönem açık kalmalı (k={k})"
            );
            assert_eq!(ack_conf(&s), 1000, "ACK tikinde güven FULL (k={k})");
            unacked.clear();
        }
        unacked.extend(fresh);
        assert_ne!(
            s.state(),
            SenderState::RecoverFwd,
            "ACK akarken RECOVER_FWD yok (k={k})"
        );
        assert_eq!(
            s.emergency_level(),
            EmergencyLevel::Arq,
            "ACK akarken emergency yok (k={k})"
        );
    }
}

/// Eski motor: bugünkü davranış pinli (düzeltme `cc_on()` ile kapılı). Boşta süre sessizliğe
/// eklendiği için resume'un ilk mesajı RECOVER_FWD'ye girer: tikte Dup2, sonraki tikte Dup3.
#[test]
fn legacy_resume_after_idle_behaviour_is_pinned() {
    let mut s = warm_then_idle(Sender::new());
    assert!(!s.cc_on());

    s.on_event(Event::AppData(unit(1)), T_RESUME);
    assert_eq!(
        s.state(),
        SenderState::RecoverFwd,
        "eski motor: resume tikinde RECOVER_FWD"
    );
    assert_eq!(s.emergency_level(), EmergencyLevel::Dup2);
    assert!(
        ack_conf(&s) < 300,
        "eski motor: güven escalate eşiğinin altında"
    );

    s.on_event(Event::TimerFired(Timer::Pto), T_RESUME + MS);
    assert_eq!(s.state(), SenderState::RecoverFwd);
    assert_eq!(s.emergency_level(), EmergencyLevel::Dup3);
}
