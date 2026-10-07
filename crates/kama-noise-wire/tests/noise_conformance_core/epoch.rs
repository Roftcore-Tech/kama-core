//! EPOCH MATRİSİ (A3.2 → A5.4) — epoch 0→1 confirmation-safe geçiş; noise-wire §4 (beş adım) +
//! KS-GRACE-1 (`N_g` = 64 AEAD-geçerli epoch-1 paketi, onaylayan #1 · `T_g` = 30 s, onay anından ·
//! hangisi önce · kapanınca epoch-0 DISCARD, yeniden açılmaz).
//!
//! **[2026-10-08, A5.4 — L-A5c PR-2] Sınanan şey GERÇEK ürün yoludur:** iki uç
//! `NoiseWireProtection::with_control` ile kurulur, gerçek el sıkışma (SSI1 → SSR1) ve
//! HANDLE_ASSIGN'la bağlanır; `DhEpochMachine` protector'ın açma yolunda sürülür.
//! ~~`DhEpochMachine` bugün ürün kodunda hiçbir yere bağlı değildir; aşağıdaki `Endpoint` TEST
//! TUTKALIDIR~~ — tutkal kalktı. Hücreler iki zarf kipinde koşar: legacy 22 B (epoch telde YOK →
//! grace içinde ≤ 2 anahtar denemesi) ve extended 26 B (epoch telde → tek anahtar).
//!
//! **Uçuştaki epoch-0 paketi:** Alice'in bekleyen oturumu HANDLE_ASSIGN gelince epoch-0 için
//! kurulur (0-RTT, ccr §0.7-A "epoch-0-usable"); RESPONSE'tan ÖNCE mühürlenen paketler geçişten
//! sonra Bob'a ulaşabilir — grace onlar içindir.
//!
//! # Kırmızı hücreler
//!
//! [`DECLARED_RED`] BOŞTUR (A5.4 kabul ölçütü). Beyan mekanizması korunur: sarmalayıcı beyanla
//! gerçeği ÇAPRAZ denetler — beyansız kırmızı da, beyanlı ama yeşile dönmüş hücre de testi düşürür.

use core::time::Duration;

use kama_contracts::handles::SessionHandle;
use kama_contracts::s3::{
    HubHandle, PacketProtection, ProtectionContext, SessionCommand, SessionControl, SessionEvent,
    SessionResult,
};
use kama_noise_wire::clock::ManualClock;
use kama_noise_wire::epoch_machine::{RecvEpoch0, GRACE_MIN_PACKETS, GRACE_T_G};
use kama_noise_wire::protector::NoiseWireProtection;
use kama_noise_wire::session_control::EnvelopeKind;

use crate::s3_support::{peer, INITIATOR, PEER};
use crate::target::real_endpoint;

/// KS-GRACE-1 `N_g` (kodda `GRACE_N_G`; eski adı `GRACE_MIN_PACKETS`).
const N_G: u32 = GRACE_MIN_PACKETS;
/// Matrisin iki uca bağladığı hub tutamacı.
const HUB: u32 = 0x0000_E90C;

/// Kırmızı hücre kaydı: beklenen / gözlenen / kanıt yeri.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Red {
    pub expected: &'static str,
    pub observed: &'static str,
    pub at: &'static str,
}

pub type Cell = Result<(), Red>;

/// Bugün kırmızı olan hücreler — A5.4'ten beri BOŞ. ~~"cell_5b_n_g_is_fixed",
/// "cell_6_time_closes_grace", "cell_7_whichever_first", "cell_9_at_most_two_key_trials"~~
pub const DECLARED_RED: &[&str] = &[];

/// Koşucunun okuduğu makine satırı.
pub const RED_SENTINEL: &str = "EPOCH-RED-CELL";

/// Beyan ↔ gerçek çapraz denetimi.
///
/// # Panics
/// Hücre beyansız kırmızıysa ya da beyanlı olduğu hâlde yeşilse.
pub fn check(name: &str, outcome: Cell) {
    let declared = DECLARED_RED.contains(&name);
    match (outcome, declared) {
        (Ok(()), false) => {}
        (Err(red), true) => println!(
            "{RED_SENTINEL}: {name} | beklenen: {} | gözlenen: {} | {}",
            red.expected, red.observed, red.at
        ),
        (Err(red), false) => panic!("BEYANSIZ KIRMIZI {name}: {red:?}"),
        (Ok(()), true) => panic!("{name} yeşile döndü: DECLARED_RED'den sil"),
    }
}

// ── Fikstür: iki gerçek uç ───────────────────────────────────────────────────────────────────

/// Alice (`INITIATOR`) INIT'i yolladı, tutamacını aldı; Bob (`PEER`) kabul etti ve RESPONSE'u
/// üretti — RESPONSE Alice'e henüz TESLİM EDİLMEDİ (hücreler teslimi kendileri sürer).
struct World {
    a: NoiseWireProtection,
    b: NoiseWireProtection,
    ha: SessionHandle,
    hb: SessionHandle,
    clock_b: ManualClock,
    init: Vec<u8>,
    resp: Vec<u8>,
}

fn assign(c: &mut NoiseWireProtection, session: SessionHandle) {
    assert_eq!(
        c.command(SessionCommand::AssignHandle {
            session,
            hub_handle: HubHandle::from_raw(HUB),
        }),
        SessionResult::NoOutput
    );
}

fn world(kind: EnvelopeKind) -> World {
    let (mut a, _) = real_endpoint(peer(INITIATOR), kind);
    let (mut b, clock_b) = real_endpoint(peer(PEER), kind);
    let SessionResult::Initiated {
        session: ha,
        bytes: init,
        ..
    } = a.command(SessionCommand::Initiate {
        peer: peer(PEER),
        early_unit: None,
    })
    else {
        panic!("Initiate bekleyen oturum üretmeli");
    };
    assign(&mut a, ha);
    let SessionResult::EstablishedAndReply {
        session: hb,
        bytes: resp,
        ..
    } = b.command(SessionCommand::AcceptInit { wire: init.clone() })
    else {
        panic!("AcceptInit oturum + RESPONSE üretmeli");
    };
    assign(&mut b, hb);
    while a.poll_event().is_some() {}
    while b.poll_event().is_some() {}
    World {
        a,
        b,
        ha,
        hb,
        clock_b,
        init,
        resp,
    }
}

impl World {
    /// Alice mühürler; döner: (wire, dh_epoch).
    fn seal_a(&mut self, frame: &[u8]) -> (Vec<u8>, u8) {
        let u = self
            .a
            .seal(ProtectionContext::Session(self.ha), frame)
            .expect("Alice seal");
        (u.bytes, u.meta.dh_epoch)
    }

    fn seal_b(&mut self) -> u8 {
        self.b
            .seal(ProtectionContext::Session(self.hb), b"bob")
            .expect("Bob seal")
            .meta
            .dh_epoch
    }

    fn deliver_response(&mut self) -> SessionResult {
        self.a.command(SessionCommand::AcceptResponse {
            wire: self.resp.clone(),
        })
    }

    fn b_open(&mut self, w: &[u8]) -> bool {
        self.b.open(w).is_some()
    }

    fn b_epoch(&self) -> (u8, RecvEpoch0) {
        self.b.epoch_state_for_test(self.hb).expect("Bob oturumu")
    }

    fn b_grace_open(&self) -> bool {
        matches!(self.b_epoch().1, RecvEpoch0::Grace { .. })
    }

    /// Alice'in bir epoch-1 paketi Bob'a ulaşır.
    fn epoch1_to_b(&mut self) {
        let (p, e) = self.seal_a(b"epoch-1");
        assert_eq!(e, 1);
        assert!(self.b_open(&p), "geçerli epoch-1 paketi açılmalı");
    }
}

/// Onaylanmış çift: Alice RESPONSE'tan ÖNCE `inflight` epoch-0 paketi (0-RTT) mühürledi (uçuşta);
/// RESPONSE teslim edildi; Alice'in ilk epoch-1 paketi Bob'a ulaştı → onay, grace #1.
fn confirmed(kind: EnvelopeKind, inflight: usize) -> (World, Vec<Vec<u8>>) {
    let mut w = world(kind);
    let early: Vec<Vec<u8>> = (0..inflight)
        .map(|_| {
            let (p, e) = w.seal_a(b"0-RTT");
            assert_eq!(e, 0, "RESPONSE'tan önce epoch-0");
            p
        })
        .collect();
    assert_eq!(
        w.deliver_response(),
        SessionResult::Established { session: w.ha }
    );
    w.epoch1_to_b();
    assert!(
        matches!(w.b_epoch(), (1, RecvEpoch0::Grace { counted: 1, .. })),
        "onay: Bob epoch-1'e geçer, onaylayan paket #1"
    );
    (w, early)
}

fn tamper(wire: &[u8]) -> Vec<u8> {
    let mut w = wire.to_vec();
    let last = w.len() - 1;
    w[last] ^= 1;
    w
}

// ── Matris ───────────────────────────────────────────────────────────────────────────────────

/// 1 — RESPONSE normal: Bob onaydan önce epoch-0'da gönderir; Alice RESPONSE'la epoch-1'e geçer;
/// ilk geçerli epoch-1 paketiyle onay: Bob epoch-1'e geçer, grace açılır, epoch-0 hâlâ kabul.
pub fn cell_1_response_normal(kind: EnvelopeKind) -> Cell {
    let mut w = world(kind);
    assert_eq!(w.seal_b(), 0, "Bob onaydan önce epoch-0'da gönderir");
    let (early, e0) = w.seal_a(b"0-RTT");
    assert_eq!(e0, 0);
    assert_eq!(
        w.deliver_response(),
        SessionResult::Established { session: w.ha }
    );
    assert_eq!(w.a.poll_event(), Some(SessionEvent::Ready(w.ha)));
    w.epoch1_to_b();
    assert_eq!(w.seal_b(), 1, "onay: Bob epoch-1'e geçer");
    assert!(w.b_grace_open());
    assert!(w.b_open(&early), "grace boyunca epoch-0 hâlâ kabul");
    Ok(())
}

/// 2 — RESPONSE kayıp: INIT'in (byte-özdeş) yeniden gönderimi aynı oturumu ve aynı RESPONSE'u
/// verir; epoch-0 açık kalır; onay yok.
pub fn cell_2_response_lost(kind: EnvelopeKind) -> Cell {
    let mut w = world(kind);
    let fp = w.b.state_fingerprint();
    let again = w.b.command(SessionCommand::AcceptInit {
        wire: w.init.clone(),
    });
    assert!(
        matches!(again, SessionResult::EstablishedAndReply { session, ref bytes, .. }
            if session == w.hb && *bytes == w.resp),
        "yeniden gönderim aynı oturum + byte-özdeş RESPONSE vermeli"
    );
    assert_eq!(w.b.state_fingerprint(), fp, "ikinci oturum / state yok");
    let (p, e) = w.seal_a(b"0-RTT");
    assert_eq!(e, 0, "Alice epoch-0'da kalır");
    assert!(w.b_open(&p), "Bob epoch-0'ı kabul eder");
    assert_eq!(
        w.b_epoch(),
        (0, RecvEpoch0::Accepting),
        "onay yok, grace yok"
    );
    assert_eq!(w.a.pending_len(), 1, "Alice'in INIT'i bekliyor");
    Ok(())
}

/// 3 — RESPONSE sırasız / çift (INIT yeniden gönderiminden sonra): tek oturum, çift kurulum yok.
pub fn cell_3_response_reordered(kind: EnvelopeKind) -> Cell {
    let mut w = world(kind);
    let SessionResult::EstablishedAndReply { bytes: second, .. } =
        w.b.command(SessionCommand::AcceptInit {
            wire: w.init.clone(),
        })
    else {
        panic!("yeniden gönderilen INIT önbellekli RESPONSE almalı");
    };
    assert_eq!(second, w.resp);
    assert_eq!(
        w.deliver_response(),
        SessionResult::Established { session: w.ha }
    );
    assert_eq!(
        w.a.command(SessionCommand::AcceptResponse { wire: second }),
        SessionResult::Failed,
        "geç gelen ikinci RESPONSE ikinci kurulum yapmaz"
    );
    assert_eq!(w.a.pending_len(), 0);
    assert_eq!(w.a.poll_event(), Some(SessionEvent::Ready(w.ha)));
    assert_eq!(w.a.poll_event(), None, "tek Ready");
    Ok(())
}

/// 4 — AEAD-geçersiz epoch-1 paketi onayı tetiklemez, grace sayacını ilerletmez, state'e dokunmaz.
pub fn cell_4_invalid_epoch1_is_inert(kind: EnvelopeKind) -> Cell {
    let mut w = world(kind);
    assert_eq!(
        w.deliver_response(),
        SessionResult::Established { session: w.ha }
    );
    let (first, _) = w.seal_a(b"ilk epoch-1");
    let fp = w.b.state_fingerprint();
    assert!(!w.b_open(&tamper(&first)));
    assert_eq!(
        w.b.state_fingerprint(),
        fp,
        "geçersiz paket state'e dokundu"
    );
    assert_eq!(w.b_epoch(), (0, RecvEpoch0::Accepting), "onay tetiklendi");
    assert!(w.b_open(&first));
    assert!(
        matches!(w.b_epoch(), (1, RecvEpoch0::Grace { counted: 1, .. })),
        "geçerli paket onaylar ve #1 sayılır"
    );
    Ok(())
}

/// 5 — `N_g` geçerli epoch-1 paketi (onaylayan dahil) grace'i kapatır; sonraki epoch-0 paketi
/// reddedilir; yeniden açılmaz.
pub fn cell_5_count_closes_grace(kind: EnvelopeKind) -> Cell {
    let (mut w, early) = confirmed(kind, 1);
    for i in 2..=N_G {
        assert!(w.b_grace_open(), "{i}. paketten önce grace açık");
        w.epoch1_to_b();
    }
    assert_eq!(w.b_epoch().1, RecvEpoch0::Discarded);
    assert!(!w.b_open(&early[0]), "kapanıştan sonra epoch-0 reddedilir");
    // Yeniden açılmaz: sonraki paket ve zaman kabulü geri getirmez.
    w.epoch1_to_b();
    w.clock_b.advance(GRACE_T_G * 2);
    let _ = w.b.poll_event();
    assert_eq!(w.b_epoch().1, RecvEpoch0::Discarded);
    Ok(())
}

/// 5b — `N_g` sabittir: kurucuya verilen bir bütçe YOKTUR; grace tam `N_g` pakette kapanır.
pub fn cell_5b_n_g_is_fixed(kind: EnvelopeKind) -> Cell {
    let (mut w, _) = confirmed(kind, 0);
    let mut counted = 1;
    while w.b_grace_open() {
        w.epoch1_to_b();
        counted += 1;
        assert!(counted <= N_G, "N_g'de kapanmadı");
    }
    assert_eq!(counted, N_G, "onaylayan dahil tam N_g");
    Ok(())
}

/// 6 — `T_g` monotonic süre grace'i kapatır (onay anından); gecikmiş epoch-0 paketi reddedilir.
pub fn cell_6_time_closes_grace(kind: EnvelopeKind) -> Cell {
    let (mut w, early) = confirmed(kind, 1);
    w.clock_b.advance(GRACE_T_G - Duration::from_millis(1));
    let _ = w.b.poll_event();
    assert!(w.b_grace_open(), "T_g dolmadan açık");
    w.clock_b.advance(Duration::from_millis(1));
    assert!(!w.b_open(&early[0]), "T_g dolunca epoch-0 reddedilir");
    assert_eq!(w.b_epoch().1, RecvEpoch0::Discarded);
    Ok(())
}

/// 7 — hangisi önce: `N_g − 1` paket + `T_g`'den kısa süre → açık; `N_g`'inci paket → kapanır.
/// Ters sıra: az paket + `T_g` → kapanır.
pub fn cell_7_whichever_first(kind: EnvelopeKind) -> Cell {
    let (mut w, early) = confirmed(kind, 2);
    for _ in 2..N_G {
        w.epoch1_to_b();
    }
    w.clock_b.advance(GRACE_T_G - Duration::from_millis(1));
    assert!(
        w.b_open(&early[0]),
        "N_g − 1 paket + T_g'den kısa süre: grace açık"
    );
    w.epoch1_to_b();
    assert!(!w.b_open(&early[1]), "N_g'inci pakette kapanır");

    let (mut w2, early2) = confirmed(kind, 1);
    w2.epoch1_to_b();
    w2.clock_b.advance(GRACE_T_G);
    assert!(!w2.b_open(&early2[0]), "T_g: az pakete rağmen kapanır");
    Ok(())
}

/// 8 — TEK pencere (identity §15.5): epoch-1 anahtarıyla açılan paketin sayacı tek pencereye
/// commit edilir; başarısız deneme hiçbir şeyi değiştirmez (H-NO-MUTATE); grace içindeki epoch-0
/// paketi aynı pencerede kendi (daha düşük) sayacıyla kabul edilir, tekrarı reddedilir.
pub fn cell_8_commit_goes_to_the_single_window(kind: EnvelopeKind) -> Cell {
    let (mut w, early) = confirmed(kind, 1);
    let before = w.b.recv_expected_pn_for_test(w.hb).expect("pencere");
    let fp = w.b.state_fingerprint();
    let forged = tamper(&w.seal_a(b"sahte").0);
    assert!(!w.b_open(&forged));
    assert_eq!(
        w.b.state_fingerprint(),
        fp,
        "başarısız deneme state'i değiştirdi"
    );
    w.epoch1_to_b();
    assert_eq!(
        w.b.recv_expected_pn_for_test(w.hb),
        Some(before + 2),
        "sahte paketin tükettiği PN atlanır, tek pencere"
    );
    assert!(w.b_open(&early[0]), "grace içinde epoch-0, kendi sayacıyla");
    assert!(!w.b_open(&early[0]), "replay");
    assert_eq!(
        w.b.recv_expected_pn_for_test(w.hb),
        Some(before + 2),
        "eski sayaç pencereyi geri almaz"
    );
    Ok(())
}

/// 9 — grace içinde en fazla iki anahtar denemesi: legacy zarfta epoch telde YOK → epoch-1 önce,
/// sonra epoch-0 (≤ 2); extended zarfta epoch telde → hep 1. Grace kapanınca epoch-0 paketi düşer.
pub fn cell_9_at_most_two_key_trials(kind: EnvelopeKind) -> Cell {
    let (mut w, early) = confirmed(kind, 2);
    w.epoch1_to_b();
    let e1_trials = w.b.last_key_trials();
    assert!(w.b_open(&early[0]), "grace içinde epoch-0 açılır");
    let e0_trials = w.b.last_key_trials();
    let want = match kind {
        EnvelopeKind::Legacy => (1, 2),
        EnvelopeKind::Extended => (1, 1),
    };
    assert_eq!(
        (e1_trials, e0_trials),
        want,
        "anahtar denemesi (epoch-1 paketi, epoch-0 paketi)"
    );
    while w.b_grace_open() {
        w.epoch1_to_b();
    }
    assert!(!w.b_open(&early[1]), "grace kapanınca epoch-0 paketi düşer");
    Ok(())
}

/// `N_g` kodda adlandırılmış sabitten gelir ve spec'teki değere eşittir.
pub fn n_g_matches_spec() {
    const NAME: &str = "N_g = ";
    let spec = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../specs/kama-noise-wire-tdd-v2.0.md"
    ))
    .expect("noise-wire spec okunmalı");
    let values: Vec<u32> = spec
        .lines()
        .filter(|l| !l.contains("~~"))
        .flat_map(|l| {
            l.match_indices(NAME)
                .map(move |(i, _)| &l[i + NAME.len()..])
        })
        .filter_map(|rest| {
            let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
            digits.parse().ok()
        })
        .collect();
    assert!(!values.is_empty(), "spec'te sayısal N_g ataması bulunamadı");
    assert!(
        values.iter().all(|v| *v == N_G),
        "spec {values:?} ≠ GRACE_MIN_PACKETS {N_G}"
    );
}

/// Beyan listesi kendi içinde tutarlı ve her adı bir hücreye karşılık geliyor.
pub fn declared_red_is_consistent(known: &[&str]) {
    for (i, name) in DECLARED_RED.iter().enumerate() {
        assert!(!DECLARED_RED[..i].contains(name), "çift beyan: {name}");
        assert!(known.contains(name), "bilinmeyen hücre beyanı: {name}");
    }
}

/// Matrisi, verilen zarf kipinde hücre başına ayrı `#[test]` olarak sarar.
#[macro_export]
macro_rules! noise_epoch_suite {
    ($suite:ident, $kind:expr; $($cell:ident),+ $(,)?) => {
        mod $suite {
            $(
                #[test]
                fn $cell() {
                    $crate::epoch::check(stringify!($cell), $crate::epoch::$cell($kind));
                }
            )+

            #[test]
            fn n_g_matches_spec() {
                $crate::epoch::n_g_matches_spec();
            }

            #[test]
            fn declared_red_is_consistent() {
                $crate::epoch::declared_red_is_consistent(&[$(stringify!($cell)),+]);
            }
        }
    };
}
