//! [IW] PreConverge ilk-uçuş penceresi — kuruluş olasılığının doğrudan kaldıracı.
//!
//! NEDEN ÖNEMLİ (ölçülü zincir): PreConverge'den çıkışın TEK yolu bir RTT örneğidir
//! (`sender.rs`: `if self.rtt.srtt().is_some() { Startup }`), 31 B/s tabanını kaldırmanın da
//! tek yolu odur (`compute_send_rate` PreConverge'de koşulsuz `PRE_CONVERGE_FLOOR_BW` döner).
//! RTT örneği ise YALNIZ orijinal DATA emisyonlarından alınır (Karn: `on_ack`'te
//! `p.origin == TransmitKind::Data` kapısı) — kopyalar ve probe'lar örnek üretmez.
//!
//! Dolayısıyla ilk uçuştaki ORİJİNAL sayısı N, kuruluş olasılığını belirler: hiç örnek alamama
//! olasılığı `p_loss^N`. Sevk davranışında N = 1 (pacer clamp'i), yani %50 kayıpta kuruluş
//! yazı-tura. QUIC'in IW=10'u (RFC 9002 §7.2) aynı rolü oynar ve handshake ürünü DEĞİLDİR —
//! bu bir congestion-control tercihidir, handshake'siz bir transport da taşıyabilir.
//!
//! Bu dosya iki şeyi pinler: (1) sevk davranışı gerçekten tek-paket, (2) pencere açıkken ilk
//! uçuş bütçe kadar orijinal taşıyor ve KAPALIYKEN davranış birebir aynı kalıyor.

use adaptive_transport::sender::*;

/// t=0'da (hiç timer/ACK olmadan) wire'a çıkan ORİJİNAL DATA sayısı = ilk uçuş.
fn first_flight_originals(iw_bytes: u64, unit_size: u64, offered: u64) -> usize {
    let mut s = Sender::new();
    // Kuruluş-denemesi kolu kapalı: bu test İLK UÇUŞU ölçer, sonraki merdiveni değil.
    s.set_establish_probe(false);
    if iw_bytes > 0 {
        s.set_initial_window(InitialWindowConfig { bytes: iw_bytes });
    }
    let mut originals = 0usize;
    for id in 0..offered {
        for d in s.on_event(
            Event::AppData(OpaqueUnit {
                id,
                size: unit_size,
            }),
            0,
        ) {
            if d.kind == TransmitKind::Data {
                originals += 1;
            }
        }
    }
    originals
}

/// Sevk davranışı: ilk uçuş TAM BİR paket — kuruluş tek bir datagramın sağ kalmasına bağlı.
#[test]
fn shipped_initial_flight_is_exactly_one_packet() {
    assert_eq!(
        first_flight_originals(0, 283, 20),
        1,
        "sevk davranışı: pacer burst-cap'i front_size'a clamp'ler → ilk uçuş tek paket"
    );
    // Birim boyundan bağımsız: küçük birimde de tek paket (clamp birim-boyuna kilitli).
    assert_eq!(first_flight_originals(0, 64, 20), 1);
}

/// Pencere açıkken ilk uçuş bütçe kadar orijinal taşır. 2400 B / 283 B ≈ 8 paket — QUIC'in
/// IW=10'uyla aynı mertebe, ve hepsi Karn-uygun (her biri bağımsız bir RTT örneği şansı).
#[test]
fn initial_window_widens_the_first_flight_to_the_budget() {
    let n = first_flight_originals(2400, 283, 20);
    assert!(
        n >= 8,
        "2400 B bütçe 283 B'lik birimden en az 8 orijinal taşımalı; ölçülen {n}"
    );
    assert!(
        n <= 9,
        "bütçe AŞILMAMALI (2400/283 = 8.5 → en çok 8-9); ölçülen {n}"
    );
    // Bütçe ölçeklenir: yarısı yarı kadar paket.
    let half = first_flight_originals(1200, 283, 20);
    assert!(
        (4..=5).contains(&half),
        "1200 B bütçe ~4 orijinal taşımalı; ölçülen {half}"
    );
}

/// Sunulan birim sayısı bütçeden azsa pencere ONLARI aşmaz (yapay trafik üretmez).
#[test]
fn initial_window_never_invents_traffic_beyond_what_the_app_offered() {
    assert_eq!(
        first_flight_originals(2400, 283, 3),
        3,
        "app 3 birim sunduysa ilk uçuş 3'ü geçemez"
    );
}

/// Default KAPALI: `Sender::new()` penceresiz gelir (R1 — kampanya görmemiş eşik sevk edilmez).
#[test]
fn initial_window_is_off_by_default() {
    assert_eq!(
        Sender::new().initial_window(),
        InitialWindowConfig::default()
    );
    assert_eq!(Sender::new().initial_window().bytes, 0);
}
