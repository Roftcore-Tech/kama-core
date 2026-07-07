//! Deterministik zaman soyutlaması (ADR-17).
//!
//! Engine zamanı bu trait üzerinden okur; böylece testler wall-clock yerine
//! `tokio::time::pause`/`advance` ile zamanı deterministik sürebilir.

use std::future::Future;
use tokio::time::Instant;

/// Engine'in zamanı okuduğu monoton saat.
pub trait Clock: Clone + Send + Sync + 'static {
    /// Şu anki monoton an.
    fn now(&self) -> Instant;

    /// Monoton saat `deadline`'a ulaşınca tamamlanır.
    //
    // `async fn` in trait yerine RPITIT + `+ Send` — `async_fn_in_trait` lint'i
    // `clippy -D warnings` altında fail ederdi (CLAUDE.md §7 CI kapısı).
    fn sleep_until(&self, deadline: Instant) -> impl Future<Output = ()> + Send;
}

/// `tokio::time` ile desteklenen production saat (`tokio::time::pause`'a uyar).
#[derive(Debug, Clone, Copy, Default)]
pub struct TokioClock;

impl Clock for TokioClock {
    fn now(&self) -> Instant {
        Instant::now()
    }

    fn sleep_until(&self, deadline: Instant) -> impl Future<Output = ()> + Send {
        tokio::time::sleep_until(deadline)
    }
}
