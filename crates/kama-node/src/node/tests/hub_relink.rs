//! [HLR-STALE] Sahip şartı (#180), `kama-node` düzeyi: aynı eşte ESKİ hattın (`h1`) geç hatası,
//! `h2`'de çalışan yeni oturumu yenilemeye SOKMAZ; `h2`'nin kaybı tam bir yenileme başlatır.
//!
//! * `ROUTE_MISS{h1}` → `HubLink` eski hattı `known_links`'te tuttuğu için
//!   `HubNotice::HandleLost{peer, h1}` verir (`client-core` `hub::link` testi
//!   `hlr_stale_old_line_route_miss_names_the_old_handle`).
//! * Eski hatta ait `UNKNOWN_HANDLE` → `HubLink` bildirim üretmez ya da (eşin açık denemesi için)
//!   yalnız `HubNotice::AttemptEnded{peer}` verir (`hlr_stale_late_unknown_handle_of_the_old_line_
//!   gives_no_notice`). Bu test ikincisini, bileşim köküne ulaşabilen tek biçimi verir.
//!
//! **Belirlenimli düzenek:** iki elle sürülen düğüm (`test_support::manual`: ürün kurulum gövdesi,
//! süreç-içi röle — röle INIT başına bir hub handle'ı atar, transport onu oturumun hattı olarak
//! tutar). Hub bildirimleri `NodeLoop::on_hub_notices`'e (ürünün `drive_hub` gövdesi) elle verilir:
//! doğrudan kipte hub tarafı yoktur. Application saati ilerletilmez. Tele çıkan INIT, rölenin
//! gönderim kaydından sayılır (`carrier::envelope_counts_for_test`). Bekleyen koşul döngüleri
//! (`support::until`) yalnız koşulu yeniden denetler; sonuç süreye bağlı değildir.

use std::sync::Arc;
use std::time::Instant;

use client_core::hub::relink::RelinkStats;
use client_core::hub::HubNotice;
use client_core::transport::ActiveLink;
use kama_contracts::handles::PeerRef;
use kama_contracts::s3::HubHandle;

use super::{call, connect_manuals, peer_of, Command, NodeLoop};
use crate::event::NodeEvent;
use crate::test_support::{self as support, Events, ManualClock, TempDir};
use crate::Built;

const ALICE: &str = "alice";
const BOB: &str = "bob";
/// "Olmadı" iddialarından önce sürülen tur sayısı (iki uç; her turda her düğüm bir kez): röle
/// zarfı bir turda taşır, bir INIT çıksaydı bu turlarda görülürdü.
const QUIET_TURNS: usize = 8;
/// Yenilemeden sonra gönderilen metin (yeni oturumda iki uç arasında akış).
const AFTER_TEXT: &str = "yeni-hatta-metin";

struct Pair {
    alice: Built,
    bob: Built,
    bob_events: Events,
    /// Alice'in gözünden Bob.
    bob_peer: PeerRef,
    _root: TempDir,
}

/// Alice ↔ Bob kurulu; dönen değer + Alice'in oturumunun hattı (`h1`).
fn connected_pair(label: &str) -> (Pair, HubHandle) {
    let root = TempDir::new(label);
    let phones = support::provision(root.path(), &[ALICE, BOB]);
    let (mut alice, mut alice_events) =
        support::manual(&phones[ALICE], Arc::new(ManualClock::default()));
    let (mut bob, mut bob_events) = support::manual(&phones[BOB], Arc::new(ManualClock::default()));
    connect_manuals(
        &mut alice.node,
        &mut alice_events,
        ALICE,
        &mut bob.node,
        &mut bob_events,
        BOB,
    );
    let bob_peer = peer_of(&alice.node, BOB);
    let h1 = line(&alice.node, bob_peer);
    let pair = Pair {
        alice,
        bob,
        bob_events,
        bob_peer,
        _root: root,
    };
    (pair, h1)
}

/// Eşin etkin oturumunun hattı (bilinmeli: röle her INIT'e handle atar).
fn line(node: &NodeLoop, peer: PeerRef) -> HubHandle {
    match node.messenger.transport().active_link(peer) {
        ActiveLink::On(h) => h,
        other => panic!("etkin oturumun hattı bilinmeli: {other:?}"),
    }
}

/// Tele (röleye) çıkan INIT sayısı.
fn inits(node: &NodeLoop) -> u64 {
    crate::carrier::envelope_counts_for_test(node.peers.me()).inits_sent
}

/// `(etkin, bekleyen yenileme, onaysız provisional)`.
fn sessions(node: &NodeLoop, peer: PeerRef) -> (usize, usize, usize) {
    node.messenger.transport().renewal_state_for_test(peer)
}

fn lost(peer: PeerRef, h: HubHandle) -> HubNotice {
    HubNotice::HandleLost { peer, h }
}

impl Pair {
    fn turns(&mut self, n: usize) {
        for _ in 0..n {
            self.bob.node.turn();
            self.alice.node.turn();
        }
    }

    /// `h`'nin kaybı (Alice'in etkin hattı) → yöneticiden TEK yenileme, tele TEK INIT → iki uç
    /// turlanır: Alice yeni hatta keser ve Bob yeni oturumu onaylar (Alice'in yeni ucunun ilk
    /// epoch-1 paketi; iki uçta aynı hat, tek etkin oturum, artık yok). Yeni hattı döner.
    ///
    /// Bob'un onayı beklenir: bir sonraki yenileme Bob ilkini onaylamadan başlarsa Bob'daki ikinci
    /// provisional açılmaz (bu PR'ın kapsamı dışında kayda geçen ayrı bulgu); burada sınanan şey
    /// eski hattın geç hatasıdır, ardışık yenileme yarışı değil.
    fn renew_lost_line(&mut self, h: HubHandle) -> HubHandle {
        let peer = self.bob_peer;
        let a = &mut self.alice.node;
        assert_eq!(line(a, peer), h, "kaybolan hat etkin oturumun hattı");
        let (started, sent) = (a.relinks.stats().started, inits(a));
        a.on_hub_notices(Instant::now(), vec![lost(peer, h)]);
        assert_eq!(a.relinks.stats().started, started + 1, "yenileme başladı");
        assert_eq!(inits(a), sent + 1, "tele TEK INIT");
        let me = self.alice.node.peers.me();
        let new = support::until("Alice yeni hatta kesti, Bob onayladı", || {
            self.bob.node.turn();
            self.alice.node.turn();
            let (a, b) = (&self.alice.node, &self.bob.node);
            match (
                a.messenger.transport().active_link(peer),
                b.messenger.transport().active_link(me),
            ) {
                (ActiveLink::On(n), ActiveLink::On(m))
                    if n != h
                        && m == n
                        && sessions(a, peer) == (1, 0, 0)
                        && sessions(b, me) == (1, 0, 0) =>
                {
                    Some(n)
                }
                _ => None,
            }
        });
        assert_eq!(
            inits(&self.alice.node),
            sent + 1,
            "yenileme başka INIT çıkarmadı"
        );
        new
    }

    /// Eski hattın geç hatası (`notice`) yeni oturumu yenilemez: yenileme sayısı ve tele INIT aynı,
    /// Alice'in etkin oturumu `current` hattında, bekleyen yenileme yok. Dönen değer: bildirimden
    /// önceki ve sonraki yönetici sayaçları (düşen bildirim sayaçta görünür).
    fn stale_notice_changes_nothing(
        &mut self,
        notice: HubNotice,
        current: HubHandle,
    ) -> (RelinkStats, RelinkStats) {
        let peer = self.bob_peer;
        let a = &mut self.alice.node;
        let before = a.relinks.stats();
        let (started, sent) = (before.started, inits(a));
        a.on_hub_notices(Instant::now(), vec![notice]);
        self.turns(QUIET_TURNS);
        let a = &mut self.alice.node;
        // Yönetici bekleyen isteği tutmuyor: sonraki turlar da INIT çıkarmaz.
        a.on_hub_notices(Instant::now(), Vec::new());
        self.turns(QUIET_TURNS);
        let a = &self.alice.node;
        assert_eq!(
            a.relinks.stats().started,
            started,
            "{notice:?}: relink.started artmadı"
        );
        assert_eq!(inits(a), sent, "{notice:?}: tele INIT çıkmadı");
        assert_eq!(line(a, peer), current, "etkin oturum güncel hatta");
        assert_eq!(sessions(a, peer).0, 1, "tek etkin oturum");
        assert_eq!(sessions(a, peer).1, 0, "bekleyen yenileme yok");
        (before, a.relinks.stats())
    }

    /// Alice yeni oturumda metin gönderir, Bob alır (Bob'un provisional'ı onaylanır).
    fn text_flows(&mut self) {
        let peer = self.bob_peer;
        let msg = call(&mut self.alice.node, |reply| Command::SendText {
            peer,
            text: AFTER_TEXT.to_owned(),
            reply,
        })
        .expect("metin");
        support::until("bob: TextReceived(yeni hatta)", || {
            self.alice.node.turn();
            self.bob.node.turn();
            self.bob_events.take(|event| {
                matches!(
                    event,
                    NodeEvent::TextReceived { peer_id, msg_id, text }
                        if peer_id == ALICE && *msg_id == msg && text == AFTER_TEXT
                )
                .then_some(())
            })
        });
    }
}

/// `ROUTE_MISS{h1}` geç gelir (`HandleLost{bob, h1}`; Alice h2'de): relink.started artmaz, tele INIT
/// çıkmaz, etkin oturum h2'de, `stale_handle_lost` +1 (istek kaydedilmez). Ardından `ROUTE_MISS{h2}`
/// tam bir yenileme başlatır. ESKİ DAVRANIŞ: `HandleLost{bob, h1}` → `relink(bob)` → h2'deki çalışan
/// oturum için ikinci INIT.
#[test]
fn late_route_miss_of_the_old_line_does_not_renew_the_new_session() {
    let (mut p, h1) = connected_pair("hlr-stale-route-miss");
    let h2 = p.renew_lost_line(h1);
    assert_ne!(h2, h1);
    let (before, after) = p.stale_notice_changes_nothing(lost(p.bob_peer, h1), h2);
    assert_eq!(
        after.stale_handle_lost,
        before.stale_handle_lost + 1,
        "düşen bildirim sayıldı"
    );
    assert_eq!(after.requests, before.requests, "istek kaydedilmedi");
    let h3 = p.renew_lost_line(h2);
    assert_ne!(h3, h2);
    p.text_flows();
}

/// Eski hatta ait geç `UNKNOWN_HANDLE` (bileşim köküne yalnız `AttemptEnded{bob}` olarak ulaşır;
/// Alice h2'de, bekleyen yenileme yok): relink.started artmaz, tele INIT çıkmaz; `restart_renewal`
/// yeniden başlatacak yenileme bulmaz, `nothing` +1. Ardından `ROUTE_MISS{h2}` tam bir yenileme
/// başlatır.
#[test]
fn late_unknown_handle_of_the_old_line_does_not_renew_the_new_session() {
    let (mut p, h1) = connected_pair("hlr-stale-unknown-handle");
    let h2 = p.renew_lost_line(h1);
    assert_ne!(h2, h1);
    let (before, after) =
        p.stale_notice_changes_nothing(HubNotice::AttemptEnded { peer: p.bob_peer }, h2);
    assert_eq!(
        after.nothing,
        before.nothing + 1,
        "iş çıkmayan deneme sayıldı"
    );
    assert_eq!(after.stale_handle_lost, before.stale_handle_lost);
    let h3 = p.renew_lost_line(h2);
    assert_ne!(h3, h2);
    p.text_flows();
}
