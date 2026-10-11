//! K4G-02 (plan D2 v1.1) — `init_counter` kalıcılığı: ENJEKTE EDİLEN [`CounterStore`].
//!
//! noise-wire §0.7 RESTART-POLİTİKASI (PATCH-15):
//! "sender init_counter'ı-cihaz-başına-KALICI+MONOTON-tutar". Depo, [`EntropySource`] ve
//! [`MonotonicClock`] gibi kurulumda verilir (`ControlConfig::counter_store`); `SessionControl`
//! her INIT için [`CounterStore::reserve_next`]'i BİR kez çağırır ve INIT'i dönen değerle
//! mühürler. Sayacı kendisi artırmaz, belleğe almaz; write-ahead garantisi deponundur,
//! `SessionControl`'ün değil.
//!
//! Ürün deposu redb desteklidir (K4G-06). Bu modülde yalnız sözleşme ve test deposu
//! `MemoryCounterStore` (`test-util`) vardır. Sprint-4e modeli `restart::InitCounterStore`
//! (`&mut self`, hata dönemez) bu sözleşmenin yerine GEÇMEZ; ürün yolu onu kullanmaz.
//!
//! [`EntropySource`]: crate::entropy::EntropySource
//! [`MonotonicClock`]: crate::clock::MonotonicClock

/// `init_counter` ayrılamadı. Hangi değişken dönerse dönsün değer AYRILMAMIŞTIR.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum CounterStoreError {
    /// Sayaç uzayı tükendi (`u64`): yeni değer kalıcılaştırılamaz.
    #[error("init_counter space exhausted")]
    Exhausted,
    /// Depo okunamadı, yazılamadı ya da işlem kalıcılaştırılamadı (commit).
    #[error("init_counter could not be persisted")]
    Persist,
}

/// Cihaz başına TEK `init_counter` deposu (eş argümanı YOK: bütün eşlere giden INIT'ler aynı
/// diziden değer alır).
///
/// Sözleşme: Döndürülen değer, çağrı dönmeden **önce** kalıcılaştırılmıştır; aynı depo aynı değeri
/// iki kez döndürmez; depo hata verirse değer **ayrılmamıştır**.
///
/// Ek (RESTART-POLİTİKASI "MONOTON"): döndürülen değerler kesin artandır. İşlem tektir: depo
/// mevcut değeri okur, artırır, kalıcılaştırır (commit), ancak sonra değeri döndürür.
/// `load()`/`store()` çifti YOKTUR (D2).
pub trait CounterStore: Send + Sync {
    /// Sonraki `init_counter`'ı ayırır ve kalıcılaştırır; değer ancak bundan sonra döner.
    ///
    /// # Errors
    /// [`CounterStoreError`] — değer ayrılmamıştır; çağıran INIT GÖNDERMEZ.
    fn reserve_next(&self) -> Result<u64, CounterStoreError>;
}

/// Bellek içi depo — YALNIZ TEST: "kalıcılık" süreç ömrüyle sınırlıdır. Yeniden başlatma, aynı
/// örneği yeni bir `SessionControl`'e vererek benzetilir; yeni bir örnek, kalıcı durumu kaybolmuş
/// bir cihazdır (sayaç baştan).
#[cfg(any(test, feature = "test-util"))]
#[derive(Debug, Default)]
pub struct MemoryCounterStore {
    /// Sonraki ayrılacak değer.
    next: std::sync::Mutex<u64>,
}

#[cfg(any(test, feature = "test-util"))]
impl MemoryCounterStore {
    /// İlk değeri 0 olan depo.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// İlk değeri `next` olan depo.
    #[must_use]
    pub fn starting_at(next: u64) -> Self {
        Self {
            next: std::sync::Mutex::new(next),
        }
    }
}

#[cfg(any(test, feature = "test-util"))]
impl CounterStore for MemoryCounterStore {
    fn reserve_next(&self) -> Result<u64, CounterStoreError> {
        // Zehirlenme yalnız başka bir iş parçacığının kilit altında paniklemesiyle olur; kilit
        // altındaki tek yazma aşağıdaki atamadır, değer tutarsız kalamaz.
        let mut next = self
            .next
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let value = *next;
        // `u64::MAX` dağıtılmaz: ardılı kalıcılaştırılamaz (aynı değer iki kez dönerdi).
        let after = value.checked_add(1).ok_or(CounterStoreError::Exhausted)?;
        *next = after;
        Ok(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_store_reserves_strictly_increasing_values_from_zero() {
        let s = MemoryCounterStore::new();
        let got: Vec<u64> = (0..5).map(|_| s.reserve_next().unwrap()).collect();
        assert_eq!(got, [0, 1, 2, 3, 4]);
    }

    #[test]
    fn exhausted_memory_store_reserves_nothing() {
        let s = MemoryCounterStore::starting_at(u64::MAX - 1);
        assert_eq!(s.reserve_next(), Ok(u64::MAX - 1));
        assert_eq!(s.reserve_next(), Err(CounterStoreError::Exhausted));
        assert_eq!(
            s.reserve_next(),
            Err(CounterStoreError::Exhausted),
            "hata durumu ilerletmez"
        );
    }

    #[test]
    fn memory_store_never_returns_a_value_twice_across_threads() {
        const THREADS: usize = 4;
        const PER_THREAD: usize = 1_000;
        let s = std::sync::Arc::new(MemoryCounterStore::new());
        let handles: Vec<_> = (0..THREADS)
            .map(|_| {
                let s = std::sync::Arc::clone(&s);
                std::thread::spawn(move || {
                    (0..PER_THREAD)
                        .map(|_| s.reserve_next().unwrap())
                        .collect::<Vec<u64>>()
                })
            })
            .collect();
        let mut all: Vec<u64> = handles
            .into_iter()
            .flat_map(|h| h.join().unwrap())
            .collect();
        all.sort_unstable();
        all.dedup();
        assert_eq!(all.len(), THREADS * PER_THREAD, "aynı değer iki kez döndü");
        assert_eq!(s.reserve_next(), Ok((THREADS * PER_THREAD) as u64));
    }
}
