//! K4G-05b — zamanlama testleri (K4G-05b notu §6 "05b testleri", Ek B.2): BURST (B1) tur başı
//! tabanı kolları (karışık boy, 2·MTU + 1 ARQ başı, bekleyen prob hedefi + aynı tur, boş kuyruk
//! kontrolü, PreConverge v1 kontrolü), PAY sınıflar arası kopya payı + DUR-31 rezervasyonu (karışık
//! boy; CEVAP PAKETİ 10 DUR-31 bağlayıcı okuması: iki iddia ayrı), DUR-41 kara delik birimi kopya
//! zinciri sıklığı (PAY düzeni, rampa, aralıklı trafik + kuyruk-probu), madde 6 sıfırlama. P0 BURST/PAY
//! (eşit boy) kolları `k4g_p0_engine.rs`'de iddiaları gevşetilmeden kalır.
//!
//! Her test başında amaç ve TABANDA (K4G-05b öncesi motor) beklenen sonuç [çıkarım; koşulmadı].
//!
//! Koş: `CARGO_BUILD_JOBS=2 cargo test -p adaptive-transport --test k4g05b_scheduling -- --nocapture --test-threads=1`

mod k4g05b_support;

use adaptive_transport::emergency::HysteresisConfig;
use adaptive_transport::pacer::DEFAULT_BURST_CAP;
use adaptive_transport::rate::DEFAULT_MTU;
use adaptive_transport::sender::{SenderState, TransmitDecision, TransmitKind};
use k4g05b_support::*;

fn assert_fixture(test: &str, fixture: &[String]) {
    assert!(
        fixture.is_empty(),
        "{test}: FİKSTÜR GEÇERSİZ (sınanacak durum kurulmadı; sonuç kırmızı SAYILMAZ):\n{}",
        fixture.join("\n")
    );
}

// ════════════════════════════════════════════════════════════════════════════════════════════
// BURST (B1) — tur başı BURST_CAP tabanı (engine §13 v1.1)
// ════════════════════════════════════════════════════════════════════════════════════════════

/// P0 BURST modeli: darboğaz 1064 B/s, tek yön 200 ms, 0-10 s ısınma 100 B / 100 ms, sonra küçük
/// DATA / 200 ms; `B_T_V`'de V girer.
const B_LINK_BPS: u64 = 1064;
const B_WARM_UNIT: u64 = 100;
const B_WARM_END: u64 = 10 * SEC;
const B_GAP: u64 = 200 * MS;
const B_T_V: u64 = 20 * SEC;
const B_SMALL: u64 = 37;
/// 2·MTU + 1: v1 tavanının (`max(tx başı, 2·MTU)`) bir bayt üstü.
const B_OVER_2MTU: u64 = 2 * DEFAULT_MTU + 1;

#[derive(Clone, Copy)]
struct BurstArm {
    /// V'nin boyu.
    big: u64,
    /// k'inci küçük DATA'nın boyu.
    small: fn(u64) -> u64,
    /// true: V'den sonra DATA yok (V uçbirim kuyruğu) ve V'nin BÜTÜN iletimleri düşer; false:
    /// küçük DATA sürer, V'nin YALNIZ ilk (Data) iletimi düşer.
    terminal: bool,
    horizon: u64,
}

struct BurstOut {
    sim: Sim,
    v: u64,
    t_v_sent: Option<u64>,
    /// V'nin yeniden emisyonları (an, tür).
    reemit: Vec<(u64, TransmitKind)>,
    /// V'nin kopyası vadeliyken (bu adımda yeni kurulmamış) BURST_CAP < V boyu gözlemi (an, cap).
    cap_below_due_copy: Vec<(u64, u64)>,
    /// V gönderildikten sonra birikimli kayıp-ilanı sayacı arttı mı.
    declared: bool,
    rate_at_v: u64,
}

fn small_37(_k: u64) -> u64 {
    B_SMALL
}

/// Karışık boy: 37 B ve 150 B dönüşümlü (sunulan ≈ 467 B/s < darboğaz).
fn small_mixed(k: u64) -> u64 {
    if k.is_multiple_of(2) {
        B_SMALL
    } else {
        150
    }
}

/// Ürün pompası 1 ms (P0 BURST gerekçesi: kırpma her olayda yeniden hesaplanır).
fn burst_run(arm: BurstArm) -> BurstOut {
    let mut sim = Sim::new(
        product_sender(),
        PACE_TICK_NS,
        Path::new(one_way_fixed, Some(B_LINK_BPS)),
        Path::new(one_way_fixed, None),
    );
    let mut next_id: u64 = 0;
    let mut k_small: u64 = 0;
    let mut v: Option<u64> = None;
    let mut loss_base: Option<u64> = None;
    let mut declared = false;
    let mut rate_at_v = 0;
    let mut cap_below_due_copy: Vec<(u64, u64)> = Vec::new();
    let mut due_prev: Option<u64> = None;
    loop {
        let now = sim.now;
        if now == B_T_V {
            let id = next_id;
            next_id += 1;
            let terminal = arm.terminal;
            sim.drop_fwd = Box::new(move |d: &TransmitDecision| {
                d.unit_id == id
                    && if terminal {
                        carries_unit(d.kind)
                    } else {
                        d.kind == TransmitKind::Data
                    }
            });
            rate_at_v = sim.s.diag_pacer().0;
            sim.app(id, arm.big);
            v = Some(id);
        }
        let data_on = !arm.terminal || now < B_T_V;
        let gap = if now < B_WARM_END { 100 * MS } else { B_GAP };
        if data_on && now.is_multiple_of(gap) {
            let size = if now < B_WARM_END {
                B_WARM_UNIT
            } else {
                let s = (arm.small)(k_small);
                k_small += 1;
                s
            };
            sim.app(next_id, size);
            next_id += 1;
        }
        sim.timers();
        if let Some(id) = v {
            let (pkt, time, _) = sim.s.diag_loss();
            let sent = data_seq(&sim.log, id).is_some();
            if sent {
                let base = *loss_base.get_or_insert(pkt + time);
                declared |= pkt + time > base;
            }
            let due = sim.s.diag_unit_recovery(id).dup_due_ns;
            let cap = sim.s.diag_pacer().2;
            if due.is_some_and(|d| d <= now) && due_prev.is_some() && cap < arm.big {
                cap_below_due_copy.push((now, cap));
            }
            due_prev = due;
        }
        if now >= arm.horizon {
            break;
        }
        sim.advance();
    }
    let v = v.unwrap_or_default();
    let t_v_sent = sim
        .log
        .iter()
        .find(|e| e.d.unit_id == v && e.d.kind == TransmitKind::Data && e.t >= B_T_V)
        .map(|e| e.t);
    let reemit: Vec<(u64, TransmitKind)> = sim
        .log
        .iter()
        .filter(|e| e.t >= B_T_V && e.d.unit_id == v && is_reemission(e.d.kind))
        .map(|e| (e.t, e.d.kind))
        .collect();
    BurstOut {
        sim,
        v,
        t_v_sent,
        reemit,
        cap_below_due_copy,
        declared,
        rate_at_v,
    }
}

fn burst_report(tag: &str, out: &BurstOut) {
    println!(
        "[{tag}] V=u{} ilk Data={:?} ms; kayıp ilanı={}; V'de hız={} B/s; yeniden emisyon ({}): {:?}; \
         vadeli kopyada cap<V={}",
        out.v,
        out.t_v_sent.map(|t| t / MS),
        out.declared,
        out.rate_at_v,
        out.reemit.len(),
        out.reemit
            .iter()
            .take(8)
            .map(|&(t, k)| (t / MS, k))
            .collect::<Vec<_>>(),
        out.cap_below_due_copy.len()
    );
}

/// BURST — karışık boy DATA (37/150 B) + 1050 B ARQ başı (Ek B.2 "BURST (B1) kolları").
///
/// AMAÇ: tur başı taban ARQ planı başını kapsar → token 1050 B'ye birikir ve V ARQ ile yeniden
/// gönderilir (kayıt token birikimiyle gönderilir — üç kabul koşulu (iii)). Fikstür: V'nin ilk
/// Data'sı çıktı ve V kayıp ilan edildi.
///
/// TABANDA: KIRMIZI beklenir (cap = clamp(hız × 40 ms, tx başı, …) ≤ 150 B < 1050 B; P0 BURST
/// sınıfı).
#[test]
fn k4g05b_burst_mixed_size_arq_head() {
    const TEST: &str = "k4g05b_burst_mixed_size_arq_head";
    let out = burst_run(BurstArm {
        big: 1050,
        small: small_mixed,
        terminal: false,
        horizon: B_T_V + 60 * SEC,
    });
    burst_report("BURST-karışık", &out);
    let mut fixture: Vec<String> = Vec::new();
    if out.t_v_sent.is_none() || !out.declared {
        fixture.push(format!(
            "V'nin ilk Data'sı çıkmadı ya da V kayıp ilan edilmedi (ilk={:?}, ilan={})",
            out.t_v_sent, out.declared
        ));
    }
    assert_fixture(TEST, &fixture);
    assert!(
        !out.reemit.is_empty(),
        "{TEST}: 1050 B ARQ başı 60 s boyunca yeniden gönderilmedi (tur başı taban ARQ başını kapsamalı)"
    );
}

/// BURST — 2·MTU + 1 (2401 B) ARQ başı, 37 B DATA akışı.
///
/// AMAÇ: taban v1 tavanını (max(tx başı, 2·MTU) = 2400 B) aşan kaydı da kapsar → V ARQ ile yeniden
/// gönderilir (token üretilmez; yalnız birikime izin). TABANDA: KIRMIZI beklenir (cap ≤ 2400 B <
/// 2401 B: `try_send` hiç geçmez, uyanma yok).
#[test]
fn k4g05b_burst_over_2mtu_arq_head() {
    const TEST: &str = "k4g05b_burst_over_2mtu_arq_head";
    let out = burst_run(BurstArm {
        big: B_OVER_2MTU,
        small: small_37,
        terminal: false,
        horizon: B_T_V + 60 * SEC,
    });
    burst_report("BURST-2MTU+1", &out);
    let mut fixture: Vec<String> = Vec::new();
    if out.t_v_sent.is_none() || !out.declared {
        fixture.push(format!(
            "V'nin ilk Data'sı çıkmadı ya da V kayıp ilan edilmedi (ilk={:?}, ilan={})",
            out.t_v_sent, out.declared
        ));
    }
    assert_fixture(TEST, &fixture);
    assert!(
        !out.reemit.is_empty(),
        "{TEST}: 2401 B ARQ başı 60 s boyunca yeniden gönderilmedi"
    );
}

/// BURST — bekleyen kuyruk-probu hedefi (2401 B, uçbirim kuyruğu; V'den sonra DATA yok, V'nin
/// bütün iletimleri düşer) + "aynı tur" kuralı.
///
/// AMAÇ: taban bekleyen prob hedefini kapsar → üç prob da gerçekten çıkar (bütçe tükenir) ve V
/// devirle kopya kuyruğuna geçer; kopya kaydı devir turunda oluşur, SONRAKİ turların tabanına girer:
/// kopya vadeliyken (yeni kurulduğu adım dışında) BURST_CAP ≥ V boyu; devirden sonra kopya çıkar.
///
/// TABANDA: KIRMIZI beklenir (boş kuyrukta cap = clamp(…, DEFAULT_MTU, 2·MTU) ≤ 2400 B < 2401 B:
/// prob hiç gönderilemez, hak harcanmaz; D1 de devreye girmez).
#[test]
fn k4g05b_burst_probe_target_and_same_turn() {
    const TEST: &str = "k4g05b_burst_probe_target_and_same_turn";
    let out = burst_run(BurstArm {
        big: B_OVER_2MTU,
        small: small_37,
        terminal: true,
        horizon: B_T_V + 60 * SEC,
    });
    burst_report("BURST-prob", &out);
    let mut fixture: Vec<String> = Vec::new();
    if out.t_v_sent.is_none() {
        fixture.push("V'nin ilk Data'sı çıkmadı".to_string());
    }
    assert_fixture(TEST, &fixture);
    let probes = out.sim.s.tail_probe_diag_ext().1;
    let copies = out
        .reemit
        .iter()
        .filter(|&&(_, k)| k == TransmitKind::Duplicate)
        .count();
    assert_eq!(
        probes,
        probe_budget(),
        "{TEST}: 2401 B prob hedefi için bütün prob hakları gerçek emisyonla harcanmalı"
    );
    assert!(
        copies >= 1,
        "{TEST}: devirden sonra 2401 B kopya gönderilmedi"
    );
    assert!(
        out.cap_below_due_copy.is_empty(),
        "{TEST}: vadeli kopya başı tur başı tabana girmedi (an ms, cap): {:?}",
        out.cap_below_due_copy
            .iter()
            .take(8)
            .map(|&(t, c)| (t / MS, c))
            .collect::<Vec<_>>()
    );
}

/// BURST — KONTROL: boş kuyruk (taban DEFAULT_MTU, v1). Isınma fazında (100 B / 100 ms, kayıp yok)
/// tx kuyruğu boşken hesaplanan her tur için BURST_CAP = clamp(hız × 40 ms, DEFAULT_MTU, 2·MTU)
/// (v1 ifadesi; yol başları ≤ DATA başı olduğundan ifade v1 ile birebir aynı kalır).
///
/// TABANDA: YEŞİL beklenir (kontrol).
#[test]
fn k4g05b_burst_empty_queue_floor_default_mtu() {
    const TEST: &str = "k4g05b_burst_empty_queue_floor_default_mtu";
    let mut sim = Sim::new(
        product_sender(),
        PACE_TICK_NS,
        Path::new(one_way_fixed, Some(B_LINK_BPS)),
        Path::new(one_way_fixed, None),
    );
    sim.w.turns = Some(Vec::new());
    let mut next_id: u64 = 0;
    while sim.now < B_WARM_END {
        if sim.now.is_multiple_of(100 * MS) {
            sim.app(next_id, B_WARM_UNIT);
            next_id += 1;
        }
        sim.timers();
        sim.advance();
    }
    let turns = sim.w.turns.take().unwrap_or_default();
    let mut checked = 0u64;
    let mut bad: Vec<(u64, u64, u64)> = Vec::new();
    for pair in turns.windows(2) {
        let (prev, cur) = (&pair[0], &pair[1]);
        if cur.is_app || prev.backlog_after != 0 || cur.state_after == SenderState::PreConverge {
            continue;
        }
        let window = cur.rate_after.saturating_mul(BURST_WINDOW_NS) / SEC;
        let expected = window.clamp(DEFAULT_MTU, 2 * DEFAULT_MTU);
        checked += 1;
        if cur.cap_after != expected {
            bad.push((cur.t / MS, cur.cap_after, expected));
        }
    }
    println!(
        "[BURST-boş] denetlenen tur={checked}; uyuşmayan={}",
        bad.len()
    );
    assert!(
        checked >= 100,
        "{TEST}: FİKSTÜR GEÇERSİZ — boş kuyruklu tur yok ({checked})"
    );
    assert!(
        bad.is_empty(),
        "{TEST}: boş kuyrukta cap v1 ifadesinden ayrıştı (ms, cap, beklenen): {:?}",
        bad.iter().take(8).collect::<Vec<_>>()
    );
}

/// BURST — KONTROL: PreConverge v1 (engine §13 v1.1: tur başı tabanı PRE_CONVERGE DIŞINDA; 7-EK B1
/// beyanlı istisna). Ölçüm yokken (ACK yok) 2401 B birim gönderildikten sonra tx kuyruğu boşken
/// hesaplanan her turda BURST_CAP = max(DEFAULT_MTU, iw) = 2400 B — vadeli/yakın kopya başı (2401 B)
/// tabana GİRMEZ (v1); kopya kuruluş probuyla (token-muaf) çıkar.
///
/// TABANDA: YEŞİL beklenir (kontrol).
#[test]
fn k4g05b_burst_preconverge_v1_control() {
    const TEST: &str = "k4g05b_burst_preconverge_v1_control";
    let mut d = Drv::new(product_sender());
    d.w.turns = Some(Vec::new());
    d.app(0, B_OVER_2MTU);
    d.pump_to(130 * SEC, 10 * MS);
    let turns = d.w.turns.take().unwrap_or_default();
    let iw = DEFAULT_BURST_CAP;
    let mut checked = 0u64;
    let mut bad: Vec<(u64, u64)> = Vec::new();
    for pair in turns.windows(2) {
        let (prev, cur) = (&pair[0], &pair[1]);
        if cur.is_app || prev.backlog_after != 0 || cur.state_after != SenderState::PreConverge {
            continue;
        }
        checked += 1;
        if cur.cap_after != DEFAULT_MTU.max(iw) {
            bad.push((cur.t / MS, cur.cap_after));
        }
    }
    let dups = d
        .log
        .iter()
        .filter(|e| e.d.kind == TransmitKind::Duplicate)
        .count();
    println!(
        "[BURST-PreConverge] denetlenen tur={checked}; uyuşmayan={}; kuruluş probu kopyası={dups}",
        bad.len()
    );
    assert!(
        checked >= 100 && d.s.state() == SenderState::PreConverge,
        "{TEST}: FİKSTÜR GEÇERSİZ — PreConverge'de boş kuyruklu tur yok ({checked}) ya da PreConverge'den çıkıldı"
    );
    assert!(
        bad.is_empty(),
        "{TEST}: PreConverge'de cap v1 değerinden (2400 B) ayrıştı (ms, cap): {:?}",
        bad.iter().take(8).collect::<Vec<_>>()
    );
}

// ════════════════════════════════════════════════════════════════════════════════════════════
// PAY — sınıflar arası kopya payı + DUR-31 rezervasyonu (karışık boy)
// ════════════════════════════════════════════════════════════════════════════════════════════

const P_LINK_BPS: u64 = 1064;
const P_U: u64 = 4;
const P_U_SIZE: u64 = 1050;
const P_DATA: u64 = 37;
const P_BACKLOG: u64 = 8000;

/// PAY kolu tek yön yayılımı: ilk ACK kuruluş probundan (0.5 s) önce gelsin diye 100 ms.
fn one_way_100ms(_t: u64) -> u64 {
    100 * MS
}

/// PAY + DUR-31 — 37 B DATA seli + 1050 B vadeli kopya başı (CEVAP PAKETİ 10 DUR-31 bağlayıcı
/// okuması; Ek B.2 "PAY ve sıfırlama"; engine §13 v1.1 İSTİSNA).
///
/// Kurulum: darboğaz 1064 B/s, tek yön 100 ms, ürün pompası 1 ms. U (1050 B) PreConverge'de
/// ısınmayla gider (§3.3-B geometrik kopya; PRE_CONVERGE çıkışında vade ≤ çıkış + RTO_MAX), U'nun
/// bütün iletimleri düşer. İlk ACK'te 8000 × 37 B DATA birikimi girer (pace'i doldurur; tur başı
/// kredi ≈ hız × 1 ms ≪ 1050 B). Tur = `schedule_transmit` çağrısı (DUR-24 (a)); j = 0 kopya
/// vadesinden sonraki ilk tur.
///
/// İDDİA 1 (hizmetin başlaması ≤ K_ARQ_SHARE tur): j ≥ K_ARQ_SHARE olan hiçbir turda, U yeniden
/// gönderilene dek DATA emisyonu yoktur (pay turu j < K_ARQ_SHARE'dir; rezervasyon orada başlar;
/// CEVAP 11/12 DUR-31: DATA, ARQ ve prob rezervin ALTINA inemez — bu düzende token rezerv + 37 B'ye
/// hiç ulaşmadan baş gider). İDDİA 2 (rezervasyon → emisyon arası DATA yok): rezervasyonun
/// gözlendiği ilk tur j* (DATA yok, birikim var, tur sonu token ≥ DATA boyu) ile U'nun yeniden
/// emisyon turu arasında DATA emisyonu yoktur; emisyon turunda U'nun kararı her DATA'dan önce
/// gelir. İDDİA 3: U yeniden gönderilir (token yetince; ARQ payı aynı turdaysa ARQ kopyasıyla —
/// U'nun yeniden emisyonu sayılır).
///
/// TABANDA: KIRMIZI beklenir (kopya drenajı DATA'dan sonra, ilk token reddinde durur; pay yok).
///
/// [K4G-05b rev6 · DUR-8 biçimi, FİKSTÜR + İDDİA 1; CEVAP PAKETİ 15 v3 K1 (c), K3] NEDEN DEĞİŞTİ
/// (teşhis; ölçüm `kod-run/r5` 37b78aad ve `kod-run/r6` 33938987): ölçüm penceresi bayattı. Pencere
/// başı, selin olduğu anda okunan vadeydi (PRE_CONVERGE çıkışı + RTO_MAX = 30.3 s). v1.1'de U'nun
/// ARQ'su sel altında 37 B DATA tarafından aç bırakılıyordu (v1 ARQ↔DATA boy ters dönmesi) ve U'nun
/// kopyası o vadede kalıyordu. rev6'da ARQ payı da rezerv alır (K3): U'nun ARQ yeniden gönderimleri
/// selde de çıkar (ölçüldü: 0.755 / 8.028 / 26.553 s, hepsi düşer). ARQ_RETX_CAP'te birim geometrik
/// sahibine devredilir ve vadesi şimdiye çekilir (madde 3, `copy_ensure_geometric(pull)`; ölçülen
/// gerçek vade 29.196 s — seldeki okumadan ÖNCE). U'nun kopyası 30.013 s'de, vadeden 0.817 s sonra
/// çıktı; arada 0 DATA. Eski pencere 30.3 s'de başladığından bu kopyayı dışarıda bıraktı. Eski test
/// 32.814 s'deki SONRAKİ kopyayı yakaladı; o kopyayı kopya kaybının later-ACK çekmesi üretir. Aradaki
/// 62 DATA turu, U'nun vadeli kopya kaydı yokken çıkan DATA'dır. YENİ FİKSTÜR: pencere başı = selden
/// sonra U'nun kopya kaydının, U'nun ARQ işi yokken vadesine İLK ulaştığı an (her adımda okunur).
/// YENİ İDDİA 1 (K1 niteleyicisi "karşılık hakkı yokken" + A.4 rev6 (iii)): vadeden U'nun yeniden
/// emisyonuna dek en çok iki DATA emisyonu (Ek B.6 (h.3) T_max(kopya) dizisi: DATA hak sahibi → ARQ →
/// DATA hak sahibi → kopya); U vadeden sonra ≤ (2·37 + 1050 + 1050) B / pace_min + (K_ARQ_SHARE + 4)
/// tur içinde yeniden gönderilir (pace_min = penceredeki en düşük tur sonu hızı; ölçüldü: 817 ms ≤
/// 2711 ms). İDDİA 2 ve 3 aynen. 2×2: eski kod + eski test YEŞİL · yeni kod + eski test KIRMIZI
/// (ölçüldü, r5: 62 DATA turu, bayat pencere) · yeni kod + yeni test YEŞİL (ölçüldü, r6) · eski kod +
/// yeni test FİKSTÜR GEÇERSİZ (ölçüldü, r6: "gerçek vade" yok). Fikstür eski kodda kurulamaz: K3'e
/// bağlıdır, eski kodda U'nun ARQ'su aç kalır ve "ARQ işi yokken vade" anı hiç oluşmaz. Bu kırmızı
/// kanıt değildir.
#[test]
fn k4g05b_pay_mixed_size_reservation() {
    const TEST: &str = "k4g05b_pay_mixed_size_reservation";
    let mut sim = Sim::new(
        product_sender(),
        PACE_TICK_NS,
        Path::new(one_way_100ms, Some(P_LINK_BPS)),
        Path::new(one_way_100ms, None),
    );
    sim.drop_fwd = Box::new(|d: &TransmitDecision| d.unit_id == P_U && carries_unit(d.kind));
    let mut fixture: Vec<String> = Vec::new();
    for id in 0..P_U {
        sim.app(id, 100);
    }
    sim.app(P_U, P_U_SIZE);
    sim.timers();
    let mut t_flood: Option<u64> = None;
    while sim.now < 10 * SEC {
        sim.advance();
        if sim.last_ack_at == Some(sim.now) && sim.s.state() != SenderState::PreConverge {
            t_flood = Some(sim.now);
            sim.w.turns = Some(Vec::new());
            for i in 0..P_BACKLOG {
                sim.app(P_U + 1 + i, P_DATA);
            }
            sim.timers();
            break;
        }
        sim.timers();
    }
    let due_at_flood = sim.s.diag_unit_recovery(P_U).dup_due_ns;
    if t_flood.is_none() || due_at_flood.is_none() {
        fixture.push(format!(
            "PreConverge'den çıkılmadı ya da U'nun kopya kaydı yok (sel={t_flood:?}, vade={due_at_flood:?})"
        ));
    }
    if unit_tx(&sim.log, P_U)
        .iter()
        .any(|e| e.d.kind == TransmitKind::Duplicate)
    {
        fixture.push("U'nun kopyası selden önce çıktı (kuruluş probu)".to_string());
    }
    // [rev6 · DUR-8 FİKSTÜR] Pencere başı = U'nun kopya kaydının, U'nun ARQ işi yokken vadesine İLK
    // ulaştığı an (vade ARQ_RETX_CAP devriyle öne çekilebilir; selin anındaki vade bayattır).
    let mut t_due: Option<u64> = None;
    let search_end = sim.now + 60 * SEC;
    while sim.now < search_end {
        let r = sim.s.diag_unit_recovery(P_U);
        if let Some(d) = r.dup_due_ns {
            if d <= sim.now && r.pending_lost_retx.is_none() {
                t_due = Some(d);
                break;
            }
        }
        sim.advance();
        sim.timers();
    }
    if t_due.is_none() {
        fixture.push(
            "selden sonra 60 s içinde U'nun kopya kaydı ARQ işi yokken vadeye ulaşmadı".to_string(),
        );
    }
    let t_due = t_due.unwrap_or_default();
    let end = t_due + 20 * SEC;
    while sim.now < end {
        sim.advance();
        sim.timers();
    }
    fn is_u(d: &TransmitDecision) -> bool {
        d.unit_id == P_U && is_reemission(d.kind)
    }
    fn has_data(r: &TurnRec) -> bool {
        r.out.iter().any(|d| d.kind == TransmitKind::Data)
    }
    let turns = sim.w.turns.take().unwrap_or_default();
    let window: Vec<&TurnRec> = turns.iter().filter(|r| r.t >= t_due).collect();
    let j_re = window.iter().position(|r| r.out.iter().any(is_u));
    let upto = j_re.unwrap_or(window.len());
    let j_star = window[..upto]
        .iter()
        .position(|r| !has_data(r) && r.backlog_after > 0 && r.tokens_after >= P_DATA);
    if window[..upto].iter().any(|r| r.backlog_after == 0) {
        fixture.push("ölçüm penceresinde DATA birikimi tükendi (doygunluk yok)".to_string());
    }
    if j_star.is_none() {
        fixture.push(
            "rezervasyon gözlenmedi (kopya doğrudan hizmet aldı) — DUR-31 kolu sınanmadı"
                .to_string(),
        );
    }
    let data_after_k: Vec<u64> = window[..upto]
        .iter()
        .enumerate()
        .filter(|&(j, r)| j >= K_ARQ_SHARE && has_data(r))
        .map(|(j, _)| j as u64)
        .collect();
    let data_after_star: Vec<u64> = match j_star {
        Some(js) => window[..upto]
            .iter()
            .enumerate()
            .filter(|&(j, r)| j > js && has_data(r))
            .map(|(j, _)| j as u64)
            .collect(),
        None => Vec::new(),
    };
    let order_ok = j_re.is_some_and(|j| {
        let out = &window[j].out;
        let u_pos = out.iter().position(is_u);
        let d_pos = out.iter().position(|d| d.kind == TransmitKind::Data);
        match (u_pos, d_pos) {
            (Some(u), Some(dp)) => u < dp,
            (Some(_), None) => true,
            _ => false,
        }
    });
    // [rev6 · İDDİA 1] Vadeden U'nun yeniden emisyonuna dek DATA emisyonu (karşılık hakkıyla ≤ 2) ve
    // U'nun gecikmesi ≤ (2·S_DATA + S_ARQ + S_kopya) / pace_min + (K_ARQ_SHARE + 4) tur.
    let data_before_re = window[..upto].iter().filter(|r| has_data(r)).count();
    let rate_min = window[..upto.min(window.len())]
        .iter()
        .map(|r| r.rate_after)
        .min()
        .unwrap_or(MIN_RATE_BPS)
        .max(MIN_RATE_BPS);
    let bound_ns =
        (2 * P_DATA + 2 * P_U_SIZE) * SEC / rate_min + (K_ARQ_SHARE as u64 + 4) * PACE_TICK_NS;
    let t_re = j_re.map(|j| window[j].t);
    println!(
        "[PAY-DUR31] sel={:?} ms selde okunan vade={:?} ms gerçek vade={} ms; j*={j_star:?} \
         j_re={j_re:?} (an={:?} ms); vadeden U'ya dek DATA={data_before_re}; K sonrası DATA turları={}; \
         j* sonrası DATA turları={}; gecikme={:?} ms ≤ sınır {} ms (pace_min {rate_min} B/s)",
        t_flood.map(|t| t / MS),
        due_at_flood.map(|t| t / MS),
        t_due / MS,
        t_re.map(|t| t / MS),
        data_after_k.len(),
        data_after_star.len(),
        t_re.map(|t| (t - t_due) / MS),
        bound_ns / MS
    );
    // Teşhis kanıtı: U'nun bütün iletimleri (an ms, tür) — K3 ile ARQ yeniden gönderimleri ve
    // ARQ_RETX_CAP devrinden sonraki kopya, pencere başının neden selde okunan vadeden ayrıldığını gösterir.
    println!(
        "[PAY-DUR31] U iletimleri (ms, tür): {:?}",
        unit_tx(&sim.log, P_U)
            .iter()
            .map(|e| (e.t / MS, e.d.kind))
            .take(16)
            .collect::<Vec<_>>()
    );
    assert_fixture(TEST, &fixture);
    assert!(
        j_re.is_some(),
        "{TEST}: İDDİA 3 — vadeli 1050 B kopya başı 20 s içinde hizmet almadı (U yeniden gönderilmedi)"
    );
    assert!(
        data_before_re <= 2,
        "{TEST}: İDDİA 1 (rev6) — vadeden U'ya dek {data_before_re} DATA emisyonu > 2 (karşılık hakkı \
         dizisi en çok iki DATA başı; tur j: {:?})",
        data_after_k.iter().take(8).collect::<Vec<_>>()
    );
    assert!(
        t_re.is_some_and(|t| t - t_due <= bound_ns),
        "{TEST}: İDDİA 1 (rev6) — U vadeden {:?} ms sonra çıktı > (2·37 + 2·1050) B / {rate_min} B/s + \
         (K_ARQ_SHARE + 4) tur = {} ms",
        t_re.map(|t| (t - t_due) / MS),
        bound_ns / MS
    );
    assert!(
        data_after_star.is_empty(),
        "{TEST}: İDDİA 2 — rezervasyon başladıktan sonra U'dan önce DATA çıktı (tur j): {:?}",
        data_after_star.iter().take(8).collect::<Vec<_>>()
    );
    assert!(
        order_ok,
        "{TEST}: İDDİA 2 — emisyon turunda U'nun kararı DATA'dan önce gelmeli"
    );
}

// ════════════════════════════════════════════════════════════════════════════════════════════
// DUR-41 — kara delik birimi: kopya zincirinin sıklığı (Ek A A.7 "Beyanlı sınırlar" + DUR-41 (B))
// ════════════════════════════════════════════════════════════════════════════════════════════
//
// Ortak türetim [türetim] (Ek A A.7; DUR-41 (B)): b = zincirin tabanı (açılıştaki aralık),
// m = ⌈log2(RTO_MAX/b)⌉. Kayıt her emisyonda `şimdi + aralık`'a yeniden kurulur ve aralık ×2'ler
// (≤ RTO_MAX; `emit_copy_at`). Öne çekilmemiş k. aralık (0 tabanlı) g_k ≥ I_(k+1) = min(2^(k+1)·b,
// RTO_MAX): ilk m − 1 aralıktan sonra ≥ RTO_MAX. Δ penceresinde n emisyon için
// e_(n−1) − e_0 = Σ_(j<n−1) g_j < Δ ve I_j = RTO_MAX (j ≥ m) ⇒ (n − m)·RTO_MAX < Δ ⇒
// n ≤ m + ⌊Δ/RTO_MAX⌋. Öne çekme yalnız vadeyi değiştirir (aralık değişmez): tek öne çekme bir
// aralığı kısaltır, sonrakileri değiştirmez ⇒ n ≤ m + ⌊Δ/RTO_MAX⌋ + 1 ve kısa aralık (g_k < I_(k+1))
// ≤ 1. Bu sınırlar yalnız ZİNCİR emisyonlarını sayar (A.7: tek seferlik kopya zincir dışı) ve
// dönem içinde geçerlidir (madde 6 sıfırlaması yeni dönemdir; ön koşul: ilerleme boşluğu < H).

/// [DUR-41] m = ⌈log2(RTO_MAX/b)⌉ (Ek A A.7 "Beyanlı sınırlar"; b = zincir tabanı, ≤ RTO_MAX).
fn chain_ramp_m(base_ns: u64) -> u64 {
    let cap = rto_max_ns();
    let ratio = cap.div_ceil(base_ns.clamp(1, cap));
    u64::from(u64::BITS - ratio.saturating_sub(1).leading_zeros())
}

/// [DUR-41] Öne çekilmemiş zincirin k. aralığının (0 tabanlı) alt sınırı I_(k+1) = min(2^(k+1)·b,
/// RTO_MAX).
fn chain_gap_floor_ns(base_ns: u64, k: usize) -> u64 {
    let cap = rto_max_ns();
    let mut f = base_ns.clamp(1, cap);
    for _ in 0..=k {
        f = f.saturating_mul(2).min(cap);
        if f == cap {
            break;
        }
    }
    f
}

/// [DUR-41] Öne çekilmemiş aralık alt sınırının altında kalan aralıklar `(k, aralık ms, sınır ms)`.
fn chain_short_gaps(times: &[u64], base_ns: u64) -> Vec<(usize, u64, u64)> {
    times
        .windows(2)
        .enumerate()
        .filter_map(|(k, w)| {
            let g = w[1] - w[0];
            let floor = chain_gap_floor_ns(base_ns, k);
            (g < floor).then_some((k, g / MS, floor / MS))
        })
        .collect()
}

/// [DUR-41] H'nin kanaldan bağımsız alt sınırı (sender.rs `unowned_horizon_ns`):
/// H = max(3·srtt, K_CONF_EAI·EAI)·1000/escalate_permille ve EAI ≥ 65 B / 31 B/s ⇒
/// H ≥ 2 · 65/31 s · 1000/300 ≈ 13.98 s. Bütün ilerleme boşlukları bunun altındaysa madde 6
/// sıfırlaması (Sıfırlama (ii)) olamaz → pencere tek zincir dönemidir.
fn h_floor_ns() -> u64 {
    /// sender.rs `K_CONF_EAI` (özel sabit; değer koddan).
    const K_CONF_EAI: u64 = 2;
    /// sender.rs `EAI_SACK_WIRE_BYTES` / `EAI_RETURN_BW_PRIOR` (özel sabitler; değer koddan).
    const EAI_SACK_B: u64 = 65;
    const EAI_RETURN_BPS: u64 = 31;
    let eai_floor = EAI_SACK_B * SEC / EAI_RETURN_BPS;
    K_CONF_EAI * eai_floor * 1000 / u64::from(HysteresisConfig::fixture().escalate_permille)
}

/// [DUR-41] Ek B.6 h.3 T_max(kopya) = (2·S_d + S_a + S_c)/R + (K + 4)τ + P_pf/R.
fn t_max_copy_ns(s_d: u64, s_a: u64, s_c: u64, rate: u64, tau: u64, p_pf: u64) -> u64 {
    (2 * s_d + s_a + s_c + p_pf) * SEC / rate.max(1) + (K_ARQ_SHARE as u64 + 4) * tau
}

/// [DUR-41] `[a, b)` aralığındaki FEC parite baytı (P_pf'nin parite kısmı).
fn parity_bytes(log: &[Emit], a: u64, b: u64) -> u64 {
    log.iter()
        .filter(|e| e.d.kind == TransmitKind::FecParity && (a..b).contains(&e.t))
        .map(|e| e.d.size)
        .sum()
}

/// [DUR-41] Düzenli DATA üreticisi: her `period`'da bir `size` B birim, kimlik `next_id`'den artan.
struct Feed {
    next_id: u64,
    next_at: u64,
    period: u64,
    size: u64,
}

impl Feed {
    fn tick(&mut self, sim: &mut Sim) {
        while sim.now >= self.next_at {
            sim.app(self.next_id, self.size);
            self.next_id += 1;
            self.next_at += self.period;
        }
    }
}

/// [DUR-41] Sürüş penceresinde toplanan gözlem.
#[derive(Default)]
struct WindowObs {
    /// İlerleme anları: en büyük ACK'li seq'in arttığı adımlar (gerçek ilerlemenin alt kümesi →
    /// buradan ölçülen boşluk gerçek ilerleme boşluğunun üst sınırıdır).
    progress: Vec<u64>,
    /// Olay sonu en düşük pace (B/s; `TurnRec::rate_after`).
    rate_min: Option<u64>,
    /// RECOVER_FWD görüldü mü (tek seferlik kopya kaynağı; A.7 zincir sayımının dışı).
    fwd_seen: bool,
    /// Kuyruk-probu birikimli bayt sayısı: başta ve `(an, değer)` değiştiği adımlarda.
    probe_bytes_start: u64,
    probe_bytes: Vec<(u64, u64)>,
    /// Kuyruk-probu birikimli emisyon sayısı: başta ve sonda.
    probes_start: u64,
    probes_end: u64,
}

impl WindowObs {
    /// `[a, b)` aralığında çıkan kuyruk-probu baytı (P_pf'nin prob kısmı).
    fn probe_bytes_in(&self, a: u64, b: u64) -> u64 {
        let at = |t: u64| {
            self.probe_bytes
                .iter()
                .take_while(|&&(ts, _)| ts < t)
                .last()
                .map_or(self.probe_bytes_start, |&(_, v)| v)
        };
        at(b).saturating_sub(at(a))
    }

    /// `[from, to]` içinde ardışık ilerlemeler arası boşluklar (uçlar dahil).
    fn progress_gaps(&self, from: u64, to: u64) -> Vec<u64> {
        let mut prev = from;
        let mut gaps = Vec::new();
        for &t in self.progress.iter().filter(|&&t| (from..=to).contains(&t)) {
            gaps.push(t - prev);
            prev = t;
        }
        gaps.push(to.saturating_sub(prev));
        gaps
    }
}

/// [DUR-41] `stop` doğru olana ya da `end`'e dek sür; her adımda ilerlet → (varsa) besle →
/// zamanlayıcılar (PAY düzeninin sırası). `stop` her adımdan önce sorulur.
fn drive_window(
    sim: &mut Sim,
    mut feed: Option<&mut Feed>,
    end: u64,
    mut stop: impl FnMut(&Sim) -> bool,
) -> WindowObs {
    let (probes_start, probe_bytes_start, ..) = sim.s.tail_probe_diag();
    let mut o = WindowObs {
        probe_bytes_start,
        probes_start,
        ..WindowObs::default()
    };
    let mut acked = sim.max_acked;
    let mut pb = probe_bytes_start;
    sim.w.turns = Some(Vec::new());
    while sim.now < end && !stop(&*sim) {
        sim.advance();
        if let Some(f) = feed.as_deref_mut() {
            f.tick(sim);
        }
        sim.timers();
        if sim.max_acked > acked {
            acked = sim.max_acked;
            o.progress.push(sim.now);
        }
        let now_pb = sim.s.tail_probe_diag().1;
        if now_pb != pb {
            pb = now_pb;
            o.probe_bytes.push((sim.now, pb));
        }
        if let Some(turns) = sim.w.turns.as_mut() {
            for r in turns.drain(..) {
                o.rate_min = Some(o.rate_min.map_or(r.rate_after, |m| m.min(r.rate_after)));
                o.fwd_seen |= r.state_after == SenderState::RecoverFwd;
            }
        }
    }
    sim.w.turns = None;
    o.probes_end = sim.s.tail_probe_diag().0;
    o
}

/// [DUR-41] `unit`'in ilk Duplicate emisyonuna dek sür (en geç `limit`); ilk kopyanın anı.
fn drive_to_first_copy(
    sim: &mut Sim,
    feed: Option<&mut Feed>,
    unit: u64,
    limit: u64,
) -> Option<u64> {
    let mut seen = 0usize;
    let mut c0: Option<u64> = None;
    let _ = drive_window(sim, feed, limit, |s| {
        if let Some(e) = s.log[seen..]
            .iter()
            .find(|e| e.d.unit_id == unit && e.d.kind == TransmitKind::Duplicate)
        {
            c0 = Some(e.t);
        }
        seen = s.log.len();
        c0.is_some()
    });
    c0
}

/// [K4G-05b DUR-41 · kara delik bekçisi, PAY düzeni] Darboğaz 1064 B/s, 37 B DATA seli, yol canlı;
/// U'nun (1050 B) BÜTÜN iletimleri düşer. ARQ_RETX_CAP devrinden sonra U kopya kuyruğundadır; her
/// kopyası sonraki DATA'nın ACK'iyle kayıp ilan edilir (later-ACK kanıtı).
///
/// Bu düzende zincirin aralığı c0'da ZATEN RTO_MAX'tır: U'nun kaydı PRE_CONVERGE §3.3-B kaydıdır
/// (ilk gecikme (1050 + 65)/31 s × 3/2 ≈ 53.95 s) ve PRE_CONVERGE çıkışında aralık RTO_MAX'a kırpılır
/// (`on_event_core`, "PRE_CONVERGE çıkışında bekleyen kopyaların aralığı ve vadesi RTO_MAX'a
/// kırpılır"; engine §13 v1.1). Rampa burada SINANMAZ (bkz. `k4g05b_blackhole_chain_ramp_a7_bound`).
///
/// İDDİALAR [türetim, yukarıdaki ortak türetim; c0'da aralık RTO_MAX ⇒ I_j = RTO_MAX]:
/// (1) 1. indeksten sonraki her aralık ≥ RTO_MAX (dönemin tek öne çekmesi en çok g_0'ı kısaltır:
///     c0 kopyasının kaybı dönemdeki ilk kayıp kanıtıdır); (2) pencere W'da kopya ≤ ⌊W/RTO_MAX⌋ + 2
///     ((n − 2)·RTO_MAX < W); (3) İKİ YÖNLÜ: her aralık ve uç boşluğu ≤ RTO_MAX + T_max(kopya) —
///     kayıt emisyondan ≤ RTO_MAX sonra vadeli, vadeli kopya ≤ T_max(kopya) içinde çıkar (Ek B.6 h.3:
///     S_d = 37 B, S_a = S_c = 1050 B, R = penceredeki en düşük pace, τ = 1 ms, P_pf = aralıktaki
///     kuyruk-probu + FEC parite baytı); (4) kopya ≥ ⌈W / (RTO_MAX + T_max(kopya))⌉ ((3)'ün sonucu;
///     T_max(kopya) < 7.5 s iken ⌊W/RTO_MAX⌋ = 5'e eşittir).
///
/// Ön koşullar (FİKSTÜR): sel kuruldu; c0 bulundu; c0'da U'nun geometrik aralığı = RTO_MAX (diag);
/// [c0, uç) boyunca en büyük ilerleme boşluğu < H'nin alt sınırı (madde 6 sıfırlaması yok); pencerede
/// U'nun Duplicate kökenli kaybı ≥ 1 (diag `copy_losses`); U teyitsiz.
///
/// E/Y [çıkarım]: 33938987 (sınırsız çekme) KIRMIZI — r7'de 150 s'de 84 kopya, aralıklar 1.335–2.801 s
/// (ölçüldü, eski bekçiyle); e6e711fe YEŞİL — r7'de 6 kopya, aralıklar [2801, 30971, 31329, 32337,
/// 31272] ms (ölçüldü); DUR-41 (B) YEŞİL — ARQ_RETX_CAP devri dönemin tek öne çekmesini c0'dan önce
/// kullanır, c0 kaybı çekmez: ≈ 5 kopya, her aralık ≥ RTO_MAX.
#[test]
fn k4g05b_blackhole_copy_interval_backoff() {
    const TEST: &str = "k4g05b_blackhole_copy_interval_backoff";
    const WINDOW: u64 = 150 * SEC;
    fn is_u_copy(e: &Emit) -> bool {
        e.d.unit_id == P_U && e.d.kind == TransmitKind::Duplicate
    }
    let cap = rto_max_ns();
    let mut sim = Sim::new(
        product_sender(),
        PACE_TICK_NS,
        Path::new(one_way_100ms, Some(P_LINK_BPS)),
        Path::new(one_way_100ms, None),
    );
    sim.drop_fwd = Box::new(|d: &TransmitDecision| d.unit_id == P_U && carries_unit(d.kind));
    let mut fixture: Vec<String> = Vec::new();
    for id in 0..P_U {
        sim.app(id, 100);
    }
    sim.app(P_U, P_U_SIZE);
    sim.timers();
    let mut flooded = false;
    while sim.now < 10 * SEC {
        sim.advance();
        if sim.last_ack_at == Some(sim.now) && sim.s.state() != SenderState::PreConverge {
            for i in 0..P_BACKLOG {
                sim.app(P_U + 1 + i, P_DATA);
            }
            flooded = true;
            sim.timers();
            break;
        }
        sim.timers();
    }
    if !flooded {
        fixture.push("PreConverge'den çıkılmadı (sel kurulmadı)".to_string());
    }
    let t_search = sim.now + 90 * SEC;
    let c0 = drive_to_first_copy(&mut sim, None, P_U, t_search);
    if c0.is_none() {
        fixture.push("U 90 s içinde kopya kuyruğundan gönderilmedi".to_string());
    }
    let c0 = c0.unwrap_or(sim.now);
    let r0 = sim.s.diag_unit_recovery(P_U);
    if r0.geo_interval_ns != Some(cap) {
        fixture.push(format!(
            "c0'da U'nun geometrik aralığı RTO_MAX değil ({:?} ms): zincir RTO_MAX'ı PRE_CONVERGE \
             kırpmasından devralmadı — bu bekçinin türetimi geçersiz",
            r0.geo_interval_ns.map(|v| v / MS)
        ));
    }
    let end = c0 + WINDOW;
    let obs = drive_window(&mut sim, None, end, |_| false);
    let r1 = sim.s.diag_unit_recovery(P_U);
    let losses = r1.copy_losses.saturating_sub(r0.copy_losses);
    let pulls = r1.loss_pulls.saturating_sub(r0.loss_pulls);
    let max_progress_gap = obs
        .progress_gaps(c0, end)
        .into_iter()
        .max()
        .unwrap_or(WINDOW);
    let h_floor = h_floor_ns();
    if max_progress_gap >= h_floor {
        fixture.push(format!(
            "pencerede ilerleme boşluğu {} ms ≥ H alt sınırı {} ms (madde 6 sıfırlaması dışlanamaz)",
            max_progress_gap / MS,
            h_floor / MS
        ));
    }
    if losses == 0 {
        fixture.push(
            "pencerede U'nun Duplicate kökenli kaybı yok — later-ACK kanıtı sınanmadı".to_string(),
        );
    }
    if obs.rate_min.is_none() {
        fixture.push("pencerede tur kaydı yok (pace ölçülemedi)".to_string());
    }
    let rate_min = obs.rate_min.unwrap_or(1).max(1);
    let times: Vec<u64> = sim
        .log
        .iter()
        .filter(|e| is_u_copy(e) && (c0..end).contains(&e.t))
        .map(|e| e.t)
        .collect();
    let gaps: Vec<u64> = times.windows(2).map(|w| w[1] - w[0]).collect();
    // (3) Her aralık ve uç boşluğu için kendi P_pf'siyle RTO_MAX + T_max(kopya).
    let bound_for = |a: u64, b: u64| {
        let p_pf = obs.probe_bytes_in(a, b) + parity_bytes(&sim.log, a, b);
        (
            cap + t_max_copy_ns(P_DATA, P_U_SIZE, P_U_SIZE, rate_min, PACE_TICK_NS, p_pf),
            p_pf,
        )
    };
    let mut spans: Vec<(u64, u64)> = times.windows(2).map(|w| (w[0], w[1])).collect();
    if let Some(&last) = times.last() {
        spans.push((last, end));
    }
    let mut over: Vec<(u64, u64, u64)> = Vec::new();
    let mut p_pf_max = 0u64;
    let mut g_bound_max = 0u64;
    for &(a, b) in &spans {
        let (bound, p_pf) = bound_for(a, b);
        p_pf_max = p_pf_max.max(p_pf);
        g_bound_max = g_bound_max.max(bound);
        if b - a > bound {
            over.push(((a - c0) / MS, (b - a) / MS, bound / MS));
        }
    }
    let g_all = cap + t_max_copy_ns(P_DATA, P_U_SIZE, P_U_SIZE, rate_min, PACE_TICK_NS, p_pf_max);
    let n = times.len() as u64;
    let n_max = WINDOW / cap + 2;
    let n_min = WINDOW.div_ceil(g_all);
    let short_after_first: Vec<(usize, u64)> = gaps
        .iter()
        .enumerate()
        .skip(1)
        .filter(|&(_, &g)| g < cap)
        .map(|(k, &g)| (k, g / MS))
        .collect();
    println!(
        "[kara delik PAY] c0={} ms aralık(c0)={:?} ms; kopya={n} (sınır {n_min}..={n_max}; ⌊W/RTO_MAX⌋={}) \
         anlar(c0'dan ms)={:?} aralıklar(ms)={:?}; uç boşluğu={:?} ms; RTO_MAX+T_max üst={} ms (pace_min \
         {rate_min} B/s, P_pf_max {p_pf_max} B); aşan (an, aralık, sınır ms)={over:?}; 1. indeksten sonra \
         kısa={short_after_first:?}; Dup kökenli kayıp={losses} gerçek öne çekme={pulls} (ömür {}) hak \
         kullanıldı={}; en büyük ilerleme boşluğu={} ms (H alt sınırı {} ms); prob {}→{}",
        c0 / MS,
        r0.geo_interval_ns.map(|v| v / MS),
        WINDOW / cap,
        times.iter().map(|t| (t - c0) / MS).collect::<Vec<_>>(),
        gaps.iter().map(|g| g / MS).collect::<Vec<_>>(),
        times.last().map(|&t| (end - t) / MS),
        g_bound_max / MS,
        r1.loss_pulls,
        r1.loss_pull_used,
        max_progress_gap / MS,
        h_floor / MS,
        obs.probes_start,
        obs.probes_end
    );
    assert_fixture(TEST, &fixture);
    assert!(
        !sim.w.confirmed_set.contains(&P_U),
        "{TEST}: FİKSTÜR GEÇERSİZ — U teyit edildi (kara delik kurulmadı)"
    );
    assert!(
        short_after_first.is_empty(),
        "{TEST}: (1) 1. indeksten sonra RTO_MAX'tan kısa kopya aralığı (k, ms): {short_after_first:?}"
    );
    assert!(
        n <= n_max,
        "{TEST}: (2) {} s pencerede {n} kopya > ⌊W/RTO_MAX⌋ + 2 = {n_max}",
        WINDOW / SEC
    );
    assert!(
        over.is_empty(),
        "{TEST}: (3) RTO_MAX + T_max(kopya)'yı aşan aralık (c0'dan an ms, aralık ms, sınır ms): {over:?}"
    );
    assert!(
        n >= n_min,
        "{TEST}: (4) {} s pencerede {n} kopya < ⌈W/(RTO_MAX + T_max(kopya))⌉ = {n_min}",
        WINDOW / SEC
    );
}

/// [K4G-05b DUR-41 · rampa] Ek A A.7 + DUR-41 (B) sınırının TAM biçimi: zincir tabanı srtt'den
/// başlar (PRE_CONVERGE kırpması mirası YOK), U kara delikte, ACK akışı sürekli.
///
/// Kurulum: darboğazsız yol, tek yön 100 ms; ısınma 4 × 100 B; yakınsamadan sonra 20 ms'de bir
/// 100 B arka plan DATA'sı (kimlik 1000+) uca dek; 1 s sonra U (1050 B) sunulur — ilk iletimi
/// PRE_CONVERGE DIŞINDA olduğundan §3.3-B kaydı almaz. U'nun bütün iletimleri düşer: Data → ARQ ×3
/// (her biri sonraki DATA'nın ACK'iyle kayıp) → ARQ_RETX_CAP devri zinciri AÇAR (vade şimdi, aralık
/// taban = srtt; kuruluş öne çekme sayılmaz) → zincirin her kopyası later-ACK ile kayıp ilan edilir.
///
/// İDDİALAR (ortak türetim; b = c0 sonrası aralığın yarısı — kayıt c0'da `min(2b, RTO_MAX)`'a
/// yeniden kurulur; ön koşul 2b < RTO_MAX): (1) öne çekilmemiş alt sınırın (g_k ≥ min(2^(k+1)·b,
/// RTO_MAX); ilk m − 1 aralıktan sonra RTO_MAX) altında kalan aralık ≤ 1 (dönemin tek öne çekmesi);
/// (2) W'da zincir emisyonu ≤ m + ⌊W/RTO_MAX⌋ + 1; (3) dönemde GERÇEK öne çekme ≤ 1 (diag
/// `loss_pulls`, girdinin ömrü = bu zincirin tek dönemi).
///
/// Ön koşullar (FİKSTÜR): PRE_CONVERGE'den çıkıldı; U'nun sunulduğu anda geometrik kaydı yok; c0
/// bulundu; 2b < RTO_MAX; [c0, uç) boyunca en büyük ilerleme boşluğu < H alt sınırı; pencerede
/// kuyruk-probu yok ve RECOVER_FWD yok (U'nun Duplicate'leri yalnız zincir); U'nun Duplicate kökenli
/// kaybı ≥ 1; U teyitsiz.
///
/// E/Y [çıkarım]: 33938987 KIRMIZI (her kayıp çeker: aralık ≈ kayıp tespiti + taban'da kalır);
/// e6e711fe YEŞİL (bu düzende yalnız Duplicate kolu çeker, o da dönemde bir kez; ARQ_RETX_CAP devri
/// zinciri AÇAR, çekmez; tek seferlik ekleme yeri yok); DUR-41 (B) YEŞİL. Bekçi sınırsız ya da
/// dönemde birden çok öne çekmeyi yakalar; e6e711fe ile DUR-41'i ayıran düzen
/// `k4g05b_blackhole_intermittent_tail_on_chain_bound`'dur.
#[test]
fn k4g05b_blackhole_chain_ramp_a7_bound() {
    const TEST: &str = "k4g05b_blackhole_chain_ramp_a7_bound";
    const U: u64 = 4;
    const U_SIZE: u64 = 1050;
    const BG_SIZE: u64 = 100;
    const BG_PERIOD: u64 = 20 * MS;
    const WINDOW: u64 = 150 * SEC;
    let cap = rto_max_ns();
    let mut sim = Sim::new(
        product_sender(),
        PACE_TICK_NS,
        Path::new(one_way_100ms, None),
        Path::new(one_way_100ms, None),
    );
    sim.drop_fwd = Box::new(|d: &TransmitDecision| d.unit_id == U && carries_unit(d.kind));
    let mut fixture: Vec<String> = Vec::new();
    for id in 0..U {
        sim.app(id, BG_SIZE);
    }
    sim.timers();
    let _ = drive_window(&mut sim, None, 10 * SEC, |s| {
        s.last_ack_at.is_some() && s.s.state() != SenderState::PreConverge
    });
    if sim.s.state() == SenderState::PreConverge {
        fixture.push("10 s içinde PRE_CONVERGE'den çıkılmadı".to_string());
    }
    let mut feed = Feed {
        next_id: 1000,
        next_at: sim.now,
        period: BG_PERIOD,
        size: BG_SIZE,
    };
    let t_u = sim.now + SEC;
    let _ = drive_window(&mut sim, Some(&mut feed), t_u, |_| false);
    sim.app(U, U_SIZE);
    if sim.s.diag_unit_recovery(U).geo_interval_ns.is_some() {
        fixture.push("U sunulduğu anda geometrik kayıt aldı (§3.3-B mirası)".to_string());
    }
    let c0 = drive_to_first_copy(&mut sim, Some(&mut feed), U, t_u + 60 * SEC);
    if c0.is_none() {
        fixture.push("U 60 s içinde kopya kuyruğundan gönderilmedi".to_string());
    }
    let c0 = c0.unwrap_or(sim.now);
    let r0 = sim.s.diag_unit_recovery(U);
    let srtt_c0 = sim.s.path_timing_snapshot().srtt_ns;
    let i1 = r0.geo_interval_ns.unwrap_or(cap);
    if i1 >= cap {
        fixture.push(format!(
            "c0 sonrası aralık {} ms RTO_MAX'ta — rampa yok (taban srtt'den başlamadı)",
            i1 / MS
        ));
    }
    let base = (i1 / 2).max(1);
    let m = chain_ramp_m(base);
    let end = c0 + WINDOW;
    let obs = drive_window(&mut sim, Some(&mut feed), end, |_| false);
    let r1 = sim.s.diag_unit_recovery(U);
    let losses = r1.copy_losses.saturating_sub(r0.copy_losses);
    let max_progress_gap = obs
        .progress_gaps(c0, end)
        .into_iter()
        .max()
        .unwrap_or(WINDOW);
    let h_floor = h_floor_ns();
    if max_progress_gap >= h_floor {
        fixture.push(format!(
            "pencerede ilerleme boşluğu {} ms ≥ H alt sınırı {} ms",
            max_progress_gap / MS,
            h_floor / MS
        ));
    }
    if obs.probes_end != obs.probes_start {
        fixture.push(format!(
            "pencerede kuyruk-probu çıktı ({} → {}): ACK akışı PTO'yu aştı",
            obs.probes_start, obs.probes_end
        ));
    }
    if obs.fwd_seen {
        fixture.push("pencerede RECOVER_FWD (tek seferlik kopya kaynağı)".to_string());
    }
    if losses == 0 {
        fixture.push("pencerede U'nun Duplicate kökenli kaybı yok".to_string());
    }
    let times: Vec<u64> = sim
        .log
        .iter()
        .filter(|e| {
            e.d.unit_id == U && e.d.kind == TransmitKind::Duplicate && (c0..end).contains(&e.t)
        })
        .map(|e| e.t)
        .collect();
    let short = chain_short_gaps(&times, base);
    let n = times.len() as u64;
    let n_max = m + WINDOW / cap + 1;
    println!(
        "[kara delik rampa] c0={} ms taban b={} ms (srtt(c0) {:?} ms) m={m}; kopya={n} ≤ m + ⌊W/RTO_MAX⌋ + 1 \
         = {n_max}; aralıklar(ms)={:?}; öne çekilmemiş alt sınırın altında (k, ms, sınır ms)={short:?}; \
         Dup kökenli kayıp={losses}; gerçek öne çekme (ömür)={} hak kullanıldı={}; en büyük ilerleme \
         boşluğu={} ms",
        c0 / MS,
        base / MS,
        srtt_c0.map(|v| v / MS),
        times.windows(2).map(|w| (w[1] - w[0]) / MS).collect::<Vec<_>>(),
        r1.loss_pulls,
        r1.loss_pull_used,
        max_progress_gap / MS
    );
    assert_fixture(TEST, &fixture);
    assert!(
        !sim.w.confirmed_set.contains(&U),
        "{TEST}: FİKSTÜR GEÇERSİZ — U teyit edildi (kara delik kurulmadı)"
    );
    assert!(
        short.len() <= 1,
        "{TEST}: (1) öne çekilmemiş alt sınırın altında {} aralık > 1 (dönemin tek öne çekmesi): {short:?}",
        short.len()
    );
    assert!(
        n <= n_max,
        "{TEST}: (2) {} s pencerede {n} zincir emisyonu > m + ⌊W/RTO_MAX⌋ + 1 = {n_max}",
        WINDOW / SEC
    );
    assert!(
        r1.loss_pulls <= 1,
        "{TEST}: (3) dönemde {} gerçek öne çekme > 1",
        r1.loss_pulls
    );
}

/// Aralıklı trafik kolu tek yön yayılımı: RTT 40 ms (`burst_loss_recovery.rs` deseni).
fn one_way_20ms(_t: u64) -> u64 {
    20 * MS
}

/// [K4G-05b DUR-41 · aralıklı trafik, kuyruk-probu AÇIK] Ürün yapılandırması (`product_sender`:
/// validated_v1 prob), darboğazsız yol, RTT 40 ms; 1 s'de bir 200 B birim (kimlik 0, 1, 2, …; uca
/// dek); U = birim 10 (t = 10 s), U'nun BÜTÜN iletimleri düşer. İlerleme boşlukları ≈ 1 s: taban
/// PTO'dan uzun, H'den kısa (wf_060b3c83 bulgu 1 rejimi). Her boşlukta prob U'nun uçuştaki kaydını
/// hedefler (v1 mirası, `oldest_probe_eligible_seq`; DUR-42) → prob kayıtlarının kaybı ARQ'ya girer
/// → ARQ_RETX_CAP devri birimin geometrik kaydını bulur (later-ACK kayıp kanıtı).
///
/// İDDİA (ortak türetim; b = c0 sonrası aralığın yarısı): U'nun ZİNCİR + ÇEKME bileşeni (bütün
/// Duplicate'leri — RECOVER_FWD yokken hepsi zincir emisyonudur) ≤ m + ⌊W/RTO_MAX⌋ + 1; öne
/// çekilmemiş alt sınırın altında kalan aralık ≤ 1; dönemde gerçek öne çekme ≤ 1 (diag).
///
/// ÖLÇÜLÜR, ASSERT EDİLMEZ (DUR-42): U'nun iletim kırılımı — Data / Retransmit(prob) /
/// Retransmit(ARQ) / Duplicate(zincir) / Duplicate(çekme sonrası) (prob = olay düzeyi `Emit::probe`
/// bayrağı: aynı olayda çıkan ARQ yeniden gönderimi de prob sayılır; çekme sonrası = öne çekilmemiş
/// alt sınırın altındaki aralığın kopyası) — ve prob bileşeni ile v1 bütçesi
/// [türetim]: prob, ilerleme her geldiğinde yeniden silahlanır (`on_ack_progress` →
/// `probe_index = 0`) ve silahlanma başına ≤ max_tail_probes = 3 → pencerede ≤ 3 × (taban PTO'yu
/// aşan ilerleme boşluğu sayısı).
///
/// Ön koşullar (FİKSTÜR): c0 bulundu; 2b < RTO_MAX; en büyük ilerleme boşluğu < H alt sınırı;
/// pencerede ≥ 1 kuyruk-probu (PTO < boşluk rejimi kuruldu); RECOVER_FWD yok; U teyitsiz.
///
/// E/Y [çıkarım]: e6e711fe KIRMIZI — ARQ_RETX_CAP devrinin çekmesi sınırsızdır; prob → ARQ →
/// CapExceeded döngüsü boşluk başına bir öne çekme üretir, kopya sayısı ≈ boşluk sayısı (≈ 150)
/// mertebesine çıkar. DUR-41 (B) YEŞİL — üç yer tek hakkı paylaşır, zincir kendi takvimindedir.
#[test]
fn k4g05b_blackhole_intermittent_tail_on_chain_bound() {
    const TEST: &str = "k4g05b_blackhole_intermittent_tail_on_chain_bound";
    const U: u64 = 10;
    const SIZE: u64 = 200;
    const WINDOW: u64 = 150 * SEC;
    let cap = rto_max_ns();
    let mut sim = Sim::new(
        product_sender(),
        PACE_TICK_NS,
        Path::new(one_way_20ms, None),
        Path::new(one_way_20ms, None),
    );
    sim.drop_fwd = Box::new(|d: &TransmitDecision| d.unit_id == U && carries_unit(d.kind));
    let mut fixture: Vec<String> = Vec::new();
    let mut feed = Feed {
        next_id: 0,
        next_at: 0,
        period: SEC,
        size: SIZE,
    };
    feed.tick(&mut sim);
    sim.timers();
    let c0 = drive_to_first_copy(&mut sim, Some(&mut feed), U, 60 * SEC);
    if c0.is_none() {
        fixture.push("U 60 s içinde kopya kuyruğundan gönderilmedi".to_string());
    }
    let c0 = c0.unwrap_or(sim.now);
    let first_u = sim.unit_tx(U).first().map(|e| (e.t, e.d.kind));
    if first_u.map(|(_, k)| k) != Some(TransmitKind::Data) {
        fixture.push(format!("U'nun ilk iletimi Data değil: {first_u:?}"));
    }
    let r0 = sim.s.diag_unit_recovery(U);
    let i1 = r0.geo_interval_ns.unwrap_or(cap);
    if i1 >= cap {
        fixture.push(format!(
            "c0 sonrası aralık {} ms RTO_MAX'ta — rampa yok",
            i1 / MS
        ));
    }
    let base = (i1 / 2).max(1);
    let m = chain_ramp_m(base);
    let base_pto = sim.s.diag_recovery_timers().base_pto_ns;
    let end = c0 + WINDOW;
    let obs = drive_window(&mut sim, Some(&mut feed), end, |_| false);
    let r1 = sim.s.diag_unit_recovery(U);
    let pgaps = obs.progress_gaps(c0, end);
    let max_progress_gap = pgaps.iter().copied().max().unwrap_or(WINDOW);
    let gaps_over_pto = pgaps.iter().filter(|&&g| g > base_pto).count() as u64;
    let h_floor = h_floor_ns();
    if max_progress_gap >= h_floor {
        fixture.push(format!(
            "pencerede ilerleme boşluğu {} ms ≥ H alt sınırı {} ms",
            max_progress_gap / MS,
            h_floor / MS
        ));
    }
    if obs.probes_end == obs.probes_start {
        fixture.push("pencerede kuyruk-probu çıkmadı — PTO < boşluk rejimi kurulmadı".to_string());
    }
    if obs.fwd_seen {
        fixture.push("pencerede RECOVER_FWD (tek seferlik kopya kaynağı)".to_string());
    }
    let tx: Vec<Emit> = sim
        .unit_tx(U)
        .into_iter()
        .filter(|e| (c0..end).contains(&e.t))
        .collect();
    let n_data = tx.iter().filter(|e| e.d.kind == TransmitKind::Data).count();
    let n_probe = tx
        .iter()
        .filter(|e| e.d.kind == TransmitKind::Retransmit && e.probe)
        .count() as u64;
    let n_arq = tx
        .iter()
        .filter(|e| e.d.kind == TransmitKind::Retransmit && !e.probe)
        .count();
    let times: Vec<u64> = tx
        .iter()
        .filter(|e| e.d.kind == TransmitKind::Duplicate)
        .map(|e| e.t)
        .collect();
    let short = chain_short_gaps(&times, base);
    let n_dup = times.len() as u64;
    let n_pulled = short.len() as u64;
    let n_max = m + WINDOW / cap + 1;
    let probe_budget_v1 = probe_budget() * gaps_over_pto;
    println!(
        "[kara delik aralıklı] c0={} ms taban b={} ms m={m} taban PTO={} ms; ilerleme boşluğu: en büyük {} ms, \
         PTO'yu aşan {gaps_over_pto}; U kırılımı W={} s: Data={n_data} Retransmit(prob)={n_probe} \
         Retransmit(ARQ)={n_arq} Duplicate(zincir)={} Duplicate(çekme sonrası)={n_pulled}; zincir + çekme \
         {n_dup} ≤ m + ⌊W/RTO_MAX⌋ + 1 = {n_max}; kısa aralıklar (k, ms, sınır ms)={:?}; gerçek öne çekme \
         (ömür)={} hak kullanıldı={}; prob bileşeni {n_probe} (v1 bütçesi ≤ 3 × {gaps_over_pto} = \
         {probe_budget_v1}; {} B/s); prob {}→{}",
        c0 / MS,
        base / MS,
        base_pto / MS,
        max_progress_gap / MS,
        WINDOW / SEC,
        n_dup - n_pulled,
        short.iter().take(16).collect::<Vec<_>>(),
        r1.loss_pulls,
        r1.loss_pull_used,
        n_probe * SIZE / (WINDOW / SEC),
        obs.probes_start,
        obs.probes_end
    );
    assert_fixture(TEST, &fixture);
    assert!(
        !sim.w.confirmed_set.contains(&U),
        "{TEST}: FİKSTÜR GEÇERSİZ — U teyit edildi (kara delik kurulmadı)"
    );
    assert!(
        n_dup <= n_max,
        "{TEST}: zincir + çekme bileşeni {n_dup} > m + ⌊W/RTO_MAX⌋ + 1 = {n_max} (Ek A A.7 + DUR-41)"
    );
    assert!(
        short.len() <= 1,
        "{TEST}: öne çekilmemiş alt sınırın altında {} aralık > 1: {:?}",
        short.len(),
        short.iter().take(16).collect::<Vec<_>>()
    );
    assert!(
        r1.loss_pulls <= 1,
        "{TEST}: dönemde {} gerçek öne çekme > 1",
        r1.loss_pulls
    );
}

// ════════════════════════════════════════════════════════════════════════════════════════════
// Madde 6 — sıfırlama
// ════════════════════════════════════════════════════════════════════════════════════════════

/// Sıfırlama (madde 6; AMENDMENT-12 "Sıfırlama" (ii); Ek B.2 "PAY ve sıfırlama").
///
/// Kurulum (ağsız): ısınma (4 × 300 B, 300 ms'de ACK) → u4..u8 (300 B) 500 ms'de girer, hiçbir
/// iletimi ACK'lenmez → prob → devir → beş geometrik zincir (aralık srtt'den ×2, ≤ RTO_MAX). 100 s'den
/// sonraki ilk zincir turunda (sessizlik ≥ H; aralıklar 30 s'de; birimlerin son K_MAP iletimi
/// kopyadır — 05b'de budama kapalı olduğundan eski Data/prob kayıtları uçuşta kalabilir ve
/// sıfırlama ACK'iyle kayıp ilan edilip ARQ'ya girer) u4'ün o kopyası 300 ms sonra (≈ srtt; şişkin
/// RTT örneği tabanı bozmasın) ACK'lenir → u4 teyit, ilk ilerlemeli ACK = sıfırlama anı.
///
/// AMAÇ: (1) o ACK'ten hemen sonra u5..u8'in geometrik aralığı tabana döner (≤ 2·srtt); (2) u5..u8'in
/// sıfırlama sonrası ilk kopyaları birim kimliği sırasıyla çıkar ve en geç sıfırlama + taban +
/// (4 × 300 B)/hız + 2 tik içinde (sıfırlamasız bir sonraki kopya ≈ 30 s sonra olurdu); (3) her
/// turda emisyon ≤ token bütçesi (tur sonu BURST_CAP; INV-RATE-1 — sıfırlama token üretmez).
///
/// TABANDA: KIRMIZI beklenir (D1: devir yok, kopya zinciri yok; sıfırlama yok).
#[test]
fn k4g05b_reset_after_silence_order_and_budget() {
    const TEST: &str = "k4g05b_reset_after_silence_order_and_budget";
    const UNIT: u64 = 300;
    const STEP: u64 = 10 * MS;
    let units: [u64; 4] = [5, 6, 7, 8];
    let mut d = Drv::new(product_sender());
    let mut fixture: Vec<String> = Vec::new();
    for id in 0..4 {
        d.app(id, UNIT);
    }
    d.pump_to(300 * MS, STEP);
    let warm: Vec<u64> = (0..4).filter_map(|id| d.data_seq(id)).collect();
    d.ack(warm);
    d.pump_to(500 * MS, STEP);
    d.w.units = vec![4, 5, 6, 7, 8];
    for id in 4..=8 {
        d.app(id, UNIT);
    }
    d.pump_to(100 * SEC, STEP);
    let n_before = d.unit_tx(4).len();
    while d.unit_tx(4).len() == n_before && d.now < 140 * SEC {
        d.now += STEP;
        d.tick();
    }
    let last_u4 = d.unit_tx(4).last().map(|e| e.d.seq);
    let ack_at = d.now + 300 * MS;
    d.pump_to(ack_at, STEP);
    let t_reset = d.now;
    let all_copies = (4..=8).all(|u| d.s.diag_dup_queue_units().contains(&u));
    let non_dup_inflight = (4..=8).any(|u| {
        d.unit_tx(u)
            .iter()
            .rev()
            .take(k_map() as usize)
            .any(|e| e.d.kind != TransmitKind::Duplicate)
    });
    if !all_copies || non_dup_inflight {
        fixture.push(format!(
            "sıfırlama anında beş birimin yalnız kopya kayıtları olmalı (hepsi kuyrukta={all_copies}, \
             son K_MAP kayıtta kopya-dışı={non_dup_inflight})"
        ));
    }
    let before_reset: Vec<(u64, Option<u64>)> = units
        .iter()
        .map(|&u| (u, d.s.diag_unit_recovery(u).dup_interval_ns))
        .collect();
    assert_fixture(TEST, &fixture);
    d.w.turns = Some(Vec::new());
    d.ack(last_u4.into_iter().collect());
    let srtt = d.s.path_timing_snapshot().srtt_ns.unwrap_or(SEC);
    let rate = d.s.diag_pacer().0.max(1);
    let after_reset: Vec<(u64, Option<u64>)> = units
        .iter()
        .map(|&u| (u, d.s.diag_unit_recovery(u).dup_interval_ns))
        .collect();
    d.pump_to(t_reset + 10 * SEC, STEP);
    let first_post: Vec<(u64, Option<(u64, u64)>)> = units
        .iter()
        .map(|&u| {
            let e = d.log.iter().find(|e| {
                e.t >= t_reset && e.d.unit_id == u && e.d.kind == TransmitKind::Duplicate
            });
            (u, e.map(|e| (e.t, e.d.seq)))
        })
        .collect();
    let turns = d.w.turns.take().unwrap_or_default();
    let over_budget: Vec<(u64, u64, u64)> = turns
        .iter()
        .map(|r| (r.t, r.out.iter().map(|x| x.size).sum::<u64>(), r.cap_after))
        .filter(|&(_, bytes, cap)| bytes > cap)
        .collect();
    println!(
        "[SIFIRLAMA] sıfırlama={} ms; u4 teyit={}; aralık öncesi={before_reset:?} sonrası={after_reset:?}; \
         srtt={} ms hız={rate} B/s; ilk kopyalar (birim, (an ms, seq))={:?}",
        t_reset / MS,
        d.w.confirmed_set.contains(&4),
        srtt / MS,
        first_post
            .iter()
            .map(|&(u, e)| (u, e.map(|(t, s)| (t / MS, s))))
            .collect::<Vec<_>>()
    );
    assert!(
        d.w.confirmed_set.contains(&4),
        "{TEST}: u4'ün canlı kopyasının ACK'i u4'ü teyit etmeli"
    );
    for &(u, iv) in &after_reset {
        assert!(
            iv.is_some_and(|i| i <= 2 * srtt),
            "{TEST}: sıfırlamadan sonra u{u}'nın aralığı tabana dönmedi: {iv:?} (srtt {srtt})"
        );
    }
    let bound = t_reset + srtt + 4 * UNIT * SEC / rate + 2 * STEP;
    let mut prev_seq = 0u64;
    for &(u, e) in &first_post {
        let Some((t, seq)) = e else {
            panic!("{TEST}: u{u} sıfırlamadan sonra 10 s içinde kopyalanmadı");
        };
        assert!(
            t <= bound,
            "{TEST}: u{u}'nın sıfırlama sonrası ilk kopyası {} ms > sınır {} ms",
            t / MS,
            bound / MS
        );
        assert!(
            seq > prev_seq,
            "{TEST}: sıfırlama sonrası kopyalar birim kimliği sırasıyla çıkmalı (u{u} seq {seq} ≤ {prev_seq})"
        );
        prev_seq = seq;
    }
    assert!(
        over_budget.is_empty(),
        "{TEST}: turda emisyon BURST_CAP'i aştı (an ms, bayt, cap): {:?}",
        over_budget
            .iter()
            .take(8)
            .map(|&(t, b, c)| (t / MS, b, c))
            .collect::<Vec<_>>()
    );
}
