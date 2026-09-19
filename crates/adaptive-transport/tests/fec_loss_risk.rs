//! [FEC-RISK] Deneysel FEC kayıp-riski tahmincisi.
//!
//! Bu dosya üç şeyi savunur:
//!  (a) kol KAPALIYKEN davranış bugünküyle özdeş (kanonik kayıp tek girdi),
//!  (b) risk sinyali `Suspected` olaylara yaş-ağırlıklı güven verir ve `Spurious` gelince
//!      katkı KENDİLİĞİNDEN sıfırlanır,
//!  (c) sinyal ARQ/CC/pacing yollarına SIZMAZ — tek tüketici FEC oranı kararıdır.
//!
//! (c) bu kolun tüm meşruiyetidir: kanonik kayıp yeniden tanımlanmıyor, ayrı bir okuma ekleniyor.

use adaptive_transport::loss::{LossDetector, SuspectConfidence, ThresholdLossDetector};
use adaptive_transport::sender::*;
use adaptive_transport::sender::{
    fec_arbiter, learned_persist_min_ns, FecArbiterReason, PersistMode, SpeculativeLevel,
    SpeculativeProposal,
};
use std::time::Duration;

const MS: u64 = 1_000_000;

/// Akış sürücüsü: birim sun, RTT örneği ver, sonra timer pompala; `acked` kayıp deseni imal eder.
fn drive(mut s: Sender, units: u64, acked: Vec<u64>) -> Sender {
    s.set_establish_probe(false);
    for i in 0..units {
        s.on_event(
            Event::AppData(OpaqueUnit {
                id: 1000 + i,
                size: 101,
            }),
            0,
        );
    }
    for t in 1..=20u64 {
        s.on_event(Event::TimerFired(Timer::Pto), t * 200 * MS);
    }
    s.on_event(
        Event::AckReceived(AckInfo {
            acked_seqs: vec![0],
            ack_delay: Duration::ZERO,
        }),
        4_100 * MS,
    );
    for t in 21..=220u64 {
        s.on_event(Event::TimerFired(Timer::Pto), t * 200 * MS);
    }
    s.on_event(
        Event::AckReceived(AckInfo {
            acked_seqs: acked,
            ack_delay: Duration::ZERO,
        }),
        45_000 * MS,
    );
    for t in 221..=320u64 {
        s.on_event(Event::TimerFired(Timer::Pto), t * 200 * MS);
    }
    s
}

fn gap_pattern() -> Vec<u64> {
    (1..400).filter(|q| q % 2 == 0).collect()
}

// ── (a) Kol KAPALI = sevk davranışı ──────────────────────────────────────────────────────────

#[test]
fn kol_varsayilan_olarak_kapalidir() {
    assert!(
        !Sender::new().fec_loss_risk_config().enabled,
        "deneysel kol default AÇIK olamaz — ölçülmemiş davranış sevk edilmez"
    );
}

/// Kol kapalıyken risk teşhisi hiç yazılmaz ve FEC oranı kanonik kayıptan gelir.
#[test]
fn kapali_kolda_risk_hesaplanmaz() {
    let s = drive(Sender::new(), 80, gap_pattern());
    assert_eq!(s.diag_risk_ppm(), (0, 0), "kapalı kolda risk ölçülmemeli");
    let (_, _, t_risk, _) = s.diag_fec_risk_times();
    assert_eq!(t_risk, 0, "kapalı kolda risk damgası basılmamalı");
}

// ── (b) Risk sinyali erken ve düzeltilebilir ─────────────────────────────────────────────────

/// Kolun ÇEKİRDEK iddiası: risk, onaydan ÖNCE sıfırdan çıkar. Aynı akış, tek eksen = bayrak.
#[test]
fn risk_onaydan_once_sifirdan_cikar() {
    let mut on = Sender::new();
    on.set_fec_loss_risk(FecLossRiskConfig::experimental_v1());
    let on = drive(on, 80, gap_pattern());

    let (t_susp, t_conf, t_risk, _) = on.diag_fec_risk_times();
    assert!(t_susp > 0, "önkoşul: kayıp kuşkusu ilan edilmeli");
    assert!(t_risk > 0, "risk sinyali sıfırdan ÇIKMALI");
    assert!(
        t_risk >= t_susp,
        "risk kuşkudan önce doğamaz (nedensellik): risk={t_risk} susp={t_susp}"
    );
    if t_conf > 0 {
        assert!(
            t_risk <= t_conf,
            "kolun TEK amacı bu: risk onaydan ÖNCE görünmeli (risk={t_risk} conf={t_conf})"
        );
    }
    assert!(on.diag_risk_ppm().1 > 0, "tepe risk sıfırdan büyük olmalı");
}

/// Ağırlık eğrisi monoton: yaşlanan kuşku daha çok güven taşır, ama onayın (1000‰) altında kalır.
#[test]
fn guven_egrisi_monoton_ve_onayin_altinda() {
    let w = SuspectConfidence::default();
    let srtt = 200 * MS;
    let a = w.for_age(srtt / 8, srtt); // < srtt/4
    let b = w.for_age(srtt / 3, srtt); // < srtt/2
    let c = w.for_age(3 * srtt / 4, srtt); // < srtt
    let d = w.for_age(2 * srtt, srtt); // >= srtt
    assert!(a < b && b < c, "eğri monoton artmalı: {a} {b} {c}");
    assert!(
        c <= d && d < 1000,
        "onaylanmamış kuşku ASLA tam güven (1000‰) almamalı: {d}"
    );
}

/// srtt ölçülmemişken kuşkuya güven VERİLMEZ → sonuç kanonikle özdeş. Ölçüm yokken risk uydurulmaz
/// (bu, düzeltmeye çalıştığımız "prior'dan eşik türetme" kusurunun tekrarı olurdu).
#[test]
fn srtt_yokken_risk_kanonikle_ozdes() {
    let mut d = ThresholdLossDetector::default();
    d.on_sent(1, 0, 1);
    d.on_sent(2, 0, 1);
    d.on_sent(3, 0, 1);
    d.on_sent(4, 0, 1);
    let _ = d.on_ack(&[4], 0);
    let _ = d.detect_time_losses(10 * MS, Duration::from_millis(1), Duration::from_millis(1));
    let canon = d.loss_windowed_ppm();
    let risk = d.fec_loss_risk_ppm(20 * MS, 0, SuspectConfidence::default());
    assert_eq!(risk, canon, "srtt yokken risk kanonik değerden SAPAMAZ");
}

/// Geç ACK gelince (`Spurious`) katkı sıfırlanır — tahmin kendi hatasını düzeltir.
/// Bu, reorder'lı kanalda yanlış-pozitif parity bedelinin ÜST SINIRINI belirleyen mekanizmadır.
#[test]
fn spurious_katkiyi_sifirlar() {
    let w = SuspectConfidence::default();
    let srtt = 200 * MS;
    let mut d = ThresholdLossDetector::default();
    for seq in 1..=8u64 {
        d.on_sent(seq, 0, 1);
    }
    // 8'e ACK → 1..7 arasındaki boşluklar paket-eşiğiyle kayıp ilan edilir.
    let _ = d.on_ack(&[8], 0);
    let _ = d.detect_time_losses(MS, Duration::from_millis(1), Duration::from_millis(1));
    let before = d.fec_loss_risk_ppm(50 * MS, srtt, w);
    assert!(before > 0, "önkoşul: kuşku riski sıfırdan büyük olmalı");

    // Geç ACK'ler gelir → olaylar Spurious'a döner.
    let _ = d.on_ack(&[1, 2, 3, 4, 5, 6, 7], 60 * MS);
    let after = d.fec_loss_risk_ppm(70 * MS, srtt, w);
    assert_eq!(after, 0, "geç ACK gelen olaylar riske katkı VERMEMELİ");
}

// ── (c) İZOLASYON — sinyal FEC dışına sızmıyor ───────────────────────────────────────────────

/// Kolun meşruiyet kanıtı: bayrak AÇIK/KAPALI arasında ARQ agresifliği, mod, pacing hızı,
/// bant tahmini ve state-machine fazı DEĞİŞMEZ. Değişmesine izin verilen tek şey FEC oranıdır.
#[test]
fn risk_sinyali_arq_cc_ve_pacing_yollarina_sizmaz() {
    let off = drive(Sender::new(), 80, gap_pattern());
    let mut on_s = Sender::new();
    on_s.set_fec_loss_risk(FecLossRiskConfig::experimental_v1());
    let on = drive(on_s, 80, gap_pattern());

    // Önkoşul: kol gerçekten çalıştı (aksi hâlde test vakum olurdu).
    assert!(on.diag_risk_ppm().1 > 0, "önkoşul: risk sinyali üretilmeli");
    // KAPSAM (denetim bulgusu): bu sürücüde FEC profili YOK → parity hiç emit edilmez; eşitlikler
    // "AKTÜASYONSUZ mekanik izolasyon"u pinler. Parity gerçekten emit edildiğinde hız düzlemi
    // eşitliği yapısal olarak tutamaz (parity inflight'a girer) — o, FEC'in doğal bedelidir.
    assert_eq!(
        on.diag_fec_actuation().emitted_r_total,
        0,
        "kapsam: aktüasyonsuz sürüş"
    );

    assert_eq!(
        off.state(),
        on.state(),
        "state-machine fazı risk sinyalinden ETKİLENMEMELİ"
    );
    assert_eq!(
        off.diag_rate(),
        on.diag_rate(),
        "hız düzlemi (state, pacing_rate, btlbw, cwnd, inflight) risk sinyalinden \
         ETKİLENMEMELİ — INV-RATE-1'in mekanik kanıtı"
    );
    assert_eq!(
        off.diag_ledger(),
        on.diag_ledger(),
        "kanonik kayıp defteri (unloss, loss_ppm, spurious) DEĞİŞMEMELİ"
    );
    assert_eq!(
        off.diag_confirm_path(),
        on.diag_confirm_path(),
        "onay yolu (T_confirm, confirmed_total) DEĞİŞMEMELİ — ARQ semantiği korunur"
    );
}

/// Kanonik metrik yeniden tanımlanmadı: `loss_windowed_ppm` hâlâ YALNIZ `Confirmed` sayar.
#[test]
fn kanonik_kayip_yeniden_tanimlanmadi() {
    let w = SuspectConfidence::default();
    let srtt = 200 * MS;
    let mut d = ThresholdLossDetector::default();
    for seq in 1..=8u64 {
        d.on_sent(seq, 0, 1);
    }
    let _ = d.on_ack(&[8], 0);
    let _ = d.detect_time_losses(MS, Duration::from_millis(1), Duration::from_millis(1));
    assert_eq!(
        d.loss_windowed_ppm(),
        0,
        "kanonik metrik Suspected'ı SAYMAMALI (ARQ semantiği)"
    );
    assert!(
        d.fec_loss_risk_ppm(50 * MS, srtt, w) > 0,
        "risk metriği ise Suspected'ı görmeli — iki metrik AYRI sorular sorar"
    );
}

// ── [FEC-RISK v2] ARBİTER + SINIRLI SPEKÜLATİF BOOTSTRAP ────────────────────────────────────
//
// Owner tasarımı (2026-09-05): "belirsizliği bekleyerek çözme; belirsizlik arttıkça eylemin
// büyüklüğünü sınırla." Kollar yalnız ÖNERİR; parity'yi tek yer (arbiter) yazar; max, toplama değil.

fn sp(level: SpeculativeLevel, ratio: u32, dl: bool) -> SpeculativeProposal {
    SpeculativeProposal {
        level,
        ratio_permille: ratio,
        deadline_assisted: dl,
    }
}

/// max, TOPLAMA DEĞİL: iki kol aynı kaybı görüp parity'yi iki kez sayamaz.
#[test]
fn arbiter_max_alir_toplamaz() {
    let c = (250, FecArbiterReason::ConfirmedFuzzy);
    let s = sp(SpeculativeLevel::L2Persistent, 250, false);
    let (r, why) = fec_arbiter(c, 200, s);
    assert_eq!(r, 250, "toplam olsaydı 700 olurdu");
    assert_eq!(
        why,
        FecArbiterReason::ConfirmedFuzzy,
        "eşitlikte yerleşik yol kazanır"
    );
}

/// Sebep atfı doğru kolu gösterir; spekülatif L1/L2 ve deadline ayrışır.
#[test]
fn arbiter_sebep_atfi_dogru() {
    let none = (0, FecArbiterReason::ConfirmedFuzzy);
    assert_eq!(
        fec_arbiter(none, 0, sp(SpeculativeLevel::L0None, 0, false)).1,
        FecArbiterReason::None
    );
    assert_eq!(
        fec_arbiter(none, 500, sp(SpeculativeLevel::L0None, 0, false)).1,
        FecArbiterReason::StructureGate
    );
    assert_eq!(
        fec_arbiter(none, 0, sp(SpeculativeLevel::L1Bootstrap, 125, false)).1,
        FecArbiterReason::SpeculativeBootstrap
    );
    assert_eq!(
        fec_arbiter(none, 0, sp(SpeculativeLevel::L2Persistent, 250, false)).1,
        FecArbiterReason::SpeculativePersistent
    );
    assert_eq!(
        fec_arbiter(none, 0, sp(SpeculativeLevel::L1Bootstrap, 250, true)).1,
        FecArbiterReason::DeadlineAssisted
    );
    assert_eq!(
        fec_arbiter(
            (100, FecArbiterReason::ConfirmedPolicy),
            0,
            sp(SpeculativeLevel::L0None, 0, false)
        )
        .1,
        FecArbiterReason::ConfirmedPolicy
    );
}

/// Temiz yol: hiç olay yokken split sıfırdır → spekülatif kol hiçbir şey öneremez.
/// Tahminci kayıp İCAT EDEMEZ — yalnız declare()'ın ürettiği olayları ağırlıklandırır.
#[test]
fn temiz_yolda_split_sifir() {
    let d = ThresholdLossDetector::default();
    let r = d.fec_loss_risk_split(1_000 * MS, 200 * MS, SuspectConfidence::default(), 100 * MS);
    assert_eq!(
        (
            r.instant_ppm,
            r.persistent_ppm,
            r.reorder_pressure_permille,
            r.declared_units
        ),
        (0, 0, 0, 0)
    );
}

/// Persistence gerçek kaybı reorder'dan ZAMANLA ayırır: taze gap persistent'a girmez, yaşlanınca girer.
#[test]
fn persistence_yasla_devreye_girer() {
    let w = SuspectConfidence::default();
    let srtt = 200 * MS;
    let mut d = ThresholdLossDetector::default();
    for seq in 1..=8u64 {
        d.on_sent(seq, 0, 1);
    }
    let _ = d.on_ack(&[8], 0);
    let _ = d.detect_time_losses(MS, Duration::from_millis(1), Duration::from_millis(1));
    let fresh = d.fec_loss_risk_split(20 * MS, srtt, w, srtt / 2); // yaş 20 ms < 100 ms
    assert!(
        fresh.instant_ppm > 0 && fresh.persistent_ppm == 0,
        "taze gap: instant>0, persistent=0: {fresh:?}"
    );
    let aged = d.fec_loss_risk_split(150 * MS, srtt, w, srtt / 2); // yaş 150 ms ≥ 100 ms
    assert!(
        aged.persistent_ppm > 0,
        "yaşlanan gap persistent'a girmeli: {aged:?}"
    );
    assert!(
        aged.instant_ppm >= fresh.instant_ppm,
        "instant yaşla düşmez"
    );
}

/// Reorder baskısı: geç ACK gelen olaylar Spurious olur → reorder_pressure yükselir, instant düşer.
#[test]
fn spurious_reorder_baskisini_yukseltir() {
    let w = SuspectConfidence::default();
    let srtt = 200 * MS;
    let mut d = ThresholdLossDetector::default();
    for seq in 1..=8u64 {
        d.on_sent(seq, 0, 1);
    }
    let _ = d.on_ack(&[8], 0);
    let _ = d.detect_time_losses(MS, Duration::from_millis(1), Duration::from_millis(1));
    let before = d.fec_loss_risk_split(50 * MS, srtt, w, srtt / 2);
    assert_eq!(before.reorder_pressure_permille, 0);
    let _ = d.on_ack(&[1, 2, 3, 4, 5, 6, 7], 60 * MS);
    let after = d.fec_loss_risk_split(70 * MS, srtt, w, srtt / 2);
    assert_eq!(
        after.reorder_pressure_permille, 1000,
        "tüm gap'ler doldu → tam baskı"
    );
    assert_eq!(after.instant_ppm, 0, "Spurious katkı vermez");
    assert_eq!(
        after.declared_units, before.declared_units,
        "payda pencereyle aynı kalır"
    );
}

/// Reorder indirimi TAVANI düşürür ama K=8'de bile ≥1 repair'e YUVARLANIR (discount, veto değil).
/// Denetim bulgusu: eski test yalnız permille>0 pinliyordu; `125×0.5=62‰` r=0'a yuvarlanıyordu.
#[test]
fn reorder_indirimi_veto_degil() {
    let cfg = FecLossRiskConfig::default();
    let disc = u64::from(1000u32) * u64::from(cfg.reorder_discount_max_permille) / 1000;
    let discounted =
        u32::try_from(u64::from(cfg.bootstrap_cap_permille) * (1000 - disc.min(999)) / 1000)
            .unwrap();
    let floored = discounted.max(cfg.spec_min_permille);
    assert!(
        discounted < cfg.spec_min_permille,
        "önkoşul: taban olmasa yuvarlama 0 verirdi ({discounted}‰)"
    );
    assert!(
        compute_requested_r(discounted, 8, 4).1 == 0,
        "tabansız: K=8'de VETO"
    );
    assert!(
        compute_requested_r(floored, 8, 4).1 >= 1,
        "tabanlı: K=8'de ≥1 repair ({floored}‰)"
    );
}

/// Kol AÇIK + kayıp deseni → spekülatif seviyeler ve arbiter sebepleri histogramda görünür;
/// kanonik yol (mode/arq/state/pacing/ledger/onay) OFF ile ÖZDEŞ kalır (izolasyon v2).
#[test]
fn v2_izolasyon_ve_gorunurluk() {
    let off = drive(Sender::new(), 80, gap_pattern());
    let mut on_s = Sender::new();
    on_s.set_fec_loss_risk(FecLossRiskConfig::experimental_v1());
    let on = drive(on_s, 80, gap_pattern());

    let lv = on.diag_spec_levels();
    assert!(
        lv[1] + lv[2] > 0,
        "önkoşul: spekülatif kol öneri üretmeli: {lv:?}"
    );
    let arb = on.diag_arbiter_reasons();
    assert!(
        arb.iter().sum::<u64>() > 0,
        "arbiter her karar için sebep sayar"
    );

    assert_eq!(off.state(), on.state());
    assert_eq!(
        off.diag_rate(),
        on.diag_rate(),
        "hız düzlemi değişmez (INV-RATE-1)"
    );
    assert_eq!(
        off.diag_ledger(),
        on.diag_ledger(),
        "kanonik defter değişmez"
    );
    assert_eq!(
        off.diag_confirm_path(),
        on.diag_confirm_path(),
        "onay yolu değişmez"
    );
    // OFF kolunda arbiter yalnız kanonik sebepleri sayar, spekülatif sebep SIFIR.
    let arb_off = off.diag_arbiter_reasons();
    assert_eq!(
        arb_off[4] + arb_off[5] + arb_off[6],
        0,
        "OFF: spekülatif sebep olamaz: {arb_off:?}"
    );
}

/// Politika AÇIKKEN spekülatif kol politikanın sert tavanını AŞAMAZ ve güvenlik geri dönüşünde
/// SUSAR — lane'ler arası max() politikanın "asla daha agresif değil" sözleşmesini bozamaz.
/// Kanıt: politika `hard_max=0` ile açık → spekülatif sebep histogramda SIFIR.
#[test]
fn politika_acikken_spekulatif_kol_tavani_asamaz() {
    use adaptive_transport::fusion_policy::{FusionPolicy, FusionSafetyConfig};
    let mut s = Sender::new();
    s.set_fec_loss_risk(FecLossRiskConfig::experimental_v1());
    s.set_fusion_policy(
        FusionPolicy::calibration_v2_deadline().with_safety(FusionSafetyConfig {
            hard_max_redundancy_permille: 0,
            ..FusionSafetyConfig::default()
        }),
    );
    let s = drive(s, 80, gap_pattern());
    let lv = s.diag_spec_levels();
    assert!(lv[0] > 0, "önkoşul: kol koştu");
    let arb = s.diag_arbiter_reasons();
    assert_eq!(
        arb[4] + arb[5] + arb[6],
        0,
        "sert tavan 0 → spekülatif kol parity öneremez: {arb:?}"
    );
    assert_eq!(s.last_decision().fec_parity_ratio_permille, 0);
}

/// Politika AÇIK + risk KAPALI: arbiter politikanın kararını BİREBİR geçirir (eski erken-return
/// yolunun byte-özdeşliği). Politika `diag_fusion_actions` ile sayılır (arbiter sıfır kararı
/// tasarım gereği `None` kovasına yazar — `ConfirmedPolicy` yalnız FecAssist'te görünür);
/// spekülatif/gate/fuzzy sebebi SIFIR; oran politikanın {0, 250} kümesinden.
#[test]
fn politika_acik_risk_kapali_arbiter_politikayi_birebir_gecirir() {
    use adaptive_transport::fusion_policy::FusionPolicy;
    let mut s = Sender::new();
    s.set_fusion_policy(FusionPolicy::calibration_v2_deadline());
    s.declare_delivery_deadline(400_000_000);
    let s = drive(s, 80, gap_pattern());
    let fa = s.diag_fusion_actions();
    assert!(
        fa.iter().sum::<u64>() > 0,
        "önkoşul: politika her karar için sayıldı: {fa:?}"
    );
    let arb = s.diag_arbiter_reasons();
    assert_eq!(
        arb[1] + arb[3] + arb[4] + arb[5] + arb[6],
        0,
        "politika açıkken fuzzy/gate/spekülatif sebep olamaz: {arb:?}"
    );
    assert_eq!(
        arb[0] + arb[2],
        arb.iter().sum::<u64>(),
        "yalnız None ya da ConfirmedPolicy"
    );
    let r = s.last_decision().fec_parity_ratio_permille;
    assert!(
        r == 0 || r == 250,
        "oran politikanın {{0, 250}} kümesinden olmalı: {r}"
    );
}

/// [HANDOFF İNVARYANTI] Kanonik lane hiçbir şey ÖNERMİYORKEN ilk Confirmed'ın gelmesi spekülatif
/// öneriyi DÜŞÜREMEZ. Ölçüldü (handoff ailesi, n=3): eski klamp t_conf'ta 250→125‰ uçurumu üretiyordu.
///
/// Desen BURST (ardışık kayıp): yapı-gate `BlockedBurstLeftToArq` → structure=0; fuzzy bu sürüşte 0.
/// Böylece "kanonik önermiyor" önkoşulu sağlanır (IID desende gate açılır, test sonuçsuz olurdu).
/// İddia örnek-başına koşulludur ve vakum-olmama şartı vardır.
#[test]
fn handoff_devralan_yoksa_spekulatif_dusmez() {
    let mut s = Sender::new();
    s.set_establish_probe(false);
    s.set_fec_loss_risk(FecLossRiskConfig::experimental_v1());
    // burst: 40..120 arası ardışık kayıp, gerisi ACK.
    let acked: Vec<u64> = (1..400).filter(|q| !(40..120).contains(q)).collect();
    let mut samples = Vec::new();
    let mut step = |s: &mut Sender, ev: Event, t: u64| {
        s.on_event(ev, t);
        samples.push(s.diag_ts_sample());
    };
    for i in 0..80u64 {
        step(
            &mut s,
            Event::AppData(OpaqueUnit {
                id: 1000 + i,
                size: 101,
            }),
            0,
        );
    }
    for t in 1..=20u64 {
        step(&mut s, Event::TimerFired(Timer::Pto), t * 200 * MS);
    }
    step(
        &mut s,
        Event::AckReceived(AckInfo {
            acked_seqs: vec![0],
            ack_delay: Duration::ZERO,
        }),
        4_100 * MS,
    );
    for t in 21..=220u64 {
        step(&mut s, Event::TimerFired(Timer::Pto), t * 200 * MS);
    }
    step(
        &mut s,
        Event::AckReceived(AckInfo {
            acked_seqs: acked,
            ack_delay: Duration::ZERO,
        }),
        45_000 * MS,
    );
    for t in 221..=320u64 {
        step(&mut s, Event::TimerFired(Timer::Pto), t * 200 * MS);
    }
    let first_conf = samples
        .iter()
        .position(|x| x.confirmed_ppm > 0)
        .expect("önkoşul: Confirmed gelmeli");
    let pre_max = samples[..first_conf]
        .iter()
        .map(|x| x.spec_prop)
        .max()
        .unwrap_or(0);
    assert!(pre_max > 0, "önkoşul: onaydan önce spekülatif öneri > 0");
    // Onay sonrası, kanonik lane ÖNERMEZKEN ve seviye L2 iken alınan örnekler:
    let quiet: Vec<u32> = samples[first_conf..]
        .iter()
        .filter(|x| x.confirmed_prop == 0 && x.structure_prop == 0 && x.level_idx == 2)
        .map(|x| x.spec_prop)
        .collect();
    assert!(
        quiet.len() >= 8,
        "vakum: onay sonrası 'kanonik önermiyor ∧ L2' örneği yok ({} adet)",
        quiet.len()
    );
    let post_min = *quiet.iter().min().unwrap();
    assert!(
        post_min >= pre_max,
        "HANDOFF CLIFF: kanonik lane önermezken onay sonrası L2 önerisi düştü: pre_max={pre_max} post_min={post_min}"
    );
}

// ── [PERSIST] Öğrenilen reorder ufku ─────────────────────────────────────────────────────────
//
// Ölçüldü (saf reorder, RTT 200): spurious yaşları 91–131 ms, sabit eşik 100–110 ms → çakışma
// inşa gereği (netem'de reorder ömrü OWD'ye bağlı). Ufuk kanal-türevli olmalı; L1 anında kalmalı.

/// Quantile deterministik: yaşlar ilan→un-loss ile üretilir (test-özel API yok).
/// ack(8) packet-threshold (K=3) ile yalnız seq 1..5'i ilan eder (6,7 gap<3; time-threshold 1.125 ms
/// > 1 ms geçen süre) → 5 olay. Un-loss zamanları = yaşlar: 80,90,100,110,120 ms.
#[test]
fn spurious_yas_quantile_deterministik() {
    let mut d = ThresholdLossDetector::default();
    for seq in 1..=8u64 {
        d.on_sent(seq, 0, 1);
    }
    let _ = d.on_ack(&[8], 0);
    let _ = d.detect_time_losses(MS, Duration::from_millis(1), Duration::from_millis(1));
    assert_eq!(d.spurious_age_quantile(950), None, "örnek yokken None");
    for (i, seq) in (1..=5u64).enumerate() {
        let _ = d.on_ack(&[seq], (80 + 10 * i as u64) * MS);
    }
    assert_eq!(d.spurious_age_samples(), 5);
    assert_eq!(
        d.spurious_age_quantile(500),
        Some(100 * MS),
        "medyan: rank ceil(2.5)=3 → 100 ms"
    );
    assert_eq!(
        d.spurious_age_quantile(950),
        Some(120 * MS),
        "Q95 küçük n'de ≈ maksimum"
    );
    assert_eq!(d.spurious_age_quantile(1000), Some(120 * MS));
    assert_eq!(
        d.spurious_age_quantile(200),
        Some(80 * MS),
        "Q20: rank ceil(1.0)=1 → 80 ms"
    );
}

/// Ufuk = max(taban, ρ·Q); örnek yoksa taban; tavan varsa tavan.
#[test]
fn ogrenilen_ufuk_taban_rho_tavan() {
    let floor = 50 * MS;
    assert_eq!(
        learned_persist_min_ns(floor, None, 1250, None),
        floor,
        "örnek yok → taban"
    );
    assert_eq!(
        learned_persist_min_ns(floor, Some(20 * MS), 1250, None),
        floor,
        "küçük Q → taban"
    );
    assert_eq!(
        learned_persist_min_ns(floor, Some(120 * MS), 1250, None),
        150 * MS,
        "ρ×Q"
    );
    assert_eq!(
        learned_persist_min_ns(floor, Some(120 * MS), 1250, Some(100 * MS)),
        100 * MS,
        "tavan"
    );
}

/// COLD-START KAPISI: persistence modu L1'in İLK ateşleme anını DEĞİŞTİREMEZ (L1 anında kalır).
#[test]
fn persist_modu_l1_zamanlamasini_degistirmez() {
    let mut fixed = Sender::new();
    fixed.set_fec_loss_risk(FecLossRiskConfig::experimental_v1());
    let fixed = drive(fixed, 80, gap_pattern());
    let mut learned = Sender::new();
    learned.set_fec_loss_risk(FecLossRiskConfig {
        persist_mode: PersistMode::LearnedSpuriousAge,
        persist_srtt_permille: 250,
        ..FecLossRiskConfig::experimental_v1()
    });
    let learned = drive(learned, 80, gap_pattern());
    let (_, _, _, t1_fixed, _) = fixed.diag_spec_signals();
    let (_, _, _, t1_learned, _) = learned.diag_spec_signals();
    assert!(t1_fixed > 0, "önkoşul: L1 ateşledi");
    assert_eq!(
        t1_fixed, t1_learned,
        "L1 anı persistence modundan bağımsız olmalı"
    );
    assert_eq!(
        fixed.diag_risk_ppm(),
        learned.diag_risk_ppm(),
        "instant risk moddan bağımsız"
    );
}

/// Default config sevk davranışıdır: FixedSrtt, 0.5×srtt, tavan yok.
#[test]
fn persist_varsayilani_sevk() {
    let c = FecLossRiskConfig::default();
    assert_eq!(c.persist_mode, PersistMode::FixedSrtt);
    assert_eq!(c.persist_srtt_permille, 500);
    assert_eq!(c.persist_cap_srtt_permille, u32::MAX);
}

// ═══════════ [EVIDENCE ENSTRÜMANI] kanıt-kütlesi tanısı (politika kolu KALDIRILDI — Case B+D) ═══════════

/// Burst zaman çizgisi (handoff ailesiyle aynı): 80 birim, RTT örneği 4.1 s, 40..120 ardışık kayıp.
/// L2 (persistent) onaydan ÖNCE oluşur; `sent_window_units` = 80 (tek uçuş).
fn burst_drive(mut s: Sender) -> (Sender, Vec<TsSample>) {
    s.set_establish_probe(false);
    let acked: Vec<u64> = (1..400).filter(|q| !(40..120).contains(q)).collect();
    let mut samples = Vec::new();
    let mut step = |s: &mut Sender, ev: Event, t: u64| {
        s.on_event(ev, t);
        samples.push(s.diag_ts_sample());
    };
    for i in 0..80u64 {
        step(
            &mut s,
            Event::AppData(OpaqueUnit {
                id: 1000 + i,
                size: 101,
            }),
            0,
        );
    }
    for t in 1..=20u64 {
        step(&mut s, Event::TimerFired(Timer::Pto), t * 200 * MS);
    }
    step(
        &mut s,
        Event::AckReceived(AckInfo {
            acked_seqs: vec![0],
            ack_delay: Duration::ZERO,
        }),
        4_100 * MS,
    );
    for t in 21..=220u64 {
        step(&mut s, Event::TimerFired(Timer::Pto), t * 200 * MS);
    }
    step(
        &mut s,
        Event::AckReceived(AckInfo {
            acked_seqs: acked,
            ack_delay: Duration::ZERO,
        }),
        45_000 * MS,
    );
    for t in 221..=320u64 {
        step(&mut s, Event::TimerFired(Timer::Pto), t * 200 * MS);
    }
    (s, samples)
}

/// Geçiş enstrümanı: ilk L1/L2 anındaki (kütle, instant, persistent) dolu ve tutarlı.
#[test]
fn evidence_gecis_enstrumani_dolu() {
    let mut s = Sender::new();
    s.set_fec_loss_risk(FecLossRiskConfig::experimental_v1());
    let (s, samples) = burst_drive(s);
    let (l1, l2) = s.diag_spec_transitions();
    eprintln!("gecis: L1={l1:?} L2={l2:?}");
    // Kütle = o anda pencerede GÖNDERİLMİŞ birimler (sunulan 80 değil; pacing ilk uçuşu sınırlar).
    let first_l1 = samples
        .iter()
        .find(|x| x.level_idx == 1)
        .expect("önkoşul: L1 örneği");
    let first_l2 = samples
        .iter()
        .find(|x| x.level_idx == 2)
        .expect("önkoşul: L2 örneği");
    assert!(l1.0 > 0 && l1.0 <= 80, "ilk L1'de kütle (0, 80]: {l1:?}");
    assert_eq!(
        l1.0, first_l1.evidence_mass,
        "geçiş-diag'ı ile ts örneği tutarlı (L1)"
    );
    assert_eq!(l1.1, first_l1.instant_ppm);
    assert!(l1.1 >= 10_000, "ilk L1'de instant ≥ l1_min: {l1:?}");
    assert_eq!(
        l2.0, first_l2.evidence_mass,
        "geçiş-diag'ı ile ts örneği tutarlı (L2)"
    );
    assert!(l2.2 >= 10_000, "ilk L2'de persistent ≥ l2_min: {l2:?}");
    assert!(l2.0 >= l1.0, "kütle L1'den L2'ye azalmaz (aynı uçuş)");
}

// ═══════════════ [FEC v1] sınırlı politika + C4 guard'ı G1 ═══════════════

/// Ürün politikası değerleri (kapanış kararı): A 250-250-250 + P2 + G1(300‰). Transport default'u nötr kalır.
#[test]
fn bounded_v1_degerleri_ve_default_notr() {
    let b = FecLossRiskConfig::bounded_v1();
    assert!(b.enabled);
    assert_eq!(b.bootstrap_cap_permille, 250);
    assert_eq!(b.persistent_cap_permille, 250);
    assert_eq!(b.persist_mode, PersistMode::LearnedSpuriousAge);
    assert_eq!(
        (
            b.persist_srtt_permille,
            b.persist_rho_permille,
            b.persist_q_permille
        ),
        (500, 1250, 950)
    );
    assert_eq!(b.reorder_suppress_permille, 300);
    let d = FecLossRiskConfig::default();
    assert!(!d.enabled);
    assert_eq!(
        d.reorder_suppress_permille,
        u32::MAX,
        "default: G1 kapalı (nötr)"
    );
}

/// G1: kısmi geç-ACK (Spurious) ile reorder-baskısı ≥ 300‰ olunca spekülatif ÖNERİ susar; risk
/// hesabı, `t_first_risk` ve ilk L1 anı DEĞİŞMEZ (yalnız öneri bastırılır). Eşik kapalıyken öneri sürer.
#[test]
fn g1_reorder_baskisi_oneriyi_bastirir_riski_degistirmez() {
    fn scenario(cfg: FecLossRiskConfig, late_upto: u64) -> (Sender, Vec<TsSample>) {
        // Zaman çizgisi handoff ailesiyle aynı: PreConverge ilk ACK'e kadar yalnız seq 0'ı gönderir;
        // 80 birim ancak 4.1 s'deki ACK'ten sonra 44 s'ye kadar tele çıkar.
        let mut s = Sender::new();
        s.set_establish_probe(false);
        s.set_fec_loss_risk(cfg);
        let mut samples = Vec::new();
        let mut step = |s: &mut Sender, ev: Event, t: u64| {
            s.on_event(ev, t);
            samples.push(s.diag_ts_sample());
        };
        for i in 0..80u64 {
            step(
                &mut s,
                Event::AppData(OpaqueUnit {
                    id: 1000 + i,
                    size: 101,
                }),
                0,
            );
        }
        for t in 1..=20u64 {
            step(&mut s, Event::TimerFired(Timer::Pto), t * 200 * MS);
        }
        step(
            &mut s,
            Event::AckReceived(AckInfo {
                acked_seqs: vec![0],
                ack_delay: Duration::ZERO,
            }),
            4_100 * MS,
        );
        for t in 21..=220u64 {
            step(&mut s, Event::TimerFired(Timer::Pto), t * 200 * MS);
        }
        // 41..79 ACK → 1..40 paket-eşiğiyle Suspected (declared ≥ 8).
        step(
            &mut s,
            Event::AckReceived(AckInfo {
                acked_seqs: (41..80).collect(),
                ack_delay: Duration::ZERO,
            }),
            45_000 * MS,
        );
        for t in 226..=230u64 {
            step(&mut s, Event::TimerFired(Timer::Pto), t * 200 * MS);
        }
        // Geç-ACK: 1..late_upto Spurious (reorder), kalanı Suspected → rp = spurious/declared, risk sürer.
        step(
            &mut s,
            Event::AckReceived(AckInfo {
                acked_seqs: (1..late_upto).collect(),
                ack_delay: Duration::ZERO,
            }),
            46_200 * MS,
        );
        for t in 232..=400u64 {
            step(&mut s, Event::TimerFired(Timer::Pto), t * 200 * MS);
        }
        (s, samples)
    }
    let (g1, s_g1) = scenario(FecLossRiskConfig::bounded_v1(), 21);
    let (open, s_open) = scenario(
        FecLossRiskConfig {
            reorder_suppress_permille: u32::MAX,
            ..FecLossRiskConfig::bounded_v1()
        },
        21,
    );
    // Geç-ACK anı: ilk rp > 0 örneği. Pencere zamanla kayar (W_loss) → spurious kanıtı SONRA yaşlanır ve
    // rp tekrar 0'a iner; bu yüzden guard'ın etkisi geç-ACK'i izleyen örneklerde sınanır.
    let cut = s_open
        .iter()
        .position(|x| x.reorder_pressure_permille > 0)
        .expect("önkoşul: spurious gelmeli (rp > 0)");
    let win = |v: &[TsSample]| -> Vec<TsSample> { v[cut..(cut + 6).min(v.len())].to_vec() };
    eprintln!(
        "g1 pencere: {:?}",
        win(&s_open)
            .iter()
            .map(|x| (x.reorder_pressure_permille, x.instant_ppm, x.spec_prop))
            .collect::<Vec<_>>()
    );
    assert!(
        win(&s_open)
            .iter()
            .all(|x| x.reorder_pressure_permille >= 300 && x.instant_ppm >= 10_000),
        "önkoşul: rp ≥ 300‰ ∧ instant ≥ l1_min: {:?}",
        win(&s_open)
    );
    assert!(
        win(&s_open)
            .iter()
            .all(|x| x.spec_prop > 0 && x.level_idx >= 1),
        "eşik kapalı: öneri sürer"
    );
    assert!(
        win(&s_g1)
            .iter()
            .all(|x| x.spec_prop == 0 && x.level_idx == 0),
        "G1: öneri bastırılır (L0): {:?}",
        win(&s_g1)
    );
    // Risk hesabı ve zaman damgaları özdeş: G1 yalnız öneriyi bastırır.
    assert!(
        s_g1.iter()
            .zip(&s_open)
            .all(|(a, b)| a.instant_ppm == b.instant_ppm
                && a.persistent_ppm == b.persistent_ppm
                && a.reorder_pressure_permille == b.reorder_pressure_permille),
        "tahminci G1'den bağımsız"
    );
    assert_eq!(
        g1.diag_fec_risk_times().2,
        open.diag_fec_risk_times().2,
        "t_first_risk değişmez"
    );
    assert_eq!(
        g1.diag_spec_signals().3,
        open.diag_spec_signals().3,
        "ilk L1 anı değişmez (geç-ACK'ten önce)"
    );
    assert!(g1.diag_spec_signals().3 > 0, "önkoşul: L1 ateşledi");
    // Geç-ACK'ten ÖNCE öneriler özdeş (guard rp yokken inert).
    assert_eq!(&s_g1[..cut], &s_open[..cut], "rp = 0 iken byte-özdeş");
    // Eşik semantiği: spurious payı eşiğin ALTINDA (1..10 → rp = 250‰ < 300) ⇒ guard bastırmaz; seri
    // eşiği kapalı kolla özdeş. Guard salt mevcut split'in fonksiyonu (latch/tarihçe yok).
    let (_, s_g1_low) = scenario(FecLossRiskConfig::bounded_v1(), 11);
    let (_, s_open_low) = scenario(
        FecLossRiskConfig {
            reorder_suppress_permille: u32::MAX,
            ..FecLossRiskConfig::bounded_v1()
        },
        11,
    );
    let rp_low = s_open_low
        .iter()
        .map(|x| x.reorder_pressure_permille)
        .max()
        .unwrap_or(0);
    assert!(
        rp_low > 0 && rp_low < 300,
        "önkoşul: 0 < rp < 300‰ ({rp_low})"
    );
    assert_eq!(s_g1_low, s_open_low, "eşik altında G1 byte-özdeş");
}

// ═══════════ [KURULUŞ-GUARD] PreConverge'de hiçbir kol parity çıkaramaz ═══════════

/// Denetim bulgusu (2026-09-08): `structure_gate_fec` PreConverge'de bloke ediyordu ama
/// `speculative_propose` hiçbir `SenderState` kontrolü içermiyor. FEC ürün profilinde default
/// AÇIK olduğundan bu canlı bir yol. Guard arbiter ÇIKIŞINA taşındı; bu test onu pinler.
///
/// Senaryo: Karn-dışı ACK — retransmit belirsizliği yüzünden RTT örneği ÜRETİLMEZ (oturum
/// PreConverge'de kalır) ama SACK boşlukları kaybı ilan eder → spekülatif kol parity önerir.
#[test]
fn kurulus_guard_preconverge_de_arbiter_ciktisini_sifirlar() {
    let mut s = Sender::new();
    s.set_establish_probe(false);
    s.set_fec_loss_risk(FecLossRiskConfig::experimental_v1());
    for i in 0..80u64 {
        s.on_event(
            Event::AppData(OpaqueUnit {
                id: 1000 + i,
                size: 101,
            }),
            0,
        );
    }
    let mut saw_proposal_in_preconverge = false;
    let mut max_final_in_preconverge = 0u32;
    for t in 1..=40u64 {
        s.on_event(Event::TimerFired(Timer::Pto), t * 200 * MS);
        if s.state() == SenderState::PreConverge {
            let x = s.diag_ts_sample();
            max_final_in_preconverge = max_final_in_preconverge.max(x.final_ratio);
            if x.spec_prop > 0 || x.structure_prop > 0 || x.confirmed_prop > 0 {
                saw_proposal_in_preconverge = true;
            }
        }
    }
    assert_eq!(
        s.state(),
        SenderState::PreConverge,
        "önkoşul: ACK yok → oturum kurulmadı"
    );
    assert_eq!(
        max_final_in_preconverge, 0,
        "PreConverge'de arbiter çıkışı parity YAZAMAZ (kuruluş guard'ı)"
    );
    // Guard sayacı, öneri gerçekten oluştuysa artmış olmalı (vakum testi değil).
    if saw_proposal_in_preconverge {
        assert!(
            s.diag_preconverge_parity_blocked() > 0,
            "öneri vardı ama guard sayacı artmadı"
        );
    }
}

/// Guard yalnız PreConverge'i keser: oturum kurulduktan sonra parity normal akar (regresyon pini).
#[test]
fn kurulus_guard_sadece_preconverge_i_keser() {
    let s = drive(
        {
            let mut x = Sender::new();
            x.set_fec_loss_risk(FecLossRiskConfig::experimental_v1());
            x
        },
        80,
        gap_pattern(),
    );
    assert_ne!(
        s.state(),
        SenderState::PreConverge,
        "önkoşul: oturum kuruldu"
    );
    let levels = s.diag_spec_levels();
    assert!(
        levels[1] + levels[2] > 0,
        "kuruluş sonrası spekülatif kol ateşlemeli: {levels:?}"
    );
}

// ═══════════ [FAZ-1] DÖNÜŞ-YOLU GÖZLEMİ — ileri kayıp ≠ dönüş bozulması ═══════════

use adaptive_transport::return_path::{AckObservation, ReturnPathHealth, ReturnPathObserver};

fn obs(seen: u32, sent: u64) -> AckObservation {
    AckObservation {
        wire_bytes: 65,
        seen_count: seen,
        v2: true,
        sent_datagrams: sent,
    }
}

/// R1 — YALNIZ İLERİ-YÖN bozulması: ACK'ler ritminde akıyor (süreklilik tam) ama alıcı
/// gönderilenin çok azını görüyor → `forward_loss` YÜKSEK, `return_impairment` SIFIR.
#[test]
fn r1_ileri_yon_kaybi_donus_bozulmasi_olarak_gorunmez() {
    let mut o = ReturnPathObserver::default();
    // Alıcının NOMİNAL ritmi (2 datagram/ACK) → dönüş yolu sağlıklı; ileri yönde %50 kayıp.
    // (Fixture 20 datagram/ACK iken mutlak ölçü haklı olarak %90 dönüş-ACK kaybı bildiriyordu.)
    let (mut sent, mut seen) = (0u64, 0u32);
    for i in 0..10u64 {
        sent += 2;
        seen += 1;
        o.on_ack(obs(seen, sent), (i + 1) * 100 * MS);
    }
    let s = o.state(10 * 100 * MS);
    assert!(s.direction_evidence, "yön kanıtı oluşmalı");
    // Bir-ACK gecikmeli kümülatif oran: sent_{n-1}=180, seen_n=100 → 444‰ (muhafazakâr: gerçek
    // %50'nin altında, çünkü son pencerenin uçuşu henüz sayılmaz).
    assert!(
        (350..=550).contains(&s.forward_loss_permille),
        "ileri kayıp ≈ %50 olmalı (gecikme yüzünden hafif düşük): {s:?}"
    );
    assert_eq!(
        s.return_impairment_permille, 0,
        "geri bildirim kesintisiz → dönüş bozulması SIFIR (INV-RETURN-1): {s:?}"
    );
    assert_eq!(s.health, ReturnPathHealth::Healthy);
}

/// R2 — YALNIZ DÖNÜŞ bozulması: alıcı gönderilenin TAMAMINI görüyor (ileri yön temiz) ama
/// ACK'lerin çoğu dönüşte kayboluyor → `forward_loss` SIFIR, `return_impairment` YÜKSEK.
#[test]
fn r2_donus_yolu_bozulmasi_ileri_kayip_olarak_gorunmez() {
    let mut o = ReturnPathObserver::default();
    let (mut sent, mut seen) = (0u64, 0u32);
    // İlk iki ACK ritmi kurar (100 ms), sonra ACK'ler 5 pencerede bir geliyor.
    for i in 0..3u64 {
        sent += 20;
        seen += 20;
        o.on_ack(obs(seen, sent), (i + 1) * 100 * MS);
    }
    let mut t = 300 * MS;
    for _ in 0..6u64 {
        t += 500 * MS; // 5 pencere = 4 kaçırılmış
        sent += 100;
        seen += 100;
        o.on_ack(obs(seen, sent), t);
    }
    let s = o.state(t);
    assert!(s.direction_evidence);
    assert_eq!(
        s.forward_loss_permille, 0,
        "alıcı her şeyi gördü → ileri kayıp SIFIR: {s:?}"
    );
    // Ayırt edici üçlü (hepsi dönüş yolunu gösterir, hiçbiri ileri yönü suçlamaz):
    assert!(
        s.return_impairment_permille >= 300,
        "kümülatif süreklilik kaybı görünmeli: {s:?}"
    );
    assert!(
        s.ack_interval_ns >= 300 * MS,
        "geri-bildirim ritmi yavaşladı: {s:?}"
    );
    assert!(
        s.measured_return_bw_bps < 500,
        "ölçülen dönüş debisi düştü (R0'da >1000 B/s): {s:?}"
    );
}

/// R5 — geri bildirim ÖLDÜ: son ACK'ten sonra uzun sessizlik → `Dead`; ileri-yön ölçüsü
/// (son bilinen `forward_loss`) bundan ETKİLENMEZ.
#[test]
fn r5_olu_geri_bildirim_dead_olur_ileri_olcuyu_bozmaz() {
    let mut o = ReturnPathObserver::default();
    let (mut sent, mut seen) = (0u64, 0u32);
    for i in 0..5u64 {
        sent += 2; // nominal ritim → dönüş yolu sağlıklı başlar
        seen += 2;
        o.on_ack(obs(seen, sent), (i + 1) * 100 * MS);
    }
    let healthy = o.state(520 * MS);
    assert_eq!(healthy.health, ReturnPathHealth::Healthy);
    let degraded = o.state(500 * MS + 350 * MS);
    assert_eq!(degraded.health, ReturnPathHealth::Degraded);
    let dead = o.state(500 * MS + 5_000 * MS);
    assert_eq!(dead.health, ReturnPathHealth::Dead, "{dead:?}");
    assert_eq!(dead.forward_loss_permille, healthy.forward_loss_permille);
}

/// R0 — sağlıklı simetrik: örnek biriktikçe güven artar, ölçülen dönüş bandı ÖNSELE (31 B/s)
/// eşit DEĞİLDİR — ölçüm önselin yerine geçmez, onun yanında durur.
#[test]
fn r0_saglikli_yolda_olculen_donus_bandi_onselden_bagimsizdir() {
    let mut o = ReturnPathObserver::default();
    let (mut sent, mut seen) = (0u64, 0u32);
    for i in 0..8u64 {
        sent += 10;
        seen += 10;
        o.on_ack(obs(seen, sent), (i + 1) * 50 * MS);
    }
    let s = o.state(400 * MS);
    assert_eq!(s.confidence_permille, 1000, "8 örnekte güven doyar");
    // 65 B / 50 ms = 1300 B/s — 31 B/s önseliyle hiçbir ilgisi yok.
    assert!(
        s.measured_return_bw_bps > 1000,
        "ölçülen dönüş bandı gerçek debiden gelir: {s:?}"
    );
    assert_eq!(s.ack_interval_ns, 50 * MS);
}

/// R5' — MUTLAK dönüş-ACK-kaybı: alıcının nominal ritmi (ACK_FREQ_N=2 datagram/ACK) referans
/// alınır; ACK başına 30 datagram gözlenirse dönüşte ~%93 ACK kaybı vardır. Bu ölçü EWMA'nın
/// yeni ritme uyum sağlamasından ETKİLENMEZ ve ileri-yön ölçüsünü BOZMAZ.
#[test]
fn r5b_mutlak_donus_ack_kaybi_ewma_uyumundan_etkilenmez() {
    let mut o = ReturnPathObserver::default();
    // 10 ACK, her biri 300 ms arayla; ACK başına 30 datagram (nominal 2) → ~933‰ ACK kaybı.
    let (mut sent, mut seen) = (0u64, 0u32);
    for i in 0..10u64 {
        sent += 30;
        seen += 30;
        o.on_ack(obs(seen, sent), (i + 1) * 300 * MS);
    }
    let s = o.state(3_000 * MS);
    assert_eq!(s.datagrams_per_ack_x100 / 100, 30, "{s:?}");
    assert!(
        s.return_ack_loss_permille >= 900,
        "dönüş ACK kaybı ~933‰ olmalı: {s:?}"
    );
    assert_eq!(
        s.forward_loss_permille, 0,
        "ileri yol temiz — dönüş kaybı oraya YAZILMAZ (INV-RETURN-1): {s:?}"
    );
    assert_eq!(s.health, ReturnPathHealth::Dead, "{s:?}");
}

/// Sağlıklı ritim (2 datagram/ACK) mutlak ölçüde SIFIR kayıp verir — yanlış alarm yok.
#[test]
fn r0b_nominal_ritim_donus_kaybi_uretmez() {
    let mut o = ReturnPathObserver::default();
    let (mut sent, mut seen) = (0u64, 0u32);
    for i in 0..12u64 {
        sent += 2;
        seen += 2;
        o.on_ack(obs(seen, sent), (i + 1) * 40 * MS);
    }
    let s = o.state(480 * MS);
    assert_eq!(s.return_ack_loss_permille, 0, "{s:?}");
    assert_eq!(s.health, ReturnPathHealth::Healthy);
}

/// Kanıt eşiği: V1 (seen_count'suz) ACK'ler yön-ayrımı ÜRETMEZ — uydurulmuş sayı yayılmaz.
#[test]
fn v1_ack_yon_ayrimi_uretmez() {
    let mut o = ReturnPathObserver::default();
    for i in 0..6u64 {
        o.on_ack(
            AckObservation {
                wire_bytes: 59,
                seen_count: 0,
                v2: false,
                sent_datagrams: (i + 1) * 20,
            },
            (i + 1) * 100 * MS,
        );
    }
    let s = o.state(600 * MS);
    assert!(!s.direction_evidence, "V2 yoksa yön kanıtı YOK");
    assert_eq!(s.forward_loss_permille, 0);
    assert!(s.ack_samples == 6 && s.v2_samples == 0);
}

// ═══════════ [FAZ-2] FEC SPEC-TAMLIĞI — kalem 2 ve 4 ═══════════

/// KALEM-4: acil-durum kurtarma framing'i FEC blok framing'ine göre seçilir (§11 / TDD §5.6).
/// FEC kapalıyken `Tiny` (D× kopya), FEC açıkken `SegmentBlock` (minimal parity). Çağıran eskiden
/// `Framing::Tiny`'yi SABİT veriyordu → spec'in segment-block kolu ulaşılamazdı.
#[test]
fn kalem4_acil_framing_fec_aktifligine_bagli() {
    use adaptive_transport::emergency::{Framing, RecoveryAction};
    // FEC KAPALI (default Manual{false}) → Tiny kolu → kopya, parity YOK.
    let mut off = Sender::new();
    off.set_establish_probe(false);
    let off = drive(off, 40, gap_pattern());
    // FEC AÇIK (Controller) → SegmentBlock kolu.
    let mut on = Sender::new();
    on.set_establish_probe(false);
    let _ = on.set_fec_actuation_mode(FecActuationMode::Controller);
    let on = drive(on, 40, gap_pattern());
    // Her iki kolda da acil-durum seviyesine girilmemiş olabilir; framing seçimi yine de
    // last_recovery'nin TÜRÜNDEN okunur (Arq seviyesinde ikisi de Arq'tır).
    let f_off = off.last_recovery();
    let f_on = on.last_recovery();
    assert!(
        !matches!(f_off, RecoveryAction::MinimalParity { .. }),
        "FEC kapalıyken minimal-parity kolu SEÇİLEMEZ: {f_off:?}"
    );
    // Framing eşlemesinin kendisi (seviyeden bağımsız) doğrudan pinlenir:
    let cfg = adaptive_transport::emergency::EmergencyConfig::fixture();
    let a_tiny = adaptive_transport::emergency::action(
        adaptive_transport::emergency::EmergencyLevel::Dup2,
        Framing::Tiny,
        &cfg,
    );
    let a_block = adaptive_transport::emergency::action(
        adaptive_transport::emergency::EmergencyLevel::Dup2,
        Framing::SegmentBlock,
        &cfg,
    );
    assert!(matches!(a_tiny, RecoveryAction::Duplication { .. }));
    assert!(
        matches!(a_block, RecoveryAction::MinimalParity { permille } if permille > 0),
        "segment-block → minimal parity (§11 NORMATİF): {a_block:?}"
    );
    let _ = f_on;
}

/// KALEM-2: §8.5 bütçe clamp'i bağlandı, ama BANT KANITI YOKKEN parity'yi SIFIRLAMAZ.
/// (Formülün literal okuması BtlBw<125 B/s'te cap=0 verir ve §11'in minimal-parity tabanıyla
/// çelişir; sıfırlama vakası ayrı sayaçla görünür kılınır, sessizce seçilmez.)
#[test]
fn kalem2_bant_kaniti_yokken_clamp_parity_sifirlamaz() {
    let mut s = Sender::new();
    s.set_establish_probe(false);
    s.set_fec_loss_risk(FecLossRiskConfig::experimental_v1());
    let s = drive(s, 80, gap_pattern());
    // Bu sürücüde ölçülen bant çok küçüktür → clamp sıfırlamak isterdi.
    let would_zero = s.diag_parity_budget_would_zero();
    let levels = s.diag_spec_levels();
    assert!(
        levels[1] + levels[2] > 0,
        "önkoşul: spekülatif kol öneri üretmeli: {levels:?}"
    );
    assert!(
        would_zero > 0,
        "dar bantta clamp sıfırlamak isterdi ve bu GÖRÜNÜR olmalı"
    );
    // Ve gerçekten sıfırlanmamış olmalı: son karar oranı > 0.
    assert!(
        s.diag_ts_sample().final_ratio > 0,
        "kanıt yokken clamp parity'yi silmemeli: {:?}",
        s.diag_ts_sample()
    );
}

// ═══════════ [FAZ-3] RUNTIME TELEMETRİ — yapılandırıldı ≠ ulaşılabilir ≠ ateşledi ═══════════

/// "Yapılandırıldı" ile "ateşledi" AYRI olgulardır: tail-recovery açıkken bile hiç probe
/// çıkmayabilir. Telemetri ikisini karıştırmamalı (denetimin "kod var → koşmuştur" hatası).
#[test]
fn faz3_yapilandirildi_atesledi_ayrimi() {
    use adaptive_transport::tail_recovery::TailRecoveryConfig;
    // (a) tail KAPALI (Sender default'u): configured=false, ateşleme=0.
    let off = Sender::new();
    let a_off = off.diag_activation();
    assert!(!a_off.tail_configured, "Sender default'unda tail KAPALI");
    assert_eq!(a_off.tail_probes, 0);

    // (b) tail AÇIK ama hiç sürülmedi: configured=true, ateşleme HÂLÂ 0.
    let mut on = Sender::new();
    on.set_tail_recovery_config(TailRecoveryConfig::validated_v1());
    let a_on = on.diag_activation();
    assert!(a_on.tail_configured, "validated_v1 → configured");
    assert_eq!(
        a_on.tail_probes, 0,
        "yapılandırma ATEŞLEME DEĞİLDİR: {a_on:?}"
    );
    assert_eq!(a_on.left_preconverge_ns, 0, "oturum kurulmadı");
}

/// Ulaşılabilirlik ve tahminciler: ACK'li bir akıştan sonra faz geçişleri sayılır, PreConverge'den
/// çıkış anı damgalanır ve karar GİRDİLERİ (srtt/btlbw/pacer) ürün derlemesinde okunabilir olur.
#[test]
fn faz3_ulasilabilirlik_ve_tahminciler_dolu() {
    let s = drive(Sender::new(), 80, gap_pattern());
    let a = s.diag_activation();
    assert!(
        a.state_transitions > 0,
        "faz geçişi olmalı (ulaşılabilirlik kanıtı): {a:?}"
    );
    assert!(
        a.left_preconverge_ns > 0,
        "ilk RTT örneğiyle PreConverge terk edilmeli: {a:?}"
    );
    assert!(a.srtt_us > 0, "srtt ölçülmeli: {a:?}");
    assert!(a.pacer_rate_bps > 0, "pacer hızı okunabilmeli: {a:?}");
    // Kayıp sürüşünde ARQ fazına girilmiş olmalı ve retransmit ateşlemiş olmalı.
    assert!(
        a.recover_arq_entries > 0 || a.retransmits > 0,
        "kayıplı sürüşte ARQ kolu ateşlemeli: {a:?}"
    );
}

/// Vakum koruması: FEC yapılandırılmamışken parity sayaçları SIFIR kalır — telemetri
/// "ateşledi" diye yanlış kanıt üretmez.
#[test]
fn faz3_fec_yapilandirilmamisken_parity_sayaclari_sifir() {
    let s = drive(Sender::new(), 80, gap_pattern());
    let a = s.diag_activation();
    assert!(!a.fec_configured, "bu sürüşte FEC aktüasyonu yok");
    assert_eq!(a.parity_emitted, 0, "{a:?}");
    assert_eq!(a.parity_token_denied, 0, "{a:?}");
}
