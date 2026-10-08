//! [L-T1] Bellek içi zarf taşıyıcısı — YALNIZ test (`test-util`).
//!
//! [`crate::transport::EnvelopeLink`]'in hub benzeri bir test uygulaması: düğümler kimlikleriyle
//! (`PeerRef`) kayıt olur; INIT ve RESPONSE gönderimleri hub'da BEKLER ve yalnız [`MemoryHub::relay`]
//! çağrılınca taşınır. Böylece bekleyen bir niyet, test onu açıkça sürmedikçe çözülmez (S1 sözleşme
//! süitinin `pump` kancasının istediği şey).
//!
//! Rölede INIT başına bir hub tutamacı atanır: başlatana `HandleAssigned`, alıcıya aynı tutamaçla
//! `IncomingInit` gider (direct-mode: iki yön aynı tutamacı kullanır). RESPONSE gönderen tarafın
//! verdiği tutamaçla başlatana döner. Hub RÖLESİ, hop-ARQ ve HubChannel bu taşıyıcıda YOKTUR.

use std::collections::{BTreeMap, VecDeque};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use kama_contracts::handles::PeerRef;
use kama_contracts::s3::HubHandle;

use crate::transport::{EnvelopeLink, LinkAttempt, LinkError, LinkEvent};

/// Rölede bekleyen gönderim.
enum Outbound {
    Init {
        from: PeerRef,
        to: PeerRef,
        attempt: LinkAttempt,
        envelope_type: u8,
        bytes: Vec<u8>,
    },
    Response {
        to: PeerRef,
        hub_handle: HubHandle,
        envelope_type: u8,
        bytes: Vec<u8>,
    },
}

impl Outbound {
    fn to(&self) -> PeerRef {
        match self {
            Self::Init { to, .. } | Self::Response { to, .. } => *to,
        }
    }
}

#[derive(Default)]
struct HubState {
    next_attempt: u64,
    next_handle: u32,
    outbound: VecDeque<Outbound>,
    inboxes: BTreeMap<PeerRef, VecDeque<LinkEvent>>,
}

fn lock(m: &Mutex<HubState>) -> MutexGuard<'_, HubState> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Bellek içi hub (bkz. modül belgesi). Kopyaları aynı hub'ı paylaşır.
#[derive(Clone, Default)]
pub struct MemoryHub {
    inner: Arc<Mutex<HubState>>,
}

impl MemoryHub {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// `me` kimliğiyle bir düğüm taşıyıcısı açar (gelen kutusu kaydedilir).
    #[must_use]
    pub fn link(&self, me: PeerRef) -> MemoryLink {
        lock(&self.inner).inboxes.entry(me).or_default();
        MemoryLink {
            me,
            hub: Arc::clone(&self.inner),
        }
    }

    /// `peer` kayıtlı mı.
    #[must_use]
    pub fn is_registered(&self, peer: PeerRef) -> bool {
        lock(&self.inner).inboxes.contains_key(&peer)
    }

    /// Bekleyen gönderimlerin varış noktaları arasında KAYITSIZ olanlar (fikstür o eşi yaratır).
    #[must_use]
    pub fn unregistered_destinations(&self) -> Vec<PeerRef> {
        let st = lock(&self.inner);
        let mut out: Vec<PeerRef> = st
            .outbound
            .iter()
            .map(Outbound::to)
            .filter(|to| !st.inboxes.contains_key(to))
            .collect();
        out.sort();
        out.dedup();
        out
    }

    /// Bekleyen gönderimleri taşır; kayıtsız varış noktasına gidenler kuyrukta KALIR. Taşınan
    /// gönderim sayısını döner.
    pub fn relay(&self) -> usize {
        let mut st = lock(&self.inner);
        let pending = std::mem::take(&mut st.outbound);
        let mut moved = 0;
        for o in pending {
            if !st.inboxes.contains_key(&o.to()) {
                st.outbound.push_back(o);
                continue;
            }
            moved += 1;
            match o {
                Outbound::Init {
                    from,
                    to,
                    attempt,
                    envelope_type,
                    bytes,
                } => {
                    st.next_handle = st.next_handle.wrapping_add(1).max(1);
                    let hub_handle = HubHandle::from_raw(st.next_handle);
                    if let Some(q) = st.inboxes.get_mut(&from) {
                        q.push_back(LinkEvent::HandleAssigned {
                            attempt,
                            hub_handle,
                        });
                    }
                    if let Some(q) = st.inboxes.get_mut(&to) {
                        q.push_back(LinkEvent::IncomingInit {
                            hub_handle,
                            envelope_type,
                            bytes,
                        });
                    }
                }
                Outbound::Response {
                    to,
                    hub_handle,
                    envelope_type,
                    bytes,
                } => {
                    if let Some(q) = st.inboxes.get_mut(&to) {
                        q.push_back(LinkEvent::Response {
                            hub_handle,
                            envelope_type,
                            bytes,
                        });
                    }
                }
            }
        }
        moved
    }

    /// `to`'nun gelen kutusuna doğrudan bir olay koyar (ör. sahte RESPONSE) — röle beklemez.
    pub fn inject(&self, to: PeerRef, ev: LinkEvent) {
        lock(&self.inner)
            .inboxes
            .entry(to)
            .or_default()
            .push_back(ev);
    }

    /// Rölede bekleyen gönderim yok ve hiçbir gelen kutusunda okunmamış olay yok.
    #[must_use]
    pub fn is_idle(&self) -> bool {
        let st = lock(&self.inner);
        st.outbound.is_empty() && st.inboxes.values().all(VecDeque::is_empty)
    }
}

/// Bir düğümün [`MemoryHub`] taşıyıcısı.
pub struct MemoryLink {
    me: PeerRef,
    hub: Arc<Mutex<HubState>>,
}

impl EnvelopeLink for MemoryLink {
    fn send_init(
        &mut self,
        peer: PeerRef,
        envelope_type: u8,
        bytes: Vec<u8>,
    ) -> Result<LinkAttempt, LinkError> {
        let mut st = lock(&self.hub);
        st.next_attempt = st.next_attempt.wrapping_add(1);
        let attempt = LinkAttempt(st.next_attempt);
        st.outbound.push_back(Outbound::Init {
            from: self.me,
            to: peer,
            attempt,
            envelope_type,
            bytes,
        });
        Ok(attempt)
    }

    fn send_response(
        &mut self,
        peer: PeerRef,
        hub_handle: HubHandle,
        envelope_type: u8,
        bytes: Vec<u8>,
    ) -> Result<(), LinkError> {
        lock(&self.hub).outbound.push_back(Outbound::Response {
            to: peer,
            hub_handle,
            envelope_type,
            bytes,
        });
        Ok(())
    }

    fn poll(&mut self) -> Option<LinkEvent> {
        lock(&self.hub)
            .inboxes
            .get_mut(&self.me)
            .and_then(VecDeque::pop_front)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn peer(n: u8) -> PeerRef {
        PeerRef::from_dest_hash([n; 32])
    }

    #[test]
    fn nothing_moves_without_relay_and_unregistered_destinations_wait() {
        let hub = MemoryHub::new();
        let mut a = hub.link(peer(1));
        let attempt = a.send_init(peer(2), 1, b"ssi1".to_vec()).expect("init");
        assert!(a.poll().is_none(), "röle olmadan hiçbir şey taşınmaz");
        assert_eq!(hub.unregistered_destinations(), [peer(2)]);
        assert_eq!(hub.relay(), 0, "kayıtsız varış kuyrukta kalır");

        let mut b = hub.link(peer(2));
        assert_eq!(hub.relay(), 1);
        let Some(LinkEvent::HandleAssigned {
            attempt: got,
            hub_handle,
        }) = a.poll()
        else {
            panic!("başlatana tutamaç atanmalı");
        };
        assert_eq!(got, attempt);
        assert_eq!(
            b.poll(),
            Some(LinkEvent::IncomingInit {
                hub_handle,
                envelope_type: 1,
                bytes: b"ssi1".to_vec(),
            })
        );
        b.send_response(peer(1), hub_handle, 2, b"ssr1".to_vec())
            .expect("response");
        assert!(a.poll().is_none());
        hub.relay();
        assert_eq!(
            a.poll(),
            Some(LinkEvent::Response {
                hub_handle,
                envelope_type: 2,
                bytes: b"ssr1".to_vec(),
            })
        );
        assert!(hub.is_idle());
    }
}
