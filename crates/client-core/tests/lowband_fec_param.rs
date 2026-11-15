//! PR #190 kontrolleri (sahip, 2026-11-11). (1) PARAM geri çekilmesi #202 ile main'e girdi; testi
//! `tests/param_backoff.rs` (belirlenimli, 6 s karartma). (2) normal bantta küçük birimli akışta FEC
//! teslimi/kurtarımı geriliyor mu (parite tavanı kontrolcü yolunda bloğun kaynak baytıyla
//! ölçeklenir; elle-r koluna uygulanmaz); (3) 25 B/s hatta tavan kararı başlangıç sırasından
//! (PARAM ↔ ilk veri, dolayısıyla ilk RTT) bağımsız mı; (4) şekillendirilmiş normal bant hatta
//! ölçülmüş kapasite zaman kapısını açıyor mu; (5) hat akış sürerken 12,5 kB/s'den 25 B/s'ye
//! düşünce zaman kapısı kapanıyor mu.
//!
//! Gerçek UDP; kayıp belirlenimli (alıcı seam'i / hat ikizinde sıra numarası). (2)–(5) netem
//! `rate` + `delay` sıra kuralını uygulayan kullanıcı-alanı hat ikizi üzerinden (netem/tc
//! gerekmez); gerçek zamanlıdır: (2) ≈ 4 s, (3) ≈ 50 s, (4) ≈ 5 s, (5) ≈ 20 s.

use std::collections::VecDeque;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use client_core::runtime::{ClientEndpoint, RuntimeConfig, ServerEndpoint};
use client_core::seam::IdentitySeam;
use kama_contracts::s3::{
    Opened, PacketProtection, ProtectionClass, ProtectionContext, ProtectionLimits, SealError,
    SealedUnit,
};

/// IdentitySeam + alıcıda belirli birimlerin DATA'sı kalıcı düşer (yeniden iletimler dahil);
/// onarım sembolleri ve kontrol çerçeveleri geçer.
struct DropUnitsSeam {
    drop: &'static [u64],
    inner: IdentitySeam,
}

impl PacketProtection for DropUnitsSeam {
    fn seal(&mut self, ctx: ProtectionContext, frame: &[u8]) -> Result<SealedUnit, SealError> {
        self.inner.seal(ctx, frame)
    }
    fn open(&mut self, wire: &[u8]) -> Option<Opened> {
        let od = self.inner.open(wire)?;
        let unit = client_core::wire::decode_data(&od.frame)
            .map(|(_, u, _)| u)
            .or_else(|| client_core::wire::decode_data_frag(&od.frame).map(|(_, u, _, _)| u));
        if unit.is_some_and(|u| self.drop.contains(&u)) {
            return None;
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

async fn collect(
    server: &mut client_core::runtime::ServerHandle,
    n: usize,
    budget: Duration,
) -> Vec<u64> {
    let mut got = std::collections::BTreeSet::new();
    let _ = tokio::time::timeout(budget, async {
        while got.len() < n {
            let d = server.next_delivery().await.expect("teslim akışı açık");
            got.insert(d.unit_id);
        }
    })
    .await;
    got.into_iter().collect()
}

/// (2) Normal bant, küçük birimli akış, kontrolcü yolu (ürün profili 0x05, E 1024), hızlı hat
/// ikizi (1,25 MB/s · tek yön 2 ms). Blok 0'da kayıp geçmişi kurulur (birim 3 alıcıda kalıcı
/// silinir), 1,5 s duraklama, blok 1 kayıp geçmişiyle kapanır (birim 11 kalıcı silinir).
///
/// İddia (400 B): parite tavanı normal bantta FEC'i kesmez — kontrolcünün istediği r tavanın
/// altında kalır (`fec_parity_payload_clamped = 0`), parite planlanır, tele çıkar, birim 11 FEC ile
/// kurtarılır, blok 1 8/8. Koşulsuz ve belirlenimli:
/// * Kontrolcünün oranı blok 1'de spekülatif koldan gelir (birim 3'ün şüpheli kaybı, L2 → 250‰ →
///   r 2). Motorun bütçe kırpması (`cap = BtlBw · 1000 / 125 000`, sender `fuzzy_evaluate`) bu
///   oranı ancak BtlBw ≥ 31,25 kB/s iken kesmez. RTT ≥ 4 ms olduğundan blok 0'ın ACK'leri geçerli
///   teslim hızı örneği verir (≥ 1 ms aralık); patlama örneği ≥ ~100 kB/s → kırpma 900‰.
/// * Tavan iki dalda da r 2'yi kesmez: zaman kapısı açıksa profil tavanı 4, kapalıysa bayt
///   kapısı ⌊3 416 / 1 052⌋ = 3. Kapının girdisi son örneği de okur; blok 1'in ilk ACK'i duraklamayı
///   kapsayan düşük örnek verebilir, sonuç değişmez.
///
/// Neden loopback değil: loopback'te RTT 1 ms'nin altına iner, blok 0'ın örnekleri reddedilir
/// (`bw.rs` `MIN_RATE_INTERVAL`) ve BtlBw 0 kalır. Blok 1'in 8. sunumundan önce bir ACK işlenirse
/// BtlBw duraklamayı kapsayan tek örnekle 284 B/s olur, bütçe kırpması 250‰'i 2‰'e indirir, r 0
/// (ölçüldü: yük altında 160 koşuda 4; kırpma 2‰ ya da 6‰, BtlBw 284 / 852 B/s,
/// `fec_parity_payload_clamped = 0` — tavan kesmedi).
/// Bu, tavandan bağımsız bir motor davranışıdır; aynı kırpma (4) testinde de görülür: 12,5 kB/s'de
/// BtlBw ≈ 3 kB/s → 24‰ → r 0.
///
/// 65 B: blok kaynağı 736 B < 1 052 B; zaman kapısı son örneğe bağlı (yukarıda) → bayt kapısında
/// r 0 olabilir. Yalnız karar sayısı ve teslim sınanır.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn small_unit_flow_keeps_fec_at_normal_band_for_400b_and_65b_units() {
    const K: usize = 8;
    for unit_len in [400usize, 65usize] {
        let mut p = shaped_pair_with_delay(
            1_250_000,
            Duration::from_millis(2),
            Start::DataFirst,
            &[],
            Box::new(DropUnitsSeam {
                drop: &[3, 11],
                inner: IdentitySeam::new(),
            }),
        )
        .await;
        // Blok 0: kayıp geçmişi (birim 3 kalıcı düşer; ilk blok ACK'siz kapanır → r = 0).
        for i in 0..K {
            p.client.send(vec![i as u8; unit_len]).expect("offer");
        }
        let got0 = collect(&mut p.server, K - 1, Duration::from_secs(10)).await;
        assert_eq!(
            got0.len(),
            K - 1,
            "birim {unit_len} B: blok 0'da 7/8 (3 kalıcı silik)"
        );
        tokio::time::sleep(Duration::from_millis(1500)).await;
        // Blok 1: kayıp geçmişiyle kapanır; birim 11 kalıcı düşer.
        for i in K..2 * K {
            p.client.send(vec![i as u8; unit_len]).expect("offer");
        }
        let got1 = collect(&mut p.server, K, Duration::from_secs(20)).await;
        let (cs, ss, _lens) = p.finish().await;
        println!(
            "[birim {unit_len} B] blok1 teslim={} karar={} zaman_kapısı={} kapasite_max={} B/s latched={} repairs_tx={} clamped={} recovered={} undelivered={}",
            got1.len(),
            cs.fec_cap_decisions,
            cs.fec_cap_time_gate_open,
            cs.fec_cap_path_bw_max_bps,
            cs.fec_blocks_latched,
            cs.fec_repairs_tx,
            cs.fec_parity_payload_clamped,
            ss.fec_recovered_units,
            cs.undelivered
        );
        assert_eq!(
            cs.fec_cap_decisions, 2,
            "birim {unit_len} B: iki tam blok: {cs:?}"
        );
        if unit_len == 400 {
            assert_eq!(
                cs.fec_parity_payload_clamped, 0,
                "400 B: tavan normal bantta kontrolcünün r'sini kesmez: {cs:?}"
            );
            assert!(
                cs.fec_blocks_latched >= 1,
                "400 B: kayıp geçmişli blok 1 parite planlar: {cs:?}"
            );
            assert!(cs.fec_repairs_tx >= 1, "400 B: parite tele çıkar: {cs:?}");
            assert!(
                ss.fec_recovered_units >= 1,
                "400 B: birim 11 FEC ile kurtarılır: {ss:?}"
            );
            assert_eq!(got1.len(), K, "400 B: blok 1 8/8 (11 FEC ile): {cs:?}");
        } else {
            assert!(got1.len() >= K - 1, "65 B: blok 1 en az 7/8 (ARQ): {cs:?}");
        }
    }
}

// ── Şekillendirilmiş hat (netem kuralı, belirlenimli kayıp) ────────────────────────────────────

/// Datagram başına tel zarfı (IPv4 20 + UDP 8): serileştirme süresine girer.
const LINE_OVERHEAD: u64 = 28;
/// Bu boyun altındaki datagram veri değildir (PARAM 51 B; ACK ve PARAM yanıtı daha küçük).
/// 65 B ileti 92 B, 400 B ileti 427 B, parite 1 052 B.
const DATA_MIN_LEN: usize = 80;
/// Profil 0x05 parite datagramı (UDP gövdesi).
const PARITY_DGRAM_LEN: usize = 1052;
/// Zaman kapısı eşiği: parite datagramı 2 s'de serileşir (1 052 B / 2 s).
const TIME_GATE_BPS: u64 = 526;

/// Tek yön hat: Linux netem `rate` + `delay` sıra kuralı (`sch_netem.c` `netem_enqueue`). Çıkış
/// anı `max(şimdi + gecikme, önceki çıkış) + (bayt + zarf) / hız`, FIFO. Kayıp belirlenimli: veri
/// boyundaki datagramların (`DATA_MIN_LEN` ve üstü) sıra numarası `drop_data` içindeyse datagram hatta
/// girmez (netem'deki gibi kaybolan paket hat süresi harcamaz). Desen zamanlamadan bağımsızdır.
struct Line {
    rate_bps: u64,
    delay: Duration,
    drop_data: &'static [u64],
    data_seen: u64,
    last_tts: Option<Instant>,
    wire: VecDeque<(Instant, Vec<u8>)>,
    /// Hatta giren datagramların UDP gövde boyları (gönderim sırasıyla).
    admitted_lens: Vec<usize>,
}

impl Line {
    fn new(rate_bps: u64, delay: Duration, drop_data: &'static [u64]) -> Self {
        Self {
            rate_bps,
            delay,
            drop_data,
            data_seen: 0,
            last_tts: None,
            wire: VecDeque::new(),
            admitted_lens: Vec::new(),
        }
    }

    fn offer(&mut self, now: Instant, pkt: Vec<u8>) {
        if pkt.len() >= DATA_MIN_LEN {
            let i = self.data_seen;
            self.data_seen += 1;
            if self.drop_data.contains(&i) {
                return;
            }
        }
        let ser = Duration::from_nanos(
            (pkt.len() as u64 + LINE_OVERHEAD) * 1_000_000_000 / self.rate_bps.max(1),
        );
        let start = self
            .last_tts
            .map_or(now + self.delay, |l| l.max(now + self.delay));
        let tts = start + ser;
        self.last_tts = Some(tts);
        self.admitted_lens.push(pkt.len());
        self.wire.push_back((tts, pkt));
    }

    fn take_due(&mut self, now: Instant) -> Vec<Vec<u8>> {
        let mut out = Vec::new();
        while self.wire.front().is_some_and(|(t, _)| *t <= now) {
            if let Some((_, p)) = self.wire.pop_front() {
                out.push(p);
            }
        }
        out
    }
}

/// Başlangıç sırası: istemcinin ilk veri datagramı ile ondan önce çıkan PARAM denemeleri hatta
/// hangi sırayla girer. Gerçek koşuda bu sıra istemci döngüsündeki yarıştan çıkar ve ilk RTT
/// örneğini belirler (25 B/s'de veri önce: RTT₁ kısa; PARAM önce: veri PARAM'ın serileştirmesi
/// kadar geç varır, RTT₁ uzar).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Start {
    DataFirst,
    ParamFirst,
}

/// Röle: ileri yönde hem bir veri hem bir PARAM datagramı görülene dek (en çok 1 s) gelenleri
/// tutar, `Start`'a göre sıralayıp hatta verir (veri önce ya da PARAM'lar önce; tür içi sıra
/// korunur); sonrası geldiği sırayla. Dönüş yönü olduğu gibi.
async fn line_relay(
    front: tokio::net::UdpSocket,
    back: tokio::net::UdpSocket,
    fwd: Arc<Mutex<Line>>,
    rev: Arc<Mutex<Line>>,
    start: Start,
    mut stop: tokio::sync::oneshot::Receiver<()>,
) {
    let mut client: Option<SocketAddr> = None;
    let mut held: Vec<Vec<u8>> = Vec::new();
    let mut released = false;
    let mut first_at: Option<Instant> = None;
    let mut up = vec![0u8; 2048];
    let mut down = vec![0u8; 2048];
    let mut tick = tokio::time::interval(Duration::from_millis(1));
    loop {
        tokio::select! {
            r = front.recv_from(&mut up) => {
                let Ok((n, from)) = r else { continue };
                client = Some(from);
                let now = Instant::now();
                if released {
                    fwd.lock().expect("kilit").offer(now, up[..n].to_vec());
                } else {
                    first_at.get_or_insert(now);
                    held.push(up[..n].to_vec());
                }
            }
            r = back.recv(&mut down) => {
                let Ok(n) = r else { continue };
                rev.lock().expect("kilit").offer(Instant::now(), down[..n].to_vec());
            }
            _ = tick.tick() => {
                let now = Instant::now();
                let data_held = held.iter().any(|p| p.len() >= DATA_MIN_LEN);
                let ctrl_held = held.iter().any(|p| p.len() < DATA_MIN_LEN);
                if !released
                    && ((data_held && ctrl_held)
                        || first_at.is_some_and(|t| now.duration_since(t) > Duration::from_secs(1)))
                {
                    match start {
                        Start::DataFirst => held.sort_by_key(|p| p.len() < DATA_MIN_LEN),
                        Start::ParamFirst => held.sort_by_key(|p| p.len() >= DATA_MIN_LEN),
                    }
                    let mut f = fwd.lock().expect("kilit");
                    for p in held.drain(..) {
                        f.offer(now, p);
                    }
                    released = true;
                }
                let due_f = fwd.lock().expect("kilit").take_due(now);
                for p in due_f {
                    let _ = back.send(&p).await;
                }
                let due_r = rev.lock().expect("kilit").take_due(now);
                if let Some(to) = client {
                    for p in due_r {
                        let _ = front.send_to(&p, to).await;
                    }
                }
            }
            _ = &mut stop => break,
        }
    }
}

/// İstemci ↔ röle ↔ sunucu (ürün varsayılanı: profil 0x05 E 1024, port motoru), iki yön aynı hız
/// ve gecikme; kayıp yalnız ileri yönde ve belirlenimli.
struct ShapedPair {
    server: client_core::runtime::ServerHandle,
    client: client_core::runtime::ClientHandle,
    fwd: Arc<Mutex<Line>>,
    rev: Arc<Mutex<Line>>,
    stop: tokio::sync::oneshot::Sender<()>,
    relay: tokio::task::JoinHandle<()>,
}

async fn shaped_pair(
    rate_bps: u64,
    start: Start,
    drop_data: &'static [u64],
    server_seam: Box<dyn PacketProtection>,
) -> ShapedPair {
    shaped_pair_with_delay(
        rate_bps,
        Duration::from_millis(300),
        start,
        drop_data,
        server_seam,
    )
    .await
}

/// [`shaped_pair`], tek yön gecikmesi verilerek.
async fn shaped_pair_with_delay(
    rate_bps: u64,
    delay: Duration,
    start: Start,
    drop_data: &'static [u64],
    server_seam: Box<dyn PacketProtection>,
) -> ShapedPair {
    let cfg = RuntimeConfig::default();
    let server = ServerEndpoint::bind("127.0.0.1:0".parse().unwrap(), cfg.clone(), server_seam)
        .await
        .expect("server bind");
    let front = tokio::net::UdpSocket::bind("127.0.0.1:0")
        .await
        .expect("röle ön");
    let back = tokio::net::UdpSocket::bind("127.0.0.1:0")
        .await
        .expect("röle arka");
    back.connect(server.local_addr())
        .await
        .expect("röle→sunucu");
    let front_addr = front.local_addr().expect("adres");
    let fwd = Arc::new(Mutex::new(Line::new(rate_bps, delay, drop_data)));
    let rev = Arc::new(Mutex::new(Line::new(rate_bps, delay, &[])));
    let (stop, stop_rx) = tokio::sync::oneshot::channel();
    let relay = tokio::spawn(line_relay(
        front,
        back,
        Arc::clone(&fwd),
        Arc::clone(&rev),
        start,
        stop_rx,
    ));
    let client = ClientEndpoint::connect(front_addr, cfg, Box::new(IdentitySeam::new()))
        .await
        .expect("client connect");
    ShapedPair {
        server,
        client,
        fwd,
        rev,
        stop,
        relay,
    }
}

impl ShapedPair {
    /// Hat hızını iki yönde birden değiştirir (kuyruktaki datagramların çıkış anı değişmez;
    /// netem `tc qdisc change … rate` gibi).
    fn set_rate(&self, rate_bps: u64) {
        self.fwd.lock().expect("kilit").rate_bps = rate_bps;
        self.rev.lock().expect("kilit").rate_bps = rate_bps;
    }

    /// Kapatır: istemci ve sunucu istatistikleri, ileri hatta giren datagram boyları.
    async fn finish(
        self,
    ) -> (
        client_core::runtime::ClientStats,
        client_core::runtime::ServerStats,
        Vec<usize>,
    ) {
        let cs = self.client.shutdown().await;
        let ss = self.server.shutdown().await;
        let _ = self.stop.send(());
        let _ = self.relay.await;
        let lens = self.fwd.lock().expect("kilit").admitted_lens.clone();
        (cs, ss, lens)
    }
}

struct LowbandRun {
    start: Start,
    first_ack: Option<Duration>,
    cs: client_core::runtime::ClientStats,
    fwd_lens: Vec<usize>,
}

/// 25 B/s · 300 ms iki yön, 65 B ileti (92 B datagram ≈ 4,8 s hat). İleti 0 hemen sunulur; ilk
/// ACK'ten (RTT₁) sonra 1–6; bloğu kapatan ileti 7, t0 + 45 s'de. Kontrolcü pariteyi ancak
/// PreConverge çıkışı ve kayıp şüphesinden sonra ister (bu desende ilk risk ≈ 40 s); karar o
/// yüzden geç verilir.
async fn lowband_run(start: Start, drop_data: &'static [u64]) -> LowbandRun {
    let mut p = shaped_pair(25, start, drop_data, Box::new(IdentitySeam::new())).await;
    let t0 = Instant::now();
    p.client.send(vec![0x5A; 65]).expect("offer 0");
    let first_ack = tokio::time::timeout(Duration::from_secs(40), p.client.acked())
        .await
        .ok()
        .flatten()
        .map(|_| t0.elapsed());
    for i in 1..7u8 {
        p.client.send(vec![0x5A ^ i; 65]).expect("offer");
    }
    tokio::time::sleep(Duration::from_secs(45).saturating_sub(t0.elapsed())).await;
    p.client
        .send(vec![0xA5; 65])
        .expect("offer 7 (blok kapanır)");
    tokio::time::sleep(Duration::from_secs(3)).await;
    let (cs, _ss, fwd_lens) = p.finish().await;
    LowbandRun {
        start,
        first_ack,
        cs,
        fwd_lens,
    }
}

/// (3) 25 B/s hat (gerçek yığın r25 hücresinin bant/gecikme ikizi), ürün profili 0x05 (E 1024),
/// 65 B ileti bloğu. Aynı hat, aynı kayıp deseni; tek fark başlangıç sırası:
/// - veri önce → RTT₁ ≈ 7,8 s → pacer hedefi `ilk_pencere / RTT₁` ≈ 618 B/s;
/// - PARAM önce → RTT₁ ≈ 10,9 s → pacer hedefi ≈ 439 B/s.
///
/// Eski karar girdisi (pacer hedefi) zaman kapısını (1 052 B / 2 s = 526 B/s) yalnız birinci
/// sırada açıyordu: aynı hatta parite kararı başlangıç sırasına bağlıydı. Yeni girdi ölçülmüş yol
/// kapasitesidir (BtlBw); iki sırada da hattın ~25 B/s'sinin altında kalır → iki sırada da bayt
/// kapısı, tavan kontrolcünün istediği pariteyi keser, pahalı parite hatta çıkmaz.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn lowband_parity_cap_does_not_depend_on_start_order() {
    // Veri boyundaki ileri datagramların sıra numarası (0 = ilk veri, başlangıç değiş tokuşu).
    const DROP: &[u64] = &[1, 2, 3, 5, 6];
    let (a, b) = tokio::join!(
        lowband_run(Start::DataFirst, DROP),
        lowband_run(Start::ParamFirst, DROP)
    );
    let parity_on_line = |r: &LowbandRun| {
        r.fwd_lens
            .iter()
            .filter(|&&l| l >= PARITY_DGRAM_LEN)
            .count()
    };
    for r in [&a, &b] {
        println!(
            "[{:?}] RTT₁={:?} pacer={} B/s karar={} zaman_kapısı={} kapasite_max={} B/s clamped={} latched={} repairs_tx={} hatta_parite={} param={}",
            r.start,
            r.first_ack,
            r.cs.fate.activation.pacer_rate_bps,
            r.cs.fec_cap_decisions,
            r.cs.fec_cap_time_gate_open,
            r.cs.fec_cap_path_bw_max_bps,
            r.cs.fec_parity_payload_clamped,
            r.cs.fec_blocks_latched,
            r.cs.fec_repairs_tx,
            parity_on_line(r),
            r.cs.param_sends,
        );
    }
    // Ön koşul: iki sıra farklı RTT₁ üretir ve eski girdi kapıyı yalnız veri-önce sırasında açardı.
    // Bu tutmuyorsa test artık eski kusurun koşulunu üretmiyordur (hat ikizi ya da motor değişmiş).
    let (ta, tb) = (
        a.first_ack.expect("veri önce: ilk ACK"),
        b.first_ack.expect("PARAM önce: ilk ACK"),
    );
    assert!(
        tb > ta + Duration::from_secs(2),
        "ön koşul: PARAM önce RTT₁'i uzatmalı: {ta:?} ↔ {tb:?}"
    );
    assert!(
        a.cs.fate.activation.pacer_rate_bps >= TIME_GATE_BPS
            && b.cs.fate.activation.pacer_rate_bps < TIME_GATE_BPS,
        "ön koşul: pacer hedefi kapı eşiğinin iki yanında olmalı: {} ↔ {}",
        a.cs.fate.activation.pacer_rate_bps,
        b.cs.fate.activation.pacer_rate_bps
    );
    for r in [&a, &b] {
        let cs = &r.cs;
        assert!(
            cs.fec_cap_decisions >= 1,
            "[{:?}] blok kapanmalı: {cs:?}",
            r.start
        );
        assert_eq!(
            cs.fec_cap_time_gate_open, 0,
            "[{:?}] 25 B/s'de zaman kapısı açılmamalı: {cs:?}",
            r.start
        );
        assert!(
            cs.fec_cap_path_bw_max_bps > 0 && cs.fec_cap_path_bw_max_bps < TIME_GATE_BPS / 4,
            "[{:?}] karar girdisi ölçülmüş ve eşiğin çok altında olmalı: {} B/s",
            r.start,
            cs.fec_cap_path_bw_max_bps
        );
        assert!(
            cs.fec_parity_payload_clamped >= 1,
            "[{:?}] tavan kontrolcünün istediği pariteyi kesmeli: {cs:?}",
            r.start
        );
        assert_eq!(
            cs.fec_blocks_latched, 0,
            "[{:?}] parite planlanmamalı",
            r.start
        );
        assert_eq!(
            cs.fec_repairs_tx, 0,
            "[{:?}] parite gönderilmemeli",
            r.start
        );
        assert_eq!(
            parity_on_line(r),
            0,
            "[{:?}] hatta 1 052 B parite datagramı olmamalı: {:?}",
            r.start,
            r.fwd_lens
        );
    }
}

/// (4) Normal bant, şekillendirilmiş hat: 12,5 kB/s · 300 ms iki yön, 400 B birim, kontrolcü yolu.
/// Tavan kararının girdisi gerçek RTT'li bir hatta da kapasiteyi gösterir: blok 0 (8 birim bir
/// arada) kapandığında henüz ACK yoktur → kapasite ölçülmemiş → bayt kapısı. Blok 0'ın ACK'leri
/// teslim hızı örneği üretir (8 × 427 B bir RTT'de ≈ 3–4 kB/s ≥ 526 B/s) → blok 1 zaman kapısından
/// geçer, tavan kesmez. Kontrolcünün kendi r'si bu sınamanın dışındadır (kayıp desenine göre parite
/// yerine kopyalamayı seçebilir); parite gönderimi ve kurtarımı (2) testinde koşulsuz.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn normal_band_shaped_line_measured_capacity_opens_time_gate() {
    const K: usize = 8;
    // Veri boyundaki ileri datagramların sıra numarası: blok 0'da kayıp geçmişi (ARQ onarır).
    let mut p = shaped_pair(
        12_500,
        Start::DataFirst,
        &[1, 4],
        Box::new(IdentitySeam::new()),
    )
    .await;
    for i in 0..K {
        p.client.send(vec![i as u8; 400]).expect("offer");
    }
    let got0 = collect(&mut p.server, K, Duration::from_secs(20)).await;
    assert_eq!(got0.len(), K, "blok 0 8/8 (ARQ)");
    tokio::time::sleep(Duration::from_millis(1500)).await;
    for i in K..2 * K {
        p.client.send(vec![i as u8; 400]).expect("offer");
    }
    let got1 = collect(&mut p.server, K, Duration::from_secs(20)).await;
    let (cs, _ss, _lens) = p.finish().await;
    println!(
        "[12,5 kB/s 400 B] blok1 teslim={} karar={} zaman_kapısı={} kapasite_max={} B/s clamped={} latched={}",
        got1.len(),
        cs.fec_cap_decisions,
        cs.fec_cap_time_gate_open,
        cs.fec_cap_path_bw_max_bps,
        cs.fec_parity_payload_clamped,
        cs.fec_blocks_latched,
    );
    assert_eq!(cs.fec_cap_decisions, 2, "iki tam blok: {cs:?}");
    assert!(
        cs.fec_cap_time_gate_open >= 1 && cs.fec_cap_path_bw_max_bps >= TIME_GATE_BPS,
        "blok 1 ölçülmüş kapasiteyle zaman kapısından geçmeli: {cs:?}"
    );
    assert_eq!(
        cs.fec_parity_payload_clamped, 0,
        "normal bantta tavan kesmemeli: {cs:?}"
    );
    assert_eq!(got1.len(), K, "blok 1 8/8: {cs:?}");
}

/// (5) Hat akış sürerken düşer: 12,5 kB/s · 300 ms iki yön, sonra 25 B/s. Bloklar: 0 ve 1 hızlı
/// hatta (8 × 400 B, bir arada), 2 düşük hatta (8 × 65 B). Düşüşten sonra bir ileti gönderilip
/// ACK'i beklenir (hattın yeni hızını taşıyan ilk teslim hızı örneği), blok 2 ondan sonra kapanır.
///
/// BtlBw max filtresi hızlı hattın örneğini bırakmaz: düşük, uygulamayla sınırlı örnek yalnız mevcut
/// en büyükten büyükse filtreye girer ve pencere dolunca en yeni kabul edilmiş örnek tutulur. Girdi
/// yalnız BtlBw iken blok 2 de zaman kapısından geçiyordu (r_max = profil tavanı; 25 B/s'de
/// 1 052 B parite ≈ 43 s hat). Girdi son teslim hızı örneğiyle sınırlanınca blok 2 bayt kapısında
/// kalır: zaman kapısı yalnız blok 1'de açılır.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn rate_drop_mid_flow_closes_time_gate() {
    const K: usize = 8;
    let mut p = shaped_pair(12_500, Start::DataFirst, &[], Box::new(IdentitySeam::new())).await;
    for i in 0..K {
        p.client.send(vec![i as u8; 400]).expect("offer");
    }
    let got0 = collect(&mut p.server, K, Duration::from_secs(20)).await;
    assert_eq!(got0.len(), K, "blok 0 8/8");
    tokio::time::sleep(Duration::from_secs(1)).await;
    for i in K..2 * K {
        p.client.send(vec![i as u8; 400]).expect("offer");
    }
    let got1 = collect(&mut p.server, K, Duration::from_secs(20)).await;
    assert_eq!(got1.len(), K, "blok 1 8/8");
    // Blok 1'in ACK'leri istemciye dönsün (hızlı hat), sonra düşüş.
    tokio::time::sleep(Duration::from_secs(1)).await;
    p.set_rate(25);
    let probe = 2 * K as u64;
    p.client
        .send(vec![0x5A; 65])
        .expect("offer (düşüş sonrası ilk ileti)");
    let probe_acked = tokio::time::timeout(Duration::from_secs(60), async {
        while let Some(u) = p.client.acked().await {
            if u == probe {
                return true;
            }
        }
        false
    })
    .await
    .unwrap_or(false);
    assert!(probe_acked, "düşüş sonrası ilk ileti ACK'lenmeli");
    for i in 1..K {
        p.client.send(vec![0x5A ^ i as u8; 65]).expect("offer");
    }
    tokio::time::sleep(Duration::from_secs(1)).await;
    let (cs, _ss, _lens) = p.finish().await;
    println!(
        "[düşüş 12,5 kB/s → 25 B/s] karar={} zaman_kapısı={} kapasite_max={} B/s clamped={} latched={}",
        cs.fec_cap_decisions,
        cs.fec_cap_time_gate_open,
        cs.fec_cap_path_bw_max_bps,
        cs.fec_parity_payload_clamped,
        cs.fec_blocks_latched,
    );
    assert_eq!(cs.fec_cap_decisions, 3, "üç tam blok: {cs:?}");
    assert!(
        cs.fec_cap_path_bw_max_bps >= TIME_GATE_BPS,
        "blok 1 hızlı hatta zaman kapısından geçmeli: {cs:?}"
    );
    assert_eq!(
        cs.fec_cap_time_gate_open, 1,
        "zaman kapısı yalnız blok 1'de açılmalı (blok 2 düşük hatta): {cs:?}"
    );
}
