//! [CC-PORT · gerçek yığın hücresi] Ürün runtime'ı (ClientEndpoint ↔ ServerEndpoint, gerçek UDP)
//! kullanıcı-alanı biçimlendirici röle üzerinden: hız (bayt/s), sınırlı FIFO kuyruk (paket), tek
//! yön gecikme, tohumlu rastgele kayıp — iki yönde ayrı. netem/tc GEREKTİRMEZ; motor simülasyonu
//! (`adaptive-transport/tests/cc_port_sim.rs`) ile netem arasındaki "gerçek yığın" boşluğunu kapatır.
//!
//! Gerçek zamanlıdır (belirlenimli değil): kayıp deseni tohumla sabit, zamanlama değil. Kapı
//! eşikleri bu yüzden geniştir. `CC_SHAPED=1` ile koşar (ölçüm); `CC_SHAPED_TRACE=1` rölede her
//! sunulan paketi yazar (an, yön, tel baytı, kuyruk doluluğu, düşme) — paket boyutundan türü
//! okunur (ör. 25 B/s hücresinde 120 B = 65 B ileti, 79 B = PARAM, 1080 B = FEC parite);
//! `CC_SHAPED_ENGINE=legacy|cc-port`
//! tek motor; `CC_SHAPED_SEED`, `CC_SHAPED_UNITS`, `CC_SHAPED_RATE`, `CC_SHAPED_Q`,
//! `CC_SHAPED_DELAY_MS`, `CC_SHAPED_JITTER_MS` (normal σ; yeniden sıralama üretir), `CC_SHAPED_LOSS_PM`,
//! `CC_SHAPED_UNIT_BYTES`, `CC_SHAPED_REV_RATE` (dönüş hızı), `CC_SHAPED_IW`, `CC_SHAPED_TMAX_S`,
//! `CC_SHAPED_INTERVAL_MS` (ileti hücresi: birimler bu aralıkla sunulur; 0 = hepsi başta) hücre
//! anahtarları. Yön hızları (dağıtım profili): `CC_SHAPED_LINK=1` iki uca verir (alıcıda ACK
//! bütçesi, göndericide yol ilk penceresi ve profil pacer tavanı);
//! `CC_SHAPED_LINK_SIDE=server|client` yalnız bir uca; `CC_SHAPED_LINK_FWD` / `CC_SHAPED_LINK_REV`
//! profilin söylediği ileri/dönüş hızı (B/s; varsayılan hattın gerçek hızı — profil ≠ gerçek
//! hücresi için).
//!
//! Kuyruk ayrıştırması (yalnız ölçüm; varsayılan kapalı): `CC_SHAPED_TL=1` rölenin gördüğü her
//! paketi sabit boyutlu kayda yazar (koşu sırasında ayırma ve biçimlendirme yok), göndericinin
//! birim teyitlerini toplar ve alıcı bütün birimleri teslim ettikten sonra göndericinin son teyidini
//! bekler; koşu sonunda `ÖZET` / `TL-*` satırları yazılır (`CC_SHAPED_TL_DUMP=1` ileri paketleri de).
//! `CC_SHAPED_LIVE=1` ile birlikte prob sayacı ve PTO durumu da örneklenir. Deney kancaları:
//! `CC_SHAPED_DROP_UNIT=<birim>[:<n>]` bu birimin ileri yöndeki ilk `n` (varsayılan 1) iletimini
//! düşürür; `CC_SHAPED_DROP_FINAL_ACKS=<k>` alıcı bütün birimleri aldıktan sonra dönüşe sunulan ilk
//! `k` ACK çerçevesini düşürür.

use std::collections::VecDeque;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use adaptive_transport::ack::{AckFrame, FRAME_TYPE_ACK, FRAME_TYPE_ACK_V2};
use client_core::runtime::{ClientEndpoint, RuntimeConfig, ServerEndpoint, TransportEngine};
use client_core::seam::{IdentitySeam, IDENTITY_TAG};
use client_core::wire::{decode_data, FRAME_PARAM_ACK, FRAME_RECOVERED};
use tokio::net::UdpSocket;

const DATAGRAM_MAX: usize = 2048;
const TICK: Duration = Duration::from_millis(1);
/// UDP/IP zarfı (serileştirme hesabına girer; röle yalnız UDP gövdesini görür).
const UDP_IP_OVERHEAD: u64 = 28;
/// [KUYRUK-AYRIŞTIRMA] Kayıtta tutulan düz çerçeve başı (en büyük ACK V2 çerçevesi 15 + 4 × 16 B).
const TL_HEAD: usize = 80;
/// [KUYRUK-AYRIŞTIRMA] Yön başına kayıt sığası (baştan ayrılır; dolunca kayıt düşer ve sayılır).
const TL_CAP: usize = 4096;

/// [KUYRUK-AYRIŞTIRMA] Rölenin gördüğü bir paket (sabit boyutlu; çözümleme koşu sonunda).
#[derive(Debug, Clone, Copy)]
struct TlRec {
    offered: Instant,
    /// Karşı uca varış anı; düşen pakette `None`.
    arrives: Option<Instant>,
    /// 0 iletildi · 1 kuyruk taştı · 2 rastgele kayıp · 3 deney düşürmesi.
    fate: u8,
    /// Tel boyu (UDP/IP dahil).
    wire: u16,
    /// IdentitySeam etiketindeki PN.
    pn: u64,
    head: [u8; TL_HEAD],
    head_len: u8,
}

/// IdentitySeam datagramını düz çerçeve + PN'e ayırır (etiketin ilk 8 baytı PN, BE).
fn seam_split(pkt: &[u8]) -> Option<(&[u8], u64)> {
    if pkt.len() <= IDENTITY_TAG {
        return None;
    }
    let (frame, tag) = pkt.split_at(pkt.len() - IDENTITY_TAG);
    let pn = u64::from_be_bytes(tag[..8].try_into().ok()?);
    Some((frame, pn))
}

fn is_ack_frame(frame: &[u8]) -> bool {
    matches!(frame.first(), Some(&FRAME_TYPE_ACK | &FRAME_TYPE_ACK_V2))
}

/// Bir yönün biçimlendiricisi: serileştirme + sınırlı kuyruk + gecikme + rastgele kayıp.
struct Shaper {
    rate_bps: u64,
    limit: usize,
    delay: Duration,
    /// Gecikme jitter'ı (normal dağılım σ; netem `delay 300ms 60ms` gibi). Varışlar varış anına
    /// göre sıralanır → jitter yeniden sıralama üretir (netem'deki gibi).
    jitter: Duration,
    loss_permille: u64,
    lcg: u64,
    /// Serileştirmesi süren paketlerin bitiş anları (kuyruk doluluğu).
    busy: VecDeque<Instant>,
    /// Varış anı sırasıyla bekleyenler.
    wire: VecDeque<(Instant, Vec<u8>)>,
    sent: u64,
    bytes: u64,
    dropped_queue: u64,
    dropped_random: u64,
    /// `CC_SHAPED_TRACE=1`: her sunulan paket (an, yön, bayt, kuyruk, sonuç) stdout'a.
    trace: Option<(&'static str, Instant)>,
    /// [KUYRUK-AYRIŞTIRMA] `CC_SHAPED_TL=1` kayıtları (sığa `TL_CAP`, baştan ayrılır).
    tl: Option<Vec<TlRec>>,
    tl_overflow: u64,
    /// [deney (i)] İleri yön: bu birimin kalan belirlenimli düşürme sayısı.
    drop_unit: Option<(u64, u32)>,
    /// [deney (ii)] Dönüş yönü: alıcı bütün birimleri aldıktan (`complete`) sonra düşecek ACK sayısı.
    drop_final_acks: u32,
    complete: Option<Instant>,
    dropped_forced: u64,
}

impl Shaper {
    fn new(
        rate_bps: u64,
        limit: usize,
        delay: Duration,
        jitter: Duration,
        loss_permille: u64,
        seed: u64,
    ) -> Self {
        Self {
            rate_bps,
            limit,
            delay,
            jitter,
            loss_permille,
            lcg: seed
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407),
            busy: VecDeque::new(),
            wire: VecDeque::new(),
            sent: 0,
            bytes: 0,
            dropped_queue: 0,
            dropped_random: 0,
            trace: None,
            tl: None,
            tl_overflow: 0,
            drop_unit: None,
            drop_final_acks: 0,
            complete: None,
            dropped_forced: 0,
        }
    }

    /// [KUYRUK-AYRIŞTIRMA] Sabit boyutlu kayıt (sığa doluysa sayılır, ayırma yok).
    fn tl_push(&mut self, now: Instant, arrives: Option<Instant>, fate: u8, size: u64, pkt: &[u8]) {
        let Some(tl) = self.tl.as_mut() else {
            return;
        };
        if tl.len() >= TL_CAP {
            self.tl_overflow += 1;
            return;
        }
        let (frame, pn) = seam_split(pkt).unwrap_or((&[], u64::MAX));
        let n = frame.len().min(TL_HEAD);
        let mut head = [0u8; TL_HEAD];
        head[..n].copy_from_slice(&frame[..n]);
        tl.push(TlRec {
            offered: now,
            arrives,
            fate,
            wire: u16::try_from(size).unwrap_or(u16::MAX),
            pn,
            head,
            head_len: n as u8,
        });
    }

    /// [deney (i)/(ii)] Bu paket belirlenimli düşer mi (eşleşen paket düşürme hakkını tüketir).
    fn forced_drop(&mut self, pkt: &[u8]) -> bool {
        let Some((frame, _)) = seam_split(pkt) else {
            return false;
        };
        if let Some((unit, left)) = self.drop_unit.as_mut() {
            if *left > 0 && decode_data(frame).is_some_and(|(_, u, _)| u == *unit) {
                *left -= 1;
                return true;
            }
        }
        if self.complete.is_some() && self.drop_final_acks > 0 && is_ack_frame(frame) {
            self.drop_final_acks -= 1;
            return true;
        }
        false
    }

    fn rand_permille(&mut self) -> u64 {
        self.lcg = self
            .lcg
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (self.lcg >> 33) % 1000
    }

    /// [0, 1) belirlenimli.
    fn unit(&mut self) -> f64 {
        self.lcg = self
            .lcg
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        ((self.lcg >> 11) as f64) / ((1u64 << 53) as f64)
    }

    /// Jitter'lı gecikme: `delay + N(0, jitter)` (Box-Muller), alt sınır 0.
    fn delay_sample(&mut self) -> Duration {
        if self.jitter.is_zero() {
            return self.delay;
        }
        let u1 = self.unit().max(1e-12);
        let u2 = self.unit();
        let z = (-2.0 * u1.ln()).sqrt() * (2.0 * std::f64::consts::PI * u2).cos();
        let d = self.delay.as_secs_f64() + z * self.jitter.as_secs_f64();
        Duration::from_secs_f64(d.max(0.0))
    }

    fn trace_line(&self, now: Instant, size: u64, what: &str) {
        if let Some((tag, t0)) = self.trace {
            println!(
                "İZ t={:.2}s {tag} bayt={size} kuyruk={} {what}",
                now.duration_since(t0).as_secs_f64(),
                self.busy.len()
            );
        }
    }

    fn offer(&mut self, now: Instant, pkt: Vec<u8>) {
        while self.busy.front().is_some_and(|&t| t <= now) {
            self.busy.pop_front();
        }
        let size = pkt.len() as u64 + UDP_IP_OVERHEAD;
        if self.busy.len() >= self.limit {
            self.dropped_queue += 1;
            self.trace_line(now, size, "KUYRUK_DÜŞTÜ");
            self.tl_push(now, None, 1, size, &pkt);
            return;
        }
        self.trace_line(now, size, "");
        let start = self.busy.back().copied().unwrap_or(now).max(now);
        let ser = Duration::from_nanos(size * 1_000_000_000 / self.rate_bps.max(1));
        let done = start + ser;
        self.busy.push_back(done);
        self.sent += 1;
        self.bytes += size;
        // Rastgele çekim deney kancasından ÖNCE ve her pakette aynı sırayla: kanca açıkken de tohumun
        // kayıp deseni değişmez.
        let random_drop = self.loss_permille > 0 && self.rand_permille() < self.loss_permille;
        let forced = self.forced_drop(&pkt);
        if random_drop {
            self.dropped_random += 1;
            self.trace_line(now, size, "YOLDA_DÜŞTÜ");
            self.tl_push(now, None, 2, size, &pkt);
            return;
        }
        if forced {
            self.dropped_forced += 1;
            self.trace_line(now, size, "DENEY_DÜŞTÜ");
            self.tl_push(now, None, 3, size, &pkt);
            return;
        }
        let at = done + self.delay_sample();
        self.tl_push(now, Some(at), 0, size, &pkt);
        // Varış anına göre sıralı ekle (jitter yeniden sıralama üretebilir).
        let pos = self
            .wire
            .iter()
            .rposition(|(t, _)| *t <= at)
            .map_or(0, |i| i + 1);
        self.wire.insert(pos, (at, pkt));
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

#[derive(Debug, Clone, Copy)]
struct Dir {
    rate_bps: u64,
    limit: usize,
    delay: Duration,
    jitter: Duration,
    loss_permille: u64,
}

struct RelayStats {
    fwd_sent: u64,
    fwd_bytes: u64,
    fwd_dropped_queue: u64,
    fwd_dropped_random: u64,
    rev_sent: u64,
    rev_bytes: u64,
    rev_dropped_queue: u64,
}

/// Röle: `front` (istemci tarafı) ↔ `back` (sunucuya bağlı). Kilit `await` boyunca tutulmaz.
async fn relay(
    front: UdpSocket,
    back: UdpSocket,
    fwd: Arc<Mutex<Shaper>>,
    rev: Arc<Mutex<Shaper>>,
    units: u64,
    mut stop: tokio::sync::oneshot::Receiver<()>,
) {
    let mut client: Option<SocketAddr> = None;
    let mut up = vec![0u8; DATAGRAM_MAX];
    let mut down = vec![0u8; DATAGRAM_MAX];
    let mut tick = tokio::time::interval(TICK);
    // [KUYRUK-AYRIŞTIRMA] Alıcıya varan farklı birimler: hepsi varınca dönüş yönü `complete`
    // işaretlenir (deney (ii) ve son teyit ölçümü). Varan paketin başlığı okunur, ayırma yok.
    let mut arrived = vec![false; usize::try_from(units).unwrap_or(0)];
    let mut arrived_n = 0u64;
    loop {
        tokio::select! {
            r = front.recv_from(&mut up) => {
                let Ok((n, from)) = r else { continue };
                client = Some(from);
                fwd.lock().expect("kilit").offer(Instant::now(), up[..n].to_vec());
            }
            r = back.recv(&mut down) => {
                let Ok(n) = r else { continue };
                rev.lock().expect("kilit").offer(Instant::now(), down[..n].to_vec());
            }
            _ = tick.tick() => {
                let now = Instant::now();
                let due_f = fwd.lock().expect("kilit").take_due(now);
                for p in due_f {
                    if arrived_n < units {
                        let unit = seam_split(&p)
                            .and_then(|(f, _)| decode_data(f))
                            .map(|(_, u, _)| u);
                        if let Some(slot) = unit.and_then(|u| arrived.get_mut(u as usize)) {
                            if !*slot {
                                *slot = true;
                                arrived_n += 1;
                                if arrived_n == units {
                                    rev.lock().expect("kilit").complete = Some(now);
                                }
                            }
                        }
                    }
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

/// [ASİMETRİK-YOL] Hücrenin dağıtım profili: uçlara verilen yön hızları. Profil gerçek hattan
/// farklı olabilir (`fwd_bps`/`rev_bps` ≠ biçimlendirici hızları) ve yalnız bir uca verilebilir.
#[derive(Debug, Clone, Copy)]
struct LinkProfile {
    /// Profilin söylediği ileri yön (istemci → sunucu) hızı, B/s.
    fwd_bps: u64,
    /// Profilin söylediği dönüş yönü (sunucu → istemci) hızı, B/s.
    rev_bps: u64,
    /// Alıcıya (sunucu) verilir: ACK dönüş bütçesi.
    server: bool,
    /// Göndericiye (istemci) verilir: yol ilk penceresi.
    client: bool,
}

impl LinkProfile {
    /// İki uca, gerçek hat hızlarıyla.
    fn exact(fwd: Dir, rev: Dir) -> Self {
        Self {
            fwd_bps: fwd.rate_bps,
            rev_bps: rev.rate_bps,
            server: true,
            client: true,
        }
    }
}

struct Outcome {
    completed: Option<Duration>,
    delivered: u64,
    stats: RelayStats,
}

/// [KUYRUK-AYRIŞTIRMA] Ayna örneği sığası.
const LIVE_CAP: usize = 8192;

/// [KUYRUK-AYRIŞTIRMA] Göndericinin canlı aynasından prob sayacı ve PTO durumu (salt gözlem).
#[derive(Debug, Clone, Copy)]
struct LiveRec {
    at: Instant,
    tail_probes: u64,
    retransmits: u64,
    srtt_us: u64,
    base_pto_ns: u64,
    tail_pto_ns: u64,
    probe_index: u32,
    armed_at: Option<Instant>,
    btlbw_bps: u64,
    pacer_rate_bps: u64,
    pacer_tokens: u64,
    inflight_bytes: u64,
    pending_lost: u32,
}

impl LiveRec {
    fn of(at: Instant, l: &client_core::runtime::ClientLiveDiag) -> Self {
        let timers = l.recovery.map(|r| r.timers);
        let armed_at = match (l.clock_origin, timers.and_then(|t| t.tail_armed_at_ns)) {
            (Some(o), Some(ns)) => Some(o + Duration::from_nanos(ns)),
            _ => None,
        };
        Self {
            at,
            tail_probes: l.tail_probes,
            retransmits: l.retransmits,
            srtt_us: l.srtt_us,
            base_pto_ns: timers.map_or(0, |t| t.base_pto_ns),
            tail_pto_ns: timers.map_or(0, |t| t.tail_pto_ns),
            probe_index: timers.map_or(0, |t| t.tail_probe_index),
            armed_at,
            btlbw_bps: l.control.btlbw_bps,
            pacer_rate_bps: l.control.pacer_rate_bps,
            pacer_tokens: l.control.pacer_tokens,
            inflight_bytes: l.control.inflight_bytes,
            pending_lost: timers.map_or(0, |t| t.pending_lost_len),
        }
    }

    /// Kayda değer değişiklik: sayaç, tur ya da pencere değişti ya da son kayıttan 250 ms geçti.
    fn differs(&self, next: &Self) -> bool {
        self.tail_probes != next.tail_probes
            || self.retransmits != next.retransmits
            || self.probe_index != next.probe_index
            || self.armed_at != next.armed_at
            || self.pending_lost != next.pending_lost
            || next.at.duration_since(self.at) >= Duration::from_millis(250)
    }
}

struct TlInput<'a> {
    t0: Instant,
    seed: u64,
    completed: Option<Duration>,
    deliv_at: &'a [Option<Instant>],
    conf_at: &'a [Option<Instant>],
    fwd: &'a [TlRec],
    rev: &'a [TlRec],
    overflow: u64,
    complete_at: Option<Instant>,
    live: &'a [LiveRec],
    live_end: Option<client_core::runtime::ClientLiveDiag>,
    probes: u64,
    retransmits: u64,
    forced: u64,
}

/// [KUYRUK-AYRIŞTIRMA] Koşu sonu çözümleme ve yazım (ölçüm bittikten sonra; sıcak yol değil).
fn tl_report(x: &TlInput<'_>) {
    let s = |i: Instant| i.saturating_duration_since(x.t0).as_secs_f64();
    let so = |i: Option<Instant>| i.map_or("-".to_string(), |i| format!("{:.2}", s(i)));
    let fate = |f: u8| match f {
        0 => "ok",
        1 => "kuyruk_düştü",
        2 => "yolda_düştü",
        _ => "DENEY_düştü",
    };
    // PN → birim (ileri DATA kayıtlarından).
    let mut pn_unit: std::collections::HashMap<u64, u64> = std::collections::HashMap::new();
    let mut max_pn = 0u64;
    for r in x.fwd {
        if let Some((_, u, _)) = decode_data(&r.head[..usize::from(r.head_len)]) {
            pn_unit.insert(r.pn, u);
        }
        if r.pn != u64::MAX {
            max_pn = max_pn.max(r.pn);
        }
    }
    let last_deliv = x.deliv_at.iter().flatten().max().copied();
    let all_conf = x.conf_at.iter().all(Option::is_some);
    let last_conf = if all_conf {
        x.conf_at.iter().flatten().max().copied()
    } else {
        None
    };
    // Dönüş çözümlemesi: varış sırasıyla (FIFO, jitter yok) yeni teyit edilen birimler.
    let mut acked_units: std::collections::HashSet<u64> = std::collections::HashSet::new();
    let (mut ack_n, mut ack_b, mut ack_b_conf, mut ack_b_done) = (0u64, 0u64, 0u64, 0u64);
    let (mut pa_n, mut pa_b, mut rc_n, mut rc_b, mut other_b) = (0u64, 0u64, 0u64, 0u64, 0u64);
    let done_inst = x.completed.map(|d| x.t0 + d);
    let mut lines: Vec<String> = Vec::new();
    for r in x.rev {
        let frame = &r.head[..usize::from(r.head_len)];
        let wire = u64::from(r.wire);
        match frame.first() {
            Some(&FRAME_TYPE_ACK | &FRAME_TYPE_ACK_V2) => {
                ack_n += 1;
                ack_b += wire;
                if last_conf.is_none_or(|c| r.offered <= c) {
                    ack_b_conf += wire;
                }
                if done_inst.is_none_or(|c| r.offered <= c) {
                    ack_b_done += wire;
                }
                let Ok(a) = AckFrame::decode(frame) else {
                    lines.push(format!("TL-ACK sunum={:.2} çözülemedi", s(r.offered)));
                    continue;
                };
                let pns = a.acked_full(max_pn + 1);
                let mut fresh: Vec<u64> = Vec::new();
                if r.fate == 0 {
                    for pn in &pns {
                        if let Some(&u) = pn_unit.get(pn) {
                            if acked_units.insert(u) {
                                fresh.push(u);
                            }
                        }
                    }
                }
                fresh.sort_unstable();
                lines.push(format!(
                    "TL-ACK sunum={:.2} varış={} sonuç={} bayt={wire} aralık={} alıcı_tutma={:.0}ms largest_pn={} teyitli_pn={} yeni_birim={fresh:?}",
                    s(r.offered),
                    so(r.arrives),
                    fate(r.fate),
                    a.ranges.len(),
                    a.ack_delay_duration().as_secs_f64() * 1000.0,
                    pns.iter().copied().max().unwrap_or(0),
                    pns.len(),
                ));
            }
            Some(&FRAME_PARAM_ACK) => {
                pa_n += 1;
                pa_b += wire;
                lines.push(format!(
                    "TL-PARAMACK sunum={:.2} varış={} bayt={wire}",
                    s(r.offered),
                    so(r.arrives)
                ));
            }
            Some(&FRAME_RECOVERED) => {
                rc_n += 1;
                rc_b += wire;
                lines.push(format!(
                    "TL-RECOVERED sunum={:.2} varış={} bayt={wire}",
                    s(r.offered),
                    so(r.arrives)
                ));
            }
            _ => other_b += wire,
        }
    }
    // İleri: birim başına iletimler.
    let mut per_unit: std::collections::BTreeMap<u64, Vec<&TlRec>> =
        std::collections::BTreeMap::new();
    let (mut data_n, mut data_b, mut param_n) = (0u64, 0u64, 0u64);
    for r in x.fwd {
        let frame = &r.head[..usize::from(r.head_len)];
        if let Some((_, u, _)) = decode_data(frame) {
            per_unit.entry(u).or_default().push(r);
            data_n += 1;
            data_b += u64::from(r.wire);
        } else if frame.first() == Some(&client_core::wire::FRAME_PARAM) {
            param_n += 1;
        }
    }
    let gap = match (last_deliv, last_conf) {
        (Some(d), Some(c)) => format!("{:.2}", c.saturating_duration_since(d).as_secs_f64()),
        _ => "-".to_string(),
    };
    println!(
        "ÖZET tohum={} tamamlanma={} alıcı_son_teslim={} hepsi_alıcıda={} gönderici_son_teyit={} teyit_farkı={gap} prob={} retx={} deney_düşürme={} ileri_veri={data_n}/{data_b}B param={param_n} dönüş_ack={ack_n}/{ack_b}B dönüş_ack_teslime_dek={ack_b_done}B dönüş_ack_teyide_dek={ack_b_conf}B param_ack={pa_n}/{pa_b}B recovered={rc_n}/{rc_b}B dönüş_diğer={other_b}B kayıt_taşma={}",
        x.seed,
        x.completed.map_or("YOK".to_string(), |d| format!("{:.2}", d.as_secs_f64())),
        so(last_deliv),
        so(x.complete_at),
        so(last_conf),
        x.probes,
        x.retransmits,
        x.forced,
        x.overflow,
    );
    for l in &lines {
        println!("{l}");
    }
    // Son 5 teslim edilen birim: iletimleri, teslim ve teyit.
    let mut order: Vec<(Instant, u64)> = x
        .deliv_at
        .iter()
        .enumerate()
        .filter_map(|(u, t)| t.map(|t| (t, u as u64)))
        .collect();
    order.sort_unstable();
    for &(t, u) in order.iter().rev().take(5) {
        let tx: Vec<String> = per_unit
            .get(&u)
            .map(|v| {
                v.iter()
                    .map(|r| {
                        format!(
                            "{:.2}[pn{} {}→{}]",
                            s(r.offered),
                            r.pn,
                            fate(r.fate),
                            so(r.arrives)
                        )
                    })
                    .collect()
            })
            .unwrap_or_default();
        println!(
            "TL-BİRİM u={u} teslim={:.2} teyit={} iletim={}",
            s(t),
            so(x.conf_at.get(u as usize).copied().flatten()),
            tx.join(" ")
        );
    }
    for l in x.live {
        println!(
            "TL-CANLI t={:.2} prob={} retx={} srtt={}ms taban_pto={}ms pto={}ms tur={} pencere={} btlbw={} pacer={}B/s jeton={} uçuş={}B kayıp_kuyruğu={}",
            s(l.at),
            l.tail_probes,
            l.retransmits,
            l.srtt_us / 1000,
            l.base_pto_ns / 1_000_000,
            l.tail_pto_ns / 1_000_000,
            l.probe_index,
            so(l.armed_at),
            l.btlbw_bps,
            l.pacer_rate_bps,
            l.pacer_tokens,
            l.inflight_bytes,
            l.pending_lost,
        );
    }
    if let Some(l) = x.live_end.as_ref() {
        let origin = l.clock_origin;
        for a in l.acks.recent() {
            let at = origin.map_or(0.0, |o| s(o + Duration::from_nanos(a.at_ns)));
            println!(
                "TL-MOTOR-ACK t={at:.2} yeni_birim={} uçuş={}→{} kayıp_kuyruğu={}→{} kopya={}→{} çıkan_datagram={} prob={}→{}",
                a.new_units,
                a.before.inflight_len,
                a.after.inflight_len,
                a.before.pending_lost_len,
                a.after.pending_lost_len,
                a.before.dup_queue_len,
                a.after.dup_queue_len,
                a.tx_datagrams_during,
                a.before.tail_probes,
                a.after.tail_probes,
            );
        }
    }
    if std::env::var("CC_SHAPED_TL_DUMP").is_ok() {
        for r in x.fwd {
            let frame = &r.head[..usize::from(r.head_len)];
            let what = decode_data(frame).map_or_else(
                || format!("tür=0x{:02x}", frame.first().copied().unwrap_or(0)),
                |(_, u, _)| format!("birim={u}"),
            );
            println!(
                "TL-İLERİ sunum={:.3} pn={} {what} bayt={} sonuç={} varış={}",
                s(r.offered),
                r.pn,
                r.wire,
                fate(r.fate),
                so(r.arrives)
            );
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn run_cell(
    engine: TransportEngine,
    fwd_cfg: Dir,
    rev_cfg: Dir,
    seed: u64,
    units: u64,
    unit_bytes: usize,
    t_max: Duration,
    interval: Duration,
    link: Option<LinkProfile>,
) -> Outcome {
    let cfg = RuntimeConfig {
        engine,
        // `CC_SHAPED_LIVE=1`: gönderici canlı aynası (salt gözlem) — saniyede bir iz satırı.
        live_diag: std::env::var("CC_SHAPED_LIVE").is_ok(),
        // `CC_SHAPED_IW=<B>`: ilk pencere (keşif).
        initial_window_bytes: std::env::var("CC_SHAPED_IW")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(client_core::runtime::KamaV1Profile::INITIAL_WINDOW_BYTES),
        ..RuntimeConfig::default()
    };
    // [ASİMETRİK-YOL] Yön hızları her uca kendi bakış açısından — alıcı (sunucu) yukarı = dönüş,
    // aşağı = ileri; gönderici (istemci) yukarı = ileri, aşağı = dönüş.
    let server_cfg = RuntimeConfig {
        link_rates: link
            .filter(|l| l.server)
            .map(|l| client_core::runtime::LinkRates {
                uplink_bps: l.rev_bps,
                downlink_bps: l.fwd_bps,
            }),
        ..cfg.clone()
    };
    let cfg = RuntimeConfig {
        link_rates: link
            .filter(|l| l.client)
            .map(|l| client_core::runtime::LinkRates {
                uplink_bps: l.fwd_bps,
                downlink_bps: l.rev_bps,
            }),
        ..cfg
    };
    let server = ServerEndpoint::bind(
        "127.0.0.1:0".parse().unwrap(),
        server_cfg,
        Box::new(IdentitySeam::new()),
    )
    .await
    .expect("server bind");
    let front = UdpSocket::bind("127.0.0.1:0").await.expect("röle ön");
    let back = UdpSocket::bind("127.0.0.1:0").await.expect("röle arka");
    back.connect(server.local_addr())
        .await
        .expect("röle→sunucu");
    let front_addr = front.local_addr().expect("adres");
    let fwd = Arc::new(Mutex::new(Shaper::new(
        fwd_cfg.rate_bps,
        fwd_cfg.limit,
        fwd_cfg.delay,
        fwd_cfg.jitter,
        fwd_cfg.loss_permille,
        seed,
    )));
    let rev = Arc::new(Mutex::new(Shaper::new(
        rev_cfg.rate_bps,
        rev_cfg.limit,
        rev_cfg.delay,
        rev_cfg.jitter,
        rev_cfg.loss_permille,
        seed ^ 0x5A5A,
    )));
    if std::env::var("CC_SHAPED_TRACE").is_ok() {
        let t0 = Instant::now();
        fwd.lock().expect("kilit").trace = Some(("İLERİ", t0));
        rev.lock().expect("kilit").trace = Some(("DÖNÜŞ", t0));
    }
    // [KUYRUK-AYRIŞTIRMA] Kayıt ve deney kancaları (yalnız ölçüm anahtarlarıyla; kapalıyken röle
    // bugünküyle aynı).
    let tl_on = std::env::var("CC_SHAPED_TL").is_ok();
    {
        let mut f = fwd.lock().expect("kilit");
        let mut r = rev.lock().expect("kilit");
        if tl_on {
            f.tl = Some(Vec::with_capacity(TL_CAP));
            r.tl = Some(Vec::with_capacity(TL_CAP));
        }
        f.drop_unit = std::env::var("CC_SHAPED_DROP_UNIT").ok().and_then(|v| {
            let mut it = v.split(':');
            let unit = it.next()?.parse().ok()?;
            let n = it.next().map_or(Some(1), |n| n.parse().ok())?;
            Some((unit, n))
        });
        r.drop_final_acks = u32::try_from(env_u64("CC_SHAPED_DROP_FINAL_ACKS", 0)).unwrap_or(0);
    }
    let (stop_tx, stop_rx) = tokio::sync::oneshot::channel();
    let relay_task = tokio::spawn(relay(
        front,
        back,
        Arc::clone(&fwd),
        Arc::clone(&rev),
        units,
        stop_rx,
    ));

    let mut client = ClientEndpoint::connect(front_addr, cfg, Box::new(IdentitySeam::new()))
        .await
        .expect("client connect");
    let mut server = server;
    let t0 = Instant::now();
    let mut offered = 0u64;
    let mut offer_at: Vec<Instant> = Vec::with_capacity(units as usize);
    let mut offer_one = |client: &client_core::runtime::ClientHandle,
                         offer_at: &mut Vec<Instant>| {
        let mut payload = vec![0x5Au8; unit_bytes];
        payload[..8].copy_from_slice(&offered.to_be_bytes());
        client.send(payload).expect("offer");
        offer_at.push(Instant::now());
        offered += 1;
    };
    if interval.is_zero() {
        for _ in 0..units {
            offer_one(&client, &mut offer_at);
        }
    } else {
        offer_one(&client, &mut offer_at);
    }
    let mut delivered = 0u64;
    let mut delays: Vec<Duration> = Vec::new();
    let mut marks: [Option<Duration>; 3] = [None; 3];
    // [KUYRUK-AYRIŞTIRMA] Birim başına alıcı teslimi ve gönderici teyidi (sığa baştan); ayna örnekleri.
    let n_units = usize::try_from(units).unwrap_or(0);
    let mut deliv_at: Vec<Option<Instant>> = vec![None; n_units];
    let mut conf_at: Vec<Option<Instant>> = vec![None; n_units];
    let mut confirmed = 0u64;
    let mut done_at: Option<Duration> = None;
    let live_on = std::env::var("CC_SHAPED_LIVE").is_ok();
    let mut live_log: Vec<LiveRec> =
        Vec::with_capacity(if tl_on && live_on { LIVE_CAP } else { 0 });
    let loop_end = tokio::time::timeout(t_max, async {
        let mut next_offer = tokio::time::interval(if interval.is_zero() {
            Duration::from_secs(3600)
        } else {
            interval
        });
        next_offer.tick().await;
        let mut live_tick = tokio::time::interval(Duration::from_secs(1));
        let mut conf_tick = tokio::time::interval(Duration::from_millis(2));
        // TL kipinde alıcı bitirdikten sonra da göndericinin son teyidi beklenir.
        while delivered < units || (tl_on && confirmed < units) {
            let d = tokio::select! {
                d = server.next_delivery(), if delivered < units => d,
                _ = conf_tick.tick(), if tl_on => {
                    let now = Instant::now();
                    while let Some(u) = client.try_acked() {
                        if let Some(slot) = conf_at.get_mut(u as usize) {
                            if slot.is_none() {
                                *slot = Some(now);
                                confirmed += 1;
                            }
                        }
                    }
                    if live_on && live_log.len() < LIVE_CAP {
                        let rec = LiveRec::of(now, &client.live_diag());
                        if live_log.last().is_none_or(|p| p.differs(&rec)) {
                            live_log.push(rec);
                        }
                    }
                    continue;
                }
                _ = live_tick.tick(), if live_on && !tl_on => {
                    let l = client.live_diag();
                    let c = l.control;
                    println!(
                        "CANLI t={:>5.1}s teslim={delivered} durum={:?} btlbw={} B/s pacer={} B/s uçuş={} B srtt={} ms rtprop={} ms rtt_örnek={} retx={} prob={} ack_çerçeve={}",
                        t0.elapsed().as_secs_f64(),
                        c.state,
                        c.btlbw_bps,
                        c.pacer_rate_bps,
                        c.inflight_bytes,
                        l.srtt_us / 1000,
                        l.rtprop_us / 1000,
                        l.rtt_samples,
                        l.retransmits,
                        l.tail_probes,
                        l.ack.frames,
                    );
                    continue;
                }
                _ = next_offer.tick(), if !interval.is_zero() && offer_at.len() < units as usize => {
                    offer_one(&client, &mut offer_at);
                    continue;
                }
            };
            if let Some(d) = d {
                delivered += 1;
                if let Some(&t) = offer_at.get(d.unit_id as usize) {
                    delays.push(t.elapsed());
                }
                if let Some(slot) = deliv_at.get_mut(d.unit_id as usize) {
                    *slot = Some(Instant::now());
                }
                for (i, pct) in [10u64, 50, 90].iter().enumerate() {
                    if marks[i].is_none() && delivered * 100 >= units * pct {
                        marks[i] = Some(t0.elapsed());
                    }
                }
                if delivered == units {
                    done_at = Some(t0.elapsed());
                }
            } else {
                break;
            }
        }
        t0.elapsed()
    })
    .await
    .ok();
    // TL kipinde tamamlanma = alıcının son teslimi (son teyit beklenirken zaman aşımı olsa da).
    let completed = if tl_on { done_at } else { loop_end };
    // [KUYRUK-AYRIŞTIRMA] Göndericinin ACK çerçevesi halkası (motorun olay öncesi/sonrası görünümü).
    let live_end = (tl_on && live_on).then(|| client.live_diag());
    let f = |d: Option<Duration>| d.map_or("-".to_string(), |d| format!("{:.1}", d.as_secs_f64()));
    delays.sort_unstable();
    let p = |q: usize| {
        delays
            .get((delays.len() * q / 100).min(delays.len().saturating_sub(1)))
            .map(|d| format!("{:.2}", d.as_secs_f64()))
            .unwrap_or("-".to_string())
    };
    println!(
        "[{engine:?}] PROFİL: %10={} %50={} %90={} son={} birim_gecikme p50={} p95={}",
        f(marks[0]),
        f(marks[1]),
        f(marks[2]),
        f(completed),
        p(50),
        p(95)
    );
    let cstats = client.shutdown().await;
    let _ = server.shutdown().await;
    let _ = stop_tx.send(());
    let _ = relay_task.await;
    let f = fwd.lock().expect("kilit");
    let r = rev.lock().expect("kilit");
    if tl_on {
        tl_report(&TlInput {
            t0,
            seed,
            completed,
            deliv_at: &deliv_at,
            conf_at: &conf_at,
            fwd: f.tl.as_deref().unwrap_or(&[]),
            rev: r.tl.as_deref().unwrap_or(&[]),
            overflow: f.tl_overflow + r.tl_overflow,
            complete_at: r.complete,
            live: &live_log,
            live_end,
            probes: cstats.fate.activation.tail_probes,
            retransmits: cstats.fate.activation.retransmits,
            forced: f.dropped_forced + r.dropped_forced,
        });
    }
    Outcome {
        completed,
        delivered,
        stats: RelayStats {
            fwd_sent: f.sent,
            fwd_bytes: f.bytes,
            fwd_dropped_queue: f.dropped_queue,
            fwd_dropped_random: f.dropped_random,
            rev_sent: r.sent,
            rev_bytes: r.bytes,
            rev_dropped_queue: r.dropped_queue,
        },
    }
}

fn env_u64(k: &str, d: u64) -> u64 {
    std::env::var(k)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(d)
}

/// Ölçüm hücresi (yalnız `CC_SHAPED=1`): varsayılan 12,5 kB/s · 6 paket kuyruk · 300 ms tek yön ·
/// %30 kayıp (manşet yaklaşığı, p1000) · 150 × 1000 B (150 kB), dönüş yolu aynı hız/gecikme, kayıpsız.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn shaped_relay_measure() {
    if std::env::var("CC_SHAPED").is_err() {
        return;
    }
    let rate = env_u64("CC_SHAPED_RATE", 12_500);
    let q = env_u64("CC_SHAPED_Q", 6) as usize;
    let delay = Duration::from_millis(env_u64("CC_SHAPED_DELAY_MS", 300));
    let jitter = Duration::from_millis(env_u64("CC_SHAPED_JITTER_MS", 0));
    let loss = env_u64("CC_SHAPED_LOSS_PM", 300);
    let units = env_u64("CC_SHAPED_UNITS", 150);
    let unit_bytes = env_u64("CC_SHAPED_UNIT_BYTES", 1000) as usize;
    let seed = env_u64("CC_SHAPED_SEED", 1);
    let engines: Vec<TransportEngine> = match std::env::var("CC_SHAPED_ENGINE").as_deref() {
        Ok("legacy") => vec![TransportEngine::Legacy],
        Ok("cc-port") => vec![TransportEngine::CcPort],
        _ => vec![TransportEngine::Legacy, TransportEngine::CcPort],
    };
    let fwd = Dir {
        rate_bps: rate,
        limit: q,
        delay,
        jitter,
        loss_permille: loss,
    };
    let rev = Dir {
        // `CC_SHAPED_REV_RATE=<B/s>`: dönüş yolu hızı (asimetrik yol; varsayılan ileri hız).
        rate_bps: env_u64("CC_SHAPED_REV_RATE", rate),
        limit: q.max(64),
        delay,
        jitter,
        loss_permille: 0,
    };
    let side = std::env::var("CC_SHAPED_LINK_SIDE").ok();
    let link = (std::env::var("CC_SHAPED_LINK").is_ok() || side.is_some()).then(|| LinkProfile {
        fwd_bps: env_u64("CC_SHAPED_LINK_FWD", fwd.rate_bps),
        rev_bps: env_u64("CC_SHAPED_LINK_REV", rev.rate_bps),
        server: side.as_deref() != Some("client"),
        client: side.as_deref() != Some("server"),
    });
    for eng in engines {
        let o = run_cell(
            eng,
            fwd,
            rev,
            seed,
            units,
            unit_bytes,
            Duration::from_secs(env_u64("CC_SHAPED_TMAX_S", 180)),
            Duration::from_millis(env_u64("CC_SHAPED_INTERVAL_MS", 0)),
            link,
        )
        .await;
        println!(
            "[{eng:?}] shaped r{rate} rr{} q{q} d{}±{}ms l{loss}‰ u{units}×{unit_bytes} seed{seed} profil={}: tamamlanma={} teslim={}/{units} ileri={} pkt / {} B (kuyruk_düşen={} rastgele_düşen={}) dönüş={} pkt / {} B (düşen={})",
            rev.rate_bps,
            delay.as_millis(),
            jitter.as_millis(),
            link.map_or("kapalı".to_string(), |l| format!(
                "{}{}:{}/{}",
                if l.server { "S" } else { "" },
                if l.client { "C" } else { "" },
                l.fwd_bps,
                l.rev_bps
            )),
            o.completed.map_or("YOK".to_string(), |d| format!("{:.2} s", d.as_secs_f64())),
            o.delivered,
            o.stats.fwd_sent,
            o.stats.fwd_bytes,
            o.stats.fwd_dropped_queue,
            o.stats.fwd_dropped_random,
            o.stats.rev_sent,
            o.stats.rev_bytes,
            o.stats.rev_dropped_queue,
        );
    }
}

/// [ASİMETRİK-YOL] Kalıcı kapı — saha koşulu: ileri 12,5 kB/s, dönüş 62 B/s (yukarı yön ≈ 0,5 kbit/s),
/// tek yön 300 ms, %20 ileri kayıp, 40 × 1000 B. Yön hızları verilmezse port motoru bu hücrede 57–62 s
/// sürüyor ve dönüşe 3,6–4,2 kB ACK basıyor: ACK'ler 62 B/s hatta sıra bekliyor, gönderici ACK-saatli
/// döngüde yakınsamıyor. Yön hızlarıyla (alıcı ACK bütçesi + yol ilk penceresi + profil pacer
/// tavanı; PARAM geri çekilmesi tabanında) ölçüm: tohum 1–2 15,3–19,0 s, dönüş 565–693 B; bu kapı
/// (tohum 1) 3 koşu 14,9–19,0 s, 630–693 B. Alıcının akış sonu yedeği + PARAM PN kaydı +
/// PARAM_ACK seyreltmesi (`CC_SHAPED_TL` ayrıştırması; profil pacer tavanından önceki tabanda
/// ölçüldü): tohum 1–3 × 2 alıcıda son teslim 16,8–20,2 s → 9,9–11,4 s. Eşikler (≤ 35 s,
/// ≤ 1 500 B) gerçek zaman gürültüsüne geniş; yön hızları olmadan hücre iki eşiği de aşar. Kuyruk
/// kuralları alıcı birim testlerinde belirlenimli sınanır.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn asymmetric_return_path_with_link_rates_bounds_acks_and_completes() {
    let fwd = Dir {
        rate_bps: 12_500,
        limit: 64,
        delay: Duration::from_millis(300),
        jitter: Duration::ZERO,
        loss_permille: 200,
    };
    let rev = Dir {
        rate_bps: 62,
        limit: 64,
        delay: Duration::from_millis(300),
        jitter: Duration::ZERO,
        loss_permille: 0,
    };
    let o = run_cell(
        TransportEngine::CcPort,
        fwd,
        rev,
        1,
        40,
        1000,
        Duration::from_secs(120),
        Duration::ZERO,
        Some(LinkProfile::exact(fwd, rev)),
    )
    .await;
    report("B", &o);
    let done = o
        .completed
        .expect("asimetrik hücre 120 s içinde tamamlanmalı");
    assert_eq!(o.delivered, 40, "40/40 teslim");
    assert!(
        done <= Duration::from_secs(35),
        "yön hızlarıyla tamamlanma {done:?} > 35 s (yön hızları olmadan 57–62 s)"
    );
    assert!(
        o.stats.rev_bytes <= 1_500,
        "dönüşe {} B > 1500 B (bütçe ihlali; yön hızları olmadan 3,6–4,2 kB)",
        o.stats.rev_bytes
    );
}

/// Kapı özeti (CI günlüğünde eşiklerin yanında görünür).
fn report(tag: &str, o: &Outcome) {
    println!(
        "KAPI {tag}: tamamlanma={} teslim={} ileri_kuyruk_düşen={} ileri_rastgele_düşen={} dönüş={} pkt / {} B",
        o.completed
            .map_or("YOK".to_string(), |d| format!("{:.2} s", d.as_secs_f64())),
        o.delivered,
        o.stats.fwd_dropped_queue,
        o.stats.fwd_dropped_random,
        o.stats.rev_sent,
        o.stats.rev_bytes
    );
}

/// Tek yön 300 ms, jitter'sız yön.
fn dir300(rate_bps: u64, limit: usize, loss_permille: u64) -> Dir {
    Dir {
        rate_bps,
        limit,
        delay: Duration::from_millis(300),
        jitter: Duration::ZERO,
        loss_permille,
    }
}

/// [ASİMETRİK-YOL] Kalıcı kapı — doğru profil, sığ kuyruk: ileri 12,5 kB/s (kuyruk 16 paket), dönüş
/// 62 B/s, %20 ileri kayıp, 40 × 1000 B, profil iki uçta ve hatla aynı. Profil ilk pencereyi
/// ≈ 67 kB'ye büyütür; pencere profil hızıyla serileşir (pacer tavanı). Tavansız pacer ilk RTT
/// örneğinden önce ≈ 840 kB/s ile çalışıyor ve 40 birimi 50 ms'de 16 paketlik kuyruğa boşaltıyordu:
/// tohum 1/2 37,5/42,3 s, ileri kuyruk düşmesi 28/33 (düşenler uçuşun sonu, yalnız PTO ile
/// seziliyordu). Tavanla tohum 1/2 14,9/19,5 s, düşme 0/1; bu kapı (tohum 1) 3 koşu 19,0–20,3 s,
/// düşme 0–1. Eşik: düşme ≤ 8 (asıl ayırıcı), tamamlanma ≤ 35 s.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn link_rates_initial_window_is_paced_into_a_shallow_queue() {
    let fwd = dir300(12_500, 16, 200);
    let rev = dir300(62, 64, 0);
    let o = run_cell(
        TransportEngine::CcPort,
        fwd,
        rev,
        1,
        40,
        1000,
        Duration::from_secs(120),
        Duration::ZERO,
        Some(LinkProfile::exact(fwd, rev)),
    )
    .await;
    report("C0", &o);
    let done = o
        .completed
        .expect("sığ kuyruk hücresi 120 s içinde tamamlanmalı");
    assert_eq!(o.delivered, 40, "40/40 teslim");
    assert!(
        o.stats.fwd_dropped_queue <= 8,
        "ileri kuyrukta {} paket düştü > 8 (ilk pencere profil hızıyla serileşmiyor; tavansız 28–33)",
        o.stats.fwd_dropped_queue
    );
    assert!(
        done <= Duration::from_secs(35),
        "tamamlanma {done:?} > 35 s (tavansız 37–42 s)"
    );
}

/// [ASİMETRİK-YOL] Kalıcı kapı — fazla iyimser profil, sığ kuyruk: ileri gerçek 2,5 kB/s (kuyruk 8
/// paket), profil 12,5 kB/s (5 kat); dönüş 62 B/s, %20 ileri kayıp, 24 × 1000 B. Profil ilk
/// pencereyi gerçeğin 5 katına büyütür; ilk ACK'ten önce bunu bilecek ölçüm yoktur, ilk uçuşun
/// fazlası kuyrukta düşer (16–17 paket). Profil sınanmadan Startup hızı "ilk pencere / min_rtt"
/// tabanında kilitli kalıyor, profil tavanı da ölçümden bağımsız taban gibi davranıyordu: kayıp
/// raporunun açtığı yeniden iletimler 12,2 kB/s ve 10·MTU'luk patlamayla kuyruğa çıkıp yeniden
/// düşüyordu. Tohum 1 × 2 koşu: profil sınamasız 40,4 s, kuyruk düşmesi 23; yön hızları kapalı kol
/// 33,9 s; profil ilk yeterli ölçümde geri çekilince (`Sender::set_cc_profile_base_window`) 31,3 s,
/// düşme 16. Tohum 2–8 (1–2 koşu) geri çekmeyle 17,5–46,2 s, sınamasız 24,3–51,0 s, kapalı kol
/// 27,3–50,5 s. Eşikler: düşme ≤ 19 (asıl ayırıcı; sınamasız 21–23), tamamlanma ≤ 38 s
/// (sınamasız 40,4 s; kapalı kolun ≈ 1,1 katı).
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn overoptimistic_link_profile_on_shallow_queue_still_completes() {
    let fwd = dir300(2_500, 8, 200);
    let rev = dir300(62, 64, 0);
    let o = run_cell(
        TransportEngine::CcPort,
        fwd,
        rev,
        1,
        24,
        1000,
        Duration::from_secs(150),
        Duration::ZERO,
        Some(LinkProfile {
            fwd_bps: 12_500,
            ..LinkProfile::exact(fwd, rev)
        }),
    )
    .await;
    report("C", &o);
    let done = o
        .completed
        .expect("fazla iyimser profil hücresi 150 s içinde tamamlanmalı");
    assert_eq!(o.delivered, 24, "24/24 teslim");
    assert!(
        o.stats.fwd_dropped_queue <= 19,
        "ileri kuyrukta {} paket düştü > 19 (profil ilk ölçümde geri çekilmedi; ilk uçuş 16–17)",
        o.stats.fwd_dropped_queue
    );
    assert!(
        done <= Duration::from_secs(38),
        "fazla iyimser profil: tamamlanma {done:?} > 38 s (yön hızları kapalı kol 33,9 s)"
    );
}

/// [ASİMETRİK-YOL] Kalıcı kapı — profil yalnız alıcıda: ileri 12,5 kB/s, dönüş 62 B/s, tek yön
/// 300 ms, %20 ileri kayıp, 20 × 1000 B. Alıcı ACK'leri bütçeyle seyreltir, göndericinin ilk
/// penceresi büyümez (4,8 kB): ACK'ler saniyeler arayla gelir, gönderici küçük pencereyle ACK
/// bekler. Kalıcı bekleme olmamalı. Ölçüm (tohum 1, 3 koşu): 21,5–25,5 s, dönüş 574–706 B; yön
/// hızları kapalı aynı hücre 24,8–25,4 s, 1,6–1,7 kB (2 koşu). Eşik 60 s (ilerleyen taşımaya iki
/// kattan fazla pay; kalıcı beklemede 150 s'de zaman aşımı).
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn link_rates_on_receiver_only_completes() {
    let fwd = dir300(12_500, 64, 200);
    let rev = dir300(62, 64, 0);
    let o = run_cell(
        TransportEngine::CcPort,
        fwd,
        rev,
        1,
        20,
        1000,
        Duration::from_secs(150),
        Duration::ZERO,
        Some(LinkProfile {
            client: false,
            ..LinkProfile::exact(fwd, rev)
        }),
    )
    .await;
    report("D-alıcı", &o);
    let done = o
        .completed
        .expect("yalnız alıcıda profil: 150 s içinde tamamlanmalı");
    assert_eq!(o.delivered, 20, "20/20 teslim");
    assert!(
        done <= Duration::from_secs(60),
        "yalnız alıcıda profil: tamamlanma {done:?} > 60 s"
    );
}

/// [ASİMETRİK-YOL] Kalıcı kapı — profil yalnız göndericide: aynı hücre. İlk pencere büyür
/// (≈ 67 kB, profil hızıyla serileşir), alıcı bütçesiz quinn kuralıyla ACK'ler: ACK'ler 62 B/s
/// dönüş kuyruğunda birikir, RTT örnekleri şişer. Kalıcı bekleme olmamalı. Ölçüm (tohum 1, 3 koşu):
/// 12,5–26,8 s, dönüş 1 717–1 838 B (bütçesiz). Şişen örneklerle ilk yeterli teslim hızı profilin
/// beşte birinin altında kalır, profil geri çekilir (yalnız Startup hız tabanı; pencere korunur):
/// tohum 1 × 2 koşu 11,3 s, dönüş 1 721 B; geri çekmesiz 25,3 s (1 koşu). Eşik 60 s.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn link_rates_on_sender_only_completes() {
    let fwd = dir300(12_500, 64, 200);
    let rev = dir300(62, 64, 0);
    let o = run_cell(
        TransportEngine::CcPort,
        fwd,
        rev,
        1,
        20,
        1000,
        Duration::from_secs(150),
        Duration::ZERO,
        Some(LinkProfile {
            server: false,
            ..LinkProfile::exact(fwd, rev)
        }),
    )
    .await;
    report("D-gönderici", &o);
    let done = o
        .completed
        .expect("yalnız göndericide profil: 150 s içinde tamamlanmalı");
    assert_eq!(o.delivered, 20, "20/20 teslim");
    assert!(
        done <= Duration::from_secs(60),
        "yalnız göndericide profil: tamamlanma {done:?} > 60 s"
    );
}
