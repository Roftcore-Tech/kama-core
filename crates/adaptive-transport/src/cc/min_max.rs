//! Kathleen Nichols'ın pencereli max filtresi — **quinn-proto 0.11.18
//! `src/congestion/bbr/min_max.rs` portu** (CC-PORT, 2026-11-10).
//!
//! Kaynak lisansı: quinn-proto Apache-2.0 / MIT; özgün algoritma Google, BSD
//! (<https://groups.google.com/forum/#!topic/bbr-dev/3RTgkzi5ZD8>). Davranış birebir;
//! yalnız görünürlük (`pub(crate)`) ve belge dili değişti.
//!
//! Pencere **tur sayısıdır**, zaman damgası değil. Sabit alan, sabit zaman; en iyi /
//! ikinci / üçüncü değeri tutar ve ölçüm zamanlarının sıralı kalmasını sağlar.

/// Tur tabanlı pencereli max filtresi (quinn `MinMax`).
#[derive(Copy, Clone, Debug)]
pub(crate) struct MinMax {
    /// Pencere: tur sayısı (quinn varsayılanı 10).
    window: u64,
    samples: [MinMaxSample; 3],
}

#[derive(Debug, Copy, Clone, Default)]
struct MinMaxSample {
    /// Tur numarası (zaman damgası değil).
    time: u64,
    value: u64,
}

impl MinMax {
    /// Güncel max.
    pub(crate) fn get(&self) -> u64 {
        self.samples[0].value
    }

    fn fill(&mut self, sample: MinMaxSample) {
        self.samples.fill(sample);
    }

    /// Filtreyi sıfırlar.
    pub(crate) fn reset(&mut self) {
        self.fill(Default::default());
    }

    /// `measurement`'ı `current_round` turunda filtreye verir (quinn `update_max`).
    pub(crate) fn update_max(&mut self, current_round: u64, measurement: u64) {
        let sample = MinMaxSample {
            time: current_round,
            value: measurement,
        };

        if self.samples[0].value == 0 /* başlatılmamış */
            || /* yeni max */ sample.value >= self.samples[0].value
            || /* pencerede hiç örnek kalmadı */ sample.time - self.samples[2].time > self.window
        {
            self.fill(sample);
            return;
        }

        if sample.value >= self.samples[1].value {
            self.samples[2] = sample;
            self.samples[1] = sample;
        } else if sample.value >= self.samples[2].value {
            self.samples[2] = sample;
        }

        self.subwin_update(sample);
    }

    fn subwin_update(&mut self, sample: MinMaxSample) {
        let dt = sample.time - self.samples[0].time;
        if dt > self.window {
            self.samples[0] = self.samples[1];
            self.samples[1] = self.samples[2];
            self.samples[2] = sample;
            if sample.time - self.samples[0].time > self.window {
                self.samples[0] = self.samples[1];
                self.samples[1] = self.samples[2];
                self.samples[2] = sample;
            }
        } else if self.samples[1].time == self.samples[0].time && dt > self.window / 4 {
            self.samples[2] = sample;
            self.samples[1] = sample;
        } else if self.samples[2].time == self.samples[1].time && dt > self.window / 2 {
            self.samples[2] = sample;
        }
    }
}

impl Default for MinMax {
    fn default() -> Self {
        Self {
            window: 10,
            samples: [Default::default(); 3],
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    /// quinn'in kendi testi (min_max.rs `test`) — birebir.
    #[test]
    fn quinn_reference_sequence() {
        let round = 25;
        let mut min_max = MinMax::default();
        min_max.update_max(round + 1, 100);
        assert_eq!(100, min_max.get());
        min_max.update_max(round + 3, 120);
        assert_eq!(120, min_max.get());
        min_max.update_max(round + 5, 160);
        assert_eq!(160, min_max.get());
        min_max.update_max(round + 7, 100);
        assert_eq!(160, min_max.get());
        min_max.update_max(round + 10, 100);
        assert_eq!(160, min_max.get());
        min_max.update_max(round + 14, 100);
        assert_eq!(160, min_max.get());
        min_max.update_max(round + 16, 100);
        assert_eq!(100, min_max.get());
        min_max.update_max(round + 18, 130);
        assert_eq!(130, min_max.get());
    }
}
