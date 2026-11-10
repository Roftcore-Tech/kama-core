//! [S2-A5-min / HB2] `metrics()` `rtt_ms` / `loss_pct` (KK-H59) — salt okunur.
//!
//! Kaynak: `ClientTransport::session_live_diags()` (+ kapanan oturumların toplamı
//! `retired_live_diag()`). Canlı ayna yalnız `RuntimeConfig::live_diag` açıkken vardır (ürün
//! varsayılanı KAPALI, DUR-19); kapalıyken her değer `ClientLiveDiag::default()`'tur ve
//! `clock_origin` `None`'dır. O durumda bu modül değer ÜRETMEZ (`None`: "uydurma değer verilmez",
//! `NodeMetrics` belgesi). Motor davranışı değişmez; hub kipi canlı aynayı kendiliğinden açmaz.
//!
//! * `rtt_ms` = eşin hazır oturumunun `srtt_us / 1000`; üst düzey = hazır oturumların en küçüğü.
//!   Örneği olmayan (`srtt_us = 0`) oturum sayılmaz.
//! * `loss_pct` = son [`LOSS_WINDOW`]'daki `retransmits` artışı / `tx_datagrams` artışı × 100 (A5
//!   kartı). Pencerede datagram gönderilmediyse `None`.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::time::{Duration, Instant};

use client_core::runtime::ClientLiveDiag;
use kama_contracts::handles::SessionHandle;

/// `loss_pct` penceresi (A5 kartı: 10 s).
pub(crate) const LOSS_WINDOW: Duration = Duration::from_secs(10);

/// Pencere örnekleme aralığı; örnek sayısı `LOSS_WINDOW / SAMPLE_PERIOD + 2` ile sınırlı.
const SAMPLE_PERIOD: Duration = Duration::from_secs(1);

/// Örnek tavanı (SR-AUDIT-001).
const MAX_SAMPLES: usize = 12;

/// Toplam sayaçların zaman örnekleri: `(an, retransmits, tx_datagrams)`.
#[derive(Debug, Default)]
pub(crate) struct LossWindow {
    samples: VecDeque<(Instant, u64, u64)>,
}

impl LossWindow {
    /// Toplamları gözler; pencere içindeki kayıp yüzdesini döner.
    pub(crate) fn observe(&mut self, now: Instant, retx: u64, tx: u64) -> Option<f32> {
        // Toplamlar geri giderse (sayaç tabanı değişti) pencere baştan başlar.
        if let Some(&(_, r, t)) = self.samples.back() {
            if retx < r || tx < t {
                self.samples.clear();
            }
        }
        let due = self
            .samples
            .back()
            .is_none_or(|&(at, ..)| now.saturating_duration_since(at) >= SAMPLE_PERIOD);
        if due {
            self.samples.push_back((now, retx, tx));
        }
        // Taban = pencerenin başından eski olmayan en eski örnek; ondan eskiler düşer.
        while self.samples.len() >= 2
            && now.saturating_duration_since(self.samples[1].0) >= LOSS_WINDOW
        {
            self.samples.pop_front();
        }
        while self.samples.len() > MAX_SAMPLES {
            self.samples.pop_front();
        }
        let &(_, r0, t0) = self.samples.front()?;
        let sent = tx.saturating_sub(t0);
        if sent == 0 {
            return None;
        }
        #[allow(clippy::cast_precision_loss)] // sayaçlar ölçü için f32'ye iner (overlay değeri)
        Some(retx.saturating_sub(r0) as f32 / sent as f32 * 100.0)
    }
}

/// Düğüm turunun A5 durumu (`NodeLoop` alanı; `snapshot(&self)` içinden güncellenir).
#[derive(Debug, Default)]
pub(crate) struct A5 {
    loss: RefCell<LossWindow>,
}

/// Ayna var mı (ayna kapalıyken `clock_origin` `None`).
fn mirrored(diag: &ClientLiveDiag) -> bool {
    diag.clock_origin.is_some()
}

/// Hazır oturumların en küçük sRTT'si (ms).
pub(crate) fn rtt_ms(
    diags: &[(SessionHandle, ClientLiveDiag)],
    ready: &[SessionHandle],
) -> Option<u32> {
    diags
        .iter()
        .filter(|(h, d)| ready.contains(h) && mirrored(d) && d.srtt_us > 0)
        .map(|(_, d)| u32::try_from(d.srtt_us / 1000).unwrap_or(u32::MAX))
        .min()
}

impl A5 {
    /// Son pencerenin kayıp yüzdesi; ayna yoksa `None` (pencere de ilerlemez).
    pub(crate) fn loss_pct(
        &self,
        now: Instant,
        diags: &[(SessionHandle, ClientLiveDiag)],
        retired: &ClientLiveDiag,
    ) -> Option<f32> {
        if !diags.iter().any(|(_, d)| mirrored(d)) {
            return None;
        }
        let (retx, tx) = diags
            .iter()
            .map(|(_, d)| d)
            .chain(std::iter::once(retired))
            .fold((0u64, 0u64), |(r, t), d| {
                (
                    r.saturating_add(d.retransmits),
                    t.saturating_add(d.tx_datagrams),
                )
            });
        self.loss.borrow_mut().observe(now, retx, tx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn diag(srtt_us: u64, retx: u64, tx: u64, mirror: bool) -> ClientLiveDiag {
        ClientLiveDiag {
            srtt_us,
            retransmits: retx,
            tx_datagrams: tx,
            clock_origin: mirror.then(Instant::now),
            ..ClientLiveDiag::default()
        }
    }

    /// Sahte saatle 10 s pencere: taban pencerenin başından eski olmayan en eski örnektir.
    #[test]
    fn loss_window_uses_the_last_ten_seconds() {
        let t0 = Instant::now();
        let s = Duration::from_secs;
        let mut w = LossWindow::default();
        assert_eq!(w.observe(t0, 0, 0), None, "gönderim yok");
        assert_eq!(w.observe(t0 + s(1), 1, 10), Some(10.0));
        assert_eq!(w.observe(t0 + s(5), 3, 50), Some(6.0));
        // t0+11: t0 örneği pencere dışında → taban t0+1 (1, 10): (3-1)/(60-10).
        assert_eq!(w.observe(t0 + s(11), 3, 60), Some(4.0));
        // Uzun sessizlik: taban t0+11 → pencerede gönderim yok.
        assert_eq!(w.observe(t0 + s(30), 3, 60), None);
        assert!(w.samples.len() <= MAX_SAMPLES);
        // Saniyeden sık gözlem yeni örnek eklemez.
        let n = w.samples.len();
        let _ = w.observe(t0 + s(30) + Duration::from_millis(20), 3, 61);
        assert_eq!(w.samples.len(), n);
    }

    #[test]
    fn decreasing_totals_restart_the_window() {
        let t0 = Instant::now();
        let mut w = LossWindow::default();
        let _ = w.observe(t0, 5, 100);
        assert_eq!(w.observe(t0 + Duration::from_secs(2), 1, 10), None);
        assert_eq!(
            w.observe(t0 + Duration::from_secs(3), 2, 20),
            Some(10.0),
            "yeni taban (1, 10)"
        );
    }

    #[test]
    fn many_samples_stay_bounded() {
        let t0 = Instant::now();
        let mut w = LossWindow::default();
        for i in 0..1000u64 {
            let _ = w.observe(t0 + Duration::from_millis(i * 1500), i, i * 10);
        }
        assert!(w.samples.len() <= MAX_SAMPLES);
    }

    /// `rtt_ms`: yalnız hazır, aynalı ve örnekli oturumlar; en küçük.
    #[test]
    fn rtt_is_the_minimum_over_ready_mirrored_sessions() {
        let (a, b, c) = (
            SessionHandle::from_raw(1),
            SessionHandle::from_raw(2),
            SessionHandle::from_raw(3),
        );
        let diags = vec![
            (a, diag(42_500, 0, 0, true)),
            (b, diag(7_900, 0, 0, true)),
            (c, diag(1_000, 0, 0, true)),
        ];
        assert_eq!(rtt_ms(&diags, &[a, b]), Some(7), "c hazır değil");
        assert_eq!(rtt_ms(&diags, &[]), None);
        let off = vec![(a, diag(42_500, 0, 0, false))];
        assert_eq!(rtt_ms(&off, &[a]), None, "ayna kapalı");
        let unsampled = vec![(a, diag(0, 0, 0, true))];
        assert_eq!(rtt_ms(&unsampled, &[a]), None, "örnek yok");
    }

    /// `loss_pct`: ayna kapalıyken `None` ve pencere ilerlemez; kapanan oturumlar toplama girer.
    #[test]
    fn loss_needs_a_mirror_and_counts_retired_sessions() {
        let a5 = A5::default();
        let h = SessionHandle::from_raw(1);
        let t0 = Instant::now();
        let off = vec![(h, diag(0, 9, 90, false))];
        assert_eq!(a5.loss_pct(t0, &off, &ClientLiveDiag::default()), None);
        assert!(a5.loss.borrow().samples.is_empty());
        let on = vec![(h, diag(0, 0, 0, true))];
        assert_eq!(a5.loss_pct(t0, &on, &ClientLiveDiag::default()), None);
        let on = vec![(h, diag(0, 1, 40, true))];
        let retired = diag(0, 1, 10, false);
        assert_eq!(
            a5.loss_pct(t0 + Duration::from_secs(1), &on, &retired),
            Some(4.0),
            "(1+1)/(40+10)"
        );
    }

    // ── uçtan uca: düğüm ölçüsü ──────────────────────────────────────────────────────────

    use crate::test_support::{event_channel, serial, TempDir};
    use crate::{KamaNode, NodeEvent, NodeListener};
    use kama_provision::format::DeviceName;
    use kama_provision::provision::InitRequest;
    use std::net::{SocketAddr, UdpSocket};
    use std::sync::mpsc::Sender;

    struct Fwd(Sender<NodeEvent>);
    impl NodeListener for Fwd {
        fn on_event(&self, event: NodeEvent) {
            let _ = self.0.send(event);
        }
    }

    /// `alice` + `bob`, doğrudan kip (127.0.0.1 boş portlar).
    fn pair(root: &TempDir) -> [std::path::PathBuf; 2] {
        let names = [
            DeviceName::new("alice").expect("ad"),
            DeviceName::new("bob").expect("ad"),
        ];
        let sockets: Vec<UdpSocket> = (0..2)
            .map(|_| UdpSocket::bind("127.0.0.1:0").expect("soket"))
            .collect();
        let ports: Vec<SocketAddr> = sockets
            .iter()
            .map(|s| s.local_addr().expect("adres"))
            .collect();
        drop(sockets);
        let out = root.path().join("authority");
        crate::wiring::provision_for_test(&InitRequest {
            out_dir: out.clone(),
            devices: names.to_vec(),
            hub: None,
            direct: names.iter().cloned().zip(ports).collect(),
        });
        names.map(|n| out.join(n.file_name()))
    }

    /// KK-H59 / HB2-M3 DONE: canlı ayna AÇIK düğümde trafik sonrası `rtt_ms` ve `loss_pct`
    /// `Some`; ürün kurucusunda (ayna KAPALI) ikisi de `None`. Doğrudan kipte
    /// `last_heartbeat_age_s` `None`. Motor çağrı dizisi değişmez (yalnız okuma).
    #[test]
    fn live_mirror_fills_rtt_and_loss_and_product_path_stays_none() {
        let _serial = serial();
        for live in [true, false] {
            let root = TempDir::new(if live { "a5-live" } else { "a5-product" });
            let [alice_cfg, bob_cfg] = pair(&root);
            let (atx, mut alice_events) = event_channel();
            let (btx, mut bob_events) = event_channel();
            let start = |cfg: &std::path::Path, data: &str, tx| {
                let data = root.path().join(data);
                let l: Box<dyn NodeListener> = Box::new(Fwd(tx));
                if live {
                    KamaNode::start_live_diag_for_test(cfg, &data, l)
                } else {
                    KamaNode::start(cfg, &data, l)
                }
                .expect("düğüm")
            };
            let alice = start(&alice_cfg, "alice-data", atx);
            let bob = start(&bob_cfg, "bob-data", btx);
            alice.connect("bob").expect("connect");
            for events in [&mut alice_events, &mut bob_events] {
                events.wait("Connected", |e| {
                    matches!(e, NodeEvent::Connected { .. }).then_some(())
                });
            }
            for i in 0..5 {
                alice.send_text("bob", &format!("a5-{i}")).expect("metin");
                bob_events.wait("TextReceived", |e| {
                    matches!(e, NodeEvent::TextReceived { .. }).then_some(())
                });
            }
            let metrics = crate::test_support::until("ölçü", || {
                let m = alice.metrics();
                (!live || (m.rtt_ms.is_some() && m.loss_pct.is_some())).then_some(m)
            });
            if live {
                assert!(
                    metrics.loss_pct.is_some_and(|l| (0.0..=100.0).contains(&l)),
                    "{metrics:?}"
                );
            } else {
                assert_eq!(
                    (metrics.rtt_ms, metrics.loss_pct),
                    (None, None),
                    "{metrics:?}"
                );
            }
            assert_eq!(metrics.last_heartbeat_age_s, None, "doğrudan kip (C9)");
            alice.shutdown().expect("kapanış");
            bob.shutdown().expect("kapanış");
        }
    }
}
