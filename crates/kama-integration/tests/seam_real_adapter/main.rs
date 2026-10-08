//! **real-adapter** — yalnız gerçek implementasyonda anlamı olan testler.
//!
//! Socket reset, PMTU kara deliği, kripto dizini, ürün builder'ının reddettiği yapılandırmalar:
//! bunlar sahtede anlamsızdır (sahtede socket ve Crypto yoktur), o yüzden conformance-core'un
//! dışında dururlar. Ayrı ikilide olmaları sahteyi "gerçek gibi" davranmaya zorlayan sahte-sadakat
//! baskısını da ortadan kaldırır.
//!
//! # L-T1 (2026-10-08): gerçek hedef KAYITLI
//!
//! ~~Bugün: BEYAN EDİLMİŞ BOŞLUK — gerçek adapter yok~~ *[2026-10-08: kapandı]*. Gerçek hedef
//! `harness::real::RealTarget`'tır (`client_core::transport::ClientTransport` + gerçek kripto + UDP
//! loopback). Bu ikili onun VARLIĞINI sınar ve runner'ın okuduğu sentinel satırını basar
//! (`PRESENT`); kayıt düşerse [`real_target_is_registered`] kırmızıya döner.

#[path = "../common/seam/harness.rs"]
mod harness;

use std::net::SocketAddr;

use client_core::memory_link::MemoryHub;
use client_core::transport::{ClientTransport, ClientTransportConfig, TransportBuildError};
use harness::target::{TargetKind, REGISTERED_TARGETS};
use kama_contracts::s1::{AbandonReason, ConnectPhase, ConnectResult, KamaTransport, SessionEvent};
use kama_noise_wire::seam_mock::{fixture_directory, fixture_protection};
use kama_noise_wire::session_control::EnvelopeKind;

/// Runner'ın okuduğu makine-satırı (`ABSENT` = beyan edilmiş boşluk, çıkış 3).
const REAL_TARGET_SENTINEL: &str = "SEAM-TARGET-REAL";

/// Gerçek hedefe özgü olup HENÜZ yazılmamış vakalar — beyandır, iddia değildir.
const REAL_ONLY_CASES_PLANNED: &[&str] = &[
    "socket reset (ECONNRESET) — oturum durumu ne olur",
    "PMTU kara deliği — segment bütçesi daralınca capabilities ne der",
];

/// Gerçek hedef kayıtlı ve ailesi `Real`.
#[test]
fn real_target_is_registered() {
    let real: Vec<&str> = REGISTERED_TARGETS
        .iter()
        .filter(|(_, kind)| *kind == TargetKind::Real)
        .map(|(id, _)| *id)
        .collect();
    assert_eq!(
        real,
        ["real"],
        "gerçek hedef kayıtlı olmalı (L-T1); kayıt düştüyse kapsam sessizce daralmıştır"
    );
    println!("{REAL_TARGET_SENTINEL}: PRESENT gap=none");
}

/// Planlanan gerçek-hedef vakaları kimlikli ve tekrarsız beyan edilir.
#[test]
fn planned_real_only_cases_are_declared() {
    for (i, case) in REAL_ONLY_CASES_PLANNED.iter().enumerate() {
        assert!(!case.trim().is_empty(), "boş vaka beyanı (index {i})");
        assert!(
            !REAL_ONLY_CASES_PLANNED[..i].contains(case),
            "vaka iki kez beyan edildi: {case}"
        );
    }
}

fn loopback() -> SocketAddr {
    SocketAddr::from(([127, 0, 0, 1], 0))
}

/// S1 F0.2(iv): ürün builder'ı kurtarma KAPALI yapılandırmayı reddeder (`reliable = false` yalnız
/// test yapılandırmasıdır).
#[test]
fn product_builder_rejects_recovery_off() {
    let me = harness::test_peer(0x0A);
    let dir = fixture_directory([*me.as_bytes()]).expect("dizin");
    let (crypto, _clock) = fixture_protection(*me.as_bytes(), dir, EnvelopeKind::Legacy);
    let mut cfg = ClientTransportConfig::product(loopback());
    cfg.recovery = false;
    let hub = MemoryHub::new();
    let r = ClientTransport::new(cfg, crypto, Box::new(hub.link(me)));
    assert!(
        matches!(r, Err(TransportBuildError::RecoveryOff)),
        "ürün builder'ı recovery=false'u reddetmeli"
    );
}

/// G1 (2026-10-08): doğrulanmış eş dizininde OLMAYAN eşe `connect` → `Pending` + `Measuring` +
/// `ConnectAbandoned(LocalPermanent)` — Crypto'nun reddi opaktır, Application'a kripto nedeni
/// sızmaz; niyet kapalı bir nedenle biter. (Sahtede dizin yoktur.)
#[test]
fn peer_outside_the_directory_is_a_local_permanent_abandon() {
    let me = harness::test_peer(0x0B);
    let stranger = harness::test_peer(0x0C);
    let dir = fixture_directory([*me.as_bytes()]).expect("dizin");
    let (crypto, _clock) = fixture_protection(*me.as_bytes(), dir, EnvelopeKind::Legacy);
    let mut cfg = ClientTransportConfig::product(loopback());
    // Adres defterinde olsa bile (direct-mode) eş malzemesi yoktur.
    cfg.peer_addrs.insert(stranger, loopback());
    let hub = MemoryHub::new();
    let mut t = ClientTransport::new(cfg, crypto, Box::new(hub.link(me))).expect("hedef");
    let ticket = match t.connect(stranger, None) {
        Ok(ConnectResult::Pending(ticket)) => ticket,
        other => panic!("Pending bekleniyordu, gelen {other:?}"),
    };
    assert_eq!(
        t.poll_event(),
        Some(SessionEvent::ConnectProgress(
            ticket,
            ConnectPhase::Measuring
        ))
    );
    assert_eq!(
        t.poll_event(),
        Some(SessionEvent::ConnectAbandoned(
            ticket,
            AbandonReason::LocalPermanent
        ))
    );
    assert!(t.poll_event().is_none());
    assert!(hub.is_idle(), "reddedilen niyet zarf göndermemeli");
}
