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

// ═══════════════════════════════ [EVIDENCE] kanıt-kütlesi kolu ═══════════════════════════════

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

fn with_evidence(mode: EvidenceMode, min: u64, full: u64) -> Sender {
    let mut s = Sender::new();
    s.set_fec_loss_risk(FecLossRiskConfig {
        evidence_mode: mode,
        evidence_min_units: min,
        evidence_full_units: full,
        ..FecLossRiskConfig::experimental_v1()
    });
    s
}

/// Default sevk: kanıt kütlesi OKUNMAZ.
#[test]
fn evidence_varsayilani_off() {
    assert_eq!(
        FecLossRiskConfig::default().evidence_mode,
        EvidenceMode::Off
    );
    assert_eq!(
        FecLossRiskConfig::experimental_v1().evidence_mode,
        EvidenceMode::Off
    );
}

/// Off'ta knob'lar inert: örnek dizisi ve seviye sayaçları knob'dan bağımsız ÖZDEŞ.
#[test]
fn evidence_off_knoblari_okumaz() {
    let (a, sa) = burst_drive(with_evidence(EvidenceMode::Off, 32, 64));
    let (b, sb) = burst_drive(with_evidence(EvidenceMode::Off, 1_000_000, 1));
    assert_eq!(sa, sb, "Off'ta knob'lar çıktıyı değiştiremez");
    assert_eq!(a.diag_spec_levels(), b.diag_spec_levels());
    assert!(a.diag_spec_signals().4 > 0, "önkoşul: burst L2 üretir");
}

/// M1 taban: kütle < min ⇒ L2 UYGUN DEĞİL → L1; L1 anı ve instant risk DEĞİŞMEZ. min ≤ kütle ⇒ Off'a özdeş.
#[test]
fn evidence_floor_l2_yi_l1_e_indirir_l1_dokunulmaz() {
    let (off, s_off) = burst_drive(with_evidence(EvidenceMode::Off, 32, 64));
    let (fl, s_fl) = burst_drive(with_evidence(EvidenceMode::L2Floor, 10_000, 64));
    let (fl1, s_fl1) = burst_drive(with_evidence(EvidenceMode::L2Floor, 1, 64));
    let (_, _, _, t1_off, t2_off) = off.diag_spec_signals();
    let (_, _, _, t1_fl, t2_fl) = fl.diag_spec_signals();
    assert!(t2_off > 0, "önkoşul: Off'ta L2 var");
    assert_eq!(t1_off, t1_fl, "L1 anı kütleden bağımsız");
    assert_eq!(t2_fl, 0, "kütle 80 < 10 000 ⇒ L2 hiç uygun değil");
    assert_eq!(fl.diag_spec_levels()[2], 0);
    assert!(
        s_off
            .iter()
            .zip(&s_fl)
            .all(|(x, y)| x.instant_ppm == y.instant_ppm
                && x.persistent_ppm == y.persistent_ppm
                && x.evidence_mass == y.evidence_mass),
        "tahminci (instant/persistent/kütle) politikadan bağımsız"
    );
    // Taban altında kalan L2 örnekleri L1 tavanına iner (125), asla sıfıra değil.
    let demoted: Vec<(u32, u32)> = s_off
        .iter()
        .zip(&s_fl)
        .filter(|(x, _)| x.level_idx == 2)
        .map(|(x, y)| (x.spec_prop, y.spec_prop))
        .collect();
    assert!(!demoted.is_empty());
    assert!(
        demoted.iter().all(|&(o, f)| f > 0 && f <= o),
        "indirilen öneri >0 ve ≤ Off: {demoted:?}"
    );
    assert_eq!(s_off, s_fl1, "min=1 ⇒ Off'a byte-özdeş");
    assert_eq!(fl1.diag_spec_levels(), off.diag_spec_levels());
}

/// M2 aritmetiği: `l1 + (l2−l1)·min(em, full)/full` — tamsayı; Off/Floor'da l2 sabit.
#[test]
fn evidence_scaled_tavan_dogrusal_tamsayi() {
    let s = with_evidence(EvidenceMode::L2Scaled, 32, 64);
    assert_eq!(s.l2_cap_with_evidence(0), 125);
    assert_eq!(s.l2_cap_with_evidence(32), 187);
    assert_eq!(s.l2_cap_with_evidence(64), 250);
    assert_eq!(s.l2_cap_with_evidence(1_000), 250);
    assert_eq!(
        with_evidence(EvidenceMode::Off, 32, 64).l2_cap_with_evidence(0),
        250
    );
    assert_eq!(
        with_evidence(EvidenceMode::L2Floor, 32, 64).l2_cap_with_evidence(0),
        250
    );
    assert_eq!(
        with_evidence(EvidenceMode::L2Scaled, 32, 0).l2_cap_with_evidence(0),
        250,
        "full=0 → değişmez"
    );
}

/// M2 uçtan uca: full ≫ kütle ⇒ L2 örnekleri L1 tavanına ölçeklenir; seviye L2 KALIR; L1 anı değişmez.
#[test]
fn evidence_scaled_l2_yetkisini_olcekler_seviyeyi_korur() {
    let (off, s_off) = burst_drive(with_evidence(EvidenceMode::Off, 32, 64));
    let (sc, s_sc) = burst_drive(with_evidence(EvidenceMode::L2Scaled, 32, 1_000_000));
    assert_eq!(
        off.diag_spec_signals().3,
        sc.diag_spec_signals().3,
        "L1 anı"
    );
    assert_eq!(
        off.diag_spec_signals().4,
        sc.diag_spec_signals().4,
        "L2 anı (seviye korunur)"
    );
    assert_eq!(off.diag_spec_levels(), sc.diag_spec_levels());
    let pairs: Vec<(u32, u32)> = s_off
        .iter()
        .zip(&s_sc)
        .filter(|(x, _)| x.level_idx == 2 && x.confirmed_prop == 0 && x.structure_prop == 0)
        .map(|(x, y)| (x.spec_prop, y.spec_prop))
        .collect();
    assert!(!pairs.is_empty(), "vakum: L2 ∧ kanonik sessiz örneği yok");
    assert!(
        pairs.iter().all(|&(o, s)| s < o && s > 0),
        "ölçekli < Off, >0: {pairs:?}"
    );
}

/// Temiz yol: kayıp yoksa hiçbir modda L1/L2 yok, risk 0.
#[test]
fn evidence_temiz_yolda_hicbir_mod_ateslemez() {
    for (m, min, full) in [
        (EvidenceMode::Off, 32, 64),
        (EvidenceMode::L2Floor, 1, 64),
        (EvidenceMode::L2Scaled, 32, 1),
    ] {
        let s = drive(with_evidence(m, min, full), 80, (0..400).collect());
        assert_eq!(s.diag_spec_levels()[1], 0, "{m:?}: L1 yok");
        assert_eq!(s.diag_spec_levels()[2], 0, "{m:?}: L2 yok");
        assert_eq!(s.diag_risk_ppm().0, 0, "{m:?}: risk 0");
    }
}

/// Geçiş enstrümanı: ilk L1/L2 anındaki (kütle, instant, persistent) dolu ve tutarlı.
#[test]
fn evidence_gecis_enstrumani_dolu() {
    let (s, samples) = burst_drive(with_evidence(EvidenceMode::Off, 32, 64));
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
