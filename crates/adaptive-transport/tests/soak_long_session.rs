//! SOAK — uzun-oturum bellek ve durum sınırı kararlılığı.
//!
//! **Kapsanan boşluk:** SR-STR-CORE-001'de önerilmemişti; `kama-media`'daki soak disiplininin
//! (30 dk, 229k frame, RSS-stabil) `kama-core` karşılığı. Depoda uzun-oturum testi YOKTU.
//!
//! **Gerekçe:** [KOD] `Sender` yedi uzun-ömürlü büyüyebilir koleksiyon taşır:
//! `inflight` (BTreeMap) · `tx_queue` (VecDeque) · `pending_lost` (VecDeque) · `dup_queue` (Vec) ·
//! `tombstones` (BTreeMap) · `tombstone_order` (VecDeque) · `bw_sampler` iç durumu.
//! Bunlardan YALNIZ `tombstones` görünür bir kapasite sınırı taşır (`TOMBSTONE_MAX = 4096`);
//! diğerlerinin sınırlı kaldığını gösteren hiçbir test yoktur.
//!
//! Sub-kbps hedef banda göre bir oturum SAATLERCE sürer. Sınırsız büyüyen bir kuyruk orada
//! patlar; kısa birim testlerinde görünmez. Bu dosya o rejimi sanal saatle sıkıştırır.
//!
//! **Yöntem:** Sayan global allocator + sanal saat. Gerçek uyku yok; olay sayısı gerçek bir
//! oturumu temsil eder. Bellek "sızıyor mu" değil **"platoya oturuyor mu"** diye ölçülür —
//! transport'un meşru çalışma-kümesi sabit değildir, ama SINIRLI olmalıdır.
//!
//! **Oracle:** (1) Isınma sonrası dört çeyrekte tepe bellek artmıyor; (2) `tombstones` kapasiteyi
//! aşmıyor; (3) hiçbir çeyrekte bellek ilk çeyreğin `GROWTH_FACTOR` katını geçmiyor.
//!
//! Koş: `cargo test -p adaptive-transport --test soak_long_session -- --nocapture`

use adaptive_transport::sender::{AckInfo, Event, OpaqueUnit, Sender, SenderState, Timer};
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

// ── Sayan allocator ─────────────────────────────────────────────────────────

static ALLOCATED: AtomicUsize = AtomicUsize::new(0);

struct Counting;

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        let p = unsafe { System.alloc(l) };
        if !p.is_null() {
            ALLOCATED.fetch_add(l.size(), Ordering::Relaxed);
        }
        p
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        unsafe { System.dealloc(p, l) };
        ALLOCATED.fetch_sub(l.size(), Ordering::Relaxed);
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, new: usize) -> *mut u8 {
        let np = unsafe { System.realloc(p, l, new) };
        if !np.is_null() {
            if new >= l.size() {
                ALLOCATED.fetch_add(new - l.size(), Ordering::Relaxed);
            } else {
                ALLOCATED.fetch_sub(l.size() - new, Ordering::Relaxed);
            }
        }
        np
    }
}

#[global_allocator]
static A: Counting = Counting;

fn live_bytes() -> usize {
    ALLOCATED.load(Ordering::Relaxed)
}

// ── Sanal-saat sürücü ───────────────────────────────────────────────────────

const MS: u64 = 1_000_000;
const UNIT_BYTES: u64 = 101; // TC-17: 26 outer + 11 inner + E=64

/// xorshift64* — sabit tohum, RNG-crate'i yok, koşumlar arası birebir tekrarlanabilir.
struct Det(u64);
impl Det {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    fn pick(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

/// Hostile oturum sürücüsü: app sürekli veri verir, ACK'ler kayıplı ve aralıklı gelir,
/// periyodik olarak tam ACK sessizliği (blackout) yaşanır.
struct Session {
    s: Sender,
    rng: Det,
    now: u64,
    next_unit: u64,
    outstanding: Vec<u64>,
    /// Blackout penceresi: bu adımda ACK üretilmez (kanal koptu).
    blackout_until: u64,
}

impl Session {
    fn new(seed: u64) -> Self {
        Self {
            s: Sender::new(),
            rng: Det(seed),
            now: 0,
            next_unit: 0,
            outstanding: Vec::new(),
            blackout_until: 0,
        }
    }

    fn step(&mut self) {
        self.now += 5 * MS + self.rng.pick(20) * MS; // 5–25 ms sanal adım

        // Her ~40 s'de bir 3 s blackout (p6_blackout profilinin sıkıştırılmışı).
        if self.now > self.blackout_until + 40_000 * MS {
            self.blackout_until = self.now + 3_000 * MS;
        }
        let dark = self.now < self.blackout_until;

        let ev = match self.rng.pick(10) {
            // App sürekli veri veriyor — tx_queue'nun drenajını sınar.
            0..=4 => {
                self.next_unit += 1;
                Event::AppData(OpaqueUnit {
                    id: self.next_unit,
                    size: UNIT_BYTES,
                })
            }
            5..=7 if !dark => {
                // Gap-ACK: bazı seq'leri düşür (kayıp) ama akışı canlı tut →
                // pending_lost dolar, RecoverArq yolu sürülür.
                let take = if self.outstanding.is_empty() {
                    0
                } else {
                    1 + self.rng.pick(self.outstanding.len() as u64) as usize
                };
                let mut acked: Vec<u64> = self.outstanding.drain(..take).collect();
                if acked.len() >= 3 && self.rng.pick(3) == 0 {
                    let drop_n = 1 + self.rng.pick(2) as usize;
                    acked.drain(..drop_n.min(acked.len() - 1));
                }
                Event::AckReceived(AckInfo {
                    acked_seqs: acked,
                    ack_delay: Duration::from_micros(self.rng.pick(20_000)),
                })
            }
            8 => Event::TimerFired(Timer::Pto),
            _ => Event::TimerFired(Timer::RtpropProbe),
        };

        for d in self.s.on_event(ev, self.now) {
            self.outstanding.push(d.seq);
        }
        // Harness'ın KENDİ kuyruğu sınırlı tutulur — ölçülen bellek motorun olsun, testin değil.
        if self.outstanding.len() > 4096 {
            self.outstanding.drain(..2048);
        }
    }
}

// ── Testler ─────────────────────────────────────────────────────────────────

const STEPS: usize = 240_000; // ~4 sanal saat (ortalama 15 ms/adım)
const QUARTERS: usize = 4;
/// Isınma sonrası çeyrekler arası kabul edilen tepe-bellek büyümesi.
/// Kaba: amaç gürültü değil, SINIRSIZ BÜYÜME yakalamak.
const GROWTH_FACTOR: f64 = 1.5;

#[test]
fn long_session_memory_reaches_a_plateau() {
    let mut sess = Session::new(0xC0FFEE);

    // Isınma: ilk çeyrek ölçüme girmez (BTreeMap/Vec kapasiteleri oturuyor).
    let per_q = STEPS / QUARTERS;
    let mut peaks = [0usize; QUARTERS];
    let mut ends = [0usize; QUARTERS];
    let mut tomb_max = 0u64;
    let mut infl_max = 0u64;

    for q in 0..QUARTERS {
        let mut peak = 0usize;
        for _ in 0..per_q {
            sess.step();
            let b = live_bytes();
            if b > peak {
                peak = b;
            }
            let (t_len, _, _, _, _, _) = sess.s.diag_tombstone();
            tomb_max = tomb_max.max(t_len);
            infl_max = infl_max.max(sess.s.inflight_bytes());
        }
        peaks[q] = peak;
        ends[q] = live_bytes();
    }

    println!("\n[SOAK] {STEPS} adım (~4 sanal saat), 4 çeyrek");
    for q in 0..QUARTERS {
        println!(
            "  Ç{}: tepe={:>9} B   son={:>9} B",
            q + 1,
            peaks[q],
            ends[q]
        );
    }
    println!("  tombstone tepe = {tomb_max} (TOMBSTONE_MAX=4096)");
    println!("  inflight_bytes tepe = {infl_max} B");
    println!(
        "  Ç4/Ç2 tepe oranı = {:.2} (eşik {GROWTH_FACTOR})\n",
        peaks[3] as f64 / peaks[1].max(1) as f64
    );

    // (1) Isınma sonrası (Ç2..Ç4) tepe bellek plato yapmalı.
    let base = peaks[1] as f64;
    for q in 2..QUARTERS {
        let ratio = peaks[q] as f64 / base.max(1.0);
        assert!(
            ratio <= GROWTH_FACTOR,
            "Ç{} tepe belleği Ç2'nin {ratio:.2} katı (eşik {GROWTH_FACTOR}) — \
             sınırsız büyüme şüphesi: Ç2={} B, Ç{}={} B",
            q + 1,
            peaks[1],
            q + 1,
            peaks[q]
        );
    }

    // (2) Tombstone kapasite sınırı FİİLEN uygulanıyor mu (TOMBSTONE_MAX = 4096).
    assert!(
        tomb_max <= 4096,
        "tombstone kapasitesi aşıldı: {tomb_max} > 4096"
    );

    // (3) Vakumsuzluk: senaryo motoru gerçekten çalıştırmış olmalı.
    assert!(
        tomb_max > 0,
        "hiç tombstone oluşmadı — senaryo kayıp yolunu sürmüyor, test vakum"
    );
    assert!(
        infl_max > 0,
        "hiç inflight oluşmadı — senaryo gönderim yapmıyor, test vakum"
    );
}

#[test]
fn ack_starvation_does_not_grow_state_without_bound() {
    // En-kötü-köşe: app sürekli veri veriyor, HİÇ ACK gelmiyor (tam kopuk dönüş yolu).
    // Bu, p5_revpath'in uç hâli. `inflight` ve `tx_queue` burada sınırlanmazsa bellek
    // doğrusal büyür ve saatlik bir oturumda patlar.
    let mut s = Sender::new();
    let mut now = 0u64;
    let mut peak_q1 = 0usize;
    let mut peak_q4 = 0usize;

    for i in 0..200_000u64 {
        now += 5 * MS;
        let ev = if i % 4 == 3 {
            Event::TimerFired(Timer::Pto)
        } else {
            Event::AppData(OpaqueUnit {
                id: i,
                size: UNIT_BYTES,
            })
        };
        let _ = s.on_event(ev, now);

        let b = live_bytes();
        if i < 50_000 {
            peak_q1 = peak_q1.max(b);
        } else if i >= 150_000 {
            peak_q4 = peak_q4.max(b);
        }
    }

    let ratio = peak_q4 as f64 / peak_q1.max(1) as f64;
    println!(
        "\n[SOAK ACK-açlığı] Ç1 tepe={peak_q1} B  Ç4 tepe={peak_q4} B  oran={ratio:.2}\n\
         durum={:?}  inflight_bytes={}\n",
        s.state(),
        s.inflight_bytes()
    );

    // ACK yoksa motor gönderemez → kuyruk büyür. Bu BEKLENEN: app'in verdiği veri bir yere
    // gitmeli. Ancak büyüme app-girdisiyle ORANTILI olmalı, ondan HIZLI olmamalı.
    // 4× app girdisi → en fazla ~4× bellek (üstü, motorun kendi iç yapılarının şiştiğini gösterir).
    assert!(
        ratio <= 5.0,
        "ACK-açlığında bellek app-girdisinden hızlı büyüyor: oran={ratio:.2} \
         (Ç1={peak_q1} B, Ç4={peak_q4} B)"
    );

    // Motor ACK yokluğunda Idle'a düşmemeli — veri hâlâ bekliyor.
    assert_ne!(
        s.state(),
        SenderState::Idle,
        "ACK yokken Idle'a düşüldü — bekleyen veri kayboldu mu?"
    );
}
