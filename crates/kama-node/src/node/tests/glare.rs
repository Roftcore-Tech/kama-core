//! Eşzamanlı connect (glare) — DR-CONNECT-1 karar #4 ve K2, identity §16 "Eşzamanlı INIT — glare":
//! `dest_hash`'i küçük olan kazanır; kazanan gelen INIT'i sessiz düşer (RESPONSE yok), kaybeden
//! bekleyen INIT'ini bırakıp gelen INIT'i işler; tek oturum doğar; kaybedenin `early_unit`'ini
//! Transport saklar ve kazanan oturum `Ready` olunca ilk normal `send()` olarak yollar.
//!
//! **Belirlenimli düzenek:** iki elle sürülen düğüm (`test_support::manual`: ürün kurulum gövdesi
//! ve süreç-içi röle). İki `connect` — erken metinleriyle (`Messenger::connect_with_text`; düğüm
//! API'sinde erken metin yoktur, motor doğrudan çağrılır ve bileti düğüm kaydına
//! `NodeLoop::connect` gibi yazılır) — HİÇBİR düğüm kutusunu okumadan uygulanır: iki taraf da
//! bekleyen INIT taşır (gerçek glare). Ancak sonra turlar sürülür. Saat İLERLETİLMEZ
//! (`ManualClock`): çözüm bir zamanlayıcıyla gelemez — `HALF_OPEN_TTL` yok, iptal yok.
//!
//! **"≤ 2 RTT" yapısal ölçülür:** loopback'te duvar saati anlamsızdır (zarflar süreç-içi). Tur
//! turları sayılır (her turda her düğüm bir kez): kaybeden İLK turunda (kazananın INIT'ini
//! yanıtlayarak), kazanan kaybedenin ilk turundan sonraki İLK turunda (tek RESPONSE'u işleyerek)
//! bağlanır — iki INIT çaprazlanır, tek zarf gidiş-dönüşü. Rölede RESPONSE adedi doğrudan
//! sayılmaz (röleye gözlem kancası yok); yerine yapısal karşılığı sınanır: her yanıt bir
//! yanıtlayan oturumu doğurur — kaybedenin oturumu yanıtlayan (bağlandığında gönderim epoch'u 0),
//! kazananınki başlatan (epoch 1) ve her iki tarafta da TEK oturum tutamacı görülür.
//!
//! İki sıralama koşulur: INIT'ini İLK veren ve turda İLK sürülen kazanan / kaybeden. Roller
//! çalışma anında `dest_hash` karşılaştırmasıyla belirlenir (provisioning rastgele).

use std::collections::BTreeSet;
use std::sync::Arc;

use kama_app::messaging::{ConnectState, MsgId};
use kama_contracts::handles::{PeerRef, SessionHandle};
use kama_contracts::s1::ConnectResult;

use super::{call, peer_state, Command};
use crate::event::{LinkState, NodeEvent};
use crate::test_support::{self as support, Events, ManualClock, Phone, TempDir};
use crate::Built;

const ALICE: &str = "alice";
const BOB: &str = "bob";

/// El sıkışmanın tur tavanı (her turda her düğüm bir kez sürülür): iki INIT çaprazlanır, kaybeden
/// yanıtlar, kazanan yanıtı işler — kazanan önce sürülürse ikinci tura sarkar.
const HANDSHAKE_ROUNDS: usize = 2;
/// Sıralamadaki iki taraf (0: INIT'ini önce veren ve turda önce sürülen).
const SIDES: usize = 2;

/// Bir taraf: elle sürülen düğüm + gözlemleri.
struct Side {
    name: &'static str,
    built: Built,
    events: Events,
    /// Düğüm olayları, geliş sırasıyla (`Metrics` hariç).
    log: Vec<NodeEvent>,
    /// Messenger'ın eş için benimsediği oturum tutamaçları (her turdan sonra örneklenir).
    sessions: BTreeSet<SessionHandle>,
}

impl Side {
    fn new(name: &'static str, phone: &Phone) -> Self {
        // Application saati elle ilerler ve HİÇ ilerletilmez.
        let (built, events) = support::manual(phone, Arc::new(ManualClock::default()));
        Self {
            name,
            built,
            events,
            log: Vec::new(),
            sessions: BTreeSet::new(),
        }
    }

    fn me(&self) -> PeerRef {
        self.built.node.peers.me()
    }

    /// Bir düğüm turu + gözlem.
    fn turn(&mut self, peer: PeerRef) {
        self.built.node.turn();
        self.log.extend(self.events.drain_all());
        if let Some(session) = self.built.node.messenger.session_of(peer) {
            self.sessions.insert(session);
        }
    }

    fn count(&self, pick: impl Fn(&NodeEvent) -> bool) -> usize {
        self.log.iter().filter(|&event| pick(event)).count()
    }

    fn connected_count(&self, peer_id: &str) -> usize {
        self.count(|event| matches!(event, NodeEvent::Connected { peer_id: got } if got == peer_id))
    }

    fn text_count(&self, peer_id: &str, msg_id: MsgId, text: &str) -> usize {
        self.count(|event| {
            matches!(
                event,
                NodeEvent::TextReceived { peer_id: got, msg_id: id, text: body }
                    if got == peer_id && *id == msg_id && body == text
            )
        })
    }

    fn receipt_count(&self, peer_id: &str, msg_id: MsgId) -> usize {
        self.count(|event| {
            matches!(
                event,
                NodeEvent::Receipt { peer_id: got, msg_id: id } if got == peer_id && *id == msg_id
            )
        })
    }
}

fn early_text(name: &str) -> String {
    format!("{name}: erken metin")
}

fn later_text(name: &str) -> String {
    format!("{name}: bağlandıktan sonra")
}

/// Erken metinli `connect` (S1 K6) — tur YOK. Bileti `NodeLoop::connect` gibi düğüm kaydına
/// yazar: niyet `ConnectAbandoned` ile biterse düğüm onu `Closed { Abandoned }` olarak verir.
fn connect_with_early_text(side: &mut Side, peer: PeerRef) -> MsgId {
    let text = early_text(side.name);
    let node = &mut side.built.node;
    let (result, msg_id) = node
        .messenger
        .connect_with_text(peer, &text)
        .expect("connect_with_text");
    match result {
        ConnectResult::Pending(ticket) => {
            node.tickets.insert(ticket, peer);
        }
        ConnectResult::Ready(session) => {
            panic!("glare düzeneğinde hazır oturum olamaz: {session:?}")
        }
    }
    msg_id
}

/// Bir koşu: `first_mover_wins` doğruysa INIT'ini röleye İLK veren ve turda İLK sürülen kazanandır.
fn simultaneous_connect(first_mover_wins: bool) {
    let root = TempDir::new("glare");
    let phones = support::provision(root.path(), &[ALICE, BOB]);
    let alice = Side::new(ALICE, &phones[ALICE]);
    let bob = Side::new(BOB, &phones[BOB]);
    // DR-CONNECT-1 #4: `dest_hash`'i küçük olan kazanır (`PeerRef` = `dest_hash` baytları,
    // sözlük sırası — Crypto'nun karşılaştırmasıyla aynı).
    let (winner, loser) = if alice.me() < bob.me() {
        (alice, bob)
    } else {
        (bob, alice)
    };
    let mut sides = if first_mover_wins {
        [winner, loser]
    } else {
        [loser, winner]
    };
    let w = usize::from(!first_mover_wins);
    let l = SIDES - 1 - w;
    let ids = [sides[0].me(), sides[1].me()];
    let names = [sides[0].name, sides[1].name];
    let order = format!(
        "kazanan {} / kaybeden {} (önce davranan: {})",
        names[w], names[l], names[0]
    );

    // Faz 0 — iki connect, hiçbir kutu okunmadan: iki taraf da bekleyen INIT taşır.
    let early = [
        connect_with_early_text(&mut sides[0], ids[1]),
        connect_with_early_text(&mut sides[1], ids[0]),
    ];

    // El sıkışma — yalnız turlar; saat ilerlemez.
    let mut connected_at = [None; SIDES];
    let mut epoch_at_connect = [None; SIDES];
    let mut step = 0;
    for _ in 0..HANDSHAKE_ROUNDS {
        for (i, side) in sides.iter_mut().enumerate() {
            step += 1;
            let peer = 1 - i;
            side.turn(ids[peer]);
            if connected_at[i].is_none() && side.connected_count(names[peer]) > 0 {
                connected_at[i] = Some(step);
                epoch_at_connect[i] = Some(peer_state(&side.built.node.snapshot(), names[peer]));
            }
        }
    }
    // Adım numarası: tur r (0'dan), taraf i → 2r + i + 1.
    let loser_first_turn = l + 1;
    let winner_turn_after_it = if w > l { w + 1 } else { w + 1 + SIDES };
    assert_eq!(
        connected_at[l],
        Some(loser_first_turn),
        "{order}: kaybeden ilk turunda kazananın INIT'ini yanıtlayarak bağlanır"
    );
    assert_eq!(
        connected_at[w],
        Some(winner_turn_after_it),
        "{order}: kazanan tek RESPONSE'u kaybedenin ilk turundan sonraki ilk turunda işler"
    );
    assert_eq!(
        epoch_at_connect[l],
        Some((LinkState::Connected, Some(0))),
        "{order}: kaybedenin oturumu YANITLAYAN oturumudur (kendi INIT'inden oturum doğmadı)"
    );
    assert_eq!(
        epoch_at_connect[w],
        Some((LinkState::Connected, Some(1))),
        "{order}: kazananın oturumu kendi INIT'inin BAŞLATAN oturumudur"
    );

    // Bağlandıktan sonra iki yönde birer metin.
    let mut later = [0; SIDES];
    for (i, side) in sides.iter_mut().enumerate() {
        let peer = ids[1 - i];
        let text = later_text(side.name);
        later[i] = call(&mut side.built.node, |reply| Command::SendText {
            peer,
            text,
            reply,
        })
        .expect("send_text");
    }
    let delivered = |sides: &[Side; SIDES]| {
        (0..SIDES).all(|i| {
            let j = 1 - i;
            sides[j].text_count(names[i], early[i], &early_text(names[i])) > 0
                && sides[j].text_count(names[i], later[i], &later_text(names[i])) > 0
                && sides[i].receipt_count(names[j], early[i]) > 0
                && sides[i].receipt_count(names[j], later[i]) > 0
        })
    };
    support::until(
        &format!("{order}: iki yönde metinler ve alındıları"),
        || {
            for (i, side) in sides.iter_mut().enumerate() {
                side.turn(ids[1 - i]);
            }
            delivered(&sides).then_some(())
        },
    );
    // Durgunluk: uçlarda teyidi bekleyen birim kalmaz (geç bir tekrar sayımı bozamaz).
    support::until(&format!("{order}: durgunluk"), || {
        for (i, side) in sides.iter_mut().enumerate() {
            side.turn(ids[1 - i]);
        }
        sides
            .iter()
            .all(|side| side.built.node.messenger.transport().outstanding_for_test() == 0)
            .then_some(())
    });

    for i in 0..SIDES {
        let j = 1 - i;
        let side = &sides[i];
        let node = &side.built.node;
        let peer = ids[j];
        assert_eq!(
            side.connected_count(names[j]),
            1,
            "{order}: {} eşine tam bir Connected: {:?}",
            side.name,
            side.log
        );
        assert_eq!(
            side.count(|event| matches!(event, NodeEvent::Closed { .. })),
            0,
            "{order}: {} — Closed / ConnectAbandoned yok: {:?}",
            side.name,
            side.log
        );
        assert_eq!(
            side.sessions.len(),
            1,
            "{order}: {} tek oturum tutamacı gördü: {:?}",
            side.name,
            side.sessions
        );
        let session = node.messenger.session_of(peer);
        assert_eq!(
            node.messenger.transport().active_session_for_test(peer),
            session,
            "{order}: {} — Transport'un etkin oturumu Messenger'ınkiyle aynı",
            side.name
        );
        assert!(
            matches!(node.messenger.connect_state(peer), ConnectState::Ready(_)),
            "{order}: {} — bekleyen niyet kalmadı",
            side.name
        );
        assert!(
            node.tickets.is_empty(),
            "{order}: {} — bilet kalmadı: {:?}",
            side.name,
            node.tickets
        );
        // Erken metin ve sonraki metin karşıda TAM BİR KEZ; alındıları gönderende tam bir kez.
        assert_eq!(
            sides[j].text_count(names[i], early[i], &early_text(names[i])),
            1,
            "{order}: {}'nin erken metni tek oturumdan tam bir kez",
            names[i]
        );
        assert_eq!(
            sides[j].text_count(names[i], later[i], &later_text(names[i])),
            1,
            "{order}: {}'nin metni tam bir kez",
            names[i]
        );
        assert_eq!(side.receipt_count(names[j], early[i]), 1);
        assert_eq!(side.receipt_count(names[j], later[i]), 1);
    }
}

/// INIT'ini önce veren ve turda önce sürülen KAZANAN.
#[test]
fn simultaneous_connect_resolves_to_one_session_when_the_winner_moves_first() {
    let _serial = support::serial();
    simultaneous_connect(true);
}

/// INIT'ini önce veren ve turda önce sürülen KAYBEDEN.
#[test]
fn simultaneous_connect_resolves_to_one_session_when_the_loser_moves_first() {
    let _serial = support::serial();
    simultaneous_connect(false);
}
