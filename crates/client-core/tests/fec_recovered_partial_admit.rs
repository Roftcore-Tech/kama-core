//! [INV-ACK-DELIVER-1 · F4 — geri bildirimi aç profil] Bloğu çözülen ama teslim kanalına GİREMEYEN
//! kurtarılmış birim göndericide teyitsiz kalır ve sonunda yeniden iletimle (DATA yolu) teslim
//! edilir.
//!
//! Gerçek istemci + sunucu, gerçek UDP loopback; yön hızı profili açık ve geri bildirim aç
//! (istemci ileri 12 500 B/s, dönüş 62 B/s: bütçeli ACK aralığı ≥ 1 s, `link_feedback_starved`).
//! Ürün FEC profili (0x05, K = 8; aç profilde PTT dışı oran 500 ‰ → blok başına 4 onarım
//! PLANLANIR; blok ikinci onarımla çözülür, gelen RECOVERED kuyruktaki pariteyi iptal eder, telde
//! 2–4 onarım görülür; test bu sayıya bağlı değildir). Sunucunun teslim kanalı `K − 1` yerlidir,
//! tüketici ilk evrede DURUR.
//!
//! Düzenek (belirlenimli; zamanlama yalnız üst sınır):
//! - Sunucu seam'i ilk evrede 6 ve 7 numaralı birimlerin BÜTÜN DATA emisyonlarını düşürür: ikisi de
//!   yalnız FEC ile çözülebilir. Hangi sırada çözülürse çözülsün kanala en çok `K − 1` birim girer ve
//!   kurtarılanlar ESI sırasıyla kapıdan geçtiği için kanala giremeyen birim 7'dir. `fec_ingest`
//!   birim 7'yi RECOVERED listesine YAZMAZ (`fec_recovered_not_admitted`), 6'yı yazar: KISMİ liste.
//! - İstemci seam'i ilk evrede gelen ACK çerçevelerini düşürür: bu evrede bir birimi teyit edebilen
//!   TEK çerçeve RECOVERED'dır. `outstanding()` 8'in altına inince RECOVERED işlenmiştir. Seam
//!   geçen RECOVERED'ın listesini kaydeder.
//!
//! İddialar (ilk evre), sırayla — alıcı ile göndericinin kırılması ayrı mesajla görünür:
//! 1. Alıcı: RECOVERED yalnız ESI 6'yı listeler (kanala giremeyen 7 listede yok).
//! 2. Gönderici: RECOVERED yalnız listelediğini teyit eder — teyitli küme tam {6}, teyitsiz `K − 1`.
//!
//! İkinci evre (iki seam serbest, tüketici açık): sekiz birimin hepsi tam bir kez ve bayt-eş teslim
//! edilir, sekizi de teyit edilir; FEC ile teslim edilen ve RECOVERED ile teyit edilen yalnız birim
//! 6'dır, birim 7 DATA yolundan gelir. ESKİ DAVRANIŞ (993e08be, RECOVERED-BLOK): gönderici bu ucun
//! kapattığı bloğun listede olmayan ve depoda tutulan bütün birimlerini de teyit ediyordu; birim 7
//! teyitli sayılıp payload'ı silinir, bir daha tele çıkmaz, alıcı onu hiç teslim etmez.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use adaptive_transport::ack::{
    FRAME_TYPE_ACK, FRAME_TYPE_ACK_BITMAP, FRAME_TYPE_ACK_BITMAP_PTT, FRAME_TYPE_ACK_V2,
};
use client_core::runtime::{
    ClientEndpoint, ClientHandle, LinkRates, RuntimeConfig, ServerEndpoint, ServerHandle,
};
use client_core::seam::IdentitySeam;
use client_core::wire::{decode_data, decode_fec, decode_recovered, FRAME_RECOVERED};
use kama_contracts::s3::{
    Opened, PacketProtection, ProtectionClass, ProtectionContext, ProtectionLimits, SealError,
    SealedUnit,
};
use kama_transport_schema::inner_kind::FEC_REPAIR;

/// Ürün FEC profili 0x05'in blok boyu.
const K: u64 = 8;
/// Sunucu teslim kanalı: bloğun bir birimi sığmaz.
const QUEUE: usize = (K - 1) as usize;
/// Yalnız FEC ile çözülebilen iki birim (bloğun son iki kaynağı).
const HELD: [u64; 2] = [K - 2, K - 1];
/// Kanala giren kurtarılmış birim: RECOVERED'ın tek listelediği.
const ADMITTED: u64 = K - 2;
/// Kanala giremeyen kurtarılmış birim: kurtarılanlar ESI sırasıyla kapıdan geçer, sonuncusu düşer.
const NOT_ADMITTED: u64 = K - 1;
/// Birim boyu: profil 0x05'te onarım alan, ileri hatta 1 s içinde serileşen boy.
const UNIT_BYTES: usize = 900;
/// İstemcinin yön hızları (geri bildirimi aç ürün profili).
const FWD_BPS: u64 = 12_500;
const RET_BPS: u64 = 62;
/// Beklenen olayın üst sınırı (dönüş 62 B/s; ACK aralığı ≥ 1 s).
const WAIT: Duration = Duration::from_secs(60);
/// Koşul yoklama aralığı.
const POLL: Duration = Duration::from_millis(1);
/// RECOVERED işlendikten sonra ilk evrenin sürdüğü süre: eski davranışta o anda bütün birimler
/// teyit edilir (tek `drive`); bu süre teyit drenajının ve tele çıkmış emisyonların sunucuda
/// işlenmesinin bitmesi içindir (loopback; mikrosaniyeler).
const SETTLE: Duration = Duration::from_millis(500);

#[derive(Debug, Default)]
struct ServerObs {
    /// Blok başına seam'den geçen onarım.
    repairs_passed: BTreeMap<u32, u64>,
    /// Birim başına düşürülen DATA emisyonu.
    dropped: BTreeMap<u64, u64>,
}

/// Sunucu yönü: `hold` açıkken `HELD` birimlerinin bütün DATA emisyonları düşer.
struct ServerSeam {
    hold: Arc<AtomicBool>,
    obs: Arc<Mutex<ServerObs>>,
    inner: IdentitySeam,
}

impl PacketProtection for ServerSeam {
    fn seal(&mut self, ctx: ProtectionContext, frame: &[u8]) -> Result<SealedUnit, SealError> {
        self.inner.seal(ctx, frame)
    }
    fn open(&mut self, wire: &[u8]) -> Option<Opened> {
        let od = self.inner.open(wire)?;
        let mut obs = self.obs.lock().expect("kilit");
        if let Some((_, u, _)) = decode_data(&od.frame) {
            if self.hold.load(Ordering::SeqCst) && HELD.contains(&u) {
                *obs.dropped.entry(u).or_default() += 1;
                return None;
            }
        } else if let Some((h, _)) = decode_fec(&od.frame) {
            if h.kind == FEC_REPAIR {
                *obs.repairs_passed.entry(h.block_id).or_default() += 1;
            }
        }
        Some(od)
    }
    fn sealed_len(&self, ctx: ProtectionContext, n: usize) -> Result<usize, SealError> {
        self.inner.sealed_len(ctx, n)
    }
    fn conservative_limits(&self, class: ProtectionClass) -> ProtectionLimits {
        self.inner.conservative_limits(class)
    }
    fn limits(&self, ctx: ProtectionContext) -> ProtectionLimits {
        self.inner.limits(ctx)
    }
}

#[derive(Debug, Default)]
struct ClientObs {
    /// Düşürülen ACK çerçevesi.
    acks_dropped: u64,
    /// Geçen RECOVERED çerçevesi.
    recovered_seen: u64,
    /// Geçen RECOVERED çerçevelerinin `(block_id, ESI listesi)` içeriği.
    recovered_listed: Vec<(u32, Vec<u16>)>,
}

/// İstemci yönü: `mute` açıkken gelen ACK çerçeveleri düşer; RECOVERED ve diğerleri geçer.
struct ClientSeam {
    mute: Arc<AtomicBool>,
    obs: Arc<Mutex<ClientObs>>,
    inner: IdentitySeam,
}

impl PacketProtection for ClientSeam {
    fn seal(&mut self, ctx: ProtectionContext, frame: &[u8]) -> Result<SealedUnit, SealError> {
        self.inner.seal(ctx, frame)
    }
    fn open(&mut self, wire: &[u8]) -> Option<Opened> {
        let od = self.inner.open(wire)?;
        let mut obs = self.obs.lock().expect("kilit");
        match od.frame.first().copied() {
            Some(
                FRAME_TYPE_ACK
                | FRAME_TYPE_ACK_V2
                | FRAME_TYPE_ACK_BITMAP
                | FRAME_TYPE_ACK_BITMAP_PTT,
            ) if self.mute.load(Ordering::SeqCst) => {
                obs.acks_dropped += 1;
                return None;
            }
            Some(FRAME_RECOVERED) => {
                obs.recovered_seen += 1;
                if let Some(listed) = decode_recovered(&od.frame) {
                    obs.recovered_listed.push(listed);
                }
            }
            _ => {}
        }
        Some(od)
    }
    fn sealed_len(&self, ctx: ProtectionContext, n: usize) -> Result<usize, SealError> {
        self.inner.sealed_len(ctx, n)
    }
    fn conservative_limits(&self, class: ProtectionClass) -> ProtectionLimits {
        self.inner.conservative_limits(class)
    }
    fn limits(&self, ctx: ProtectionContext) -> ProtectionLimits {
        self.inner.limits(ctx)
    }
}

async fn wait_until(what: &str, mut cond: impl FnMut() -> bool) {
    tokio::time::timeout(WAIT, async {
        while !cond() {
            tokio::time::sleep(POLL).await;
        }
    })
    .await
    .unwrap_or_else(|_| panic!("üst süre içinde sağlanmalı: {what}"));
}

async fn drain(server: &mut ServerHandle, n: usize) -> BTreeMap<u64, Vec<u8>> {
    tokio::time::timeout(WAIT, async {
        let mut got = BTreeMap::new();
        while got.len() < n {
            let d = server.next_delivery().await.expect("teslim akışı açık");
            assert!(
                got.insert(d.unit_id, d.payload).is_none(),
                "kimlik {} iki kez teslim edildi",
                d.unit_id
            );
        }
        got
    })
    .await
    .unwrap_or_else(|_| panic!("üst süre içinde {n} birimin hepsi teslim edilmeli (kayıp yok)"))
}

async fn acked_all(client: &mut ClientHandle, acked: &mut BTreeSet<u64>) {
    tokio::time::timeout(WAIT, async {
        while acked.len() < K as usize {
            let u = client.acked().await.expect("ACK akışı açık");
            acked.insert(u);
        }
    })
    .await
    .expect("bütün birimler teyitli");
}

/// İki birim FEC ile çözülür, yalnız biri teslim kanalına girer: öbürü RECOVERED'a yazılmaz ve
/// göndericide teyitsiz (yeniden iletilebilir) kalır; kanal açılınca DATA yolundan teslim edilir.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn recovered_but_not_admitted_unit_stays_unconfirmed_and_is_delivered_by_retransmission() {
    let hold = Arc::new(AtomicBool::new(true));
    let mute = Arc::new(AtomicBool::new(true));
    let sobs = Arc::new(Mutex::new(ServerObs::default()));
    let cobs = Arc::new(Mutex::new(ClientObs::default()));

    let server_cfg = RuntimeConfig {
        delivery_queue: QUEUE,
        link_rates: Some(LinkRates {
            uplink_bps: RET_BPS,
            downlink_bps: FWD_BPS,
        }),
        ..RuntimeConfig::default()
    };
    let client_cfg = RuntimeConfig {
        link_rates: Some(LinkRates {
            uplink_bps: FWD_BPS,
            downlink_bps: RET_BPS,
        }),
        ..RuntimeConfig::default()
    };
    assert!(
        client_core::runtime::link_feedback_starved(&client_cfg),
        "düzenek: istemci geri bildirimi aç profilde"
    );
    let mut server = ServerEndpoint::bind(
        "127.0.0.1:0".parse().unwrap(),
        server_cfg,
        Box::new(ServerSeam {
            hold: Arc::clone(&hold),
            obs: Arc::clone(&sobs),
            inner: IdentitySeam::new(),
        }),
    )
    .await
    .expect("server bind");
    let mut client = ClientEndpoint::connect(
        server.local_addr(),
        client_cfg,
        Box::new(ClientSeam {
            mute: Arc::clone(&mute),
            obs: Arc::clone(&cobs),
            inner: IdentitySeam::new(),
        }),
    )
    .await
    .expect("client connect");
    let payload = |i: u64| {
        let mut p = vec![u8::try_from(i).expect("küçük"); UNIT_BYTES];
        p[..8].copy_from_slice(&i.to_be_bytes());
        p
    };

    // ── Evre 1: tüketici durur; blok 0 kanal doluyken çözülür; RECOVERED kısmi ──
    let t1 = Instant::now();
    for i in 0..K {
        client.send(payload(i)).expect("offer");
    }
    wait_until(
        "RECOVERED istemcide işlendi (bu evrede tek teyit kaynağı)",
        || cobs.lock().expect("kilit").recovered_seen >= 1 && client.outstanding() < K as usize,
    )
    .await;
    tokio::time::sleep(SETTLE).await;
    let evre1 = t1.elapsed();
    let mut acked = BTreeSet::new();
    while let Some(u) = client.try_acked() {
        acked.insert(u);
    }
    let outstanding = client.outstanding();
    {
        let s = sobs.lock().expect("kilit");
        assert!(
            HELD.iter()
                .all(|u| s.dropped.get(u).copied().unwrap_or(0) >= 1),
            "düzenek: iki birimin DATA'sı ilk evrede düştü: {:?}",
            s.dropped
        );
    }
    let listed = cobs.lock().expect("kilit").recovered_listed.clone();
    let admitted_esi = u16::try_from(ADMITTED).expect("küçük");
    assert_eq!(
        listed,
        vec![(0, vec![admitted_esi])],
        "ALICI [F4]: RECOVERED yalnız kanala giren kurtarılmış birim {ADMITTED}'yı listelemeli; \
         kanala giremeyen {NOT_ADMITTED} listede olmamalı"
    );
    assert!(
        acked == BTreeSet::from([ADMITTED]) && outstanding == QUEUE,
        "GÖNDERİCİ: RECOVERED yalnız listelediği birimi teyit etmeli — teyitli tam \
         {{{ADMITTED}}}, teyitsiz {QUEUE} (kanala giremeyen {NOT_ADMITTED} dahil; göndericinin \
         ARQ'su onu yeniden iletir); teyit edilenler {acked:?}, teyitsiz {outstanding}"
    );

    // ── Evre 2: seam'ler serbest, tüketici açık: birim 7 DATA yolundan gelir ──
    hold.store(false, Ordering::SeqCst);
    mute.store(false, Ordering::SeqCst);
    let t2 = Instant::now();
    let got = drain(&mut server, K as usize).await;
    let evre2_teslim = t2.elapsed();
    assert_eq!(
        got.keys().copied().collect::<Vec<_>>(),
        (0..K).collect::<Vec<_>>()
    );
    for (id, p) in &got {
        assert_eq!(p, &payload(*id), "yük bayt-eş (birim {id})");
    }
    acked_all(&mut client, &mut acked).await;
    let evre2_teyit = t2.elapsed();

    let cs = client.shutdown().await;
    let ss = server.shutdown().await;
    let (s, c) = (sobs.lock().expect("kilit"), cobs.lock().expect("kilit"));
    eprintln!(
        "ÖLÇÜM recovered-kısmi: evre1_ms={} evre2_teslim_ms={} evre2_teyit_ms={} (üst sınır \
         {} ms/bekleme) delivered={} fec_blocks_decoded={} fec_recovered_units={} \
         fec_recovered_not_admitted={} data_not_admitted_full={} fec_recovered_ctrl_tx={} \
         cs.fec_recovered_ctrl_units={} repairs_passed={:?} dropped={:?} acks_dropped={} \
         recovered_seen={} recovered_listed={:?}",
        evre1.as_millis(),
        evre2_teslim.as_millis(),
        evre2_teyit.as_millis(),
        WAIT.as_millis(),
        ss.delivered,
        ss.fec_blocks_decoded,
        ss.fec_recovered_units,
        ss.fec_recovered_not_admitted,
        ss.data_not_admitted_full,
        ss.fec_recovered_ctrl_tx,
        cs.fec_recovered_ctrl_units,
        s.repairs_passed,
        s.dropped,
        c.acks_dropped,
        c.recovered_seen,
        c.recovered_listed
    );
    assert!(
        ss.fec_blocks_decoded >= 1 && ss.fec_recovered_not_admitted == 1,
        "düzenek: blok kanal doluyken çözüldü ve tam bir kurtarım kanala giremedi: {ss:?}"
    );
    assert_eq!(
        ss.fec_recovered_units, 1,
        "yalnız birim {ADMITTED} FEC ile teslim edildi; birim {NOT_ADMITTED} DATA yolundan: {ss:?}"
    );
    assert_eq!(
        cs.fec_recovered_ctrl_units, 1,
        "RECOVERED ile teyit edilen yalnız birim {ADMITTED}: {cs:?}"
    );
    assert_eq!(ss.delivered, K, "her birim tam bir kez: {ss:?}");
}
