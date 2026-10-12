//! Zarf taşıyıcısı — INIT/RESPONSE zarflarını (SSI1/SSR1) eşe götüren
//! `client_core::transport::EnvelopeLink`: [`LoopbackEnvelopeRelay`], süreç-içi zarf rölesi.
//!
//! ürün taşıyıcısı değildir — iki cihaz arası taşıyıcı = hub Plane 1 rölesi, S4, hub sprinti
//!
//! # DUR-K4G-11 kararı (Abkarada, CEVAP PAKETİ 3, 2026-10-12): Seçenek B
//!
//! Bu sprintte zarf taşıyıcısı süreç-içi röledir: AYNI süreçteki düğümler (CORE testi, INT-02)
//! zarflarını bununla değiş tokuş eder; iki telefon arası oturum hub sprintiyle gelir. (A) ürün
//! `DirectUdpLink` yeni bir tel çerçevesi isterdi ve INIT'in yeniden gönderimi (T3) yokken yüksek
//! kayıpta (%30) bağlanma kurulamazdı; (C) veri soketinde yeni çerçeve bağlam-önce dağıtımı
//! delerdi — ikisi reddedildi. **INT-02 beyanlı boşluk:** veri datagramları UDP loopback +
//! netem'den geçer; el sıkışma zarfları süreç-içi röleden netem DIŞINDA geçer.
//!
//! # Tasarım
//!
//! * **Kayıt** — süreç-genel TEK tablo (`static`): cihaz kimliği (`PeerRef`; `dest_hash`'ten
//!   türer, süreç içinde küresel tutarlı) → kutu. [`product_envelope_link`] kaydeder; aynı kimlik
//!   canlıyken ikinci kayıt [`NodeError::IdentityInUse`] (sessiz devralma YOK). Kaydın sahibi uç
//!   noktadır: uç nokta düşünce kayıt silinir (Drop panik etmez; zehirli kilit
//!   `PoisonError::into_inner` ile okunur).
//! * **Jeton** — her kayıt kendine özgü bir jeton taşır; uç nokta yalnız KENDİ jetonlu kaydına
//!   dokunur (gönderim, yoklama, silme). Eski bir uç noktanın geç Drop'u, aynı kimliğin YENİ
//!   kaydını silemez; kaydı düşmüş eski uç nokta hiçbir şey gönderemez ve okuyamaz.
//! * **Tutamaç** — `send_init(peer)`: `peer` kayıtlıysa röle süreç-tekil bir `HubHandle` atar
//!   (1'den başlar, 0 verilmez, sarma YOK: tükenirse `Err(LinkError)`); başlatanın kutusuna
//!   `HandleAssigned { attempt, hub_handle }`, alıcınınkine AYNI tutamaçla `IncomingInit` girer —
//!   direct-mode'da `client_core::memory_link::MemoryHub::relay` ile aynı anlam (iki yön aynı
//!   tutamacı kullanır). `send_response(peer, hub_handle)` yalnız `peer`'in kutusuna `Response`
//!   koyar.
//! * **Kayıtsız eş** — `Err(LinkError)`; HİÇBİR şey kuyrukta tutulmaz (bekletme = store-and-forward
//!   = plan §4.5 YASAK). Sonucu: transport bu hatayı yok sayar (`client-core` `transport.rs`:
//!   `if let Ok(a) = self.link.send_init(..)` — `connect`, `reinitiate`, yenileme), niyet bekler;
//!   INIT'in yeniden gönderimi (T3) YOKTUR. Koddan doğrulanan taze INIT yolları: (1) Crypto
//!   bekleyen INIT'i `HALF_OPEN_TTL` (7 gün) dolunca düşürür (`Closed(KeyHygiene)`) ve transport
//!   niyeti yeni bir INIT'le sürdürür (`reinitiate`); (2) uygulama niyeti iptal edip
//!   (`cancel_connect`) yeniden `connect` eder. Eşin kendi INIT'iyle (glare) niyetin çözülmesi
//!   burada doğrulanmadı. Bu yüzden düğümler bağlanmadan ÖNCE başlatılmış olmalıdır.
//! * **Tavan** — her kutu [`MAILBOX_CAP`] olayla sınırlı (SR-AUDIT-001). Taşmada gönderim
//!   `Err(LinkError)` döner ve hiçbir şey kuyruğa girmez; INIT'te İKİ kutu (başlatanınki ve
//!   alıcınınki) da önceden denetlenir.
//! * **Opaklık** — zarf baytları ve türü OPAKTIR (S3 K6): röle onları değiştirmez, ayrıştırmaz;
//!   kutuda yalnız taşınan olayın içinde durur. Röle protokol mantığı içermez: yalnız yönlendirme
//!   ve tutamaç ataması.

use std::collections::{BTreeMap, VecDeque};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use client_core::transport::{EnvelopeLink, LinkAttempt, LinkError, LinkEvent};
use kama_contracts::handles::PeerRef;
use kama_contracts::s3::HubHandle;

use crate::error::NodeError;

/// Kutu başına okunmamış zarf olayı tavanı — yerel bellek sınırı, normatif DEĞİL `[KALİBRASYON]`
/// (SR-AUDIT-001). Meşru yük küçüktür: kutu sahibinin kendi INIT'lerinin `HandleAssigned` ve
/// `Response`'u (bekleyen niyet tavanı `client_core::transport::PENDING_INIT_CAP` + yenilemeler)
/// ile eşlerinden gelen INIT'ler (provisioning ≤ 63 eş); kutu her düğüm turunda boşaltılır. Tavan,
/// olaylarını okumayan (takılmış) bir uç noktanın kutusunun sınırsız büyümesini keser. Taşmada
/// davranış: gönderim `Err(LinkError)`, kuyruğa bir şey girmez (sayaç tutulmaz).
pub(crate) const MAILBOX_CAP: usize = 1024;

/// Röle atadığı ilk `HubHandle` değeri (0 verilmez).
const FIRST_HUB_HANDLE: u32 = 1;

/// Uç noktanın verdiği ilk `LinkAttempt` değeri.
const FIRST_ATTEMPT: u64 = 1;

/// Süreç-genel röle tablosu (bkz. modül belgesi).
static RELAY: Mutex<Relay> = Mutex::new(Relay::new());

/// Zehirlenmeye karşı panik yapmaz: tablonun her değişikliği tek bir kilitli adımda tamamlanır.
fn relay() -> MutexGuard<'static, Relay> {
    RELAY.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Kayıtlı uç noktaların kutuları ve süreç-tekil tutamaç sayacı.
struct Relay {
    mailboxes: BTreeMap<PeerRef, Mailbox>,
    /// Sıradaki atanacak `HubHandle` değeri.
    next_handle: u32,
}

/// Bir kaydın kutusu: sahibinin jetonu + okunmamış olaylar (≤ [`MAILBOX_CAP`]).
struct Mailbox {
    owner: Arc<Token>,
    events: VecDeque<LinkEvent>,
}

/// Kayıt jetonu: kimliği yalnız adresidir (`Arc::ptr_eq`); canlı iki jeton aynı adresi paylaşamaz.
struct Token;

impl Relay {
    const fn new() -> Self {
        Self {
            mailboxes: BTreeMap::new(),
            next_handle: FIRST_HUB_HANDLE,
        }
    }

    /// `me`'nin kutusu, YALNIZ jetonu `owner` ise.
    fn owned_mut(&mut self, me: PeerRef, owner: &Arc<Token>) -> Option<&mut Mailbox> {
        self.mailboxes
            .get_mut(&me)
            .filter(|mailbox| Arc::ptr_eq(&mailbox.owner, owner))
    }

    /// `peer`'in kutusunda yer var mı (kayıtsız eş → `false`).
    fn has_room(&self, peer: PeerRef) -> bool {
        self.mailboxes
            .get(&peer)
            .is_some_and(|mailbox| mailbox.events.len() < MAILBOX_CAP)
    }

    /// Kayıtlı ve yeri olan `peer`'in kutusuna olay koyar; denetim çağırandadır.
    fn deliver(&mut self, peer: PeerRef, event: LinkEvent) {
        if let Some(mailbox) = self.mailboxes.get_mut(&peer) {
            mailbox.events.push_back(event);
        }
    }
}

/// Süreç-içi zarf rölesinin bir uç noktası (bkz. modül belgesi).
///
/// ürün taşıyıcısı değildir — iki cihaz arası taşıyıcı = hub Plane 1 rölesi, S4, hub sprinti
pub(crate) struct LoopbackEnvelopeRelay {
    /// Bu cihazın kimliği.
    me: PeerRef,
    /// Bu uç noktanın kaydının jetonu.
    owner: Arc<Token>,
    /// Sıradaki `LinkAttempt` değeri (uç nokta başına; transport onu yalnız kendi INIT'leriyle
    /// eşler).
    next_attempt: u64,
}

impl LoopbackEnvelopeRelay {
    /// `me`'yi kaydeder.
    ///
    /// # Errors
    /// [`NodeError::IdentityInUse`] — bu süreçte `me` kimliğiyle canlı bir uç nokta var.
    fn register(me: PeerRef) -> Result<Self, NodeError> {
        let mut relay = relay();
        if relay.mailboxes.contains_key(&me) {
            return Err(NodeError::IdentityInUse);
        }
        let owner = Arc::new(Token);
        relay.mailboxes.insert(
            me,
            Mailbox {
                owner: Arc::clone(&owner),
                events: VecDeque::new(),
            },
        );
        Ok(Self {
            me,
            owner,
            next_attempt: FIRST_ATTEMPT,
        })
    }
}

impl EnvelopeLink for LoopbackEnvelopeRelay {
    fn send_init(
        &mut self,
        peer: PeerRef,
        envelope_type: u8,
        bytes: Vec<u8>,
    ) -> Result<LinkAttempt, LinkError> {
        if peer == self.me {
            // Kendine INIT'in yolu yoktur (eş dizini bu cihazı içermez).
            return Err(LinkError);
        }
        let mut relay = relay();
        // Kendi kaydı (jeton), iki kutuda da yer, sayaçların ardılı — HEPSİ değişiklikten ÖNCE.
        let own_room = relay
            .owned_mut(self.me, &self.owner)
            .is_some_and(|mailbox| mailbox.events.len() < MAILBOX_CAP);
        if !own_room || !relay.has_room(peer) {
            return Err(LinkError);
        }
        let next_attempt = self.next_attempt.checked_add(1).ok_or(LinkError)?;
        let raw_handle = relay.next_handle;
        let next_handle = raw_handle.checked_add(1).ok_or(LinkError)?;
        let attempt = LinkAttempt(self.next_attempt);
        let hub_handle = HubHandle::from_raw(raw_handle);
        self.next_attempt = next_attempt;
        relay.next_handle = next_handle;
        relay.deliver(
            self.me,
            LinkEvent::HandleAssigned {
                attempt,
                hub_handle,
            },
        );
        relay.deliver(
            peer,
            LinkEvent::IncomingInit {
                hub_handle,
                envelope_type,
                bytes,
            },
        );
        Ok(attempt)
    }

    fn send_response(
        &mut self,
        peer: PeerRef,
        hub_handle: HubHandle,
        envelope_type: u8,
        bytes: Vec<u8>,
    ) -> Result<(), LinkError> {
        let mut relay = relay();
        if relay.owned_mut(self.me, &self.owner).is_none() || !relay.has_room(peer) {
            return Err(LinkError);
        }
        relay.deliver(
            peer,
            LinkEvent::Response {
                hub_handle,
                envelope_type,
                bytes,
            },
        );
        Ok(())
    }

    fn poll(&mut self) -> Option<LinkEvent> {
        relay().owned_mut(self.me, &self.owner)?.events.pop_front()
    }
}

impl Drop for LoopbackEnvelopeRelay {
    fn drop(&mut self) {
        // Yalnız KENDİ jetonlu kayıt silinir; panik yolu yok.
        let mut relay = relay();
        if relay.owned_mut(self.me, &self.owner).is_some() {
            relay.mailboxes.remove(&self.me);
        }
    }
}

/// Ürün zarf taşıyıcısının TEK kurucusu: `me`'yi süreç-içi röleye kaydeder.
///
/// # Errors
/// [`NodeError::IdentityInUse`] — bu süreçte aynı cihaz kimliğiyle çalışan bir düğüm var.
pub(crate) fn product_envelope_link(me: PeerRef) -> Result<Box<dyn EnvelopeLink>, NodeError> {
    Ok(Box::new(LoopbackEnvelopeRelay::register(me)?))
}

#[cfg(test)]
mod tests {
    //! Röle testleri. Röle süreç-geneldir ve testler paralel koşar: her test KENDİ kimliklerini
    //! üretir ([`fresh_identity`]); sabit kimlik kullanılmaz.

    use std::sync::atomic::{AtomicU64, Ordering};

    use super::*;

    /// Testin zarf türü (röle yorumlamaz; yalnız aynen taşındığı sınanır).
    const INIT_TYPE: u8 = 0x01;
    const RESPONSE_TYPE: u8 = 0x02;
    /// Test kimliklerinin dolgu baytı: gerçek `dest_hash` rastgele 32 bayttır, test kimliği dolgu +
    /// süreç-tekil sayaçtır — ikisi çakışmaz.
    const TEST_IDENTITY_FILL: u8 = 0xC4;
    /// Bayt sınaması deseninin boyu: her bayt değeri en az bir kez.
    const PATTERN_LEN: usize = 300;

    /// Süreç içinde tekil test kimliği.
    fn fresh_identity() -> PeerRef {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let mut dest_hash = [TEST_IDENTITY_FILL; 32];
        dest_hash[..8].copy_from_slice(&n.to_be_bytes());
        PeerRef::from_dest_hash(dest_hash)
    }

    fn register_fresh() -> (PeerRef, LoopbackEnvelopeRelay) {
        let me = fresh_identity();
        (me, LoopbackEnvelopeRelay::register(me).expect("kayıt"))
    }

    fn pattern() -> Vec<u8> {
        (0..PATTERN_LEN)
            .map(|i| u8::try_from(i % (usize::from(u8::MAX) + 1)).expect("bir bayt"))
            .collect()
    }

    fn mailbox_len(me: PeerRef) -> Option<usize> {
        relay()
            .mailboxes
            .get(&me)
            .map(|mailbox| mailbox.events.len())
    }

    /// (i) Kayıtsız eşe INIT reddedilir ve hiçbir iz kalmaz: başlatana tutamaç yok, eş sonradan
    /// kaydolsa da zarf saklanmamıştır (store-and-forward yok). Düşen eşin kaydı da gider.
    #[test]
    fn init_to_an_unregistered_peer_is_refused_and_leaves_no_trace() {
        let (_, mut alice) = register_fresh();
        let absent = fresh_identity();
        assert_eq!(
            alice.send_init(absent, INIT_TYPE, pattern()),
            Err(LinkError)
        );
        assert_eq!(alice.poll(), None, "başlatana HandleAssigned yok");
        let mut late = LoopbackEnvelopeRelay::register(absent).expect("sonradan kayıt");
        assert_eq!(late.poll(), None, "zarf saklanmadı");
        drop(late);
        assert_eq!(mailbox_len(absent), None, "Drop kaydı sildi");
        assert_eq!(
            alice.send_init(absent, INIT_TYPE, pattern()),
            Err(LinkError)
        );
        assert_eq!(
            alice.send_response(
                absent,
                HubHandle::from_raw(FIRST_HUB_HANDLE),
                RESPONSE_TYPE,
                pattern()
            ),
            Err(LinkError),
            "RESPONSE da kayıtsız eşe gitmez"
        );
    }

    /// (ii) Aynı kimlik canlıyken ikinci kayıt tipli hata alır; ilk kayıt etkilenmez.
    #[test]
    fn second_registration_of_a_live_identity_is_refused_and_the_first_keeps_working() {
        let (alice_id, mut alice) = register_fresh();
        let (bob_id, mut bob) = register_fresh();
        assert!(matches!(
            LoopbackEnvelopeRelay::register(alice_id),
            Err(NodeError::IdentityInUse)
        ));
        let attempt = bob.send_init(alice_id, INIT_TYPE, pattern()).expect("INIT");
        assert!(matches!(alice.poll(), Some(LinkEvent::IncomingInit { .. })));
        assert!(matches!(
            bob.poll(),
            Some(LinkEvent::HandleAssigned { attempt: got, .. }) if got == attempt
        ));
        alice
            .send_response(
                bob_id,
                HubHandle::from_raw(FIRST_HUB_HANDLE),
                RESPONSE_TYPE,
                Vec::new(),
            )
            .expect("ilk kayıt çalışır");
    }

    /// (iii) Kuşak: eski bir uç noktanın geç Drop'u aynı kimliğin YENİ kaydını silmez. Genel akış
    /// bu sırayı üretemez (canlı kimliğin ikinci kaydı reddedilir); test onu kaydı doğrudan
    /// düşürerek kurar. Kaydı düşmüş eski uç nokta gönderemez ve yeni kaydın kutusunu okuyamaz.
    #[test]
    fn late_drop_of_a_stale_endpoint_never_removes_the_new_registration() {
        let (me, mut old) = register_fresh();
        let (bob_id, mut bob) = register_fresh();
        relay().mailboxes.remove(&me);
        let mut new = LoopbackEnvelopeRelay::register(me).expect("yeni kayıt");
        bob.send_init(me, INIT_TYPE, pattern())
            .expect("yeni kayda INIT");
        assert_eq!(old.poll(), None, "eski uç nokta yeni kutuyu okuyamaz");
        assert_eq!(old.send_init(bob_id, INIT_TYPE, pattern()), Err(LinkError));
        drop(old);
        assert_eq!(mailbox_len(me), Some(1), "yeni kayıt ve kutusu duruyor");
        assert!(matches!(new.poll(), Some(LinkEvent::IncomingInit { .. })));
        drop(new);
        assert_eq!(mailbox_len(me), None, "sahibi düşünce kayıt gider");
    }

    /// (iv) Bir INIT'in iki ucu AYNI tutamacı taşır (`HandleAssigned` ↔ `IncomingInit`); iki INIT
    /// iki FARKLI, sıfırdan farklı tutamaç ve iki farklı deneme kimliği alır.
    #[test]
    fn init_assigns_one_handle_to_both_ends_and_a_fresh_handle_per_init() {
        let (bob_id, mut bob) = register_fresh();
        let (_, mut alice) = register_fresh();
        let first = alice
            .send_init(bob_id, INIT_TYPE, pattern())
            .expect("INIT 1");
        let second = alice
            .send_init(bob_id, INIT_TYPE, pattern())
            .expect("INIT 2");
        assert_ne!(first, second);
        let assigned: Vec<(LinkAttempt, HubHandle)> = std::iter::from_fn(|| alice.poll())
            .map(|event| match event {
                LinkEvent::HandleAssigned {
                    attempt,
                    hub_handle,
                } => (attempt, hub_handle),
                other => panic!("başlatanın kutusunda yalnız HandleAssigned: {other:?}"),
            })
            .collect();
        let incoming: Vec<HubHandle> = std::iter::from_fn(|| bob.poll())
            .map(|event| match event {
                LinkEvent::IncomingInit { hub_handle, .. } => hub_handle,
                other => panic!("alıcının kutusunda yalnız IncomingInit: {other:?}"),
            })
            .collect();
        assert_eq!(
            assigned
                .iter()
                .map(|&(attempt, _)| attempt)
                .collect::<Vec<_>>(),
            [first, second]
        );
        let handles: Vec<HubHandle> = assigned.iter().map(|&(_, handle)| handle).collect();
        assert_eq!(handles, incoming, "iki uç aynı tutamaç");
        assert_ne!(handles[0], handles[1], "her INIT yeni tutamaç");
        assert!(handles.iter().all(|handle| handle.raw() != 0), "0 verilmez");
    }

    /// (v) RESPONSE yalnız hedef eşin kutusuna gider: gönderen ve üçüncü uç nokta bir şey görmez.
    #[test]
    fn response_reaches_only_the_addressed_peer() {
        let (alice_id, mut alice) = register_fresh();
        let (_, mut bob) = register_fresh();
        let (_, mut carol) = register_fresh();
        let handle = HubHandle::from_raw(FIRST_HUB_HANDLE);
        bob.send_response(alice_id, handle, RESPONSE_TYPE, pattern())
            .expect("RESPONSE");
        assert_eq!(
            alice.poll(),
            Some(LinkEvent::Response {
                hub_handle: handle,
                envelope_type: RESPONSE_TYPE,
                bytes: pattern(),
            })
        );
        assert_eq!(alice.poll(), None);
        assert_eq!(bob.poll(), None, "gönderene bir şey dönmez");
        assert_eq!(carol.poll(), None, "üçüncü uç nokta görmez");
    }

    /// (vi) Kutu tavanı: dolu kutuya gönderim `Err(LinkError)` alır ve HİÇBİR kutuya bir şey girmez
    /// (INIT'te başlatanın kutusu da); bir olay okununca gönderim yeniden kabul edilir.
    #[test]
    fn full_mailbox_refuses_without_queueing_anything() {
        let (alice_id, mut alice) = register_fresh();
        let (bob_id, mut bob) = register_fresh();
        let (_, mut carol) = register_fresh();
        let handle = HubHandle::from_raw(FIRST_HUB_HANDLE);
        for _ in 0..MAILBOX_CAP {
            bob.send_response(alice_id, handle, RESPONSE_TYPE, Vec::new())
                .expect("tavana dek");
        }
        assert_eq!(
            bob.send_response(alice_id, handle, RESPONSE_TYPE, Vec::new()),
            Err(LinkError)
        );
        assert_eq!(
            carol.send_init(alice_id, INIT_TYPE, Vec::new()),
            Err(LinkError),
            "alıcının kutusu dolu"
        );
        assert_eq!(carol.poll(), None, "başlatana HandleAssigned girmedi");
        assert_eq!(
            alice.send_init(bob_id, INIT_TYPE, Vec::new()),
            Err(LinkError),
            "başlatanın KENDİ kutusu dolu"
        );
        assert_eq!(mailbox_len(bob_id), Some(0), "alıcıya IncomingInit girmedi");
        assert_eq!(mailbox_len(alice_id), Some(MAILBOX_CAP));
        assert!(alice.poll().is_some());
        bob.send_response(alice_id, handle, RESPONSE_TYPE, Vec::new())
            .expect("yer açılınca kabul");
        assert_eq!(mailbox_len(alice_id), Some(MAILBOX_CAP));
        assert_eq!(bob.poll(), None);
    }

    /// (vii) Zarf baytları ve türü değişmeden iletilir (S3 K6: opak).
    #[test]
    fn envelope_bytes_and_type_pass_through_unchanged() {
        let (alice_id, mut alice) = register_fresh();
        let (bob_id, mut bob) = register_fresh();
        let init = pattern();
        let mut response = pattern();
        response.reverse();
        alice
            .send_init(bob_id, INIT_TYPE, init.clone())
            .expect("INIT");
        let handle = match bob.poll() {
            Some(LinkEvent::IncomingInit {
                hub_handle,
                envelope_type,
                bytes,
            }) => {
                assert_eq!(envelope_type, INIT_TYPE);
                assert_eq!(bytes, init, "INIT baytları aynen");
                hub_handle
            }
            other => panic!("IncomingInit bekleniyordu: {other:?}"),
        };
        bob.send_response(alice_id, handle, RESPONSE_TYPE, response.clone())
            .expect("RESPONSE");
        assert!(matches!(
            alice.poll(),
            Some(LinkEvent::HandleAssigned { .. })
        ));
        assert_eq!(
            alice.poll(),
            Some(LinkEvent::Response {
                hub_handle: handle,
                envelope_type: RESPONSE_TYPE,
                bytes: response,
            }),
            "RESPONSE baytları ve tutamacı aynen"
        );
    }
}
