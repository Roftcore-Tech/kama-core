//! [ACK-BITMAP · YETENEK ANLAŞMASI] Eski/yeni uç dört birleşimi (gerçek UDP 127.0.0.1, gerçek
//! `client_task` / `server_task`): 0x15 bitmap ACK yalnız İKİ uç da bitmap biçimine izin verdiğinde
//! ve veri gönderen uç CAPS ile çözebildiğini bildirdiğinde yazılır; aksi hâlde oturum bugünkü 0x14
//! yoluyla tamamlanır.
//!
//! "Eski" uç burada `ack_format = V2` yapılandırmasıdır. Bunun main'deki ikiliyle aynı davrandığı
//! şuna dayanır (kod okuması, aynı commit'te değişmeyen yollar): V2 istemcisi CAPS göndermez ve
//! 0x15'i bilinmeyen çerçeve sayar (main'deki gibi); V2 sunucusu CAPS'i bitmap'i açmadan yok
//! sayar — main'deki sunucu aynı çerçeveyi bilinmeyen çerçeve sayıp yok sayar (yuva açmadan).
//! İki sürümün ikilisiyle çapraz koşu bu testte yoktur.
//!
//! Profil: dar dönüş (sunucunun yukarı yönü 62 B/s) — alıcı bütçeli ve geri bildirimi aç, yani
//! bitmap yazmaya uygun. Loopback kayıpsızdır; amaç biçim anlaşmasıdır, kayıp kurtarma değil.

use std::time::Duration;

use client_core::runtime::{
    AckFormat, ClientEndpoint, ClientStats, LinkRates, RuntimeConfig, ServerEndpoint, ServerStats,
};
use client_core::seam::IdentitySeam;

const N: u64 = 12;
const FWD_BPS: u64 = 12_500;
const REV_BPS: u64 = 62;

fn cfg(format: AckFormat, link: LinkRates) -> RuntimeConfig {
    RuntimeConfig {
        ack_format: format,
        link_rates: Some(link),
        drain_cap: Duration::from_millis(200),
        ..RuntimeConfig::default()
    }
}

async fn run(client_format: AckFormat, server_format: AckFormat) -> (ClientStats, ServerStats) {
    let mut server = ServerEndpoint::bind(
        "127.0.0.1:0".parse().unwrap(),
        cfg(
            server_format,
            LinkRates {
                uplink_bps: REV_BPS,
                downlink_bps: FWD_BPS,
            },
        ),
        Box::new(IdentitySeam::new()),
    )
    .await
    .expect("server bind");
    let mut client = ClientEndpoint::connect(
        server.local_addr(),
        cfg(
            client_format,
            LinkRates {
                uplink_bps: FWD_BPS,
                downlink_bps: REV_BPS,
            },
        ),
        Box::new(IdentitySeam::new()),
    )
    .await
    .expect("client connect");
    for i in 0..N {
        client
            .send(format!("kama-caps-{i}").into_bytes())
            .expect("offer");
    }
    let done = tokio::time::timeout(Duration::from_secs(90), async {
        let mut delivered = 0u64;
        let mut acked = 0u64;
        while delivered < N || acked < N {
            tokio::select! {
                Some(_) = server.next_delivery(), if delivered < N => delivered += 1,
                Some(_) = client.acked(), if acked < N => acked += 1,
            }
        }
        (delivered, acked)
    })
    .await;
    let cs = client.shutdown().await;
    let ss = server.shutdown().await;
    assert_eq!(
        done.expect("90 s içinde teslim ve teyit"),
        (N, N),
        "istemci {client_format:?} × sunucu {server_format:?}"
    );
    (cs, ss)
}

#[tokio::test(flavor = "multi_thread")]
async fn bitmap_is_used_only_when_both_ends_allow_it() {
    use AckFormat::{Bitmap, V2};
    for (c, s) in [(V2, V2), (Bitmap, V2), (V2, Bitmap), (Bitmap, Bitmap)] {
        let (cs, ss) = run(c, s).await;
        eprintln!(
            "CAPS istemci={c:?} sunucu={s:?}: caps_sends={} caps_rx={} ack_bitmap_rx={} acked={} unknown_frames(istemci)={}",
            cs.caps_sends, ss.caps_rx, cs.ack_bitmap_rx, cs.acked, cs.unknown_frames
        );
        let tag = format!("istemci {c:?} × sunucu {s:?}");
        // CAPS yalnız bitmap'e izin veren istemciden çıkar; bounded retry.
        assert_eq!(cs.caps_sends > 0, c == Bitmap, "{tag}: CAPS");
        assert!(cs.caps_sends <= 8, "{tag}: CAPS deneme tavanı");
        // 0x15 yalnız iki uç da izin verdiğinde.
        assert_eq!(
            cs.ack_bitmap_rx > 0,
            c == Bitmap && s == Bitmap,
            "{tag}: bitmap ACK"
        );
        // V2 istemcisi hiçbir koşulda 0x15 görmez (bilinmeyen çerçeve sayılmaz).
        if c == V2 {
            assert_eq!(cs.unknown_frames, 0, "{tag}: istemciye bilinmeyen çerçeve");
        }
    }
}
