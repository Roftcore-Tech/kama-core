//! Düğüm-tekil tutamaçlar (sahip kararı, CEVAP PAKETİ 2 madde 1; #122 beyanlı istisna 10, #123
//! istisna 3).
//!
//! `kama-app`'te `stream_id` ve `transfer_id` EŞ BAŞINA kalıcı sayaçlardır ve yalnız AÇIK
//! akış/aktarımlar arasında benzersizdir: kapatılmış bir tel kimliği başka bir eşin açık
//! akışına/aktarımına yeniden verilebilir. API'deki çağrıların (`ptt_push`, `ptt_close`,
//! `cancel_file`) eş almaması bu yüzden çıplak tel kimliğiyle use-after-close açığı olurdu. Bu
//! modülün tutamaçları o açığı kapatır:
//!
//! * **düğüm-tekil** — iki tür ([`StreamHandle`], [`FileHandle`]) TEK sayaçtan çıkar;
//! * **kesin artan, yeniden kullanılmaz** — sayaç yalnız artar, `u64` tükenirse
//!   [`NodeError::HandlesExhausted`] (sarma YOK);
//! * **bellek içi** — süreç ömrüyle sınırlıdır (kalıcı değildir; kalıcı olan tel sayaçlarıdır);
//! * düğüm (tutamaç → (eş, tel kimliği)) eşlemesini [`Registry`]'de tutar; kapanan tutamacın
//!   girdisi silinir ve bir daha hiçbir değere çözülmez: kapanmış tutamaçla gelen çağrı
//!   [`NodeError::HandleClosed`] alır, hiç verilmemiş değer [`NodeError::UnknownHandle`].

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::error::NodeError;

/// Giden PTT akışının düğüm tutamacı (`ptt_open` döner; `ptt_push` / `ptt_close` alır). Tel
/// `stream_id`'si DEĞİLDİR.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct StreamHandle(u64);

/// Giden dosya aktarımının düğüm tutamacı (`send_file` döner; `cancel_file` alır). Tel
/// `transfer_id`'si DEĞİLDİR.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FileHandle(u64);

impl StreamHandle {
    /// Ham değerden tutamaç (FFI gidiş-dönüşü için; K4G-15). Düğümün vermediği değer
    /// [`NodeError::UnknownHandle`] alır.
    #[must_use]
    pub const fn from_raw(raw: u64) -> Self {
        Self(raw)
    }

    /// Ham değer.
    #[must_use]
    pub const fn raw(self) -> u64 {
        self.0
    }
}

impl FileHandle {
    /// Ham değerden tutamaç (FFI gidiş-dönüşü için; K4G-15). Düğümün vermediği değer
    /// [`NodeError::UnknownHandle`] alır.
    #[must_use]
    pub const fn from_raw(raw: u64) -> Self {
        Self(raw)
    }

    /// Ham değer.
    #[must_use]
    pub const fn raw(self) -> u64 {
        self.0
    }
}

/// Düğümün TEK tutamaç sayacı. `0` hiç verilmez (FFI'da "tutamaç yok" değeri olarak serbest kalır);
/// ilk tutamaç [`HandleAllocator::FIRST`].
#[derive(Debug)]
pub(crate) struct HandleAllocator {
    /// Sıradaki verilecek değer.
    next: AtomicU64,
}

impl HandleAllocator {
    /// İlk verilen tutamaç.
    pub(crate) const FIRST: u64 = 1;

    pub(crate) fn new() -> Self {
        Self {
            next: AtomicU64::new(Self::FIRST),
        }
    }

    /// Yeni, daha önce hiç verilmemiş bir değer. İki iş parçacığından eşzamanlı çağrı aynı değeri
    /// alamaz (tek atomik güncelleme).
    ///
    /// # Errors
    /// [`NodeError::HandlesExhausted`] — `u64::MAX` verilmez (ardılı yazılamaz; sarma YOK).
    pub(crate) fn issue(&self) -> Result<u64, NodeError> {
        self.next
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |next| {
                next.checked_add(1)
            })
            .map_err(|_| NodeError::HandlesExhausted)
    }

    /// Bulunamayan tutamacın hatası: bu düğüm onu verdiyse kapanmıştır, vermediyse bilinmez.
    pub(crate) fn missing(&self, raw: u64) -> NodeError {
        if (Self::FIRST..self.next.load(Ordering::Acquire)).contains(&raw) {
            NodeError::HandleClosed
        } else {
            NodeError::UnknownHandle
        }
    }

    /// Yalnız test: sayacı tavana yaklaştırır.
    #[cfg(test)]
    fn starting_at(next: u64) -> Self {
        Self {
            next: AtomicU64::new(next),
        }
    }
}

/// Açık tutamaç → değer (eş, tel kimliği) eşlemesi. Boyu motorun tablosuyla sınırlıdır: girdi
/// yalnız motor akışı/aktarımı kabul edince eklenir ve akış/aktarım kapanınca silinir.
#[derive(Debug)]
pub(crate) struct Registry<V> {
    open: BTreeMap<u64, V>,
}

impl<V> Registry<V> {
    pub(crate) fn new() -> Self {
        Self {
            open: BTreeMap::new(),
        }
    }

    pub(crate) fn insert(&mut self, raw: u64, value: V) {
        self.open.insert(raw, value);
    }

    /// Açık tutamacın değeri.
    ///
    /// # Errors
    /// [`NodeError::HandleClosed`] / [`NodeError::UnknownHandle`] ([`HandleAllocator::missing`]).
    pub(crate) fn get(&self, raw: u64, handles: &HandleAllocator) -> Result<&V, NodeError> {
        self.open.get(&raw).ok_or_else(|| handles.missing(raw))
    }

    /// Tutamacı kapatır: değer döner, girdi silinir (tutamaç bir daha çözülmez).
    ///
    /// # Errors
    /// [`Registry::get`] ile aynı.
    pub(crate) fn remove(&mut self, raw: u64, handles: &HandleAllocator) -> Result<V, NodeError> {
        self.open.remove(&raw).ok_or_else(|| handles.missing(raw))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handles_are_strictly_increasing_and_never_reused() {
        let handles = HandleAllocator::new();
        let issued: Vec<u64> = (0..64).map(|_| handles.issue().expect("tutamaç")).collect();
        assert_eq!(issued.first(), Some(&HandleAllocator::FIRST));
        assert!(
            issued.windows(2).all(|w| w[0] < w[1]),
            "kesin artan: {issued:?}"
        );
    }

    #[test]
    fn concurrent_issue_never_returns_a_value_twice() {
        const THREADS: usize = 4;
        const PER_THREAD: usize = 1_000;
        let handles = std::sync::Arc::new(HandleAllocator::new());
        let workers: Vec<_> = (0..THREADS)
            .map(|_| {
                let handles = std::sync::Arc::clone(&handles);
                std::thread::spawn(move || {
                    (0..PER_THREAD)
                        .map(|_| handles.issue().expect("tutamaç"))
                        .collect::<Vec<u64>>()
                })
            })
            .collect();
        let mut all: Vec<u64> = workers
            .into_iter()
            .flat_map(|w| w.join().expect("iş parçacığı"))
            .collect();
        all.sort_unstable();
        all.dedup();
        assert_eq!(
            all.len(),
            THREADS * PER_THREAD,
            "aynı değer iki kez verildi"
        );
    }

    #[test]
    fn exhausted_space_issues_nothing_and_never_wraps() {
        let handles = HandleAllocator::starting_at(u64::MAX - 1);
        assert_eq!(handles.issue().expect("son değer"), u64::MAX - 1);
        assert!(matches!(handles.issue(), Err(NodeError::HandlesExhausted)));
        assert!(
            matches!(handles.issue(), Err(NodeError::HandlesExhausted)),
            "hata durumu ilerletmez"
        );
    }

    /// Kapanmış tutamaç tipli hata alır ve — tel kimliği başka eşin açık akışına yeniden verilse
    /// bile — o akışa ÇÖZÜLMEZ (use-after-close kapalı).
    #[test]
    fn closed_handle_is_typed_and_never_resolves_to_another_peers_stream() {
        const ALICE: u8 = 1;
        const BOB: u8 = 2;
        const SAME_WIRE_ID: u32 = 0;
        let handles = HandleAllocator::new();
        let mut streams: Registry<(u8, u32)> = Registry::new();

        let first = handles.issue().expect("tutamaç");
        streams.insert(first, (ALICE, SAME_WIRE_ID));
        assert_eq!(
            streams.remove(first, &handles).expect("açık"),
            (ALICE, SAME_WIRE_ID)
        );

        // Bob'un açık akışı AYNI tel kimliğini aldı (eş başına sayaç) — yeni tutamaçla.
        let second = handles.issue().expect("tutamaç");
        streams.insert(second, (BOB, SAME_WIRE_ID));
        assert!(second > first, "tutamaç yeniden kullanılmaz");

        assert!(matches!(
            streams.get(first, &handles),
            Err(NodeError::HandleClosed)
        ));
        assert!(matches!(
            streams.remove(first, &handles),
            Err(NodeError::HandleClosed)
        ));
        assert_eq!(
            streams.get(second, &handles).expect("açık"),
            &(BOB, SAME_WIRE_ID)
        );
    }

    #[test]
    fn never_issued_values_are_unknown_not_closed() {
        let handles = HandleAllocator::new();
        let registry: Registry<()> = Registry::new();
        assert!(matches!(
            registry.get(0, &handles),
            Err(NodeError::UnknownHandle)
        ));
        assert!(matches!(
            registry.get(HandleAllocator::FIRST, &handles),
            Err(NodeError::UnknownHandle)
        ));
        let issued = handles.issue().expect("tutamaç");
        assert!(matches!(
            registry.get(issued, &handles),
            Err(NodeError::HandleClosed)
        ));
        assert!(matches!(
            registry.get(issued + 1, &handles),
            Err(NodeError::UnknownHandle)
        ));
    }

    #[test]
    fn handle_types_round_trip_their_raw_value() {
        assert_eq!(StreamHandle::from_raw(7).raw(), 7);
        assert_eq!(FileHandle::from_raw(9).raw(), 9);
    }
}
