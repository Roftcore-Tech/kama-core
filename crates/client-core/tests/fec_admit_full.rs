//! [INV-ACK-DELIVER-1 · F4 — FEC kolu] Teslim kanalı doluyken FEC ile KURTARILAN birim.
//!
//! Gerçek UDP loopback; statik-r FEC (profil 0x03, K=8, R=4); teslim kanalı küçük
//! (`delivery_queue = 4`); tüketici önce DURUR. Alıcı seam'i seçilen birimin YALNIZ İLK DATA
//! emisyonunu düşürür (belirlenimli; yeniden iletim geçer). Beklenen akış:
//!
//! 1. Kanal dolar; sonraki DATA motora verilmez (ACK yok).
//! 2. Repair'ler gelir, blok çözülür; kurtarılan birim kanala GİREMEZ → RECOVERED'a yazılmaz
//!    (`fec_recovered_not_admitted > 0`); kaynak seq ACK'siz kalır.
//! 3. Tüketici açılır; gönderici kaynak birimi ARQ ile yeniden iletir; birim DATA olarak teslim.
//!
//! İddia: hiçbir kurtarılmış ya da ACK'lenmiş birim kaybolmaz (gönderilen küme == teslim edilen
//! küme, her biri tam bir kez, yük bayt-eş); `delivery_dropped = 0`; kontrol trafiği durmaz —
//! PARAM bağlanır ve tüketici açıldıktan sonra gelen bir kurtarım RECOVERED ile bildirilir.
//!
//! Ön koşullar zamanlamaya bağlı DEĞİL (ilk testte): seam, sunucu görevinde datagram başına SIRAYLA
//! çağrılır — bir datagramın `open`'ı, öncekinin işlenmesi bittikten sonradır. Test bunu kullanır:
//! (a) tüketici, blok 0'ın çözülmesi için gereken her çerçeve (R onarım + PARAM) seam'den geçip
//! ARDINDAN bir datagram daha açılana ve kanal doluluğu canlı sayaçta görünene dek bırakılmaz
//! (sabit uyku yok); (b) blok 1'de hedef birimin BÜTÜN DATA emisyonları, blok 1'in kurtarımı
//! garantileyen sayıda onarımı seam'den geçene dek düşer. Eski düzende (yalnız ilk emisyon düşer)
//! birimin ARQ yeniden iletimi onarımlardan önce gelebiliyordu: blok kaynaklarla tamamlanır, kurtarım
//! hiç olmaz (CI kırmızısı; teşhis PR gövdesinde).
//!
//! İkinci kol (fragmenter açık): kurtarılan chunk, daha önce ACK'lenmiş chunk'larla birlikte bir
//! MESAJI tamamlar ve kanal doluyken gelir. Mesaj birleştiriciden silinmemeli: aksi hâlde ACK'lenmiş
//! chunk'lar kaybolur ve mesaj hiç teslim edilmez.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use client_core::runtime::{
    ClientEndpoint, ClientStats, FecRuntimeConfig, RuntimeConfig, ServerEndpoint, ServerStats,
};
use client_core::seam::IdentitySeam;
use client_core::wire::{decode_data, decode_data_frag, decode_fec, decode_param};
use kama_contracts::s3::{
    Opened, PacketProtection, ProtectionClass, ProtectionContext, ProtectionLimits, SealError,
    SealedUnit,
};
use kama_transport_schema::inner_kind::FEC_REPAIR;

/// Teslim kanalı kapasitesi (dolsun diye küçük).
const QUEUE: usize = 4;
/// Profil 0x03 blok boyu.
const K: u64 = 8;
/// Statik repair sayısı (blok başına).
const R: u16 = 4;
/// Beklenen olayın üst sınırı.
const WAIT: Duration = Duration::from_secs(60);
/// Koşul yoklama aralığı (süre değil koşul beklenir; üst sınır [`WAIT`]).
const POLL: Duration = Duration::from_millis(1);
/// Tüketicinin durduğu süre: kanal dolar, repair'ler gelir, blok kanal doluyken çözülür.
const STALL: Duration = Duration::from_millis(1500);

/// Seam'in sunucu yönündeki gözlemi (test ↔ seam ortak). Seam sunucu görevinde datagram başına
/// sırayla çağrılır: `opened` bir arttığında önceki her datagramın işlenmesi bitmiştir.
#[derive(Debug, Default)]
struct SeamObs {
    /// `open` çağrısı (iç seam açtı; geçen + düşen).
    opened: u64,
    /// Blok başına seam'den GEÇEN onarım çerçevesi.
    repairs_passed: BTreeMap<u32, u64>,
    /// PARAM çerçevesi seam'den geçti.
    param_passed: bool,
    /// Birim başına düşürülen DATA emisyonu.
    dropped: BTreeMap<u64, u64>,
}

/// Kapı: `unit`'in BÜTÜN DATA emisyonları, seam `block`'un en az `repairs` onarımını geçirene dek
/// düşer; sonra geçer (ARQ yolu açık kalır).
#[derive(Debug, Clone, Copy)]
struct Hold {
    unit: u64,
    block: u32,
    repairs: u64,
}

/// IdentitySeam + alıcı yönünde: `first_drop` kümesindeki birimlerin YALNIZ İLK DATA emisyonu
/// düşer; `hold` varsa kapı kuralı uygulanır (S3 K5: `open` `None`, durum değişmez). Diğer her
/// emisyon geçer. Gözlem `obs`'ta.
struct ScriptedDrop {
    first_drop: Arc<Mutex<BTreeSet<u64>>>,
    hold: Option<Hold>,
    obs: Arc<Mutex<SeamObs>>,
    inner: IdentitySeam,
}

impl PacketProtection for ScriptedDrop {
    fn seal(&mut self, ctx: ProtectionContext, frame: &[u8]) -> Result<SealedUnit, SealError> {
        self.inner.seal(ctx, frame)
    }
    fn open(&mut self, wire: &[u8]) -> Option<Opened> {
        let od = self.inner.open(wire)?;
        let mut obs = self.obs.lock().expect("kilit");
        obs.opened += 1;
        let unit = decode_data(&od.frame)
            .map(|(_, u, _)| u)
            .or_else(|| decode_data_frag(&od.frame).map(|(_, u, _, _)| u));
        if let Some(u) = unit {
            let held = self.hold.is_some_and(|h| {
                h.unit == u && obs.repairs_passed.get(&h.block).copied().unwrap_or(0) < h.repairs
            });
            if held || self.first_drop.lock().expect("kilit").remove(&u) {
                *obs.dropped.entry(u).or_default() += 1;
                return None;
            }
        } else if let Some((h, _)) = decode_fec(&od.frame) {
            if h.kind == FEC_REPAIR {
                *obs.repairs_passed.entry(h.block_id).or_default() += 1;
            }
        } else if decode_param(&od.frame).is_some() {
            obs.param_passed = true;
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

fn cfg(frag: bool) -> RuntimeConfig {
    RuntimeConfig {
        // Profil 0x03: K=8, R=4, E=64 → chunk ≤ 64 − 11 = 53 B.
        max_unit_bytes: 48,
        max_message_bytes: frag.then_some(4096),
        fec: Some(FecRuntimeConfig {
            profile_id: 0x03,
            manual_r: Some(R),
        }),
        drain_cap: Duration::from_millis(300),
        delivery_queue: QUEUE,
        ..RuntimeConfig::default()
    }
}

/// Teslimleri `n` tanesine dek okur (kimlik → yük); her kimlik tam bir kez gelmeli.
async fn drain(
    server: &mut client_core::runtime::ServerHandle,
    n: usize,
) -> std::collections::BTreeMap<u64, Vec<u8>> {
    tokio::time::timeout(WAIT, async {
        let mut got = std::collections::BTreeMap::new();
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
    .expect("üst süre içinde bütün birimler teslim edilmeli (kayıp yok)")
}

async fn acked(client: &mut client_core::runtime::ClientHandle, n: u64) {
    tokio::time::timeout(WAIT, async {
        for _ in 0..n {
            client.acked().await.expect("ACK akışı açık");
        }
    })
    .await
    .expect("bütün birimler teyitli");
}

async fn finish(
    client: client_core::runtime::ClientHandle,
    server: client_core::runtime::ServerHandle,
) -> (ClientStats, ServerStats) {
    let cs = client.shutdown().await;
    let ss = server.shutdown().await;
    (cs, ss)
}

/// `cond` sağlanana dek bekler (koşul beklenir, süre değil); üst sınır [`WAIT`].
async fn wait_until(what: &str, mut cond: impl FnMut() -> bool) {
    tokio::time::timeout(WAIT, async {
        while !cond() {
            tokio::time::sleep(POLL).await;
        }
    })
    .await
    .unwrap_or_else(|_| panic!("üst süre içinde sağlanmalı: {what}"));
}

/// Tek-chunk birimler: blok kanal doluyken çözülür, kurtarılan birimler kanala giremez ve
/// RECOVERED'a yazılmaz; yeniden iletimle teslim edilir. Sonra ikinci blokta kanal boşken bir
/// kurtarım RECOVERED ile bildirilir (kontrol trafiği durmadı).
#[tokio::test(flavor = "multi_thread")]
async fn fec_recovery_while_delivery_full_loses_nothing_and_control_continues() {
    // Blok 1'de kapıdaki birim (ESI 2). Kurtarımı garantileyen onarım sayısı: blok 1 kapanırken
    // alıcıda K − 2 kaynak vardır (kapıdaki birim ile henüz gönderilmemiş son birim hariç) → çözüm
    // için 2 sembol daha gerekir; 2 onarım, son birimin varış sırasından bağımsız yeter.
    const HELD: u64 = K + 2;
    const HELD_UNTIL_REPAIRS: u64 = 2;
    assert!(
        QUEUE as u64 + u64::from(R) >= K && u64::from(R) >= HELD_UNTIL_REPAIRS,
        "düzenek: dolu kanaldaki kaynaklar + onarımlar bloğu çözer"
    );
    // Blok 0'da birim 2 yalnız ilk emisyonunda düşer; blok 1'de HELD kapıya bağlı.
    let first_drop = Arc::new(Mutex::new(BTreeSet::from([2u64])));
    let obs = Arc::new(Mutex::new(SeamObs::default()));
    let mut server = ServerEndpoint::bind(
        "127.0.0.1:0".parse().unwrap(),
        cfg(false),
        Box::new(ScriptedDrop {
            first_drop: Arc::clone(&first_drop),
            hold: Some(Hold {
                unit: HELD,
                block: 1,
                repairs: HELD_UNTIL_REPAIRS,
            }),
            obs: Arc::clone(&obs),
            inner: IdentitySeam::new(),
        }),
    )
    .await
    .expect("server bind");
    let mut client = ClientEndpoint::connect(
        server.local_addr(),
        cfg(false),
        Box::new(IdentitySeam::new()),
    )
    .await
    .expect("client connect");
    let payload = |i: u64| format!("fec-full-{i}").into_bytes();

    // ── Faz 1: tüketici durur; blok 0 kanal doluyken çözülür ──
    for i in 0..K {
        client.send(payload(i)).expect("offer");
    }
    // Tüketici, blok 0'ın çözümü için gereken her çerçeve alıcıda İŞLENENE dek durur (sabit uyku
    // yok): kanal doldu (canlı sayaç: dolu kanala çarpan DATA ⇒ kanaldaki QUEUE kaynak işlenmişti),
    // blok 0'ın R onarımı ve PARAM seam'den geçti (bağlanmadan gelen onarım bekletilir, PARAM
    // işlenince birleştiriciye girer) ve ARDINDAN bir datagram daha açıldı (seam sıralı ⇒
    // öncekilerin hepsi işlendi). Bu anda blokta QUEUE kaynak + R onarım ≥ K sembol vardır: blok,
    // kanal doluyken çözülmüştür.
    wait_until(
        "kanal doldu; blok 0'ın onarımları ve PARAM seam'den geçti",
        || {
            let o = obs.lock().expect("kilit");
            server.data_not_admitted_full() > 0
                && o.param_passed
                && o.repairs_passed.get(&0).copied().unwrap_or(0) >= u64::from(R)
        },
    )
    .await;
    let seen = obs.lock().expect("kilit").opened;
    wait_until("ardından bir datagram daha açıldı", || {
        obs.lock().expect("kilit").opened > seen
    })
    .await;
    assert_eq!(server.delivery_dropped(), 0, "dolu kanalda düşme yok");
    let got = drain(&mut server, K as usize).await;
    assert_eq!(
        got.keys().copied().collect::<Vec<_>>(),
        (0..K).collect::<Vec<_>>()
    );
    for (id, p) in &got {
        assert_eq!(p, &payload(*id), "yük bayt-eş (birim {id})");
    }
    acked(&mut client, K).await;

    // ── Faz 2: tüketici açık; blok 1'de HELD FEC ile kurtarılır ve RECOVERED gider ──
    // Blok 1'in ilk K − 1 kaynağı gider; blok kapanmadığı için onarım YOK, HELD'in her emisyonu
    // kapıda düşer. Tüketici HELD dışındaki K − 2 birimi alır: teslim ⇒ kabul ⇒ blok
    // birleştiricisinde kaynak; kanal boş.
    for i in K..2 * K - 1 {
        client.send(payload(i)).expect("offer");
    }
    let mut got2 = drain(&mut server, (K - 2) as usize).await;
    assert_eq!(
        got2.keys().copied().collect::<Vec<_>>(),
        (K..2 * K - 1).filter(|&i| i != HELD).collect::<Vec<_>>(),
        "ön koşul: blok 1 kapanmadan HELD dışındaki kaynaklar teslim edildi"
    );
    // Son kaynak bloğu kapatır → R onarım. Çözüm en geç ikinci onarımda, kanalda en çok son birim
    // varken olur; HELD'in DATA'sı ancak ondan sonra geçer.
    client.send(payload(2 * K - 1)).expect("offer");
    for (id, p) in drain(&mut server, 2).await {
        assert!(
            got2.insert(id, p).is_none(),
            "kimlik {id} iki kez teslim edildi"
        );
    }
    assert_eq!(
        got2.keys().copied().collect::<Vec<_>>(),
        (K..2 * K).collect::<Vec<_>>()
    );
    for (id, p) in &got2 {
        assert_eq!(p, &payload(*id), "yük bayt-eş (birim {id})");
    }
    acked(&mut client, K).await;
    assert!(
        first_drop.lock().expect("kilit").is_empty(),
        "birim 2'nin ilk emisyonu düşürüldü"
    );
    let held_drops = obs
        .lock()
        .expect("kilit")
        .dropped
        .get(&HELD)
        .copied()
        .unwrap_or(0);

    let (cs, ss) = finish(client, server).await;
    eprintln!(
        "ÖLÇÜM fec-full: delivered={} fec_recovered_units={} fec_recovered_not_admitted={} \
         data_not_admitted_full={} delivery_dropped={} fec_recovered_ctrl_tx={} \
         cs.fec_recovered_ctrl_units={} cs.tx_retx_bytes={} param_bound_peers={} \
         fec_blocks_decoded={} held_drops={held_drops}",
        ss.delivered,
        ss.fec_recovered_units,
        ss.fec_recovered_not_admitted,
        ss.data_not_admitted_full,
        ss.delivery_dropped,
        ss.fec_recovered_ctrl_tx,
        cs.fec_recovered_ctrl_units,
        cs.tx_retx_bytes,
        ss.param_bound_peers,
        ss.fec_blocks_decoded
    );
    assert_eq!(ss.delivered, 2 * K, "her birim tam bir kez: {ss:?}");
    assert_eq!(ss.delivery_dropped, 0, "ACK'lenmiş birim düşmedi: {ss:?}");
    assert!(
        held_drops >= 1,
        "ön koşul: HELD'in emisyonu onarımlardan önce kapıda düştü"
    );
    assert!(
        ss.fec_recovered_not_admitted > 0,
        "ön koşul: blok kanal doluyken çözüldü ve kurtarım kabul edilmedi: {ss:?}"
    );
    assert!(
        ss.data_not_admitted_full > 0,
        "ön koşul: kanal doldu: {ss:?}"
    );
    assert_eq!(
        ss.param_bound_peers, 1,
        "PARAM bağlandı (kontrol düzlemi): {ss:?}"
    );
    assert!(
        ss.fec_recovered_units >= 1,
        "kanal boşken kurtarılan birim teslim edildi: {ss:?}"
    );
    assert!(
        ss.fec_recovered_ctrl_tx >= 1 && cs.fec_recovered_ctrl_units >= 1,
        "kanal boşken kurtarım RECOVERED ile bildirildi (kontrol trafiği durmadı): {cs:?} {ss:?}"
    );
    assert_eq!(cs.undelivered, 0, "{cs:?}");
}

/// Fragmenter açık: mesaj A = 2 chunk (birim 0, 1). Birim 0'ın ilk emisyonu düşer; birim 1 gelir,
/// birleştiriciye girer ve ACK'lenir. Tek-chunk B..E kanalı doldurur. Repair'ler birim 0'ı kurtarır,
/// A tamamlanır AMA kanal doludur. Beklenen: A birleştiricide kalır (silinmez), kurtarım RECOVERED'a
/// yazılmaz, birim 0 yeniden iletilince A tamamlanıp teslim edilir. Eski davranış: A silinir,
/// ACK'lenmiş birim 1 kaybolur, A hiç teslim edilmez.
#[tokio::test(flavor = "multi_thread")]
async fn fec_completed_message_while_delivery_full_keeps_its_acked_chunks() {
    let pending = Arc::new(Mutex::new(BTreeSet::from([0u64])));
    let mut server = ServerEndpoint::bind(
        "127.0.0.1:0".parse().unwrap(),
        cfg(true),
        Box::new(ScriptedDrop {
            first_drop: Arc::clone(&pending),
            hold: None,
            obs: Arc::default(),
            inner: IdentitySeam::new(),
        }),
    )
    .await
    .expect("server bind");
    let mut client = ClientEndpoint::connect(
        server.local_addr(),
        cfg(true),
        Box::new(IdentitySeam::new()),
    )
    .await
    .expect("client connect");

    // A: 2 chunk (48 + 40 B). B..H: tek chunk. Toplam 1 + 7 mesaj = 9 birim (blok 0 = birim 0..7).
    let a: Vec<u8> = (0..88u32).map(|i| (i % 251) as u8).collect();
    client.send(a.clone()).expect("offer A");
    let singles: Vec<Vec<u8>> = (1..8u8).map(|i| vec![i; 20]).collect();
    for s in &singles {
        client.send(s.clone()).expect("offer");
    }
    tokio::time::sleep(STALL).await;
    assert_eq!(server.delivery_dropped(), 0, "dolu kanalda düşme yok");

    let got = drain(&mut server, 1 + singles.len()).await;
    assert_eq!(got.get(&0), Some(&a), "A tam ve bayt-eş teslim edildi");
    for (i, s) in singles.iter().enumerate() {
        assert_eq!(got.get(&(i as u64 + 1)), Some(s), "mesaj {}", i + 1);
    }
    acked(&mut client, 1 + singles.len() as u64).await;

    let (cs, ss) = finish(client, server).await;
    eprintln!(
        "ÖLÇÜM fec-frag-full: delivered={} msgs_reassembled={} fec_recovered_units={} \
         fec_recovered_not_admitted={} data_not_admitted_full={} delivery_dropped={} \
         msg_asm_evicted={}",
        ss.delivered,
        ss.msgs_reassembled,
        ss.fec_recovered_units,
        ss.fec_recovered_not_admitted,
        ss.data_not_admitted_full,
        ss.delivery_dropped,
        ss.msg_asm_evicted
    );
    assert_eq!(ss.delivered, 1 + singles.len() as u64, "{ss:?}");
    assert_eq!(ss.delivery_dropped, 0, "{ss:?}");
    assert_eq!(cs.undelivered, 0, "{cs:?}");
    assert!(
        ss.fec_recovered_not_admitted > 0,
        "ön koşul: A'yı tamamlayan kurtarım kanal doluyken geldi: {ss:?}"
    );
}
