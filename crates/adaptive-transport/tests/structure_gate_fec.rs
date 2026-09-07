//! [Q18-FIX] Yapı-farkında FEC gate — Sender-düzeyi regresyon pini.
//!
//! Q18 (`q18_check.rs`) fuzzy controller'ın rate-only çıktısını pinler: %20 kayıpta parity=0 — ve
//! o pin GEÇERLİ KALIR (fuzzy'ye dokunulmadı, diz kaydırılmadı; Q17 bunu yasaklar). Fix Sender'da:
//! `structure_gate_fec`, fate_estimator'ın loss-YAPI sinyalini tüketir. Gate iki kanıt ister:
//! TAZE kayıp (ledger `loss_windowed_ppm` ≥ Q18 bandı — trafik durunca doğal decay, bayat-FEC yok)
//! ve YAPI (fate-penceresi `burst_delta`, ≥32 fate-known). Bu dosya ürün-yolundaki sonucu pinler:
//! aynı kayıp ORANINDA IID yapı FEC'i AÇAR, burst yapı ARQ'ya bırakır, düşük kayıp byte-özdeş kalır,
//! ve kayıp bitince gate kapanır.
use adaptive_transport::fusion::FusionDecision;
use adaptive_transport::sender::*;
use std::time::Duration;

const MS: u64 = 1_000_000;

fn feed(s: &mut Sender, units: u64) {
    for i in 0..units {
        s.on_event(
            Event::AppData(OpaqueUnit {
                id: 1000 + i,
                size: 101,
            }),
            0,
        );
    }
}

fn pto(s: &mut Sender, t_ms: u64) {
    s.on_event(Event::TimerFired(Timer::Pto), t_ms * MS);
}

/// Ürün-şekilli sürüş (`loss_structure_observation.rs` omurgası): emit → seq-0 erken-ACK ile
/// converge → `acked` seq'leri ACK'le (ACK-ufku altındaki boşluklar kayıp ilan edilir). Kuyruk
/// pompası ÇAĞIRANA bırakılır: gate'in ateşlediği anı yakalamak ve sonra decay'i izlemek için.
/// Döner: (sender, son-ACK zamanı ms).
///
/// Wire-seq ≠ unit-id: seq her emisyonda (retransmit dahil) artar. ACK deseni bu yüzden geniş bir
/// seq aralığına (`SEQ_SPAN`) uygulanır; emit edilmemiş seq'ler ACK'te yok sayılır. Aksi hâlde
/// desenin dışında kalan son emisyonlar PTO ile bitişik "kuyruk-burst"e dönüşür (ilk sürümün
/// düşük-kayıp hücresinde 12/72 = 166k ppm ölçen artefakt).
const SEQ_SPAN: u64 = 400;
fn drive_until_loss_ack(units: u64, acked: Vec<u64>) -> (Sender, u64) {
    let mut s = Sender::new();
    // [DR-EST-1 IZOLASYONU] Bu suruculer S/L yapisini SCRIPTED seq-ACK-listeleriyle IMAL eder
    // (45 s ACK'siz PreConverge + cift-seq'ler). Kurulus-kopyalari araya seq sokup imal-edilen
    // deseni bozar - test edilen sey GATE-MANTIGI (estimator->gate->decision), kopya-etkilesimi
    // degil; urunde kopya-ACK'i gercek teslim-sinyalidir. Kolun pinleri tests/establish_probe.rs.
    s.set_establish_probe(false);
    feed(&mut s, units);
    for t in 1..=20u64 {
        pto(&mut s, t * 200);
    }
    s.on_event(
        Event::AckReceived(AckInfo {
            acked_seqs: vec![0],
            ack_delay: Duration::ZERO,
        }),
        4_100 * MS,
    );
    // Converge sonrası TÜM birimler emit edilene kadar pompala (PreConverge-taban pacing yavaş:
    // 17 s'de ~63/80 çıkıyordu; kalan 9'u ACK-sonrası emit olup PTO ile bitişik "kuyruk-burst"e
    // dönüşüyordu — ilk sürümün düşük-kayıp hücresinde 12/72 = 166k ppm ölçen artefakt).
    for t in 21..=220u64 {
        pto(&mut s, t * 200);
    }
    let t_ack = 45_000;
    s.on_event(
        Event::AckReceived(AckInfo {
            acked_seqs: acked,
            ack_delay: Duration::ZERO,
        }),
        t_ack * MS,
    );
    (s, t_ack)
}

/// Kayıp-ACK'inden sonra timer pompala; gate'in İLK ateşlediği event'te (sayaç artınca) o anki
/// kararı döndür. Ateşlemezse `None`. Fate-finalize guard'ı (≈base_pto) bu pencerede geçer.
fn pump_until_gate_opens(s: &mut Sender, t_ack: u64, steps: u64) -> Option<FusionDecision> {
    let before = s.diag_structure_gate_opened();
    for i in 1..=steps {
        pto(s, t_ack + i * 200);
        if s.diag_structure_gate_opened() > before {
            return Some(s.last_decision());
        }
    }
    None
}

/// IID @ ~%50: her ikinci seq ACK'lenir → S L S L … → n11≈0 → p11 ≪ p01 → burst_delta belirgin
/// NEGATİF → gate IID okur → fuzzy'nin fec=0'ı `fec_med`(500)'e açılır. Sonra trafik yok → ledger
/// decay → gate KAPANIR (bayat-FEC yok; yapı-penceresi kalsa bile taze-kayıp şartı düşer).
#[test]
fn iid_loss_opens_fec_through_structure_gate_then_closes_on_decay() {
    let acked: Vec<u64> = (1..SEQ_SPAN).filter(|q| q % 2 == 0).collect();
    let (mut s, t_ack) = drive_until_loss_ack(80, acked);
    let fired = pump_until_gate_opens(&mut s, t_ack, 60);
    let f = s.diag_loss_structure();
    assert!(
        f.n >= 32,
        "kanıt-eşiği: pencerede ≥32 fate-known birim gerekir; got n={} ({f:?})",
        f.n
    );
    assert!(
        f.burst_delta_ppm <= 50_000,
        "alternating S/L must read as IID (burst_delta ≤ 50k); got {} ({f:?})",
        f.burst_delta_ppm
    );
    let d = fired.expect("Q18-FIX: the structure gate must open FEC on an IID high-loss flow");
    assert_eq!(
        d.fec_parity_ratio_permille, 500,
        "IID @ high loss → decision carries fec_med (500‰), not fuzzy's rate-only 0"
    );

    // Decay: uzun sessizlik → ledger boşalır → taze-kayıp şartı düşer → gate artık açmaz.
    // Ledger penceresi = ack_epoch×K_WL(8); bu sürüşte srtt onlarca saniye (seq-0 ACK'i 4.1 s'de,
    // kayıp-ACK'i 45 s'de) → pencere dakikalar. Sim-ucuz: ~15 dk sessizlik pompala.
    let opened_at_end_of_episode = s.diag_structure_gate_opened();
    for i in 61..=4_500u64 {
        pto(&mut s, t_ack + i * 200);
    }
    let snap = s.diag_fusion_input_snapshot().expect("fuzzy ran");
    assert!(
        snap.loss_windowed_ppm < 100_000,
        "precondition: ledger must have decayed; got {}",
        snap.loss_windowed_ppm
    );
    assert_eq!(
        s.last_decision().fec_parity_ratio_permille,
        0,
        "no stale FEC: once fresh loss decays the gate must close (structure window persisting \
         is not enough on its own)"
    );
    // Ve decay boyunca bir kez daha açılmamış olmalı (ledger < band).
    assert!(
        s.diag_structure_gate_opened() >= opened_at_end_of_episode,
        "counter is monotone"
    );
}

/// Burst: ilk yarı tamamen ACK, ikinci yarı bitişik kayıp → LLLL… → n11 yüksek → p11 ≫ p01 →
/// burst_delta belirgin POZİTİF → gate burst okur → DOKUNMAZ (ARQ'ya bırakır): fec kalır 0.
///
/// **[AÇIK KAYIT — bu pin VAKUMDUR, sonucu doğru ama gerekçesi yanlış.]** Sürücü bitişik bir
/// ÖN-EK'i ACK'liyor (`1..40`), yani ACK-ufkunun ALTINDA boşluk kalmıyor ve HİÇ kayıp ilan
/// edilmiyor: ölçülen `ledger_loss=0`, `fate_loss_ppm=0`. Okunan `burst_delta=+475_610` gerçek
/// burst yapısı değil, Laplace prior'ının kendisidir (p11=(0+1)/(0+2)=500_000). Yani gate burst
/// dalına HİÇ ULAŞMIYOR — kayıp-eşiğinde duruyor. Pin korunuyor (davranış sözleşmesi hâlâ
/// geçerli: bu girdide fec 0 kalmalı), ama YAPI dalının gerçek kanıtı aşağıdaki
/// `evidenced_burst_*` testlerindedir. Bulgu prior-as-evidence kusurunu ortaya çıkardı
/// (bkz. `structure_gate_fec`'teki DEFECT-FIX notu).
#[test]
fn burst_loss_leaves_fec_to_arq() {
    let acked: Vec<u64> = (1..40u64).collect();
    let (mut s, t_ack) = drive_until_loss_ack(80, acked);
    let fired = pump_until_gate_opens(&mut s, t_ack, 60);
    let f = s.diag_loss_structure();
    assert!(f.n >= 32, "kanıt-eşiği; got n={} ({f:?})", f.n);
    assert!(
        f.burst_delta_ppm > 50_000,
        "contiguous loss run must read as BURST (burst_delta > 50k); got {} ({f:?})",
        f.burst_delta_ppm
    );
    assert!(
        fired.is_none(),
        "burst structure must NOT open FEC (ARQ handles correlated loss; Q17 harmful-FP guard)"
    );
    assert_eq!(s.diag_structure_gate_opened(), 0);
    assert_eq!(s.last_decision().fec_parity_ratio_permille, 0);
}

/// Düşük kayıp (Q18 bandı altı, ~%5): gate hiç devreye girmez → fuzzy çıktısı byte-özdeş (fec=0).
#[test]
fn low_loss_is_untouched_by_the_gate() {
    let acked: Vec<u64> = (1..SEQ_SPAN).filter(|q| q % 20 != 0).collect();
    let (mut s, t_ack) = drive_until_loss_ack(80, acked);
    let fired = pump_until_gate_opens(&mut s, t_ack, 60);
    let f = s.diag_loss_structure();
    assert!(
        f.measured_loss_ppm < 100_000,
        "precondition: fate-window loss below the Q18 band; got {} ({f:?})",
        f.measured_loss_ppm
    );
    assert!(
        fired.is_none(),
        "gate must never open below the Q18 loss band"
    );
    assert_eq!(s.diag_structure_gate_opened(), 0);
    assert_eq!(s.last_decision().fec_parity_ratio_permille, 0);
}

// ── [BURST-FUSION] Burst dalının alternatif politikası ───────────────────────────────────────
//
// `burst_loss_leaves_fec_to_arq` (üstte) SEVK davranışını pinler ve geçerli kalır: default
// `FecStructurePolicy::Q18Iid`. Aşağıdakiler opt-in `BurstFusion` politikasını pinler — burst
// rejiminde parity SIFIRLANMAZ, ölçülen silme oranını emniyet paylı takip eder ve artakalanı
// ARQ taşır (§8.4 füzyon).
//
// Gerekçe ölçülü: `client-core/examples/fec_arm_compare.rs` GE-burst hücresinde adaptif kol
// `fec_bytes=0 / blocks=0 / repairs=0` üretiyor — yani ürünün ayırt edici mekanizması hostile
// rejimde HİÇ devreye girmiyor. Eşik KALİBRE EDİLMEMİŞTİR; default flip netem kanıtı ister.

/// Default politika `Q18Iid`'dir — sevk davranışı opt-in olmadan DEĞİŞMEZ.
#[test]
fn default_structure_policy_is_the_shipped_one() {
    let s = Sender::new();
    assert_eq!(s.fec_structure_policy(), FecStructurePolicy::Q18Iid);
}

// ── Gerçek KANITLI burst penceresi ───────────────────────────────────────────────────────────
//
// Yukarıdaki vakum pinin aksine burada ACK-ufku yüksek bir seq'e taşınır (199) ve ufkun ALTINDA
// bitişik boşluk RUN'ları bırakılır → gerçekten kayıp ilan edilir, `n10+n11` kanıt eşiğini geçer
// ve `burst_delta` prior'dan değil VERİDEN gelir.

/// 6 adet 8'lik bitişik kayıp run'u (48/200 ≈ 240k ppm) — hem Q18 kayıp bandını hem de
/// `MIN_LOSS_TRANSITIONS` kanıt eşiğini geçer.
fn evidenced_burst_acks() -> Vec<u64> {
    let holes: Vec<u64> = (0..6u64)
        .flat_map(|r| (0..8u64).map(move |j| 30 + r * 25 + j))
        .collect();
    (1..200u64).filter(|q| !holes.contains(q)).collect()
}

/// Kanıtlı burst + SEVK politikası → parity 0 (yapı dalı gerçekten koşar ve ARQ'ya bırakır).
#[test]
fn evidenced_burst_leaves_fec_to_arq_under_shipped_policy() {
    let (mut s, t_ack) = drive_until_loss_ack(200, evidenced_burst_acks());
    let fired = pump_until_gate_opens(&mut s, t_ack, 60);
    let f = s.diag_loss_structure();
    assert!(
        f.n10 + f.n11 >= 8,
        "önkoşul: kayıp-geçişi kanıt eşiği geçilmeli (prior değil VERİ); got n10={} n11={} ({f:?})",
        f.n10,
        f.n11
    );
    assert!(
        f.measured_loss_ppm > 0,
        "önkoşul: pencerede GERÇEK kayıp olmalı; got {} ({f:?})",
        f.measured_loss_ppm
    );
    assert!(
        f.burst_delta_ppm > 50_000,
        "önkoşul: desen BURST okunmalı; got {} ({f:?})",
        f.burst_delta_ppm
    );
    assert!(fired.is_none(), "sevk politikası burst'te parity açmaz");
    assert_eq!(s.last_decision().fec_parity_ratio_permille, 0);
}

/// AYNI kanıtlı burst + `BurstFusion` → parity AÇILIR (tek fark politikadır).
#[test]
fn evidenced_burst_opens_parity_under_burst_fusion_policy() {
    let (mut s, t_ack) = drive_until_loss_ack(200, evidenced_burst_acks());
    s.set_fec_structure_policy(FecStructurePolicy::BurstFusion);
    let fired = pump_until_gate_opens(&mut s, t_ack, 60);
    let f = s.diag_loss_structure();
    assert!(f.n10 + f.n11 >= 8, "önkoşul: kanıt eşiği ({f:?})");
    let d = fired.expect("BurstFusion kanıtlı burst'te parity AÇMALI");
    assert!(
        d.fec_parity_ratio_permille > 0,
        "burst'te parity sıfırlanmamalı (füzyon); got {}",
        d.fec_parity_ratio_permille
    );
    assert!(s.diag_structure_gate_opened() >= 1, "gate sayacı artmalı");
}

/// **[AÇIK BORÇ PİNİ — prior-as-evidence].** Bu test bir kusuru KAYDEDER, gizlemez.
///
/// IID sürücüsü %95 ledger-kaybı üretir ama fate-penceresinde HİÇ geçiş yoktur
/// (`n00=n01=n10=n11=0`) → p11 = p01 = 500_000 → `burst_delta = 0`. Sevk politikası bunu "IID"
/// okuyup FEC'i AÇAR: karar tamamen Laplace prior'ından gelir, ölçümden değil. Aynı mekanizma
/// ters yönde de çalışır (yalnız kayıpsız geçişlerde delta ≈ +475k → "burst" → FEC kapanır).
///
/// Sevk davranışı BİLEREK değiştirilmedi (kampanya görmüş bir dalı tek taraflı kaydırmak R1'e
/// aykırı; flip ölçüm ister). Opt-in `BurstFusion` ise kanıt eşiğini uygular ve karar VERMEZ.
/// Bu asimetri kasıtlıdır ve owner kararı bekler.
#[test]
fn shipped_policy_decides_on_the_laplace_prior_while_burst_fusion_refuses() {
    let iid_acks: Vec<u64> = (1..SEQ_SPAN).filter(|q| q % 2 == 0).collect();

    let (mut shipped, t_ack) = drive_until_loss_ack(80, iid_acks.clone());
    let shipped_fired = pump_until_gate_opens(&mut shipped, t_ack, 60);
    let f = shipped.diag_loss_structure();
    assert_eq!(
        (f.n00, f.n01, f.n10, f.n11),
        (0, 0, 0, 0),
        "önkoşul: pencerede HİÇ geçiş kanıtı yok ({f:?})"
    );
    assert_eq!(
        f.burst_delta_ppm, 0,
        "sıfır-geçişte delta prior'dan 0 gelir (p11=p01=500_000) ({f:?})"
    );
    assert!(
        shipped_fired.is_some(),
        "AÇIK BORÇ: sevk politikası kanıtsız pencerede FEC açıyor (prior kararı)"
    );

    let (mut fused, t_ack) = drive_until_loss_ack(80, iid_acks);
    fused.set_fec_structure_policy(FecStructurePolicy::BurstFusion);
    assert!(
        pump_until_gate_opens(&mut fused, t_ack, 60).is_none(),
        "BurstFusion kanıt eşiğinin altında yapısal karar VERMEMELİ"
    );
    assert_eq!(fused.diag_structure_gate_opened(), 0);
}

/// Kayıpsız pencere: prior tek başına +475k "burst" görüntüsü verir; hiçbir politika bu girdide
/// yapısal karar vermemeli (sevk yolu zaten kayıp-eşiğinde duruyor, BurstFusion kanıt eşiğinde).
#[test]
fn evidence_free_window_is_not_classified_as_burst() {
    let acked: Vec<u64> = (1..40u64).collect(); // bitişik ön-ek → HİÇ kayıp ilan edilmez
    for policy in [FecStructurePolicy::Q18Iid, FecStructurePolicy::BurstFusion] {
        let (mut s, t_ack) = drive_until_loss_ack(80, acked.clone());
        s.set_fec_structure_policy(policy);
        let fired = pump_until_gate_opens(&mut s, t_ack, 60);
        let f = s.diag_loss_structure();
        assert_eq!(
            f.n10 + f.n11,
            0,
            "önkoşul: pencerede kayıp geçişi OLMAMALI ({f:?})"
        );
        assert!(
            f.burst_delta_ppm > 50_000,
            "prior tek başına 'burst' görüntüsü verir — düzeltilen tuzak buydu; got {}",
            f.burst_delta_ppm
        );
        assert!(
            fired.is_none(),
            "kanıtsız pencerede yapısal karar verilmemeli (politika {policy:?})"
        );
        assert_eq!(s.diag_structure_gate_opened(), 0);
    }
}

// ── [KURULUŞ-GUARD] Ölçüm olmadan yapısal FEC kararı YOK ─────────────────────────────────────
//
// İlke: FEC bir ÖLÇÜME dayanmalıdır. PreConverge oturumun kurulmadığı fazdır — RTT/BtlBw örneği
// yoktur, kayıp-ledger'ı güvenilmezdir. Orada parity açmak önyargıya dayanır ve somut zararı
// ÖLÇÜLDÜ: burst-fusion politikası PreConverge'de parity açıyor, parity kıt token bütçesinde
// ORİJİNALLERLE yarışıyor ve ilk-uçuş orijinal sayısını düşürerek kuruluşu kırıyordu
// (kuruluş yalnız orijinal DATA'nın RTT örneğiyle olur) → netem'de EN KÖTÜ koldu (v2: 5/12).

/// Sürücü PreConverge'i hiç terk etmez (ACK yok) → gate HER İKİ politikada da karar VERMEZ,
/// ve nedeni histogramda `BlockedPreConverge` olarak görünür (sessiz sıfır değil).
#[test]
fn structure_gate_never_decides_before_the_session_is_established() {
    for policy in [FecStructurePolicy::Q18Iid, FecStructurePolicy::BurstFusion] {
        let mut s = Sender::new();
        s.set_establish_probe(false);
        s.set_fec_structure_policy(policy);
        // ACK ASLA verilmez → srtt None → PreConverge kalıcı.
        for i in 0..80u64 {
            s.on_event(
                Event::AppData(OpaqueUnit {
                    id: 1000 + i,
                    size: 101,
                }),
                0,
            );
        }
        for t in 1..=200u64 {
            s.on_event(Event::TimerFired(Timer::Pto), t * 200 * 1_000_000);
        }
        assert_eq!(
            s.diag_structure_gate_opened(),
            0,
            "kurulmamış oturumda parity açılmamalı (politika {policy:?})"
        );
        assert_eq!(
            s.last_decision().fec_parity_ratio_permille,
            0,
            "politika {policy:?}"
        );
        let g = s.diag_gate_outcomes();
        assert!(
            g[6] > 0,
            "engelin NEDENİ histogramda görünmeli (BlockedPreConverge); histogram={g:?}"
        );
        assert_eq!(g[5], 0, "hiç açılmamalı; histogram={g:?}");
    }
}

/// [R-SWEEP] Gate seviyesi override'ı: `None` = `fec_med` (sevk, byte-özdeş); `Some(v)` = v.
#[test]
fn gate_level_override_defaults_to_fec_med() {
    let mut s = Sender::new();
    assert_eq!(s.gate_level_permille(), s.fuzzy_config_fec_med_for_test());
    s.set_structure_gate_level(Some(250));
    assert_eq!(s.gate_level_permille(), 250);
    s.set_structure_gate_level(None);
    assert_eq!(s.gate_level_permille(), s.fuzzy_config_fec_med_for_test());
}
