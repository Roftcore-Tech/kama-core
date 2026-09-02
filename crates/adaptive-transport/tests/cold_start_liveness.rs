//! Phase-A (Commit-FO) — COLD-START LIVENESS direct-integration (GERÇEK Sender; UDP/harness-YOK;
//! SİMÜLE-ZAMAN: now_ns elle-ilerletilir, gerçek-uyku-YOK → ~61s bound anında-taranır).
//!
//! ÇERÇEVE: Bu deney ARQ ile FEC'i karşılaştırmaz. ARQ bütün-kollarda-aktiftir; ölçülen ARQ üzerine-
//! eklenen proaktif-FEC'in marjinal-faydasıdır. (Bu faz UTILITY değil LIVENESS sınıflandırır.)
//!
//! PROGRESS INVARIANT (PRE-DECLARED; koddan, sonuç-görülmeden — bkz. sender.rs 944-954/1177/1382-1388/
//! 2727-2742/2669-2698): PreConverge'de TEK ACK-BAĞIMSIZ ilerleme = §3.3-B geometrik-kopya; ilk-kopya
//! `first_delay = (3/2)(unit_size + PC_SACK_WIRE=65)/PRE_CONVERGE_FLOOR_BW=31` saniye; sonraki-kopyalar
//! ×2 backoff. State-ilerleme (PreConverge→Startup) YAPISAL-ACK-KAPILI (srtt.is_some yalnız boş-olmayan-
//! ACK ile; sender.rs 1385/1136/987-989). Bound: ilk-eligible-send'den-sonra en-geç 2×first_delay içinde
//! ACK-BAĞIMSIZ bir Duplicate emit-EDİLMELİ (yoksa SilentStall); state-ilerleme ACK-YOKKEN yapısal-
//! imkansızdır (bu bir liveness-özelliğidir, harness-artefaktı-DEĞİL).
//!
//! DR-CS-1 ROLÜ (F0): bu-4-test FLAG-OFF (`experimental-cold-start` KAPALI) beklentileridir ve
//! AYNEN KORUNUR — INV-CS-5 flag-OFF-kimliğinin canlı-pinleri (DI-5). Flag-ON kabul-matrisi ayrı:
//! tests/dr_cs1_acceptance.rs (F0'da-ignored-iskelet).

use adaptive_transport::sender::{
    AckInfo, Event, OpaqueUnit, Sender, SenderState, Timer, TransmitKind,
};
use std::time::Duration;

const FLOOR_BW: u64 = 31; // PRE_CONVERGE_FLOOR_BW (sender.rs §596)
const PC_SACK_WIRE: u64 = 65;

fn first_delay_ns(size: u64) -> u64 {
    // (3/2)·(size + 65)/31 saniye → ns.
    ((size + PC_SACK_WIRE) * 1_000_000_000 / FLOOR_BW) * 3 / 2
}

struct Emit {
    t_ns: u64,
    kind: TransmitKind,
    #[allow(dead_code)]
    seq: u64,
    unit_id: u64,
}

/// İlk-source gönder; ACK-VERME; now_ns'i `horizon_ns`'e kadar `step_ns` ile ilerlet + her adımda Pto-fire.
/// Dönüş: (initial_emit, tüm-emit-timeline, gözlenen-state'ler).
fn cold_start_no_ack(size: u64, horizon_ns: u64, step_ns: u64) -> (Vec<Emit>, Vec<SenderState>) {
    let mut s = Sender::new();
    let mut emits = Vec::new();
    let mut states = Vec::new();
    // t=0: ilk AppData → PreConverge + initial burst (1-birim).
    let out = s.on_event(Event::AppData(OpaqueUnit { id: 1, size }), 0);
    for d in out {
        emits.push(Emit {
            t_ns: 0,
            kind: d.kind,
            seq: d.seq,
            unit_id: d.unit_id,
        });
    }
    states.push(s.state());
    // ACK YOK (ilk-paket-kaybı). Zamanı ilerlet + Pto-fire.
    let mut t = step_ns;
    while t <= horizon_ns {
        let out = s.on_event(Event::TimerFired(Timer::Pto), t);
        for d in out {
            emits.push(Emit {
                t_ns: t,
                kind: d.kind,
                seq: d.seq,
                unit_id: d.unit_id,
            });
        }
        states.push(s.state());
        t += step_ns;
    }
    (emits, states)
}

/// PROGRESS INVARIANT — ilk-paket-kaybı + ACK-yok: en-geç 2×first_delay içinde ACK-BAĞIMSIZ Duplicate
/// (§3.3-B geometrik-kopya) emit-edilir (SilentStall DEĞİL). Ama state PreConverge'de PİNLİ kalır (ACK-
/// kapılı). Bu, gerçek-Sender-özelliği (direct-integration; harness-artefaktı-değil).
#[test]
fn cold_start_first_loss_recovers_ack_free_but_state_pinned() {
    let size = 1200u64;
    let fd = first_delay_ns(size);
    let horizon = fd * 3; // 3×first_delay (~183s simüle; gerçek-uyku-yok)
    let step = 100_000_000u64; // 100ms simüle-adım
    let (emits, states) = cold_start_no_ack(size, horizon, step);

    // (1) t=0'da initial Data burst (1-birim) — ilk eligible send.
    let initial: Vec<_> = emits.iter().filter(|e| e.t_ns == 0).collect();
    assert!(
        initial.iter().any(|e| e.kind == TransmitKind::Data),
        "t=0 initial Data-emit bekleniyor (cold-start ilk-eligible-send)"
    );

    // (2) ACK-BAĞIMSIZ Duplicate en-geç 2×first_delay içinde emit-edilmeli (SilentStall-değil).
    let first_dup = emits
        .iter()
        .find(|e| e.kind == TransmitKind::Duplicate)
        .map(|e| e.t_ns);
    assert!(
        first_dup.is_some(),
        "ACK-bağımsız §3.3-B geometrik-kopya (Duplicate) HİÇ emit-edilmedi → SilentStall (bound {}s)",
        horizon / 1_000_000_000
    );
    let dup_t = first_dup.unwrap();
    assert!(
        dup_t <= 2 * fd,
        "ilk-Duplicate t={}s > 2×first_delay={}s (progress-bound-aşımı)",
        dup_t / 1_000_000_000,
        2 * fd / 1_000_000_000
    );
    // Duplicate ilk-flight-unit'i (seq 0) yeniden-gönderir.
    // Duplicate FRESH-seq alır (nonce-reuse-invariant) ama AYNI unit'i (unit_id=1) yeniden-gönderir.
    assert_eq!(
        emits
            .iter()
            .find(|e| e.kind == TransmitKind::Duplicate)
            .unwrap()
            .unit_id,
        1,
        "geometrik-kopya ilk-flight unit'ini (unit_id=1) retransmit-etmeli (seq-yeni)"
    );

    // (3) State ACK-YOKKEN yapısal olarak PreConverge'de PİNLİ (asla Startup'a geçmez).
    assert!(
        states.iter().all(|st| *st == SenderState::PreConverge),
        "ACK-yok → state PreConverge'de-pinli olmalı (ACK-kapılı çıkış); gözlenen: {:?}",
        states.iter().find(|st| **st != SenderState::PreConverge)
    );
}

/// EVIDENCE — cold-start emit/state timeline'ını CSV-artefaktına yaz (Phase-A/FP kanıtı). Birden-çok
/// unit-boyutu için first_delay ve GÖZLENEN ilk-Duplicate zamanını belgeler (bound-doğrulama).
#[test]
fn cold_start_timeline_evidence_artifact() {
    // M2a REPO HYGIENE: a test MUST NOT write into the repo tree. Default target is this test
    // binary's private temp dir (CARGO_TARGET_TMPDIR is per-package and per-run, so parallel tests
    // cannot collide). The canonical repo evidence is regenerated ONLY on an explicit opt-in, and
    // the target files are announced when it happens.
    // Scientific content, test inputs and transport behaviour are UNCHANGED — only the destination.
    let repo_dir = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../reports/d1-exit/data/cold-start"
    );
    let regen_repo = std::env::var("COLD_START_EVIDENCE_REPO").is_ok();
    let tmp_dir = format!("{}/cold-start-evidence", env!("CARGO_TARGET_TMPDIR"));
    let dir: &str = if regen_repo {
        eprintln!(
            "COLD_START_EVIDENCE_REPO set — regenerating canonical evidence in {repo_dir}: \
             summary.csv, timeline-{{65,300,1200}}B.csv"
        );
        repo_dir
    } else {
        &tmp_dir
    };
    std::fs::create_dir_all(dir).ok();
    let mut sum = String::from(
        "unit_size,first_delay_s,observed_first_dup_s,within_2x_bound,dup_count,final_state_preconverge\n",
    );
    for size in [65u64, 300, 1200] {
        let fd = first_delay_ns(size);
        let (emits, states) = cold_start_no_ack(size, fd * 4, 50_000_000);
        let first_dup = emits
            .iter()
            .find(|e| e.kind == TransmitKind::Duplicate)
            .map(|e| e.t_ns);
        let dup_count = emits
            .iter()
            .filter(|e| e.kind == TransmitKind::Duplicate)
            .count();
        let pinned = states.iter().all(|st| *st == SenderState::PreConverge);
        let obs = first_dup.map_or(-1.0, |t| t as f64 / 1e9);
        let within = first_dup.is_some_and(|t| t <= 2 * fd);
        sum.push_str(&format!(
            "{size},{:.1},{obs:.1},{within},{dup_count},{pinned}\n",
            fd as f64 / 1e9
        ));
        // per-size detaylı emit-timeline (ilk 40 emit).
        let mut det = String::from("t_s,kind,seq,unit_id\n");
        for e in emits.iter().take(40) {
            det.push_str(&format!(
                "{:.2},{:?},{},{}\n",
                e.t_ns as f64 / 1e9,
                e.kind,
                e.seq,
                e.unit_id
            ));
        }
        std::fs::write(format!("{dir}/timeline-{size}B.csv"), det).ok();
    }
    std::fs::write(format!("{dir}/summary.csv"), &sum).ok();
    // Kanıt-özeti her-koşulda PreConverge-pinli + Duplicate-var olmalı (invariant-tekrarı).
    assert!(
        sum.contains(",true,"),
        "en-az-bir boyutta within-bound Duplicate + pinned"
    );
    // Content is verified AT the destination, wherever that is.
    let written = std::fs::read_to_string(format!("{dir}/summary.csv"))
        .expect("evidence summary must be readable at its destination");
    assert_eq!(
        written, sum,
        "written evidence must match what was asserted"
    );
    for size in [65u64, 300, 1200] {
        let t = std::fs::read_to_string(format!("{dir}/timeline-{size}B.csv"))
            .expect("per-size timeline must be readable at its destination");
        assert!(
            t.starts_with("t_s,kind,seq,unit_id\n"),
            "timeline-{size}B.csv header"
        );
    }
}

/// KONTROL — hayatta-kalan ACK PreConverge'i açar: ilk-source teslim + ACK gelirse state Startup'a
/// (veya ötesine) ilerler. Bu, mekanizmanın ACK-varlığında çalıştığını kanıtlar (deadlock ACK-yokluğuna-özgü).
#[test]
fn cold_start_surviving_ack_unlocks_preconverge() {
    let mut s = Sender::new();
    let _ = s.on_event(Event::AppData(OpaqueUnit { id: 1, size: 1200 }), 0);
    assert_eq!(
        s.state(),
        SenderState::PreConverge,
        "cold-start → PreConverge"
    );
    // seq 0 için ACK teslim et (RTT-örneği → srtt.is_some).
    let _ = s.on_event(
        Event::AckReceived(AckInfo {
            acked_seqs: vec![0],
            ack_delay: Duration::from_millis(1),
        }),
        50_000_000, // 50ms RTT
    );
    assert_ne!(
        s.state(),
        SenderState::PreConverge,
        "hayatta-kalan-ACK → PreConverge'den-çıkış (srtt-kilidi-açıldı)"
    );
}

/// SINIFLANDIRMA-yardımcısı: ACK-yokluğunda state'in HİÇBİR-koşulda ilerlemediğini (Idle-dışı) ve tek-
/// ilerlemenin caller-clocked-geometrik-kopya olduğunu belgeler — "harness-artefaktı-değil, yapısal".
#[test]
fn cold_start_state_progression_is_ack_gated_not_timer_gated() {
    let size = 1200u64;
    let fd = first_delay_ns(size);
    // Çok-uzun ufuk (5×first_delay) + sık Pto → zaman-tetikli-ilerleme OLSAYDI görülürdü.
    let (emits, states) = cold_start_no_ack(size, fd * 5, 50_000_000);
    // Zaman-tetikli hiçbir state-ilerleme YOK (hepsi PreConverge).
    let progressed = states.iter().any(|st| *st != SenderState::PreConverge);
    assert!(
        !progressed,
        "zaman-tek-başına state-ilerletMEZ (ACK-kapılı)"
    );
    // Ama ACK-bağımsız Duplicate'ler VAR (birden-fazla; geometrik-backoff).
    let dups = emits
        .iter()
        .filter(|e| e.kind == TransmitKind::Duplicate)
        .count();
    assert!(
        dups >= 1,
        "en-az-bir ACK-bağımsız geometrik-kopya beklenir (caller-clocked ilerleme)"
    );
}
