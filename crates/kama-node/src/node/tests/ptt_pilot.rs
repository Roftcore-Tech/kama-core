//! [PTT-BİRLEŞİK · pilot · #233] Gerçek istemcinin (düğümün) pilot bağlantısı.
//!
//! İki elle sürülen ÜRÜN düğümü (`test_support::manual`: `KamaNode::build`, gerçek
//! `ClientTransport`, gerçek kripto, loopback UDP). Pilot ayarı `KAMA_PTT_PILOT`'un test
//! karşılığıyla (`PTT_PILOT_FOR_TEST`; süreç ortamı paylaşıldığından ortam değişkeni kurulmaz),
//! yön hızı profili ürün yoluyla (`<data_dir>/link-profile`) verilir.
//!
//! Tel gözlemi: Alice'in Transport birim izi (`UnitTraceKind::Sent`: `send`'in kabul ettirdiği
//! her AMP kaydının BOYU, sırasıyla; iz yük baytı taşımaz) ve Bob'un PTT motoru (akış birleşik
//! açılışla mı açıldı, `END` ile mi bitti; `PttOpened.bundle_ms`). Kayıtların bayt bayt eşitliği
//! `kama-app` `pilot_toward_an_unknown_or_refusing_peer_is_byte_identical_to_the_flag_off_path`
//! testindedir; burada düğümün o yolu seçtiği sınanır.

use std::fs;
use std::time::Instant;

use client_core::transport::UnitTraceKind;
use kama_app::codec::PttCodec;
use kama_app::ptt::{PttStats, SPURT_PILOT_BUNDLE_CODE};

use super::{call, connect_manuals, peer_of, Command};
use crate::error::NodeError;
use crate::event::NodeEvent;
use crate::test_support::{self as support, Events, Phone, TempDir};
use crate::Built;

const ALICE: &str = "alice";
const BOB: &str = "bob";
/// Çağıranın (media) verdiği süre: pilot seçmezse akış bununla açılır.
const CALLER_MS: u8 = 40;
/// Demet yükü ve basıştaki demet sayısı.
const PAYLOAD: usize = 24;
const BUNDLES: usize = 4;
/// Birleşik biçimde açılış bilgili demet başlığı ile kompakt başlık farkı (7 B − 2 B).
const OPEN_EXTRA: u32 = 5;

/// Baz (Alice) ve saha (Bob) yön hızı profilleri.
const BASE_RATES: &str = "12500,62";
const FIELD_RATES: &str = "62,12500";

/// Düğümü `pilot` ayarıyla (`None` = ayar yok) ve yön hızı profiliyle kurar; birim izi açık.
fn node(phone: &Phone, pilot: Option<&str>, rates: &str) -> (Built, Events) {
    fs::create_dir_all(phone.data_dir()).expect("veri dizini");
    // CAPS, bitmap ACK ve 0x17 yalnız geri bildirimi aç yön hızı profilinde çıkar
    // (`runtime::link_feedback_starved`): pilotun hattı (baz 12 500 / 62 B/s, saha tersi).
    fs::write(phone.data_dir().join("link-profile"), rates).expect("profil");
    crate::PTT_PILOT_FOR_TEST.with(|v| *v.borrow_mut() = pilot.map(str::to_owned));
    let (mut built, events) = support::manual(phone, support::system_clock());
    crate::PTT_PILOT_FOR_TEST.with(|v| *v.borrow_mut() = None);
    built.node.set_unit_trace_for_test(true);
    (built, events)
}

/// Bir basışın gözlemi.
struct Press {
    /// Düğümün döndürdüğü `bundle_ms` kodu ve birleşik biçim bildirimi.
    code: u8,
    coalesced: bool,
    /// Alice'in `send`'e kabul ettirdiği kayıtların boyları, sırasıyla.
    sent: Vec<u32>,
    /// Bob'un `PttOpened.bundle_ms`'i ve aldığı demet sayısı.
    bob_bundle_ms: u8,
    bob_bundles: usize,
    /// Bob'un PTT sayaçlarındaki artış: (birleşik açılış, `END` ile bitiş).
    bob_spurt: (u64, u64),
}

/// Basış: `open_press` (ya da `pilot = false` iken bugünkü `ptt_open` yolu) · [`BUNDLES`] demet
/// (`timed` iken oluşum anlı ve sonuncusu `last`) · kapanış; Bob `PttClosed` görene dek turlanır.
fn press(
    alice: &mut Built,
    bob: &mut Built,
    bob_events: &mut Events,
    pilot: bool,
    timed: bool,
) -> Press {
    let _ = alice
        .node
        .messenger
        .transport_mut()
        .take_unit_trace_for_test();
    let before: PttStats = bob.node.ptt.stats();
    let to_bob = peer_of(&alice.node, BOB);
    let opened = call(&mut alice.node, |reply| Command::PttOpen {
        peer: to_bob,
        codec: PttCodec::Lyra,
        bundle_ms: CALLER_MS,
        pilot,
        reply,
    })
    .expect("ptt_open");
    let (stream, code) = (opened.stream, opened.bundle_ms);
    for k in 0..BUNDLES {
        let timed = timed.then(|| (Instant::now(), k + 1 == BUNDLES));
        call(&mut alice.node, |reply| Command::PttPush {
            stream,
            payload: vec![0x5A; PAYLOAD],
            timed,
            reply,
        })
        .expect("ptt_push");
    }
    call(&mut alice.node, |reply| Command::PttClose { stream, reply }).expect("ptt_close");
    let (mut bob_bundle_ms, mut bob_bundles) = (None, 0usize);
    support::until("bob: PttClosed", || {
        alice.node.turn();
        bob.node.turn();
        let mut closed = false;
        for event in bob_events.drain_all() {
            match event {
                NodeEvent::PttOpened { bundle_ms, .. } => bob_bundle_ms = Some(bundle_ms),
                NodeEvent::PttBundle { .. } => bob_bundles += 1,
                NodeEvent::PttClosed { .. } => closed = true,
                _ => {}
            }
        }
        closed.then_some(())
    });
    let (trace, dropped) = alice
        .node
        .messenger
        .transport_mut()
        .take_unit_trace_for_test();
    assert_eq!(dropped, 0);
    let sent = trace
        .iter()
        .filter(|r| r.kind == UnitTraceKind::Sent)
        .map(|r| r.len)
        .collect();
    let after = bob.node.ptt.stats();
    Press {
        code,
        coalesced: opened.coalesced,
        sent,
        bob_bundle_ms: bob_bundle_ms.expect("bob: PttOpened"),
        bob_bundles,
        bob_spurt: (
            after.spurt_opens - before.spurt_opens,
            after.spurt_ends - before.spurt_ends,
        ),
    }
}

/// Birleşik 300 ms basışın beklenen gözlemi: ayrı açılış/kapanış kaydı YOK (yalnız [`BUNDLES`]
/// kayıt), ilk üçü açılış bilgili, sonuncusu kompakt `END`; Bob akışı 300 ms koduyla birleşik
/// açılıştan açar ve `END` ile bitirir.
fn assert_coalesced_300(p: &Press, what: &str) {
    assert_eq!(
        p.code, SPURT_PILOT_BUNDLE_CODE,
        "{what}: düğüm 300 ms kodunu döner"
    );
    assert_eq!(
        p.sent.len(),
        BUNDLES,
        "{what}: ayrı PTT_OPEN / PTT_CLOSE yok: {:?}",
        p.sent
    );
    assert!(
        p.sent[..3].iter().all(|&len| len == p.sent[3] + OPEN_EXTRA),
        "{what}: ilk üç demet açılışlı (7 B), sonuncu kompakt END (2 B): {:?}",
        p.sent
    );
    assert_eq!(p.bob_bundle_ms, SPURT_PILOT_BUNDLE_CODE, "{what}");
    assert_eq!(p.bob_bundles, BUNDLES, "{what}");
    assert_eq!(p.bob_spurt, (1, 1), "{what}: birleşik açılış ve END");
    assert!(
        p.coalesced,
        "{what}: düğüm birleşik biçimi çağırana bildirir"
    );
}

/// Bayrak açık + YÖNETİLEN eş: ilk basış (bant içi bildirim beklenmeden) birleşik biçimde ve
/// 300 ms'yle gider; düğüm seçilen süreyi çağırana döner.
#[test]
fn provisioned_peer_gets_coalesced_300_ms_bundles_from_the_node() {
    let _serial = support::serial();
    let root = TempDir::new("ptt-pilot-provisioned");
    let phones = support::provision(root.path(), &[ALICE, BOB]);
    let (mut alice, mut alice_events) = node(&phones[ALICE], Some("1:bob"), BASE_RATES);
    let (mut bob, mut bob_events) = node(&phones[BOB], Some("1"), FIELD_RATES);
    assert!(alice.node.messenger.transport().ptt_coalesce_enabled());
    connect_manuals(
        &mut alice.node,
        &mut alice_events,
        ALICE,
        &mut bob.node,
        &mut bob_events,
        BOB,
    );
    let p = press(&mut alice, &mut bob, &mut bob_events, true, true);
    assert_coalesced_300(&p, "yönetilen eş");
}

/// Bayrak açık + BİLİNMEYEN eş: basış bugünkü uyumlu biçimle gider ve kayıt dizisi bayrak kapalı
/// düğümün (bugünkü `ptt_open` / `ptt_push` yolu ve `open_press` / `ptt_push_timed` yolu)
/// dizisiyle aynıdır. Basış sırasında gelen 0x17 o basışı DEĞİŞTİRMEZ; düğüm onu kaydeder ve
/// SONRAKİ basış birleşik 300 ms olur (bant içi doğrulama).
#[test]
fn unknown_peer_stays_compatible_and_a_later_press_uses_the_in_band_verified_pilot() {
    let _serial = support::serial();
    // Kol 0: bayrak kapalı, bugünkü yol. Kol 1: bayrak kapalı, pilot API'si. Kol 2: bayrak açık,
    // yetenek bilinmiyor, pilot API'si.
    let mut sequences: Vec<Vec<u32>> = Vec::new();
    for (arm, pilot_setting, pilot_api) in [
        ("kapalı/bugünkü", None, false),
        ("kapalı/pilot API", None, true),
        ("açık/bilinmiyor", Some("1"), true),
    ] {
        let root = TempDir::new("ptt-pilot-unknown");
        let phones = support::provision(root.path(), &[ALICE, BOB]);
        let (mut alice, mut alice_events) = node(&phones[ALICE], pilot_setting, BASE_RATES);
        let (mut bob, mut bob_events) = node(&phones[BOB], pilot_setting, FIELD_RATES);
        assert_eq!(
            alice.node.messenger.transport().ptt_coalesce_enabled(),
            pilot_setting.is_some()
        );
        connect_manuals(
            &mut alice.node,
            &mut alice_events,
            ALICE,
            &mut bob.node,
            &mut bob_events,
            BOB,
        );
        let to_bob = peer_of(&alice.node, BOB);
        let session = alice.node.messenger.session_of(to_bob).expect("oturum");
        assert_eq!(
            alice.node.messenger.transport().peer_ptt_coalesce(session),
            None,
            "{arm}: basıştan önce yetenek bilinmiyor"
        );
        let p = press(&mut alice, &mut bob, &mut bob_events, pilot_api, pilot_api);
        assert_eq!(p.code, CALLER_MS, "{arm}: çağıranın süresi");
        assert_eq!(
            p.sent.len(),
            BUNDLES + 2,
            "{arm}: PTT_OPEN · demetler · PTT_CLOSE"
        );
        assert_eq!(p.bob_bundle_ms, CALLER_MS, "{arm}");
        assert_eq!(p.bob_bundles, BUNDLES, "{arm}");
        assert_eq!(p.bob_spurt, (0, 0), "{arm}: ayrı kayıtlar");
        assert!(!p.coalesced, "{arm}: düğüm uyumlu biçimi bildirir");
        sequences.push(p.sent);
        if pilot_setting.is_some() {
            // Bant içi 0x17 (ilk basışın verisine gelen ACK) düğümce kaydedilir.
            support::until("alice: bant içi 0x17", || {
                alice.node.turn();
                bob.node.turn();
                let _ = bob_events.drain_all();
                (alice.node.messenger.transport().peer_ptt_coalesce(session) == Some(true))
                    .then_some(())
            });
            let later = press(&mut alice, &mut bob, &mut bob_events, true, true);
            assert_coalesced_300(&later, "bant içi doğrulanmış eş, sonraki basış");
        }
    }
    assert_eq!(
        sequences[1], sequences[0],
        "bayrak kapalı: pilot API'si bugünkü yolla aynı"
    );
    assert_eq!(
        sequences[2], sequences[0],
        "bilinmeyen eş: bayrak kapalı yolla aynı"
    );
}

/// Yönetilen eş adı cihaz dosyasında yoksa düğüm sessizce pilotsuz başlamaz.
#[test]
fn unknown_provisioned_name_is_a_configuration_error() {
    let _serial = support::serial();
    let root = TempDir::new("ptt-pilot-bad-name");
    let phones = support::provision(root.path(), &[ALICE, BOB]);
    crate::PTT_PILOT_FOR_TEST.with(|v| *v.borrow_mut() = Some("1:zed".to_owned()));
    let built = support::try_manual(&phones[ALICE], support::system_clock());
    crate::PTT_PILOT_FOR_TEST.with(|v| *v.borrow_mut() = None);
    assert!(
        matches!(built, Err(NodeError::Config(_))),
        "kurulum reddedilmeli"
    );
}
