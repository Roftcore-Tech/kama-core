//! K4G-05b — RTT örnek kabul kuralı ve tazelik ölçütü (madde 7; engine §6 v1.1 KABUL KURALI;
//! CEVAP PAKETİ 10 DUR-29 (a) dar okuma; Ek B.2 "Soğuk başlangıç (rev4)", "RTT tazelik ve
//! bağımsızlık"): kural birim testleri (çerçevenin en büyüğü / geri düşüş yok, doygun ack_delay,
//! tutma süresiyle şişmiş RAW), köken Duplicate (soğuk başlangıç C), soğuk başlangıç A (ilk kabul
//! sonlu sürede + yakınsama sınırı + iki koşu bit-özdeş), soğuk başlangıç B (doygun ilk ACK bayat
//! tohum bırakmaz), bağımsızlık (teslim / BW-örneği / delivered-kredi RTT kararından bağımsız),
//! tazelik sayacı birim testi ve 600 s kopya/FEC ağırlıklı tazelik (kırmızıysa DUR). Yol değişimi
//! (0.4 → 2 s) P0 `p0_rtt_path_change_converges` ile yeşil kalmalıdır (iddiası değişmedi).
//!
//! Her test başında amaç ve TABANDA (K4G-05b öncesi motor) beklenen sonuç [çıkarım; koşulmadı].
//!
//! Koş: `CARGO_BUILD_JOBS=2 cargo test -p adaptive-transport --test k4g05b_rtt -- --nocapture --test-threads=1`

mod k4g05b_support;

use adaptive_transport::ack::ACK_DELAY_UNIT_US;
use adaptive_transport::sender::{SenderState, TransmitDecision, TransmitKind};
use k4g05b_support::*;
use std::time::Duration;

/// V2 kodlama tavanı: 65535 × 256 µs ≈ 16.78 s (sender.rs `ACK_DELAY_SATURATED` ile aynı türetim).
fn saturated_ack_delay() -> Duration {
    Duration::from_micros(ACK_DELAY_UNIT_US * u64::from(u16::MAX))
}

fn srtt(d: &Drv) -> Option<u64> {
    d.s.path_timing_snapshot().srtt_ns
}

// ════════════════════════════════════════════════════════════════════════════════════════════
// Kural birim testleri
// ════════════════════════════════════════════════════════════════════════════════════════════

/// KABUL KURALI (a)/(b), dar okuma (DUR-29 (a)): çerçevenin EN BÜYÜK seq'i bu çerçeveyle ilk kez
/// onaylanmıyorsa örnek YOK — daha küçük yeni-onaylı kayda GERİ DÜŞÜLMEZ (ack_delay en büyüğündür).
/// Kurulum: u0 (seq 0) ve u1 (seq 1) t=0'da; 100 ms'de ACK [1] → örnek (100 ms); 300 ms'de ACK
/// [0, 1] → en büyük (1) zaten onaylı → örnek yok; u0 yine de teyit edilir (teslim RTT kararından
/// bağımsız).
///
/// TABANDA: KIRMIZI beklenir (v1 en büyük yeni-onaylı Data'ya — seq 0 — geri düşer: 300 ms örnek).
#[test]
fn k4g05b_rtt_rule_frame_max_no_fallback() {
    const TEST: &str = "k4g05b_rtt_rule_frame_max_no_fallback";
    let mut d = Drv::new(product_sender());
    d.app(0, 300);
    d.app(1, 300);
    let (Some(s0), Some(s1)) = (d.data_seq(0), d.data_seq(1)) else {
        panic!("{TEST}: FİKSTÜR GEÇERSİZ — u0/u1 t=0'da gönderilmedi");
    };
    d.now = 100 * MS;
    d.ack(vec![s1]);
    let samples_1 = d.s.diag_rtt_sample_count();
    let srtt_1 = srtt(&d);
    assert_eq!(
        (samples_1, srtt_1),
        (1, Some(100 * MS)),
        "{TEST}: ilk çerçevenin en büyüğü (seq {s1}) örnek vermeli"
    );
    d.now = 300 * MS;
    d.ack(vec![s0, s1]);
    assert_eq!(
        d.s.diag_rtt_sample_count(),
        samples_1,
        "{TEST}: en büyük seq zaten onaylı — seq {s0}'a geri düşülmemeli"
    );
    assert_eq!(srtt(&d), srtt_1, "{TEST}: srtt değişmemeli");
    assert!(
        d.w.confirmed_set.contains(&0),
        "{TEST}: u0 teyit edilmeli (teslim RTT kararından bağımsız)"
    );
}

/// KABUL KURALI (d) + soğuk başlangıç kol B (Ek B.2 "Soğuk başlangıç (rev4)"): doygun ack_delay
/// (V2 tavanı ≈ 16.78 s; alıcının tutma süresi bilinmez) örnek VERMEZ — srtt yokken de (ilk örnek)
/// aynen; bayat tohum oluşmaz (srtt `None`, PreConverge sürer, kural reddi sayacı 1); birim yine de
/// teyit edilir; sonraki doygun olmayan ilk ACK tohumu kendi gönderim anından kurar (150 ms tam).
/// Kurulum: u0 ve u1 t=0'da (ilk uçuş bütçesi ikisini de taşır); 100 ms'de YALNIZ u0 doygun
/// ack_delay'le ACK'lenir — u1 uçuşta kaldığından motor boşa düşmez (`update_state`'te Idle,
/// PreConverge'den önce değerlendirilir: tek birimli kurulumda teyit motoru boşaltıp durumu Idle
/// yapardı); 150 ms'de u1 sıfır ack_delay'le ACK'lenir.
///
/// TABANDA: KIRMIZI beklenir (kural (d) yok: doygun ACK düzeltme reddiyle RAW örnek olarak girer).
#[test]
fn k4g05b_rtt_rule_saturated_ack_delay_refused() {
    const TEST: &str = "k4g05b_rtt_rule_saturated_ack_delay_refused";
    let mut d = Drv::new(product_sender());
    d.app(0, 300);
    d.app(1, 300);
    let (Some(s0), Some(s1)) = (d.data_seq(0), d.data_seq(1)) else {
        panic!("{TEST}: FİKSTÜR GEÇERSİZ — u0/u1 t=0'da gönderilmedi");
    };
    d.now = 100 * MS;
    d.ack_with_delay(vec![s0], saturated_ack_delay());
    assert_eq!(
        srtt(&d),
        None,
        "{TEST}: doygun ack_delay bayat tohum bıraktı"
    );
    assert_eq!(
        d.s.state(),
        SenderState::PreConverge,
        "{TEST}: örnek yokken PreConverge sürmeli"
    );
    assert_eq!(
        d.s.diag_rtt_freshness().2,
        1,
        "{TEST}: kural reddi sayacı (diag_rtt_freshness().2) 1 olmalı"
    );
    assert!(
        d.w.confirmed_set.contains(&0),
        "{TEST}: u0 teyit edilmeli (teslim RTT kararından bağımsız)"
    );
    d.now = 150 * MS;
    d.ack(vec![s1]);
    assert_eq!(
        srtt(&d),
        Some(150 * MS),
        "{TEST}: ilk kabul edilen örnek kendi gönderim anından (150 ms)"
    );
}

/// KABUL KURALI (e): rtprop varken min-rtt guard düzeltmeyi reddettiyse `ack_delay < rtprop`
/// olmalıdır — aksi hâlde RAW tutma süresiyle en az rtprop kadar şişmiştir (FLOOR kodlama) → örnek
/// YOK. Kurulum: rtprop = 100 ms; (1) RAW 250 ms, ack_delay 200 ms → düzeltilmiş 50 ms < rtprop →
/// guard reddi; 200 ≥ 100 → kural reddi (örnek yok); (2) RAW 150 ms, ack_delay 60 ms → guard reddi;
/// 60 < 100 → RAW 150 ms örneği KABUL (srtt yükselir).
///
/// TABANDA: KIRMIZI beklenir (kural (e) yok: (1) RAW 250 ms örnek olarak girer).
#[test]
fn k4g05b_rtt_rule_raw_inflated_refused() {
    const TEST: &str = "k4g05b_rtt_rule_raw_inflated_refused";
    let mut d = Drv::new(product_sender());
    d.app(0, 300);
    let s0 = d.data_seq(0).unwrap_or_default();
    d.now = 100 * MS;
    d.ack(vec![s0]);
    let base = (
        d.s.diag_rtt_sample_count(),
        d.s.diag_rtt().0,
        d.s.diag_rtt_freshness().2,
    );
    d.now = 200 * MS;
    d.app(1, 300);
    let s1 = d.data_seq(1);
    d.now = 450 * MS;
    d.ack_with_delay(s1.into_iter().collect(), Duration::from_millis(200));
    let after_1 = (
        d.s.diag_rtt_sample_count(),
        d.s.diag_rtt().0,
        d.s.diag_rtt_freshness().2,
    );
    let srtt_1 = srtt(&d);
    d.now = 500 * MS;
    d.app(2, 300);
    let s2 = d.data_seq(2);
    d.now = 650 * MS;
    d.ack_with_delay(s2.into_iter().collect(), Duration::from_millis(60));
    let after_2 = (
        d.s.diag_rtt_sample_count(),
        d.s.diag_rtt().0,
        d.s.diag_rtt_freshness().2,
    );
    println!(
        "[RTT-e] (örnek, guard_reddi, kural_reddi): taban={base:?} (1)={after_1:?} (2)={after_2:?}"
    );
    if base.0 != 1 || s1.is_none() || s2.is_none() {
        panic!("{TEST}: FİKSTÜR GEÇERSİZ — rtprop kurulmadı ya da birimler gönderilmedi");
    }
    assert_eq!(
        after_1,
        (base.0, base.1 + 1, base.2 + 1),
        "{TEST}: (1) RAW şişmiş örnek reddedilmeli (guard reddi + kural reddi, örnek yok)"
    );
    assert_eq!(
        after_2,
        (after_1.0 + 1, after_1.1 + 1, after_1.2),
        "{TEST}: (2) ack_delay < rtprop iken RAW örnek kabul edilmeli"
    );
    assert!(
        srtt(&d) > srtt_1,
        "{TEST}: (2) RAW 150 ms örneği srtt'yi yükseltmeli"
    );
}

/// Köken Duplicate + soğuk başlangıç kol C: ilk çerçevenin en büyüğü DR-EST-1 kuruluş probunun
/// kopyasıdır (köken Duplicate, 500 ms'de) → örnek ONUN gönderim anından alınır: 700 − 500 = 200 ms
/// (ilk örnek → srtt tam 200 ms); orijinal Data'nın anından (0) alınsaydı 700 ms olurdu.
///
/// TABANDA: KIRMIZI beklenir (v1 yalnız Data kökenliyi örnekler: 700 ms).
#[test]
fn k4g05b_rtt_origin_duplicate_cold_start_c() {
    const TEST: &str = "k4g05b_rtt_origin_duplicate_cold_start_c";
    let mut d = Drv::new(product_sender());
    d.app(0, 1200);
    d.pump_to(500 * MS, 10 * MS);
    let s0 = d.data_seq(0);
    let dup = d
        .log
        .iter()
        .find(|e| e.d.kind == TransmitKind::Duplicate && e.d.unit_id == 0)
        .map(|e| (e.t, e.d.seq));
    let (Some(s0), Some((t_dup, s_dup))) = (s0, dup) else {
        panic!("{TEST}: FİKSTÜR GEÇERSİZ — Data ya da kuruluş probu kopyası yok: {s0:?} {dup:?}");
    };
    d.now = 700 * MS;
    d.ack(vec![s0, s_dup]);
    assert_eq!(
        srtt(&d),
        Some(700 * MS - t_dup),
        "{TEST}: örnek en büyük seq'in (kopya, {} ms) kendi gönderim anından alınmalı",
        t_dup / MS
    );
}

// ════════════════════════════════════════════════════════════════════════════════════════════
// Soğuk başlangıç A — kilitlenme yok + yakınsama sınırı + belirlenimlilik
// ════════════════════════════════════════════════════════════════════════════════════════════

const CS_RTT: u64 = 2 * ONE_WAY_NS;
const CS_N: u32 = 64;
const CS_HORIZON: u64 = 600 * SEC;

/// %30 belirlenimli ileri kayıp (seq mod 10 < 3; bütün türler).
fn cs_drop(d: &TransmitDecision) -> bool {
    d.seq % 10 < 3
}

/// Soğuk başlangıç A sonucu: (ilk kabul anı, ilk srtt, N'inci kabuldeki srtt, (örnek sayısı, srtt) izi).
type ColdStartA = (
    Option<u64>,
    Option<u64>,
    Option<u64>,
    Vec<(u32, Option<u64>)>,
);

/// Soğuk başlangıç A koşusu: RTT 400 ms (tek yön 200 ms, darboğaz yok, ürün pompası 1 ms → bütün
/// varışlar tik hizalı), 200 B / 100 ms, %30 belirlenimli kayıp, gerçek alıcı.
fn cold_start_a() -> ColdStartA {
    let mut sim = Sim::new(
        product_sender(),
        PACE_TICK_NS,
        Path::new(one_way_fixed, None),
        Path::new(one_way_fixed, None),
    );
    sim.drop_fwd = Box::new(cs_drop);
    let mut next_id: u64 = 0;
    let mut t_first: Option<u64> = None;
    let mut srtt_first: Option<u64> = None;
    let mut srtt_n: Option<u64> = None;
    let mut trace: Vec<(u32, Option<u64>)> = Vec::new();
    let mut last_count = 0u32;
    while sim.now < CS_HORIZON {
        if sim.now.is_multiple_of(100 * MS) {
            sim.app(next_id, 200);
            next_id += 1;
        }
        sim.timers();
        sim.advance();
        let count = sim.s.diag_rtt_sample_count();
        if count != last_count {
            last_count = count;
            let st = sim.s.path_timing_snapshot().srtt_ns;
            trace.push((count, st));
            if t_first.is_none() {
                t_first = Some(sim.now);
                srtt_first = st;
            }
            if count >= CS_N {
                srtt_n = st;
                break;
            }
        }
    }
    (t_first, srtt_first, srtt_n, trace)
}

/// Soğuk başlangıç kol A (Ek B.2 "Soğuk başlangıç (rev4)"; C9: srtt-yok istisnası kalktı, kural ilk
/// örnekte de aynen).
///
/// AMAÇ: ilk kabul SONLU sürede (≤ 600 s; kilitlenirse DUR) ve N = 64 kabulde RTT ≤ srtt ≤ RTT +
/// ⌈(7/8)^64·(srtt_r − RTT)⌉ + 256 µs (srtt_r = ilk kabuldeki srtt; 256 µs = V2 ack_delay FLOOR
/// birimi: düzeltilmiş örnek ∈ [RTT, RTT + 256 µs) — her kabul edilen örnek o iletimin kendi
/// gönderim anından); iki koşu bit-özdeş (RNG yok, ADR-17).
///
/// TABANDA: KIRMIZI beklenir (v1 en büyük olmayan Data'yı en büyüğün ack_delay'iyle örnekler →
/// şişmiş örnek, srtt sınırın üstünde).
#[test]
fn k4g05b_rtt_cold_start_a_converges_deterministic() {
    const TEST: &str = "k4g05b_rtt_cold_start_a_converges_deterministic";
    let a = cold_start_a();
    let b = cold_start_a();
    let (t_first, srtt_first, srtt_n, trace) = &a;
    println!(
        "[RTT-A] ilk kabul={:?} ms srtt_r={:?} µs srtt_{CS_N}={:?} µs; iz (örnek, srtt µs) ilk 8={:?}",
        t_first.map(|t| t / MS),
        srtt_first.map(|v| v / 1000),
        srtt_n.map(|v| v / 1000),
        trace
            .iter()
            .take(8)
            .map(|&(c, s)| (c, s.map(|v| v / 1000)))
            .collect::<Vec<_>>()
    );
    assert!(
        t_first.is_some(),
        "{TEST}: DUR — 600 s içinde hiç RTT örneği kabul edilmedi (kural ilk örneği kilitledi)"
    );
    let (Some(sr), Some(sn)) = (*srtt_first, *srtt_n) else {
        panic!(
            "{TEST}: {CS_N} kabul edilen örneğe ulaşılmadı (iz uzunluğu {})",
            trace.len()
        );
    };
    let decay = 0.875f64.powi(CS_N as i32);
    let slack = (decay * sr.abs_diff(CS_RTT) as f64).ceil() as u64;
    let upper = CS_RTT + slack + ACK_DELAY_UNIT_US * 1000;
    assert!(
        (CS_RTT..=upper).contains(&sn),
        "{TEST}: srtt_{CS_N} = {sn} ns ∉ [RTT {CS_RTT}, {upper}] (srtt_r {sr})"
    );
    assert_eq!(a, b, "{TEST}: iki koşu bit-özdeş olmalı (belirlenimlilik)");
}

// ════════════════════════════════════════════════════════════════════════════════════════════
// Bağımsızlık — teslim, BW-örneği, delivered-kredi RTT kararından bağımsız
// ════════════════════════════════════════════════════════════════════════════════════════════

/// Sender birim testlerinin kol-2 kalıbı (TUR-2c) kamu API'siyle: u(1200 B) ısınma, u42 = seq 1 ve
/// 9 küçük birim (seq 2..10); 30 ms'de ACK [2..10] → seq 1 paket eşiğiyle kayıp + aynı olayda ARQ.
/// Dönüş: (sürücü, retransmit seq'i).
fn karn_setup() -> (Drv, Option<u64>) {
    let mut d = Drv::new(product_sender());
    d.app(0, 1200);
    d.now = 10 * MS;
    d.ack(vec![0]);
    d.now = 20 * MS;
    d.app(42, 100);
    for i in 1..10 {
        d.app(i, 100);
    }
    d.now = 30 * MS;
    d.ack((2..=10).collect());
    let retx = d
        .log
        .iter()
        .find(|e| e.t == 30 * MS && e.d.kind == TransmitKind::Retransmit && e.d.unit_id == 42)
        .map(|e| e.d.seq);
    (d, retx)
}

/// Bir çerçevenin muhasebe izi (RTT dışı): teyit çıktısı, tombstone sayaçları, BtlBw örnekleri,
/// uçuş baytı, kayıp sayaçları.
type Ledger = (
    Vec<(u64, u64)>,
    (u64, u64, u64, u64, u64, u64),
    Vec<(u64, u64)>,
    u64,
    (u64, u64, u64),
);

fn ledger(d: &Drv) -> Ledger {
    (
        d.w.confirmed.clone(),
        d.s.diag_tombstone(),
        d.s.diag_btlbw_samples(),
        d.s.inflight_bytes(),
        d.s.diag_loss(),
    )
}

/// Bağımsızlık (Ek B.2 "RTT tazelik ve bağımsızlık"): aynı olay dizisi iki motorda; YALNIZ test
/// çerçevesinin ack_delay'i farklı (A: 0 → örnek kabul; B: doygun → kural (d) reddi). Kol 1: kol-2
/// geç orijinal ACK'i (tombstone: delivered-kredi, rate-örneği yok); kol 2: retransmit'in kendi
/// ACK'i (uçuş: BW-örneği). AMAÇ: her iki kolda teyit çıktısı, tombstone sayaçları (hit, ambig,
/// bw_rec, kredi), BtlBw örnekleri, uçuş baytı ve kayıp sayaçları A ile B'de AYNI; yalnız RTT
/// örnek sayısı farklı (A = B + 1).
///
/// TABANDA: örnek-sayısı iddiası KIRMIZI beklenir (kural (d) yok: B de örnekler; kol 1'de v1 kol-2
/// RTT almaz — A da örneklemez); muhasebe eşitliği tabanda da yeşil.
#[test]
fn k4g05b_rtt_independence_delivery_bw_credit() {
    const TEST: &str = "k4g05b_rtt_independence_delivery_bw_credit";
    for arm in ["kol-2 geç orijinal", "retransmit uçuş"] {
        let run = |delay: Duration| -> (Drv, Option<u64>) {
            let (mut d, retx) = karn_setup();
            if arm == "kol-2 geç orijinal" {
                d.now = 400 * MS;
                d.ack_with_delay(vec![1], delay);
            } else if let Some(r) = retx {
                d.now = 60 * MS;
                d.ack_with_delay(vec![r], delay);
            }
            (d, retx)
        };
        let (a, retx) = run(Duration::ZERO);
        let (b, _) = run(saturated_ack_delay());
        assert!(
            retx.is_some(),
            "{TEST}: FİKSTÜR GEÇERSİZ — 30 ms'de seq 1 için retransmit çıkmadı"
        );
        let (la, lb) = (ledger(&a), ledger(&b));
        println!(
            "[RTT-bağımsızlık {arm}] örnek A={} B={}; tombstone A={:?} B={:?}",
            a.s.diag_rtt_sample_count(),
            b.s.diag_rtt_sample_count(),
            la.1,
            lb.1
        );
        assert_eq!(
            la, lb,
            "{TEST} [{arm}]: teslim/BW/kredi/kayıp muhasebesi RTT kararından bağımsız olmalı"
        );
        assert_eq!(
            a.s.diag_rtt_sample_count(),
            b.s.diag_rtt_sample_count() + 1,
            "{TEST} [{arm}]: yalnız RTT kararı farklı olmalı (A kabul, B doygun ack_delay reddi)"
        );
    }
}

// ════════════════════════════════════════════════════════════════════════════════════════════
// Tazelik ölçütü
// ════════════════════════════════════════════════════════════════════════════════════════════

/// Tazelik ölçütü sayacı (madde 7 TAZELİK; Ek B.2 "sayaç doğruluğu ayrı birim testinde (yapay
/// donma)"). Sınır = max(10 × srtt, RTO_MAX) = 30 s (srtt 100 ms). AMAÇ: son kabulden 29 s sonra
/// `rtt_stale` = 0; 31 s sonra TAM 1; donma sürerken (70 s) yine 1 (dönem başına bir kez); yeni
/// kabul edilen örnek dönemi kapatır (son kabul anı güncellenir); ikinci donma 31 s sonra 2. Sayaç
/// yalnız gözlemdir.
///
/// TABANDA: KIRMIZI beklenir (sayaç yok — derlenmez).
#[test]
fn k4g05b_rtt_stale_counter_once_per_episode() {
    const TEST: &str = "k4g05b_rtt_stale_counter_once_per_episode";
    const STEP: u64 = 100 * MS;
    let mut d = Drv::new(product_sender());
    d.app(0, 300);
    let s0 = d.data_seq(0);
    d.now = 100 * MS;
    d.ack(s0.into_iter().collect());
    let t_sample = d.now;
    assert_eq!(
        d.s.rtt_staleness_limit_ns(),
        Some(rto_max_ns()),
        "{TEST}: sınır max(10 × srtt, RTO_MAX) = RTO_MAX olmalı (srtt 100 ms)"
    );
    d.app(1, 300); // donma boyunca uçuşta kalır (ACK'lenmez)
    d.pump_to(t_sample + 29 * SEC, STEP);
    let at_29 = d.s.diag_rtt_freshness().1;
    d.pump_to(t_sample + 31 * SEC, STEP);
    let at_31 = d.s.diag_rtt_freshness().1;
    d.pump_to(t_sample + 70 * SEC, STEP);
    let at_70 = d.s.diag_rtt_freshness().1;
    let t2 = d.now;
    d.app(2, 300);
    let s2 = d.data_seq(2);
    d.now = t2 + 100 * MS;
    d.ack(s2.into_iter().collect());
    let last_after = d.s.diag_rtt_freshness().0;
    let t_sample2 = d.now;
    d.pump_to(t_sample2 + 31 * SEC, STEP);
    let at_second = d.s.diag_rtt_freshness().1;
    println!(
        "[RTT-tazelik] rtt_stale: 29 s={at_29} 31 s={at_31} 70 s={at_70}; yeni örnek={:?} ms; ikinci donma={at_second}",
        last_after.map(|t| t / MS)
    );
    assert_eq!(at_29, 0, "{TEST}: sınırdan önce rtt_stale artmamalı");
    assert_eq!(
        at_31, 1,
        "{TEST}: sınır aşılınca rtt_stale tam bir kez artmalı"
    );
    assert_eq!(
        at_70, 1,
        "{TEST}: aynı donma döneminde ikinci artış olmamalı"
    );
    assert_eq!(
        last_after,
        Some(t_sample2),
        "{TEST}: yeni kabul edilen örnek son kabul anını güncellemeli"
    );
    assert_eq!(
        at_second, 2,
        "{TEST}: ikinci donma dönemi rtt_stale'i bir kez daha artırmalı"
    );
}

/// Dönüş yolu 20 s'de bir 1 s açık (ACK sessizliği → RECOVER_FWD).
fn ret_window(t: u64) -> bool {
    (t / SEC).is_multiple_of(20)
}

/// Kopya/FEC ağırlıklı 600 s tazelik (Ek B.2 "RTT tazelik ve bağımsızlık" — KIRMIZIYSA DUR: kural bu
/// trafikte örnek akışını kesiyor, sahip kararı).
///
/// Kurulum: ürün + statik-r FEC (her 8 DATA'da blok kapanışı, r = 4), 200 B / 100 ms, %30
/// belirlenimli ileri kayıp, dönüş yolu 20 s'de bir yalnız 1 s açık → aralarda ACK sessizliği H'yi
/// aşar → RECOVER_FWD (DATA başına kopya); tek yön 200 ms; 600 s.
///
/// AMAÇ: her tikte son kabul edilen örneğin yaşı ≤ max(10 × srtt, RTO_MAX) (ilk kabulden sonra) VE
/// koşu sonunda `rtt_stale` = 0. Fikstür: RECOVER_FWD'ye girildi; parite ve kopya gönderildi.
///
/// TABANDA: derlenmez (tazelik erişimcileri yok); davranışça v1 Data dışı kökeni örneklemediğinden
/// kopya ağırlıklı trafikte örnek akışı seyrekleşir [çıkarım].
#[test]
fn k4g05b_rtt_freshness_copy_fec_heavy_600s() {
    const TEST: &str = "k4g05b_rtt_freshness_copy_fec_heavy_600s";
    const STEP: u64 = 10 * MS;
    const HORIZON: u64 = 600 * SEC;
    let mut sim = Sim::new(
        product_sender_static_r(),
        STEP,
        Path::new(one_way_fixed, None),
        Path::new(one_way_fixed, None),
    );
    sim.drop_fwd = Box::new(cs_drop);
    sim.ret_open = Some(ret_window);
    let mut next_id: u64 = 0;
    let mut block: u32 = 0;
    let mut stale_age: Vec<(u64, u64, u64)> = Vec::new();
    let mut max_age = 0u64;
    while sim.now < HORIZON {
        if sim.now.is_multiple_of(100 * MS) {
            sim.app(next_id, 200);
            next_id += 1;
            if next_id.is_multiple_of(FEC_K) {
                let auth = sim.s.open_fec_block_authority();
                let _ = sim.s.close_fec_block_manual(auth, block, FEC_R, 200);
                block += 1;
            }
        }
        sim.timers();
        sim.advance();
        let (last, _, _) = sim.s.diag_rtt_freshness();
        if let (Some(l), Some(lim)) = (last, sim.s.rtt_staleness_limit_ns()) {
            let age = sim.now.saturating_sub(l);
            max_age = max_age.max(age);
            if age > lim {
                stale_age.push((sim.now, age, lim));
            }
        }
    }
    let w = &sim.w;
    let fwd = w.first_state_at_or_after(SenderState::RecoverFwd, 0);
    let parity = sim
        .log
        .iter()
        .filter(|e| e.d.kind == TransmitKind::FecParity)
        .count();
    let dups = sim
        .log
        .iter()
        .filter(|e| e.d.kind == TransmitKind::Duplicate)
        .count();
    let (_, rtt_stale, refused) = sim.s.diag_rtt_freshness();
    println!(
        "[RTT-600s] RECOVER_FWD ilk={:?} ms; parite={parity} kopya={dups}; örnek={} kural_reddi={refused}; \
         azami yaş={} ms; rtt_stale={rtt_stale}",
        fwd.map(|t| t / MS),
        sim.s.diag_rtt_sample_count(),
        max_age / MS
    );
    let mut fixture: Vec<String> = Vec::new();
    if fwd.is_none() || parity == 0 || dups == 0 {
        fixture.push(format!(
            "kopya/FEC ağırlıklı rejim kurulmadı (RECOVER_FWD={fwd:?}, parite={parity}, kopya={dups})"
        ));
    }
    assert!(
        fixture.is_empty(),
        "{TEST}: FİKSTÜR GEÇERSİZ:\n{}",
        fixture.join("\n")
    );
    assert!(
        stale_age.is_empty(),
        "{TEST}: DUR — son kabul edilen örneğin yaşı sınırı aştı (an ms, yaş ms, sınır ms): {:?}",
        stale_age
            .iter()
            .take(8)
            .map(|&(t, a, l)| (t / MS, a / MS, l / MS))
            .collect::<Vec<_>>()
    );
    assert_eq!(
        rtt_stale, 0,
        "{TEST}: DUR — rtt_stale > 0 (kabul kuralı bu trafikte örnek akışını kesiyor)"
    );
}
