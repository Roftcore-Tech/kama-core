//! K4G-05b — zamanlama testleri (K4G-05b notu §6 "05b testleri", Ek B.2): BURST (B1) tur başı
//! tabanı kolları (karışık boy, 2·MTU + 1 ARQ başı, bekleyen prob hedefi + aynı tur, boş kuyruk
//! kontrolü, PreConverge v1 kontrolü), PAY sınıflar arası kopya payı + DUR-31 rezervasyonu (karışık
//! boy; CEVAP PAKETİ 10 DUR-31 bağlayıcı okuması: iki iddia ayrı), madde 6 sıfırlama. P0 BURST/PAY
//! (eşit boy) kolları `k4g_p0_engine.rs`'de iddiaları gevşetilmeden kalır.
//!
//! Her test başında amaç ve TABANDA (K4G-05b öncesi motor) beklenen sonuç [çıkarım; koşulmadı].
//!
//! Koş: `CARGO_BUILD_JOBS=2 cargo test -p adaptive-transport --test k4g05b_scheduling -- --nocapture --test-threads=1`

mod k4g05b_support;

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

/// [K4G-05b rev6 · kara delik bekçisi] Bir birimin ardışık kopya emisyonları: `m = ⌈log2(RTO_MAX/taban)⌉`
/// olmak üzere ilk m + 2 aralıktan sonra her aralık ≥ RTO_MAX (geometrik aralık ×2, ≤ RTO_MAX); izinli
/// olay olarak dönem başına TEK bir later-ACK öne çekmesi (`exceptions_allowed`) kalabilir. Ayrıca
/// pencere W içinde kopya sayısı ≤ m + ⌊W/RTO_MAX⌋ + 3 (+ izinli çekme). İhlalde panik.
fn assert_copy_backoff(
    tag: &str,
    times: &[u64],
    base_ns: u64,
    window_ns: u64,
    exceptions_allowed: usize,
) {
    let base = base_ns.clamp(1, rto_max_ns());
    let ratio = rto_max_ns().div_ceil(base);
    let m = u64::from(u64::BITS - ratio.saturating_sub(1).leading_zeros()) as usize;
    let gaps: Vec<u64> = times.windows(2).map(|w| w[1] - w[0]).collect();
    let short_late: Vec<(usize, u64)> = gaps
        .iter()
        .enumerate()
        .filter(|&(k, &g)| k >= m + 2 && g < rto_max_ns())
        .map(|(k, &g)| (k, g / MS))
        .collect();
    let count_bound = m as u64 + window_ns / rto_max_ns() + 3 + exceptions_allowed as u64;
    println!(
        "[kara delik] {tag}: kopya={} aralıklar(ms)={:?} m={m} sayı sınırı={count_bound} kısa geç aralık={short_late:?}",
        times.len(),
        gaps.iter().map(|g| g / MS).collect::<Vec<_>>()
    );
    assert!(
        short_late.len() <= exceptions_allowed,
        "{tag}: {m}+2. aralıktan sonra RTO_MAX'tan kısa kopya aralığı (izinli {exceptions_allowed}): {short_late:?}"
    );
    assert!(
        times.len() as u64 <= count_bound,
        "{tag}: {} s pencerede {} kopya > m + ⌊W/RTO_MAX⌋ + 3 + izinli = {count_bound}",
        window_ns / SEC,
        times.len()
    );
}

/// [K4G-05b rev6 · kara delik bekçisi] PAY düzeni (darboğaz 1064 B/s, 37 B DATA seli, yol canlı), U'nun
/// (1050 B) BÜTÜN iletimleri düşer. ARQ_RETX_CAP devrinden sonra U kopya kuyruğundadır; her kopyası
/// sonraki DATA'nın ACK'iyle kayıp ilan edilir (later-ACK kanıtı).
///
/// AMAÇ: kara deliğe düşmüş birimin ardışık kopya aralıkları, tanımlı olaylar dışında RTO_MAX'a dek
/// büyür. Tanımlı olaylar: zincirin kuruluşu, madde 6 sıfırlaması (bu düzende yok: ACK akışı
/// sürekli), dönem başına tek later-ACK öne çekmesi (9868ba31). Ön koşul (assert'li): U kopya
/// kuyruğuna girdi; pencere boyunca ACK akışı sürdü (yol canlı).
///
/// 9868ba31 ÖNCESİ: KIRMIZI beklenir. Ölçüldü (kod-run/r6, 33938987): kopyalar ≈ 1.5–2.8 s arayla,
/// aralık büyümüyor.
#[test]
fn k4g05b_blackhole_copy_interval_backoff() {
    const TEST: &str = "k4g05b_blackhole_copy_interval_backoff";
    const WINDOW: u64 = 150 * SEC;
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
    // U'nun kopya kuyruğuna girdiği ilk kopya emisyonu (ARQ_RETX_CAP devri sonrası).
    let t_search = sim.now + 90 * SEC;
    let first_copy = |sim: &Sim| {
        unit_tx(&sim.log, P_U)
            .iter()
            .find(|e| e.d.kind == TransmitKind::Duplicate)
            .map(|e| e.t)
    };
    while first_copy(&sim).is_none() && sim.now < t_search {
        sim.advance();
        sim.timers();
    }
    let c0 = first_copy(&sim);
    if c0.is_none() {
        fixture.push("U 90 s içinde kopya kuyruğundan gönderilmedi".to_string());
    }
    let c0 = c0.unwrap_or_default();
    let base = sim.s.path_timing_snapshot().srtt_ns.unwrap_or(SEC);
    let end = c0 + WINDOW;
    while sim.now < end {
        sim.advance();
        sim.timers();
    }
    if !sim.last_ack_at.is_some_and(|t| t + 5 * SEC >= end) {
        fixture.push(format!(
            "pencere boyunca ACK akışı sürmedi (son ACK {:?} ms, uç {} ms) — later-ACK kanıtı sınanmadı",
            sim.last_ack_at.map(|t| t / MS),
            end / MS
        ));
    }
    let times: Vec<u64> = unit_tx(&sim.log, P_U)
        .iter()
        .filter(|e| e.d.kind == TransmitKind::Duplicate && (c0..end).contains(&e.t))
        .map(|e| e.t)
        .collect();
    assert_fixture(TEST, &fixture);
    assert!(
        !sim.w.confirmed_set.contains(&P_U),
        "{TEST}: FİKSTÜR GEÇERSİZ — U teyit edildi (kara delik kurulmadı)"
    );
    assert_copy_backoff(TEST, &times, base, WINDOW, 1);
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
