//! Deterministik scriptable link simülatörü (ADR-17: RNG YOK, scripted).
//!
//! Link, paket başına sabit bir script ile tanımlanır. Aynı script → birebir aynı
//! sonuç (golden referans). loss = [`Fate::Drop`]; jitter/bandwidth/reorder =
//! [`Fate::Deliver`] gecikme değerleriyle ifade edilir (reorder, farklı
//! gecikmelerden emergent olarak çıkar).

use crate::clock::Clock;
use std::time::Duration;

/// Bir paketin simüle link'i geçerkenki kaderi.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fate {
    /// Verilen tek-yön gecikmeden sonra teslim.
    Deliver(Duration),
    /// Düşürüldü (loss).
    Drop,
}

/// Teslim edilen bir paket: gönderim-sıra indeksi + start'tan itibaren gecikme.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Delivery {
    /// 0-tabanlı gönderim sırası.
    pub index: usize,
    /// Start'tan itibaren varış gecikmesi.
    pub delay: Duration,
}

/// Sabit per-paket script ile tanımlı deterministik link. Paket `i`,
/// `script[i]` kaderini alır; script dışındaki paketler `default` alır.
#[derive(Debug, Clone)]
pub struct LinkSim {
    script: Vec<Fate>,
    default: Fate,
}

impl LinkSim {
    /// Bir script ve script-dışı paketler için varsayılan kader ile kurar.
    #[must_use]
    pub fn new(script: Vec<Fate>, default: Fate) -> Self {
        Self { script, default }
    }

    /// `index`'inci paketin kaderi.
    #[must_use]
    pub fn fate(&self, index: usize) -> Fate {
        self.script.get(index).copied().unwrap_or(self.default)
    }

    /// `0..count` indeksli paketleri `clock.now()`'da gönderir, her paketin
    /// gecikmesini `clock` üzerinden bekler ve teslimleri **varış sırasında**
    /// döner (reorder, farklı gecikmelerden emergent). `tokio::time::pause`
    /// altında deterministik ve wall-clock'suz çalışır.
    pub async fn deliver<C: Clock>(&self, clock: &C, count: usize) -> Vec<Delivery> {
        let start = clock.now();

        // Teslim edilecekleri (index, gecikme) olarak topla.
        let mut sched: Vec<(usize, Duration)> = (0..count)
            .filter_map(|i| match self.fate(i) {
                Fate::Deliver(d) => Some((i, d)),
                Fate::Drop => None,
            })
            .collect();

        // Varışa göre sırala; eşit gecikmede index ile deterministik tie-break.
        sched.sort_by(|a, b| a.1.cmp(&b.1).then(a.0.cmp(&b.0)));

        let mut out = Vec::with_capacity(sched.len());
        for (index, delay) in sched {
            clock.sleep_until(start + delay).await;
            out.push(Delivery { index, delay });
        }
        out
    }
}
