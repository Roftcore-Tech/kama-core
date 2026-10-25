//! **K4G-05b — payload sahipliği: runtime yalnız motor teyidiyle siler** (CEVAP 13 §3, 11-EK §3).
//!
//! Envanter: `docs/design/K4G-05b-RUNTIME-ENVANTER.md`. Motorun bir iletim kaydını unuttuğu her
//! yolda (tombstone TTL'i, TOMBSTONE_MAX tahliyesi, kopya kökenli kayıp) o kaydın GEÇ ACK'i
//! motorda teyit üretmez. main'de (`1b1425d2`) runtime aynı ACK'te payload'ı siliyordu: sonraki
//! yeniden gönderim gövdesiz (store-miss) çıkar, haritalar sızar. Bu dosya gerçek runtime ile
//! (`ClientEndpoint` / `ServerEndpoint`, UDP loopback) her yolu AYRI sınar.
//!
//! AĞ: istemci ile sunucu arasında bir röle (gerçek UDP). `IdentitySeam` düz metin taşıdığından
//! röle çerçeveyi okur ama yalnız ağın yapabildiğini yapar: datagramı geçirir, düşürür ya da
//! geciktirir. Çerçeve değiştirilmez, üretilmez. "Geç ACK" = alıcının ürettiği gerçek ACK
//! datagramının ağda tutulup sonradan istemciye verilmesidir. Sonraki ACK'ler o PN'i yeniden
//! bildirmesin diye alıcının ACK aralığı sınırı kullanılır (`MAX_ACK_RANGES`): hedeften sonra
//! tek sayılı dolgu birimlerinin İLK datagramı düşer. Alıcının alım kümesi parçalanır ve hedefin
//! bloğu ACK çerçevesinin dışında kalır (`AckFrame::from_received`, gerçek alıcı davranışı).
//!
//! Her yol için beş iddia:
//! 1. Geç ACK birimi teyit etmez (`acked()` olayı yok).
//! 2. Payload yerinde: birim teyitsiz depoda.
//! 3. Sonraki yeniden gönderim payload'la çıkar: alıcıya giden gövde birimin gerçek gövdesidir ve
//!    `stale_decisions == 0`.
//! 4. O gönderimin ACK'i birimi teyit eder, payload düşer.
//! 5. Runtime haritaları boştur (sızıntı yok).
//!
//! TABAN (main `1b1425d2`): iddia 1 KIRMIZI beklenir (ESI testinde faz 2, bekçilerde store-miss).
//! İddia 5'in kancası (`SenderRecoveryDiag::{seq_to_unit_len, unit_seqs_len}`) main'de yoktur;
//! taban kolu, `assert_runtime_maps_empty` gövdesi boş bir kopyayla koşar.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::net::SocketAddr;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use adaptive_transport::ack::{AckFrame, FRAME_TYPE_ACK, FRAME_TYPE_ACK_V2, MAX_ACK_RANGES};
use client_core::runtime::{
    ClientEndpoint, ClientHandle, ClientStats, CtxDispatchCounters, ExpectedSessions,
    FecRuntimeConfig, RuntimeConfig, RuntimeError, SenderRecoveryDiag, ServerEndpoint,
    ServerHandle, ServerStats,
};
use client_core::seam::{IdentitySeam, IDENTITY_TAG};
use client_core::wire::{decode_data, decode_fec};
use tokio::net::UdpSocket;
use tokio::sync::mpsc;

// ── Sabitler ─────────────────────────────────────────────────────────────────────────────────

/// Genel bekleme tavanı: loopback'te beklenen olayın ÜST sınırı; aşılırsa kırmızı.
const WAIT: Duration = Duration::from_secs(10);
/// "Gelmedi" iddialarının penceresi.
const QUIET: Duration = Duration::from_millis(300);
/// Yoklama adımı.
const STEP: Duration = Duration::from_millis(1);
/// Kapanış boşaltmasının tavanı.
const DRAIN: Duration = Duration::from_millis(500);
/// Yeniden gönderim bekleme tavanı: kopya ve prob geri çekilmesinin üst sınırı RTO_MAX = 30 s
/// (transport-layer-tdd AMENDMENT-12) + pay.
const RETX_WAIT: Duration = Duration::from_secs(45);
/// Tombstone TTL'inin ALT sınırı: TTL = K_TOMB × T_loss, T_loss ≥ K_c × EAI, EAI ≥ 65 B / 31 B·s⁻¹
/// (`sender.rs` v1 sabitleri K_TOMB = 8, CADENCE_KC = 3/2, EAI_SACK_WIRE_BYTES = 65,
/// EAI_RETURN_BW_PRIOR = 31; 05b değiştirmedi) ⇒ 8 · 3/2 · 65/31 s = 25 161 ms. Yalnız ön koşul
/// denetimidir: TTL testinde kayıt bundan ERKEN düştüyse yol TTL değildir; tahliye testinde
/// bundan GEÇ düştüyse yol tahliye değildir.
const TTL_LOWER_BOUND: Duration = Duration::from_millis(8 * 3 * 65_000 / (2 * 31));
/// TTL bekleme tavanı: alt sınırın birkaç katı (T_loss'un hız terimi loopback'te küçüktür).
const TTL_WAIT: Duration = Duration::from_secs(120);
/// Tahliye bekleme tavanı (tahliye TTL alt sınırından önce olmalıdır; pay ön koşul denetimindedir).
/// ~~Tahliye kolunda geç ACK öncesi trafik boşaltmasının da bütçesidir~~ [K4G-05c] boşaltma bütçesi
/// artık `CAP_DRAIN_WAIT`'tir: tahliye edilen kayıtların geç ACK'i teyit etmez, birimler ek bir
/// yeniden gönderim turuyla teyit edilir (11-EK §4'ün beklenen bedeli; 05b ölçümü: 10 s'de 1/2 koşu
/// kırmızı, 300 s bütçede adım 6,2 s sürdü).
const CAP_WAIT: Duration = Duration::from_secs(60);
/// [K4G-05c] Tahliye kolunda geç ACK öncesi trafik boşaltmasının bütçesi (`CAP_WAIT` tahliye
/// beklemesi olarak ayrı kalır). 05c fikstürü, mesafe kapısı içinde her dolgunun ilk iki iletimini
/// düşürür: tek yönlü iki kayıplı dolgu, ARQ tavanı ve kopya sahipliği uzun bir yeniden gönderim
/// kuyruğu doğurur — ölçülen boşaltma 101,9 s (1 koşu, 900 s bütçeyle; run2 `teshis-cap900.log`),
/// 60 s bütçede 3/3 kırmızı. Yalnız üst sınırdır (koşul `outstanding == 1`); iddialar değişmedi.
const CAP_DRAIN_WAIT: Duration = Duration::from_secs(300);
/// Motorun tombstone tavanı (`sender.rs` TOMBSTONE_MAX = 4096, v1; 05b değiştirmedi).
const TOMBSTONE_CAP: u32 = 4096;
/// Bir yoklama turunda sunulan tahliye dolgusu (teklif kuyruğu 1024'ün altında).
const CAP_BATCH: u64 = 64;
/// Alıcının bir ACK çerçevesinde bildirdiği blok sayısı: en üst blok + `MAX_ACK_RANGES` aralık.
const ACK_BLOCKS: u64 = MAX_ACK_RANGES as u64 + 1;
/// Parçalama dolgusu: hedefin bloğunu ACK çerçevesinin dışına itmek için alıcıda hedefin üstünde
/// `ACK_BLOCKS`'tan fazla blok gerekir; her tek sayılı dolgu bir boşluk açar (pay: 4 blok).
const FRAG_FILLERS: u64 = 2 * (ACK_BLOCKS + 4);
/// Tahliye dolgusu: her birinin İLK İKİ datagramı düşer (orijinal + ilk ARQ yeniden gönderimi) →
/// iki kayıp ilanı → iki tombstone; dolgu başına iki tombstone, tavanın yarısı + pay birimle.
/// [K4G-05c] Eski hâl (`TOMBSTONE_CAP + 4 × ACK_BLOCKS` dolgu, birim başına bir tombstone) hedefin
/// ARDINDAN 4096'dan fazla birim sunuyordu; 05c'nin gönderen mesafe kapısı (en yeni − en eski
/// teyitsiz kimlik < `max_inflight_units` = 4096) teyitsiz hedeften 4095 kimlik ötesini almadığı
/// için fazlası teklif kuyruğunda kalıyordu (ölçüm: `outstanding = 112`, motorda teyitsiz 1). Yol
/// ve iddialar aynı; dolgu pencerenin içinde kalır (ön koşul testte denetlenir).
const CAP_FILLERS: u64 = TOMBSTONE_CAP as u64 / 2 + 2 * ACK_BLOCKS;
/// Bekleme boyunca trafik aralığı. Alıcı ACK_FREQ_N = 2 ile ilerleme ≈ 2 × bu aralıkta gelir; bu,
/// tail-recovery PTO'sunun alt sınırının (`max_ack_delay` 25 ms) altındadır. Böylece hedefin
/// kaydı prob emekliliğiyle çoğalmaz (birim başına kayıt sınırına ulaşmaz) ve kopyaları kayıp
/// ilanıyla düşer — TTL testinde kaydı TTL'den başka bir yol düşürmez.
const TRICKLE: Duration = Duration::from_millis(5);
/// FEC kolları: profil 0x03 (K = 8, R_max = 4, E = 64; `kama_fec_codec` katalogu).
const FEC_PROFILE: u8 = 0x03;
/// Statik parite sayısı = R_max: parite ESI'leri 0..R, kaynak birim 0..R ile ad çakışır.
const PARITY_R: u16 = 4;
/// FEC kolunda birim tavanı: 48 + SOURCE_PREFIX_LEN 11 ≤ E 64.
const FEC_UNIT_BYTES: usize = 48;
/// ESI testinde bloğun tamamlanma tavanı (PRE_CONVERGE kopya takvimi: kuruluş probu 0.5 s'den
/// katlanır; kuruluştan sonra zaman eşiği ≥ 3.15 s).
const ESI_WAIT: Duration = Duration::from_secs(30);
/// Bekçi: teyitten sonra motorun kalan kayıtlarının akıbetinin belirlendiği pencere — zaman
/// eşiği kayıp ilanının tabanı (3/2 × EAI ≥ 3.15 s) + pay.
const SETTLE: Duration = Duration::from_secs(5);
/// Bekçi kanalı: birim sayısı, bekleme tavanı, tohum ve olasılıklar (milyonda).
const GUARD_UNITS: u64 = 400;
const GUARD_WAIT: Duration = Duration::from_secs(60);
const GUARD_SEED: u64 = 0x4B34_4730_3562;
const GUARD_DATA_DROP_PPM: u64 = 200_000;
const GUARD_DATA_DELAY_PPM: u64 = 100_000;
const GUARD_ACK_DROP_PPM: u64 = 100_000;
/// Bekçi gecikmesi: paket eşiği kayıp ilanından (loopback'te ms) çok uzun, tombstone TTL'inden
/// (≥ 25 s) çok kısa → geç orijinal, yeniden gönderimden SONRA ve kaydı hâlâ canlıyken gelir.
const GUARD_DELAY: Duration = Duration::from_millis(300);
/// Bekçi FEC kolunda kalıcı silinen birimler (DATA'ları retransmit dahil hep düşer).
const GUARD_ERASED: [u64; 2] = [2, 5];
/// Knuth MMIX LCG (belirlenimli sözde-rastgele; yalnız test kanalı).
const LCG_MUL: u64 = 6_364_136_223_846_793_005;
const LCG_INC: u64 = 1_442_695_040_888_963_407;
const PPM: u64 = 1_000_000;
/// `IdentitySeam` etiketinde PN'in kapladığı önek (u64 BE; seam.rs S4).
const PN_PREFIX: usize = 8;
/// Rölenin gecikme kuyruğu tick'i ve gecikmiş datagram enjeksiyon kuyruğu.
const RELAY_TICK: Duration = Duration::from_millis(1);
const INJECT_QUEUE: usize = 64;
/// UDP datagram tamponu.
const DATAGRAM_MAX: usize = 65_536;

/// Birime özgü, SABİT boylu gövde: alıcıdaki bayt eşliği içerikle doğrulanır; boy eşitliği
/// PRE_CONVERGE kuruluş probunun hedef seçimini belirlenimli kılar (eşit vade → önce sunulan).
fn body(unit: u64) -> Vec<u8> {
    format!("k4g05b-birim-{unit:06}").into_bytes()
}

/// FEC kapalı, canlı ayna açık (teşhis aynası salt gözlemdir; ürün varsayılanı kapalı).
fn cfg_no_fec() -> RuntimeConfig {
    RuntimeConfig {
        fec: None,
        drain_cap: DRAIN,
        live_diag: true,
        ..RuntimeConfig::default()
    }
}

/// Statik-r FEC (Manual): parite her blokta R adet, belirlenimli.
fn cfg_fec_static() -> RuntimeConfig {
    RuntimeConfig {
        max_unit_bytes: FEC_UNIT_BYTES,
        fec: Some(FecRuntimeConfig {
            profile_id: FEC_PROFILE,
            manual_r: Some(PARITY_R),
        }),
        drain_cap: DRAIN,
        live_diag: true,
        ..RuntimeConfig::default()
    }
}

/// Profil 0x03'ün blok boyu K (katalogdan).
fn block_k() -> u64 {
    u64::from(
        kama_fec_codec::by_id(FEC_PROFILE)
            .expect("profil 0x03 katalogda")
            .k_shape,
    )
}

/// İddia 5 — runtime haritaları boş (sızıntı yok).
///
/// [TABAN] `SenderRecoveryDiag::{seq_to_unit_len, unit_seqs_len}` main'de (`1b1425d2`) yoktur;
/// taban kopyasında YALNIZ bu gövde boştur.
fn assert_runtime_maps_empty(r: &SenderRecoveryDiag, path: &str) {
    assert_eq!(
        (r.seq_to_unit_len, r.unit_seqs_len),
        (0, 0),
        "[{path}] (5) teyitsiz birim kalmadığında runtime haritaları boş olmalı: {r:?}"
    );
}

/// Hedefin rölede geçen iletiminin motor kaydı düştü mü: hedef teyitsiz depodaki en eski birimdir
/// ve motorun o birim için tuttuğu en eski kayıt (uçuş, tombstone, kayıp kuyruğu), o iletimin
/// rölede görüldüğü andan SONRA gönderilmiştir — ya da birimin hiç kaydı kalmamıştır.
fn record_gone(r: &SenderRecoveryDiag, origin: Instant, unit: u64, passed_at: Instant) -> bool {
    let Some(u) = r.oldest_unacked.filter(|u| u.unit_id == unit) else {
        return false;
    };
    u.engine
        .earliest_record_send_ns
        .is_none_or(|ns| origin + Duration::from_nanos(ns) > passed_at)
}

// ── Ağ (röle) ────────────────────────────────────────────────────────────────────────────────

/// `IdentitySeam` teli: `çerçeve ‖ etiket`, etiketin ilk 8 baytı PN (BE). Döner: (çerçeve, PN).
fn split_wire(wire: &[u8]) -> Option<(&[u8], u64)> {
    let cut = wire.len().checked_sub(IDENTITY_TAG)?;
    let pn = u64::from_be_bytes(wire.get(cut..cut + PN_PREFIX)?.try_into().ok()?);
    Some((&wire[..cut], pn))
}

/// Hedef DIŞI DATA datagramlarının kapısı.
#[derive(Debug, Clone, Default)]
enum DataGate {
    #[default]
    All,
    Nothing,
    Only(BTreeSet<u64>),
    Except(BTreeSet<u64>),
}

impl DataGate {
    fn admits(&self, unit: u64) -> bool {
        match self {
            Self::All => true,
            Self::Nothing => false,
            Self::Only(units) => units.contains(&unit),
            Self::Except(units) => !units.contains(&unit),
        }
    }
}

/// Bekçi kanalı: belirlenimli sözde-rastgele kayıp ve gecikme.
#[derive(Debug, Clone)]
struct Chaos {
    state: u64,
    data_drop_ppm: u64,
    data_delay_ppm: u64,
    delay: Duration,
    ack_drop_ppm: u64,
}

impl Chaos {
    fn roll(&mut self, ppm: u64) -> bool {
        self.state = self.state.wrapping_mul(LCG_MUL).wrapping_add(LCG_INC);
        (self.state >> 33) % PPM < ppm
    }
}

/// İleri yön kararı.
enum Forward {
    Pass,
    Drop,
    Delay(Instant),
}

/// Rölenin kuralları (test yazar) ve gözlemleri (röle yazar).
#[derive(Default)]
struct NetState {
    // ── İleri yön (istemci → sunucu) kuralları
    /// Hedef birim: geç ACK'i üretilen birim.
    target: Option<u64>,
    /// Hedefin YALNIZ bu sıradaki (0 tabanlı) DATA datagramı alıcıya geçer, diğerleri düşer.
    target_pass_nth: u32,
    /// Faz B: hedefin bütün datagramları geçer (ve gövdeleri kaydedilir).
    target_open: bool,
    /// İlk DATA datagramı düşen birimler (orijinal kaybolur, yeniden gönderimler geçer).
    drop_first: BTreeSet<u64>,
    /// [K4G-05c] İlk İKİ DATA datagramı düşen birimler (orijinal ve ilk yeniden gönderim kaybolur,
    /// sonrakiler geçer) — tahliye kolu.
    drop_first_two: BTreeSet<u64>,
    /// Hedef dışı DATA kapısı.
    data_gate: DataGate,
    /// Bekçi kanalı (hedef dışı DATA ve bütün ACK'ler).
    chaos: Option<Chaos>,
    // ── Dönüş yönü (sunucu → istemci) kuralları
    /// Hedefin geçen datagramını bildiren ACK datagramları gecikme hattına alınır.
    hold_target_acks: bool,
    /// Bütün dönüş datagramları tutulur (PRE_CONVERGE'i uzatmak için).
    hold_all: bool,
    // ── Gözlemler
    max_fwd_pn: Option<u64>,
    data_seen: BTreeMap<u64, u32>,
    data_passed: BTreeMap<u64, u32>,
    /// Hedefin alıcıya geçen datagramının PN'i ve rölede görüldüğü an.
    hold_pn: Option<u64>,
    target_passed_at: Option<Instant>,
    /// Faz B'de alıcıya geçen hedef gövdeleri.
    target_after_open: Vec<Vec<u8>>,
    /// Hedefin PN'ini bildiren, ağda tutulan ACK datagramları (geliş sırasıyla).
    held: Vec<Vec<u8>>,
    /// `hold_all` süresince tutulan dönüş datagramları.
    pre_held: Vec<Vec<u8>>,
    parity_pns: BTreeSet<u64>,
    acks_forwarded: u64,
    acks_with_all_parity: u64,
    delayed: VecDeque<(Instant, Vec<u8>)>,
}

impl NetState {
    fn on_forward(&mut self, wire: &[u8], now: Instant) -> Forward {
        let Some((frame, pn)) = split_wire(wire) else {
            return Forward::Pass;
        };
        self.max_fwd_pn = Some(self.max_fwd_pn.map_or(pn, |m| m.max(pn)));
        if decode_fec(frame).is_some() {
            self.parity_pns.insert(pn);
            return Forward::Pass;
        }
        let Some((_, unit, payload)) = decode_data(frame) else {
            return Forward::Pass; // PARAM / CS-probe: kontrol düzlemi geçer.
        };
        let seen = self.data_seen.entry(unit).or_insert(0);
        let nth = *seen;
        *seen += 1;
        if self.target == Some(unit) {
            if self.target_open {
                self.target_after_open.push(payload.to_vec());
                return Forward::Pass;
            }
            if nth == self.target_pass_nth {
                self.hold_pn = Some(pn);
                self.target_passed_at = Some(now);
                return Forward::Pass;
            }
            return Forward::Drop;
        }
        if nth == 0 && self.drop_first.contains(&unit) {
            return Forward::Drop;
        }
        if nth < 2 && self.drop_first_two.contains(&unit) {
            return Forward::Drop;
        }
        if !self.data_gate.admits(unit) {
            return Forward::Drop;
        }
        if let Some(c) = self.chaos.as_mut() {
            let (drop_ppm, delay_ppm, delay) = (c.data_drop_ppm, c.data_delay_ppm, c.delay);
            if c.roll(drop_ppm) {
                return Forward::Drop;
            }
            if c.roll(delay_ppm) {
                return Forward::Delay(now + delay);
            }
        }
        *self.data_passed.entry(unit).or_insert(0) += 1;
        Forward::Pass
    }

    /// `true` = istemciye geçir.
    fn on_back(&mut self, wire: &[u8]) -> bool {
        let Some((frame, _)) = split_wire(wire) else {
            return true;
        };
        let is_ack = frame
            .first()
            .is_some_and(|&t| t == FRAME_TYPE_ACK || t == FRAME_TYPE_ACK_V2);
        let acked: Vec<u64> = if is_ack {
            AckFrame::decode(frame)
                .map(|f| f.acked_full(self.max_fwd_pn.map_or(0, |m| m + 1)))
                .unwrap_or_default()
        } else {
            Vec::new()
        };
        if self.hold_target_acks && self.hold_pn.is_some_and(|p| acked.contains(&p)) {
            self.held.push(wire.to_vec());
            return false;
        }
        if self.hold_all {
            self.pre_held.push(wire.to_vec());
            return false;
        }
        if let Some(c) = self.chaos.as_mut() {
            let ppm = c.ack_drop_ppm;
            if is_ack && c.roll(ppm) {
                return false;
            }
        }
        if is_ack {
            self.acks_forwarded += 1;
            if !self.parity_pns.is_empty() && self.parity_pns.iter().all(|p| acked.contains(p)) {
                self.acks_with_all_parity += 1;
            }
        }
        true
    }

    fn take_due(&mut self, now: Instant) -> Vec<Vec<u8>> {
        let mut due = Vec::new();
        while self.delayed.front().is_some_and(|(at, _)| *at <= now) {
            if let Some((_, wire)) = self.delayed.pop_front() {
                due.push(wire);
            }
        }
        due
    }
}

fn lock(m: &Mutex<NetState>) -> MutexGuard<'_, NetState> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

/// İstemci ↔ röle ↔ sunucu. İstemci rölenin ön adresine bağlanır; röle arka soketiyle sunucuya.
struct Net {
    front: SocketAddr,
    state: Arc<Mutex<NetState>>,
    inject: mpsc::Sender<Vec<u8>>,
}

impl Net {
    async fn start(server: SocketAddr) -> Self {
        let front = UdpSocket::bind("127.0.0.1:0")
            .await
            .expect("röle ön soketi");
        let back = UdpSocket::bind("127.0.0.1:0")
            .await
            .expect("röle arka soketi");
        back.connect(server).await.expect("röle → sunucu");
        let addr = front.local_addr().expect("röle adresi");
        let state = Arc::new(Mutex::new(NetState::default()));
        let (inject, rx) = mpsc::channel(INJECT_QUEUE);
        tokio::spawn(relay(front, back, Arc::clone(&state), rx));
        Self {
            front: addr,
            state,
            inject,
        }
    }

    fn with<R>(&self, f: impl FnOnce(&mut NetState) -> R) -> R {
        f(&mut lock(&self.state))
    }

    /// Ağda tutulmuş bir datagramı istemciye şimdi verir (gecikmiş datagram).
    async fn deliver_late(&self, wire: Vec<u8>) {
        self.inject.send(wire).await.expect("röle açık");
    }
}

/// Röle döngüsü. Kilit hiçbir `await` boyunca tutulmaz. Enjeksiyon kanalı kapanınca biter.
async fn relay(
    front: UdpSocket,
    back: UdpSocket,
    state: Arc<Mutex<NetState>>,
    mut inject: mpsc::Receiver<Vec<u8>>,
) {
    let mut client: Option<SocketAddr> = None;
    let mut up = vec![0u8; DATAGRAM_MAX];
    let mut down = vec![0u8; DATAGRAM_MAX];
    let mut tick = tokio::time::interval(RELAY_TICK);
    loop {
        tokio::select! {
            r = front.recv_from(&mut up) => {
                let Ok((n, from)) = r else { continue };
                client = Some(from);
                let verdict = lock(&state).on_forward(&up[..n], Instant::now());
                match verdict {
                    Forward::Pass => {
                        let _ = back.send(&up[..n]).await;
                    }
                    Forward::Drop => {}
                    Forward::Delay(at) => {
                        let wire = up[..n].to_vec();
                        lock(&state).delayed.push_back((at, wire));
                    }
                }
            }
            r = back.recv(&mut down) => {
                let Ok(n) = r else { continue };
                let pass = lock(&state).on_back(&down[..n]);
                if let (true, Some(to)) = (pass, client) {
                    let _ = front.send_to(&down[..n], to).await;
                }
            }
            _ = tick.tick() => {
                let due = lock(&state).take_due(Instant::now());
                for wire in due {
                    let _ = back.send(&wire).await;
                }
            }
            m = inject.recv() => {
                let Some(wire) = m else { break };
                if let Some(to) = client {
                    let _ = front.send_to(&wire, to).await;
                }
            }
        }
    }
}

// ── Düzenek ──────────────────────────────────────────────────────────────────────────────────

struct Rig {
    client: ClientHandle,
    server: ServerHandle,
    net: Net,
    next_unit: u64,
    /// İstemcinin `acked()` olayları (sırasıyla) ve kümesi.
    acked: Vec<u64>,
    acked_set: BTreeSet<u64>,
    /// Sunucunun ilk teslimleri (birim → gövde).
    delivered: BTreeMap<u64, Vec<u8>>,
    /// [K4G-05c] İstemcinin gönderen mesafe kapısı (`max_inflight_units`): en eski teyitsiz
    /// birimden bu kadar kimlik ötesi sunulsa da gönderilmez (teklif kuyruğunda bekler).
    window: u64,
}

impl Rig {
    async fn new(cfg: RuntimeConfig) -> Self {
        let server = ServerEndpoint::bind(
            "127.0.0.1:0".parse().expect("adres"),
            cfg.clone(),
            Box::new(IdentitySeam::new()),
        )
        .await
        .expect("sunucu");
        Self::with_server(server, cfg).await
    }

    /// [K4G-05c] OTURUM kipli veri alıcısı (`bind_expecting`; S1 kabul kuralı: oturum penceresi +
    /// teslim kanalında sıra başı rezervi). `IdentitySeam`'in tek oturumu beklenir ve hazırdır.
    async fn new_session_keyed(server_cfg: RuntimeConfig, client_cfg: RuntimeConfig) -> Self {
        let server = ServerEndpoint::bind_expecting(
            "127.0.0.1:0".parse().expect("adres"),
            server_cfg,
            Box::new(IdentitySeam::new()),
            None,
            ExpectedSessions::only(IdentitySeam::SESSION),
            CtxDispatchCounters::new(),
        )
        .await
        .expect("sunucu");
        Self::with_server(server, client_cfg).await
    }

    async fn with_server(server: ServerHandle, cfg: RuntimeConfig) -> Self {
        let net = Net::start(server.local_addr()).await;
        let window = cfg.max_inflight_units as u64;
        let client = ClientEndpoint::connect(net.front, cfg, Box::new(IdentitySeam::new()))
            .await
            .expect("istemci");
        Self {
            client,
            server,
            net,
            next_unit: 0,
            acked: Vec::new(),
            acked_set: BTreeSet::new(),
            delivered: BTreeMap::new(),
            window,
        }
    }

    /// [K4G-05c] Sıradaki birim gönderen mesafe kapısının içinde mi: `next_unit − O < window`, O =
    /// testin gördüğü en eski teyitsiz birim (teyit olayı geç gelebilir → O gerçeğinden küçük ya da
    /// eşit; koşul ihtiyatlıdır).
    fn next_in_window(&self) -> bool {
        let oldest = (0..self.next_unit)
            .find(|u| !self.acked_set.contains(u))
            .unwrap_or(self.next_unit);
        self.next_unit - oldest < self.window
    }

    /// Olay akışlarını `d` boyunca boşaltır (teyit kanalı taşmasın, teslim kanalı dolmasın).
    async fn pump(&mut self, d: Duration) {
        let until = tokio::time::Instant::now() + d;
        loop {
            tokio::select! {
                () = tokio::time::sleep_until(until) => return,
                Some(u) = self.client.acked() => {
                    self.acked.push(u);
                    self.acked_set.insert(u);
                }
                Some(dl) = self.server.next_delivery() => {
                    self.delivered.entry(dl.unit_id).or_insert(dl.payload);
                }
            }
        }
    }

    /// [K4G-05c] Teslimleri OKUMADAN yalnız teyit akışını `d` boyunca boşaltır: uygulama (tüketici)
    /// durur, veri alıcısının teslim kanalı dolar.
    async fn pump_acks_only(&mut self, d: Duration) {
        let until = tokio::time::Instant::now() + d;
        loop {
            tokio::select! {
                () = tokio::time::sleep_until(until) => return,
                Some(u) = self.client.acked() => {
                    self.acked.push(u);
                    self.acked_set.insert(u);
                }
            }
        }
    }

    /// Bir birim sunar (dolu kuyrukta boşaltıp yeniden dener); birim kimliğini döner. Fragmentasyon
    /// kapalı: birim kimliği sunum sırasıdır.
    async fn send(&mut self) -> u64 {
        let unit = self.next_unit;
        let deadline = Instant::now() + WAIT;
        loop {
            match self.client.send(body(unit)) {
                Ok(()) => break,
                Err(RuntimeError::QueueFull) if Instant::now() < deadline => {
                    self.pump(STEP).await;
                }
                Err(e) => panic!("birim {unit} sunulamadı: {e}"),
            }
        }
        self.next_unit += 1;
        unit
    }

    async fn wait(&mut self, what: &str, limit: Duration, mut done: impl FnMut(&Self) -> bool) {
        let deadline = Instant::now() + limit;
        loop {
            if done(self) {
                return;
            }
            assert!(
                Instant::now() < deadline,
                "{what}: {limit:?} içinde olmadı — {}",
                self.describe()
            );
            self.pump(STEP).await;
        }
    }

    /// `wait` + bekleme boyunca `TRICKLE` aralığıyla trafik (geçen birimler).
    ///
    /// [K4G-05c] Trafik gönderen mesafe kapısının içinde kalır (`next_in_window`): teyitsiz hedef
    /// uzun süre beklerken (TTL kolu ≥ 25 s, faz B ≤ RTO_MAX) birim sayısı makine hızına göre 4096'yı
    /// aşabilir; aşan birimler teklif kuyruğunda kalır ve "yalnız hedef teyitsiz" boşaltması hiç
    /// sağlanmazdı. Pencere doluyken birim sunulmaz; hedefin kaydını TTL'den başka bir yol yine
    /// düşürmez (orijinal K_MAP budamasından muaf, tahliye için tombstone sayısı yetmez).
    async fn wait_trickle(
        &mut self,
        what: &str,
        limit: Duration,
        mut done: impl FnMut(&Self) -> bool,
    ) {
        let deadline = Instant::now() + limit;
        let mut next = Instant::now();
        loop {
            if done(self) {
                return;
            }
            assert!(
                Instant::now() < deadline,
                "{what}: {limit:?} içinde olmadı — {}",
                self.describe()
            );
            if Instant::now() >= next && self.next_in_window() {
                self.send().await;
                next = Instant::now() + TRICKLE;
            }
            self.pump(STEP).await;
        }
    }

    /// Şu andan SONRA alınmış kurtarma görüntüsü (görev `RECOVERY_DIAG_PERIOD`'da bir yeniler).
    async fn fresh_recovery(&mut self) -> SenderRecoveryDiag {
        let since = Instant::now();
        let mut got = None;
        self.wait("taze kurtarma görüntüsü", WAIT, |r| {
            got = r.client.live_diag().recovery.filter(|x| x.taken_at > since);
            got.is_some()
        })
        .await;
        got.expect("koşul sağlandı")
    }

    /// Kırmızı mesajı için durum özeti (gövde baytı içermez).
    fn describe(&self) -> String {
        let live = self.client.live_diag();
        let net = lock(&self.net.state);
        format!(
            "outstanding={} son_teyitler={:?} rtt_samples={} retransmits={} dups={} \
             recovery={:?} hedef={:?} hedef_görülen={:?} hedef_pn={:?} tutulan_ack={} \
             ön_tutulan={} parite_pn={} ack_geçen={}",
            self.client.outstanding(),
            self.acked.iter().rev().take(8).collect::<Vec<_>>(),
            live.rtt_samples,
            live.retransmits,
            live.dups,
            live.recovery,
            net.target,
            net.target.and_then(|t| net.data_seen.get(&t).copied()),
            net.hold_pn,
            net.held.len(),
            net.pre_held.len(),
            net.parity_pns.len(),
            net.acks_forwarded,
        )
    }

    async fn finish(self) -> (ClientStats, ServerStats) {
        let Self { client, server, .. } = self;
        let cs = client.shutdown().await;
        let ss = server.shutdown().await;
        (cs, ss)
    }

    /// TTL ve tahliye kollarının ortak kurulumu: kuruluş birimi W teyitli; hedef T'nin YALNIZ ilk
    /// datagramı alıcıya geçer ve onu bildiren ACK'ler ağda tutulur; parçalama dolgusunun tek sayılı
    /// birimlerinin ilk datagramı düşer → alıcıda T'nin bloğu ACK çerçevesinin dışına itilir, ACK
    /// akışı (T hariç) sürer ve T'nin orijinali kayıp ilan edilir. Döner: T.
    async fn target_with_held_ack(&mut self) -> u64 {
        let w = self.send().await;
        self.wait("kuruluş birimi teyit edilmeli", WAIT, |r| {
            r.acked_set.contains(&w)
        })
        .await;
        let t = self.next_unit;
        let fillers = (t + 1)..=(t + FRAG_FILLERS);
        self.net.with(|s| {
            s.target = Some(t);
            s.target_pass_nth = 0;
            s.hold_target_acks = true;
            s.drop_first.extend(fillers.clone().filter(|u| u % 2 == 1));
        });
        assert_eq!(self.send().await, t, "fikstür: hedef kimliği");
        for _ in fillers {
            self.send().await;
        }
        self.wait("dolgu teyit edilmeli (yalnız hedef teyitsiz)", WAIT, |r| {
            r.client.outstanding() == 1
        })
        .await;
        let want = body(t);
        self.wait("alıcı hedefi orijinalinden teslim etmeli", WAIT, |r| {
            r.delivered.get(&t).is_some_and(|b| *b == want)
        })
        .await;
        self.net.with(|s| {
            assert!(s.hold_pn.is_some(), "ön koşul: hedefin ilk datagramı geçti");
            assert!(
                !s.held.is_empty(),
                "ön koşul: hedefi bildiren ACK ağda tutuldu"
            );
        });
        t
    }

    /// Faz A (geç ACK) + faz B (yeniden gönderim): beş iddia. Ağda tutulan İLK ACK (hedefin PN'ini
    /// ilk bildiren datagram) istemciye şimdi verilir. `drain_budget`: geç ACK öncesi trafik
    /// boşaltmasının yol başına bütçesi (yalnız bütçe; iddialar yoldan bağımsız).
    async fn late_ack_keeps_payload_then_resend_confirms(
        mut self,
        t: u64,
        path: &str,
        drain_budget: Duration,
    ) {
        let drain_started = Instant::now();
        self.wait(
            "trafik teyit edilmeli (yalnız hedef teyitsiz)",
            drain_budget,
            |r| r.client.outstanding() == 1,
        )
        .await;
        eprintln!(
            "[{path}] geç ACK öncesi trafik boşaltması: {:?} (bütçe {drain_budget:?})",
            drain_started.elapsed()
        );
        let late = self
            .net
            .with(|s| {
                s.hold_target_acks = false;
                s.held.first().cloned()
            })
            .expect("ağda tutulan geç ACK");
        let mark = self.acked.len();
        self.net.deliver_late(late).await;
        // Bariyer: sonra sunulan birimin teyidi geldiğinde geç ACK işlenmiştir (aynı soket, sıra).
        let barrier = self.send().await;
        self.wait("bariyer birimi teyit edilmeli", WAIT, |r| {
            r.acked_set.contains(&barrier)
        })
        .await;
        self.pump(QUIET).await;

        // (1) Geç ACK teyit üretmez.
        assert!(
            !self.acked[mark..].contains(&t),
            "[{path}] (1) geç ACK hedefi TEYİT ETMEMELİ — motor o kaydı unutmuştu; geç ACK \
             sonrası olaylar: {:?}",
            &self.acked[mark..]
        );
        assert_eq!(
            self.client.outstanding(),
            1,
            "[{path}] (1) hedef teyitsiz kalmalı"
        );
        // (2) Payload yerinde.
        let rec = self.fresh_recovery().await;
        assert_eq!(
            (rec.unacked_units, rec.oldest_unacked.map(|u| u.unit_id)),
            (1, Some(t)),
            "[{path}] (2) payload teyitsiz depoda kalmalı: {rec:?}"
        );

        // Faz B: hedefin sonraki iletimleri alıcıya geçer.
        self.net.with(|s| s.target_open = true);
        self.wait_trickle(
            &format!("[{path}] (4) yeniden gönderimin ACK'i hedefi teyit etmeli"),
            RETX_WAIT,
            |r| r.acked_set.contains(&t),
        )
        .await;
        // (3) Yeniden gönderim gövdeli.
        let lens = self.net.with(|s| {
            let want = body(t);
            (
                s.target_after_open.iter().map(Vec::len).collect::<Vec<_>>(),
                s.target_after_open.iter().all(|b| *b == want),
            )
        });
        assert!(
            !lens.0.is_empty() && lens.1,
            "[{path}] (3) yeniden gönderim birimin gerçek gövdesini taşımalı (store-miss yok): \
             gövde boyları {:?}, beklenen {}",
            lens.0,
            body(t).len()
        );
        // (4) Teyit sonrası payload düştü; (5) haritalar boş.
        self.wait("trafik teyit edilmeli", WAIT, |r| {
            r.client.outstanding() == 0
        })
        .await;
        let rec = self.fresh_recovery().await;
        assert_eq!(
            (rec.unacked_units, rec.oldest_unacked.map(|u| u.unit_id)),
            (0, None),
            "[{path}] (4) teyitten sonra payload düşmeli: {rec:?}"
        );
        assert_runtime_maps_empty(&rec, path);

        let total = self.next_unit;
        let (cs, _ss) = self.finish().await;
        assert_eq!(
            cs.stale_decisions, 0,
            "[{path}] (3) gövdesiz (store-miss) emisyon olmamalı: {cs:?}"
        );
        assert_eq!(cs.undelivered, 0, "[{path}] {cs:?}");
        assert_eq!(cs.acked, total, "[{path}] her birim bir kez teyit: {cs:?}");
    }
}

// ── Testler: yol başına bir ──────────────────────────────────────────────────────────────────

/// Envanter satır 1 — **tombstone TTL'i**. Hedefin orijinali alıcıya ulaşır, ACK'i ağda tutulur;
/// motor orijinali kayıp ilan eder (tombstone) ve TTL (K_TOMB × T_loss ≥ 25.2 s) dolunca kaydı
/// unutur. Sonra tutulan ACK gelir. main: runtime payload'ı siler + `acked()` → (1) KIRMIZI.
#[tokio::test(flavor = "multi_thread")]
async fn late_ack_after_ttl_keeps_payload() {
    let mut rig = Rig::new(cfg_no_fec()).await;
    let t = rig.target_with_held_ack().await;
    let passed_at = rig
        .net
        .with(|s| s.target_passed_at)
        .expect("hedefin orijinali geçti");
    let mut gone_at = None;
    rig.wait_trickle(
        "hedefin orijinal kaydı tombstone TTL'iyle düşmeli",
        TTL_WAIT,
        |r| {
            let live = r.client.live_diag();
            match (live.clock_origin, live.recovery) {
                (Some(origin), Some(rec)) if record_gone(&rec, origin, t, passed_at) => {
                    gone_at = Some(rec.taken_at);
                    true
                }
                _ => false,
            }
        },
    )
    .await;
    let gone = gone_at.expect("koşul sağlandı").duration_since(passed_at);
    assert!(
        gone >= TTL_LOWER_BOUND,
        "ön koşul: kayıt TTL alt sınırından ({TTL_LOWER_BOUND:?}) önce düştü ({gone:?}) — yol \
         TTL değil (birim başına kayıt budaması ya da tahliye?)"
    );
    rig.late_ack_keeps_payload_then_resend_confirms(t, "ttl", WAIT)
        .await;
}

/// Envanter satır 2 — **TOMBSTONE_MAX tahliyesi**. Kurulum TTL koluyla aynı; ardından her birinin
/// ilk iki iletimi düşen ≈ 2048 dolgu birimi bir TTL içinde 4096+ tombstone üretir ve en eski kayıt
/// (hedefin orijinali) tavandan tahliye edilir. Sonra tutulan ACK gelir.
///
/// [K4G-05c] Fikstür, gönderen mesafe kapısının (hedef teyitsizken en çok hedef + 4095 kimlik)
/// İÇİNDE kalacak biçimde yeniden kuruldu: tombstone'lar birim sayısıyla değil aynı birimlerin
/// tekrarlanan kayıplarıyla üretilir (bkz. `CAP_FILLERS`). Yol (tahliye), ön koşullar (TTL alt
/// sınırından önce düşüş, tablo tavana ulaştı) ve beş iddia DEĞİŞMEDİ.
#[tokio::test(flavor = "multi_thread")]
async fn late_ack_after_cap_keeps_payload() {
    let mut rig = Rig::new(cfg_no_fec()).await;
    let t = rig.target_with_held_ack().await;
    let passed_at = rig
        .net
        .with(|s| s.target_passed_at)
        .expect("hedefin orijinali geçti");
    let first = rig.next_unit;
    let end = first + CAP_FILLERS;
    let window = RuntimeConfig::default().max_inflight_units as u64;
    assert!(
        (end - 1) - t < window,
        "ön koşul: dolgu gönderen mesafe kapısının içinde kalmalı (son kimlik {} − hedef {t} <          max_inflight_units {window})",
        end - 1
    );
    rig.net.with(|s| s.drop_first_two.extend(first..end));
    let deadline = Instant::now() + CAP_WAIT;
    let mut max_tombs = 0u32;
    let gone_at = loop {
        let live = rig.client.live_diag();
        if let (Some(origin), Some(rec)) = (live.clock_origin, live.recovery) {
            max_tombs = max_tombs.max(rec.timers.tombstones_len);
            if record_gone(&rec, origin, t, passed_at) {
                break rec.taken_at;
            }
        }
        assert!(
            Instant::now() < deadline,
            "hedefin orijinal kaydı tahliye edilmeli ({CAP_WAIT:?}; en büyük tombstone \
             {max_tombs}) — {}",
            rig.describe()
        );
        let mut batch = 0;
        while rig.next_unit < end && batch < CAP_BATCH {
            rig.send().await;
            batch += 1;
        }
        rig.pump(STEP).await;
    };
    let gone = gone_at.duration_since(passed_at);
    assert!(
        gone < TTL_LOWER_BOUND,
        "ön koşul: kayıt TTL alt sınırından ({TTL_LOWER_BOUND:?}) önce düşmeli (tahliye); düştü: \
         {gone:?}"
    );
    assert!(
        max_tombs >= TOMBSTONE_CAP,
        "ön koşul: tombstone tablosu tavana ({TOMBSTONE_CAP}) ulaşmalı; görülen {max_tombs}"
    );
    while rig.next_unit < end {
        rig.send().await;
    }
    rig.late_ack_keeps_payload_then_resend_confirms(t, "cap", CAP_DRAIN_WAIT)
        .await;
}

/// Envanter satır 3 — **kopya kökenli kayıp** (kayıt tombstone bırakmadan düşer). Hedef ilk
/// birimdir; RTT örneği yokken (PRE_CONVERGE) motorun tek yeniden gönderim yolu kopya
/// makinesidir, dolayısıyla hedefin İKİNCİ iletimi bir kopyadır (kuruluş probu). Orijinal düşer,
/// kopya alıcıya ulaşır; kopyayı bildiren ACK ağda tutulur. Kuruluştan sonra kopya kayıp ilan
/// edilir (kaydı silinir). Sonra tutulan ACK gelir.
#[tokio::test(flavor = "multi_thread")]
async fn late_ack_after_dup_loss_keeps_payload() {
    let mut rig = Rig::new(cfg_no_fec()).await;
    let t = rig.next_unit;
    rig.net.with(|s| {
        s.target = Some(t);
        s.target_pass_nth = 1;
        s.hold_target_acks = true;
        s.hold_all = true;
    });
    assert_eq!(rig.send().await, t, "fikstür: hedef kimliği");
    let w = rig.send().await;
    rig.wait(
        "hedefin ikinci iletimi (kopya) alıcıya geçmeli",
        WAIT,
        |r| r.net.with(|s| s.hold_pn.is_some()),
    )
    .await;
    rig.wait("motorun kopya kararı aynada görünmeli", WAIT, |r| {
        r.client.live_diag().dups >= 1
    })
    .await;
    let live = rig.client.live_diag();
    assert_eq!(
        (live.rtt_samples, live.retransmits),
        (0, 0),
        "ön koşul: hedefin ikinci iletimi RTT örneği ve ARQ yokken çıktı → kopya (Duplicate) — \
         dups={}",
        live.dups
    );
    // Dönüş yolu açılır: kopyayı BİLDİRMEYEN tutulmuş ACK'ler (W'nin) istemciye verilir → kuruluş.
    let pre = rig.net.with(|s| {
        s.hold_all = false;
        std::mem::take(&mut s.pre_held)
    });
    assert!(!pre.is_empty(), "ön koşul: W'nin ACK'i tutulmuş olmalı");
    for wire in pre {
        rig.net.deliver_late(wire).await;
    }
    rig.wait("W teyit edilmeli", WAIT, |r| r.acked_set.contains(&w))
        .await;
    // Parçalama: kopyanın bloğu ACK çerçevesinin dışına itilir, ilerleme sürer → kopya kayıp ilan
    // edilir (paket eşiği; dolgunun seq'leri kopyanınkinden büyük).
    let first = rig.next_unit;
    let fillers = first..first + FRAG_FILLERS;
    rig.net
        .with(|s| s.drop_first.extend(fillers.clone().filter(|u| u % 2 == 1)));
    for _ in fillers {
        rig.send().await;
    }
    rig.wait("dolgu teyit edilmeli (yalnız hedef teyitsiz)", WAIT, |r| {
        r.client.outstanding() == 1
    })
    .await;
    let want = body(t);
    rig.wait("alıcı hedefi kopyadan teslim etmeli", WAIT, |r| {
        r.delivered.get(&t).is_some_and(|b| *b == want)
    })
    .await;
    rig.net.with(|s| {
        assert!(
            !s.held.is_empty(),
            "ön koşul: kopyayı bildiren ACK ağda tutuldu"
        );
    });
    rig.late_ack_keeps_payload_then_resend_confirms(t, "dup_loss", WAIT)
        .await;
}

/// Envanter satır 5 — **ESI ad çakışması (ters yön)**: parite ACK'i payload'ı silmez, teyit
/// üretmez ve esi adaşı kaynak birimin kopyalarını durdurmaz. Blok 0'ın kaynakları alıcıya
/// ulaşmaz, R parite ulaşır ve ACK'lenir (alıcı çözemez: kaynak yok). İstemci PRE_CONVERGE'dedir:
/// ARQ ve PTO kapalı, tek kurtarma kopyadır. Faz 2'de yalnız esi adaşı kaynaklar (0..R) geçer;
/// onların kopyaları sürüyorsa alıcı R kaynak + R pariteyle bloğu çözer ve K/K teyit gelir.
/// main: parite ACK'i (`unit.id = esi`) 0..R birimlerinin kopyalarını iptal eder → o birimler
/// kuruluşa dek hiç gönderilmez, kuruluş da gelmez → faz 2 KIRMIZI.
#[tokio::test(flavor = "multi_thread")]
async fn parity_ack_keeps_payload_and_copies_of_esi_namesake() {
    let k = block_k();
    let r = u64::from(PARITY_R);
    let mut rig = Rig::new(cfg_fec_static()).await;
    rig.net.with(|s| s.data_gate = DataGate::Nothing);
    for _ in 0..k {
        rig.send().await;
    }
    rig.wait(
        "R parite alıcıya ulaşmalı ve ACK'i istemciye geçmeli",
        WAIT,
        |x| {
            x.net
                .with(|s| (s.parity_pns.len() as u64) >= r && s.acks_with_all_parity >= 1)
        },
    )
    .await;
    let forwarded = rig.net.with(|s| s.acks_forwarded);
    rig.wait("istemci geçen ACK çerçevelerini işlemeli", WAIT, |x| {
        x.client.live_diag().ack.frames >= forwarded
    })
    .await;
    rig.pump(QUIET).await;
    // Faz 1: parite ACK'i payload'ı silmez, teyit üretmez.
    assert!(
        rig.acked.is_empty(),
        "parite ACK'i teyit ÜRETMEMELİ: {:?}",
        rig.acked
    );
    assert_eq!(rig.client.outstanding() as u64, k, "payload'lar yerinde");
    let rec = rig.fresh_recovery().await;
    assert_eq!(u64::from(rec.unacked_units), k, "{rec:?}");
    let live = rig.client.live_diag();
    assert_eq!(
        live.rtt_samples, 0,
        "ön koşul: PRE_CONVERGE (parite ACK'i RTT örneği vermez)"
    );
    // Faz 2: yalnız esi adaşı kaynaklar geçer.
    rig.net.with(|s| {
        s.data_gate = DataGate::Only((0..r).collect());
        s.data_passed.clear();
    });
    rig.wait(
        "esi adaşı birimler hizmet görmeye devam etmeli ve blok tamamlanmalı (K/K teyit)",
        ESI_WAIT,
        |x| x.client.outstanding() == 0,
    )
    .await;
    let passed = rig.net.with(|s| s.data_passed.clone());
    for u in 0..r {
        assert!(
            passed.get(&u).is_some_and(|n| *n >= 1),
            "esi adaşı birim {u} parite ACK'inden sonra yeniden gönderilmeli: {passed:?}"
        );
    }
    let delivered_ok = (0..k).all(|u| rig.delivered.get(&u).is_some_and(|b| *b == body(u)));
    let rec = rig.fresh_recovery().await;
    assert_runtime_maps_empty(&rec, "esi");
    let (cs, ss) = rig.finish().await;
    assert_eq!(cs.stale_decisions, 0, "{cs:?}");
    assert_eq!(cs.undelivered, 0, "{cs:?}");
    assert_eq!(
        (cs.acked, cs.fec_recovered_ctrl_units),
        (r, k - r),
        "esi adaşları ACK'le, kalanlar RECOVERED ile: {cs:?}"
    );
    assert_eq!(ss.fec_recovered_units, k - r, "{ss:?}");
    assert!(
        delivered_ok,
        "alıcı K birimi gerçek gövdeleriyle teslim etmeli"
    );
}

/// Store-miss bekçisi — kayıp + yeniden sıralama (ARQ, FEC kapalı): DATA'nın %20'si düşer, %10'u
/// 300 ms gecikir (geç orijinal yeniden gönderimden sonra gelir), ACK'lerin %10'u düşer. Bütün
/// birimler teslim + teyit; gövdesiz emisyon yok, haritalar boş. main: teyit kümesi yok — birim
/// geç orijinalin ACK'iyle silinirken uçuştaki yeniden gönderimi kaybolursa ARQ gövdesiz gider
/// (envanter satır 7).
#[tokio::test(flavor = "multi_thread")]
async fn store_miss_guard_loss_and_reorder() {
    let mut rig = Rig::new(cfg_no_fec()).await;
    rig.net.with(|s| {
        s.chaos = Some(Chaos {
            state: GUARD_SEED,
            data_drop_ppm: GUARD_DATA_DROP_PPM,
            data_delay_ppm: GUARD_DATA_DELAY_PPM,
            delay: GUARD_DELAY,
            ack_drop_ppm: GUARD_ACK_DROP_PPM,
        });
    });
    for _ in 0..GUARD_UNITS {
        rig.send().await;
    }
    rig.wait("bütün birimler teyit edilmeli", GUARD_WAIT, |r| {
        r.client.outstanding() == 0
    })
    .await;
    rig.wait("bütün birimler teslim edilmeli", WAIT, |r| {
        (r.delivered.len() as u64) == GUARD_UNITS
    })
    .await;
    assert!(
        (0..GUARD_UNITS).all(|u| rig.delivered.get(&u).is_some_and(|b| *b == body(u))),
        "alıcı gerçek gövdeleri teslim etmeli"
    );
    rig.pump(SETTLE).await;
    let rec = rig.fresh_recovery().await;
    assert_eq!(rec.unacked_units, 0, "{rec:?}");
    assert_runtime_maps_empty(&rec, "bekçi-kayıp");
    let (cs, _ss) = rig.finish().await;
    assert_eq!(
        cs.stale_decisions, 0,
        "store-miss bekçisi: teyitli birime karar (gövdesiz emisyon) olmamalı: {cs:?}"
    );
    assert_eq!(cs.undelivered, 0, "{cs:?}");
    assert_eq!(cs.acked, GUARD_UNITS, "{cs:?}");
}

/// Store-miss bekçisi — FEC RECOVERED (statik-r): iki birimin DATA'sı kalıcı düşer, alıcı bloğu
/// çözer ve RECOVERED bildirir. Teyitten sonra motorun kalan kayıtları (uçuştaki yeniden
/// gönderimler) gövdesiz karar üretmemeli. main: RECOVERED kayıp ve kopya kuyruğunu temizler ama
/// uçuştaki kayıt sonradan ARQ'ya girer (envanter satır 8).
#[tokio::test(flavor = "multi_thread")]
async fn store_miss_guard_fec_recovered() {
    let k = block_k();
    let mut rig = Rig::new(cfg_fec_static()).await;
    rig.net
        .with(|s| s.data_gate = DataGate::Except(GUARD_ERASED.into_iter().collect()));
    for _ in 0..k {
        rig.send().await;
    }
    rig.wait(
        "blok teyit edilmeli (silinenler RECOVERED ile)",
        WAIT,
        |r| r.client.outstanding() == 0,
    )
    .await;
    rig.wait("bütün birimler teslim edilmeli", WAIT, |r| {
        (r.delivered.len() as u64) == k
    })
    .await;
    assert!(
        (0..k).all(|u| rig.delivered.get(&u).is_some_and(|b| *b == body(u))),
        "alıcı gerçek gövdeleri teslim etmeli (silinenler FEC ile)"
    );
    rig.pump(SETTLE).await;
    let rec = rig.fresh_recovery().await;
    assert_eq!(rec.unacked_units, 0, "{rec:?}");
    assert_runtime_maps_empty(&rec, "bekçi-fec");
    let erased = GUARD_ERASED.len() as u64;
    let (cs, ss) = rig.finish().await;
    assert_eq!(
        cs.stale_decisions, 0,
        "store-miss bekçisi: RECOVERED sonrası gövdesiz emisyon olmamalı: {cs:?}"
    );
    assert_eq!(cs.undelivered, 0, "{cs:?}");
    assert_eq!(
        (cs.acked, cs.fec_recovered_ctrl_units),
        (k - erased, erased),
        "{cs:?}"
    );
    assert_eq!(ss.fec_recovered_units, erased, "{ss:?}");
}

// ── K4G-05c: K_MAP budaması ve birim başına O(1) runtime kaydı ───────────────────────────────

/// [K4G-05c · C11 DUR-30, DUR-05c-14 (A)] Kayıt-unutma yolu 4 — **K_MAP budaması** (gerçek
/// runtime; not §3.3 (iii)). Hedefin orijinali düşer; İKİNCİ iletimi (kayıp ilanının getirdiği
/// yeniden gönderim — Retransmit kökenli: kayıp ilanında tombstone'a geçer, kopya kökenli kayıp
/// yoluyla silinmez) alıcıya ulaşır; onu bildiren ACK'ler ağda tutulur ve parçalama dolgusuyla ACK
/// çerçevesinin dışına itilir. Dolgu teyit edildikten sonra SESSİZLİKTE hedefin iletimleri (hepsi
/// düşer) motorda birikir: hiçbir ACK gelmediğinden kayıp ilanı olmaz, kayıt yalnız budamayla
/// düşer. Motor birim başına toplam ≤ K_MAP için ORİJİNAL DIŞINDAKİ en eski kaydı budar — ikinci
/// iletimden eski kopya-dışı kayıt yoktur, ilk budama onu seçer. Ön koşul: sessizlikteki iletim
/// sayısı en az bir budamayı zorunlu kılar (görüntüdeki kayıt sayısı A'dan K_MAP − A + 2 iletim;
/// +1 orijinalin TTL'le düşme payı) ve ikinci iletimden beri tombstone TTL'inin alt sınırı
/// (`TTL_LOWER_BOUND`) geçmemiştir (yol TTL değil). Sonra tutulan ACK gelir: beş iddia.
///
/// TABAN (05b, budama kapalı): kayıt budanmaz, tutulan ACK canlı tombstone'la eşleşir ve hedefi
/// teyit eder → iddia (1) KIRMIZI beklenir.
#[tokio::test(flavor = "multi_thread")]
async fn late_ack_after_prune_keeps_payload() {
    let k_map = adaptive_transport::sender::K_MAP as u32;
    let mut rig = Rig::new(cfg_no_fec()).await;
    let w = rig.send().await;
    rig.wait("kuruluş birimi teyit edilmeli", WAIT, |r| {
        r.acked_set.contains(&w)
    })
    .await;
    let t = rig.next_unit;
    rig.net.with(|s| {
        s.target = Some(t);
        s.target_pass_nth = 1;
        s.hold_target_acks = true;
    });
    let before = rig.client.live_diag();
    assert_eq!(rig.send().await, t, "fikstür: hedef kimliği");
    // Orijinal düşer; geçen trafik paket eşiği kanıtını getirir → ikinci iletim alıcıya geçer.
    rig.wait_trickle("hedefin ikinci iletimi alıcıya geçmeli", WAIT, |r| {
        r.net.with(|s| s.hold_pn.is_some())
    })
    .await;
    let at_pass = rig.client.live_diag();
    assert!(
        at_pass.retransmits > before.retransmits && at_pass.dups == before.dups,
        "ön koşul: hedefin ikinci iletimi Retransmit kökenli olmalı (retransmit {} → {}, kopya {} → \
         {})",
        before.retransmits,
        at_pass.retransmits,
        before.dups,
        at_pass.dups
    );
    let passed_at = rig
        .net
        .with(|s| s.target_passed_at)
        .expect("hedefin ikinci iletimi geçti");
    // Parçalama: geçen iletimin bloğu ACK çerçevesinin dışına itilir, ilerleme sürer.
    let first = rig.next_unit;
    let fillers = first..first + FRAG_FILLERS;
    rig.net
        .with(|s| s.drop_first.extend(fillers.clone().filter(|u| u % 2 == 1)));
    for _ in fillers {
        rig.send().await;
    }
    rig.wait("dolgu teyit edilmeli (yalnız hedef teyitsiz)", WAIT, |r| {
        r.client.outstanding() == 1
    })
    .await;
    let want = body(t);
    rig.wait(
        "alıcı hedefi ikinci iletiminden teslim etmeli",
        WAIT,
        |r| r.delivered.get(&t).is_some_and(|b| *b == want),
    )
    .await;
    // Sessizlik: önce görüntü (A), SONRA röle sayacı — aradaki iletim ikisine de girmez (ihtiyatlı).
    let rec = rig.fresh_recovery().await;
    let a = rec
        .oldest_unacked
        .filter(|u| u.unit_id == t)
        .map(|u| u.engine.inflight_records + u.engine.tombstone_records);
    let a = a.expect("ön koşul: hedef teyitsiz depodaki en eski birim");
    assert!(
        a <= k_map,
        "budama: birim başına toplam kayıt ≤ K_MAP ({a} > {k_map}) — {rec:?}"
    );
    let need = u64::from(k_map - a + 2);
    let seen = |r: &Rig| {
        r.net
            .with(|s| u64::from(s.data_seen.get(&t).copied().unwrap_or(0)))
    };
    let s0 = seen(&rig);
    rig.wait(
        &format!("sessizlikte hedefin en az {need} iletimi (budamayı zorunlu kılar)"),
        TTL_LOWER_BOUND,
        |r| seen(r) - s0 >= need,
    )
    .await;
    let since_pass = passed_at.elapsed();
    assert!(
        since_pass < TTL_LOWER_BOUND,
        "ön koşul: ikinci iletimden beri tombstone TTL alt sınırı ({TTL_LOWER_BOUND:?}) geçmemeli \
         ({since_pass:?}) — yol K_MAP budaması olmalı, TTL değil"
    );
    let rec = rig.fresh_recovery().await;
    let after = rec
        .oldest_unacked
        .filter(|u| u.unit_id == t)
        .map(|u| u.engine.inflight_records + u.engine.tombstone_records)
        .unwrap_or_default();
    assert!(
        after <= k_map,
        "budama: birim başına toplam kayıt ≤ K_MAP ({after} > {k_map}) — {rec:?}"
    );
    rig.net.with(|s| {
        assert!(
            !s.held.is_empty(),
            "ön koşul: hedefin ikinci iletimini bildiren ACK ağda tutuldu"
        );
    });
    rig.late_ack_keeps_payload_then_resend_confirms(t, "prune", WAIT)
        .await;
}

/// [K4G-05c] Kurtarma görüntüsü örnekleyicisi: her YENİ görüntü (`taken_at`) bir kez sayılır; birim
/// başına O(1) kaydın iki iddiasını (`seq_to_unit_len == 0`, `unit_seqs_len ≤ unacked_units`)
/// ihlal eden görüntüler toplanır.
#[derive(Default)]
struct RecordSampler {
    last: Option<Instant>,
    /// Teyitsiz birim varken alınmış görüntü sayısı (iddianın boş geçmemesi için ön koşul).
    busy: u32,
    over: Vec<SenderRecoveryDiag>,
}

impl RecordSampler {
    fn sample(&mut self, r: &Rig) {
        let Some(rec) = r.client.live_diag().recovery else {
            return;
        };
        if self.last == Some(rec.taken_at) {
            return;
        }
        self.last = Some(rec.taken_at);
        if rec.unacked_units > 0 {
            self.busy += 1;
        }
        if rec.seq_to_unit_len != 0 || rec.unit_seqs_len > rec.unacked_units {
            self.over.push(rec);
        }
    }
}

/// [K4G-05c] Birim kaydı testinde tutulan birim (bekçi kanalına ek; ilk evrede bütün iletimleri
/// düşer) ve o evrede beklenen, teyitsiz birim varken alınmış görüntü sayısı.
const RECORDS_HELD_UNIT: u64 = GUARD_UNITS / 2;
const RECORDS_HELD_SAMPLES: u32 = 2;

/// [K4G-05c · not §3.3 (ii), §6] Birim başına O(1) runtime kaydı: kayıp + yeniden sıralama
/// kanalında (bekçi kanalı) HER kurtarma görüntüsünde `seq_to_unit_len == 0` ve gönderim kaydı
/// girdisi (`unit_seqs_len`) ≤ teyitsiz birim (`unacked_units`); sonda ikisi de 0. Ön koşul:
/// teyitsiz birim varken alınmış görüntü ve yeniden gönderim (iddia boş geçmesin).
///
/// Fikstür (ilk koşudan sonra düzeltildi): bekçi kanalı loopback'te bütün birimleri iki görüntü
/// arasında (`RECOVERY_DIAG_PERIOD` = 200 ms; ilk görüntü boş depoyla görev başında) teyit
/// edebiliyordu — ölçüm: teyitsiz birimli görüntü 0, yeniden gönderim 138. Bu yüzden ilk evrede
/// bir birimin (`RECORDS_HELD_UNIT`) bütün iletimleri düşer: o birim teyitsiz ve yeniden
/// gönderilirken en az `RECORDS_HELD_SAMPLES` görüntü alınır; sonra kapı açılır ve bütün birimler
/// teyit edilir. İddialar değişmedi.
///
/// TABAN (05b): KIRMIZI beklenir — iletim → birim haritası yeniden gönderimle büyür
/// (`seq_to_unit_len` ≥ teyitsiz birim > 0).
#[tokio::test(flavor = "multi_thread")]
async fn unit_records_bounded_per_unit() {
    let mut rig = Rig::new(cfg_no_fec()).await;
    rig.net.with(|s| {
        s.chaos = Some(Chaos {
            state: GUARD_SEED,
            data_drop_ppm: GUARD_DATA_DROP_PPM,
            data_delay_ppm: GUARD_DATA_DELAY_PPM,
            delay: GUARD_DELAY,
            ack_drop_ppm: GUARD_ACK_DROP_PPM,
        });
        s.data_gate = DataGate::Except(std::iter::once(RECORDS_HELD_UNIT).collect());
    });
    for _ in 0..GUARD_UNITS {
        rig.send().await;
    }
    let mut sampler = RecordSampler::default();
    rig.wait(
        "ön koşul: tutulan birim teyitsizken görüntüler ve yeniden gönderim",
        WAIT,
        |r| {
            sampler.sample(r);
            sampler.busy >= RECORDS_HELD_SAMPLES && r.client.live_diag().retransmits >= 1
        },
    )
    .await;
    rig.net.with(|s| s.data_gate = DataGate::All);
    rig.wait("bütün birimler teyit edilmeli", GUARD_WAIT, |r| {
        sampler.sample(r);
        r.client.outstanding() == 0
    })
    .await;
    let RecordSampler {
        busy: busy_samples,
        over,
        ..
    } = sampler;
    let retransmits = rig.client.live_diag().retransmits;
    assert!(
        busy_samples >= 1 && retransmits >= 1,
        "ön koşul: teyitsiz birim varken görüntü ({busy_samples}) ve yeniden gönderim \
         ({retransmits}) olmalı"
    );
    assert!(
        over.is_empty(),
        "birim başına O(1): her görüntüde seq_to_unit_len = 0 ve unit_seqs_len ≤ unacked_units \
         olmalı; ihlal ({}): {:?}",
        over.len(),
        over.first()
    );
    rig.pump(SETTLE).await;
    let rec = rig.fresh_recovery().await;
    assert_eq!(rec.unacked_units, 0, "{rec:?}");
    assert_runtime_maps_empty(&rec, "o1");
    let (cs, _ss) = rig.finish().await;
    assert_eq!(cs.stale_decisions, 0, "{cs:?}");
    assert_eq!(cs.acked, GUARD_UNITS, "{cs:?}");
}

// ── K4G-05c: FEC dolu kanalda (not 3.6; CEVAP 11-EK §5, CEVAP 13 §7) ─────────────────────────

/// Dolu kanal kolunun teslim kanalı: K − 1 (blok boyunun bir eksiği; not D.5) → tek blok kanalı
/// doldurur, kurtarılan birime yer kalmaz.
fn fec_full_channel_server_cfg() -> RuntimeConfig {
    RuntimeConfig {
        delivery_queue: usize::try_from(block_k() - 1).expect("küçük sayı"),
        // R = min(max_peers, C − 1) = 1: tek oturumun sıra başı rezervi.
        max_peers: 1,
        ..cfg_fec_static()
    }
}

/// Durak evresinin ön koşulu: kurtarım ya da yeniden gönderim en az bir kez reddedildi ve hedef
/// dışındaki birimler teyitli (teslimler okunmaz).
async fn fec_stall_until_refused(rig: &mut Rig, target: u64) {
    let deadline = Instant::now() + WAIT;
    loop {
        rig.pump_acks_only(STEP).await;
        let refused = rig.server.ctx_dispatch_stats().full_refusals >= 1;
        let others = (0..target).all(|u| rig.acked_set.contains(&u));
        if refused && others {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "ön koşul: dolu kanalda ret ve hedef dışı teyit {WAIT:?} içinde olmadı — {}",
            rig.describe()
        );
    }
}

/// Dolu kanalda durak penceresi: hedefin bu süre boyunca teyitsiz kaldığı gözlenir.
const FEC_STALL_QUIET: Duration = Duration::from_secs(2);
/// Drenaj tavanı: reddedilmiş birimin geri çekilmiş yeniden gönderimi (RTO_MAX 30 s) için pay.
const FEC_DRAIN_WAIT: Duration = Duration::from_secs(90);
/// Kol 2 deneme penceresi (drenajdan sonra; sonuç raporlanır, iddia edilmez).
const FEC_ARM2_WINDOW: Duration = Duration::from_secs(10);

/// [K4G-05c · not 3.6 KOL 1 (zorunlu), D.5; 11-EK §5] FEC DOLU KANALDA — kol 1. Oturum kipli alıcı,
/// teslim kanalı K − 1, R = 1; tüketici durur. Blok 0'ın son biriminin (hedef) YALNIZ ilk
/// emisyonu düşer; diğer K − 1 birim kanalı doldurur (sıra başı rezervle, kalanlar F − 1 ≥ R − U
/// ile). Parite gelince blok çözülür ama kurtarılan hedefe yer yoktur → kurtarım reddedilir:
/// RECOVERED listesine girmez (alıcının tutamadığı birim FEC teyidiyle de ACK'lenmez), hedef
/// durakta teyitsiz kalır, yeniden gönderimi de reddedilir (`full_refusals`). Tüketici dönünce
/// kaynak yeniden gönderimi ulaşır → ARQ ile K/K tamamlanır.
///
/// ESI mutantı (reddedilen kurtarımın ESI'si RECOVERED'a girerse) burada ölür: gönderen hedefi
/// RECOVERED ile teyit eder, payload'u siler ve yeniden göndermez → drenajda hedef gelmez.
#[tokio::test(flavor = "multi_thread")]
async fn fec_full_channel_arm1_refused_recovery_completes_by_arq() {
    let k = block_k();
    let target = k - 1;
    let mut rig = Rig::new_session_keyed(fec_full_channel_server_cfg(), cfg_fec_static()).await;
    rig.net.with(|s| {
        s.drop_first.insert(target);
    });
    for _ in 0..k {
        rig.send().await;
    }
    fec_stall_until_refused(&mut rig, target).await;
    rig.pump_acks_only(FEC_STALL_QUIET).await;
    assert!(
        !rig.acked_set.contains(&target),
        "durakta hedef teyitsiz kalmalı: tutulamayan kurtarım RECOVERED ile teyit edilmemeli — \
         teyitler {:?}",
        rig.acked
    );
    // Drenaj: tüketici döner; hedef ARQ ile tamamlanır.
    rig.wait("drenajda K/K teslim ve teyit", FEC_DRAIN_WAIT, |r| {
        r.client.outstanding() == 0 && (r.delivered.len() as u64) == k
    })
    .await;
    assert!(
        (0..k).all(|u| rig.delivered.get(&u).is_some_and(|b| *b == body(u))),
        "alıcı K birimi gerçek gövdeleriyle teslim etmeli"
    );
    let refusals = rig.server.ctx_dispatch_stats().full_refusals;
    let (cs, ss) = rig.finish().await;
    eprintln!(
        "[FEC kol 1] full_refusals={refusals} fec_blocks_decoded={} fec_recovered_units={} \
         acked={} fec_recovered_ctrl_units={}",
        ss.fec_blocks_decoded, ss.fec_recovered_units, cs.acked, cs.fec_recovered_ctrl_units
    );
    assert!(
        ss.fec_blocks_decoded >= 1,
        "ön koşul: blok çözüldü (kurtarım üretildi ve reddedildi): {ss:?}"
    );
    assert_eq!(
        ss.fec_recovered_units, 0,
        "dolu kanalda kurtarım kanala girmedi; hedef ARQ'la geldi: {ss:?}"
    );
    assert_eq!(ss.delivery_dropped, 0, "{ss:?}");
    assert_eq!(
        (cs.acked, cs.fec_recovered_ctrl_units),
        (k, 0),
        "K birim ACK ile teyit, RECOVERED teyidi yok: {cs:?}"
    );
    assert_eq!(cs.stale_decisions, 0, "{cs:?}");
}

/// [K4G-05c · not 3.6 KOL 2 — DENEME; PoC garantisi DEĞİL (KATMAN 6 beyanlı sınırı; CEVAP 13 §7)]
/// FEC DOLU KANALDA — kol 2. Kol 1 düzeneği, ama hedefin kaynağı KALICI kaybolur (bütün
/// iletimleri düşer); onarım yeterlidir, kurtarım dolu kanalda reddedilir ve blok düşer.
/// Tüketici döndükten sonra `FEC_ARM2_WINDOW` boyunca FEC üzerinden tamamlanma GÖZLENİR ve
/// RAPORLANIR; iddia edilmez — tek başarılı koşu sınırı kaldırmaz, "FEC altında dayanıklılık
/// sağlandı" DENMEZ. İddia yalnız R2 güvenliğidir: hedef teslim edilmedikçe teyit edilmez (sahte
/// RECOVERED yok; oturum o birimde bekler), diğer birimler eksiksiz ve gövdeleri doğru, teslim
/// düşüşü yok.
#[tokio::test(flavor = "multi_thread")]
async fn fec_full_channel_arm2_permanent_source_loss_is_reported_not_guaranteed() {
    let k = block_k();
    let target = k - 1;
    let mut rig = Rig::new_session_keyed(fec_full_channel_server_cfg(), cfg_fec_static()).await;
    rig.net
        .with(|s| s.data_gate = DataGate::Except(std::iter::once(target).collect()));
    for _ in 0..k {
        rig.send().await;
    }
    fec_stall_until_refused(&mut rig, target).await;
    rig.pump_acks_only(FEC_STALL_QUIET).await;
    assert!(
        !rig.acked_set.contains(&target),
        "durakta hedef teyitsiz kalmalı — teyitler {:?}",
        rig.acked
    );
    rig.pump(FEC_ARM2_WINDOW).await;
    let completed = rig.delivered.contains_key(&target);
    eprintln!(
        "[FEC kol 2 — DENEME, garanti değil] hedef FEC üzerinden tamamlandı mı: {completed} \
         (KATMAN 6 beyanlı sınırı: dolu kanalda FEC kurtarımı kaybolabilir; kaynak kalıcı kayıpsa \
         oturum o birimde bekler)"
    );
    if completed {
        assert_eq!(
            rig.delivered.get(&target),
            Some(&body(target)),
            "tamamlandıysa gövde doğru olmalı"
        );
    } else {
        assert!(
            !rig.acked_set.contains(&target),
            "R2 güvenliği: teslim edilmeyen hedef teyit edilmemeli (sahte RECOVERED) — teyitler {:?}",
            rig.acked
        );
    }
    assert!(
        (0..target).all(|u| rig.delivered.get(&u).is_some_and(|b| *b == body(u))),
        "hedef dışı birimler gerçek gövdeleriyle teslim edilmeli"
    );
    let (_cs, ss) = rig.finish().await;
    assert_eq!(ss.delivery_dropped, 0, "{ss:?}");
}

// ── K4G-05c: gönderen mesafe kapısı (not 3.2, D.3, D.5) ──────────────────────────────────────

/// [K4G-05c · not 3.2 / D.3 / D.5 — "gönderen mesafe"] Gönderen, en yeni birimi ile en eski
/// teyitsiz birimi arasındaki kimlik mesafesi `max_inflight_units`'e ulaşınca sunumu almaz
/// (`distance_holds`); dürüst göndericinin birimi alıcının oturum penceresinin dışına düşmez
/// (`reorder_skip` = 0, pay = 0 — DUR-05c-2 (a)). Pencere 8, birim 0'ın bütün iletimleri düşer:
/// gönderen 0..7 dışında kimlik yollamaz; 0 geçince bütün birimler teslim ve teyit edilir.
#[tokio::test(flavor = "multi_thread")]
async fn sender_distance_gate_holds_offers_and_receiver_never_refuses_window() {
    const WINDOW: usize = 8;
    const UNITS: u64 = 3 * WINDOW as u64;
    let cfg = RuntimeConfig {
        max_inflight_units: WINDOW,
        ..cfg_no_fec()
    };
    let mut rig = Rig::new_session_keyed(cfg.clone(), cfg).await;
    let stuck = 0u64;
    rig.net
        .with(|s| s.data_gate = DataGate::Except(std::iter::once(stuck).collect()));
    for _ in 0..UNITS {
        rig.send().await;
    }
    rig.wait("mesafe kapısı sunumu bekletmeli", WAIT, |r| {
        r.client.ctx_dispatch_stats().distance_holds >= 1
    })
    .await;
    rig.pump(QUIET).await;
    let highest = rig.net.with(|s| s.data_seen.keys().max().copied());
    assert!(
        highest.is_some_and(|u| u < WINDOW as u64),
        "gönderen en eski teyitsiz birimden itibaren pencere dışı kimlik yollamamalı: en büyük \
         görülen {highest:?} (pencere {WINDOW})"
    );
    rig.net.with(|s| s.data_gate = DataGate::All);
    rig.wait(
        "bütün birimler teslim ve teyit edilmeli",
        RETX_WAIT,
        |r| r.client.outstanding() == 0 && (r.delivered.len() as u64) == UNITS,
    )
    .await;
    assert!(
        (0..UNITS).all(|u| rig.delivered.get(&u).is_some_and(|b| *b == body(u))),
        "alıcı gerçek gövdeleri teslim etmeli"
    );
    let stats = rig.server.ctx_dispatch_stats();
    assert_eq!(
        stats.reorder_skip, 0,
        "dürüst gönderici alıcı penceresinin dışına düşmez: {stats:?}"
    );
    let (cs, _ss) = rig.finish().await;
    assert_eq!(cs.acked, UNITS, "{cs:?}");
}
