//! Zarf rölesi ve veri iletimi (S4 §2.4(c)–(e), §2.6, H4–H7, H11; KM-HUB-TDD-001 §3 `on_store` /
//! `on_data`).
//!
//! Gönderen her zaman ADRESİNDEN tanınır (`by_addr`); kayıtsız adresten gelen STORE ve veri yüke
//! bakılmadan düşer, hiçbiri adres değiştirmez (H4). Hub zarfı açmaz: zarf yalnız tahsis anahtarındaki
//! özet için hash'lenir ve DELIVER'a aynen kopyalanır. Veri datagramı girdi dilimi OLDUĞU GİBİ
//! iletilir (bayt-özdeş; H7).

use core::time::Duration;
use std::net::SocketAddr;

use blake2::{Blake2s256, Digest};
use kama_noise_wire::entropy::EntropySource;

use crate::stats::{self, ShortId};
use crate::table::KeyState;
use crate::wire::{
    AllocationKey, Deliver, DestHash, RouteMiss, RpcFrame, StoreMessageReq, StoreMessageResp,
    ENVELOPE_DIGEST_LEN, ERROR_BAD_REQUEST, ERROR_HUB_CAPACITY, ERROR_RATE_LIMITED, METHOD_DELIVER,
    METHOD_ROUTE_MISS, ROUTE_MISS_UNKNOWN_HANDLE, STATUS_OK, STATUS_PEER_UNBOUND,
    STATUS_UNKNOWN_HANDLE,
};
use crate::{Clock, Hub, Sink};

/// `BLAKE2s(envelope)[0..8]` — tahsis anahtarının zarf özeti.
#[must_use]
pub fn envelope_digest(envelope: &[u8]) -> [u8; ENVELOPE_DIGEST_LEN] {
    let digest = Blake2s256::digest(envelope);
    let mut head = [0u8; ENVELOPE_DIGEST_LEN];
    for (slot, byte) in head.iter_mut().zip(digest.iter()) {
        *slot = *byte;
    }
    head
}

impl<C: Clock, R: EntropySource> Hub<C, R> {
    /// STORE_MESSAGE (H5 yeni hat, H6 mevcut hat).
    pub(crate) fn on_store(
        &mut self,
        src: SocketAddr,
        request: &RpcFrame<'_>,
        now: Duration,
        sink: &mut dyn Sink,
    ) {
        let Some(me) = self.table.key_at(src) else {
            stats::bump(&mut self.stats.src_unbound);
            return;
        };
        let rate = self.limits.store_rate;
        let allowed = self.table.endpoint_mut(&me).is_some_and(|endpoint| {
            endpoint.last_activity = now;
            endpoint.store_bucket.take(rate, now)
        });
        if !allowed {
            stats::bump(&mut self.stats.rate_store);
            self.reply_error(sink, src, request, ERROR_RATE_LIMITED);
            return;
        }
        let Ok(store) = StoreMessageReq::decode(request.payload) else {
            self.bad_request(sink, src, request);
            return;
        };
        // `validate`: S4 §2.4(c) (handle, tip) tablosu + RELAY_OPEN zarfı ≤ 64 B.
        let malformed = store.validate().is_err()
            || self
                .directory
                .by_dest_hash(&store.recipient_dest_hash)
                .is_none()
            || store.recipient_dest_hash == me;
        if malformed {
            self.bad_request(sink, src, request);
            return;
        }
        if store.hub_handle == 0 {
            self.store_new_link(src, me, request, store, now, sink);
        } else {
            self.store_existing_link(src, me, request, store, now, sink);
        }
    }

    /// H5 — S4 §2.6 planlayıcısının sekiz adımı, bu sırayla.
    fn store_new_link(
        &mut self,
        src: SocketAddr,
        me: DestHash,
        request: &RpcFrame<'_>,
        store: StoreMessageReq,
        now: Duration,
        sink: &mut dyn Sink,
    ) {
        let key = AllocationKey {
            sender_dest_hash: me,
            request_id: request.request_id,
            recipient_dest_hash: store.recipient_dest_hash,
            envelope_type: store.envelope_type,
            envelope_hash8: envelope_digest(&store.envelope),
        };
        let recipient_addr = self.table.bound_addr(&store.recipient_dest_hash);
        // 1. Mevcut / idempotent tahsis.
        match self.table.key_state(&key) {
            KeyState::Active(handle) => {
                stats::bump(&mut self.stats.store_repeat);
                match recipient_addr {
                    Some(to) => {
                        self.reply_store(sink, src, request, STATUS_OK, handle);
                        self.deliver(sink, to, handle, me, store);
                    }
                    None => self.peer_unbound(sink, src, request),
                }
                return;
            }
            KeyState::Retired => {
                stats::bump(&mut self.stats.store_key_retired);
                self.reply_store(sink, src, request, STATUS_UNKNOWN_HANDLE, 0);
                return;
            }
            KeyState::Absent => {}
        }
        let Some(to) = recipient_addr else {
            self.peer_unbound(sink, src, request);
            return;
        };
        // 2. Tahsis bütçesi (başarısız tahsiste de harcanır — garanti dışı).
        let rate = self.limits.link_alloc_rate;
        let allowed = self
            .table
            .endpoint_mut(&me)
            .is_some_and(|endpoint| endpoint.alloc_bucket.take(rate, now));
        if !allowed {
            stats::bump(&mut self.stats.rate_alloc);
            self.reply_error(sink, src, request, ERROR_RATE_LIMITED);
            return;
        }
        // 3–7. Plan ve denetimler; aday (yan etkisi yalnız alloc_counter).
        let Some(plan) = self.table.plan_new_link(&me, &store.recipient_dest_hash) else {
            stats::bump(&mut self.stats.hub_capacity);
            self.reply_error(sink, src, request, ERROR_HUB_CAPACITY);
            return;
        };
        let Some(handle) = self.table.pick_handle() else {
            stats::bump(&mut self.stats.hub_capacity);
            self.reply_error(sink, src, request, ERROR_HUB_CAPACITY);
            return;
        };
        // 8. Ancak şimdi: emekli + yaz.
        self.table
            .commit_new_link(&plan, handle, me, store.recipient_dest_hash, key, now);
        for retired in &plan {
            stats::log_link_retire(sink, *retired);
        }
        stats::bump(&mut self.stats.store_ok);
        stats::log_link_open(
            sink,
            handle,
            ShortId::of(&me),
            ShortId::of(&store.recipient_dest_hash),
            store.envelope_type,
        );
        self.reply_store(sink, src, request, STATUS_OK, handle);
        self.deliver(sink, to, handle, me, store);
    }

    /// H6 — mevcut hatta zarf (RESPONSE).
    fn store_existing_link(
        &mut self,
        src: SocketAddr,
        me: DestHash,
        request: &RpcFrame<'_>,
        store: StoreMessageReq,
        now: Duration,
        sink: &mut dyn Sink,
    ) {
        let handle = store.hub_handle;
        let Some(other) = self.table.link(handle).map(|link| link.other(&me)) else {
            self.reply_store(sink, src, request, STATUS_UNKNOWN_HANDLE, 0);
            return;
        };
        if other != Some(store.recipient_dest_hash) {
            // Gönderen hattın ucu değil ya da alıcı karşı uç değil.
            self.bad_request(sink, src, request);
            return;
        }
        let Some(to) = self.table.bound_addr(&store.recipient_dest_hash) else {
            self.peer_unbound(sink, src, request);
            return;
        };
        if let Some(link) = self.table.link_mut(handle) {
            link.last_activity = now;
        }
        stats::bump(&mut self.stats.store_ok);
        self.reply_store(sink, src, request, STATUS_OK, handle);
        self.deliver(sink, to, handle, me, store);
    }

    /// Veri `[h][yük]` (H4, H7).
    pub(crate) fn on_data(
        &mut self,
        src: SocketAddr,
        handle: u32,
        datagram: &[u8],
        now: Duration,
        sink: &mut dyn Sink,
    ) {
        let sender = self.table.key_at(src);
        let Some(link) = self.table.link(handle) else {
            // Bilinmeyen / emekli handle: yalnız BAĞLI kaynağa, hız sınırlı (KR-7).
            let rate = self.limits.route_miss_rate;
            let allowed = sender.is_some_and(|me| {
                self.table
                    .endpoint_mut(&me)
                    .is_some_and(|endpoint| endpoint.route_miss_bucket.take(rate, now))
            });
            if allowed {
                stats::bump(&mut self.stats.route_miss);
                let miss = RouteMiss {
                    hub_handle: handle,
                    reason: ROUTE_MISS_UNKNOWN_HANDLE,
                };
                self.send_hub_rpc(sink, src, METHOD_ROUTE_MISS, &miss.encode(), false);
            } else {
                stats::bump(&mut self.stats.drop_unknown_link);
            }
            return;
        };
        let Some(me) = sender else {
            stats::bump(&mut self.stats.src_unbound);
            return;
        };
        let Some(other) = link.other(&me) else {
            stats::bump(&mut self.stats.src_mismatch);
            return;
        };
        let Some(to) = self.table.bound_addr(&other) else {
            stats::bump(&mut self.stats.drop_peer_unbound);
            return;
        };
        if let Some(link) = self.table.link_mut(handle) {
            link.last_activity = now;
        }
        if let Some(endpoint) = self.table.endpoint_mut(&me) {
            endpoint.last_activity = now;
        }
        sink.send(to, datagram);
        stats::bump(&mut self.stats.tx_forward);
        stats::bump(&mut self.stats.relay_packets);
        // Datagram ≤ MAX_DATAGRAM; usize → u64 her hedefte sığar.
        stats::add(
            &mut self.stats.relay_bytes,
            u64::try_from(datagram.len()).unwrap_or(0),
        );
    }

    fn deliver(
        &mut self,
        sink: &mut dyn Sink,
        to: SocketAddr,
        handle: u32,
        sender: DestHash,
        store: StoreMessageReq,
    ) {
        let deliver = Deliver {
            hub_handle: handle,
            sender_dest_hash: sender,
            envelope_type: store.envelope_type,
            envelope: store.envelope,
        };
        // Zarf STORE isteğinden geldi (≤ 1208 B): kodlama hatası erişilemez; olursa yollanmaz.
        if let Ok(payload) = deliver.encode() {
            self.send_hub_rpc(sink, to, METHOD_DELIVER, &payload, true);
        }
    }

    fn reply_store(
        &mut self,
        sink: &mut dyn Sink,
        src: SocketAddr,
        request: &RpcFrame<'_>,
        status: u16,
        handle: u32,
    ) {
        let response = StoreMessageResp {
            status,
            assigned_handle: handle,
        };
        self.reply_rpc(sink, src, request, &response.encode());
    }

    fn peer_unbound(&mut self, sink: &mut dyn Sink, src: SocketAddr, request: &RpcFrame<'_>) {
        stats::bump(&mut self.stats.peer_unbound);
        self.reply_store(sink, src, request, STATUS_PEER_UNBOUND, 0);
    }

    fn bad_request(&mut self, sink: &mut dyn Sink, src: SocketAddr, request: &RpcFrame<'_>) {
        stats::bump(&mut self.stats.bad_request);
        self.reply_error(sink, src, request, ERROR_BAD_REQUEST);
    }

    /// Hub'ın başlattığı mesaj (`flags = 0`, `request_id = 0`): DELIVER (iletim) ya da ROUTE_MISS (cevap).
    fn send_hub_rpc(
        &mut self,
        sink: &mut dyn Sink,
        to: SocketAddr,
        method_id: u8,
        payload: &[u8],
        forward: bool,
    ) {
        let frame = RpcFrame {
            method_id,
            flags: 0,
            request_id: 0,
            payload,
        };
        let mut tx = core::mem::take(&mut self.tx);
        if frame.encode(&mut tx).is_ok() {
            sink.send(to, &tx);
            if forward {
                stats::bump(&mut self.stats.tx_forward);
            } else {
                stats::bump(&mut self.stats.tx_reply);
            }
        }
        self.tx = tx;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn digest_is_the_first_eight_bytes_of_blake2s() {
        let full = Blake2s256::digest(b"zarf");
        assert_eq!(envelope_digest(b"zarf").as_slice(), &full[..8]);
        assert_ne!(envelope_digest(b"zarf"), envelope_digest(b"zarF"));
    }

    /// S4 §2.4(c) tablosu artık ortak sözleşmede; hub'ın dayandığı üç birleşim tam taramayla kilitli.
    #[test]
    fn only_the_three_documented_combinations_are_valid() {
        for handle in [0u32, 7] {
            for envelope_type in 0..=u8::MAX {
                assert_eq!(
                    crate::wire::valid_combo(handle, envelope_type),
                    matches!((handle, envelope_type), (0, 1 | 5) | (7, 2)),
                    "handle {handle} tip {envelope_type}"
                );
            }
        }
    }
}
