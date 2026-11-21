//! [İŞ-32 · kenar kuralı · sahip kararı 2026-11-21] Kayıp dedektörünün dolu ACK çerçevesi kuralı
//! (`ThresholdLossDetector::on_ack_partial`) ve delik ucu çıkarımı (`loss::ack_hole_edges`).
//!
//! * dolu çerçeve hiç işlenmediyse dedektör bugünkü kuralla birebir aynıdır (bağımsız referans
//!   modelle karşılaştırma, belirlenimli betik);
//! * dolu çerçevede yalnız delik uçları ve dolu olmayan çerçevenin en büyüğünün altı kayıp
//!   sayılabilir; diğer bildirilmemiş seq'ler paket eşiğinde de zaman eşiğinde de bilinmiyor kalır;
//! * bilinmeyen seq sonra ACK'lenirse un-loss doğmaz; dolu olmayan bir çerçeve onu kanıtlarsa bugünkü
//!   gibi ilan edilir; gönderici bıraktığında dedektörden de düşer.

use std::collections::BTreeMap;
use std::time::Duration;

use adaptive_transport::loss::{
    ack_hole_edges, packet_threshold_lost, time_threshold_lost, CoverageDiag, LossDetector,
    ThresholdLossDetector, TIME_THRESH_DEN, TIME_THRESH_NUM,
};

const MS: u64 = 1_000_000;

fn range(lo: u64, hi: u64) -> Vec<u64> {
    (lo..=hi).collect()
}

#[test]
fn hole_edges_are_the_two_ends_of_each_interior_hole() {
    // Bloklar: 10..=20, 22..=23 (tek PN'lik delik 21), 26..=30 (iki PN'lik delik 24, 25),
    // 40..=50 (dokuz PN'lik delik 31..=39: yalnız uçlar 31 ve 39). En alttaki bloğun altı (≤ 9)
    // hiç üretilmez.
    let mut pns = range(40, 50);
    pns.extend(range(26, 30));
    pns.extend(range(22, 23));
    pns.extend(range(10, 20));
    assert_eq!(ack_hole_edges(&pns), vec![21, 24, 25, 31, 39]);
    // Sıra ve tekrar önemsiz.
    let mut shuffled = pns.clone();
    shuffled.reverse();
    shuffled.extend([40, 10, 26]);
    assert_eq!(ack_hole_edges(&shuffled), vec![21, 24, 25, 31, 39]);
    // Bitişik bloklar tek koşudur: delik yok.
    assert!(ack_hole_edges(&range(5, 9)).is_empty());
    assert!(ack_hole_edges(&[]).is_empty());
    assert!(ack_hole_edges(&[7]).is_empty());
    // u16 sarımını aşan tam PN'ler (çözülmüş full-counter).
    let mut wrap = range(65_530, 65_540);
    wrap.extend(range(65_500, 65_510));
    assert_eq!(ack_hole_edges(&wrap), vec![65_511, 65_529]);
}

#[test]
fn hole_edges_are_withheld_when_the_decoded_list_hit_its_cap() {
    let cap = adaptive_transport::ack::MAX_ACKED_SEQS;
    let mut pns: Vec<u64> = (0..cap as u64 - 1).collect();
    pns.push(cap as u64 + 10); // delik var ama liste tavanda: en alt blok kesilmiş olabilir
    assert_eq!(pns.len(), cap);
    assert!(ack_hole_edges(&pns).is_empty());
}

/// Bugünkü kuralın bağımsız referans modeli (paket eşiği, zaman eşiği, uyanış vadesi).
#[derive(Default)]
struct Reference {
    inflight: BTreeMap<u64, u64>,
    la: Option<u64>,
}

impl Reference {
    fn on_ack(&mut self, acked: &[u64], k: u64) -> Vec<u64> {
        for &s in acked {
            self.inflight.remove(&s);
            self.la = Some(self.la.map_or(s, |l| l.max(s)));
        }
        let Some(la) = self.la else {
            return Vec::new();
        };
        let lost: Vec<u64> = self
            .inflight
            .keys()
            .copied()
            .filter(|&s| packet_threshold_lost(s, la, k))
            .collect();
        for s in &lost {
            self.inflight.remove(s);
        }
        lost
    }

    fn detect_time(&mut self, now: u64, srtt: Duration) -> Vec<u64> {
        let Some(la) = self.la else {
            return Vec::new();
        };
        let lost: Vec<u64> = self
            .inflight
            .iter()
            .filter(|(&s, &t)| {
                s < la
                    && time_threshold_lost(
                        Duration::from_nanos(now.saturating_sub(t)),
                        srtt,
                        Duration::from_millis(1),
                        TIME_THRESH_NUM,
                        TIME_THRESH_DEN,
                    )
            })
            .map(|(&s, _)| s)
            .collect();
        for s in &lost {
            self.inflight.remove(s);
        }
        lost
    }

    fn deadline(&self, srtt: Duration) -> Option<u64> {
        let la = self.la?;
        let thresh = (srtt.as_nanos() as u64 * TIME_THRESH_NUM / TIME_THRESH_DEN).max(MS);
        self.inflight
            .iter()
            .filter(|(&s, _)| s < la)
            .map(|(_, &t)| t + thresh)
            .min()
    }
}

/// Dolu çerçeve hiç işlenmeden (`on_ack` yolu): ilanlar, uyanış vadeleri ve sayaçlar bağımsız
/// referans modelle birebir aynı. Betik belirlenimli (LCG), ~10 000 olay; dolu çerçeve yolunu hiç
/// çağırmadan `forget_unevidenced` de hiçbir şeye dokunmaz.
#[test]
fn without_a_full_frame_the_detector_matches_todays_rule_exactly() {
    let mut d = ThresholdLossDetector::new();
    let mut r = Reference::default();
    let mut lcg: u64 = 0x2545_f491_4f6c_dd1d;
    let mut next = || {
        lcg = lcg
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        lcg >> 33
    };
    let srtt = Duration::from_millis(40);
    let mut now = 0u64;
    let mut seq = 0u64;
    let mut sent: Vec<u64> = Vec::new();
    let (mut n_pkt, mut n_time) = (0usize, 0usize);
    for step in 0..10_000u32 {
        now += 1 + next() % 5 * MS;
        match next() % 4 {
            0 | 1 => {
                d.on_sent(seq, now, 1);
                r.inflight.insert(seq, now);
                sent.push(seq);
                seq += 1;
            }
            2 => {
                // ACK: son gönderilenlerden rastgele seçilmiş alt küme (kümülatif değil, sırasız).
                let k = 3 + next() % 30;
                d.set_packet_threshold(k);
                let lo = sent.len().saturating_sub(40);
                let acked: Vec<u64> = sent[lo..]
                    .iter()
                    .copied()
                    .filter(|_| next() % 3 != 0)
                    .collect();
                let lost = d.on_ack(&acked, now);
                assert_eq!(
                    lost,
                    r.on_ack(&acked, k),
                    "adım {step}: paket eşiği ilanları ayrıştı"
                );
                n_pkt += lost.len();
            }
            _ => {
                assert_eq!(
                    d.next_time_loss_deadline_ns(srtt, Duration::ZERO),
                    r.deadline(srtt),
                    "adım {step}: uyanış vadesi ayrıştı"
                );
                let lost = d.detect_time_losses(now, srtt, Duration::ZERO);
                assert_eq!(
                    lost,
                    r.detect_time(now, srtt),
                    "adım {step}: zaman eşiği ilanları ayrıştı"
                );
                n_time += lost.len();
                if let Some(&s) = sent.get(next() as usize % sent.len().max(1)) {
                    assert!(
                        !d.forget_unevidenced(s),
                        "adım {step}: dolu çerçevesiz unutma"
                    );
                }
            }
        }
    }
    assert_eq!(d.diag_coverage(), CoverageDiag::default());
    // Betik boş değil: iki eşik de ilan üretti.
    assert!(n_pkt > 50 && n_time > 50, "paket {n_pkt}, zaman {n_time}");
}

fn sent_0_to_100() -> ThresholdLossDetector {
    let mut d = ThresholdLossDetector::new();
    for s in 0..=100 {
        d.on_sent(s, s * MS, 1);
    }
    d.set_packet_threshold(3);
    d
}

/// Dolu çerçeve: bloklar 90..=100, 40..=50, 10..=20. Delik uçları 21, 39, 51, 89 kanıtlıdır ve paket
/// eşiğiyle ilan edilir; deliklerin içi (22..=38, 52..=88) ve en alttaki bloğun altı (0..=9)
/// bilinmiyor: paket eşiği de, çok sonraki zaman eşiği de onları ilan etmez, uyanış vadesi doğmaz.
#[test]
fn a_full_frame_declares_only_its_hole_edges() {
    let mut d = sent_0_to_100();
    let mut acked = range(90, 100);
    acked.extend(range(40, 50));
    acked.extend(range(10, 20));
    let missing = ack_hole_edges(&acked);
    assert_eq!(missing, vec![21, 39, 51, 89]);
    let lost = d.on_ack_partial(&acked, &missing, 200 * MS);
    assert_eq!(lost, vec![21, 39, 51, 89]);
    let srtt = Duration::from_millis(10);
    assert_eq!(d.next_time_loss_deadline_ns(srtt, Duration::ZERO), None);
    assert!(d
        .detect_time_losses(10_000 * MS, srtt, Duration::ZERO)
        .is_empty());
    for s in (0..=9).chain(22..=38).chain(52..=88) {
        assert!(d.is_held_unknown(s), "seq {s} bilinmiyor olmalı");
    }
    assert!(!d.is_held_unknown(21), "ilan edilen seq uçuşta değil");
    assert_eq!(d.diag_coverage().partial_frames, 1);
    assert_eq!(
        d.diag_coverage().edge_evidenced,
        0,
        "kanıtlılar ilan edildi"
    );
    assert_eq!(d.lost_units(), 4);
}

/// Kenar kanıtı paket eşiğini doldurmuyorsa (k büyük) zaman eşiği onu bugünkü değerle ilan eder.
#[test]
fn an_edge_below_the_packet_threshold_falls_to_the_time_threshold() {
    let mut d = sent_0_to_100();
    d.set_packet_threshold(50);
    let mut acked = range(90, 100);
    acked.extend(range(40, 50));
    let missing = ack_hole_edges(&acked); // 51, 89
    assert!(d.on_ack_partial(&acked, &missing, 200 * MS).is_empty());
    let srtt = Duration::from_millis(10);
    let thresh = 10 * MS * TIME_THRESH_NUM / TIME_THRESH_DEN;
    // En erken vade kanıtlı 51'in vadesidir (bilinmeyen 0..=39 daha eski olduğu hâlde).
    assert_eq!(
        d.next_time_loss_deadline_ns(srtt, Duration::ZERO),
        Some(51 * MS + thresh)
    );
    assert_eq!(
        d.detect_time_losses(89 * MS + thresh, srtt, Duration::ZERO),
        vec![51, 89]
    );
}

/// Bilinmeyen seq sonraki dolu OLMAYAN çerçevede de bildirilmezse artık kanıtlıdır ve bugünkü gibi
/// ilan edilir (çerçeve alıcının aldığı her şeyi bildirir).
#[test]
fn a_later_complete_frame_evidences_the_unknown_seqs() {
    let mut d = sent_0_to_100();
    let acked = range(90, 100);
    assert!(d.on_ack_partial(&acked, &[], 200 * MS).is_empty());
    assert!(d.is_held_unknown(5));
    // Dolu olmayan çerçeve: 0..=100 içinden yalnız 0..=4 eksik.
    let lost = d.on_ack(&range(5, 100), 300 * MS);
    assert_eq!(lost, vec![0, 1, 2, 3, 4]);
}

/// Bilinmeyen seq sonra ACK'lenirse kayıp ilanı hiç olmadığı için un-loss (spurious) doğmaz;
/// reorder toleransını şişiren kanıt da doğmaz.
#[test]
fn an_unknown_seq_acked_later_is_not_an_unloss() {
    let mut d = sent_0_to_100();
    let mut acked = range(90, 100);
    acked.extend(range(40, 50));
    let missing = ack_hole_edges(&acked);
    let _ = d.on_ack_partial(&acked, &missing, 200 * MS);
    let late = range(0, 30);
    assert!(d.on_ack_partial(&late, &[], 300 * MS).is_empty());
    assert!(d.take_unlossed().is_empty());
    assert_eq!(d.spurious_units_total(), 0);
}

/// Gönderici kaydı bıraktığında bilinmeyen seq dedektörden düşer; kanıtlı seq'e dokunulmaz.
#[test]
fn forgetting_drops_only_unknown_seqs() {
    let mut d = sent_0_to_100();
    d.set_packet_threshold(50);
    let mut acked = range(90, 100);
    acked.extend(range(40, 50));
    let missing = ack_hole_edges(&acked); // 51, 89 kanıtlı ama k = 50'yi doldurmuyor
    let _ = d.on_ack_partial(&acked, &missing, 200 * MS);
    assert!(d.forget_unevidenced(60), "bilinmeyen unutulur");
    assert!(!d.forget_unevidenced(60), "ikinci kez yok");
    assert!(!d.forget_unevidenced(51), "kanıtlı unutulmaz");
    assert!(!d.forget_unevidenced(95), "ACK'li zaten yok");
    assert_eq!(d.diag_coverage().forgotten, 1);
    assert_eq!(d.diag_coverage().edge_evidenced, 2);
}
