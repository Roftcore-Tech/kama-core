//! PARAM geri çekilmesi (sahip, 2026-11-11): ilk PARAM denemeleri ve teyitleri kaybolduğunda
//! üstel geri çekilmeli yeniden gönderim toparlıyor mu.
//!
//! Loopback, gerçek UDP; kayıp belirlenimli (egress kapısı / alıcı seam'i), netem yok.

use std::time::Duration;

use client_core::runtime::{
    ClientEndpoint, EgressPolicy, FecRuntimeConfig, RuntimeConfig, ServerEndpoint,
};
use client_core::seam::IdentitySeam;
use kama_contracts::s3::{
    Opened, PacketProtection, ProtectionClass, ProtectionContext, ProtectionLimits, SealError,
    SealedUnit,
};

/// İstemci çıkışı: koşu saatinin ilk `until_ns` nanosaniyesinde HİÇBİR datagram çıkmaz
/// (PARAM denemeleri, ilk veri ve kopyaları dahil). `arm` no-op: kapı kuruluştan itibaren.
struct BlackoutEgress {
    until_ns: u64,
    refused: u64,
}

impl EgressPolicy for BlackoutEgress {
    fn admit(&mut self, _len: usize, now_ns: u64) -> bool {
        if now_ns < self.until_ns {
            self.refused += 1;
            return false;
        }
        true
    }
}

/// IdentitySeam + alıcıda belirli birimlerin DATA'sı kalıcı düşer (yeniden iletimler dahil);
/// onarım sembolleri ve kontrol çerçeveleri geçer.
struct DropUnitsSeam {
    drop: &'static [u64],
    inner: IdentitySeam,
}

impl PacketProtection for DropUnitsSeam {
    fn seal(&mut self, ctx: ProtectionContext, frame: &[u8]) -> Result<SealedUnit, SealError> {
        self.inner.seal(ctx, frame)
    }
    fn open(&mut self, wire: &[u8]) -> Option<Opened> {
        let od = self.inner.open(wire)?;
        let unit = client_core::wire::decode_data(&od.frame)
            .map(|(_, u, _)| u)
            .or_else(|| client_core::wire::decode_data_frag(&od.frame).map(|(_, u, _, _)| u));
        if unit.is_some_and(|u| self.drop.contains(&u)) {
            return None;
        }
        Some(od)
    }
    fn sealed_len(&self, ctx: ProtectionContext, n: usize) -> Result<usize, SealError> {
        self.inner.sealed_len(ctx, n)
    }
    fn conservative_limits(&self, class: ProtectionClass) -> ProtectionLimits {
        self.inner.conservative_limits(class)
    }
    fn limits(&self, ctx: ProtectionContext) -> ProtectionLimits {
        self.inner.limits(ctx)
    }
}

/// Profil 0x03 (K 8, R 4, E 64), elle r = 4; 48 B birim.
fn fec03(delivery_queue: usize) -> RuntimeConfig {
    RuntimeConfig {
        max_unit_bytes: 48,
        fec: Some(FecRuntimeConfig {
            profile_id: 0x03,
            manual_r: Some(4),
        }),
        drain_cap: Duration::from_millis(300),
        delivery_queue,
        ..RuntimeConfig::default()
    }
}

async fn collect(
    server: &mut client_core::runtime::ServerHandle,
    n: usize,
    budget: Duration,
) -> Vec<u64> {
    let mut got = std::collections::BTreeSet::new();
    let _ = tokio::time::timeout(budget, async {
        while got.len() < n {
            let d = server.next_delivery().await.expect("teslim akışı açık");
            got.insert(d.unit_id);
        }
    })
    .await;
    got.into_iter().collect()
}

/// İlk 6 s istemciden hiçbir datagram çıkmaz. Geri çekilmeli PARAM denemeleri en erken
/// 0 / 0,2 / 0,6 / 1,4 / 3,0 / 6,2 s'de çıkar: ilk beşi yutulur, altıncısı teyit alır. Tik
/// atlaması (200 ms'lik zamanlayıcının bir turu kaçırması) denemeleri yalnız GECİKTİRİR; beşinci
/// deneme en geç ≈ 3,8 s'de, altıncısı en erken 6,2 s'de çıkar. Sonuç belirlenimli: 6 deneme,
/// 5 yutulan. Sabit 200 ms kadansta (geri çekilmesiz) aynı 6 s'de en az 15 deneme yutulur, ya da
/// 25'lik tavan dolar ve PARAM hiç teyit almaz. Beklenen ayrıca: 8/8 teslim (2'si FEC kurtarımı),
/// `param_confirmed = 1`.
#[tokio::test(flavor = "multi_thread")]
async fn param_backoff_recovers_after_initial_blackout() {
    const K: usize = 8;
    const BLACKOUT_NS: u64 = 6_000_000_000;
    let mut server = ServerEndpoint::bind(
        "127.0.0.1:0".parse().unwrap(),
        fec03(64),
        Box::new(DropUnitsSeam {
            drop: &[2, 5],
            inner: IdentitySeam::new(),
        }),
    )
    .await
    .expect("server bind");
    let mut client = ClientEndpoint::connect_with_policy(
        server.local_addr(),
        fec03(64),
        Box::new(IdentitySeam::new()),
        Some(Box::new(BlackoutEgress {
            until_ns: BLACKOUT_NS,
            refused: 0,
        })),
    )
    .await
    .expect("client connect");

    // Veri karartma bittikten ve PARAM teyidi alındıktan sonra sunulur (onarım sembolleri
    // PARAM'dan önce alıcıda anlamsızdır; bu test PARAM'ın toparlanmasını ölçer). Altıncı deneme
    // en geç ≈ 7,4 s'de çıkar (beş tik atlaması); 9 s pay bırakır.
    tokio::time::sleep(Duration::from_secs(9)).await;
    for i in 0..K {
        client
            .send(format!("param-bo-{i}").into_bytes())
            .expect("offer");
    }
    let got = collect(&mut server, K, Duration::from_secs(30)).await;
    let acked = tokio::time::timeout(Duration::from_secs(10), async {
        for _ in 0..K {
            client.acked().await.expect("acked akışı");
        }
    })
    .await;

    let cs = client.shutdown().await;
    let ss = server.shutdown().await;
    // Önce geri çekilmenin kendisi: deneme sayısı ve yutulan deneme sayısı (karartmada başka
    // datagram çıkmaz — veri 9 s'de sunulur).
    assert!(
        (5..=7).contains(&cs.param_sends),
        "geri çekilmeli PARAM denemesi 6 olmalı (5–7 pay): {cs:?}"
    );
    assert!(
        (4..=6).contains(&cs.egress_refused_pkts),
        "karartma yalnız ilk beş PARAM denemesini yutmalı (4–6 pay): {cs:?}"
    );
    assert_eq!(
        cs.param_confirmed, 1,
        "PARAM karartma sonrası teyit edilmeli: {cs:?}"
    );
    assert_eq!(got.len(), K, "8/8 teslim: {cs:?} {ss:?}");
    acked.expect("8/8 teslim-teyidi (ACK + RECOVERED)");
    assert_eq!(
        ss.fec_recovered_units, 2,
        "silinen 2 birim FEC'le kurtarılmalı: {ss:?}"
    );
    assert_eq!(ss.delivered, K as u64, "8/8: {ss:?}");
    assert_eq!(cs.undelivered, 0, "residue yok: {cs:?}");
}
