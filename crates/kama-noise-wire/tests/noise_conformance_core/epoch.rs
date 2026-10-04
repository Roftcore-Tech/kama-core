//! EPOCH MATRİSİ (A3.2) — epoch 0→1 confirmation-safe geçiş; noise-wire §4 (beş adım) + KS-GRACE-1.
//!
//! **Sınanan şey MEVCUT `epoch_machine.rs`'tir**, gerçek kripto oturumu değil. `DhEpochMachine`
//! bugün ürün kodunda hiçbir yere bağlı değildir (yalnız testler kullanır); bu yüzden aşağıdaki
//! [`Endpoint`] makineyi `KamaNoiseWireProtector`'a modül belgesinin tarif ettiği biçimde bağlayan
//! TEST TUTKALIDIR. Tutkalın yaptığı seçimler (ör. grace sayacını yalnız AEAD-geçerli epoch-1
//! paketinde ilerletmek) makinenin değil tutkalın davranışıdır; A5.4 bu tutkalı ürün koduna taşır.
//!
//! # Kırmızı hücreler
//!
//! Mevcut makinenin sağlamadığı hücreler DÜZELTİLMEZ (ürün kodu A5.4'ün işi). Her hücre
//! `Result<(), Red>` döner; [`DECLARED_RED`] hangi hücrelerin bugün kırmızı olduğunu beyan eder ve
//! sarmalayıcı beyanla gerçeği ÇAPRAZ denetler: beyansız kırmızı da, beyanlı ama yeşile dönmüş
//! hücre de testi düşürür. Böylece kapı yeşil kalır ama kırmızılar sessizce kaybolamaz.

use kama_noise_wire::epoch_machine::GRACE_MIN_PACKETS;
use kama_noise_wire::init_flight::{
    InitAdmission, InitFlight, InitReceiver, InitReplayIdentity, InitRetryConfig, RetransmitOutcome,
};
use kama_noise_wire::{
    DhEpochMachine, EpochAction, KamaNoiseWireProtector, RecvEpoch0, Role, SealArgs,
};
use kama_transport_schema::inner_kind::ShardKind;
use kama_transport_schema::{LogicalShardHeader, UnsealedTransportPacket};
use zeroize::Zeroizing;

/// KS-GRACE-1 `N_g` — kodda bu ada sahip sabit yok; karşılığı `GRACE_MIN_PACKETS`.
const N_G: u32 = GRACE_MIN_PACKETS;

/// Kırmızı hücre kaydı: beklenen / gözlenen / kanıt yeri.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Red {
    pub expected: &'static str,
    pub observed: &'static str,
    pub at: &'static str,
}

pub type Cell = Result<(), Red>;

/// Bugün kırmızı olan hücreler. Bir hücre A5.4'te yeşile dönünce buradan SİLİNİR (aynı commit).
pub const DECLARED_RED: &[&str] = &[
    "cell_5b_n_g_is_fixed",
    "cell_6_time_closes_grace",
    "cell_7_whichever_first",
    "cell_9_at_most_two_key_trials",
];

/// Koşucunun okuduğu makine satırı.
pub const RED_SENTINEL: &str = "EPOCH-RED-CELL";

/// Beyan ↔ gerçek çapraz denetimi.
///
/// # Panics
/// Hücre beyansız kırmızıysa ya da beyanlı olduğu hâlde yeşilse.
pub fn check(name: &str, outcome: Cell) {
    let declared = DECLARED_RED.contains(&name);
    match (outcome, declared) {
        (Ok(()), false) => {}
        (Err(red), true) => println!(
            "{RED_SENTINEL}: {name} | beklenen: {} | gözlenen: {} | {}",
            red.expected, red.observed, red.at
        ),
        (Err(red), false) => panic!("BEYANSIZ KIRMIZI {name}: {red:?}"),
        (Ok(()), true) => panic!("{name} yeşile döndü: DECLARED_RED'den sil"),
    }
}

// ── Test tutkalı ─────────────────────────────────────────────────────────────────────────────

const H_EXT: u32 = 0x9;

fn key(s: u8) -> Zeroizing<[u8; 32]> {
    Zeroizing::new([s; 32])
}

fn packet() -> UnsealedTransportPacket {
    UnsealedTransportPacket::FecShard {
        header: LogicalShardHeader {
            kind: ShardKind::Source,
            profile_id: 3,
            block_id: 1,
            esi: 0,
            k_actual: 1,
        },
        symbol: vec![0u8; 64],
    }
}

/// Bir uç: makine + protector + epoch başına gönderim sayacı (§4 EPOCH-PN-INIT).
struct Endpoint {
    machine: DhEpochMachine,
    protector: KamaNoiseWireProtector,
    next_pn: [u64; 2],
    /// Geçerli epoch-1 paketiyle ilerleyen grace sayacının kaç kez işlediği.
    grace_ticks: u32,
}

impl Endpoint {
    fn seal(&mut self, dh_epoch: u8) -> Vec<u8> {
        let e = usize::from(dh_epoch & 1);
        let wire_pn = self.next_pn[e];
        self.next_pn[e] += 1;
        self.protector
            .seal(
                &packet(),
                SealArgs {
                    wire_pn,
                    dh_epoch,
                    send_generation: 0,
                },
            )
            .expect("kurulu epoch anahtarıyla seal")
            .bytes
    }

    /// Alım: yalnız AEAD-geçerli paket makineyi sürer. `true` = açıldı.
    fn deliver(&mut self, wire: &[u8], dh_epoch: u8) -> bool {
        if self.protector.open(wire).is_err() {
            return false;
        }
        if dh_epoch & 1 == 1 {
            if matches!(self.machine.recv0(), RecvEpoch0::Grace { .. }) {
                self.grace_ticks += 1;
                if self.machine.on_grace_tick() == Some(EpochAction::DiscardEpoch0Recv) {
                    self.protector.discard_epoch0_recv();
                }
            }
            // Adım-3/4: ilk geçerli epoch-1 paketi = implicit confirmation (Responder'da).
            let _ = self.machine.on_valid_epoch1_received();
        }
        true
    }
}

/// Alice (initiator) ve Bob (responder); her ikisinde epoch-1 anahtarları kuruludur (Bob RESPONSE'u
/// gönderirken, Alice RESPONSE'u aldığında kurar — burada önceden kurulur, geçişi MAKİNE sürer).
fn pair(grace_budget: u32) -> (Endpoint, Endpoint) {
    let mut a = KamaNoiseWireProtector::new(H_EXT, &key(0xAA), &key(0xBB));
    let mut b = KamaNoiseWireProtector::new(H_EXT, &key(0xBB), &key(0xAA));
    a.install_epoch1_keys(&key(0xCC), &key(0xDD));
    b.install_epoch1_keys(&key(0xDD), &key(0xCC));
    let ep = |role, protector| Endpoint {
        machine: DhEpochMachine::new(role, grace_budget),
        protector,
        next_pn: [0, 0],
        grace_ticks: 0,
    };
    (ep(Role::Initiator, a), ep(Role::Responder, b))
}

/// Alice RESPONSE'u aldı, Bob ilk geçerli epoch-1 paketini aldı: grace AÇIK.
fn confirmed_pair(grace_budget: u32) -> (Endpoint, Endpoint) {
    let (mut a, mut b) = pair(grace_budget);
    assert_eq!(
        a.machine.on_response_received(),
        Some(EpochAction::InstallEpoch1AndSend)
    );
    let first = a.seal(1);
    assert!(b.deliver(&first, 1));
    assert!(matches!(b.machine.recv0(), RecvEpoch0::Grace { .. }));
    (a, b)
}

fn tamper(wire: &[u8]) -> Vec<u8> {
    let mut w = wire.to_vec();
    let last = w.len() - 1;
    w[last] ^= 1;
    w
}

// ── Matris ───────────────────────────────────────────────────────────────────────────────────

/// 1 — RESPONSE normal: epoch-1 kurulur; ilk geçerli epoch-1 paketiyle onay.
pub fn cell_1_response_normal() -> Cell {
    let (mut a, mut b) = pair(N_G);
    assert_eq!(
        b.machine.send_epoch(),
        0,
        "Bob onaydan önce epoch-0'da gönderir"
    );
    assert_eq!(
        a.machine.on_response_received(),
        Some(EpochAction::InstallEpoch1AndSend)
    );
    assert_eq!(a.machine.send_epoch(), 1);
    let first = a.seal(1);
    assert!(b.deliver(&first, 1));
    assert_eq!(b.machine.send_epoch(), 1, "onay: Bob epoch-1'e geçer");
    assert!(matches!(b.machine.recv0(), RecvEpoch0::Grace { .. }));
    assert!(
        b.machine.recv_accepts(0),
        "grace boyunca epoch-0 hâlâ kabul"
    );
    Ok(())
}

/// 2 — RESPONSE kayıp: INIT yeniden gönderimi sınırlı; epoch-0 açık kalır; onay yok.
pub fn cell_2_response_lost() -> Cell {
    let (mut a, mut b) = pair(N_G);
    // Alice RESPONSE'u hiç almadı: epoch-0'da kalır ve Bob onu kabul etmeyi sürdürür.
    let cfg = InitRetryConfig::alpha_default();
    let mut flight = InitFlight::new(vec![0xA5; 32], 1, cfg);
    let mut sends = 0u32;
    while let RetransmitOutcome::Send { datagram, .. } = flight.retransmit() {
        assert_eq!(datagram, &[0xA5; 32][..], "yeniden gönderim byte-özdeş");
        sends += 1;
        assert!(sends <= cfg.max_retries, "yeniden gönderim sınırsız");
    }
    assert_eq!(flight.retransmit(), RetransmitOutcome::Exhausted);

    assert_eq!(a.machine.send_epoch(), 0);
    let p = a.seal(0);
    assert!(b.deliver(&p, 0), "epoch-0 kullanılabilir kalır");
    assert_eq!(b.machine.send_epoch(), 0, "onay yok");
    assert_eq!(b.machine.recv0(), RecvEpoch0::Accepting, "grace başlamaz");
    Ok(())
}

/// 3 — RESPONSE sırasız (INIT yeniden gönderiminden sonra): tek oturum, çift kurulum yok.
pub fn cell_3_response_reordered() -> Cell {
    let id = InitReplayIdentity {
        sender_device_id: [1; 16],
        recipient_device_id: [2; 16],
        init_counter: 7,
    };
    let mut rx = InitReceiver::new();
    assert_eq!(rx.admit(id, [9; 32]), InitAdmission::Fresh);
    // Alice RESPONSE'u göremeden INIT'i yeniden gönderdi; Bob aynı oturuma eşler.
    assert_eq!(
        rx.admit(id, [9; 32]),
        InitAdmission::DuplicateResendConfirmation
    );
    assert_eq!(rx.established_count(), 1, "ikinci oturum yok");

    // İki RESPONSE kopyası da Alice'e ulaştı: geçiş bir kez olur.
    let (mut a, _b) = pair(N_G);
    assert_eq!(
        a.machine.on_response_received(),
        Some(EpochAction::InstallEpoch1AndSend)
    );
    let after_first = a.machine;
    assert_eq!(a.machine.on_response_received(), None, "çift kurulum yok");
    assert_eq!(a.machine, after_first, "ikinci RESPONSE state değiştirmez");
    Ok(())
}

/// 4 — AEAD-geçersiz epoch-1 paketi onayı tetiklemez, grace sayacını ilerletmez.
pub fn cell_4_invalid_epoch1_is_inert() -> Cell {
    // (a) onaydan önce
    let (mut a, mut b) = pair(N_G);
    a.machine.on_response_received();
    let forged = tamper(&a.seal(1));
    let before = b.machine;
    assert!(!b.deliver(&forged, 1));
    assert_eq!(b.machine, before, "sahte paket onay üretmez");
    assert_eq!(b.machine.send_epoch(), 0);

    // (b) grace içinde
    let (mut a, mut b) = confirmed_pair(N_G);
    let before = b.machine;
    let ticks = b.grace_ticks;
    let forged = tamper(&a.seal(1));
    assert!(!b.deliver(&forged, 1));
    assert_eq!(b.machine, before, "sahte paket grace'i ilerletmez");
    assert_eq!(b.grace_ticks, ticks);
    Ok(())
}

/// 5 — `N_g` geçerli epoch-1 paketi grace'i kapatır; sonraki epoch-0 paketi reddedilir; yeniden açılmaz.
pub fn cell_5_count_closes_grace() -> Cell {
    let (mut a, mut b) = confirmed_pair(N_G);
    let late_epoch0 = a.seal(0);
    for i in 1..=N_G {
        assert!(b.machine.recv_accepts(0), "{i}. paketten önce grace açık");
        let p = a.seal(1);
        assert!(b.deliver(&p, 1));
    }
    assert_eq!(b.grace_ticks, N_G);
    assert_eq!(b.machine.recv0(), RecvEpoch0::Discarded);
    assert!(
        !b.deliver(&late_epoch0, 0),
        "kapanıştan sonra epoch-0 reddedilir"
    );

    // Yeniden açılmaz: sonraki hiçbir makine olayı epoch-0 kabulünü geri getirmez.
    let p = a.seal(1);
    assert!(b.deliver(&p, 1));
    let _ = b.machine.on_response_received();
    let _ = b.machine.on_valid_epoch1_received();
    let _ = b.machine.on_grace_tick();
    assert!(!b.machine.recv_accepts(0));
    Ok(())
}

/// 5b — `N_g` sabittir: kurucuya verilen bütçe pencereyi büyütemez (KS-GRACE-1: sRTT bağı yok).
pub fn cell_5b_n_g_is_fixed() -> Cell {
    let (mut a, mut b) = confirmed_pair(N_G + 1);
    for _ in 0..N_G {
        let p = a.seal(1);
        assert!(b.deliver(&p, 1));
    }
    if b.machine.recv0() == RecvEpoch0::Discarded {
        return Ok(());
    }
    Err(Red {
        expected: "grace N_g geçerli epoch-1 paketinde kapanır (kurucu argümanından bağımsız)",
        observed: "bütçe = max(argüman, GRACE_MIN_PACKETS); N_g+1 verilince N_g pakette kapanmadı",
        at: "crates/kama-noise-wire/src/epoch_machine.rs:57-67",
    })
}

/// 6 — `T_g` monotonic süre grace'i kapatır.
pub fn cell_6_time_closes_grace() -> Cell {
    Err(Red {
        expected: "onay anından T_g (30 s, monotonic) sonra grace kapanır, epoch-0 reddedilir",
        observed:
            "makinede zaman girdisi yok (yalnız on_grace_tick paket sayacı); T_g sabiti kodda yok",
        at: "crates/kama-noise-wire/src/epoch_machine.rs:112-127",
    })
}

/// 7 — hangisi önce: `N_g − 1` paket + `T_g`'den kısa süre → açık; `N_g`'inci paket → kapanır.
pub fn cell_7_whichever_first() -> Cell {
    let (mut a, mut b) = confirmed_pair(N_G);
    for _ in 0..N_G - 1 {
        let p = a.seal(1);
        assert!(b.deliver(&p, 1));
    }
    assert!(b.machine.recv_accepts(0), "N_g − 1 pakette açık");
    let p = a.seal(1);
    assert!(b.deliver(&p, 1));
    assert!(!b.machine.recv_accepts(0), "N_g'inci pakette kapanır");
    // Paket yarısı yukarıda sınandı ve tutuyor. Süre yarısı kurulamıyor (hücre 6).
    Err(Red {
        expected: "paket ve süre koşulu birlikte: hangisi önce dolarsa",
        observed: "paket koşulu tutuyor; süre koşulu yok, bu yüzden 'hangisi önce' sınanamıyor",
        at: "crates/kama-noise-wire/src/epoch_machine.rs:112-127",
    })
}

/// 8 — grace içinde epoch-1 anahtarıyla açılan paket epoch-1 penceresine işlenir; epoch-0 penceresi aynı kalır.
pub fn cell_8_commit_goes_to_trial_epoch_window() -> Cell {
    let (mut a, mut b) = confirmed_pair(N_G);
    // Epoch-0 penceresini ilerlet, sonra durumunu kaydet.
    let p0 = a.seal(0);
    assert!(b.deliver(&p0, 0));
    let e0_before = b.protector.recv_expected_pn(0);
    let e1_before = b.protector.recv_expected_pn(1);
    let held_epoch0 = a.seal(0);

    let p1 = a.seal(1);
    assert!(b.deliver(&p1, 1));
    assert_eq!(b.protector.recv_expected_pn(1), e1_before + 1);
    assert_eq!(
        b.protector.recv_expected_pn(0),
        e0_before,
        "epoch-0 penceresi oynamaz"
    );
    // Davranışsal kanıt: epoch-0 penceresi bozulmadıysa bekleyen epoch-0 paketi hâlâ kabul edilir,
    // tekrarı reddedilir.
    assert!(b.deliver(&held_epoch0, 0));
    assert!(!b.deliver(&held_epoch0, 0), "replay");
    Ok(())
}

/// 9 — grace içinde en fazla iki anahtar denemesi (legacy zarfta epoch biti yok).
pub fn cell_9_at_most_two_key_trials() -> Cell {
    Err(Red {
        expected: "legacy 22 B zarfta grace içinde ≤2 anahtar denenir; deneme sayacı gözlenir",
        observed:
            "LegacySessionProtector tek anahtar/tek epoch tutar; iki anahtarlı deneme ve sayaç yok \
                   (extended zarfta epoch biti telde, deneme hep 1 — protector.rs:312-327)",
        at: "crates/kama-noise-wire/src/legacy.rs:76-116",
    })
}

/// `N_g` kodda adlandırılmış sabitten gelir ve spec'teki değere eşittir.
pub fn n_g_matches_spec() {
    const NAME: &str = "N_g = ";
    let spec = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../specs/kama-noise-wire-tdd-v2.0.md"
    ))
    .expect("noise-wire spec okunmalı");
    let values: Vec<u32> = spec
        .lines()
        .filter(|l| !l.contains("~~"))
        .flat_map(|l| {
            l.match_indices(NAME)
                .map(move |(i, _)| &l[i + NAME.len()..])
        })
        .filter_map(|rest| {
            let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
            digits.parse().ok()
        })
        .collect();
    assert!(!values.is_empty(), "spec'te sayısal N_g ataması bulunamadı");
    assert!(
        values.iter().all(|v| *v == N_G),
        "spec {values:?} ≠ GRACE_MIN_PACKETS {N_G}"
    );
}

/// Beyan listesi kendi içinde tutarlı ve her adı bir hücreye karşılık geliyor.
pub fn declared_red_is_consistent(known: &[&str]) {
    for (i, name) in DECLARED_RED.iter().enumerate() {
        assert!(!DECLARED_RED[..i].contains(name), "çift beyan: {name}");
        assert!(known.contains(name), "bilinmeyen hücre beyanı: {name}");
    }
}

/// Matrisi, mevcut makine hedefi için hücre başına ayrı `#[test]` olarak sarar.
#[macro_export]
macro_rules! noise_epoch_suite {
    ($suite:ident; $($cell:ident),+ $(,)?) => {
        mod $suite {
            $(
                #[test]
                fn $cell() {
                    $crate::epoch::check(stringify!($cell), $crate::epoch::$cell());
                }
            )+

            #[test]
            fn n_g_matches_spec() {
                $crate::epoch::n_g_matches_spec();
            }

            #[test]
            fn declared_red_is_consistent() {
                $crate::epoch::declared_red_is_consistent(&[$(stringify!($cell)),+]);
            }
        }
    };
}
