//! K4G-02 — `init_counter` kalıcılığı: enjekte edilen `CounterStore` (plan D2 v1.1; noise-wire
//! §0.7 RESTART-POLİTİKASI: "sender init_counter'ı-cihaz-başına-KALICI+MONOTON-tutar").
//!
//! * 4(a) restart — aynı depoyla kurulan ikinci `SessionControl`'ün ilk INIT sayacı, öncekinin
//!   gönderdiği bütün sayaçlardan büyük.
//! * 4(b) çakışma — karşı tarafta etkin oturumun sayacı `n` iken yeniden başlayan taraf `n` ile
//!   gelirse `Failed`, `n + 1` ile gelirse kabul (K10 provisional). #104 YORUM 9 düzeltmesi.
//! * 4(c) hata veren depo — INIT gönderilmez (opak `Failed`, neden
//!   `ControlFailure::CounterPersist`), değer ayrılmaz, düzlem durumu bayt-özdeş.
//! * Madde 2 — düzlem sayacı kendisi artırmaz ve belleğe almaz: her INIT depoya TAM BİR kez sorar,
//!   dönen değer telde AYNEN görünür; INIT üretmeyen yollar (kabul, yanıt, glare) depoya sormaz.
//!
//! Sayaç telden okunur (`handshake::parse_ssi1`): kanıt, gönderilen baytlardır.

#![cfg(feature = "test-util")]

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use kama_contracts::handles::{CloseReason, PeerRef, SessionHandle};
use kama_contracts::s3::{
    HubHandle, PacketProtection, ProtectionContext, SessionCommand, SessionControl, SessionEvent,
    SessionResult,
};
use kama_noise_wire::clock::ManualClock;
use kama_noise_wire::counter_store::{CounterStore, CounterStoreError, MemoryCounterStore};
use kama_noise_wire::entropy::OsEntropy;
use kama_noise_wire::epoch_machine::GRACE_T_G;
use kama_noise_wire::handshake::parse_ssi1;
use kama_noise_wire::peer_dir::{LocalIdentity, PeerDirectory, PeerEntry};
use kama_noise_wire::protector::NoiseWireProtection;
use kama_noise_wire::session_control::{ControlConfig, ControlFailure, EnvelopeKind};
use sha2::{Digest, Sha256};
use x25519_dalek::StaticSecret;

const ALICE: u8 = 0xA1;
const BOB: u8 = 0xB0;
const CAROL: u8 = 0xC0;

fn peer(n: u8) -> PeerRef {
    PeerRef::from_dest_hash([n; 32])
}

fn derive(label: &[u8], n: u8) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(label);
    h.update([n]);
    h.finalize().into()
}

/// TEST kimliği: anahtarlar etiketten türer — GİZLİ DEĞİLDİR.
fn identity(n: u8) -> LocalIdentity {
    LocalIdentity::new(
        StaticSecret::from(derive(b"k4g02-static", n)),
        [n; 32],
        derive(b"k4g02-cert-ref", n),
    )
}

/// Dizin: Alice, Bob, Carol.
fn directory() -> PeerDirectory {
    PeerDirectory::new([ALICE, BOB, CAROL].map(|n| {
        let id = identity(n);
        PeerEntry {
            dest_hash: id.dest_hash,
            static_x25519_pub: *id.static_public().as_bytes(),
            cert_ref: id.cert_ref,
        }
    }))
    .unwrap()
}

/// INIT'in telde taşıdığı `init_counter`.
fn wire_counter(init: &[u8]) -> u64 {
    parse_ssi1(init).expect("SSI1 ayrıştırılmalı").init_counter
}

struct Node {
    c: NoiseWireProtection,
    clock: ManualClock,
}

/// Ürün kurulumu (`with_control`); `init_counter` deposu dışarıdan verilir.
fn node(n: u8, counter_store: Arc<dyn CounterStore>) -> Node {
    let clock = ManualClock::new();
    let c = NoiseWireProtection::with_control(ControlConfig {
        identity: identity(n),
        directory: directory(),
        clock: Box::new(clock.clone()),
        entropy: Box::new(OsEntropy::new()),
        counter_store,
        envelope: EnvelopeKind::Extended,
    });
    Node { c, clock }
}

impl Node {
    /// `Initiate` → (bekleyen oturum, INIT baytları, teldeki sayaç).
    fn initiate(&mut self, to: u8) -> (SessionHandle, Vec<u8>, u64) {
        match self.c.command(SessionCommand::Initiate {
            peer: peer(to),
            early_unit: None,
        }) {
            SessionResult::Initiated { session, bytes, .. } => {
                let n = wire_counter(&bytes);
                (session, bytes, n)
            }
            other => panic!("Initiate: {other:?}"),
        }
    }

    /// `Renew` → (yeni bekleyen oturum, teldeki sayaç).
    fn renew(&mut self, h: SessionHandle) -> (SessionHandle, u64) {
        match self.c.command(SessionCommand::Renew { session: h }) {
            SessionResult::Initiated { session, bytes, .. } => (session, wire_counter(&bytes)),
            other => panic!("Renew: {other:?}"),
        }
    }

    fn assign(&mut self, h: SessionHandle, hub: u32) {
        assert_eq!(
            self.c.command(SessionCommand::AssignHandle {
                session: h,
                hub_handle: HubHandle::from_raw(hub),
            }),
            SessionResult::NoOutput
        );
    }

    fn events(&mut self) -> Vec<SessionEvent> {
        std::iter::from_fn(|| self.c.poll_event()).collect()
    }

    fn seal(&mut self, h: SessionHandle) -> Vec<u8> {
        self.c
            .seal(ProtectionContext::Session(h), b"veri")
            .expect("oturum mühürlemeli")
            .bytes
    }
}

/// Alice → Bob kurulu oturum (hub tutamacı `hub`). Döner: (Alice'in, Bob'un tutamacı, sayaç).
fn establish(a: &mut Node, b: &mut Node, hub: u32) -> (SessionHandle, SessionHandle, u64) {
    let (ha, init, n) = a.initiate(BOB);
    a.assign(ha, hub);
    let SessionResult::EstablishedAndReply {
        session: hb,
        bytes: resp,
        ..
    } = b.c.command(SessionCommand::AcceptInit { wire: init })
    else {
        panic!("AcceptInit");
    };
    b.assign(hb, hub);
    assert_eq!(
        a.c.command(SessionCommand::AcceptResponse { wire: resp }),
        SessionResult::Established { session: ha }
    );
    assert_eq!(a.events(), vec![SessionEvent::Ready(ha)]);
    assert_eq!(b.events(), vec![SessionEvent::Ready(hb)]);
    (ha, hb, n)
}

/// Hata enjekte edilebilen depo (4(c)): `fail` açıkken DEĞER AYIRMADAN hata döner (D2: "depo hata
/// verirse değer ayrılmamıştır"); kapalıyken iç bellek deposuna devreder. Çağrıları sayar.
#[derive(Default)]
struct FailingCounterStore {
    inner: MemoryCounterStore,
    fail: AtomicBool,
    calls: AtomicUsize,
}

impl FailingCounterStore {
    fn set_fail(&self, fail: bool) {
        self.fail.store(fail, Ordering::SeqCst);
    }

    fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
}

impl CounterStore for FailingCounterStore {
    fn reserve_next(&self) -> Result<u64, CounterStoreError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if self.fail.load(Ordering::SeqCst) {
            return Err(CounterStoreError::Persist);
        }
        self.inner.reserve_next()
    }
}

/// Önceden verilmiş (kesin artan, BOŞLUKLU) değerleri sırayla veren depo (madde 2): düzlem sayacı
/// kendisi türetseydi teldeki değer bu diziden saphardı. Çağrıları sayar.
struct ScriptedCounterStore {
    values: Mutex<VecDeque<u64>>,
    calls: AtomicUsize,
}

impl ScriptedCounterStore {
    fn new(values: &[u64]) -> Self {
        Self {
            values: Mutex::new(values.iter().copied().collect()),
            calls: AtomicUsize::new(0),
        }
    }

    fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
}

impl CounterStore for ScriptedCounterStore {
    fn reserve_next(&self) -> Result<u64, CounterStoreError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.values
            .lock()
            .unwrap()
            .pop_front()
            .ok_or(CounterStoreError::Exhausted)
    }
}

/// 4(a) — restart: aynı depoyla kurulan ikinci `SessionControl`'ün ilk INIT sayacı, ilk örneğin
/// gönderdiği bütün sayaçlardan büyüktür. Sayaç CİHAZ başınadır: başka eşe giden INIT ve `Renew`
/// aynı diziden, kesin artan değer alır.
#[test]
fn k4g02_4a_restart_with_same_store_resumes_above_every_sent_counter() {
    let store = Arc::new(MemoryCounterStore::new());
    let mut sent = Vec::new();
    {
        let mut a = node(ALICE, store.clone());
        let mut b = node(BOB, Arc::new(MemoryCounterStore::new()));
        let (ha, _, n0) = establish(&mut a, &mut b, 0x0101);
        let (_, _, n1) = a.initiate(CAROL);
        let (_, n2) = a.renew(ha);
        sent.extend([n0, n1, n2]);
        // Süreç ölür: düzlem bellekteki her şeyiyle gider, depo kalır.
    }
    assert!(
        sent.windows(2).all(|w| w[0] < w[1]),
        "eşler ve Renew arasında kesin artan değil: {sent:?}"
    );

    let mut restarted = node(ALICE, store);
    let (_, _, first) = restarted.initiate(BOB);
    let max_before = *sent.iter().max().unwrap();
    assert!(
        first > max_before,
        "yeniden başlatma sonrası ilk INIT {first}, önceki en büyük {max_before}'den büyük olmalı"
    );
}

/// 4(b) — çakışma (yanıtlayanın davranışı; bu kart onu DEĞİŞTİRMEZ, sınar). Bob'da Alice ile etkin
/// oturum `init_counter = n` ile kurulu; Alice'in süreci ölür ve yeniden başlar:
/// * kalıcı durumu KAYBOLMUŞ (yeni bellek deposu = K4G-02 öncesi davranış, sayaç baştan) → INIT
///   yine `n` taşır, baytları farklıdır → Bob `Failed`; Bob'un durumu bayt-özdeş, olay yok, eski
///   oturum çalışır. Provisional yol AÇILMAZ.
/// * AYNI kalıcı depoyla → INIT `n + 1` taşır → Bob kabul eder (`ProvisionalAndReply`, K10); ilk
///   epoch-1 paketi yeni oturumu onaylar, eski oturum drain → `Closed(Superseded)`.
///
/// **#104 YORUM 9 düzeltmesi.** PR #104 gövdesindeki YORUM 9: ~~"`init_counter`: eş başına,
/// bellekte, 0'dan. Süreç yeniden başlarsa sayaç sıfırlanır; bayat/yeniden sayaçlı INIT'ler K10
/// provisional yolundan geçer ve onaylanmazsa TTL ile düşer."~~ Çakışan sayaç için YANLIŞTI:
/// yanıtlayanın önbelleğinde aynı `(eş, init_counter)` varken farklı baytlı INIT provisional yola
/// GİRMEZ; `AcceptInit`'in H-RETRY sınaması (glare'den ve AEAD'den önce) onu `Failed` ile reddeder.
/// Yalnız önbellekte olmayan sayaç provisional yoldan geçer. K4G-02 ile `init_counter` cihaz
/// başına, kalıcı (`CounterStore`) ve monotondur; yeniden başlayan başlatıcı `n`'den büyük
/// değerle gelir.
#[test]
fn k4g02_4b_restart_collision_on_n_fails_and_n_plus_1_is_accepted() {
    let store_a = Arc::new(MemoryCounterStore::new());
    let mut b = node(BOB, Arc::new(MemoryCounterStore::new()));
    let (hb, n) = {
        let mut a = node(ALICE, store_a.clone());
        let (_, hb, n) = establish(&mut a, &mut b, 0x0101);
        // Alice'in süreci ölür; Bob'da `n` ile kurulmuş etkin oturum sürer.
        (hb, n)
    };

    // Kalıcı durum kaybolmuş: sayaç baştan → aynı `n`, farklı bayt (yeni geçici anahtar).
    let mut lost = node(ALICE, Arc::new(MemoryCounterStore::new()));
    let (_, init_lost, n_lost) = lost.initiate(BOB);
    assert_eq!(
        n_lost, n,
        "kalıcılık yoksa yeniden başlayan aynı sayaçla gelir"
    );
    let fp = b.c.state_fingerprint();
    assert_eq!(
        b.c.command(SessionCommand::AcceptInit { wire: init_lost }),
        SessionResult::Failed,
        "aynı sayaç, farklı bayt: provisional yola girmez"
    );
    assert_eq!(b.c.state_fingerprint(), fp, "ret Bob'un durumuna dokundu");
    assert!(b.events().is_empty(), "ret olay üretmez");
    assert!(!b.seal(hb).is_empty(), "eski oturum çalışır");

    // Aynı kalıcı depo → `n + 1` → kabul (K10 provisional).
    let mut a2 = node(ALICE, store_a);
    let (ha2, init2, n2) = a2.initiate(BOB);
    assert_eq!(n2, n + 1, "kalıcı depo `n`'in ardılını verir");
    a2.assign(ha2, 0x0202);
    let SessionResult::ProvisionalAndReply {
        new: hb2,
        supersedes,
        peer: p,
        bytes: resp2,
        ..
    } = b.c.command(SessionCommand::AcceptInit { wire: init2 })
    else {
        panic!("`n + 1` kabul edilmeli (eşle etkin oturum var → ProvisionalAndReply)");
    };
    assert_eq!((supersedes, p), (hb, peer(ALICE)));
    b.assign(hb2, 0x0202);
    assert!(b.events().is_empty(), "provisional Ready üretmez");
    assert_eq!(
        a2.c.command(SessionCommand::AcceptResponse { wire: resp2 }),
        SessionResult::Established { session: ha2 }
    );
    assert_eq!(a2.events(), vec![SessionEvent::Ready(ha2)]);

    // Onay: provisional oturumda ilk AEAD-geçerli epoch-1 paketi; eski oturum drain → kapanır.
    let w = a2.seal(ha2);
    assert!(b.c.open(&w).is_some());
    assert_eq!(b.events(), vec![SessionEvent::Confirmed(hb2)]);
    b.clock.advance(GRACE_T_G);
    assert_eq!(
        b.events(),
        vec![SessionEvent::Closed(hb, CloseReason::Superseded)]
    );
}

/// 4(c) — depo hata verirse INIT GÖNDERİLMEZ ve sayaç İLERLEMEZ. Üç INIT yolu da (ilk INIT,
/// `Renew`, `test-util` kancası) depoya BİR kez sorar; hata alınca: opak `Failed` (S3), neden
/// `ControlFailure::CounterPersist`, bekleyen INIT yok, olay yok, düzlem durumu bayt-özdeş, kurulu
/// oturum çalışır. Depo düzelince sonraki INIT, hatalardan ÖNCEKİ sıradaki değeri taşır.
#[test]
fn k4g02_4c_failing_store_sends_no_init_and_reserves_nothing() {
    let store = Arc::new(FailingCounterStore::default());
    let mut a = node(ALICE, store.clone());
    let mut b = node(BOB, Arc::new(MemoryCounterStore::new()));
    let (ha, _, n) = establish(&mut a, &mut b, 0x0101);
    assert_eq!(store.calls(), 1);

    store.set_fail(true);
    let fp = a.c.state_fingerprint();
    let persist = Some(ControlFailure::CounterPersist(CounterStoreError::Persist));
    assert_eq!(
        a.c.command(SessionCommand::Initiate {
            peer: peer(CAROL),
            early_unit: None,
        }),
        SessionResult::Failed,
        "ilk INIT yolu"
    );
    assert_eq!(a.c.last_control_failure(), persist);
    assert_eq!(
        a.c.command(SessionCommand::Renew { session: ha }),
        SessionResult::Failed,
        "Renew yolu"
    );
    assert_eq!(a.c.last_control_failure(), persist);
    assert_eq!(
        a.c.initiate_with_ephemeral_for_test(peer(CAROL), [7u8; 32]),
        None,
        "test-util kancası"
    );
    assert_eq!(a.c.counter_persist_failures(), 3);
    assert_eq!(store.calls(), 4, "her INIT girişimi depoya BİR kez sordu");
    assert_eq!(a.c.pending_len(), 0, "bekleyen INIT yok");
    assert_eq!(
        a.c.state_fingerprint(),
        fp,
        "başarısız ayırma düzlem durumuna dokundu"
    );
    assert!(a.events().is_empty(), "olay yok");
    assert!(!a.seal(ha).is_empty(), "kurulu oturum etkilenmedi");

    // Depo düzelir: başarısız ayırmalar değer TÜKETMEDİ.
    store.set_fail(false);
    let (_, _, next) = a.initiate(CAROL);
    assert_eq!(next, n + 1, "sayaç hatalarda ilerledi");
    assert_eq!(
        a.c.last_control_failure(),
        None,
        "başarılı komut nedeni temizler"
    );
    assert_eq!(a.c.counter_persist_failures(), 3, "toplam korunur");
}

/// Madde 2 — düzlem sayacı kendisi ARTIRMAZ ve belleğe ALMAZ: her INIT (ilk INIT, başka eş,
/// `Renew`, `test-util` kancası) depoya TAM BİR kez sorar ve dönen değer telde AYNEN görünür
/// (boşluklu dizi: düzlem `+1` türetseydi saphardı). Yanıtlayan (`AcceptInit`) depoya sormaz.
#[test]
fn k4g02_2_each_init_reserves_once_and_carries_the_store_value_verbatim() {
    let sa = Arc::new(ScriptedCounterStore::new(&[10, 20, 35, 1_000_000]));
    let sb = Arc::new(ScriptedCounterStore::new(&[]));
    let mut a = node(ALICE, sa.clone());
    let mut b = node(BOB, sb.clone());

    let (ha, _, c0) = establish(&mut a, &mut b, 0x0101);
    let (_, _, c1) = a.initiate(CAROL);
    let (_, c2) = a.renew(ha);
    assert_eq!([c0, c1, c2], [10, 20, 35]);
    assert_eq!(sa.calls(), 3);
    assert!(a
        .c
        .initiate_with_ephemeral_for_test(peer(CAROL), [7u8; 32])
        .is_some());
    assert_eq!(sa.calls(), 4, "test-util kancası da BİR kez sorar");
    assert_eq!(sb.calls(), 0, "yanıtlayan sayaç ayırmaz");
}

/// Madde 2 (glare) — iki uç aynı anda başlatır: her INIT BİR değer ayırır; glare'in iki kolu
/// (kazanan gelen INIT'i düşürür / kaybeden bekleyenini siler ve gelen INIT'i işler) ve yanıt yolu
/// INIT ÜRETMEZ, depoya sormaz.
#[test]
fn k4g02_2_glare_accept_and_response_paths_reserve_nothing() {
    let sa = Arc::new(ScriptedCounterStore::new(&[5]));
    let sb = Arc::new(ScriptedCounterStore::new(&[9]));
    let mut a = node(ALICE, sa.clone());
    let mut b = node(BOB, sb.clone());
    let (ha, init_a, ca) = a.initiate(BOB);
    let (_, init_b, cb) = b.initiate(ALICE);
    assert_eq!((ca, cb), (5, 9));
    // Alice ([0xA1; 32]) < Bob ([0xB0; 32]): Alice kazanır, Bob'un INIT'i sessiz düşer.
    assert_eq!(
        a.c.command(SessionCommand::AcceptInit { wire: init_b }),
        SessionResult::NoOutput
    );
    let SessionResult::EstablishedAndReply { bytes: resp, .. } =
        b.c.command(SessionCommand::AcceptInit { wire: init_a })
    else {
        panic!("kaybeden kazananın INIT'ini işler");
    };
    assert_eq!(
        a.c.command(SessionCommand::AcceptResponse { wire: resp }),
        SessionResult::Established { session: ha }
    );
    assert_eq!(
        (sa.calls(), sb.calls()),
        (1, 1),
        "glare, kabul ve yanıt yolları sayaç ayırmaz"
    );
}
