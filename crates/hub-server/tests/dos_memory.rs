//! T-B10 / T-R1'in "bellek sabit" yarısı — ÖLÇÜLEREK (KM-HUB-TDD-001 §7; S4 H3 "1. tur hiçbir şey
//! yazmaz").
//!
//! Süreç-geneli sayan ayırıcı (emsal: `adaptive-transport/tests/soak_long_session.rs`) hub'ın sel
//! altında yaptığı HER bellek ayırmayı sayar. Beklenen: ısınmadan sonra selin tamamında sıfır ayırma.
//! Sayaç süreç-geneli olduğu için dosyada TEK test vardır.

#[allow(dead_code)]
#[path = "tdd/harness.rs"]
mod harness;

use std::alloc::{GlobalAlloc, Layout, System};
use std::fmt;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

use harness::{data, test_config, FakeClock, Phone, TestRng, World};
use hub_server::wire::{envelope, ClientBindRequest, Payload};
use hub_server::{Hub, Sink};

static LIVE_BYTES: AtomicUsize = AtomicUsize::new(0);
static ALLOCATIONS: AtomicU64 = AtomicU64::new(0);

struct Counting;

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let pointer = unsafe { System.alloc(layout) };
        if !pointer.is_null() {
            LIVE_BYTES.fetch_add(layout.size(), Ordering::Relaxed);
            ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        }
        pointer
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        unsafe { System.dealloc(pointer, layout) };
        LIVE_BYTES.fetch_sub(layout.size(), Ordering::Relaxed);
    }

    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let moved = unsafe { System.realloc(pointer, layout, new_size) };
        if !moved.is_null() {
            ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
            if new_size >= layout.size() {
                LIVE_BYTES.fetch_add(new_size - layout.size(), Ordering::Relaxed);
            } else {
                LIVE_BYTES.fetch_sub(layout.size() - new_size, Ordering::Relaxed);
            }
        }
        moved
    }
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;

fn heap() -> (usize, u64) {
    (
        LIVE_BYTES.load(Ordering::Relaxed),
        ALLOCATIONS.load(Ordering::Relaxed),
    )
}

#[derive(Default)]
struct CountingSink {
    datagrams: u64,
    logs: u64,
}

impl Sink for CountingSink {
    fn send(&mut self, _to: SocketAddr, _datagram: &[u8]) {
        self.datagrams += 1;
    }

    fn log(&mut self, _line: fmt::Arguments<'_>) {
        self.logs += 1;
    }
}

const WARM_UP: usize = 1_000;
const FLOOD: usize = 10_000;

fn garbage(
    hub: &mut Hub<FakeClock, TestRng>,
    sink: &mut CountingSink,
    rng: &mut TestRng,
    scratch: &mut Vec<u8>,
) {
    let from = rng.addr();
    let len = rng.below(1400);
    scratch.clear();
    scratch.resize(len, 0);
    rng.fill_bytes(scratch);
    hub.on_datagram(from, scratch, sink);
}

/// CLIENT_BIND 1. tur, dizindeki kimlikle, rastgele (taklit) adresten; 2. tur hiç gelmez.
fn first_round(
    hub: &mut Hub<FakeClock, TestRng>,
    sink: &mut CountingSink,
    rng: &mut TestRng,
    scratch: &mut Vec<u8>,
    payload: &mut Vec<u8>,
    phone: &Phone,
) {
    let from = rng.addr();
    let mut client_nonce = [0u8; 16];
    rng.fill_bytes(&mut client_nonce);
    payload.clear();
    ClientBindRequest {
        cert_ref: phone.cert_ref,
        client_nonce,
    }
    .write(payload);
    hub_server::wire::RpcFrame {
        method_id: 0x01,
        flags: 0,
        request_id: 7,
        payload,
    }
    .encode(scratch)
    .expect("kodla");
    hub.on_datagram(from, scratch, sink);
}

#[test]
fn floods_and_steady_relay_allocate_nothing() {
    let mut rng = TestRng(0x0d05_3e30);
    let mut scratch: Vec<u8> = Vec::with_capacity(2048);
    let mut payload: Vec<u8> = Vec::with_capacity(2048);
    let alice = Phone::alice();

    let mut hub = Hub::new(test_config(), FakeClock::default(), TestRng(7)).expect("hub");
    let mut sink = CountingSink::default();
    for _ in 0..WARM_UP {
        garbage(&mut hub, &mut sink, &mut rng, &mut scratch);
        first_round(
            &mut hub,
            &mut sink,
            &mut rng,
            &mut scratch,
            &mut payload,
            &alice,
        );
    }

    // T-R1: anlamsız paket seli.
    let before = heap();
    let sent = sink.datagrams;
    for _ in 0..FLOOD {
        garbage(&mut hub, &mut sink, &mut rng, &mut scratch);
    }
    assert_eq!(sink.datagrams, sent, "0 cevap");
    assert_eq!(
        heap(),
        before,
        "T-R1: 10 000 anlamsız paket bellek ayırmadı (canlı bayt, ayırma sayısı)"
    );

    // T-B10: 1. tur seli.
    for _ in 0..FLOOD {
        first_round(
            &mut hub,
            &mut sink,
            &mut rng,
            &mut scratch,
            &mut payload,
            &alice,
        );
    }
    assert_eq!(
        sink.datagrams - sent,
        FLOOD as u64,
        "her 1. tura meydan okuma"
    );
    assert_eq!(hub.table().endpoint_count(), 0, "tablo boş");
    assert_eq!(heap(), before, "T-B10: 10 000 CLIENT_BIND bellek ayırmadı");
    assert_eq!(sink.logs, 0);

    // Kararlı iletim: paket başına bellek ayrılmaz.
    let mut world = World::new();
    let (mut a, mut b) = (Phone::alice(), Phone::bob());
    world.bind(&mut a);
    world.bind(&mut b);
    let h = world.open(&mut a, &b, envelope::SESSION_INIT, b"INIT");
    let datagram = data(h, &[0x5a; 1200]);
    let mut sink = CountingSink::default();
    for _ in 0..WARM_UP {
        world.hub.on_datagram(a.addr, &datagram, &mut sink);
    }
    let before = heap();
    for _ in 0..FLOOD {
        world.hub.on_datagram(a.addr, &datagram, &mut sink);
        world.hub.on_datagram(b.addr, &datagram, &mut sink);
    }
    assert_eq!(sink.datagrams, (WARM_UP + 2 * FLOOD) as u64);
    assert_eq!(
        heap(),
        before,
        "iletim: 20 000 veri datagramı bellek ayırmadı"
    );
}
