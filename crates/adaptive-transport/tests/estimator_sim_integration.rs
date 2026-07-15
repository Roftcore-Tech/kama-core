//! LinkSim + Clock → BW estimator entegrasyonu: sim/clock estimator'ı deterministik
//! besler (ADR-17). `tokio::time::pause` altında wall-clock yok; iki koşu birebir aynı.

use adaptive_transport::bw::{BtlBwFilter, DeliveryRateSampler};
use adaptive_transport::clock::{Clock, TokioClock};
use adaptive_transport::sim::{Fate, LinkSim};
use std::time::Duration;

async fn run() -> u64 {
    let clock = TokioClock;
    let base = clock.now();
    // 4 paket, hepsi 40ms tek-yön gecikmeyle teslim.
    let sim = LinkSim::new(
        vec![Fate::Deliver(Duration::from_millis(40)); 4],
        Fate::Drop,
    );

    let mut sampler = DeliveryRateSampler::new();
    let mut filter = BtlBwFilter::new();

    // t=0'da 4 paket gönder → token'lar.
    let tokens: Vec<_> = (0..4)
        .map(|_| sampler.on_sent(0, 1250, false, true))
        .collect();

    // Sim teslimleri (clock üzerinden zamanlanır); varış anları estimator'ı besler.
    for d in sim.deliver(&clock, 4).await {
        let now_ns = (clock.now() - base).as_nanos() as u64;
        let rs = sampler.on_ack(now_ns, tokens[d.index]);
        filter.update(rs, now_ns, true);
    }
    filter.btlbw()
}

#[tokio::test(start_paused = true)]
async fn sim_feeds_bw_estimator_deterministically() {
    let a = run().await;
    let b = run().await;
    assert_eq!(a, b, "sim + clock estimator'ı deterministik besliyor");
    assert!(a > 0, "en az bir geçerli BtlBw sample'ı");
}
