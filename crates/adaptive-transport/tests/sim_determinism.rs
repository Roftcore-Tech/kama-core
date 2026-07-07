//! Link sim determinizm + reorder kanıtı (ADR-17). `tokio::time::pause` altında
//! wall-clock yok; aynı script → birebir aynı teslim sırası.

use adaptive_transport::clock::TokioClock;
use adaptive_transport::sim::{Delivery, Fate, LinkSim};
use std::time::Duration;

fn ms(n: u64) -> Duration {
    Duration::from_millis(n)
}

#[tokio::test(start_paused = true)]
async fn sim_reorders_and_is_deterministic() {
    // idx0 geç (30ms), idx1 düşer, idx2 erken (5ms), idx3 orta (10ms)
    // → varış sırası [2, 3, 0]; idx1 kayıp.
    let script = vec![
        Fate::Deliver(ms(30)),
        Fate::Drop,
        Fate::Deliver(ms(5)),
        Fate::Deliver(ms(10)),
    ];
    let sim = LinkSim::new(script, Fate::Drop);
    let clock = TokioClock;

    let run1 = sim.deliver(&clock, 4).await;
    let run2 = sim.deliver(&clock, 4).await;

    let order: Vec<usize> = run1.iter().map(|d| d.index).collect();
    assert_eq!(order, vec![2, 3, 0], "reorder emergent + loss");

    assert_eq!(run1, run2, "aynı script iki kez → birebir aynı sonuç");

    assert_eq!(
        run1,
        vec![
            Delivery {
                index: 2,
                delay: ms(5)
            },
            Delivery {
                index: 3,
                delay: ms(10)
            },
            Delivery {
                index: 0,
                delay: ms(30)
            },
        ]
    );
}

#[tokio::test(start_paused = true)]
async fn sim_default_fate_applies_beyond_script() {
    // Script 2 paket tanımlar; 3. paket default=Deliver(1ms) alır.
    let sim = LinkSim::new(vec![Fate::Drop, Fate::Deliver(ms(2))], Fate::Deliver(ms(1)));
    let clock = TokioClock;
    let out = sim.deliver(&clock, 3).await;
    let order: Vec<usize> = out.iter().map(|d| d.index).collect();
    assert_eq!(order, vec![2, 1], "idx0 drop; idx2(1ms) idx1(2ms)'den önce");
}
