//! Commit-S — CANDIDATE-ADVERSARIAL-SIMULATION (talimat-§14): 4-candidate × zorunlu-trace-listesi,
//! SAF-kernel-üstünde (üretim-scheduling'e-bağlı-DEĞİL; policy-uygulaması-DEĞİL — karşılaştırma-verisi).
//! Determinism: tüm-trace'ler-deterministik (bursty-trace'ler sabit-seed-xorshift); contended-trace-
//! çift-koşu-bit-özdeşlik CI-smoke'ta (DUR-koşulu-denetimi). Çıktı: BKS_OUT/bks-results.csv.

use adaptive_transport::budget_kernel::{
    admit, settle, step, t_floor_ns, AdmissionCaps, AdmissionResult, BudgetContext,
    BudgetPolicyState, PolicyKind, PolicyParams,
};
use std::io::Write as _;

const UNIT: u64 = 1200;
const TICK_NS: u64 = 10_000_000; // 10ms scheduling-opportunity

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        let mut x = self.0.max(1);
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
}

/// Trace-tanımı: her-tick (data_demand, arq_demand, yeni-blok-sayısı, cancel-blok?, parity_size, srtt,
/// rate, elapsed) üreten-deterministik-jeneratör.
#[derive(Clone, Copy)]
struct Trace {
    name: &'static str,
    ticks: u64,
    rate: u64,
    srtt_ns: u64,
    parity_size: u64,
    data_per_tick: u64,
    burst_every: u64,       // 0=sabit; N=her-N.-tick'te-data (burst)
    block_every_ticks: u64, // 0=tek-blok-başta; N=her-N-tick'te-yeni-blok (planned-r4)
    cancel_at_tick: u64,    // 0=yok
    idle_gap_at: u64,       // 0=yok; N: o-tick'te elapsed=5s (scheduler-boşluğu)
    switch_policy_at: u64,  // 0=yok (mode-transition-trace'i)
    size_switch_at: u64,    // 0=yok; sonrası parity_size=1061
    seed: u64,              // bursty-jitter (0=deterministik-sabit)
}

const fn base(name: &'static str) -> Trace {
    Trace {
        name,
        ticks: 3_000, // 30s
        rate: 60_000,
        srtt_ns: 100_000_000,
        parity_size: UNIT,
        data_per_tick: 600,
        burst_every: 0,
        block_every_ticks: 50, // 500ms'de-1-blok (r4)
        cancel_at_tick: 0,
        idle_gap_at: 0,
        switch_policy_at: 0,
        size_switch_at: 0,
        seed: 0,
    }
}

fn traces() -> Vec<Trace> {
    vec![
        Trace {
            data_per_tick: 600,
            ..base("offered-eq-C")
        }, // rate 60k×10ms=600B/tick tam-doyum
        Trace {
            data_per_tick: 900,
            ..base("offered-gt-C")
        },
        Trace {
            data_per_tick: 0,
            ..base("no-data-demand")
        },
        Trace {
            burst_every: 4,
            data_per_tick: 2_400,
            seed: 11,
            ..base("bursty-data")
        },
        Trace {
            block_every_ticks: 0,
            ticks: 500,
            ..base("one-block-r4")
        },
        Trace {
            block_every_ticks: 5,
            ..base("many-blocks-gt-floor-capacity")
        }, // 50ms'de-blok
        Trace {
            block_every_ticks: 25,
            ..base("interleaved-blocks")
        }, // örtüşen-yaşam
        Trace {
            srtt_ns: 50_000_000,
            ..base("rtt-50ms")
        },
        Trace {
            srtt_ns: 1_000_000_000,
            ..base("rtt-1s")
        },
        Trace {
            srtt_ns: 5_000_000_000,
            ..base("rtt-5s")
        },
        Trace {
            rate: 3_750,
            data_per_tick: 30,
            ..base("low-bw-30kbps-serialization")
        },
        Trace {
            idle_gap_at: 1_000,
            ..base("no-scheduler-events-gap")
        },
        Trace {
            switch_policy_at: 1_500,
            ..base("mode-transition")
        },
        Trace {
            cancel_at_tick: 1_200,
            ..base("recovered-cancellation")
        },
        Trace {
            data_per_tick: 900,
            block_every_ticks: 10,
            ..base("cap-exhausted")
        },
        Trace {
            data_per_tick: 0,
            block_every_ticks: 0,
            idle_gap_at: 100,
            ticks: 400,
            ..base("credit-max-after-idle")
        },
        Trace {
            idle_gap_at: 200,
            ticks: 400,
            ..base("timer-wrap-boundary")
        }, // elapsed=u64::MAX/2-adımı
        Trace {
            size_switch_at: 1_500,
            ..base("parity-size-change")
        },
        Trace {
            block_every_ticks: 2,
            ticks: 600,
            ..base("pending-item-cap")
        }, // kuyruk-cap-baskısı
        Trace {
            parity_size: 2_000,
            block_every_ticks: 5,
            ticks: 600,
            ..base("pending-byte-cap")
        },
    ]
}

#[derive(Debug, Default, Clone, PartialEq)]
struct SimOut {
    floor_admitted: u64,
    floor_serviced: u64,
    floor_missed: u64, // admitted-ticket T_floor-aşımıyla-servis-veya-hiç
    bulk_emitted_bytes: u64,
    data_displaced_bytes: u64, // rezervasyonun-DATA'dan-kestiği (o-tick-talep-vardı-ve-limit-yetmedi)
    max_data_backlog: u64,
    max_reserved_pool: u64,
    max_parity_delay_ticks: u64,
    cap_suppressed_events: u64,
    max_credit: u64,
    max_debt: u64,
    max_queue_items: u64,
    admission_rejects: u64,
    work_conserving_ok: bool,
    per_block_first_delay_max: u64, // fairness: en-kötü-blok-ilk-parity-gecikmesi (tick)
}

#[allow(clippy::too_many_lines)]
fn run(tr: &Trace, kind: PolicyKind) -> SimOut {
    let p = PolicyParams::defaults();
    let caps = AdmissionCaps {
        pending_items_cap: 64,
        pending_bytes_cap: 64 * UNIT,
        floor_ticket_cap: 8,
        per_block_admitted_cap: 8,
        parity_size_max: 2_048,
    };
    let mut st = BudgetPolicyState::default();
    let mut rng = Rng(tr.seed);
    let mut out = SimOut {
        work_conserving_ok: true,
        ..SimOut::default()
    };
    // Kuyruk-modeli (çağıran-sahipliği): bulk[(block, kalan_adet, size, birth)], ticket[(block, birth)].
    let mut bulk: Vec<(u32, u16, u64, u64)> = Vec::new();
    let mut tickets: Vec<(u32, u64)> = Vec::new();
    let mut first_service: std::collections::BTreeMap<u32, u64> = std::collections::BTreeMap::new();
    let mut block_birth: std::collections::BTreeMap<u32, u64> = std::collections::BTreeMap::new();
    let mut next_block: u32 = 0;
    let mut kind_now = kind;
    let mut psize = tr.parity_size;
    let mut data_backlog: u64 = 0;
    let mut bucket: u64 = 0; // KALICI-token-bucket (pacer-modeli: refill+cap; spend-düşer)

    for t in 0..tr.ticks {
        let elapsed = if tr.idle_gap_at != 0 && t == tr.idle_gap_at {
            if tr.name == "timer-wrap-boundary" {
                u64::MAX / 2 // saturating-sınır-denemesi (panik-yok-şartı)
            } else {
                5_000_000_000
            }
        } else {
            TICK_NS
        };
        if tr.switch_policy_at != 0 && t == tr.switch_policy_at {
            // Mode-transition-semantiği: state-reset (kernel-sözleşmesi).
            kind_now = PolicyKind::LeftoverOnly;
            st.reset();
        }
        if tr.size_switch_at != 0 && t == tr.size_switch_at {
            psize = 1_061;
        }
        // Blok-gelişi + admission (feasibility-typed; r4: 1-floor-ticket + 3-bulk).
        let arrive = if tr.block_every_ticks == 0 {
            t == 0
        } else {
            t % tr.block_every_ticks == 0
        };
        if arrive {
            let b = next_block;
            next_block += 1;
            block_birth.insert(b, t);
            let items = bulk.iter().map(|x| u64::from(x.1)).sum::<u64>() as u32;
            let bytes = bulk.iter().map(|x| u64::from(x.1) * x.2).sum::<u64>();
            let has_ticket = tickets.iter().any(|&(tb, _)| tb == b);
            let wants_floor = matches!(
                kind_now,
                PolicyKind::FloorCap | PolicyKind::BoundedDebtFloor
            );
            match admit(
                &caps,
                items,
                bytes,
                tickets.len() as u32,
                0,
                psize,
                true,
                has_ticket,
                wants_floor,
            ) {
                AdmissionResult::AdmittedWithFloorTicket => {
                    out.floor_admitted += 1;
                    tickets.push((b, t));
                    bulk.push((b, 3, psize, t));
                }
                AdmissionResult::AdmittedBulkOnly => bulk.push((b, 4, psize, t)),
                _ => out.admission_rejects += 1,
            }
        }
        // RECOVERED-cancel: kuyruk+ticket-temizliği (blok-yarısı).
        if tr.cancel_at_tick != 0 && t == tr.cancel_at_tick {
            let cutoff = next_block / 2;
            bulk.retain(|&(b, ..)| b >= cutoff);
            tickets.retain(|&(b, _)| b >= cutoff);
        }
        // Talepler.
        let data_new = if tr.burst_every != 0 {
            if t % tr.burst_every == 0 {
                tr.data_per_tick + (rng.next() % 600)
            } else {
                0
            }
        } else {
            tr.data_per_tick
        };
        data_backlog = data_backlog.saturating_add(data_new);
        let bulk_bytes: u64 = bulk.iter().map(|x| u64::from(x.1) * x.2).sum();
        let oldest_age = tickets
            .first()
            .map_or(0, |&(_, birth)| (t - birth) * TICK_NS);
        bucket = bucket
            .saturating_add(
                tr.rate
                    .saturating_mul(elapsed)
                    .saturating_div(1_000_000_000),
            )
            .min(4 * 2_048); // burst-cap'li-KALICI-bucket (saturating: timer-wrap-trace'i)
        let avail = bucket;
        let ctx = BudgetContext {
            elapsed_ns: elapsed,
            pacing_rate_bps: tr.rate,
            available_tokens: avail,
            data_demand_bytes: data_backlog,
            arq_demand_bytes: 0,
            bulk_parity_demand_bytes: bulk_bytes,
            floor_tickets: tickets.len() as u32,
            oldest_ticket_age_ns: oldest_age,
            parity_size: psize,
            srtt_ns: tr.srtt_ns,
        };
        let d = step(kind_now, &p, &mut st, &ctx);
        // Invariant'lar (her-tick; DUR-koşulu-denetimleri):
        assert!(d.data_token_limit <= avail, "token-icadı-yok");
        assert!(
            d.parity_token_limit <= avail.saturating_add(d.borrow_allowance_bytes),
            "parity-icadı-yok"
        );
        // Work-conserving: parity-talebi-yokken DATA-tümünü-görmeli.
        if bulk_bytes == 0 && tickets.is_empty() && d.data_token_limit != avail {
            out.work_conserving_ok = false;
        }
        // Displacement = entitlement'tan-GERÇEKTEN-harcanan-parity (DATA'nın-kaybettiği-throughput-payı;
        // tick-başı-duran-rezerv-tekrar-SAYILMAZ — metrik-artefaktı-düzeltmesi). Ek: rezerv-havuz-tepe.
        let data_sent = data_backlog.min(d.data_token_limit);
        out.max_reserved_pool = out.max_reserved_pool.max(d.reserved_bytes);
        data_backlog -= data_sent;
        bucket = bucket.saturating_sub(data_sent);
        out.max_data_backlog = out.max_data_backlog.max(data_backlog);
        // Parity-servis: floor-ticket'lar-önce (F2), sonra-bulk-FIFO.
        let mut budget = d.parity_token_limit;
        let mut parity_spent: u64 = 0;
        let t_fl = t_floor_ns(&p, tr.srtt_ns, psize, tr.rate);
        for _ in 0..d.floor_grants.min(tickets.len() as u32) {
            let (b, birth) = tickets.remove(0);
            budget = budget.saturating_sub(psize);
            bucket = bucket.saturating_sub(psize);
            parity_spent += psize;
            out.floor_serviced += 1;
            if (t - birth) * TICK_NS > t_fl {
                out.floor_missed += 1; // geç-servis = miss (teşhisli)
            }
            first_service.entry(b).or_insert(t - birth);
        }
        while budget >= psize {
            let Some(front) = bulk.first_mut() else { break };
            if front.1 == 0 {
                bulk.remove(0);
                continue;
            }
            front.1 -= 1;
            budget -= front.2;
            bucket = bucket.saturating_sub(front.2);
            parity_spent += front.2;
            out.bulk_emitted_bytes += front.2;
            let (b, birth) = (front.0, front.3);
            first_service.entry(b).or_insert(t - birth);
            out.max_parity_delay_ticks = out.max_parity_delay_ticks.max(t - birth);
            if front.1 == 0 {
                bulk.remove(0);
            }
        }
        out.data_displaced_bytes += parity_spent.min(d.reserved_bytes);
        let _borrowed = settle(kind_now, &p, &mut st, &d, parity_spent);
        if d.cap_suppressed {
            out.cap_suppressed_events += 1;
        }
        out.max_credit = out
            .max_credit
            .max(st.entitlement_credit.max(st.hardcap_credit));
        out.max_debt = out.max_debt.max(st.debt_bytes);
        out.max_queue_items = out
            .max_queue_items
            .max(bulk.iter().map(|x| u64::from(x.1)).sum::<u64>() + tickets.len() as u64);
    }
    // Hiç-servis-edilmemiş-admitted-ticket'lar = miss.
    out.floor_missed += tickets.len() as u64;
    out.per_block_first_delay_max = first_service.values().copied().max().unwrap_or(0);
    out
}

/// CI-SMOKE: contended-trace'lerde çift-koşu-BİT-ÖZDEŞLİK (determinizm DUR-koşulu) + temel-invariant'lar.
#[test]
fn bks_determinism_and_invariants_smoke() {
    let contended = [
        "offered-eq-C",
        "offered-gt-C",
        "many-blocks-gt-floor-capacity",
        "cap-exhausted",
    ];
    for tr in traces().iter().filter(|t| contended.contains(&t.name)) {
        for kind in [
            PolicyKind::LeftoverOnly,
            PolicyKind::ProtectedShare,
            PolicyKind::FloorCap,
            PolicyKind::BoundedDebtFloor,
        ] {
            let a = run(tr, kind);
            let b = run(tr, kind);
            assert_eq!(a, b, "determinizm: {} {:?}", tr.name, kind);
            assert!(
                a.work_conserving_ok,
                "work-conserving: {} {:?}",
                tr.name, kind
            );
            assert!(a.max_debt <= 2 * UNIT, "debt-bound");
            assert!(
                a.max_queue_items <= 64 + 4 + 8,
                "queue-bound (cap+push-4+ticket)"
            );
        }
    }
    // timer-wrap: panik-yok (saturating).
    let wrap = traces()
        .into_iter()
        .find(|t| t.name == "timer-wrap-boundary")
        .unwrap();
    let _ = run(&wrap, PolicyKind::FloorCap);
}

/// TAM-TARAMA (ignored; BKS_OUT'a-CSV): 20-trace × 4-candidate.
#[test]
#[ignore]
fn budget_kernel_sim() {
    let out_dir = std::env::var("BKS_OUT").expect("BKS_OUT");
    std::fs::create_dir_all(&out_dir).unwrap();
    let mut f = std::fs::File::create(format!("{out_dir}/bks-results.csv")).unwrap();
    writeln!(f, "trace,policy,floor_admitted,floor_serviced,floor_missed,bulk_bytes,data_displaced,max_backlog,max_reserved_pool,max_par_delay_ticks,cap_suppressed,max_credit,max_debt,max_queue,admission_rejects,work_conserving,first_delay_max_ticks").unwrap();
    for tr in traces() {
        for kind in [
            PolicyKind::LeftoverOnly,
            PolicyKind::ProtectedShare,
            PolicyKind::FloorCap,
            PolicyKind::BoundedDebtFloor,
        ] {
            let o = run(&tr, kind);
            writeln!(
                f,
                "{},{:?},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}",
                tr.name,
                kind,
                o.floor_admitted,
                o.floor_serviced,
                o.floor_missed,
                o.bulk_emitted_bytes,
                o.data_displaced_bytes,
                o.max_data_backlog,
                o.max_reserved_pool,
                o.max_parity_delay_ticks,
                o.cap_suppressed_events,
                o.max_credit,
                o.max_debt,
                o.max_queue_items,
                o.admission_rejects,
                u8::from(o.work_conserving_ok),
                o.per_block_first_delay_max
            )
            .unwrap();
        }
    }
    eprintln!("[bks] yazıldı: {out_dir}/bks-results.csv");
}
