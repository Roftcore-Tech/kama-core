//! **K4G-01 — bağlam-önce dağıtım (D1 v1.1) regresyon testi.** Gerçek `NoiseWireProtection` (ürün
//! kurulumu `with_control`; TEST anahtarları `kama_noise_wire::seam_mock::fixture_*`, GİZLİ DEĞİL)
//! + gerçek veri yolu (`ClientEndpoint` / `ServerEndpoint`, UDP loopback).
//!
//! İLKE: Bağlam yükün tipinden türetilmez; bağlam dağıtıcıyı seçer. Yük, bağlam doğrulanmadan
//! çözülmez.
//!
//! SALDIRI (plan A1): `NoiseWireProtection::open`, tutamacı kayıtlı olmayan ve son 16 baytı sıfır
//! olan her datagramı AEAD'siz `HubChannel` olarak açar (no-op hop kanalı; tasarım gereği, M4'e
//! dek). Yoldaki saldırgan ANAHTARSIZ `çerçeve ‖ 0×16` yollar ve çerçevenin tipini de kendisi seçer
//! (`forged` bu datagramı kurar ve alıcının onu gerçekten HubChannel açtığını ön-koşul olarak
//! doğrular).
//!
//! İstemci ucu — kart 5(a)–(d):
//! * (a) uçuşta bir birim varken o birimi onaylayan düzgün biçimli ACK, HubChannel'da → birim ARQ
//!   deposunda KALIR, `hub_frames_dropped == 1`, Session durumu bayt-özdeş;
//! * (b) aynısı RECOVERED, CS_ECHO, PARAM_ACK için;
//! * (c) başka bir oturumun tutamacıyla GEÇERLİ (AEAD'li) Session datagramı → düşer,
//!   `ctx_mismatch_drops == 1`;
//! * (d) beklenen tutamaçla gelen gerçek ACK birimi hâlâ siler (pozitif kol).
//!
//! Her negatif vakadan sonra AYNI çerçeve baytları beklenen Session bağlamında yollanır ve etkisi
//! görülür: negatif kolun etkisizliği çerçevenin bozukluğundan değil, bağlamdan gelir (vakum yok).
//!
//! "Session durumu bayt-özdeş" — ölçülen ve AYIRT EDİCİ kanıt Transport düzlemindedir: ARQ deposu
//! (`outstanding`, teyit akışı) ve Session sayaçları (`acked`, `fec_recovered_ctrl_units`,
//! `cs_echoes_rx`, `param_confirmed`, `unknown_frames`) sahte datagramdan sonra değişmez ve yalnız
//! pozitif kontrollerin etkisini taşır; PARAM_ACK vakasında gözlenen durum PARAM onayının
//! kendisidir (yineleme sürer). Crypto parmak izi (`NoiseWireProtection::state_fingerprint()`, uç
//! sessizken — kurtarma kapalı, PARAM onaylı) bu yolda AYIRT EDİCİ DEĞİLDİR: `open_hub` durumsuzdur
//! ve ESKİ (bağlamsız) runtime da sahte ACK'i kriptoya dokunmadan işlerdi. Parmak izi yalnız
//! saldırının kripto durumuna dokunmadığını sınayan bir ÖN-KOŞUL denetimidir. Runtime'ın iç
//! haritaları için test-util anlık görüntüsü EKLENMEDİ (beyanlı istisna).
//!
//! Sunucu ucu (D1 iki yolu kapsar): mevcut bir Session yuvasının adresinden gelen HubChannel DATA,
//! başka bir oturumun geçerli datagramı, oturum tablosunda olmayan oturum ve 1 yuvalık eş
//! tablosunun doldurulma denemesi. ESKİ DAVRANIŞ: sunucu bağlama hiç bakmıyordu — yuva ilk
//! datagramın bağlamıyla kuruluyor, teslim YUVANIN bağlamıyla (`Session(h)`) etiketleniyordu;
//! HubChannel'ın sabit PN'i (0) yuvanın alıcısında yeniyse (PN 0 yolda kaybolduysa ya da 4096'lık
//! dedup penceresinden kaydıysa) anahtarsız DATA S1'e oturum verisi olarak ulaşıyordu.
//!
//! INV-ACK-DELIVER-1 (plan Ek 2026-10-11 (3)): teslim yoluna kabul edilmeyen birim ACK'lenmez —
//! gerçek `ClientTransport` çifti, `MemoryHub` rölesi elle sürülerek belirlenimli.

use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use adaptive_transport::ack::{AckFrame, ARRIVAL_RATE_SENTINEL};
use client_core::memory_link::MemoryHub;
use client_core::runtime::{
    ClientEndpoint, ClientHandle, ClientStats, CtxDispatchCounters, CtxDispatchStats, Delivery,
    ExpectedSessions, RuntimeConfig, ServerEndpoint, ServerHandle,
};
use client_core::transport::{ClientTransport, ClientTransportConfig};
use client_core::wire::{
    decode_data, decode_param, encode_cs_echo, encode_data, encode_param_ack, encode_recovered,
    CS_PROBE_SUBTYPE,
};
use kama_contracts::handles::{PeerRef, SessionHandle};
use kama_contracts::s1::{
    CompressionPolicy, ConnectResult, DeliveryPolicy, KamaTransport, PreparedUnit,
};
use kama_contracts::s3::{
    HubHandle, Opened, PacketProtection, ProtectionClass, ProtectionContext, ProtectionLimits,
    SealError, SealedUnit, SessionCommand, SessionControl, SessionResult,
};
use kama_noise_wire::protector::NoiseWireProtection;
use kama_noise_wire::seam_mock::{fixture_directory, fixture_protection};
use kama_noise_wire::session_control::EnvelopeKind;
use tokio::net::UdpSocket;
use tokio::time::Instant;

const ALICE: u8 = 0xA1;
const BOB: u8 = 0xB0;
const CAROL: u8 = 0xC0;

/// Oturumların hub (wire) tutamaçları. İlk baytları hiçbir çerçeve tipiyle (0x00–0x07, 0x10, 0x14)
/// çakışmaz: sahte HubChannel datagramının ilk dört baytı çerçevenin başıdır ve kayıtlı bir
/// tutamaca denk gelmez (bunu `forged` ayrıca doğrular).
const HUB_AB: HubHandle = HubHandle::from_raw(0x7E57_0AB0);
const HUB_AC: HubHandle = HubHandle::from_raw(0x7E57_0AC0);

/// No-op HubChannel'ın sıfır "etiketi" (gerçek oturumun AEAD etiketi kadar).
const HUB_NOOP_TAG: usize = 16;
/// Beklenen bir olayın (sayaç, teyit, teslim) tavanı — aşılırsa kırmızı, sessiz geçiş yok.
const WAIT: Duration = Duration::from_secs(5);
/// "Gelmedi" iddialarının penceresi.
const QUIET: Duration = Duration::from_millis(300);
/// PARAM_SET yineleme periyodunun (≥ 200 ms) birkaç katı: onaylanmamış uç bu sürede yineler.
const PARAM_WINDOW: Duration = Duration::from_millis(700);
/// ARQ yeniden gönderimini bekleme tavanı (gerçek zaman; kuruluş probu 0,5 s'den, PTO 1 s'den
/// başlar ve katlanır).
const RETX_WAIT: Duration = Duration::from_secs(15);

// ── Fikstür ──────────────────────────────────────────────────────────────────────────────────

/// Sentetik test kimliği.
fn id(n: u8) -> PeerRef {
    let mut dest_hash = [0u8; 32];
    dest_hash[0] = 0x4B;
    dest_hash[1] = n;
    PeerRef::from_dest_hash(dest_hash)
}

/// Üç kimliği de bilen dizinle ürün kurulumu (`with_control`); saat hiç ilerletilmez.
fn node(me: u8) -> NoiseWireProtection {
    let ids = [ALICE, BOB, CAROL].map(|n| *id(n).as_bytes());
    let dir = fixture_directory(ids).expect("fikstür dizini");
    let (protection, _clock) = fixture_protection(*id(me).as_bytes(), dir, EnvelopeKind::Legacy);
    protection
}

/// `SessionControl` el sıkışması (`a` başlatır, `b` yanıtlar) + iki uca aynı hub tutamacı.
/// Döner: (`a`'daki tutamaç, `b`'deki tutamaç).
fn handshake(
    a: &mut NoiseWireProtection,
    b: &mut NoiseWireProtection,
    b_id: PeerRef,
    hub: HubHandle,
) -> (SessionHandle, SessionHandle) {
    let SessionResult::Initiated {
        session: ha,
        bytes: init,
        ..
    } = a.command(SessionCommand::Initiate {
        peer: b_id,
        early_unit: None,
    })
    else {
        panic!("Initiate bekleyen oturum + INIT üretmeli");
    };
    let SessionResult::EstablishedAndReply {
        session: hb,
        bytes: resp,
        ..
    } = b.command(SessionCommand::AcceptInit { wire: init })
    else {
        panic!("AcceptInit oturum + RESPONSE üretmeli");
    };
    assert_eq!(
        a.command(SessionCommand::AcceptResponse { wire: resp }),
        SessionResult::Established { session: ha }
    );
    for (c, h) in [(&mut *a, ha), (&mut *b, hb)] {
        assert_eq!(
            c.command(SessionCommand::AssignHandle {
                session: h,
                hub_handle: hub,
            }),
            SessionResult::NoOutput
        );
        while c.poll_event().is_some() {}
    }
    (ha, hb)
}

fn lock(c: &Mutex<NoiseWireProtection>) -> MutexGuard<'_, NoiseWireProtection> {
    c.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Uç görevine verilen dikiş: kripto test gövdesinden de erişilebilir kalır (durum parmak izi,
/// sahte datagramın ön-koşul denetimi).
struct Shared(Arc<Mutex<NoiseWireProtection>>);

impl PacketProtection for Shared {
    fn seal(&mut self, ctx: ProtectionContext, frame: &[u8]) -> Result<SealedUnit, SealError> {
        lock(&self.0).seal(ctx, frame)
    }
    fn open(&mut self, wire: &[u8]) -> Option<Opened> {
        lock(&self.0).open(wire)
    }
    fn sealed_len(&self, ctx: ProtectionContext, plaintext_len: usize) -> Result<usize, SealError> {
        lock(&self.0).sealed_len(ctx, plaintext_len)
    }
    fn conservative_limits(&self, class: ProtectionClass) -> ProtectionLimits {
        lock(&self.0).conservative_limits(class)
    }
    fn limits(&self, ctx: ProtectionContext) -> ProtectionLimits {
        lock(&self.0).limits(ctx)
    }
}

/// Anahtarsız sahte HubChannel datagramı: `çerçeve ‖ 0×16` — saldırgan anahtar bilmez, çerçevenin
/// tipini de kendisi seçer. Ön-koşul (vakum değil): alıcının kriptosu onu GERÇEKTEN HubChannel
/// olarak ve çerçeve değişmeden açar (durumsuz yol; hiçbir state ilerlemez).
fn forged(receiver: &Mutex<NoiseWireProtection>, frame: &[u8]) -> Vec<u8> {
    let mut wire = frame.to_vec();
    wire.extend_from_slice(&[0u8; HUB_NOOP_TAG]);
    let opened = lock(receiver).open(&wire).expect("HubChannel açılır");
    assert_eq!(opened.ctx, ProtectionContext::HubChannel);
    assert_eq!(opened.frame, frame, "HubChannel çerçeveyi değiştirdi");
    wire
}

/// `crypto`'nun `h` oturumunda mühürlediği (o oturumun tutamacıyla, AEAD'li) datagram.
fn sealed(crypto: &mut NoiseWireProtection, h: SessionHandle, frame: &[u8]) -> Vec<u8> {
    crypto
        .seal(ProtectionContext::Session(h), frame)
        .expect("mühür")
        .bytes
}

/// `crypto`'nun `h` oturumunda mühürlediği DATA çerçevesi (birim `unit`).
fn sealed_data(crypto: &mut NoiseWireProtection, h: SessionHandle, unit: u64) -> Vec<u8> {
    let frame = encode_data(0, unit, format!("birim-{unit}").as_bytes());
    sealed(crypto, h, &frame)
}

/// `pn`'i onaylayan düzgün biçimli ACK_V2 çerçevesi (alıcının kullandığı codec).
fn ack_frame(pn: u64) -> Vec<u8> {
    AckFrame::from_received(&[pn], 0, 0, 1, ARRIVAL_RATE_SENTINEL)
        .expect("boş olmayan küme")
        .encode()
}

/// Bu dosyanın düzeneklerinde `not_ready_drops` daima 0'dır (hazır olmayan oturum yok); INV
/// testi o sayacı ayrıca okur.
fn stats(ctx_mismatch_drops: u64, hub_frames_dropped: u64) -> CtxDispatchStats {
    CtxDispatchStats {
        ctx_mismatch_drops,
        hub_frames_dropped,
        not_ready_drops: 0,
    }
}

/// Canlı sayaç `want` olana dek bekler; olmazsa kırmızı.
async fn wait_for(read: impl Fn() -> CtxDispatchStats, want: CtxDispatchStats) {
    let deadline = Instant::now() + WAIT;
    loop {
        let got = read();
        if got == want {
            return;
        }
        assert!(Instant::now() < deadline, "{want:?} olmadı, son: {got:?}");
        tokio::time::sleep(Duration::from_millis(2)).await;
    }
}

async fn udp() -> UdpSocket {
    UdpSocket::bind("127.0.0.1:0").await.expect("UDP soketi")
}

/// `deadline`'a dek bir datagram bekler.
async fn recv_until(sock: &UdpSocket, deadline: Instant) -> Option<(Vec<u8>, SocketAddr)> {
    let mut buf = vec![0u8; 2048];
    let (n, from) = tokio::time::timeout_at(deadline, sock.recv_from(&mut buf))
        .await
        .ok()?
        .ok()?;
    buf.truncate(n);
    Some((buf, from))
}

async fn send(sock: &UdpSocket, wire: &[u8], to: SocketAddr) {
    sock.send_to(wire, to).await.expect("gönder");
}

// ── İstemci ucu ──────────────────────────────────────────────────────────────────────────────

/// Alice'in `Session(h_ab)` ucu (gerçek `ClientEndpoint`, ürün profili) + Bob'un veri adresini
/// taklit eden sahte eş soketi. Session datagramlarını Bob (beklenen oturum) ve Carol (Alice'in
/// BAŞKA bir oturumu) mühürler.
struct ClientRig {
    alice: Arc<Mutex<NoiseWireProtection>>,
    bob: NoiseWireProtection,
    carol: NoiseWireProtection,
    /// Bob'daki Alice↔Bob tutamacı.
    h_ba: SessionHandle,
    /// Carol'daki Alice↔Carol tutamacı.
    h_ca: SessionHandle,
    peer: UdpSocket,
    client: ClientHandle,
    /// Uç soketinin adresi (sahte eşin hedefi).
    to: SocketAddr,
    /// Birim → ilk DATA emisyonunun PN'i (Alice mühürledi; ARQ kimliği).
    pn: BTreeMap<u64, u64>,
    /// Uçtan gelen PARAM_SET'in özeti ve şimdiye dek görülen PARAM_SET sayısı.
    param_hash: [u8; 32],
    param_sets: usize,
}

impl ClientRig {
    /// İki birim uçuşta (DATA'ları görüldü), PARAM henüz onaysız (uç PARAM_SET yineliyor).
    async fn new() -> Self {
        let mut alice = node(ALICE);
        let mut bob = node(BOB);
        let mut carol = node(CAROL);
        let (h_ab, h_ba) = handshake(&mut alice, &mut bob, id(BOB), HUB_AB);
        let (h_ac, h_ca) = handshake(&mut alice, &mut carol, id(CAROL), HUB_AC);
        assert_ne!(h_ab, h_ac, "Alice'in iki oturumu ayrı tutamaç taşır");
        let alice = Arc::new(Mutex::new(alice));

        let peer = udp().await;
        let cfg = RuntimeConfig {
            drain_cap: Duration::from_millis(100),
            ..RuntimeConfig::default()
        };
        let client = ClientEndpoint::connect_session(
            peer.local_addr().expect("eş adresi"),
            cfg,
            Box::new(Shared(Arc::clone(&alice))),
            None,
            ProtectionContext::Session(h_ab),
        )
        .await
        .expect("uç");
        // Kurtarma kapalı (yalnız test yapılandırması): ARQ retransmit / kopya / parity tele
        // çıkmaz → PARAM onaylanınca uç HİÇ mühürlemez; Crypto parmak izi kararlı kalır.
        client.set_recovery(false);
        client.send(b"birim-0".to_vec()).expect("sun");
        client.send(b"birim-1".to_vec()).expect("sun");

        let mut pn = BTreeMap::new();
        let mut param_hash = None;
        let mut param_sets = 0;
        let mut to = None;
        let deadline = Instant::now() + WAIT;
        while pn.len() < 2 || param_hash.is_none() {
            let (wire, from) = recv_until(&peer, deadline)
                .await
                .expect("iki DATA + PARAM_SET gelmeli");
            to = Some(from);
            let od = bob.open(&wire).expect("Bob açar");
            if let Some((_, unit, _)) = decode_data(&od.frame) {
                pn.entry(unit).or_insert(od.meta.full_wire_pn);
            } else if let Some((_, _, hash)) = decode_param(&od.frame) {
                param_hash = Some(hash);
                param_sets += 1;
            }
        }
        Self {
            alice,
            bob,
            carol,
            h_ba,
            h_ca,
            peer,
            client,
            to: to.expect("uç adresi"),
            pn,
            param_hash: param_hash.expect("PARAM_SET"),
            param_sets,
        }
    }

    async fn send(&self, wire: &[u8]) {
        send(&self.peer, wire, self.to).await;
    }

    /// Bob'un Alice↔Bob oturumunda mühürlediği datagram (beklenen tutamaç).
    fn bob_wire(&mut self, frame: &[u8]) -> Vec<u8> {
        sealed(&mut self.bob, self.h_ba, frame)
    }

    /// Carol'ın Alice↔Carol oturumunda mühürlediği GEÇERLİ datagram (başka oturumun tutamacı).
    fn carol_wire(&mut self, frame: &[u8]) -> Vec<u8> {
        sealed(&mut self.carol, self.h_ca, frame)
    }

    /// Alice'in Crypto durumu (oturum, pencere, sayaç, olay, demux; anahtar baytı YOK).
    fn alice_fp(&self) -> String {
        lock(&self.alice).state_fingerprint()
    }

    /// Sahte eşe `wait` boyunca gelenleri alır; PARAM_SET'leri sayar.
    async fn absorb(&mut self, wait: Duration) {
        let deadline = Instant::now() + wait;
        while let Some((wire, _)) = recv_until(&self.peer, deadline).await {
            let od = self.bob.open(&wire).expect("Bob açar");
            if decode_param(&od.frame).is_some() {
                self.param_sets += 1;
            }
        }
    }

    /// Pozitif kontrol: beklenen bağlamda doğru özetli PARAM_ACK → PARAM_SET yinelemesi durur
    /// (`PARAM_WINDOW` boyunca hiç gelmez). Bundan sonra uç sessizdir (kurtarma kapalı, yeni birim
    /// yok). Onayın kaynağı kapanışta `assert_session_clean` ile ayrıca doğrulanır.
    async fn confirm_param(&mut self) {
        let frame = encode_param_ack(&self.param_hash);
        let ack = self.bob_wire(&frame);
        self.send(&ack).await;
        let deadline = Instant::now() + WAIT;
        loop {
            let before = self.param_sets;
            self.absorb(PARAM_WINDOW).await;
            if self.param_sets == before {
                return;
            }
            assert!(Instant::now() < deadline, "PARAM_ACK onaylamadı");
        }
    }

    /// `seen`'den sonra yeni bir PARAM_SET gelene dek bekler (onaysız uç yineler); yoksa kırmızı.
    async fn expect_param_set_after(&mut self, seen: usize) {
        let deadline = Instant::now() + WAIT;
        while self.param_sets <= seen {
            assert!(Instant::now() < deadline, "PARAM_SET yinelemesi durdu");
            self.absorb(Duration::from_millis(50)).await;
        }
    }

    async fn assert_no_ack(&mut self, why: &str) {
        let got = tokio::time::timeout(QUIET, self.client.acked()).await;
        assert!(got.is_err(), "{why}: beklenmeyen teyit {got:?}");
    }

    async fn next_ack(&mut self) -> Option<u64> {
        tokio::time::timeout(WAIT, self.client.acked())
            .await
            .ok()
            .flatten()
    }

    async fn expect_ctx(&self, want: CtxDispatchStats) {
        let client = &self.client;
        wait_for(|| client.ctx_dispatch_stats(), want).await;
    }

    /// Kapanış: sayaçların son hâli + uç istatistiği.
    async fn finish(self) -> (CtxDispatchStats, ClientStats) {
        let ctx = self.client.ctx_dispatch_stats();
        (ctx, self.client.shutdown().await)
    }
}

/// PARAM bağı yalnız beklenen bağlamdaki PARAM_ACK ile onaylandı; tanınmayan çerçeve ve açma reddi
/// yok (düşen her datagram AÇILDI — düşüş kriptodan değil, bağlamdan).
fn assert_session_clean(cs: &ClientStats) {
    assert_eq!(cs.param_confirmed, 1);
    assert_eq!((cs.unknown_frames, cs.open_failures), (0, 0));
}

/// Kart 5(a). ESKİ DAVRANIŞ: istemci ACK'i bağlamına bakmadan işliyordu → birim 0 ARQ deposundan
/// silinir, bir daha gönderilmezdi (mesaj sessizce ölür).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_hub_channel_ack_cannot_remove_an_inflight_unit() {
    let mut rig = ClientRig::new().await;
    rig.confirm_param().await;
    let ack = ack_frame(rig.pn[&0]);
    let attack = forged(&rig.alice, &ack);
    let fp = rig.alice_fp();

    rig.send(&attack).await;
    rig.expect_ctx(stats(0, 1)).await;
    // Ayırt edici kanıt: ARQ deposu ve teyit akışı değişmedi.
    rig.assert_no_ack("HubChannel ACK").await;
    assert_eq!(rig.client.outstanding(), 2, "birim ARQ deposunda kalmalı");
    // Ön-koşul (AYIRT EDİCİ DEĞİL — `open_hub` durumsuz): saldırı kripto durumuna dokunmaz.
    assert_eq!(rig.alice_fp(), fp, "ön-koşul: kripto durumu değişti");

    // Aynı ACK baytları beklenen bağlamda: birim silinir (çerçeve geçerliydi, düşüş bağlamdandı).
    let real = rig.bob_wire(&ack);
    rig.send(&real).await;
    assert_eq!(rig.next_ack().await, Some(0));
    assert_eq!(rig.client.outstanding(), 1);
    let (ctx, cs) = rig.finish().await;
    assert_eq!(ctx, stats(0, 1));
    assert_eq!(cs.acked, 1, "yalnız beklenen bağlamın ACK'i silmeli");
    assert_session_clean(&cs);
}

/// Kart 5(b) — RECOVERED: birim 0'ı (blok 0 · ESI 0; her K'da) FEC-kurtarılmış bildiren çerçeve.
/// ESKİ DAVRANIŞ: birim deposundan düşer ve "teslim edildi" sayılırdı.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn b_hub_channel_recovered_cannot_remove_an_inflight_unit() {
    let mut rig = ClientRig::new().await;
    rig.confirm_param().await;
    let recovered = encode_recovered(0, &[0]);
    let attack = forged(&rig.alice, &recovered);
    let fp = rig.alice_fp();

    rig.send(&attack).await;
    rig.expect_ctx(stats(0, 1)).await;
    // Ayırt edici kanıt: ARQ deposu ve teyit akışı değişmedi.
    rig.assert_no_ack("HubChannel RECOVERED").await;
    assert_eq!(rig.client.outstanding(), 2, "birim ARQ deposunda kalmalı");
    // Ön-koşul (AYIRT EDİCİ DEĞİL — `open_hub` durumsuz): saldırı kripto durumuna dokunmaz.
    assert_eq!(rig.alice_fp(), fp, "ön-koşul: kripto durumu değişti");

    let real = rig.bob_wire(&recovered);
    rig.send(&real).await;
    assert_eq!(rig.next_ack().await, Some(0));
    let (ctx, cs) = rig.finish().await;
    assert_eq!(ctx, stats(0, 1));
    // Yalnız beklenen bağlamdaki RECOVERED birimi kapattı.
    assert_eq!((cs.fec_recovered_ctrl_units, cs.acked), (1, 0));
    assert_session_clean(&cs);
}

/// Kart 5(b) — CS_ECHO: HubChannel'daki echo RTT/echo yoluna girmez. ESKİ DAVRANIŞ: sayılır ve
/// motora `CsProbeEcho` olarak beslenirdi (bootstrap açıkken RTT örneği = kuruluş kararı).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn b_hub_channel_cs_echo_is_not_dispatched() {
    let mut rig = ClientRig::new().await;
    rig.confirm_param().await;
    let echo = encode_cs_echo(CS_PROBE_SUBTYPE, 0, 0, 0);
    let attack = forged(&rig.alice, &echo);
    let fp = rig.alice_fp();

    rig.send(&attack).await;
    rig.expect_ctx(stats(0, 1)).await;
    // Ayırt edici kanıt: ARQ deposu değişmedi; `cs_echoes_rx` kapanışta yalnız pozitif
    // kontrolü sayar.
    assert_eq!(rig.client.outstanding(), 2);
    // Ön-koşul (AYIRT EDİCİ DEĞİL — `open_hub` durumsuz): saldırı kripto durumuna dokunmaz.
    assert_eq!(rig.alice_fp(), fp, "ön-koşul: kripto durumu değişti");

    // Aynı echo beklenen bağlamda, ardından bariyer ACK: aynı soketten sırayla işlenir, teyit
    // geldiğinde echo da işlenmiştir.
    let real_echo = rig.bob_wire(&echo);
    rig.send(&real_echo).await;
    let pn0 = rig.pn[&0];
    let barrier = rig.bob_wire(&ack_frame(pn0));
    rig.send(&barrier).await;
    assert_eq!(rig.next_ack().await, Some(0));
    let (ctx, cs) = rig.finish().await;
    assert_eq!(ctx, stats(0, 1));
    assert_eq!(cs.cs_echoes_rx, 1, "yalnız beklenen bağlamın echo'su");
    assert_session_clean(&cs);
}

/// Kart 5(b) — PARAM_ACK: doğru özetle HubChannel'da → onay SAYILMAZ, PARAM_SET yinelemesi sürer.
/// ESKİ DAVRANIŞ: PARAM bağı onaylanır, yineleme durardı.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn b_hub_channel_param_ack_does_not_confirm_params() {
    let mut rig = ClientRig::new().await;
    let param_ack = encode_param_ack(&rig.param_hash);
    let attack = forged(&rig.alice, &param_ack);

    rig.send(&attack).await;
    rig.expect_ctx(stats(0, 1)).await;
    // Sahte çerçeve işlendi; o ana dek mühürlenmiş olanlar alındıktan SONRA yeni PARAM_SET gelmeli.
    rig.absorb(Duration::from_millis(50)).await;
    let seen = rig.param_sets;
    rig.expect_param_set_after(seen).await;

    // Aynı baytlar beklenen bağlamda: onaylar, yineleme durur.
    rig.confirm_param().await;
    assert_eq!(rig.client.outstanding(), 2);
    let (ctx, cs) = rig.finish().await;
    assert_eq!(ctx, stats(0, 1));
    assert_session_clean(&cs);
}

/// Kart 5(c): başka bir oturumun tutamacıyla GEÇERLİ Session datagramı (Carol'ın AEAD'li ACK'i) bu
/// ucun ARQ deposuna giremez. ESKİ DAVRANIŞ: ACK, BAŞKA oturumun PN uzayındaki numarayla bu ucun
/// birimini silerdi.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn c_valid_datagram_of_another_session_is_dropped() {
    let mut rig = ClientRig::new().await;
    rig.confirm_param().await;
    let ack = ack_frame(rig.pn[&0]);
    let wrong = rig.carol_wire(&ack);

    rig.send(&wrong).await;
    rig.expect_ctx(stats(1, 0)).await;
    rig.assert_no_ack("başka oturumun ACK'i").await;
    assert_eq!(rig.client.outstanding(), 2, "birim ARQ deposunda kalmalı");

    let real = rig.bob_wire(&ack);
    rig.send(&real).await;
    assert_eq!(rig.next_ack().await, Some(0));
    let (ctx, cs) = rig.finish().await;
    assert_eq!(ctx, stats(1, 0));
    assert_eq!(cs.acked, 1);
    // Carol'ın datagramı AÇILDI (AEAD geçerli; `open_failures == 0`): düşüş bağlamdan.
    assert_session_clean(&cs);
}

/// Kart 5(d): pozitif kol — beklenen tutamaçla gelen gerçek ACK birimi siler, sayaçlar sıfır kalır.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn d_session_ack_with_the_expected_handle_still_removes_the_unit() {
    let mut rig = ClientRig::new().await;
    rig.confirm_param().await;
    let pn0 = rig.pn[&0];
    let real = rig.bob_wire(&ack_frame(pn0));

    rig.send(&real).await;
    assert_eq!(rig.next_ack().await, Some(0));
    assert_eq!(rig.client.outstanding(), 1);
    let (ctx, cs) = rig.finish().await;
    assert_eq!(ctx, stats(0, 0));
    assert_eq!(cs.acked, 1);
    assert_session_clean(&cs);
}

// ── Sunucu ucu ───────────────────────────────────────────────────────────────────────────────

async fn server_expecting(
    crypto: &Arc<Mutex<NoiseWireProtection>>,
    expected: ExpectedSessions,
) -> ServerHandle {
    ServerEndpoint::bind_expecting(
        SocketAddr::from(([127, 0, 0, 1], 0)),
        RuntimeConfig::default(),
        Box::new(Shared(Arc::clone(crypto))),
        None,
        expected,
        CtxDispatchCounters::new(),
    )
    .await
    .expect("veri alıcısı")
}

async fn delivery(server: &mut ServerHandle) -> Delivery {
    tokio::time::timeout(WAIT, server.next_delivery())
        .await
        .expect("teslim zaman aşımı")
        .expect("teslim kanalı kapandı")
}

async fn server_ctx(server: &ServerHandle, want: CtxDispatchStats) {
    wait_for(|| server.ctx_dispatch_stats(), want).await;
}

/// Sunucu ucu: mevcut bir Session yuvasının adresinden gelen anahtarsız HubChannel DATA ve aynısı
/// yeni bir adresten. ESKİ DAVRANIŞ: ilki yuvanın bağlamıyla (`Session(h)`) S1'e teslim edilirdi
/// (PN 0 burada yolda kaybolmuştur); ikincisi HubChannel bağlamlı bir yuva açar ve ACK döndürürdü.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn server_hub_channel_data_cannot_enter_an_existing_session_slot() {
    let mut alice = node(ALICE);
    let mut bob = node(BOB);
    let (h_ab, h_ba) = handshake(&mut alice, &mut bob, id(BOB), HUB_AB);
    let alice = Arc::new(Mutex::new(alice));
    let expected = ExpectedSessions::only(h_ab);
    let mut server = server_expecting(&alice, expected).await;
    let to = server.local_addr();
    let bob_ep = udp().await;

    // Bob'un PN 0'lı datagramı yolda kaybolur (hostile ağda olağan): yuva PN 0'ı hiç görmez.
    let _lost = sealed_data(&mut bob, h_ba, 9);
    let first = sealed_data(&mut bob, h_ba, 1);
    send(&bob_ep, &first, to).await;
    let d = delivery(&mut server).await;
    assert_eq!((d.unit_id, d.ctx), (1, ProtectionContext::Session(h_ab)));

    // Saldırı: Bob'un yuvasının adresinden anahtarsız HubChannel DATA (no-op PN = 0).
    let attack = forged(&alice, &encode_data(0, 2, b"sahte"));
    send(&bob_ep, &attack, to).await;
    server_ctx(&server, stats(0, 1)).await;

    // Aynısı yeni bir adresten: yuva AÇILMAZ, yanıt (ACK) dönmez.
    let stranger = udp().await;
    let attack = forged(&alice, &encode_data(0, 3, b"sahte"));
    send(&stranger, &attack, to).await;
    server_ctx(&server, stats(0, 2)).await;
    let reply = recv_until(&stranger, Instant::now() + QUIET).await;
    assert!(reply.is_none(), "HubChannel yuva açtı (yanıt döndü)");

    // Bariyer: Bob'un sıradaki Session DATA'sı. Teslim sırası 1 → 4; sahte 2 ve 3 YOK.
    let barrier = sealed_data(&mut bob, h_ba, 4);
    send(&bob_ep, &barrier, to).await;
    let d = delivery(&mut server).await;
    assert_eq!((d.unit_id, d.ctx), (4, ProtectionContext::Session(h_ab)));
    assert!(server.try_next_delivery().is_none());
    assert_eq!(server.ctx_dispatch_stats(), stats(0, 2));
    let ss = server.shutdown().await;
    assert_eq!(ss.delivered, 2);
    assert_eq!((ss.open_failures, ss.unknown_frames), (0, 0));
}

/// Sunucu ucu: `Session(h)` için `h` oturum tablosunda beklenmeli ve eş yuvasının oturumu olmalı.
/// ESKİ DAVRANIŞ: Carol'ın geçerli datagramı Bob'un yuvasında Bob'un bağlamıyla teslim edilir
/// (oturumlar arası karışma), tabloda olmayan oturum ilk datagramıyla yuva açardı.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn server_session_context_must_be_expected_and_bound_to_the_slot() {
    let mut alice = node(ALICE);
    let mut bob = node(BOB);
    let mut carol = node(CAROL);
    let (h_ab, h_ba) = handshake(&mut alice, &mut bob, id(BOB), HUB_AB);
    let (h_ac, h_ca) = handshake(&mut alice, &mut carol, id(CAROL), HUB_AC);
    let alice = Arc::new(Mutex::new(alice));
    let expected = ExpectedSessions::only(h_ab);
    let mut server = server_expecting(&alice, expected.clone()).await;
    let to = server.local_addr();
    let bob_ep = udp().await;
    let carol_ep = udp().await;

    // Bob'un yuvası kurulur.
    let wire = sealed_data(&mut bob, h_ba, 0);
    send(&bob_ep, &wire, to).await;
    let d = delivery(&mut server).await;
    assert_eq!((d.unit_id, d.ctx), (0, ProtectionContext::Session(h_ab)));

    // Yuva tek oturuma aittir: Carol'ın GEÇERLİ datagramı Bob'un yuvasının adresinden → düşer.
    let wire = sealed_data(&mut carol, h_ca, 1);
    send(&bob_ep, &wire, to).await;
    server_ctx(&server, stats(1, 0)).await;

    // Oturum tablosunda olmayan oturum, yeni adresten: düşer, yuva AÇILMAZ (TOFU yok).
    let wire = sealed_data(&mut carol, h_ca, 2);
    send(&carol_ep, &wire, to).await;
    server_ctx(&server, stats(2, 0)).await;
    let reply = recv_until(&carol_ep, Instant::now() + QUIET).await;
    assert!(reply.is_none(), "beklenmeyen oturum yuva açtı");

    // Oturum tablosu Carol oturumunu yayımlar → aynı adresten sıradaki datagram kabul edilir.
    expected.insert_ready(h_ac);
    let wire = sealed_data(&mut carol, h_ca, 3);
    send(&carol_ep, &wire, to).await;
    let d = delivery(&mut server).await;
    assert_eq!((d.unit_id, d.ctx), (3, ProtectionContext::Session(h_ac)));

    // Yuva bağı korunur: Carol artık beklense de Bob'un yuvası onu kabul etmez.
    let wire = sealed_data(&mut carol, h_ca, 4);
    send(&bob_ep, &wire, to).await;
    server_ctx(&server, stats(3, 0)).await;

    // Bariyer: Bob'un sıradaki datagramı. Teslim sırası 0 → 3 → 5; 1, 2, 4 YOK.
    let wire = sealed_data(&mut bob, h_ba, 5);
    send(&bob_ep, &wire, to).await;
    let d = delivery(&mut server).await;
    assert_eq!((d.unit_id, d.ctx), (5, ProtectionContext::Session(h_ab)));
    assert!(server.try_next_delivery().is_none());
    let ss = server.shutdown().await;
    assert_eq!(ss.delivered, 3);
    // Carol'ın düşen datagramları AÇILDI (AEAD geçerli): düşüş kriptodan değil, bağlamdan.
    assert_eq!((ss.open_failures, ss.unknown_frames), (0, 0));
}

/// `max_peers` DoS regresyonu (plan Ek (3) madde 2): eş tablosu TEK yuvalıyken anahtarsız
/// (HubChannel) datagram ve beklenmeyen oturumun GEÇERLİ datagramı taze soketlerden yuva AÇAMAZ;
/// ardından meşru eşin beklenen oturum DATA'sı teslim edilir ve `dropped_new_peers == 0`.
///
/// 6be4ff44'te KIRMIZI: bağlam kapısı yokken açılan her datagram yeni adres için yuva yaratıyordu
/// (TOFU) — ilk anahtarsız DATA tek yuvayı HubChannel bağlamıyla alır ve kendi birimini (7) teslim
/// ettirir; Carol'ın ve Bob'un datagramları `dropped_new_peers`'a düşer (2), Bob'un birimi hiç
/// teslim edilmez → ilk teslim iddiası tutmaz. Gövde ve yardımcıları yalnız 6be4ff44'te de bulunan
/// API'leri kullanır (`ServerEndpoint::bind`, `ServerStats`, `next_delivery`); o koda taşınınca
/// iddia koşturularak doğrulanabilir.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn server_max_peers_one_is_not_taken_by_unkeyed_or_unexpected_datagrams() {
    let mut alice = node(ALICE);
    let mut bob = node(BOB);
    let mut carol = node(CAROL);
    let (h_ab, h_ba) = handshake(&mut alice, &mut bob, id(BOB), HUB_AB);
    let (h_ac, h_ca) = handshake(&mut alice, &mut carol, id(CAROL), HUB_AC);
    // Bağımsız `bind` örtük PoC oturumunu (tutamaç 0) bekler; taze Alice'in ilk oturumu odur.
    assert_eq!(h_ab, SessionHandle::from_raw(0));
    assert_ne!(h_ac, h_ab);
    let alice = Arc::new(Mutex::new(alice));
    let cfg = RuntimeConfig {
        max_peers: 1,
        ..RuntimeConfig::default()
    };
    let mut server = ServerEndpoint::bind(
        SocketAddr::from(([127, 0, 0, 1], 0)),
        cfg,
        Box::new(Shared(Arc::clone(&alice))),
    )
    .await
    .expect("veri alıcısı");
    let to = server.local_addr();

    // Taze soket 1: anahtarsız HubChannel DATA. Taze soket 2: beklenmeyen oturumun GEÇERLİ DATA'sı.
    let unkeyed = udp().await;
    let attack = forged(&alice, &encode_data(0, 7, b"sahte"));
    send(&unkeyed, &attack, to).await;
    let unexpected = udp().await;
    let wire = sealed_data(&mut carol, h_ca, 8);
    send(&unexpected, &wire, to).await;

    // Meşru eş: beklenen oturumun DATA'sı. Aynı sunucu soketine en son varır ve sırayla işlenir.
    let bob_ep = udp().await;
    let wire = sealed_data(&mut bob, h_ba, 0);
    send(&bob_ep, &wire, to).await;
    let d = delivery(&mut server).await;
    assert_eq!((d.unit_id, d.ctx), (0, ProtectionContext::Session(h_ab)));
    let ss = server.shutdown().await;
    assert_eq!(ss.dropped_new_peers, 0, "meşru eş reddedildi");
    assert_eq!(ss.delivered, 1);
    // Saldırı datagramları AÇILDI (`open_failures == 0`): düşüş kriptodan değil, bağlam kapısından.
    assert_eq!((ss.open_failures, ss.unknown_frames), (0, 0));
}

// ── INV-ACK-DELIVER-1 ────────────────────────────────────────────────────────────────────────

/// Gerçek S1 adaptörü (`ClientTransport`, ürün builder'ı) + bu dosyanın kriptosu + `MemoryHub`.
fn transport_node(me: u8, hub: &MemoryHub) -> ClientTransport<NoiseWireProtection> {
    let mut cfg = ClientTransportConfig::product(SocketAddr::from(([127, 0, 0, 1], 0)));
    cfg.runtime.drain_cap = Duration::from_millis(100);
    ClientTransport::new(cfg, node(me), Box::new(hub.link(id(me))))
        .expect("düğüm")
}

fn unit(bytes: &[u8]) -> PreparedUnit {
    PreparedUnit {
        bytes: bytes.to_vec(),
        compression_policy: CompressionPolicy::Never,
    }
}

/// `cond` doğru olana dek bekler (gerçek zaman: ARQ yeniden gönderimi gerçek saatle işler).
fn eventually(what: &str, mut cond: impl FnMut() -> bool) {
    let deadline = std::time::Instant::now() + RETX_WAIT;
    while !cond() {
        assert!(std::time::Instant::now() < deadline, "{what}");
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// INV-ACK-DELIVER-1 — teslim yoluna kabul edilmeyen birim ACK'lenmez.
///
/// Belirlenimli (`MemoryHub` rölesi elle sürülür): Alice başlatır, Bob yanıtlar. Röle 1 INIT'i
/// Bob'a, HANDLE_ASSIGN'ı Alice'e taşır → Alice'in bekleyen oturumu epoch-0 için kurulur (0-RTT;
/// S1'e açılmadı → `NotReady`); Bob `Ready` olur, RESPONSE hub'da TUTULUR. Bob'un erken DATA'sı
/// (epoch-0) Alice'in veri alıcısında açılır ama motora girmeden, ACK'siz düşer:
/// `not_ready_drops ≥ 1`, birim Bob'da teyitsiz kalır. Röle 2 RESPONSE'u taşır → Alice `Ready` →
/// Bob'un ARQ yeniden gönderimi teslim edilir; kayıp yok. ESKİ DAVRANIŞ (#114'ün ilk hâli):
/// bekleyen tutamaç hazırlık ayrımı olmadan beklenen kümedeydi → datagram ACK'lenir, teslim
/// kapısı onu `Ready` olmayan oturum diye atardı → Bob teyit alır, birim hiç teslim edilmez.
#[test]
fn inv_ack_deliver_unit_of_a_not_ready_session_is_not_acked() {
    let hub = MemoryHub::new();
    let mut alice = transport_node(ALICE, &hub);
    let mut bob = transport_node(BOB, &hub);
    alice.add_peer_addr(id(BOB), bob.data_addr());
    bob.add_peer_addr(id(ALICE), alice.data_addr());

    let Ok(ConnectResult::Pending(_)) = alice.connect(id(BOB), None) else {
        panic!("bağlantı beklemede başlamalı");
    };
    // Röle 1: INIT → Bob, HANDLE_ASSIGN → Alice. RESPONSE henüz yok.
    assert_eq!(hub.relay(), 1);
    alice.drive();
    bob.drive();
    let h_bob = bob.active_session_for_test(id(ALICE)).expect("Bob Ready");
    assert!(alice.active_session_for_test(id(BOB)).is_none());

    // RESPONSE hub'da TUTULURKEN Bob yollar.
    bob.send(h_bob, unit(b"erken"), DeliveryPolicy::Reliable)
        .expect("gönder");
    eventually("Ready olmayan oturumun datagramı ACK'siz düşmeli", || {
        alice.ctx_dispatch_stats().not_ready_drops >= 1
    });
    std::thread::sleep(QUIET);
    assert_eq!(bob.outstanding_for_test(), 1, "Ready öncesi ACK yok");
    assert!(alice.recv().is_none(), "Ready öncesi teslim yok");

    // Röle 2: RESPONSE → Alice → Ready; Bob'un yeniden gönderimi teslim edilir.
    assert_eq!(hub.relay(), 1);
    let mut delivered = None;
    eventually("Ready sonrası yeniden gönderim teslim edilmeli", || {
        delivered = alice.recv();
        delivered.is_some()
    });
    let delivered = delivered.expect("teslim");
    assert_eq!(delivered.unit, b"erken");
    assert_eq!(
        alice.active_session_for_test(id(BOB)),
        Some(delivered.session)
    );
    eventually("Ready sonrası teyit gelmeli", || {
        bob.outstanding_for_test() == 0
    });
    assert!(alice.recv().is_none(), "tek teslim");
    let ctx = alice.ctx_dispatch_stats();
    assert!(ctx.not_ready_drops >= 1);
    assert_eq!((ctx.ctx_mismatch_drops, ctx.hub_frames_dropped), (0, 0));
}
