//! TC-19 — `SenderState` geçiş matrisi (SR-STR-CORE-001 §5.4, BUL-06).
//!
//! **Kapsanan bulgu:** BUL-06. STR §3.3/§3.4: `sender.rs` fonksiyon kapsamı %82,95 ile
//! workspace'in en düşüğü, branch TOTAL %81,21; `on_event` 994 satır ve 116 dallanma noktası
//! içerir. Mevcut testler ANA yolları kapsar (cold-start çıkışı, Startup→Drain→Steady,
//! DrainRtprop dwell, RecoverFwd/RecoverArq girişleri) ancak matris bütünsel değildir.
//!
//! **Bu test aynı zamanda BUL-06'nın refactor'ı için güvenlik ağıdır:** matris yeşilken
//! `on_event`'i bölmek güvenlidir, matrissiz bölmek değildir.
//!
//! **Yöntem:** Dokuz durumun 81 geçişi tablolaştırılır ve her hücreye "izinli / yasak" etiketi
//! verilir (§3 + `update_state()` kaynak-okuması). Sonra motor deterministik bir event-yürüyüşü
//! ile sürülür; gözlemlenen her `(prev, next)` çifti tabloya karşı sınanır.
//!
//! **Determinizm:** RNG kullanılmaz. Event seçimi sabit-tohumlu bir xorshift64* ile yapılır —
//! aynı tohum her koşumda aynı diziyi üretir (repo disiplini: `desen1_sim.rs` "deterministik").
//!
//! **Vakumsuzluk kapısı:** "Yasak geçiş gözlenmedi" iddiası, yürüyüş sığsa anlamsızdır. Bu
//! yüzden test ayrıca (a) dokuz durumun HEPSİNİN ziyaret edildiğini ve (b) belirlenmiş bir
//! TANIK geçiş kümesinin fiilen gözlendiğini assert eder. Yürüyüş sığlaşırsa test kırmızı olur.
//!
//! **Oracle:** Yasak geçişler hiçbir event dizisiyle üretilemiyor; matris bu dosyada tek
//! kaynak olarak duruyor.

use adaptive_transport::sender::{AckInfo, Event, OpaqueUnit, Sender, SenderState, Timer};
use std::collections::BTreeSet;
use std::time::Duration;

const MS: u64 = 1_000_000;
const UNIT_BYTES: u64 = 101; // TC-17: FEC_WIRE_OVERHEAD(37) + E(64), profile 0x03

// ── Durum indeksleme ────────────────────────────────────────────────────────

const N: usize = 9;
const STATES: [SenderState; N] = [
    SenderState::Idle,
    SenderState::PreConverge,
    SenderState::Startup,
    SenderState::Drain,
    SenderState::ProbeUp,
    SenderState::Steady,
    SenderState::RecoverArq,
    SenderState::RecoverFwd,
    SenderState::DrainRtprop,
];

fn idx(s: SenderState) -> usize {
    STATES
        .iter()
        .position(|&x| x == s)
        .expect("bilinmeyen durum")
}

fn name(i: usize) -> &'static str {
    match STATES[i] {
        SenderState::Idle => "Idle",
        SenderState::PreConverge => "PreConverge",
        SenderState::Startup => "Startup",
        SenderState::Drain => "Drain",
        SenderState::ProbeUp => "ProbeUp",
        SenderState::Steady => "Steady",
        SenderState::RecoverArq => "RecoverArq",
        SenderState::RecoverFwd => "RecoverFwd",
        SenderState::DrainRtprop => "DrainRtprop",
    }
}

// ── 9×9 GEÇİŞ MATRİSİ — TEK KAYNAK ──────────────────────────────────────────
//
// Satır = prev, sütun = next. '.' = YASAK, 'X' = İZİNLİ.
// Türetim (`sender.rs::update_state()` + `on_event`/`on_timer` ön-mutasyonları):
//
//  R0  idle (kuyruk∧inflight∧pending_lost∧dup boş)      → Idle          [her durumdan]
//  R1  prev == PreConverge → srtt? Startup : PreConverge [K-KEYSTONE §3.3-D]
//  R2  emergency_active                                  → RecoverFwd    [§3 AMENDMENT-1]
//  R3  pending_lost boş değil                            → RecoverArq    [§3 AMENDMENT-1]
//  R4  match prev {
//        Idle        → Startup                     (ayrıca AppData ön-mutasyonu: → PreConverge)
//        Startup     → plateau ? Drain : Startup
//        Drain       → (inflight≤BDP ∨ dwell) ? Steady : Drain
//        DrainRtprop → (dwell∧qualifying ∨ max) ? Steady : DrainRtprop
//        ProbeUp     → (success∨abort∨timeout) ? Steady : ProbeUp
//        Steady      → (period∧qd∧backlog) ? ProbeUp : Steady
//                      (ayrıca on_timer(RtpropProbe) ön-mutasyonu: Steady → DrainRtprop)
//        Recover*    → base_is_startup ? Startup : Steady
//      }
//
// **[ÖLÇÜLDÜ — bu dosya] YAPISAL BULGU (TC-19/a): matris `update_state()` üzerinden DEĞİL,
// `on_event()` SINIRI üzerinden tanımlanmalıdır.** İlk türetim `update_state()`'in iç `match`'ini
// esas aldı ve iki hücrede yanlış çıktı: ölçüm `Idle → Drain` ve `Idle → RecoverFwd` geçişlerini
// FİİLEN üretti. Sebep bir invariant ihlali değil, ÖN-MUTASYONdur: `on_event(AppData)` state==Idle
// iken `update_state()` çağrılmadan ÖNCE state'i PreConverge/Startup yapar; `update_state()` o
// yeni state'ten çalışır. Yani Idle satırının gözlemlenebilir hedef kümesi
// `{Idle} ∪ PreConverge-satırı ∪ Startup-satırı`dır. Bu, motorun tek `on_event` çağrısı içinde
// iki geçiş yapabildiği anlamına gelir ve matris bunu YANSITMALIDIR.
//
// Bu bulgunun iki somut sonucu var — ikisi de test değil TASARIM sorusudur [BEKLİYOR — Abkarada]:
//   (i)  `Idle → Drain`: `plateau_reached` bayrağı Startup re-entry hook'unda sıfırlanmaz
//        (`startup_stable_rounds`/`startup_last_btlbw`/`startup_round_*`/`base_is_startup`
//        sıfırlanır, `plateau_reached` sıfırlanmaz). Önceki epizotta kazanılmış bir plato,
//        idle'dan sonraki İLK AppData'da yeni epizodu doğrudan Drain'e sokabilir — yani yeni
//        kanal hiç prob edilmeden. Survival-band'de sessizlik sonrası resume, kanalın yeniden
//        ölçülmesi GEREKEN andır. [ÇIKARIM — mekanizma; geçişin kendisi ÖLÇÜLDÜ]
//   (ii) `Idle → RecoverFwd`: uzun sessizlik ack_confidence'ı düşürür → emergency aktif →
//        resume'un İLK paketi forward-redundancy (duplicate) rejimine girer. PreConverge donması
//        bunu engellemez, çünkü önceki epizottan `srtt` Some'dur. Sub-kbps'te resume anında
//        kopya üretmek, yolun kötü olduğuna dair HİÇBİR kanıt yokken bant harcamaktır.
//
// NORMATİF YASAK — belgelenmesi gereken üç sınıf (ölçümden SONRA da geçerli):
//   (a) Idle ve PreConverge dışında HİÇBİR durumdan PreConverge'e dönülemez. Cold-start önseli
//       bir kez tahliye edilir; geri dönüş "ölçüm var ama yokmuş gibi davran" demektir. [K-KEYSTONE]
//   (b) PreConverge'den recovery'ye (RecoverArq/RecoverFwd) geçilemez — §3.3-C: loss-ilanı/PTO/
//       emergency cold-start'ta DONDURULMUŞTUR. Bu, R1'in R2/R3'ten ÖNCE gelmesiyle sağlanır.
//   (c) Startup'tan doğrudan Steady'ye geçilemez; kuyruk Drain ile boşaltılır (BBR-analog).
//       (Startup → Recover* → Steady iki adımlı yolu serbesttir.)
//
//                  Idl  Pre  Sta  Dra  Prb  Ste  RAr  RFw  DRt
const ALLOWED: [[bool; N]; N] = [
    // Idle satırı = {Idle} ∪ PreConverge ∪ Startup satırları (yukarıdaki ön-mutasyon bulgusu).
    /* Idle       */
    [t(), t(), t(), t(), f(), f(), t(), t(), f()],
    /* PreConverge*/ [t(), t(), t(), f(), f(), f(), f(), f(), f()],
    /* Startup    */ [t(), f(), t(), t(), f(), f(), t(), t(), f()],
    /* Drain      */ [t(), f(), f(), t(), f(), t(), t(), t(), f()],
    /* ProbeUp    */ [t(), f(), f(), f(), t(), t(), t(), t(), f()],
    /* Steady     */ [t(), f(), f(), f(), t(), t(), t(), t(), t()],
    /* RecoverArq */ [t(), f(), t(), f(), f(), t(), t(), t(), f()],
    /* RecoverFwd */ [t(), f(), t(), f(), f(), t(), t(), t(), f()],
    /* DrainRtprop*/ [t(), f(), f(), f(), f(), t(), t(), t(), t()],
];

const fn t() -> bool {
    true
}
const fn f() -> bool {
    false
}

// ── Deterministik event yürüyüşü ────────────────────────────────────────────

/// xorshift64* — sabit tohum, RNG-crate'i yok, her koşumda aynı dizi.
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

struct Walk {
    s: Sender,
    rng: Det,
    now: u64,
    next_unit: u64,
    /// Gönderilmiş ama henüz ACK'lenmemiş seq'ler (ACK üretmek için).
    outstanding: Vec<u64>,
    observed: [[bool; N]; N],
    visited: [bool; N],
}

impl Walk {
    fn new(seed: u64) -> Self {
        Self {
            s: Sender::new(),
            rng: Det(seed),
            now: 0,
            next_unit: 0,
            outstanding: Vec::new(),
            observed: [[false; N]; N],
            visited: [false; N],
        }
    }

    fn step(&mut self) {
        // Zaman her adımda ilerler; adım büyüklüğü dwell/period kapılarını hem içeriden hem
        // dışarıdan geçebilecek şekilde dağıtılır (1ms … 4s).
        self.now += match self.rng.pick(5) {
            0 => MS,
            1 => 10 * MS,
            2 => 60 * MS,
            3 => 400 * MS,
            _ => 4_000 * MS,
        };

        let prev = self.s.state();
        let ev = match self.rng.pick(10) {
            // AppData ağırlıklı: kuyruk boşalırsa motor Idle'a düşer ve yürüyüş sığlaşır.
            0..=4 => {
                self.next_unit += 1;
                Event::AppData(OpaqueUnit {
                    id: self.next_unit,
                    size: UNIT_BYTES,
                })
            }
            5..=7 => {
                // ACK: outstanding'in bir önekini onayla. Boşsa boş-ACK (geçerli).
                let take = if self.outstanding.is_empty() {
                    0
                } else {
                    1 + self.rng.pick(self.outstanding.len() as u64) as usize
                };
                let mut acked: Vec<u64> = self.outstanding.drain(..take).collect();

                // GAP-ACK: acked'ın başından birkaç seq'i DÜŞÜR (kaybolmuş gibi) ama sonrakileri
                // onayla. Bu, packet-threshold loss'u tetiklerken ACK akışını CANLI tutar →
                // ack_confidence sağlıklı kalır → RecoverArq (emergency değil) yolu açılır.
                // Yalnız-önek ACK'i bu ayrımı yapamaz: kayıp ancak ACK sessizliğiyle görünür,
                // o da ack_confidence'ı düşürüp RecoverFwd'a çıkarır ve RecoverArq'u gölgeler.
                if acked.len() >= 3 && self.rng.pick(3) == 0 {
                    let drop_n = 1 + self.rng.pick(2) as usize;
                    acked.drain(..drop_n.min(acked.len() - 1));
                }
                // Bazen sıradışı ACK — reorder yolunu da sür.
                if self.rng.pick(4) == 0 && !acked.is_empty() {
                    acked.reverse();
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
        // outstanding sınırsız büyürse ACK adımı yavaşlar; kuyruğu makul tut.
        if self.outstanding.len() > 512 {
            self.outstanding.drain(..256);
        }

        let next = self.s.state();
        self.observed[idx(prev)][idx(next)] = true;
        self.visited[idx(next)] = true;
    }
}

// ── Testler ─────────────────────────────────────────────────────────────────

fn explore(seeds: &[u64], steps: usize) -> ([[bool; N]; N], [bool; N]) {
    let mut obs = [[false; N]; N];
    let mut vis = [false; N];
    vis[idx(SenderState::Idle)] = true; // başlangıç durumu
    for &seed in seeds {
        let mut w = Walk::new(seed);
        for _ in 0..steps {
            w.step();
        }
        for (i, row) in w.observed.iter().enumerate() {
            vis[i] |= w.visited[i];
            for (j, &seen) in row.iter().enumerate() {
                obs[i][j] |= seen;
            }
        }
    }
    (obs, vis)
}

const SEEDS: [u64; 8] = [
    1,
    7,
    42,
    1337,
    0xDEAD_BEEF,
    0x5A5A_5A5A,
    99_991,
    2_147_483_647,
];
const STEPS: usize = 20_000;

#[test]
fn no_forbidden_transition_is_reachable() {
    let (obs, _) = explore(&SEEDS, STEPS);
    let mut violations: Vec<String> = Vec::new();
    for i in 0..N {
        for j in 0..N {
            if obs[i][j] && !ALLOWED[i][j] {
                violations.push(format!("{} → {}", name(i), name(j)));
            }
        }
    }
    assert!(
        violations.is_empty(),
        "YASAK geçiş(ler) üretildi: {violations:?}"
    );
}

#[test]
fn preconverge_is_never_re_entered() {
    // [K-KEYSTONE §3.3] Bu, matrisin en kritik tek hücresidir ve ayrı bir test hak eder:
    // cold-start önseli bir kez tahliye edilir. Idle→PreConverge (ilk AppData) ve
    // PreConverge→PreConverge (srtt henüz yok) dışında hiçbir giriş meşru değildir.
    let (obs, _) = explore(&SEEDS, STEPS);
    let pre = idx(SenderState::PreConverge);
    for i in 0..N {
        if STATES[i] == SenderState::Idle || STATES[i] == SenderState::PreConverge {
            continue;
        }
        assert!(
            !obs[i][pre],
            "PreConverge'e geri dönüldü: {} → PreConverge",
            name(i)
        );
    }
}

#[test]
fn preconverge_never_enters_recovery() {
    // §3.3-C: cold-start'ta loss-ilanı/PTO/emergency DONDURULMUŞTUR. update_state()'te R1
    // dalının R2/R3'ten ÖNCE gelmesi bunu sağlar; sıra değişirse burası kırmızı yanar.
    let (obs, _) = explore(&SEEDS, STEPS);
    let pre = idx(SenderState::PreConverge);
    for target in [SenderState::RecoverArq, SenderState::RecoverFwd] {
        assert!(
            !obs[pre][idx(target)],
            "PreConverge → {target:?} (cold-start recovery dondurulmuş olmalı)"
        );
    }
}

#[test]
fn startup_never_reaches_steady_directly() {
    // BBR-analog sözleşme: Startup çıkışı kuyruğu Drain ile boşaltır. Startup→Steady doğrudan
    // geçişi, drain edilmemiş bir kuyruğu Steady'nin gain=1 rejimine devretmek demektir.
    let (obs, _) = explore(&SEEDS, STEPS);
    assert!(
        !obs[idx(SenderState::Startup)][idx(SenderState::Steady)],
        "Startup → Steady doğrudan geçiş üretildi (Drain atlandı)"
    );
}

#[test]
fn idle_resume_can_skip_startup_probing() {
    // [ÖLÇÜLDÜ] TC-19/a bulgusunun regresyon-tanığı. Bu test bir İDDİA değil, bir KAYITTIR:
    // bugünkü davranışı sabitler ki tasarım kararı verildiğinde (bkz. matris notu (i)/(ii))
    // değişimin bilinçli olduğu görülsün. Davranış düzeltilirse bu test kırmızı yanar ve
    // matris ile birlikte güncellenir.
    let (obs, _) = explore(&SEEDS, STEPS);
    let idle = idx(SenderState::Idle);
    assert!(
        obs[idle][idx(SenderState::Drain)],
        "Idle→Drain artık üretilmiyor: plateau_reached sıfırlaması eklenmiş olabilir — \
         matris notu (i) ve bu test güncellenmeli"
    );
    assert!(
        obs[idle][idx(SenderState::RecoverFwd)],
        "Idle→RecoverFwd artık üretilmiyor: resume-emergency davranışı değişmiş olabilir — \
         matris notu (ii) ve bu test güncellenmeli"
    );
}

#[test]
fn walk_is_not_vacuous() {
    // Vakumsuzluk kapısı: yasak-geçiş iddiası ancak yürüyüş derinse anlamlıdır.
    let (obs, vis) = explore(&SEEDS, STEPS);

    let unvisited: Vec<&str> = (0..N).filter(|&i| !vis[i]).map(name).collect();
    assert!(
        unvisited.is_empty(),
        "şu durumlara hiç ulaşılamadı (yürüyüş sığ): {unvisited:?}"
    );

    // TANIK geçişler — motorun ana omurgası fiilen sürülmüş olmalı.
    let witnesses: [(SenderState, SenderState); 8] = [
        (SenderState::Idle, SenderState::PreConverge),
        (SenderState::PreConverge, SenderState::Startup),
        (SenderState::Startup, SenderState::Drain),
        (SenderState::Drain, SenderState::Steady),
        (SenderState::Steady, SenderState::DrainRtprop),
        (SenderState::Steady, SenderState::ProbeUp),
        (SenderState::Startup, SenderState::RecoverArq),
        (SenderState::Steady, SenderState::RecoverFwd),
    ];
    let missing: Vec<String> = witnesses
        .iter()
        .filter(|(a, b)| !obs[idx(*a)][idx(*b)])
        .map(|(a, b)| format!("{a:?} → {b:?}"))
        .collect();
    assert!(
        missing.is_empty(),
        "tanık geçişler gözlenmedi (yürüyüş bu yolları sürmüyor): {missing:?}"
    );
}

#[test]
fn transition_matrix_report() {
    // Raporlayıcı: hangi izinli geçişlerin fiilen gözlendiği. Assertion YOK (kapı yukarıdaki
    // testlerde); bu çıktı kapsam boşluğunun nerede olduğunu gösterir.
    // Koş: cargo test -p adaptive-transport --test sender_state_matrix -- --nocapture
    let (obs, _) = explore(&SEEDS, STEPS);
    let mut unobserved: BTreeSet<String> = BTreeSet::new();
    println!(
        "\n  prev \\ next   {}",
        (0..N)
            .map(|j| format!("{:>4}", &name(j)[..3]))
            .collect::<Vec<_>>()
            .join("")
    );
    for i in 0..N {
        let mut row = format!("  {:<13}", name(i));
        for (j, &seen) in obs[i].iter().enumerate() {
            row.push_str(match (ALLOWED[i][j], seen) {
                (true, true) => "   X",   // izinli + gözlendi
                (true, false) => "   -",  // izinli ama gözlenmedi
                (false, false) => "   .", // yasak, doğru
                (false, true) => "   !",  // YASAK ama gözlendi → yukarıdaki test düşer
            });
            if ALLOWED[i][j] && !seen {
                unobserved.insert(format!("{} → {}", name(i), name(j)));
            }
        }
        println!("{row}");
    }
    println!("\n  X=izinli/gözlendi  -=izinli/gözlenmedi  .=yasak  !=İHLAL");
    println!(
        "  gözlenmeyen izinli geçişler ({}): {:?}\n",
        unobserved.len(),
        unobserved
    );
}
