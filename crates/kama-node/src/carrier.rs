//! Zarf taşıyıcısı — INIT/RESPONSE zarflarını (SSI1/SSR1) eşe götüren
//! `client_core::transport::EnvelopeLink`: [`LoopbackEnvelopeRelay`], süreç-içi zarf rölesi.
//!
//! ürün taşıyıcısı değildir — iki cihaz arası taşıyıcı = hub Plane 1 rölesi, S4, hub sprinti
//!
//! # DUR-K4G-11 kararı (Abkarada, CEVAP PAKETİ 3, 2026-10-12): Seçenek B
//!
//! Bu sprintte zarf taşıyıcısı süreç-içi röledir: AYNI süreçteki düğümler (CORE testi, INT-02)
//! zarflarını bununla değiş tokuş eder; iki telefon arası oturum hub sprintiyle gelir. Plan Ek
//! 2026-10-12 (5): "(A) `DirectUdpLink` reddedildi: yeni tel (spec + vektör) ve INIT retry (T3)
//! olmadan %30 kayıpta bağlanma kurulamayabilir; (C) reddedildi: bağlam-önce dağıtımı deler."
//! **INT-02 beyanlı boşluk:** veri datagramları UDP loopback + netem'den geçer; el sıkışma
//! zarfları süreç-içi röleden netem DIŞINDA geçer.
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
//!   ([`FIRST_HUB_HANDLE`]'dan başlar, 0 verilmez, sarma YOK: tükenirse `Err(LinkError)`);
//!   başlatanın kutusuna `HandleAssigned { attempt, hub_handle }`, alıcınınkine AYNI tutamaçla
//!   `IncomingInit` girer — direct-mode'da `client_core::memory_link::MemoryHub::relay` ile aynı
//!   anlam (iki yön aynı tutamacı kullanır). `send_response(peer, hub_handle)` yalnız `peer`'in
//!   kutusuna `Response` koyar. Kendine INIT reddedilir.
//! * **Kayıtsız eş** — `Err(LinkError)`; HİÇBİR şey kuyrukta tutulmaz (bekletme = store-and-forward
//!   = plan §4.5 YASAK). Transport bu hatayı yok sayar (`client-core` `transport.rs`:
//!   `if let Ok(a) = self.link.send_init(..)` — `connect`, `reinitiate`, yenileme): başlatanın
//!   Crypto'sunda bekleyen INIT kalır, niyet bekler; INIT'in yeniden gönderimi (T3) YOKTUR.
//! * **Geç başlayan eş — koddan belirlenen kural** — erken başlayan A, B kayıtsızken `connect`
//!   eder: röle reddeder, A'da bekleyen INIT kalır. B başlayıp A'ya `connect` edince A'da glare
//!   oluşur (`kama-noise-wire` `session_control.rs:735-738`). `A.dest_hash < B.dest_hash` ise A,
//!   B'nin INIT'ini sessizce düşürür (`NoOutput`; `transport.rs:694-695`); A'nın INIT'i B'ye hiç
//!   ulaşmadığı ve T3 olmadığı için İKİ niyet de takılır. Ters sırada A glare'i kaybeder, B'nin
//!   INIT'ini yanıtlar ve A'nın niyeti o oturumla çözülür. Takılma ancak A'daki bekleyen INIT
//!   kalkınca çözülür: A'nın `cancel_connect`'i (ardından yeniden `connect`) ya da A'da
//!   `HALF_OPEN_TTL` (7 gün; `session_control.rs:476-481` → `Closed(KeyHygiene)` →
//!   `transport.rs:837-843` → `reinitiate`). Geç tarafın (B) kendi yeniden denemesi tek başına
//!   ÇÖZMEZ: A'daki bekleyen INIT ve glare aynı kalır.
//! * **Yeniden başlatma** — el sıkışma ortasında kapanan düğümün kutusundaki okunmamış zarflar
//!   kayıtla birlikte silinir; RESPONSE gönderim hatası transport'ta yok sayılır (`transport.rs:
//!   670-672`, `690-692`). "Bağlanmadan önce iki düğüm de başlatılmış olmalı" kuralı yeniden
//!   başlatmayı KAPSAMAZ; tek süreçli testler (CORE, INT-02) düğümleri bağlanmadan önce başlatır ve
//!   el sıkışma sırasında yeniden başlatmaz — bunu kural sayar.
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

/// Rölenin atadığı ilk `HubHandle` değeri; 0 hiç verilmez.
const FIRST_HUB_HANDLE: u32 = 1;
const _: () = assert!(FIRST_HUB_HANDLE != 0);

/// Uç noktanın verdiği ilk `LinkAttempt` değeri.
const FIRST_ATTEMPT: u64 = 1;

/// Süreç-genel röle tablosu (bkz. modül belgesi).
static RELAY: Mutex<Relay> = Mutex::new(Relay::new());

/// Zehirlenmeye karşı panik yapmaz: tablonun her değişikliği tek bir kilitli adımda tamamlanır.
fn relay() -> MutexGuard<'static, Relay> {
    RELAY.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Röle tablosu: kayıtlı uç noktaların kutuları ve süreç-tekil tutamaç sayacı. Üründe tek örnek
/// süreç-geneldir ([`RELAY`]); birim testleri yerel bir örnek üzerinde sınar.
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

/// Bir kayıt: kimlik + o kaydın jetonu.
struct Registration {
    me: PeerRef,
    owner: Arc<Token>,
}

impl Relay {
    const fn new() -> Self {
        Self {
            mailboxes: BTreeMap::new(),
            next_handle: FIRST_HUB_HANDLE,
        }
    }

    /// `me`'yi kaydeder.
    ///
    /// # Errors
    /// [`NodeError::IdentityInUse`] — `me` kimliğiyle canlı bir kayıt var.
    fn register(&mut self, me: PeerRef) -> Result<Registration, NodeError> {
        if self.mailboxes.contains_key(&me) {
            return Err(NodeError::IdentityInUse);
        }
        let owner = Arc::new(Token);
        self.mailboxes.insert(
            me,
            Mailbox {
                owner: Arc::clone(&owner),
                events: VecDeque::new(),
            },
        );
        Ok(Registration { me, owner })
    }

    /// Kaydı siler — YALNIZ jetonu `registration`'ınki ise.
    fn unregister(&mut self, registration: &Registration) {
        if self.owned_mut(registration).is_some() {
            self.mailboxes.remove(&registration.me);
        }
    }

    /// `registration`'ın kutusu, YALNIZ jetonu onunki ise.
    fn owned_mut(&mut self, registration: &Registration) -> Option<&mut Mailbox> {
        self.mailboxes
            .get_mut(&registration.me)
            .filter(|mailbox| Arc::ptr_eq(&mailbox.owner, &registration.owner))
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

    /// Sıradaki süreç-tekil `HubHandle`. Sayaç YALNIZ başarıda ilerler; 0 verilmez, sarma YOK.
    ///
    /// # Errors
    /// [`LinkError`] — tutamaç uzayı tükendi (son değer `u32::MAX` verilmez: ardılı yazılamaz).
    fn allocate_handle(&mut self) -> Result<HubHandle, LinkError> {
        let raw = self.next_handle;
        self.next_handle = raw.checked_add(1).ok_or(LinkError)?;
        Ok(HubHandle::from_raw(raw))
    }

    /// `send_init`'in gövdesi: `from`'dan `peer`'e INIT. Bütün denetimler (kendine INIT, kendi
    /// kaydı, iki kutuda yer, deneme sayacının ardılı) HİÇBİR değişiklikten ÖNCE; tutamaç tahsisi
    /// son başarısız olabilecek adımdır. Hata yolunda tablo, sayaçlar ve kutular değişmez.
    ///
    /// # Errors
    /// [`LinkError`] — kendine INIT · kaydı düşmüş uç nokta · kayıtsız eş · dolu kutu · tükenmiş
    /// deneme ya da tutamaç uzayı.
    fn route_init(
        &mut self,
        from: &Registration,
        next_attempt: &mut u64,
        peer: PeerRef,
        envelope_type: u8,
        bytes: Vec<u8>,
    ) -> Result<LinkAttempt, LinkError> {
        if peer == from.me {
            // Kendine INIT'in yolu yoktur (eş dizini bu cihazı içermez).
            return Err(LinkError);
        }
        let own_room = self
            .owned_mut(from)
            .is_some_and(|mailbox| mailbox.events.len() < MAILBOX_CAP);
        if !own_room || !self.has_room(peer) {
            return Err(LinkError);
        }
        let following_attempt = next_attempt.checked_add(1).ok_or(LinkError)?;
        let hub_handle = self.allocate_handle()?;
        let attempt = LinkAttempt(*next_attempt);
        *next_attempt = following_attempt;
        self.deliver(
            from.me,
            LinkEvent::HandleAssigned {
                attempt,
                hub_handle,
            },
        );
        self.deliver(
            peer,
            LinkEvent::IncomingInit {
                hub_handle,
                envelope_type,
                bytes,
            },
        );
        Ok(attempt)
    }

    /// `send_response`'un gövdesi: yalnız `peer`'in kutusuna `Response`.
    ///
    /// # Errors
    /// [`LinkError`] — kaydı düşmüş uç nokta · kayıtsız eş · dolu kutu.
    fn route_response(
        &mut self,
        from: &Registration,
        peer: PeerRef,
        hub_handle: HubHandle,
        envelope_type: u8,
        bytes: Vec<u8>,
    ) -> Result<(), LinkError> {
        if self.owned_mut(from).is_none() || !self.has_room(peer) {
            return Err(LinkError);
        }
        self.deliver(
            peer,
            LinkEvent::Response {
                hub_handle,
                envelope_type,
                bytes,
            },
        );
        Ok(())
    }

    /// `registration`'ın kutusundaki sıradaki olay (kaydı düşmüşse yok).
    fn take(&mut self, registration: &Registration) -> Option<LinkEvent> {
        self.owned_mut(registration)?.events.pop_front()
    }
}

/// Süreç-içi zarf rölesinin bir uç noktası (bkz. modül belgesi).
///
/// ürün taşıyıcısı değildir — iki cihaz arası taşıyıcı = hub Plane 1 rölesi, S4, hub sprinti
pub(crate) struct LoopbackEnvelopeRelay {
    /// Bu uç noktanın kaydı (kimlik + jeton).
    registration: Registration,
    /// Sıradaki `LinkAttempt` değeri (uç nokta başına; transport onu yalnız kendi INIT'leriyle
    /// eşler).
    next_attempt: u64,
}

impl LoopbackEnvelopeRelay {
    /// `me`'yi süreç-genel röleye kaydeder.
    ///
    /// # Errors
    /// [`NodeError::IdentityInUse`] — bu süreçte `me` kimliğiyle canlı bir uç nokta var.
    fn register(me: PeerRef) -> Result<Self, NodeError> {
        Ok(Self {
            registration: relay().register(me)?,
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
        relay().route_init(
            &self.registration,
            &mut self.next_attempt,
            peer,
            envelope_type,
            bytes,
        )
    }

    fn send_response(
        &mut self,
        peer: PeerRef,
        hub_handle: HubHandle,
        envelope_type: u8,
        bytes: Vec<u8>,
    ) -> Result<(), LinkError> {
        relay().route_response(&self.registration, peer, hub_handle, envelope_type, bytes)
    }

    fn poll(&mut self) -> Option<LinkEvent> {
        relay().take(&self.registration)
    }
}

impl Drop for LoopbackEnvelopeRelay {
    fn drop(&mut self) {
        // Yalnız KENDİ jetonlu kayıt silinir; panik yolu yok.
        relay().unregister(&self.registration);
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
    /// düşürerek kurar. Kaydı düşmüş eski uç nokta gönderemez (INIT de RESPONSE da) ve yeni kaydın
    /// kutusunu okuyamaz.
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
        assert_eq!(
            old.send_response(
                bob_id,
                HubHandle::from_raw(FIRST_HUB_HANDLE),
                RESPONSE_TYPE,
                pattern()
            ),
            Err(LinkError),
            "kaydı düşmüş uç nokta RESPONSE da gönderemez"
        );
        assert_eq!(
            mailbox_len(bob_id),
            Some(1),
            "bob'un kutusu değişmedi (yalnız kendi INIT'inin HandleAssigned'ı)"
        );
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

    /// Kendine INIT reddedilir ve hiçbir kutuya bir şey girmez.
    #[test]
    fn init_to_self_is_refused_and_queues_nothing() {
        let (alice_id, mut alice) = register_fresh();
        assert_eq!(
            alice.send_init(alice_id, INIT_TYPE, pattern()),
            Err(LinkError)
        );
        assert_eq!(mailbox_len(alice_id), Some(0), "kutu değişmedi");
    }

    // ── Yerel tablo: süreç-genel `RELAY`'e dokunmadan, sırasından bağımsız ─────────────────────

    /// Yerel tabloda iki kayıt (başlatan, alıcı).
    fn local_pair() -> (Relay, Registration, Registration) {
        let mut relay = Relay::new();
        let alice = relay.register(fresh_identity()).expect("kayıt");
        let bob = relay.register(fresh_identity()).expect("kayıt");
        (relay, alice, bob)
    }

    /// İlk tutamaç `FIRST_HUB_HANDLE` (≠ 0) ve tutamaçlar kesin artar.
    #[test]
    fn handles_start_above_zero_and_strictly_increase() {
        let mut relay = Relay::new();
        let first = relay.allocate_handle().expect("tutamaç");
        let second = relay.allocate_handle().expect("tutamaç");
        assert_eq!(first.raw(), FIRST_HUB_HANDLE);
        assert_ne!(first.raw(), 0, "0 verilmez");
        assert!(second.raw() > first.raw(), "kesin artan");
    }

    /// Tutamaç uzayı tükenince INIT `Err(LinkError)` alır: sayaç ilerlemez (sarma YOK), deneme
    /// sayacı ilerlemez, hiçbir kutuya olay girmez. Son verilebilir değer `u32::MAX − 1`'dir.
    #[test]
    fn exhausted_handle_space_refuses_init_without_side_effects() {
        let (mut relay, alice, bob) = local_pair();
        relay.next_handle = u32::MAX - 1;
        let mut next_attempt = FIRST_ATTEMPT;
        relay
            .route_init(&alice, &mut next_attempt, bob.me, INIT_TYPE, pattern())
            .expect("son tutamaç verilir");
        assert!(matches!(
            relay.take(&bob),
            Some(LinkEvent::IncomingInit { hub_handle, .. }) if hub_handle.raw() == u32::MAX - 1
        ));
        assert!(matches!(
            relay.take(&alice),
            Some(LinkEvent::HandleAssigned { .. })
        ));
        assert_eq!(relay.next_handle, u32::MAX);
        let attempt_before = next_attempt;
        assert_eq!(
            relay.route_init(&alice, &mut next_attempt, bob.me, INIT_TYPE, pattern()),
            Err(LinkError)
        );
        assert_eq!(
            relay.next_handle,
            u32::MAX,
            "tutamaç sayacı ilerlemedi, sarmadı"
        );
        assert_eq!(next_attempt, attempt_before, "deneme sayacı ilerlemedi");
        assert_eq!(relay.take(&alice), None, "başlatana olay girmedi");
        assert_eq!(relay.take(&bob), None, "alıcıya olay girmedi");
    }

    /// Deneme uzayı tükenince INIT `Err(LinkError)` alır ve hiçbir yan etki olmaz: deneme ve
    /// tutamaç sayaçları ilerlemez, hiçbir kutuya olay girmez.
    #[test]
    fn exhausted_attempt_space_refuses_init_without_side_effects() {
        let (mut relay, alice, bob) = local_pair();
        let mut next_attempt = u64::MAX;
        assert_eq!(
            relay.route_init(&alice, &mut next_attempt, bob.me, INIT_TYPE, pattern()),
            Err(LinkError)
        );
        assert_eq!(next_attempt, u64::MAX, "deneme sayacı ilerlemedi, sarmadı");
        assert_eq!(relay.next_handle, FIRST_HUB_HANDLE, "tutamaç harcanmadı");
        assert_eq!(relay.take(&alice), None);
        assert_eq!(relay.take(&bob), None);
    }
}
