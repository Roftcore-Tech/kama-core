//! K4G-05b — sahiplik, teyit ve kayıt sınırı testleri (K4G-05b notu §6 "05b testleri", Ek B.2):
//! üç kabul koşulu — (i) owned(U) her olayda, (ii-a) P0 runtime aynası, (ii-b) 05c-model aynası —;
//! D1 (üç kol + iki birimli kol; CEVAP PAKETİ 10 DUR-26 okuması); H; K_MAP (05b'de budama kapalı,
//! CEVAP 11 DUR-30); ESI ve parite akıbeti;
//! B2 ekleme kuralı; D2.
//!
//! Her test başında amaç ve TABANDA (K4G-05b öncesi motor, v1) beklenen sonuç yazılıdır
//! [çıkarım; koşulmadı]. P0 yöntemi: fikstür iddiaları (senaryo sınanacak durumu kurdu mu)
//! "FİKSTÜR GEÇERSİZ" ile ayrı düşer — o sonuç kırmızı SAYILMAZ, fikstür düzeltilir. Uygunluk
//! (gözlenebilir sahip) ve emisyon (gerçek `TransmitDecision`) ayrı ölçülür ve raporlanır.
//!
//! Koş: `CARGO_BUILD_JOBS=2 cargo test -p adaptive-transport --test k4g05b_ownership -- --nocapture --test-threads=1`

mod k4g05b_support;

use adaptive_transport::sender::{
    FecActuationMode, Sender, SenderState, TransmitDecision, TransmitKind,
};
use k4g05b_support::*;
use std::time::Duration;

fn assert_fixture(test: &str, fixture: &[String]) {
    assert!(
        fixture.is_empty(),
        "{test}: FİKSTÜR GEÇERSİZ (sınanacak durum kurulmadı; sonuç kırmızı SAYILMAZ):\n{}",
        fixture.join("\n")
    );
}

/// Ardışık emisyon aralıkları: `from`'dan başlayarak `times` ve en son emisyondan `end`'e.
fn max_gap(from: u64, times: &[u64], end: u64) -> u64 {
    let mut prev = from;
    let mut worst = 0;
    for &t in times {
        worst = worst.max(t.saturating_sub(prev));
        prev = t;
    }
    worst.max(end.saturating_sub(prev))
}

// ════════════════════════════════════════════════════════════════════════════════════════════
// D1 — prob bütçesi / H / bayat SACK (Ek B.2 "D1 (rev4 fikstürü, C9)")
// ════════════════════════════════════════════════════════════════════════════════════════════

const D1_UNIT: u64 = 300;
const D1_WARM: u64 = 4;
const D1_STEP: u64 = 10 * MS;
const D1_HORIZON: u64 = 600 * SEC;

struct D1Run {
    sim: Sim,
    t0: u64,
    end: u64,
    /// Kurban girişindeki taban PTO (`diag_recovery_timers().base_pto_ns`).
    base_pto_ns: u64,
    /// Prob bütçesinin tükendiği ilk gözlem anı ve o anda kurbanların sahibi.
    t_exhaust: Option<u64>,
    exhaust_owner: Vec<(u64, Option<Owner>)>,
    fixture: Vec<String>,
}

fn note_exhaust(
    sim: &Sim,
    victims: &[u64],
    t: &mut Option<u64>,
    owners: &mut Vec<(u64, Option<Owner>)>,
) {
    if t.is_none() && sim.s.tail_probe_diag_ext().1 >= probe_budget() {
        *t = Some(sim.now);
        *owners = victims.iter().map(|&v| (v, owner_of(&sim.s, v))).collect();
    }
}

/// P0 D1 fikstürü (C9 §2 rev4 D1): RTT önce kurulur; kurbanlar son ısınma ACK'inin işlendiği tikte
/// (taze ACK, uçuş boş, PreConverge dışı) en yüksek seq'lerle girer (ilk iletimleri ORİJİNAL
/// Data: ilk kurban o tikte, sonrakiler token'a bağlı olarak taban PTO dolmadan); kurbanların
/// BÜTÜN iletimleri ileri yolda düşer; başka trafik ve yeni uygulama verisi yoktur; gerçek alıcı
/// sessiz dönemde ≈ 8.39 s'de bir bayat SACK tekrarlar (receiver.rs:93-94). Pompa 10 ms (P0
/// gerekçesi).
fn d1_run(sender: Sender, victims: &[u64]) -> D1Run {
    let mut sim = Sim::new(
        sender,
        D1_STEP,
        Path::new(one_way_fixed, None),
        Path::new(one_way_fixed, None),
    );
    let dropped: Vec<u64> = victims.to_vec();
    sim.drop_fwd =
        Box::new(move |d: &TransmitDecision| carries_unit(d.kind) && dropped.contains(&d.unit_id));
    let mut fixture: Vec<String> = Vec::new();
    for id in 0..D1_WARM {
        sim.app(id, D1_UNIT);
    }
    sim.timers();
    let mut t_inject: Option<u64> = None;
    while sim.now < 10 * SEC {
        sim.advance();
        let fresh_ack = sim.last_ack_at == Some(sim.now);
        if fresh_ack && sim.s.inflight_bytes() == 0 && sim.s.state() != SenderState::PreConverge {
            t_inject = Some(sim.now);
            sim.w.units = victims.to_vec();
            for &v in victims {
                sim.app(v, D1_UNIT);
            }
            sim.timers();
            break;
        }
        sim.timers();
    }
    let t0 = t_inject.unwrap_or_default();
    if t_inject.is_none() {
        fixture.push(
            "10 s içinde ısınma ACK'leri uçuşu boşaltmadı / PreConverge'den çıkılmadı".to_string(),
        );
    }
    if sim.s.path_timing_snapshot().srtt_ns.is_none() {
        fixture.push("kurban girerken srtt yok (RTT önce kurulmalı)".to_string());
    }
    let base_pto_ns = sim.s.diag_recovery_timers().base_pto_ns;
    let end = t0 + D1_HORIZON;
    let mut t_exhaust: Option<u64> = None;
    let mut exhaust_owner: Vec<(u64, Option<Owner>)> = Vec::new();
    while sim.now < end {
        sim.advance();
        note_exhaust(&sim, victims, &mut t_exhaust, &mut exhaust_owner);
        sim.timers();
        note_exhaust(&sim, victims, &mut t_exhaust, &mut exhaust_owner);
    }
    // Kurbanların İLK iletimi ORİJİNAL Data olmalı: ilk kurban taze ACK tikinde (t0), sonrakiler
    // hiçbir kurtarma zamanlayıcısı ateşlenmeden (t0 + taban PTO'dan önce) — iki birim aynı turda
    // token'a sığmayabilir (kol 4: ikinci kurban girişten birkaç tik sonra çıkar). Ufuk boyunca
    // sürüldükten sonra denetlenir (token'a bağlı ilk iletim girişten sonraki tiklerde olabilir).
    for (i, &v) in victims.iter().enumerate() {
        let first = sim.unit_tx(v).first().copied();
        let in_window = |t: u64| {
            if i == 0 {
                t == t0
            } else {
                (t0..t0.saturating_add(base_pto_ns)).contains(&t)
            }
        };
        if !first.is_some_and(|e| e.d.kind == TransmitKind::Data && in_window(e.t)) {
            fixture.push(format!(
                "kurban u{v}'in ilk iletimi ORİJİNAL Data değil ya da penceresi dışında \
                 (t0={} ms, taban PTO={} ms; ilk iletim={:?})",
                t0 / MS,
                base_pto_ns / MS,
                first.map(|e| (e.t / MS, e.d.kind))
            ));
        }
    }
    let first_victim_seq = victims.iter().filter_map(|&v| data_seq(&sim.log, v)).min();
    if let (Some(vs), Some(ma)) = (first_victim_seq, sim.max_acked) {
        if ma >= vs {
            fixture.push(format!(
                "kurbandan (seq {vs}) sonra ACK'lenen seq var (en büyük ACK'li seq {ma})"
            ));
        }
    }
    D1Run {
        sim,
        t0,
        end,
        base_pto_ns,
        t_exhaust,
        exhaust_owner,
        fixture,
    }
}

/// D1 sonrası rapor satırı.
fn d1_report(tag: &str, run: &D1Run, victims: &[u64]) {
    for &v in victims {
        let tx = run.sim.unit_tx(v);
        println!(
            "[{tag}] u{v}: t0={} ms taban_PTO={} ms devir={:?} ms; iletimler ({}): {}",
            run.t0 / MS,
            run.base_pto_ns / MS,
            run.t_exhaust.map(|t| t / MS),
            tx.len(),
            fmt_tx(&tx[..tx.len().min(24)])
        );
    }
    let changes: Vec<(u64, u64, Option<Owner>)> = run
        .sim
        .w
        .owner_changes
        .iter()
        .map(|&(t, u, o)| (t / MS, u, o))
        .take(32)
        .collect();
    println!(
        "[{tag}] UYGUNLUK: sahip değişimleri (ms, birim, sahip)={changes:?}; sahipsiz olay={}; \
         K_MAP aşımı={}; durumlar={:?}",
        run.sim.w.orphans.len(),
        run.sim.w.kmap_over.len(),
        run.sim
            .w
            .state_changes
            .iter()
            .map(|&(t, st)| (t / MS, st))
            .take(16)
            .collect::<Vec<_>>()
    );
}

/// Kurbanın `from` sonrasındaki iletim anları.
fn tx_times_after(sim: &Sim, v: u64, from: u64) -> Vec<u64> {
    sim.unit_tx(v)
        .iter()
        .filter(|e| e.t >= from)
        .map(|e| e.t)
        .collect()
}

/// D1 kol 1 — prob AÇIK × gerçek alıcının bayat SACK'i (Ek B.2 D1 (1); A.7 uygunluk türetimi;
/// madde 4 (b) DEVİR, DUR-26 (a): devir ACK sessizliğine dayanmaz).
///
/// AMAÇ: prob bütçesinin tükendiği olayda kurban kopya kuyruğundadır (aynı tur); devir anı ≤ t_arm +
/// Σ_{i<3} min(taban_PTO·2^i, RTO_MAX) + 3 tik (her prob vade tikinde, ≤ 1 tik gecikmeyle; prob
/// token beklemesi bu fikstürde yok — P0 ölçümü probları vade tikinde gördü); devirden sonra
/// ardışık iletim aralığı ≤ RTO_MAX + 2 tik, ufkun sonuna dek; her olayda gözlenebilir sahip var
/// (üç kabul koşulu (i)); her yeniden emisyonda payload var (05c aynası store-miss 0 — koşul (ii)).
/// [CEVAP 11 DUR-30] Birim başına canlı kayıt ≤ K_MAP iddiası 05c'de budamayla birlikte sınanır;
/// burada yalnız raporlanır (05b'de budama kapalı).
///
/// TABANDA: KIRMIZI beklenir (P0 `p0_d1_orphan_after_probe_budget`: 3. probdan sonra 592 s yeniden
/// gönderim yok; bütçe tükenince sahipsiz olaylar).
#[test]
fn k4g05b_d1_probe_on_stale_sack_handover() {
    const TEST: &str = "k4g05b_d1_probe_on_stale_sack_handover";
    let victim = D1_WARM;
    let run = d1_run(product_sender(), &[victim]);
    d1_report("D1-kol1", &run, &[victim]);
    let budget = probe_budget();
    let tx = run.sim.unit_tx(victim);
    let probes = tx
        .iter()
        .filter(|e| e.probe && e.d.kind == TransmitKind::Retransmit)
        .count() as u64;
    let mut fixture = run.fixture.clone();
    if probes != budget || run.t_exhaust.is_none() {
        fixture.push(format!(
            "prob bütçesi tükenmedi: kurban prob emisyonu={probes}, bütçe={budget}, devir={:?}",
            run.t_exhaust
        ));
    }
    assert_fixture(TEST, &fixture);
    let t_ex = run.t_exhaust.unwrap_or_default();
    let w = &run.sim.w;
    assert!(
        w.orphans.is_empty(),
        "{TEST}: INV-NO-ORPHAN — sahipsiz olay(lar) (an ms, birim): {:?}",
        w.orphans
            .iter()
            .take(8)
            .map(|&(t, u)| (t / MS, u))
            .collect::<Vec<_>>()
    );
    assert_eq!(
        run.exhaust_owner,
        [(victim, Some(Owner::Copy))],
        "{TEST}: bütçenin tükendiği olayda kurban kopya kuyruğuna devredilmeli (madde 4 (b))"
    );
    let cap = rto_max_ns();
    let pto_sum: u64 = (0..budget)
        .map(|i| run.base_pto_ns.saturating_mul(1u64 << i).min(cap))
        .sum();
    let bound = run.t0 + pto_sum + budget * D1_STEP;
    assert!(
        t_ex <= bound,
        "{TEST}: devir {} ms > t_arm + Σ PTO_i + 3 tik = {} ms (taban PTO {} ms)",
        t_ex / MS,
        bound / MS,
        run.base_pto_ns / MS
    );
    let after = tx_times_after(&run.sim, victim, t_ex);
    let gap = max_gap(t_ex, &after, run.end);
    assert!(
        gap <= cap + 2 * D1_STEP,
        "{TEST}: devirden sonra en büyük iletim aralığı {} ms > RTO_MAX + 2 tik ({} iletim)",
        gap / MS,
        after.len()
    );
    assert_eq!(
        w.cm.stale_decisions, 0,
        "{TEST}: yeniden emisyonda payload yok (05c aynası store-miss)"
    );
    assert!(
        w.cm.has_payload(victim),
        "{TEST}: teyitsiz kurbanın payload'u silinmemeli"
    );
}

/// D1 kol 2 — prob KAPALI × gerçek alıcının bayat SACK'i (Ek B.2 D1 (2); madde 4 (c) H; EK4-A2 7.
/// uyanma kaynağı).
///
/// AMAÇ: prob kapsamı dışında kurban, ilerleme olmadan son iletiminden H sonra kopya kuyruğuna
/// alınır; bayat SACK H'yi ERTELEMEZ (çapa ilerlemedir). Ölçüt — sahiplenmeden önceki her olayda
/// motor GELECEKTE bir uyanma vadesi açıklar (vadesi geçmiş sahiplenmemiş olay yok:
/// `Watch::h_overdue` boş) ve sahiplenme, son açıklanan vadeden en geç bir tik sonra gözlenir
/// (≤ son iletim + H + 1 tik; bu fikstürde tek uyanma kaynağı H vadesidir); ilk kopya sahiplenmeden
/// en geç iki tik sonra çıkar; sonra ardışık aralık ≤ RTO_MAX + 2 tik; her olayda sahip var.
///
/// TABANDA: KIRMIZI beklenir (H yolu yok; bayat SACK RECOVER_FWD canlılık kolunu engeller → kurban
/// 600 s sahipsiz; uyanma vadesi açıklanmaz).
#[test]
fn k4g05b_d1_probe_off_h_adoption() {
    const TEST: &str = "k4g05b_d1_probe_off_h_adoption";
    let victim = D1_WARM;
    let run = d1_run(product_sender_probe_off(), &[victim]);
    d1_report("D1-kol2", &run, &[victim]);
    let mut fixture = run.fixture.clone();
    if run.sim.s.diag_recovery_timers().tail_enabled {
        fixture.push("kuyruk-probu kapalı olmalı".to_string());
    }
    assert_fixture(TEST, &fixture);
    let w = &run.sim.w;
    assert!(
        w.h_overdue.is_empty(),
        "{TEST}: sahiplenme öncesi vadesi geçmiş/açıklanmamış olay (an ms, birim, vade ms): {:?}",
        w.h_overdue
            .iter()
            .take(8)
            .map(|&(t, u, v)| (t / MS, u, v.map(|x| x / MS)))
            .collect::<Vec<_>>()
    );
    let adopt = w.adopted_at.get(&victim).copied();
    println!(
        "[D1-kol2] H-sahiplenmesi: (an ms, önceki vade ms)={:?}; t0={} ms",
        adopt.map(|(t, v)| (t / MS, v.map(|x| x / MS))),
        run.t0 / MS
    );
    let Some((t_adopt, w_before)) = adopt else {
        panic!("{TEST}: kurban ufuk boyunca kopya kuyruğuna alınmadı (H-sahiplenmesi yok)");
    };
    assert!(
        w_before.is_some_and(|wb| t_adopt <= wb + D1_STEP),
        "{TEST}: sahiplenme {} ms, son açıklanan vadeden ({:?} ms) bir tikten fazla sonra",
        t_adopt / MS,
        w_before.map(|x| x / MS)
    );
    let after = tx_times_after(&run.sim, victim, t_adopt);
    assert!(
        after.first().is_some_and(|&t| t <= t_adopt + 2 * D1_STEP),
        "{TEST}: sahiplenmeden sonra ilk kopya iki tik içinde çıkmadı: {:?}",
        after.first().map(|t| t / MS)
    );
    let gap = max_gap(t_adopt, &after, run.end);
    assert!(
        gap <= rto_max_ns() + 2 * D1_STEP,
        "{TEST}: sahiplenmeden sonra en büyük iletim aralığı {} ms > RTO_MAX + 2 tik",
        gap / MS
    );
    assert!(
        w.orphans.is_empty(),
        "{TEST}: sahipsiz olay(lar): {:?}",
        w.orphans.iter().take(8).collect::<Vec<_>>()
    );
    assert_eq!(w.cm.stale_decisions, 0, "{TEST}: store-miss");
}

/// D1 kol 3 — KONTROL: prob kapalı × gerçek alıcının ≈ 8.39 s'lik bayat SACK tekrarı (CEVAP PAKETİ
/// 10 DUR-26 bağlayıcı okuması: kol bayat SACK'i İÇERİR; tabanda yeşil kontrol beklenmez).
///
/// AMAÇ: kurban girişinden ufuk sonuna dek RECOVER_FWD'ye HİÇ girilmez (bayat SACK `last_ack_ns`'i
/// tazeler, ack_confidence düşmez — sahiplik bu yola dayanmaz) VE owned(U) her olayda (H penceresi →
/// kopya kuyruğu).
///
/// TABANDA: KIRMIZI beklenir (RECOVER_FWD'ye girilmez ama kurban sahipsiz kalır).
#[test]
fn k4g05b_d1_control_stale_sack_no_recover_fwd_owned() {
    const TEST: &str = "k4g05b_d1_control_stale_sack_no_recover_fwd_owned";
    let victim = D1_WARM;
    let run = d1_run(product_sender_probe_off(), &[victim]);
    d1_report("D1-kol3", &run, &[victim]);
    assert_fixture(TEST, &run.fixture);
    let w = &run.sim.w;
    let fwd = w.first_state_at_or_after(SenderState::RecoverFwd, run.t0);
    assert!(
        fwd.is_none(),
        "{TEST}: kol boyunca RECOVER_FWD'ye girildi ({:?} ms) — bayat SACK ack_confidence'ı tutmalıydı",
        fwd.map(|t| t / MS)
    );
    assert!(
        w.orphans.is_empty(),
        "{TEST}: sahipsiz olay(lar): {:?}",
        w.orphans.iter().take(8).collect::<Vec<_>>()
    );
}

/// D1 kol 4 — iki birim (A, B) prob kapsamında; problar ikisine dağılır; bütçenin tükendiği olayda
/// İKİSİ de kopya kuyruğundadır (madde 4 (b): kapsamdaki, geometrik kaydı olmayan HER teyitsiz birim).
///
/// TABANDA: KIRMIZI beklenir (bütçe tükenince ikisi de sahipsiz).
#[test]
fn k4g05b_d1_two_units_handover_same_turn() {
    const TEST: &str = "k4g05b_d1_two_units_handover_same_turn";
    let (a, b) = (D1_WARM, D1_WARM + 1);
    let run = d1_run(product_sender(), &[a, b]);
    d1_report("D1-kol4", &run, &[a, b]);
    let probes_of = |v: u64| {
        run.sim
            .unit_tx(v)
            .iter()
            .filter(|e| e.probe && e.d.kind == TransmitKind::Retransmit)
            .count()
    };
    let mut fixture = run.fixture.clone();
    if probes_of(a) == 0 || probes_of(b) == 0 || run.t_exhaust.is_none() {
        fixture.push(format!(
            "problar iki birime dağılmadı ya da bütçe tükenmedi: A={} B={} devir={:?}",
            probes_of(a),
            probes_of(b),
            run.t_exhaust
        ));
    }
    assert_fixture(TEST, &fixture);
    assert_eq!(
        run.exhaust_owner,
        [(a, Some(Owner::Copy)), (b, Some(Owner::Copy))],
        "{TEST}: bütçenin tükendiği olayda A ve B birlikte devredilmeli"
    );
    let w = &run.sim.w;
    assert!(
        w.orphans.is_empty(),
        "{TEST}: sahipsiz olay(lar): {:?}",
        w.orphans.iter().take(8).collect::<Vec<_>>()
    );
    assert_eq!(w.cm.stale_decisions, 0, "{TEST}: store-miss");
}

// ════════════════════════════════════════════════════════════════════════════════════════════
// Üç kabul koşulu (ii): iki runtime aynası; ufuk dışı geç ACK
// ════════════════════════════════════════════════════════════════════════════════════════════

const HZ_UNIT: u64 = 300;
const HZ_VICTIM: u64 = 4;
const HZ_STEP: u64 = 10 * MS;
const HZ_T_INJECT: u64 = 500 * MS;
/// [CEVAP 11 DUR-30] 05b'de budama kapalı: ORİJİNAL Data kaydı ancak tombstone TTL'iyle (prob
/// emekliliğinden ≈ 8 × 3.2 s sonra) unutulur — geç ACK o süreden sonra verilir (eski 20 s
/// budamaya dayanıyordu).
const HZ_T_LATE: u64 = 60 * SEC;
const HZ_T_FINAL: u64 = 100 * SEC;
const HZ_T_END: u64 = 130 * SEC;

struct HzLate {
    samples_before: u32,
    samples_after: u32,
    confirmed_after: bool,
    owner_after: Option<Owner>,
}

struct HzRun {
    d: Drv,
    t_final_ack: Option<u64>,
    late: Option<HzLate>,
    fixture: Vec<String>,
}

/// Ağsız sürücü: ısınma (4 × 300 B, 300 ms'de ACK) → kurban 500 ms'de tek başına girer, hiçbir
/// iletimi ACK'lenmez (prob → devir → geometrik kopya). `late_ack`: 60 s'de kurbanın ORİJİNAL Data
/// seq'i geç ACK'lenir — o kayıt artık canlı değildir (kanıt: canlı kayıt < iletim sayısı; tombstone
/// TTL'i en eski kaydı önce düşürür; 05b'de K_MAP budaması kapalı) → tanınmayan geç ACK. 100 s'de
/// kurbanın EN SON (canlı) iletimi ACK'lenir → teyit; 130 s'ye dek sürülür.
fn horizon_run(late_ack: bool) -> HzRun {
    let mut d = Drv::new(product_sender());
    let mut fixture: Vec<String> = Vec::new();
    for id in 0..4 {
        d.app(id, HZ_UNIT);
    }
    d.pump_to(300 * MS, HZ_STEP);
    let warm: Vec<u64> = (0..4).filter_map(|id| d.data_seq(id)).collect();
    if warm.len() != 4 {
        fixture.push(format!("ısınma gönderimi eksik: {warm:?}"));
    }
    d.ack(warm);
    if d.s.state() == SenderState::PreConverge {
        fixture.push("ısınma ACK'i PreConverge'i sonlandırmalı".to_string());
    }
    d.pump_to(HZ_T_INJECT, HZ_STEP);
    d.w.units = vec![HZ_VICTIM];
    d.app(HZ_VICTIM, HZ_UNIT);
    let victim_seq = d.data_seq(HZ_VICTIM);
    if victim_seq.is_none() {
        fixture.push("kurban girişte gönderilmedi".to_string());
    }
    d.pump_to(HZ_T_LATE, HZ_STEP);
    let mut late: Option<HzLate> = None;
    if late_ack {
        let r = d.s.diag_unit_recovery(HZ_VICTIM);
        let live = u64::from(r.inflight_records + r.tombstone_records);
        let txs = d.unit_tx(HZ_VICTIM).len() as u64;
        if live >= txs {
            fixture.push(format!(
                "kurbanın ilk kaydı hâlâ canlı olabilir (canlı={live}, iletim={txs}): ufuk dışı değil"
            ));
        }
        let samples_before = d.s.diag_rtt_sample_count();
        if let Some(vs) = victim_seq {
            d.ack(vec![vs]);
        }
        late = Some(HzLate {
            samples_before,
            samples_after: d.s.diag_rtt_sample_count(),
            confirmed_after: d.w.confirmed_set.contains(&HZ_VICTIM),
            owner_after: owner_of(&d.s, HZ_VICTIM),
        });
    }
    d.pump_to(HZ_T_FINAL, HZ_STEP);
    let last_seq = d.unit_tx(HZ_VICTIM).last().map(|e| e.d.seq);
    let mut t_final_ack: Option<u64> = None;
    match last_seq {
        Some(seq) => {
            d.ack(vec![seq]);
            t_final_ack = Some(d.now);
        }
        None => fixture.push("kurbanın canlı son iletimi yok".to_string()),
    }
    d.pump_to(HZ_T_END, HZ_STEP);
    HzRun {
        d,
        t_final_ack,
        late,
        fixture,
    }
}

/// Üç kabul koşulu (ii-b) — 05c-model aynası (Ek B.2 "Üç kabul koşulu testi"; B.2.2 teyit çıktısı).
///
/// AMAÇ: payload YALNIZ teyit çıktısıyla silinir → her yeniden emisyonda payload mevcut (store-miss
/// 0); her teyitli birim çıktıda TAM BİR KEZ (u0..u4), taşma 0; ufuk dışı geç ACK teyit ETMEZ, RTT
/// örneği VERMEZ, birim sahipli kalır, payload kalır ve sonraki yeniden emisyon payload'lu gider;
/// canlı kaydın ACK'i teyit eder ve o andan sonra birim için karar çıkmaz.
///
/// TABANDA: KIRMIZI beklenir (`take_confirmed_units` yok — derlenmez; davranışça D1 sahipsizliği).
#[test]
fn k4g05b_accept_05c_mirror_horizon_out_late_ack() {
    const TEST: &str = "k4g05b_accept_05c_mirror_horizon_out_late_ack";
    let run = horizon_run(true);
    assert_fixture(TEST, &run.fixture);
    let d = &run.d;
    let w = &d.w;
    let tx = d.unit_tx(HZ_VICTIM);
    println!(
        "[ii-b] kurban iletimleri ({}): {}; teyitler={:?}",
        tx.len(),
        fmt_tx(&tx[..tx.len().min(24)]),
        w.confirmed
            .iter()
            .map(|&(t, u)| (t / MS, u))
            .collect::<Vec<_>>()
    );
    let Some(late) = run.late.as_ref() else {
        panic!("{TEST}: geç ACK kolu koşmadı");
    };
    assert!(
        !late.confirmed_after,
        "{TEST}: ufuk dışı geç ACK birimi teyit etti (Ek B.2.2 (ii))"
    );
    assert_eq!(
        late.samples_after, late.samples_before,
        "{TEST}: ufuk dışı geç ACK RTT örneği verdi"
    );
    assert!(
        late.owner_after.is_some(),
        "{TEST}: ufuk dışı geç ACK'ten sonra birim sahipsiz"
    );
    let t_final = run.t_final_ack.unwrap_or(u64::MAX);
    let between = tx
        .iter()
        .filter(|e| (HZ_T_LATE + 1..t_final).contains(&e.t))
        .count();
    assert!(
        between >= 1,
        "{TEST}: geç ACK ile son ACK arasında yeniden emisyon yok"
    );
    assert_eq!(
        w.cm.stale_decisions, 0,
        "{TEST}: yeniden emisyonda payload yok (store-miss)"
    );
    for u in 0..=HZ_VICTIM {
        assert_eq!(
            w.cm.confirms(u),
            1,
            "{TEST}: u{u} teyit çıktısında tam bir kez olmalı"
        );
    }
    assert_eq!(w.cm.unknown_confirms, 0, "{TEST}: bilinmeyen teyit");
    assert_eq!(
        d.s.diag_confirmed_units_overflow(),
        0,
        "{TEST}: teyit çıktısı taşması (drenaj sözleşmesi)"
    );
    let after_confirm = tx.iter().filter(|e| e.t > t_final).count();
    assert_eq!(
        after_confirm, 0,
        "{TEST}: teyitten sonra birim için karar çıktı"
    );
    assert!(
        w.orphans.is_empty(),
        "{TEST}: sahipsiz olay(lar): {:?}",
        w.orphans.iter().take(8).collect::<Vec<_>>()
    );
}

/// Üç kabul koşulu (ii-a) — P0 runtime aynası (bugünkü runtime: eşlenen seq'in ACK'inde siler),
/// ufuk İÇİ: her silme anında birim teyit çıktısındadır; P0 aynasında store-miss 0.
///
/// TABANDA: KIRMIZI beklenir (teyit çıktısı yok; davranışça D1 sahipsizliği).
#[test]
fn k4g05b_accept_p0_mirror_deletions_confirmed() {
    const TEST: &str = "k4g05b_accept_p0_mirror_deletions_confirmed";
    let run = horizon_run(false);
    assert_fixture(TEST, &run.fixture);
    let w = &run.d.w;
    assert!(
        w.p0_unconfirmed_deletions.is_empty(),
        "{TEST}: P0 aynası teyit çıktısında olmayan birimi sildi: {:?}",
        w.p0_unconfirmed_deletions
    );
    assert_eq!(w.rt.stale_decisions, 0, "{TEST}: P0 aynasında store-miss");
}

// [CEVAP PAKETİ 10 DUR-22 düzeltmesi] Üç kabul koşulu (ii-a)'nın UFUK DIŞI geç ACK kolu (P0 aynası;
// 05b'de beyanlı kırmızı) 05c şeridinin testidir: 05b'de `#[ignore]` YOK. Metni BİREBİR
// `kama-k4g-work/k4g05c/05b-den-tasinan/k4g05b_accept_p0_mirror_horizon_out_declared_red.rs`
// dosyasına taşındı; 05c runtime silmeyi teyit çıktısına bağlayınca orada `#[ignore]`suz koşar
// (yeşil beklenir).

// ════════════════════════════════════════════════════════════════════════════════════════════
// K_MAP
// ════════════════════════════════════════════════════════════════════════════════════════════

/// K_MAP budaması 05b'de KAPALI (CEVAP PAKETİ 11 DUR-30: sabit ve `take_confirmed_units` tanımlı,
/// budama davranışı yok; budama K4G-05c'de runtime ile birlikte açılır — budamalı sürüm
/// `k4g05b_kmap_bound_and_pruned_late_ack` 05c'ye taşındı).
///
/// AMAÇ: kurbanın son prob kaydından sonra en az K_MAP iletim çıkınca kayıt sayısı K_MAP'i aşar
/// (budama yok — fikstür ön koşulu); o prob kaydı UNUTULMAZ: 200 s'deki geç ACK'i canlı kayıtla
/// eşleşir, birimi teyit eder (teyit çıktısında tam bir kez) ve teyitten sonra birim için karar
/// çıkmaz; kopya işi ≤ D_max × birim, her olayda sahip var.
///
/// TABANDA: KIRMIZI beklenir (D1: bütçe tükenince kurban sahipsiz, kopya zinciri yok).
#[test]
fn k4g05b_kmap_pruning_off_old_record_late_ack_confirms() {
    const TEST: &str = "k4g05b_kmap_pruning_off_old_record_late_ack_confirms";
    const T_LATE: u64 = 200 * SEC;
    const T_END: u64 = 260 * SEC;
    let mut d = Drv::new(product_sender());
    let mut fixture: Vec<String> = Vec::new();
    for id in 0..4 {
        d.app(id, HZ_UNIT);
    }
    d.pump_to(300 * MS, HZ_STEP);
    let warm: Vec<u64> = (0..4).filter_map(|id| d.data_seq(id)).collect();
    d.ack(warm);
    d.pump_to(HZ_T_INJECT, HZ_STEP);
    d.w.units = vec![HZ_VICTIM];
    d.app(HZ_VICTIM, HZ_UNIT);
    d.pump_to(T_LATE, HZ_STEP);
    let tx = d.unit_tx(HZ_VICTIM);
    let last_probe = tx
        .iter()
        .rposition(|e| e.probe && e.d.kind == TransmitKind::Retransmit);
    let copies_after = last_probe.map_or(0, |i| tx.len() - i - 1) as u64;
    if last_probe.is_none() || copies_after < k_map() {
        fixture.push(format!(
            "son prob kaydından sonra en az K_MAP iletim olmalı (prob={:?}, sonrası={copies_after})",
            last_probe.map(|i| tx[i].d.seq)
        ));
    }
    let r = d.s.diag_unit_recovery(HZ_VICTIM);
    let live = u64::from(r.inflight_records + r.tombstone_records);
    if live <= k_map() {
        fixture.push(format!(
            "budama kapalıyken canlı kayıt K_MAP'i aşmalı (canlı={live}, K_MAP={})",
            k_map()
        ));
    }
    assert_fixture(TEST, &fixture);
    let probe_seq = last_probe.map(|i| tx[i].d.seq).unwrap_or_default();
    d.ack(vec![probe_seq]);
    let t_confirm = d.now;
    assert!(
        d.w.confirmed_set.contains(&HZ_VICTIM),
        "{TEST}: K_MAP'ten eski kaydın (seq {probe_seq}) geç ACK'i birimi teyit etmedi — budama 05b'de kapalı"
    );
    d.pump_to(T_END, HZ_STEP);
    let after = d
        .unit_tx(HZ_VICTIM)
        .iter()
        .filter(|e| e.t > t_confirm)
        .count();
    println!(
        "[K_MAP kapalı] canlı kayıt={live} (K_MAP={}); prob-3 seq={probe_seq}; teyitten sonra karar={after}",
        k_map()
    );
    let w = &d.w;
    assert_eq!(
        w.cm.confirms(HZ_VICTIM),
        1,
        "{TEST}: kurban teyit çıktısında tam bir kez olmalı"
    );
    assert_eq!(after, 0, "{TEST}: teyitten sonra birim için karar çıktı");
    assert!(
        w.copy_work_over.is_empty(),
        "{TEST}: kopya işi D_max × birimi aştı: {:?}",
        w.copy_work_over.iter().take(4).collect::<Vec<_>>()
    );
    assert!(
        w.orphans.is_empty(),
        "{TEST}: sahipsiz olay(lar): {:?}",
        w.orphans.iter().take(8).collect::<Vec<_>>()
    );
}

// ════════════════════════════════════════════════════════════════════════════════════════════
// ESI ve parite akıbeti (DUR-25 (a): statik-r kolu)
// ════════════════════════════════════════════════════════════════════════════════════════════

const PAR_UNIT: u64 = 100;

/// Statik-r blok 0: 8 kaynak (kimlik 0..7) + 4 parite (ESI 0..3) PreConverge'de; parite seq'leri.
fn parity_block(d: &mut Drv, fixture: &mut Vec<String>) -> Vec<(u64, u64)> {
    for id in 0..FEC_K {
        d.app(id, PAR_UNIT);
    }
    let auth = d.s.open_fec_block_authority();
    let closed = d.s.close_fec_block_manual(auth, 0, FEC_R, PAR_UNIT);
    if closed != Ok(FEC_R) {
        fixture.push(format!(
            "statik-r blok kapanışı {closed:?} döndü (beklenen Ok({FEC_R}))"
        ));
    }
    d.pump_to(MS, MS);
    let parity = d.kind_seqs(TransmitKind::FecParity);
    if parity.len() != usize::from(FEC_R) {
        fixture.push(format!("parite ESI 0..{FEC_R} gönderilmedi: {parity:?}"));
    }
    parity
}

/// ESI ve parite akıbeti — İLANSIZ kol (madde 1; Ek B.2 "ESI ve parite akıbeti", B.2.3 parite süre
/// dolumu).
///
/// AMAÇ: 8 kaynak ACK'lenince uçuşta yalnız parite kalır; parite sahiplenilmez (prob hedefi değil,
/// ARQ'ya girmez, kopyası yok — teyitten sonra 0..7 kimlikli yeniden emisyon YOK), kayıp ilan
/// edilmez ve gönderiminden K_MAP × RTO_MAX sonra Expired düşer (öncesinde uçuşta: `inflight_bytes`
/// = 4 × 100; sonrasında 0; `diag_parity_expired()` = 4); süresi dolan paritenin geç ACK'i teyit,
/// tombstone isabeti ve RTT örneği VERMEZ; kaynaklar teyit çıktısında tam bir kez.
///
/// TABANDA: KIRMIZI beklenir (parite süresiz uçuşta; prob pariteyi hedefler — P0 ESI-2).
#[test]
fn k4g05b_parity_fate_undeclared_expires() {
    const TEST: &str = "k4g05b_parity_fate_undeclared_expires";
    const STEP: u64 = 100 * MS;
    let mut d = Drv::new(product_sender_static_r());
    let mut fixture: Vec<String> = Vec::new();
    let parity = parity_block(&mut d, &mut fixture);
    let data: Vec<u64> = (0..FEC_K).filter_map(|id| d.data_seq(id)).collect();
    d.pump_to(300 * MS, 10 * MS);
    d.ack(data);
    let t_conf = d.now;
    if d.s.inflight_bytes() != u64::from(FEC_R) * PAR_UNIT {
        fixture.push(format!(
            "teyitten sonra uçuşta yalnız parite kalmalı: inflight_bytes={}",
            d.s.inflight_bytes()
        ));
    }
    assert_fixture(TEST, &fixture);
    let t_parity = d
        .log
        .iter()
        .find(|e| e.d.kind == TransmitKind::FecParity)
        .map_or(0, |e| e.t);
    let horizon = k_map() * rto_max_ns();
    d.pump_to(t_parity + horizon - SEC, STEP);
    let before = (d.s.inflight_bytes(), d.s.diag_parity_expired());
    d.pump_to(t_parity + horizon + SEC, STEP);
    let after = (d.s.inflight_bytes(), d.s.diag_parity_expired());
    println!(
        "[ESI ilansız] parite(seq,esi)={parity:?}; ufuk {} s; (inflight, süre_dolumu) önce={before:?} sonra={after:?}",
        horizon / SEC
    );
    assert_eq!(
        before,
        (u64::from(FEC_R) * PAR_UNIT, 0),
        "{TEST}: parite ufuktan ÖNCE uçuşta kalmalı"
    );
    assert_eq!(
        after,
        (0, u64::from(FEC_R)),
        "{TEST}: parite K_MAP × RTO_MAX sonra Expired düşmeli"
    );
    let offending: Vec<Emit> = d
        .log
        .iter()
        .filter(|e| e.t > t_conf && is_reemission(e.d.kind) && e.d.unit_id < FEC_K)
        .copied()
        .collect();
    assert!(
        offending.is_empty(),
        "{TEST}: teyitli 0..7 kimliği için yeniden emisyon (parite sahiplenildi): {}",
        fmt_tx(&offending)
    );
    assert_eq!(
        d.s.diag_repair_loss_not_arqed(),
        0,
        "{TEST}: parite kayıp ilan edilmemeliydi"
    );
    let confirmed_before = d.w.confirmed.len();
    let hits_before = d.s.diag_tombstone().1;
    let samples_before = d.s.diag_rtt_sample_count();
    d.ack(parity.iter().map(|p| p.0).collect());
    assert_eq!(
        d.w.confirmed.len(),
        confirmed_before,
        "{TEST}: süresi dolan paritenin geç ACK'i teyit üretti"
    );
    assert_eq!(
        d.s.diag_tombstone().1,
        hits_before,
        "{TEST}: süresi dolan paritenin geç ACK'i tombstone'a çarptı"
    );
    assert_eq!(
        d.s.diag_rtt_sample_count(),
        samples_before,
        "{TEST}: parite ACK'i RTT örneği verdi"
    );
    for u in 0..FEC_K {
        assert_eq!(d.w.cm.confirms(u), 1, "{TEST}: u{u} tam bir kez teyit");
    }
}

/// ESI ve parite akıbeti — İLANLI kol (P0 ESI-1 tombstone kolunun kurulumu).
///
/// AMAÇ: u0..u4 kaynakları kayıp, 5..11 ACK'lenir → parite kayıp ilan edilir, ARQ'ya GİRMEZ
/// (`diag_repair_loss_not_arqed` = 4) ve tombstone'a geçer. Kol A: TTL içinde geç parite ACK'i
/// tombstone'a çarpar ama ESI ile aynı kimlikli u0..u3'ü TEYİT ETMEZ (hiçbir kaynağı ACK'lenmedi).
/// Kol B: TTL dolduktan sonra (tombstone'lar düşmüş) geç parite ACK'i isabet ETMEZ ve teyit üretmez.
///
/// TABANDA: KIRMIZI beklenir (geç parite ACK'i u0..u3'ü acked_units'e koyar — P0 ESI-1).
#[test]
fn k4g05b_parity_fate_declared_tombstone() {
    const TEST: &str = "k4g05b_parity_fate_declared_tombstone";
    const LOST: u64 = 5;
    let setup = |fixture: &mut Vec<String>| -> (Drv, Vec<u64>) {
        let mut d = Drv::new(product_sender_static_r());
        let parity = parity_block(&mut d, fixture);
        d.pump_to(2 * MS, MS);
        for id in FEC_K..FEC_K + 4 {
            d.app(id, PAR_UNIT);
        }
        let delivered: Vec<u64> = (LOST..FEC_K + 4).filter_map(|id| d.data_seq(id)).collect();
        if delivered.len() as u64 != FEC_K + 4 - LOST {
            fixture.push(format!("teslim edilecek veri seq eksik: {delivered:?}"));
        }
        d.pump_to(300 * MS, 10 * MS);
        d.ack(delivered);
        if d.s.diag_repair_loss_not_arqed() != u64::from(FEC_R) {
            fixture.push(format!(
                "parite kayıp ilanıyla tombstone'a girmeliydi: repair_loss_not_arqed={}",
                d.s.diag_repair_loss_not_arqed()
            ));
        }
        (d, parity.iter().map(|p| p.0).collect())
    };
    let mut fixture: Vec<String> = Vec::new();
    // Kol A — TTL içinde geç parite ACK'i.
    let (mut a, parity_a) = setup(&mut fixture);
    a.pump_to(SEC, 10 * MS);
    let hits_before = a.s.diag_tombstone().1;
    a.ack(parity_a);
    let hits_a = a.s.diag_tombstone().1.saturating_sub(hits_before);
    if hits_a != u64::from(FEC_R) {
        fixture.push(format!(
            "kol A: geç parite ACK'i tombstone'a çarpmalıydı: hit={hits_a}"
        ));
    }
    // Kol B — TTL sonrası geç parite ACK'i.
    let (mut b, parity_b) = setup(&mut fixture);
    b.pump_to(120 * SEC, 100 * MS);
    if b.s.diag_tombstone().0 != 0 {
        fixture.push(format!(
            "kol B: 120 s'de tombstone'lar düşmüş olmalı (canlı={})",
            b.s.diag_tombstone().0
        ));
    }
    assert_fixture(TEST, &fixture);
    let wrongly: Vec<u64> = (0..LOST)
        .filter(|u| a.w.confirmed_set.contains(u))
        .collect();
    assert!(
        wrongly.is_empty(),
        "{TEST}: kol A — parite ACK'i kaynakları ACK'lenmemiş birimleri teyit etti: {wrongly:?}"
    );
    let hits_b_before = b.s.diag_tombstone().1;
    let confirmed_b_before = b.w.confirmed.len();
    b.ack(parity_b);
    assert_eq!(
        b.s.diag_tombstone().1,
        hits_b_before,
        "{TEST}: kol B — TTL sonrası geç parite ACK'i tombstone'a çarptı"
    );
    assert_eq!(
        b.w.confirmed.len(),
        confirmed_b_before,
        "{TEST}: kol B — TTL sonrası geç parite ACK'i teyit üretti"
    );
}

// ════════════════════════════════════════════════════════════════════════════════════════════
// B2 ekleme kuralı
// ════════════════════════════════════════════════════════════════════════════════════════════

/// B2 ekleme kuralı (madde 5; C8 DUR-13; Ek B.2 "B2 ekleme kolu").
///
/// Kurulum: kuyruk-probu kapalı, FEC `Manual { enabled: false }` (Tiny framing → Dup3'te D = 3);
/// ısınma ACK'i (srtt ≈ 10 ms, yüksek BtlBw) → U sessizlikte uçuşta → RECOVER_FWD, Dup3. Aynı anda
/// X ve Y1..Y3 girer: RECOVER_FWD DATA'sı olarak her biri D − 1 tek seferlik kopya alır (vade
/// +20/+40 ms — henüz bekliyor). 1 ms sonra Y1..Y3 ACK'lenir → X paket eşiğiyle kayıp ilan edilir
/// ve RECOVER_FWD kayıp kolu X için yeniden D kopya ister.
///
/// AMAÇ: bekleyen tek seferlik kayıt ≤ D − 1 tutulur → X'in kopya kaydı her olayda ≤ D_max = 3
/// (X'in kaydı = kuyruk boyu − U'nun tek geometrik kaydı; Y'ler teyitle düşer).
///
/// TABANDA: KIRMIZI beklenir (v1 kayıp kolu D kayıt daha ekler: 2 + 2 = 4 > 3).
#[test]
fn k4g05b_b2_insertion_bound() {
    const TEST: &str = "k4g05b_b2_insertion_bound";
    const UNIT: u64 = 100;
    const U: u64 = 1;
    const X: u64 = 2;
    let mut s = product_sender_probe_off();
    let tiny = s.set_fec_actuation_mode(FecActuationMode::Manual { enabled: false });
    let mut d = Drv::new(s);
    let mut fixture: Vec<String> = Vec::new();
    if tiny.is_err() {
        fixture.push("FEC Manual{false} kipi kabul edilmedi".to_string());
    }
    d.app(0, 1200);
    d.now = 10 * MS;
    d.ack(vec![0]);
    d.now = 20 * MS;
    d.app(U, UNIT);
    let mut t1: Option<u64> = None;
    while d.now < 16 * SEC {
        d.now += 10 * MS;
        d.tick();
        if d.s.state() == SenderState::RecoverFwd
            && d.s.emergency_level() == adaptive_transport::emergency::EmergencyLevel::Dup3
        {
            t1 = Some(d.now);
            break;
        }
    }
    if t1.is_none() {
        fixture.push("16 s içinde RECOVER_FWD/Dup3'e girilmedi".to_string());
    }
    let ids = [X, X + 1, X + 2, X + 3];
    for &id in &ids {
        d.app(id, UNIT);
    }
    let seqs: Vec<Option<u64>> = ids.iter().map(|&id| d.data_seq(id)).collect();
    if seqs.iter().any(|s| s.is_none()) {
        fixture.push(format!("X, Y1..Y3 aynı anda gönderilmedi: {seqs:?}"));
    }
    let x_pending = d.s.diag_unit_recovery(X).dup_due_ns;
    if x_pending.is_none() {
        fixture.push("X'in RECOVER_FWD tek seferlik kopyası kurulmadı".to_string());
    }
    assert_fixture(TEST, &fixture);
    d.w.units = vec![U, X];
    let loss_before = d.s.diag_loss().0;
    d.now += MS;
    d.ack(seqs[1..].iter().flatten().copied().collect());
    let declared = d.s.diag_loss().0 > loss_before;
    let x_records = |d: &Drv| {
        let queued = u64::from(d.s.diag_recovery_timers().dup_queue_len);
        let u_has = u64::from(d.s.diag_dup_queue_units().contains(&U));
        queued.saturating_sub(u_has)
    };
    let mut worst = x_records(&d);
    let t_ack = d.now;
    while d.now < t_ack + 5 * SEC {
        d.now += 10 * MS;
        d.tick();
        worst = worst.max(x_records(&d));
    }
    println!(
        "[B2] RECOVER_FWD/Dup3 t1={:?} ms; X kayıp ilanı={declared}; X'in azami kopya kaydı={worst}; \
         U geometrik={:?}",
        t1.map(|t| t / MS),
        d.s.diag_unit_recovery(U).dup_interval_ns
    );
    assert!(
        declared,
        "{TEST}: FİKSTÜR GEÇERSİZ — X kayıp ilan edilmedi (Y'lerin ACK'i paket eşiğini aşmalı)"
    );
    assert!(
        worst <= d_max(),
        "{TEST}: X'in bekleyen kopya kaydı {worst} > D_max {}",
        d_max()
    );
    assert!(
        d.w.copy_work_over.is_empty(),
        "{TEST}: kopya işi D_max × birimi aştı: {:?}",
        d.w.copy_work_over.iter().take(4).collect::<Vec<_>>()
    );
}

// ════════════════════════════════════════════════════════════════════════════════════════════
// H — sahipsizlik süresinin çapası
// ════════════════════════════════════════════════════════════════════════════════════════════

/// `h_run` dönüşü: kurbanın ilk kopya kuyruğu anı (girişten), vadesi geçmiş olaylar, fikstür.
struct HRun {
    adopted: Option<u64>,
    overdue: Vec<(u64, u64)>,
    fixture: Vec<String>,
}

/// H sahiplenmesi için tek kurban + isteğe bağlı bayat ACK'ler / ilerleme ACK'i (prob kapalı).
///
/// İlerleme kolunda W, ısınmayla birlikte t=0'da çıkan beşinci birimdir: seq'i kurbanınkinden
/// KÜÇÜKTÜR, bu yüzden W'nin ACK'i kurbanı kayıp eşiğine sokmaz (sahiplenme ARQ'ya kaymaz; ölçülen
/// yalnız çapadır). W girişten `progress_at` sonra İLK kez ACK'lenir — gerçek ilerleme (canlı kaydın
/// ilk onayı); `ack_delay` alıcının tutma süresini (`progress_at`) taşır → örnek ısınma RTT'sine
/// eşittir, srtt ve dolayısıyla H değişmez.
fn h_run(stale_every: Option<u64>, progress_at: Option<u64>) -> HRun {
    const UNIT: u64 = 300;
    const STEP: u64 = 10 * MS;
    const VICTIM: u64 = 4;
    const W: u64 = 5;
    let mut d = Drv::new(product_sender_probe_off());
    let mut fixture: Vec<String> = Vec::new();
    for id in 0..4 {
        d.app(id, UNIT);
    }
    if progress_at.is_some() {
        d.app(W, UNIT);
    }
    let w_seq = progress_at.and_then(|_| d.data_seq(W));
    if progress_at.is_some() && w_seq.is_none() {
        fixture.push("W ısınmayla birlikte t=0'da gönderilmedi".to_string());
    }
    d.pump_to(400 * MS, STEP);
    let warm: Vec<u64> = (0..4).filter_map(|id| d.data_seq(id)).collect();
    let stale_seq = warm.first().copied().unwrap_or_default();
    d.ack(warm);
    // Kurban, son ilerlemeli ACK'in olay anında girer (çapa = son iletim = son ilerleme).
    d.w.units = vec![VICTIM];
    d.app(VICTIM, UNIT);
    let t_start = d.now;
    let victim_seq = d
        .unit_tx(VICTIM)
        .first()
        .filter(|e| e.t == t_start && e.d.kind == TransmitKind::Data)
        .map(|e| e.d.seq);
    match (victim_seq, w_seq) {
        (None, _) => fixture.push("kurban girişte Data olarak gönderilmedi".to_string()),
        (Some(v), Some(w)) if w >= v => fixture.push(format!(
            "W'nin seq'i ({w}) kurbanınkinden ({v}) küçük değil: W'nin ACK'i kurbanı kayıp \
             eşiğine sokar"
        )),
        _ => {}
    }
    while d.now < t_start + 120 * SEC {
        d.now += STEP;
        d.tick();
        if stale_every.is_some_and(|p| (d.now - t_start).is_multiple_of(p)) {
            d.ack(vec![stale_seq]);
        }
        if let (Some(tp), Some(seq)) = (progress_at, w_seq) {
            if d.now == t_start + tp {
                d.ack_with_delay(vec![seq], Duration::from_nanos(tp));
                if !d.w.confirmed_set.contains(&W) {
                    fixture.push("W'nin ACK'i ilerleme değil (W teyit edilmedi)".to_string());
                }
            }
        }
        if d.w.adopted_at.contains_key(&VICTIM) {
            break;
        }
    }
    HRun {
        adopted: d.w.adopted_at.get(&VICTIM).map(|&(t, _)| t - t_start),
        overdue: d.w.h_overdue.iter().map(|&(t, u, _)| (t, u)).collect(),
        fixture,
    }
}

/// H — sahipsizlik süresinin çapası `max(son ilerleme, son iletim)` (madde 4 (c); AMENDMENT-12;
/// DUR-26 (a)).
///
/// AMAÇ: (1) bayat ACK (yeni onay yok, 2 s'de bir < H) H sahiplenmesini ERTELEMEZ — çapa ACK
/// varışı olsaydı sahiplenme hiç olmazdı; bayat ACK'li koşuda sahiplenme OLUR; (2) ilerlemeli ACK
/// (ikinci birim W — ısınmayla t=0'da çıkar, seq'i kurbanınkinden küçük; girişten 2 s sonra İLK kez
/// ACK'lenir, örnek ısınma RTT'sine eşit) çapayı ilerletir — sahiplenme bayat ACK'li koşudan en az
/// 1 s SONRA (2 s'lik çapa ilerlemesi; H'nin kanal türevli oynaması payı); (3) üç koşuda da
/// sahiplenmeden önce vadesi geçmiş ya da açıklanmamış olay yok. Bayat/ilerlemesiz yalın koşu
/// (RECOVER_FWD girişiyle aynı eşik) yalnız raporlanır.
///
/// TABANDA: KIRMIZI beklenir (H yolu yok: bayat ACK'li ve ilerlemeli koşularda sahiplenme hiç olmaz).
#[test]
fn k4g05b_h_anchor_stale_ack_vs_progress() {
    const TEST: &str = "k4g05b_h_anchor_stale_ack_vs_progress";
    let plain_run = h_run(None, None);
    let stale_run = h_run(Some(2 * SEC), None);
    let progress_run = h_run(None, Some(2 * SEC));
    for (tag, run) in [
        ("yalın", &plain_run),
        ("bayat", &stale_run),
        ("ilerleme", &progress_run),
    ] {
        assert_fixture(&format!("{TEST} [{tag}]"), &run.fixture);
    }
    let (plain, over_a) = (plain_run.adopted, &plain_run.overdue);
    let (stale, over_b) = (stale_run.adopted, &stale_run.overdue);
    let (progress, over_c) = (progress_run.adopted, &progress_run.overdue);
    println!(
        "[H] sahiplenme (girişten ms): yalın={:?} bayat-ACK={:?} ilerleme@2s={:?}",
        plain.map(|t| t / MS),
        stale.map(|t| t / MS),
        progress.map(|t| t / MS)
    );
    let (Some(s), Some(g)) = (stale, progress) else {
        panic!(
            "{TEST}: H sahiplenmesi gözlenmedi (bayat ACK ya da ilerleme çapayı kilitledi): \
             bayat={stale:?} ilerleme={progress:?}"
        );
    };
    assert!(
        g >= s + SEC,
        "{TEST}: ilerleme çapayı ilerletmedi: bayat-ACK {} ms, ilerleme {} ms",
        s / MS,
        g / MS
    );
    for (tag, over) in [("yalın", over_a), ("bayat", over_b), ("ilerleme", over_c)] {
        assert!(
            over.is_empty(),
            "{TEST}: {tag} koşusunda vadesi geçmiş sahiplenmemiş olay: {:?}",
            over.iter().take(4).collect::<Vec<_>>()
        );
    }
}

// ════════════════════════════════════════════════════════════════════════════════════════════
// D2 — iki yönlü kesinti ve sıfırlama (Ek B.2 "D2 simgeleri/iddiaları")
// ════════════════════════════════════════════════════════════════════════════════════════════

const D2_UNIT: u64 = 1000;
const D2_N: u64 = 5;
const D2_STEP: u64 = 10 * MS;
const D2_LINK_BPS: u64 = 1064;
const D2_T_OUT: u64 = 20 * SEC;
/// Kol 2 ek DATA'sının ilk varışı (kesinti başından sonra) ve aralığı: 1000 B / 65 s ≈ 15.4 B/s ≤
/// min_rate/2 = 15.5 B/s.
const D2_TRICKLE_OFFSET: u64 = 5 * SEC;
const D2_TRICKLE_PERIOD: u64 = 65 * SEC;

struct D2Run {
    sim: Sim,
    t_r: u64,
    /// T_a: T_r sonrası ilk ilerlemeli ACK (ilk teyit anı).
    t_a: Option<u64>,
    /// T_a sonrası en eski teyitsiz birim ve T_a anındaki srtt (kopya tabanı).
    u0: Option<u64>,
    srtt_at_a: Option<u64>,
    /// T_a'yı içeren adımın sonunda u₀'nın kopya aralığı (sıfırlama kanıtı) ve vadesi (T_H).
    u0_interval_after_a: Option<u64>,
    t_h: Option<u64>,
    /// Kesinti başında (T_out) uçuştaki kurban kaydı sayısı (R₀; yalnız rapor).
    r0: u32,
    /// Kesinti başındaki srtt (kesinti boyunca örnek yok → kopya tabanı b).
    srtt_at_out: Option<u64>,
    fixture: Vec<String>,
}

/// Darboğaz 1064 B/s, tek yön 200 ms: ısınma (4 × 1000 B, BtlBw kurulur); T_out = 20 s'de u4..u8
/// aynı anda sunulur ve kesinti [T_out, T_out + Δ) iki yönde düşürür (token sınırı: birim başına
/// ≈ 1 s — K4); `trickle` ise kesinti boyunca aynı boy ek DATA ≤ min_rate/2 sunulur (kol 2).
/// Kesinti sonrası ilk geçen iletim alıcıya ulaşır → T_a.
fn d2_run(outage: u64, trickle: bool, horizon_after: u64) -> D2Run {
    let mut sim = Sim::new(
        product_sender(),
        D2_STEP,
        Path::new(one_way_fixed, Some(D2_LINK_BPS)),
        Path::new(one_way_fixed, None),
    );
    let mut fixture: Vec<String> = Vec::new();
    for id in 0..4 {
        sim.app(id, D2_UNIT);
    }
    sim.timers();
    while sim.now < D2_T_OUT {
        sim.advance();
        sim.timers();
    }
    if (0..4).any(|u| !sim.w.confirmed_set.contains(&u)) {
        fixture.push("ısınma birimleri kesintiden önce teyit edilmedi".to_string());
    }
    let t_r = D2_T_OUT + outage;
    let srtt_at_out = sim.s.path_timing_snapshot().srtt_ns;
    sim.outage = Some((D2_T_OUT, t_r));
    let victims: Vec<u64> = (4..4 + D2_N).collect();
    sim.w.units = victims.clone();
    for &v in &victims {
        sim.app(v, D2_UNIT);
    }
    let r0: u32 = victims
        .iter()
        .map(|&v| sim.s.diag_unit_recovery(v).inflight_records)
        .sum();
    let mut next_id = 4 + D2_N;
    let end = t_r + horizon_after;
    let mut t_a: Option<u64> = None;
    let mut u0: Option<u64> = None;
    let mut srtt_at_a: Option<u64> = None;
    let mut u0_interval_after_a: Option<u64> = None;
    let mut t_h: Option<u64> = None;
    while sim.now < end {
        sim.advance();
        let first_trickle = D2_T_OUT + D2_TRICKLE_OFFSET;
        if trickle
            && (first_trickle..t_r).contains(&sim.now)
            && (sim.now - first_trickle).is_multiple_of(D2_TRICKLE_PERIOD)
        {
            sim.w.units.push(next_id);
            sim.app(next_id, D2_UNIT);
            next_id += 1;
        }
        sim.timers();
        if t_a.is_none() {
            let first = sim
                .w
                .confirmed
                .iter()
                .find(|&&(t, _)| t >= t_r)
                .map(|&(t, _)| t);
            if let Some(ta) = first {
                t_a = Some(ta);
                srtt_at_a = sim.s.path_timing_snapshot().srtt_ns;
                u0 = sim
                    .w
                    .units
                    .iter()
                    .copied()
                    .filter(|u| !sim.w.confirmed_set.contains(u))
                    .min();
                u0_interval_after_a = u0.and_then(|u| sim.s.diag_unit_recovery(u).dup_interval_ns);
                t_h = u0.and_then(|u| sim.s.diag_unit_recovery(u).dup_due_ns);
            }
        }
    }
    if t_a.is_none() {
        fixture.push("kesinti sonrası ilerlemeli ACK (T_a) gözlenmedi".to_string());
    }
    if u0.is_none() {
        fixture.push("T_a'da teyitsiz birim kalmadı (sıfırlama sınanamaz)".to_string());
    }
    D2Run {
        sim,
        t_r,
        t_a,
        u0,
        srtt_at_a,
        u0_interval_after_a,
        t_h,
        r0,
        srtt_at_out,
        fixture,
    }
}

/// D2 kol 1/2 iddiaları (Ek B.2 "D2 iddiaları"; madde 6 sıfırlama). T_H = u₀'nın T_a adımı sonundaki
/// kopya vadesi (sıfırlamadan sonra); u₀ o adımda zaten gönderildiyse sınırlar kendiliğinden sağlanır.
fn d2_assert(test: &str, run: &D2Run) {
    assert_fixture(test, &run.fixture);
    let (Some(t_a), Some(u0), Some(srtt)) = (run.t_a, run.u0, run.srtt_at_a) else {
        panic!("{test}: T_a/u₀/srtt yok");
    };
    let log = &run.sim.log;
    let first_dup = log
        .iter()
        .find(|e| e.t >= t_a && e.d.kind == TransmitKind::Duplicate)
        .copied();
    let u0_emit = log
        .iter()
        .find(|e| e.t >= t_a && e.d.unit_id == u0 && is_reemission(e.d.kind))
        .copied();
    let u0_dup = log
        .iter()
        .find(|e| e.t >= t_a && e.d.unit_id == u0 && e.d.kind == TransmitKind::Duplicate)
        .copied();
    let t_h = run.t_h.unwrap_or(t_a).max(t_a);
    // HOL: u₀'nın vadesinden (T_H) sonra, u₀'nın kopyasından ÖNCE daha büyük kimlikli birimin kopyası
    // çıkmaz (kopya kuyruğu içi sıra: en eski onaysız birim önce; engine §13 v1.1). İstisna: u₀'nın
    // başka sınıfla (ARQ) gönderildiği tur — tur içi birim dışlaması (beyanlı) baş olarak u₀'yu atlar.
    let u0_other: Vec<u64> = log
        .iter()
        .filter(|e| e.t >= t_a && e.d.unit_id == u0 && e.d.kind == TransmitKind::Retransmit)
        .map(|e| e.t)
        .collect();
    let overtakers: Vec<(u64, u64)> = match u0_dup {
        Some(ud) => log
            .iter()
            .filter(|e| {
                (t_h..=ud.t).contains(&e.t)
                    && e.d.seq < ud.d.seq
                    && e.d.kind == TransmitKind::Duplicate
                    && e.d.unit_id > u0
                    && !u0_other.contains(&e.t)
            })
            .map(|e| (e.t / MS, e.d.unit_id))
            .collect(),
        None => Vec::new(),
    };
    let x_bytes: u64 = match u0_emit {
        Some(e) => log
            .iter()
            .filter(|x| (t_h..=e.t).contains(&x.t) && x.d.kind != TransmitKind::Duplicate)
            .map(|x| x.d.size)
            .sum(),
        None => 0,
    };
    let budget_ns = (D2_UNIT + x_bytes) * SEC / MIN_RATE_BPS + MS + K_ARQ_SHARE as u64 * D2_STEP;
    println!(
        "[{test}] T_r={} ms T_a={} ms u₀=u{u0} T_H={} ms srtt={} ms aralık(T_a)={:?} ms; ilk Duplicate={:?}; \
         u₀ emisyonu={:?}; u₀ kopyası={:?}; sollayan={overtakers:?}; X={x_bytes} B; R₀={}",
        run.t_r / MS,
        t_a / MS,
        t_h / MS,
        srtt / MS,
        run.u0_interval_after_a.map(|v| v / MS),
        first_dup.map(|e| (e.t / MS, e.d.unit_id)),
        u0_emit.map(|e| (e.t / MS, e.d.kind)),
        u0_dup.map(|e| e.t / MS),
        run.r0
    );
    assert!(
        run.u0_interval_after_a.is_some_and(|i| i <= 2 * srtt),
        "{test}: T_a'dan sonra u₀'nın kopya aralığı tabana dönmedi (sıfırlama): {:?} ms",
        run.u0_interval_after_a.map(|v| v / MS)
    );
    assert!(
        u0_dup.is_some(),
        "{test}: T_a sonrası u₀'nın kopyası çıkmadı"
    );
    assert!(
        overtakers.is_empty(),
        "{test}: u₀'nın vadesinden sonra daha büyük kimlikli kopya u₀'yu soldu (ms, birim): {overtakers:?}"
    );
    assert!(
        u0_emit.is_some_and(|e| e.t <= t_h + budget_ns),
        "{test}: u₀ emisyonu ≤ T_H + ⌈(S + X)/min_rate⌉ + 1 ms + K_ARQ_SHARE tur sınırını aştı"
    );
    let w = &run.sim.w;
    assert!(
        w.orphans.is_empty(),
        "{test}: sahipsiz olay(lar): {:?}",
        w.orphans.iter().take(8).collect::<Vec<_>>()
    );
    assert_eq!(w.cm.stale_decisions, 0, "{test}: store-miss");
}

/// D2 kol 1 — 60 s iki yönlü kesinti, kesintide yeni veri yok.
///
/// AMAÇ (Ek B.2 "D2 iddiaları"): kesinti H'yi aştığından T_a'daki ilk ilerlemeli ACK geometrik
/// kopya aralıklarını tabana döndürür (u₀'nın aralığı ≤ 2·srtt); u₀'nın vadesinden (T_H) sonra
/// u₀'nın kopyasından önce daha büyük kimlikli kopya çıkmaz (kopya kuyruğu içi sıra: en eski
/// onaysız birim önce); u₀ emisyonu ≤ T_H + ⌈(S + X)/min_rate⌉ + 1 ms + K_ARQ_SHARE tur; owned her
/// olayda; store-miss 0. Sapmalar (beyanlı): u₀ = T_a SONRASI en eski teyitsiz birim (T_r'deki en
/// eski birim T_a'yı üreten kopyayla teyit edilebilir — o durumda iddia boş geçerdi); notun "T_a
/// sonrası ilk Duplicate u₀'nın" cümlesi HOL biçiminde sınanır (u₀ kopyası T_a'dan hemen önce
/// çıkmışsa vadesi T_a + taban'dır ve vadesi gelmiş başka birim önce gidebilir — sıra ihlali değil;
/// ilk Duplicate raporlanır); K2–K4 koşulları raporlanır, zorlanmaz.
///
/// TABANDA: KIRMIZI beklenir (D1 sahipsizliği; sıfırlama yok).
#[test]
fn k4g05b_d2_outage_60s_reset_order() {
    let run = d2_run(60 * SEC, false, 120 * SEC);
    d2_assert("k4g05b_d2_outage_60s_reset_order", &run);
}

/// D2 kol 2 — kol 1 + kesinti boyunca aynı boy DATA ≤ min_rate/2 (1000 B / 65 s ≈ 15.4 B/s; ilk
/// varış kesinti başından 5 s sonra). AMAÇ ve TABAN: kol 1 ile aynı.
#[test]
fn k4g05b_d2_outage_60s_with_trickle() {
    let run = d2_run(60 * SEC, true, 120 * SEC);
    d2_assert("k4g05b_d2_outage_60s_with_trickle", &run);
}

/// D2 kol 3 — 600 s iki yönlü kesinti (DUR-2 (a); Ek B.2 "D2 iddiaları" kol 3).
///
/// AMAÇ: her olayda kopya işi ≤ D_max × birim; kesinti boyunca
/// her kurbanın kopya zinciri emisyonu ≤ m + ⌊Δ/RTO_MAX⌋ + 2 (m = ⌈log2(RTO_MAX/b)⌉, b = devirdeki
/// srtt; +2 = ilk kopya ve pencere kenarı); owned her olayda. [CEVAP 11 DUR-30] Birim başına canlı
/// kayıt ≤ K_MAP iddiası 05c'de budamayla birlikte sınanır; burada yalnız raporlanır.
///
/// TABANDA: KIRMIZI beklenir [aritmetik: v1 zinciri 600 s'de 10 kopya (×2 tavansız); D1
/// sahipsizliği].
#[test]
fn k4g05b_d2_outage_600s_bounds() {
    const TEST: &str = "k4g05b_d2_outage_600s_bounds";
    let outage = 600 * SEC;
    let run = d2_run(outage, false, 10 * SEC);
    let mut fixture = run.fixture.clone();
    fixture.retain(|f| !f.contains("T_a") && !f.contains("sıfırlama"));
    assert_fixture(TEST, &fixture);
    let w = &run.sim.w;
    let srtt = run.srtt_at_out.unwrap_or(SEC).max(1);
    let ratio = rto_max_ns().div_ceil(srtt);
    let m = u64::from(u64::BITS - ratio.saturating_sub(1).leading_zeros());
    let bound = m + outage / rto_max_ns() + 2;
    let mut worst = 0u64;
    for v in 4..4 + D2_N {
        let n = run
            .sim
            .unit_tx(v)
            .iter()
            .filter(|e| e.d.kind == TransmitKind::Duplicate && (D2_T_OUT..run.t_r).contains(&e.t))
            .count() as u64;
        worst = worst.max(n);
    }
    println!(
        "[D2-kol3] srtt={} ms m={m} sınır={bound}; birim başına azami kopya emisyonu={worst}; \
         K_MAP aşımı={} kopya işi aşımı={}",
        srtt / MS,
        w.kmap_over.len(),
        w.copy_work_over.len()
    );
    assert!(
        w.copy_work_over.is_empty(),
        "{TEST}: kopya işi D_max × birimi aştı: {:?}",
        w.copy_work_over.iter().take(4).collect::<Vec<_>>()
    );
    assert!(
        worst <= bound,
        "{TEST}: kopya zinciri emisyonu {worst} > m + ⌊Δ/RTO_MAX⌋ + 2 = {bound}"
    );
    assert!(
        worst >= 1,
        "{TEST}: kesinti boyunca kopya emisyonu yok (sahiplik yürümedi)"
    );
    assert!(
        w.orphans.is_empty(),
        "{TEST}: sahipsiz olay(lar): {:?}",
        w.orphans.iter().take(8).collect::<Vec<_>>()
    );
}
