//! PR #190 kontrolleri (sahip, 2026-11-11): (1) ilk PARAM denemeleri ve teyitleri kaybolduğunda
//! geri çekilmeli yeniden gönderim toparlıyor mu; (2) normal bantta küçük birimli akışta FEC
//! teslimi/kurtarımı geriliyor mu (parite tavanı kontrolcü yolunda bloğun kaynak baytıyla
//! ölçeklenir; elle-r koluna uygulanmaz).
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

/// (1) İlk 1,5 s istemciden hiçbir şey çıkmaz: PARAM'ın ilk denemeleri (0 / 0,2 / 0,6 / 1,4 s)
/// ve ilk veri kopyaları kaybolur. Beklenen: geri çekilmeli deneme (3,0 s) teyit alır, 8/8
/// teslim (2'si FEC kurtarımı), PARAM deneme sayısı sınırlı (geri çekilme: ≤ 25, bu pencerede
/// 5–7), `param_confirmed = 1`.
#[tokio::test(flavor = "multi_thread")]
async fn param_backoff_recovers_after_initial_blackout() {
    const K: usize = 8;
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
            until_ns: 1_500_000_000,
            refused: 0,
        })),
    )
    .await
    .expect("client connect");

    // Karartma boyunca PARAM denemeleri (0 / 0,2 / 0,6 / 1,4 s) yutulur; 3,0 s'deki deneme geçer.
    // Veri karartma bittikten ve PARAM teyidi alındıktan sonra sunulur (onarım sembolleri
    // PARAM'dan önce alıcıda anlamsızdır; bu test PARAM'ın toparlanmasını ölçer).
    tokio::time::sleep(Duration::from_millis(3_600)).await;
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
    assert_eq!(got.len(), K, "8/8 teslim: {cs:?} {ss:?}");
    acked.expect("8/8 teslim-teyidi (ACK + RECOVERED)");
    assert_eq!(
        cs.param_confirmed, 1,
        "PARAM karartma sonrası teyit edilmeli: {cs:?}"
    );
    assert!(
        cs.egress_refused_pkts >= 3,
        "karartma ilk denemeleri yutmalı: {cs:?}"
    );
    // Geri çekilme: 0 / 0,2 / 0,6 / 1,4 s yutulur (zamanlayıcı 200 ms adımlı; 4. deneme 1,6 s'ye
    // kayabilir ve geçer); en geç 3,0 s'deki deneme geçer, teyit loopback'te hemen döner →
    // 4–6 deneme. Sabit 200 ms kadansta karartma boyunca 8 deneme yutulurdu.
    assert!(
        (4..=6).contains(&cs.param_sends),
        "geri çekilmeli PARAM denemesi 4–6 olmalı: {cs:?}"
    );
    assert_eq!(
        ss.fec_recovered_units, 2,
        "silinen 2 birim FEC'le kurtarılmalı: {ss:?}"
    );
    assert_eq!(ss.delivered, K as u64, "8/8: {ss:?}");
    assert_eq!(cs.undelivered, 0, "residue yok: {cs:?}");
}

/// (2) Normal bant (loopback), küçük birimli akış, kontrolcü yolu (ürün profili 0x05, E 1024).
/// Blok 0'da kayıp geçmişi kurulur (birim 3 kalıcı silinir), blok 1 kayıp geçmişiyle kapanır.
/// 400 B birimlerde tavan `Σ kaynak_tel / parite_tel` = 3: kontrolcünün istediği r'yi kesmez
/// (`fec_parity_payload_clamped = 0`); kontrolcü parite planlarsa blok 1'in silinen birimi (11)
/// FEC ile kurtarılır. 65 B ileti bloklarında tavan 0: parite çıkmaz (`fec_repairs_tx = 0`),
/// kontrolcü r istediyse sayaç artar; teslim ARQ ile sürer.
#[tokio::test(flavor = "multi_thread")]
async fn small_unit_flow_keeps_fec_at_normal_band_and_tiny_block_gets_no_parity() {
    const K: usize = 8;
    for unit_len in [400usize, 65usize] {
        let cfg = || RuntimeConfig {
            drain_cap: Duration::from_millis(600),
            ..RuntimeConfig::default()
        };
        let mut server = ServerEndpoint::bind(
            "127.0.0.1:0".parse().unwrap(),
            cfg(),
            Box::new(DropUnitsSeam {
                drop: &[3, 11],
                inner: IdentitySeam::new(),
            }),
        )
        .await
        .expect("server bind");
        let client =
            ClientEndpoint::connect(server.local_addr(), cfg(), Box::new(IdentitySeam::new()))
                .await
                .expect("client connect");
        // Blok 0: kayıp geçmişi (birim 3 kalıcı düşer; ilk blok RTT örneksiz kapanır → r = 0).
        for i in 0..K {
            client.send(vec![i as u8; unit_len]).expect("offer");
        }
        let got0 = collect(&mut server, K - 1, Duration::from_secs(10)).await;
        assert_eq!(
            got0.len(),
            K - 1,
            "birim {unit_len} B: blok 0'da 7/8 (3 kalıcı silik)"
        );
        tokio::time::sleep(Duration::from_millis(1500)).await;
        // Blok 1: kayıp geçmişiyle kapanır; birim 11 kalıcı düşer.
        for i in K..2 * K {
            client.send(vec![i as u8; unit_len]).expect("offer");
        }
        let got1 = collect(&mut server, K, Duration::from_secs(20)).await;
        let cs = client.shutdown().await;
        let ss = server.shutdown().await;
        println!(
            "[birim {unit_len} B] blok1 teslim={} latched={} repairs_tx={} clamped={} recovered={} undelivered={}",
            got1.len(),
            cs.fec_blocks_latched,
            cs.fec_repairs_tx,
            cs.fec_parity_payload_clamped,
            ss.fec_recovered_units,
            cs.undelivered
        );
        assert!(
            got1.len() >= K - 1,
            "birim {unit_len} B: blok 1 en az 7/8: {cs:?}"
        );
        if unit_len == 400 {
            assert_eq!(
                cs.fec_parity_payload_clamped, 0,
                "400 B birimde tavan kontrolcüyü kesmemeli: {cs:?}"
            );
            if cs.fec_blocks_latched >= 1 {
                // Kontrolcü parite planladıysa tavan onu engellememiş olmalı ve 11 kurtarılmalı.
                assert!(cs.fec_repairs_tx >= 1, "parite tele çıkmalı: {cs:?}");
                assert!(
                    ss.fec_recovered_units >= 1,
                    "birim 11 FEC ile kurtarılmalı: {ss:?}"
                );
            }
        } else {
            assert_eq!(
                cs.fec_repairs_tx, 0,
                "65 B ileti bloğuna parite çıkmamalı: {cs:?}"
            );
            assert_eq!(cs.tx_fec_bytes, 0, "65 B: FEC baytı 0: {cs:?}");
        }
    }
}
