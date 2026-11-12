//! [T3-GLARE] Bileşim kökü düzeyi (#180 takibi; iptal: S4 C3 "bağlantı niyeti iptal edildi";
//! glare kaybı: C3 listesinde adıyla yok, identity §16 glare kuralından türetilir): düğüm
//! komutları (`Connect`, `CancelConnect`) Messenger → Transport yolundan geçer; niyetin terk edilen
//! INIT'inin taşıyıcı tekrarı durdurulur (`EnvelopeLink::cancel_init`).
//!
//! **Belirlenimli düzenek:** iki elle sürülen düğüm, ürün kurulum gövdesi
//! (`KamaNode::build_with_link`); zarf taşıyıcısı `client_core::memory_link::MemoryHub` — zarflar
//! YALNIZ `relay()` çağrısında taşınır (glare sırası testin elindedir) ve terk bildirimleri
//! kaydedilir (`cancelled_inits`; denemeler gönderim sırasıyla 1'den numaralanır). `MemoryHub` T3
//! yapmaz: tel üzerindeki T3 sayımı gerçek `HubLink` bileşimiyle `client-core` `hub::link`
//! testlerindedir. Application saati ilerletilmez; bekleyen koşul döngüleri (`support::until`)
//! yalnız koşulu yeniden denetler.
//!
//! Glare'i kimin kazanacağı çalışma anında `dest_hash` karşılaştırmasıyla belirlenir (provisioning
//! rastgele; S3 K10 / identity §16: küçük olan kazanır).

use std::sync::Arc;
use std::time::Instant;

use client_core::hub::HubNotice;
use client_core::memory_link::MemoryHub;
use client_core::transport::{ActiveLink, LinkAttempt};
use kama_contracts::handles::PeerRef;
use kama_contracts::s1::AbandonReason;
use kama_contracts::s3::HubHandle;

use super::{call, connected, peer_of, Command, NodeLoop};
use crate::event::{CloseCause, NodeEvent};
use crate::test_support::{self as support, Events, ManualClock, TempDir};
use crate::Built;

const ALICE: &str = "alice";
const BOB: &str = "bob";

struct Side {
    name: &'static str,
    built: Built,
    events: Events,
}

struct Pair {
    hub: MemoryHub,
    sides: [Side; 2],
    _root: TempDir,
}

fn pair(label: &str) -> Pair {
    let root = TempDir::new(label);
    let phones = support::provision(root.path(), &[ALICE, BOB]);
    let hub = MemoryHub::new();
    let side = |name: &'static str| {
        let (built, events) =
            support::manual_with_link(&phones[name], Arc::new(ManualClock::default()), |me| {
                Box::new(hub.link(me))
            });
        Side {
            name,
            built,
            events,
        }
    };
    let sides = [side(ALICE), side(BOB)];
    Pair {
        hub,
        sides,
        _root: root,
    }
}

fn node(p: &mut Pair, i: usize) -> &mut NodeLoop {
    &mut p.sides[i].built.node
}

/// `i`'nin gözünden öteki uç.
fn other(p: &Pair, i: usize) -> PeerRef {
    peer_of(&p.sides[i].built.node, p.sides[1 - i].name)
}

fn connect(p: &mut Pair, i: usize) {
    let peer = other(p, i);
    call(node(p, i), |reply| Command::Connect { peer, reply }).expect("connect");
}

/// Röle + iki düğüm turu, ikisi de `Connected` görene dek.
fn until_both_connected(p: &mut Pair) {
    let (mut up, names) = ([false; 2], [p.sides[0].name, p.sides[1].name]);
    support::until("iki uç Connected", || {
        p.hub.relay();
        for (i, s) in p.sides.iter_mut().enumerate() {
            s.built.node.turn();
            up[i] |= s.events.take(connected(names[1 - i])).is_some();
        }
        (up[0] && up[1]).then_some(())
    });
}

/// `(etkin, bekleyen yenileme, onaysız provisional)`.
fn sessions(p: &Pair, i: usize) -> (usize, usize, usize) {
    p.sides[i]
        .built
        .node
        .messenger
        .transport()
        .renewal_state_for_test(other(p, i))
}

fn line(p: &Pair, i: usize) -> HubHandle {
    match p.sides[i]
        .built
        .node
        .messenger
        .transport()
        .active_link(other(p, i))
    {
        ActiveLink::On(h) => h,
        got => panic!("etkin oturumun hattı bilinmeli: {got:?}"),
    }
}

/// `from` metin gönderir, öteki uç alır (röle + iki tur).
fn text_flows(p: &mut Pair, from: usize, text: &str) {
    let peer = other(p, from);
    let msg = call(node(p, from), |reply| Command::SendText {
        peer,
        text: text.to_owned(),
        reply,
    })
    .expect("metin");
    let sender = p.sides[from].name;
    support::until(&format!("TextReceived({text})"), || {
        p.hub.relay();
        for s in &mut p.sides {
            s.built.node.turn();
        }
        p.sides[1 - from].events.take(|event| {
            matches!(
                event,
                NodeEvent::TextReceived { peer_id, msg_id, text: got }
                    if peer_id == sender && *msg_id == msg && got == text
            )
            .then_some(())
        })
    });
}

/// İki düğüm aynı anda bağlanır: iki `Connect` de röleye INIT verir, hiçbiri teslim edilmeden
/// (gerçek glare; Alice'in denemesi 1, Bob'unki 2). Dönen değer: kaybedenin sırası.
fn simultaneous_connect(p: &mut Pair) -> usize {
    connect(p, 0);
    connect(p, 1);
    until_both_connected(p);
    let (a, b) = (
        p.sides[0].built.node.peers.me(),
        p.sides[1].built.node.peers.me(),
    );
    usize::from(a < b)
}

/// (a) + (d) Glare'i kaybeden düğümün niyet INIT'inin taşıyıcı tekrarı durur; kazananınki terk
/// edilmez. İki uç tek oturumda, iki yönde metin.
///
/// ESKİ DAVRANIŞ: niyet glare'le çözülüyor, deneme terk edilmiyordu (`cancelled_inits` boş).
#[test]
fn glare_loser_node_cancels_its_intent_init_and_both_nodes_connect() {
    let mut p = pair("t3g-node-glare");
    let loser = simultaneous_connect(&mut p);
    let attempt = LinkAttempt(u64::try_from(loser).expect("sıra") + 1);
    assert_eq!(
        p.hub.cancelled_inits(),
        vec![attempt],
        "yalnız kaybedenin ({}) INIT tekrarı durdu",
        p.sides[loser].name
    );
    text_flows(&mut p, 0, "glare: a->b");
    text_flows(&mut p, 1, "glare: b->a");
    assert_eq!(sessions(&p, 0), (1, 0, 0));
    assert_eq!(sessions(&p, 1), (1, 0, 0));
}

/// (b) `CancelConnect` komutu (Messenger → `KamaTransport::cancel`): INIT'in taşıyıcı denemesi
/// terk edilir — handle atandıktan sonra da (T3'ün `StoreResp OK`'tan sonra sürdüğü durum). Düğüm
/// niyetin bittiğini `Closed { Abandoned(Cancelled) }` ile bildirir.
///
/// ESKİ DAVRANIŞ: `cancelled_inits` boş — `HubLink` üstünde INIT `ATTEMPT_MAX_AGE`'e dek T3.
#[test]
fn cancel_connect_command_stops_the_intent_init() {
    let mut p = pair("t3g-node-cancel");
    connect(&mut p, 0);
    assert_eq!(p.hub.relay(), 1, "INIT → Bob (Bob turlanmaz, yanıt yok)");
    node(&mut p, 0).turn();
    let peer = other(&p, 0);
    let cancelled = call(node(&mut p, 0), |reply| Command::CancelConnect {
        peer,
        reply,
    })
    .expect("cancel_connect");
    assert!(cancelled, "bekleyen niyet vardı");
    assert_eq!(p.hub.cancelled_inits(), vec![LinkAttempt(1)]);
    let bob = p.sides[1].name;
    let Side { built, events, .. } = &mut p.sides[0];
    support::until("Closed { Abandoned(Cancelled) }", || {
        built.node.turn();
        events.take(|event| {
            matches!(
                event,
                NodeEvent::Closed {
                    peer_id,
                    reason: CloseCause::Abandoned(AbandonReason::Cancelled)
                } if peer_id == bob
            )
            .then_some(())
        })
    });
}

/// (e) DR-CONNECT-1 / INV-NO-IDLE-TEARDOWN gerilemesi yok: glare'le çözülen niyetin oturumu
/// SONRADAN hattını kaybeder (S4 C5 `HandleLost`; bileşim kökü `on_hub_notices` → yönetici →
/// `relink_line`) → yenileme INIT'i yola çıkar ve terk EDİLMEZ; iki uç yeni hatta, tek oturum, iki
/// yönde metin.
#[test]
fn glare_resolved_node_session_is_renewed_when_its_line_is_lost() {
    let mut p = pair("t3g-node-relink");
    let loser = simultaneous_connect(&mut p);
    text_flows(&mut p, 0, "glare: a->b");
    let cancelled = p.hub.cancelled_inits();
    assert_eq!(cancelled.len(), 1, "glare'deki terk");

    let h = line(&p, loser);
    let peer = other(&p, loser);
    let n = node(&mut p, loser);
    let started = n.relinks.stats().started;
    n.on_hub_notices(Instant::now(), vec![HubNotice::HandleLost { peer, h }]);
    assert_eq!(n.relinks.stats().started, started + 1, "yenileme başladı");
    let winner = 1 - loser;
    support::until("iki uç yeni hatta, tek oturum", || {
        p.hub.relay();
        for s in &mut p.sides {
            s.built.node.turn();
        }
        let (l, w) = (line(&p, loser), line(&p, winner));
        (l != h && l == w && sessions(&p, loser) == (1, 0, 0) && sessions(&p, winner) == (1, 0, 0))
            .then_some(())
    });
    text_flows(&mut p, loser, "yeni hat: kaybeden");
    text_flows(&mut p, winner, "yeni hat: kazanan");
    assert_eq!(p.hub.cancelled_inits(), cancelled, "yenileme terk edilmedi");
}
