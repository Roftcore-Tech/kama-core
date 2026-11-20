//! ACK REFERANS DÜZENEĞİ · (b) teslim doğruluğu ve ilerleme + (c) paket numarası ≠ birim.
//!
//! # Neden var
//!
//! ACK biçimini değiştirecek bir PR'ın "eski ACK'in alt kümesini taşıyor" demesi yetmez: asıl
//! sözleşme şudur — göndericinin teyitli saydığı her paket numarası alıcıya gerçekten ulaşmış ve
//! alıcı onu kabul etmiştir; ve hiçbir teyitsiz birim kalıcı olarak unutulmaz (ya teyit edilir ya
//! yeniden gönderilir). Bu dosya o sözleşmeyi GERÇEK `Sender` + GERÇEK `Receiver` + gerçek ACK
//! codec'i (`AckFrame::encode`/`decode`/`acked_full`) ile, bugünkü kodda sınar. Motor ya da tel
//! davranışı değişmez.
//!
//! # Düzenek
//!
//! Ayrık olay, 1 ms tik, belirlenimli (testin kendi LCG'si). İleri hat: serileştirme + sabit tek
//! yön gecikme + desene göre kayıp (rastgele / patlamalı / "her ikinciyi düşür" deseni — sonuncusu
//! çok sayıda blok üretip 16 aralık tavanını zorlar). Dönüş hattı: ACK baytları (`encode`) taşınır,
//! göndericide `decode` + `acked_full` ile çözülür — ürün yolu gibi (`client-core` runtime: çerçeve
//! çözülür, `acked_full` farksız motora verilir). Dönüşte ACK kaybı ve değişken gecikme (ACK
//! sırasızlığı) vardır. Gönderici `expected`'i kendi en büyük gönderdiği seq + 1'dir (runtime'daki
//! `max_sealed_pn + 1` karşılığı).
//!
//! # Değişmezler (her olayda denetlenir)
//!
//! * **D1** Yayılan her ACK'in teyit kümesi ⊆ alıcının o ana dek gerçekten aldığı seq kümesi.
//! * **D2** Göndericiye verilen her teyit seq'i hiç gönderilmiş bir seq'tir ve alınmıştır.
//! * **D3** Göndericinin teyit ettiği her birim (`take_confirmed_units`) alıcıda teslim edilmiştir
//!   (ya da (c)'de alıcının FEC kurtarma kanıtı verilmiştir).
//! * **D4** ACK'le uçuştan çıkan seq bir daha uçuşta görünmez; uçuş baytı = uçuştaki kayıtların
//!   gönderim boylarının toplamı (uçuş muhasebesi paket numarasıyla tutulur).
//! * **İLERLEME** Ufuk sonunda her birim alıcıda teslim edilmiş VE göndericide teyit edilmiştir.
//!   Teyitsiz birim kalırsa son iletim anıyla listelenir (sessiz vazgeçme = kırmızı).
//!
//! (c) ayrıca: FEC ile kurtarılan birimin düşen paketi alınmış paket numarası SAYILMAZ — ACK'te
//! görünmez, RTT örneği üretmez, `RecoveredControl` onu uçuştan ACK gibi çıkarmaz; birim teyit
//! edilir ve yeniden gönderilmez.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use adaptive_transport::ack::AckFrame;
use adaptive_transport::receiver::{Receiver, RxDatagram, RxKind};
use adaptive_transport::sender::{
    AckInfo, Event, InitialWindowConfig, OpaqueUnit, RecoveredInfo, RecoveredUnit, Sender, Timer,
    TransmitDecision, TransmitKind,
};
use adaptive_transport::tail_recovery::TailRecoveryConfig;

const MS: u64 = 1_000_000;
const SEC: u64 = 1_000_000_000;

struct Lcg(u64);
impl Lcg {
    fn new(seed: u64) -> Self {
        Self(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ 0xD1B5_4A32_D192_ED03)
    }
    fn permille(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (self.0 >> 33) % 1000
    }
}

#[derive(Debug, Clone, Copy)]
enum Loss {
    None,
    Random(u64),
    Burst {
        p_gb: u64,
        p_bg: u64,
    },
    /// İlk `n` iletimde her ikinciyi düşür (çok blok → 16 aralık tavanı), sonra `then` rastgele.
    Alternate {
        n: u64,
        then: u64,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Engine {
    /// Ürünün eski motor profili (`KAMA_TRANSPORT_ENGINE=legacy`; `ClientSession::new()`):
    /// `Sender::new()` + `validated_v1` kuyruk kurtarması + ilk pencere 2·MTU.
    Legacy,
    CcPort,
    /// Port + alıcı dönüş bütçesi (ürünün dar dönüş profili; `set_return_bandwidth`).
    CcPortBudget(u64),
}

#[derive(Debug, Clone, Copy)]
struct Cell {
    name: &'static str,
    engine: Engine,
    seed: u64,
    units: u64,
    unit_size: u64,
    fwd_bps: u64,
    one_way_ns: u64,
    loss: Loss,
    ack_loss_permille: u64,
    /// Dönüşte ACK başına ek gecikme 0..jitter (ACK'ler sırasız varabilir).
    rev_jitter_ns: u64,
    t_max_ns: u64,
}

#[derive(Default, Debug)]
struct Stats {
    acks_emitted: u64,
    acks_capped: u64,
    acks_dropped: u64,
    acks_reordered: u64,
    max_frame_bytes: usize,
    sent: u64,
    retx: u64,
    fwd_dropped: u64,
    /// Alıcıda zaten olan bir seq'in birimi için yeniden iletim (tavan/ACK kaybı yüzünden
    /// göndericinin göremediği teyit).
    spurious_retx_units: u64,
}

fn check_inflight(s: &Sender, sizes: &HashMap<u64, u64>, acked_once: &BTreeSet<u64>, at: &str) {
    let fl = s.diag_inflight_seqs();
    let mut sum = 0u64;
    for &(seq, _, _, _) in &fl {
        assert!(
            !acked_once.contains(&seq),
            "D4 [{at}]: ACK'le teyit edilen seq {seq} hâlâ uçuşta"
        );
        sum += sizes
            .get(&seq)
            .copied()
            .expect("uçuştaki seq gönderilmiş olmalı");
    }
    assert_eq!(
        s.inflight_bytes(),
        sum,
        "D4 [{at}]: uçuş baytı uçuştaki kayıtların toplamı değil"
    );
}

/// Harness dünyası: hatlar + gerçek defterler (motor DIŞI; motorun kararlarını yalnız gözler).
struct World {
    c: Cell,
    rng: Lcg,
    rev_rng: Lcg,
    rev_bps: Option<u64>,
    st: Stats,
    seq_unit: HashMap<u64, u64>,
    seq_size: HashMap<u64, u64>,
    max_sent: Option<u64>,
    rx_truth: BTreeSet<u64>,
    last_tx: HashMap<u64, u64>,
    fwd_busy: u64,
    /// (varış, sıra) → (seq, birim)
    fwd: BTreeMap<(u64, u64), (u64, u64)>,
    rev_busy: u64,
    /// (varış, sıra) → ACK baytları
    rev: BTreeMap<(u64, u64), Vec<u8>>,
    ord: u64,
    sent_n: u64,
    bad: bool,
}

impl World {
    fn emit(&mut self, decs: Vec<TransmitDecision>, now: u64) {
        for d in decs {
            if !matches!(
                d.kind,
                TransmitKind::Data | TransmitKind::Retransmit | TransmitKind::Duplicate
            ) {
                continue;
            }
            assert!(
                !self.seq_unit.contains_key(&d.seq),
                "seq {} yeniden kullanıldı",
                d.seq
            );
            if d.kind != TransmitKind::Data {
                self.st.retx += 1;
                let uid = d.unit_id;
                if self
                    .rx_truth
                    .iter()
                    .any(|q| self.seq_unit.get(q).copied() == Some(uid))
                {
                    self.st.spurious_retx_units += 1;
                }
            }
            self.seq_unit.insert(d.seq, d.unit_id);
            self.seq_size.insert(d.seq, d.size);
            self.max_sent = Some(self.max_sent.map_or(d.seq, |m| m.max(d.seq)));
            self.last_tx.insert(d.unit_id, now);
            self.st.sent += 1;
            let lost = match self.c.loss {
                Loss::None => false,
                Loss::Random(p) => self.rng.permille() < p,
                Loss::Burst { p_gb, p_bg } => {
                    self.bad = if self.bad {
                        self.rng.permille() >= p_bg
                    } else {
                        self.rng.permille() < p_gb
                    };
                    self.bad
                }
                Loss::Alternate { n, then } => {
                    if self.sent_n < n {
                        self.sent_n % 2 == 1
                    } else {
                        self.rng.permille() < then
                    }
                }
            };
            self.sent_n += 1;
            let ser = d.size * SEC / self.c.fwd_bps;
            self.fwd_busy = self.fwd_busy.max(now) + ser;
            if lost {
                self.st.fwd_dropped += 1;
                continue;
            }
            self.ord += 1;
            self.fwd.insert(
                (self.fwd_busy + self.c.one_way_ns, self.ord),
                (d.seq, d.unit_id),
            );
        }
    }

    fn push_ack(&mut self, f: &AckFrame, now: u64, largest: u64) {
        self.st.acks_emitted += 1;
        if f.ranges.len() == adaptive_transport::ack::MAX_ACK_RANGES {
            self.st.acks_capped += 1;
        }
        // D1: yayılan çerçevenin teyit kümesi alıcının gerçekte aldıklarının alt kümesi.
        for q in f.acked_full(largest + 1) {
            assert!(self.rx_truth.contains(&q), "D1: alınmamış seq {q} ACK'te");
        }
        let bytes = f.encode();
        self.st.max_frame_bytes = self.st.max_frame_bytes.max(bytes.len());
        if self.rev_rng.permille() < self.c.ack_loss_permille {
            self.st.acks_dropped += 1;
            return;
        }
        let jitter = if self.c.rev_jitter_ns > 0 {
            self.rev_rng.permille() * self.c.rev_jitter_ns / 1000
        } else {
            0
        };
        // Dönüş bütçeli hücrede dönüş hattı da serileştirir (çerçeve + ≈50 B zarf).
        let ser = self
            .rev_bps
            .map_or(0, |bw| (bytes.len() as u64 + 50) * SEC / bw);
        self.rev_busy = self.rev_busy.max(now) + ser;
        self.ord += 1;
        self.rev.insert(
            (self.rev_busy + self.c.one_way_ns + jitter, self.ord),
            bytes,
        );
    }
}

fn legacy_product_sender() -> Sender {
    let mut s = Sender::new();
    s.set_tail_recovery_config(TailRecoveryConfig::validated_v1());
    s.set_initial_window(InitialWindowConfig { bytes: 2 * 1200 });
    s
}

fn drain_confirmed(s: &mut Sender, delivered: &BTreeSet<u64>, confirmed: &mut BTreeSet<u64>) {
    for u in s.take_confirmed_units() {
        assert!(
            delivered.contains(&u),
            "D3: teslim edilmemiş birim {u} teyitli"
        );
        confirmed.insert(u);
    }
}

/// Hücreyi koşturur; değişmezler içeride iddia edilir. Döner: istatistik.
fn run(c: Cell) -> Stats {
    let (mut s, mut r) = match c.engine {
        Engine::Legacy => (legacy_product_sender(), Receiver::new()),
        Engine::CcPort => (Sender::with_cc_port(), Receiver::with_cc_port()),
        Engine::CcPortBudget(bw) => {
            let mut r = Receiver::with_cc_port();
            r.set_return_bandwidth(Some(bw));
            (Sender::with_cc_port(), r)
        }
    };
    let mut w = World {
        c,
        rng: Lcg::new(c.seed),
        rev_rng: Lcg::new(c.seed ^ 0x5A5A),
        rev_bps: match c.engine {
            Engine::CcPortBudget(bw) => Some(bw),
            _ => None,
        },
        st: Stats::default(),
        seq_unit: HashMap::new(),
        seq_size: HashMap::new(),
        max_sent: None,
        rx_truth: BTreeSet::new(),
        last_tx: HashMap::new(),
        fwd_busy: 0,
        fwd: BTreeMap::new(),
        rev_busy: 0,
        rev: BTreeMap::new(),
        ord: 0,
        sent_n: 0,
        bad: false,
    };
    let mut delivered: BTreeSet<u64> = BTreeSet::new();
    let mut confirmed: BTreeSet<u64> = BTreeSet::new();
    let mut acked_once: BTreeSet<u64> = BTreeSet::new();
    let mut last_rev_arrival = 0u64;

    // Dosya tek seferde kuyruğa.
    for i in 0..c.units {
        let d = s.on_event(
            Event::AppData(OpaqueUnit {
                id: i,
                size: c.unit_size,
            }),
            0,
        );
        w.emit(d, 0);
    }

    let mut now = 0u64;
    while now < c.t_max_ns {
        now += MS;
        let d = s.on_event(Event::TimerFired(Timer::Pto), now);
        w.emit(d, now);
        if now.is_multiple_of(200 * MS) {
            let d = s.on_event(Event::TimerFired(Timer::RtpropProbe), now);
            w.emit(d, now);
        }
        drain_confirmed(&mut s, &delivered, &mut confirmed);
        // Alıcı varışları.
        let due: Vec<(u64, u64)> = w.fwd.range(..=(now, u64::MAX)).map(|(k, _)| *k).collect();
        for k in due {
            let (seq, unit) = w.fwd.remove(&k).expect("var");
            w.rx_truth.insert(seq);
            let out = r.on_datagram(
                RxDatagram {
                    seq,
                    unit_id: unit,
                    kind: RxKind::Data,
                },
                now,
            );
            delivered.extend(out.delivered);
            if let Some(f) = out.ack {
                let largest = r.largest_seq().expect("alındı");
                w.push_ack(&f, now, largest);
            }
        }
        if let Some(f) = r.on_ack_timer(now) {
            let largest = r.largest_seq().expect("alındı");
            w.push_ack(&f, now, largest);
        }
        // Gönderici ACK varışları (sırasız olabilir).
        let due: Vec<(u64, u64)> = w.rev.range(..=(now, u64::MAX)).map(|(k, _)| *k).collect();
        for k in due {
            let bytes = w.rev.remove(&k).expect("var");
            if k.1 < w.ord && k.0 < last_rev_arrival {
                w.st.acks_reordered += 1;
            }
            last_rev_arrival = last_rev_arrival.max(k.0);
            let f = AckFrame::decode(&bytes).expect("yayılan ACK çözülmeli");
            let expected = w.max_sent.map_or(0, |m| m + 1);
            let acked = f.acked_full(expected);
            // D2: teyit edilen her seq gönderilmiş ve alınmış.
            for q in &acked {
                assert!(
                    w.seq_unit.contains_key(q),
                    "D2: hiç gönderilmemiş seq {q} teyitli"
                );
                assert!(w.rx_truth.contains(q), "D2: alınmamış seq {q} teyitli");
            }
            let before: BTreeSet<u64> = s.diag_inflight_seqs().iter().map(|e| e.0).collect();
            let decs = s.on_event(
                Event::AckReceived(AckInfo {
                    acked_seqs: acked.clone(),
                    ack_delay: f.ack_delay_duration(),
                }),
                now,
            );
            acked_once.extend(acked.into_iter().filter(|q| before.contains(q)));
            w.emit(decs, now);
            drain_confirmed(&mut s, &delivered, &mut confirmed);
            check_inflight(&s, &w.seq_size, &acked_once, c.name);
        }
        if now.is_multiple_of(100 * MS) {
            check_inflight(&s, &w.seq_size, &acked_once, c.name);
        }
        if confirmed.len() as u64 == c.units && delivered.len() as u64 == c.units {
            break;
        }
    }
    let st = w.st;
    let all: BTreeSet<u64> = (0..c.units).collect();
    let missing: Vec<(u64, Option<u64>)> = all
        .difference(&confirmed)
        .map(|u| (*u, w.last_tx.get(u).copied()))
        .collect();
    println!(
        "ACK-REF {} motor={:?} tohum={} süre={:.2}s teslim={}/{} teyit={}/{} gönderim={} \
         yeniden={} gereksiz_yeniden={} ileri_düşen={} ack={} ack_tavanda={} ack_düşen={} \
         ack_sırasız={} en_büyük_çerçeve={}B",
        c.name,
        c.engine,
        c.seed,
        now as f64 / SEC as f64,
        delivered.len(),
        c.units,
        confirmed.len(),
        c.units,
        st.sent,
        st.retx,
        st.spurious_retx_units,
        st.fwd_dropped,
        st.acks_emitted,
        st.acks_capped,
        st.acks_dropped,
        st.acks_reordered,
        st.max_frame_bytes
    );
    if c.engine == Engine::Legacy && !missing.is_empty() {
        // Eski motor (ürün geri alma yolu) yeniden iletim tavanı 3'tür (`RetxConfig::fixture`;
        // aşımda ileri kopyaya devir). Patlamalı kayıpta birim teslim edilmeden kalabilir — bu
        // ACK biçiminden bağımsız, bugünkü eski motor davranışıdır. İLERLEME yalnız ürün varsayılanı
        // port motorunda bağlayıcıdır (cc_port_sim ile aynı kural); D1–D4 her motorda bağlayıcıdır.
        let tx: Vec<(u64, usize)> = missing
            .iter()
            .map(|(u, _)| (*u, w.seq_unit.values().filter(|x| *x == u).count()))
            .collect();
        println!(
            "ACK-REF BİLİNEN-ESKİ-MOTOR [{}] tohum={} teyitsiz birim (birim, iletim sayısı): {tx:?}",
            c.name, c.seed
        );
        return st;
    }
    assert_eq!(
        delivered, all,
        "İLERLEME [{}]: alıcıda eksik birim var",
        c.name
    );
    assert!(
        missing.is_empty(),
        "İLERLEME [{}]: teyitsiz birimler (birim, son iletim ns): {missing:?}",
        c.name
    );
    st
}

const ENGINES: [Engine; 3] = [Engine::Legacy, Engine::CcPort, Engine::CcPortBudget(2_000)];

fn base(name: &'static str, engine: Engine, seed: u64) -> Cell {
    Cell {
        name,
        engine,
        seed,
        units: 300,
        unit_size: 500,
        fwd_bps: 100_000,
        one_way_ns: 50 * MS,
        loss: Loss::None,
        ack_loss_permille: 0,
        rev_jitter_ns: 0,
        t_max_ns: 300 * SEC,
    }
}

#[test]
fn ack_ref_delivery_clean_path() {
    for e in ENGINES {
        let st = run(base("temiz", e, 1));
        assert_eq!(st.fwd_dropped, 0);
    }
}

#[test]
fn ack_ref_delivery_random_and_burst_forward_loss() {
    for e in ENGINES {
        for seed in 1..=4 {
            run(Cell {
                loss: Loss::Random(200),
                ..base("rastgele-%20", e, seed)
            });
            run(Cell {
                loss: Loss::Burst {
                    p_gb: 50,
                    p_bg: 250,
                },
                ..base("patlamalı", e, seed)
            });
        }
    }
}

#[test]
fn ack_ref_delivery_ack_loss_and_reordering() {
    for e in ENGINES {
        for seed in 1..=4 {
            let st = run(Cell {
                loss: Loss::Random(100),
                ack_loss_permille: 400,
                rev_jitter_ns: 120 * MS,
                ..base("ack-kaybı+sırasızlık", e, seed)
            });
            assert!(st.acks_dropped > 0, "vakum: ACK kaybı sınanmadı");
        }
    }
}

/// Çok blok (her ikinciyi düşür) + ACK kaybı: eski bloklar 16 aralık tavanından düşer; teyidi
/// göndericiye ulaşmamış birimler yine de teyit edilmeli (gerekirse yeniden iletimle).
#[test]
fn ack_ref_delivery_range_truncation() {
    let mut capped_total = 0;
    for e in ENGINES {
        for seed in 1..=3 {
            let st = run(Cell {
                loss: Loss::Alternate { n: 200, then: 50 },
                ack_loss_permille: 500,
                rev_jitter_ns: 60 * MS,
                ..base("aralık-kesmesi", e, seed)
            });
            capped_total += st.acks_capped;
            assert_eq!(st.max_frame_bytes, 79, "tavan çerçevesine ulaşılmalı");
        }
    }
    assert!(capped_total > 0, "vakum: 16 aralık tavanı hiç kesmedi");
}

/// (c) FEC kurtarması: düşen paketin numarası alınmış SAYILMAZ; birim teyit edilir.
#[test]
fn ack_ref_fec_recovered_unit_is_not_a_received_packet_number() {
    for e in [Engine::Legacy, Engine::CcPort] {
        let (mut s, mut r) = match e {
            Engine::Legacy => (legacy_product_sender(), Receiver::new()),
            _ => (Sender::with_cc_port(), Receiver::with_cc_port()),
        };
        const LOST_UNIT: u64 = 3;
        let mut seq_unit: HashMap<u64, u64> = HashMap::new();
        let mut sent: Vec<(u64, u64, u64)> = Vec::new(); // (an, seq, birim)
        let mut note = |decs: Vec<TransmitDecision>, now: u64, sent: &mut Vec<(u64, u64, u64)>| {
            for d in decs {
                if matches!(
                    d.kind,
                    TransmitKind::Data | TransmitKind::Retransmit | TransmitKind::Duplicate
                ) {
                    seq_unit.insert(d.seq, d.unit_id);
                    sent.push((now, d.seq, d.unit_id));
                }
            }
        };
        for i in 0..8u64 {
            let d = s.on_event(Event::AppData(OpaqueUnit { id: i, size: 400 }), 0);
            note(d, 0, &mut sent);
        }
        let mut now = 0u64;
        while now < 100 * MS {
            now += MS;
            let d = s.on_event(Event::TimerFired(Timer::Pto), now);
            note(d, now, &mut sent);
        }
        let lost_seq = sent
            .iter()
            .find(|x| x.2 == LOST_UNIT)
            .map(|x| x.1)
            .expect("birim gönderildi");
        assert_eq!(
            sent.iter().filter(|x| x.2 == LOST_UNIT).count(),
            1,
            "kurtarılan birimin tek iletimi olmalı (yeniden iletim yok)"
        );
        // Alıcıya kayıp paket DIŞINDAKİ ilk iletimler ulaşır; ACK'ler gönderilir.
        let mut acks = Vec::new();
        for &(_, seq, unit) in sent.iter().filter(|(_, q, _)| *q != lost_seq) {
            let out = r.on_datagram(
                RxDatagram {
                    seq,
                    unit_id: unit,
                    kind: RxKind::Data,
                },
                now,
            );
            acks.extend(out.ack);
        }
        // Zamanlayıcı ACK'i: port alıcısı 25 ms'de, eski alıcı kadans tabanında (≈ 8,4 s) çıkarır.
        acks.extend(r.on_ack_timer(now + 30 * MS));
        now += 9 * SEC;
        acks.extend(r.on_ack_timer(now));
        assert!(!acks.is_empty());
        // Önce alıcının FEC kurtarma kanıtı (RECOVERED) gelir — ACK'lerden ÖNCE.
        let rtt_before = s.diag_rtt_sample_count();
        let fl_before = s.diag_inflight_seqs();
        let bytes_before = s.inflight_bytes();
        assert!(fl_before.iter().any(|e| e.0 == lost_seq));
        now += 50 * MS;
        let d = s.on_event(
            Event::RecoveredControl(RecoveredInfo {
                units: vec![RecoveredUnit {
                    unit_id: LOST_UNIT,
                    block_id: 0,
                    esi: LOST_UNIT as u16,
                }],
            }),
            now,
        );
        assert!(d.is_empty(), "RECOVERED gönderim üretmemeli: {d:?}");
        assert_eq!(
            s.take_confirmed_units(),
            vec![LOST_UNIT],
            "[{e:?}] RECOVERED birimi teyit etmeli"
        );
        assert_eq!(
            s.diag_rtt_sample_count(),
            rtt_before,
            "[{e:?}] RECOVERED RTT örneği üretmemeli"
        );
        assert!(
            s.diag_inflight_seqs().iter().any(|e| e.0 == lost_seq),
            "[{e:?}] RECOVERED düşen paketi (seq {lost_seq}) ACK gibi uçuştan çıkarmamalı"
        );
        assert_eq!(
            s.inflight_bytes(),
            bytes_before,
            "[{e:?}] RECOVERED uçuş baytını değiştirmemeli"
        );
        // Sonra ACK'ler: hiçbiri düşen paketin numarasını taşımaz.
        let expected = sent.iter().map(|x| x.1).max().expect("gönderildi") + 1;
        let mut confirmed_by_ack = BTreeSet::new();
        for f in acks {
            let f = AckFrame::decode(&f.encode()).expect("çöz");
            let acked = f.acked_full(expected);
            assert!(
                !acked.contains(&lost_seq),
                "[{e:?}] ACK düşen paketi teyit etti"
            );
            let d = s.on_event(
                Event::AckReceived(AckInfo {
                    acked_seqs: acked,
                    ack_delay: f.ack_delay_duration(),
                }),
                now,
            );
            note(d, now, &mut sent);
            confirmed_by_ack.extend(s.take_confirmed_units());
        }
        assert!(
            !confirmed_by_ack.contains(&LOST_UNIT),
            "[{e:?}] kurtarılan birim ACK'le yeniden teyit edilmemeli (paketi alınmadı)"
        );
        // ACK'le teyit beklenen: alıcıya bir iletimi ulaşmış her birim, kurtarılan hariç.
        let expect_others: BTreeSet<u64> = sent
            .iter()
            .filter(|x| x.1 != lost_seq && x.2 != LOST_UNIT && x.1 < expected)
            .map(|x| x.2)
            .collect();
        assert!(expect_others.len() >= 3, "vakum: ACK'lenecek birim az");
        assert_eq!(
            confirmed_by_ack, expect_others,
            "[{e:?}] diğer birimler ACK'le (gönderilen {sent:?})"
        );
        assert!(
            s.diag_rtt_sample_count() > rtt_before,
            "[{e:?}] ACK RTT örneği üretmeli"
        );
        // Ufuk: kurtarılan birim YENİDEN GÖNDERİLMEZ; düşen paket ACK'siz uçuştan çıkar (kayıp
        // ilanı ya da kararsız kopya bırakma), ACK yoluyla değil.
        let t_rec = now;
        while now < t_rec + 20 * SEC {
            now += MS;
            let d = s.on_event(Event::TimerFired(Timer::Pto), now);
            for x in &d {
                assert!(
                    x.unit_id != LOST_UNIT
                        || !matches!(x.kind, TransmitKind::Retransmit | TransmitKind::Duplicate),
                    "[{e:?}] FEC'le kurtarılan birim yeniden gönderildi: {x:?}"
                );
            }
            // Diğer birimlerin olası prob/yeniden iletimleri alıcıya ulaşır ve ACK'lenir.
            for x in d {
                if matches!(
                    x.kind,
                    TransmitKind::Data | TransmitKind::Retransmit | TransmitKind::Duplicate
                ) {
                    seq_unit.insert(x.seq, x.unit_id);
                    let out = r.on_datagram(
                        RxDatagram {
                            seq: x.seq,
                            unit_id: x.unit_id,
                            kind: RxKind::Data,
                        },
                        now,
                    );
                    if let Some(f) = out.ack {
                        let acked = f.acked_full(x.seq + 1);
                        assert!(!acked.contains(&lost_seq));
                        let _ = s.on_event(
                            Event::AckReceived(AckInfo {
                                acked_seqs: acked,
                                ack_delay: f.ack_delay_duration(),
                            }),
                            now,
                        );
                        let _ = s.take_confirmed_units();
                    }
                }
            }
            if !s.diag_inflight_seqs().iter().any(|e| e.0 == lost_seq) {
                break;
            }
        }
        assert!(
            !s.diag_inflight_seqs().iter().any(|e| e.0 == lost_seq),
            "[{e:?}] düşen paket 20 s'de uçuştan çıkmadı (uçuş muhasebesi sızıyor)"
        );
        let cancels = s.diag_confirmed_cancels();
        let (loss_pkt, loss_time, _) = s.diag_loss();
        println!(
            "ACK-REF fec motor={e:?} düşen_seq={lost_seq} kayıp_ilanı(pkt/zaman)={loss_pkt}/{loss_time} \
             kararsız_kopya={cancels:?}"
        );
    }
}
