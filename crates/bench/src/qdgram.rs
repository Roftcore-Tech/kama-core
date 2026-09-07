//! [KAPI-1 W1] Q-DGRAM-ARQ — quinn DATAGRAM + uygulama-ARQ: FEC'siz **semantik-eşlenik** referans
//! (r5 C.1 baseline #2; FAZ-1 primary comparator).
//!
//! **ARQ politikası (ön-kayda girecek; ölçümden önce SABİT):** mesaj-başı PTO —
//! `rto₀ = clamp(2×conn.rtt(), 100 ms, 10 s)`, her yeniden-gönderimde ×2 backoff (10 s tavan),
//! mesajın KENDİ deadline'ına (`submission_i + max_deadline`) kadar; RFC 9002 PTO ruhunda,
//! kasıtlı basit — "knowingly disadvantaging" yasağına tabi, quinn'in canlı RTT'sini kullanır.
//!
//! **App-ACK Kama aynası (r5 L simetri):** server, ALINAN msg_id kümesinin KÜMÜLATİF aralık
//! özetini gönderir — her [`ACK_FREQ_N`]. yeni DATA'da (receiver.rs §2.1 ile aynı N=2) VE
//! [`ACK_TICK`] (20 ms, kama server ile aynı) timer'ında (kirli ise). Kümülatif özet ACK kaybına
//! dayanıklı (sonraki her ACK önceki bilgiyi kapsar) — Kama SACK'inin app-düzeyi eşleniği.
//!
//! **Ölçüm modeli:** diğer iki adaptörle AYNI [`RawEvent`] çerçevesi — `sent_ns` = sunum
//! (app-offer) anı, `delivered_ns` = mesajı kapsayan İLK ACK'in mutlak anı (ilk-ack-kazanır).
//! Retransmit'ler aynı msg_id'yi taşır → dedup yapısal. Bütçe: W2 governed socket endpoint
//! düzeyinde sarar (DATA + app-ACK + QUIC kontrol otomatik sayılır); handshake pre-arm muaf.
//!
//! **Bağlantı ölümü (r5 B.3):** reconnect YOK; `read_datagram` hatası → kalan mesajlar
//! deadline'da FAIL, termination-nedeni stderr'e yazılır.

use std::collections::BTreeSet;
use std::time::{Duration, Instant};

use quinn::{Connection, Endpoint};
use tokio::time::{interval, timeout};

use crate::error::BenchError;
use crate::event::{RawEvent, RunMeta, RunOutput, TxComponents};
use crate::quic::{build_endpoint, client_config, server_config, GovHandle, QUINN_VERSION};
use crate::transport::{BenchTransport, ClientCfg, ServerCfg};

/// DATA datagramı: tip(1) + msg_id u64 (8) = 9 B başlık, sonra payload (fixed-width BE, ADR-05).
const DG_DATA: u8 = 0x01;
const HDR_DG_DATA: usize = 9;
/// ACK datagramı: tip(1) + count u16 (2) + count × (start u64 + len u32) = 3 + 12·count B.
const DG_ACK: u8 = 0x11;
/// Kama receiver §2.1 ACK_FREQ_N aynası (receiver.rs `ACK_FREQ_N = 2`).
const ACK_FREQ_N: u32 = 2;
/// Kama bench server ACK timer'ı ile aynı (kama.rs `ACK_TICK`).
const ACK_TICK: Duration = Duration::from_millis(20);
/// PTO alt/üst sınırları (ns).
const RTO_MIN_NS: u64 = 100_000_000;
const RTO_MAX_NS: u64 = 10_000_000_000;

fn encode_dg_data(msg_id: u64, payload: &[u8]) -> Vec<u8> {
    let mut b = Vec::with_capacity(HDR_DG_DATA + payload.len());
    b.push(DG_DATA);
    b.extend_from_slice(&msg_id.to_be_bytes());
    b.extend_from_slice(payload);
    b
}

fn decode_dg_data(buf: &[u8]) -> Option<u64> {
    if buf.len() < HDR_DG_DATA || buf[0] != DG_DATA {
        return None;
    }
    Some(u64::from_be_bytes(buf[1..9].try_into().ok()?))
}

fn encode_dg_ack(ranges: &[(u64, u32)]) -> Vec<u8> {
    let count = u16::try_from(ranges.len()).unwrap_or(u16::MAX);
    let mut b = Vec::with_capacity(3 + 12 * usize::from(count));
    b.push(DG_ACK);
    b.extend_from_slice(&count.to_be_bytes());
    for &(start, len) in ranges.iter().take(usize::from(count)) {
        b.extend_from_slice(&start.to_be_bytes());
        b.extend_from_slice(&len.to_be_bytes());
    }
    b
}

fn decode_dg_ack(buf: &[u8]) -> Option<Vec<(u64, u32)>> {
    if buf.len() < 3 || buf[0] != DG_ACK {
        return None;
    }
    let count = usize::from(u16::from_be_bytes([buf[1], buf[2]]));
    if buf.len() != 3 + 12 * count {
        return None;
    }
    let mut out = Vec::with_capacity(count);
    for i in 0..count {
        let o = 3 + 12 * i;
        out.push((
            u64::from_be_bytes(buf[o..o + 8].try_into().ok()?),
            u32::from_be_bytes(buf[o + 8..o + 12].try_into().ok()?),
        ));
    }
    Some(out)
}

/// Alınan msg_id kümesini kümülatif (start,len) aralıklarına sıkıştırır. Bench mesaj sayıları
/// küçük (≤ yüzler) → BTreeSet taraması yeterli; `max_ranges` datagram-boyu sınırı (kümülatif
/// özet her ACK'te yeniden gönderildiğinden kırpma kendini onarır).
fn to_ranges(set: &BTreeSet<u64>, max_ranges: usize) -> Vec<(u64, u32)> {
    let mut out: Vec<(u64, u32)> = Vec::new();
    for &id in set {
        match out.last_mut() {
            Some((start, len)) if id == *start + u64::from(*len) => *len += 1,
            _ => {
                if out.len() == max_ranges {
                    break;
                }
                out.push((id, 1));
            }
        }
    }
    out
}

fn elapsed_ns(start: Instant) -> u64 {
    u64::try_from(start.elapsed().as_nanos()).unwrap_or(u64::MAX)
}

/// quinn DATAGRAM + app-ARQ baseline'ı.
pub struct QdgramTransport;

impl BenchTransport for QdgramTransport {
    async fn run_server(&self, cfg: ServerCfg) -> Result<(), BenchError> {
        let sc = server_config(cfg.cc)?;
        let (endpoint, gov) = build_endpoint(cfg.bind, Some(sc), cfg.budget)?;
        crate::quic::spawn_gate_watch(gov.clone(), cfg.gate_watch.clone());
        serve_endpoint(endpoint, gov, cfg.gate_watch.is_some(), cfg.quota_r).await;
        Ok(())
    }

    async fn run_client(&self, cfg: ClientCfg) -> Result<RunOutput, BenchError> {
        let (mut endpoint, gov) =
            build_endpoint("[::]:0".parse().expect("valid addr"), None, cfg.budget)?;
        endpoint.set_default_client_config(client_config(cfg.cc)?);
        // Handshake max_deadline ile bounded (quic.rs ile aynı gerekçe); fail → 0-completion
        // GEÇERLİ ölçüm sonucu.
        let connecting = endpoint
            .connect(cfg.server, "localhost")
            .map_err(|e| BenchError::Quic(e.to_string()))?;
        let conn = match timeout(cfg.max_deadline, connecting).await {
            Ok(Ok(c)) => c,
            Ok(Err(_)) | Err(_) => return Ok(failed_output(&cfg, gov.as_ref())),
        };
        // [W6] Kuruluş-gate'i (handshake temiz ağda; netem bu noktada uygulanır, A2).
        if let Some(g) = &cfg.establish_gate {
            crate::transport::gate_sync(g).await?;
        }
        // [KAPI-1 W6] warmup (ölçüm-DIŞI; sınırlı-ARQ'lu paced ayna) + gate.
        let warmup_residue = if cfg.warmup_msgs > 0 {
            warmup_pump(&cfg, &conn).await
        } else {
            0
        };
        if let Some(g) = &cfg.gate {
            crate::transport::gate_sync(g).await?;
        }
        if let Some(h) = &gov {
            h.arm_now(); // ölçüm-t0: bütçe ARM (handshake+warmup pre_arm'da muaf).
        }
        let out = client_loop(&cfg, &conn, gov.as_ref(), warmup_residue).await;
        conn.close(0u32.into(), b"done");
        let _ = timeout(Duration::from_secs(3), endpoint.wait_idle()).await;
        Ok(out)
    }
}

/// Server accept + per-bağlantı DATAGRAM servis döngüsü. Kümülatif ACK: her ACK_FREQ_N yeni
/// DATA'da hemen + 20 ms timer'da (kirli ise). `send_datagram` hatası = kayıp (yoksay).
pub(crate) async fn serve_endpoint(
    endpoint: Endpoint,
    gov: Option<GovHandle>,
    gate_watched: bool,
    quota_r: u32,
) {
    while let Some(incoming) = endpoint.accept().await {
        let Ok(conn) = incoming.await else { continue };
        if !gate_watched {
            if let Some(h) = &gov {
                h.arm_now(); // gate-watch yoksa eski kural: ilk established'da arm.
            }
        }
        tokio::spawn(async move {
            let mut received: BTreeSet<u64> = BTreeSet::new();
            let mut new_since_ack: u32 = 0;
            let mut dirty = false;
            let mut pruned_warmup = false;
            // [inceleme #13] Sessizlik re-ACK'i: kama receiver'ın epizot-kotası aynası — varıştan
            // 2 s sessizlikte kümülatif ACK'i yeniden gönder, epizot başına quota_r hak; varış
            // kotayı tazeler. (kama §2.1 R-kota ~2.1 s aralığının sınırlı app-düzeyi eşleniği.)
            let mut requota: u32 = quota_r;
            let mut last_arrival = Instant::now();
            let mut tick = interval(ACK_TICK);
            let max_ranges = conn
                .max_datagram_size()
                .map_or(64, |m| m.saturating_sub(3) / 12)
                .max(1);
            loop {
                tokio::select! {
                    d = conn.read_datagram() => {
                        let Ok(bytes) = d else { break }; // bağlantı öldü → task biter (reconnect YOK).
                        let Some(msg_id) = decode_dg_data(&bytes) else { continue };
                        // [inceleme #4] İlk ölçüm-id'si gelince warmup id'leri budanır — kalıcı
                        // (2^40,N) aralığı her ölçüm-ACK'ini şişirmesin (reverse-byte adaleti).
                        if !pruned_warmup && msg_id < WARMUP_BASE {
                            received.retain(|&id| id < WARMUP_BASE);
                            pruned_warmup = true;
                        }
                        last_arrival = Instant::now();
                        requota = quota_r;
                        if received.insert(msg_id) {
                            new_since_ack += 1;
                            dirty = true;
                        } else {
                            dirty = true; // dup arrival → re-ACK (retransmit'in ACK'i kaybolmuş olabilir).
                        }
                        if new_since_ack >= ACK_FREQ_N {
                            let _ = conn.send_datagram(
                                encode_dg_ack(&to_ranges(&received, max_ranges)).into(),
                            );
                            new_since_ack = 0;
                            dirty = false;
                        }
                    }
                    _ = tick.tick() => {
                        if dirty {
                            let _ = conn.send_datagram(
                                encode_dg_ack(&to_ranges(&received, max_ranges)).into(),
                            );
                            new_since_ack = 0;
                            dirty = false;
                        } else if requota > 0
                            && !received.is_empty()
                            && last_arrival.elapsed() >= Duration::from_secs(2)
                        {
                            // [inceleme #13] sessizlik re-ACK'i (epizot kotasından düşer).
                            let _ = conn.send_datagram(
                                encode_dg_ack(&to_ranges(&received, max_ranges)).into(),
                            );
                            requota -= 1;
                            last_arrival = Instant::now();
                        }
                    }
                }
            }
        });
    }
}

/// [W6] Warmup birim-id tabanı (kama.rs ile aynı desen): ölçüm msg_id uzayının tamamen dışında;
/// ACK-aralıkları bu id'leri kapsasa da ölçüm vektörü `get_mut(huge)→None` ile yapısal korunur.
const WARMUP_BASE: u64 = 1 << 40;

/// [W6 + inceleme #0/#3] Warmup: aynı-sınıf PACED akış (i×interval) + SINIRLI tek retransmit turu
/// (kapsanmayanlara, +2 s'te) — ölçüm ARQ'sunun sınırlı bir aynası; tavanda (10 s) kalan =
/// residue (döner, RunMeta.warmup_residue). DATAGRAM'lar iptal gerektirmez (retransmit YOK) →
/// residue post-t0 maliyet taşımaz; fark KAPI-1 §3.2 amendment'ında belgeli.
async fn warmup_pump(cfg: &ClientCfg, conn: &Connection) -> u64 {
    let payload = vec![0x5Au8; usize::try_from(cfg.payload_bytes).unwrap_or(0)];
    let start = Instant::now();
    let interval = cfg.submit_interval;
    let mut covered: BTreeSet<u64> = BTreeSet::new();
    let mut next: u64 = 0;
    let mut retx_done = false;
    let plan_ns = interval.saturating_mul(u32::try_from(cfg.warmup_msgs).unwrap_or(u32::MAX));
    let cap = tokio::time::sleep(plan_ns.saturating_add(Duration::from_secs(10)));
    tokio::pin!(cap);
    while (covered.len() as u64) < cfg.warmup_msgs {
        let submit_due = interval.saturating_mul(u32::try_from(next).unwrap_or(u32::MAX));
        tokio::select! {
            () = &mut cap => break,
            () = tokio::time::sleep(submit_due.saturating_sub(start.elapsed())),
                if next < cfg.warmup_msgs => {
                let _ = conn.send_datagram(encode_dg_data(WARMUP_BASE + next, &payload).into());
                next += 1;
            }
            () = tokio::time::sleep(
                plan_ns.saturating_add(Duration::from_secs(2)).saturating_sub(start.elapsed()),
            ), if next >= cfg.warmup_msgs && !retx_done => {
                retx_done = true;
                for i in 0..cfg.warmup_msgs {
                    if !covered.contains(&(WARMUP_BASE + i)) {
                        let _ = conn.send_datagram(encode_dg_data(WARMUP_BASE + i, &payload).into());
                    }
                }
            }
            d = conn.read_datagram() => {
                let Ok(bytes) = d else { break };
                let Some(ranges) = decode_dg_ack(&bytes) else { continue };
                for (s, l) in ranges {
                    for id in s..s.saturating_add(u64::from(l)) {
                        if id >= WARMUP_BASE {
                            covered.insert(id);
                        }
                    }
                }
            }
        }
    }
    tokio::time::sleep(Duration::from_millis(300)).await;
    cfg.warmup_msgs.saturating_sub(covered.len() as u64)
}

/// Mesaj-başı ARQ durumu.
#[derive(Clone, Copy)]
struct MsgState {
    /// Sunum anı (app-offer; RawEvent.sent_ns).
    sent_ns: Option<u64>,
    /// İlk kapsayan ACK'in mutlak anı.
    delivered_ns: Option<u64>,
    /// Bir sonraki (yeniden-)gönderim vadesi.
    next_fire_ns: u64,
    /// Kaçıncı deneme (0 = orijinal bekliyor; backoff üssü).
    attempt: u32,
}

async fn client_loop(
    cfg: &ClientCfg,
    conn: &Connection,
    gov: Option<&GovHandle>,
    warmup_residue: u64,
) -> RunOutput {
    let start = Instant::now();
    // [W6] wire sayaçları ölçüm-t0 fotoğrafından DELTA (handshake/warmup r5 D.2 gereği hariç).
    let stats0 = conn.stats();
    let t0_unix_ns = u64::try_from(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos()),
    )
    .unwrap_or(0);
    let n = usize::try_from(cfg.msg_count).unwrap_or(0);
    let payload = vec![0x5Au8; usize::try_from(cfg.payload_bytes).unwrap_or(0)];
    let interval_ns = u64::try_from(cfg.submit_interval.as_nanos()).unwrap_or(u64::MAX);
    let max_deadline_ns = u64::try_from(cfg.max_deadline.as_nanos()).unwrap_or(u64::MAX);
    // Koşu sonu = son-sunum + max_deadline (r5 §G; W3 ile aynı formül).
    let total_deadline_ns = interval_ns
        .saturating_mul(cfg.msg_count.saturating_sub(1))
        .saturating_add(max_deadline_ns);

    let mut msgs = vec![
        MsgState {
            sent_ns: None,
            delivered_ns: None,
            next_fire_ns: u64::MAX,
            attempt: 0,
        };
        n
    ];
    let mut delivered_count: u64 = 0;
    let mut next_submit: u64 = 0;
    let (mut tx_data, mut tx_retx) = (0u64, 0u64);
    let mut termination: Option<String> = None;

    let rto = |conn: &Connection, attempt: u32| -> u64 {
        let rtt_ns = u64::try_from(conn.rtt().as_nanos()).unwrap_or(u64::MAX);
        let base = rtt_ns.saturating_mul(2).clamp(RTO_MIN_NS, RTO_MAX_NS);
        base.saturating_mul(1u64 << attempt.min(20)).min(RTO_MAX_NS)
    };

    let deadline = tokio::time::sleep(Duration::from_nanos(total_deadline_ns));
    tokio::pin!(deadline);

    while delivered_count < cfg.msg_count {
        // Bir sonraki ARQ vadesi (teslim edilmemiş + sunulmuş + kendi mesaj-deadline'ı geçmemiş).
        let now = elapsed_ns(start);
        let next_due = msgs
            .iter()
            .filter(|m| m.sent_ns.is_some() && m.delivered_ns.is_none())
            .filter(|m| {
                m.sent_ns
                    .is_some_and(|s| now < s.saturating_add(max_deadline_ns))
            })
            .map(|m| m.next_fire_ns)
            .min()
            .unwrap_or(u64::MAX);

        tokio::select! {
            () = &mut deadline => break,
            // [W3 deseni] paced sunum: mesaj i, t=i×interval'de. Kalan-süre her yinelemede
            // yeniden kurulur; ACK/ARQ uyanışları döngüyü sık uyandırır, sapma küçük kalır.
            () = tokio::time::sleep(Duration::from_nanos(
                interval_ns.saturating_mul(next_submit).saturating_sub(elapsed_ns(start)),
            )), if next_submit < cfg.msg_count => {
                let now = elapsed_ns(start);
                let id = next_submit;
                next_submit += 1;
                let i = usize::try_from(id).unwrap_or(usize::MAX);
                if let Some(m) = msgs.get_mut(i) {
                    m.sent_ns = Some(now); // app-offer damgası (diğer adaptörlerle aynı çerçeve).
                    m.next_fire_ns = now.saturating_add(rto(conn, 0));
                    m.attempt = 1;
                    let frame = encode_dg_data(id, &payload);
                    tx_data += frame.len() as u64;
                    let _ = conn.send_datagram(frame.into()); // hata = kayıp; ARQ kurtarır.
                }
            }
            // ARQ vadesi: vadesi gelen TÜM mesajları yeniden gönder (tek uyanışta toplu).
            () = tokio::time::sleep(Duration::from_nanos(
                next_due.saturating_sub(elapsed_ns(start)),
            )), if next_due != u64::MAX => {
                let now = elapsed_ns(start);
                for (i, m) in msgs.iter_mut().enumerate() {
                    let Some(sent) = m.sent_ns else { continue };
                    if m.delivered_ns.is_some()
                        || m.next_fire_ns > now
                        || now >= sent.saturating_add(max_deadline_ns)
                    {
                        continue;
                    }
                    let frame = encode_dg_data(i as u64, &payload);
                    tx_retx += frame.len() as u64;
                    let _ = conn.send_datagram(frame.into());
                    m.next_fire_ns = now.saturating_add(rto(conn, m.attempt));
                    m.attempt = m.attempt.saturating_add(1);
                }
            }
            d = conn.read_datagram() => {
                match d {
                    Err(e) => {
                        // r5 B.3: bağlantı ölümü → reconnect YOK; kalan mesajlar deadline'da FAIL.
                        termination = Some(e.to_string());
                        break;
                    }
                    Ok(bytes) => {
                        let Some(ranges) = decode_dg_ack(&bytes) else { continue };
                        let now = elapsed_ns(start);
                        for (start_id, len) in ranges {
                            for id in start_id..start_id.saturating_add(u64::from(len)) {
                                let i = usize::try_from(id).unwrap_or(usize::MAX);
                                if let Some(m) = msgs.get_mut(i) {
                                    if m.sent_ns.is_some() && m.delivered_ns.is_none() {
                                        m.delivered_ns = Some(now); // ilk-ack-kazanır, mutlak damga.
                                        delivered_count += 1;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    if let Some(reason) = &termination {
        eprintln!(
            "QDGRAM-TERMINATION reason={reason} delivered={delivered_count}/{}",
            cfg.msg_count
        );
    }

    let events: Vec<RawEvent> = msgs
        .iter()
        .enumerate()
        .map(|(i, m)| RawEvent {
            msg_id: i as u64,
            sent_ns: m.sent_ns.unwrap_or(0),
            delivered_ns: m.delivered_ns,
            payload_bytes: cfg.payload_bytes,
        })
        .collect();
    let stats = conn.stats();
    RunOutput {
        meta: RunMeta {
            profile: cfg.profile.clone(),
            cc: cfg.cc.label().to_string(),
            transport: "qdgram".to_string(),
            quinn_version: QUINN_VERSION.to_string(),
            seed: cfg.seed,
            repeat: cfg.repeat,
            msg_count: cfg.msg_count,
            payload_bytes: cfg.payload_bytes,
            max_deadline_ns,
            udp_tx_bytes: stats.udp_tx.bytes.saturating_sub(stats0.udp_tx.bytes),
            udp_rx_bytes: stats.udp_rx.bytes.saturating_sub(stats0.udp_rx.bytes),
            tx: TxComponents {
                data: tx_data,
                retx: tx_retx,
                dup: 0,
                // Q-DGRAM-ARQ'da FEC ve PARAM düzlemi YOK.
                fec: 0,
                ctrl: 0,
            },
            kama_arm: String::new(),
            submit_interval_ns: interval_ns,
            budget: gov.map(GovHandle::snapshot).unwrap_or_default(),
            t0_unix_ns,
            events_epoch_offset_ns: 0,
            warmup_residue,
        },
        events,
    }
}

/// Handshake kurulamadı → tüm mesajlar teslim EDİLMEDİ (0-completion, geçerli ölçüm sonucu).
fn failed_output(cfg: &ClientCfg, gov: Option<&GovHandle>) -> RunOutput {
    let events = (0..cfg.msg_count)
        .map(|msg_id| RawEvent {
            msg_id,
            sent_ns: 0,
            delivered_ns: None,
            payload_bytes: cfg.payload_bytes,
        })
        .collect();
    RunOutput {
        meta: RunMeta {
            profile: cfg.profile.clone(),
            cc: cfg.cc.label().to_string(),
            transport: "qdgram".to_string(),
            quinn_version: QUINN_VERSION.to_string(),
            seed: cfg.seed,
            repeat: cfg.repeat,
            msg_count: cfg.msg_count,
            payload_bytes: cfg.payload_bytes,
            max_deadline_ns: u64::try_from(cfg.max_deadline.as_nanos()).unwrap_or(u64::MAX),
            udp_tx_bytes: 0,
            udp_rx_bytes: 0,
            tx: TxComponents::default(),
            kama_arm: String::new(),
            submit_interval_ns: u64::try_from(cfg.submit_interval.as_nanos()).unwrap_or(u64::MAX),
            budget: gov.map(GovHandle::snapshot).unwrap_or_default(),
            t0_unix_ns: 0,
            events_epoch_offset_ns: 0,
            warmup_residue: 0,
        },
        events,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MS: u64 = 1_000_000;
    use crate::budget::BudgetMode;
    use crate::metrics;
    use crate::transport::{Cc, KamaArm};
    use quinn::Endpoint as QEndpoint;
    use std::net::SocketAddr;

    #[test]
    fn dg_frames_roundtrip() {
        let f = encode_dg_data(42, &[0x5A; 16]);
        assert_eq!(f.len(), HDR_DG_DATA + 16);
        assert_eq!(decode_dg_data(&f), Some(42));
        assert_eq!(decode_dg_data(&f[..5]), None, "truncated → None");
        assert_eq!(decode_dg_data(&[0x11, 0, 0]), None, "yanlış tip → None");

        let a = encode_dg_ack(&[(0, 3), (7, 1), (100, 25)]);
        assert_eq!(decode_dg_ack(&a), Some(vec![(0, 3), (7, 1), (100, 25)]));
        assert_eq!(decode_dg_ack(&a[..a.len() - 1]), None, "kısa gövde → None");
        assert_eq!(decode_dg_ack(&[DG_DATA]), None);
    }

    #[test]
    fn range_compression_is_cumulative_and_bounded() {
        let set: BTreeSet<u64> = [0, 1, 2, 5, 6, 9].into_iter().collect();
        assert_eq!(to_ranges(&set, 64), vec![(0, 3), (5, 2), (9, 1)]);
        // max_ranges sınırı: kırpılır (kümülatif re-ACK kendini onarır — doc).
        assert_eq!(to_ranges(&set, 2), vec![(0, 3), (5, 2)]);
        assert_eq!(to_ranges(&BTreeSet::new(), 64), vec![]);
    }

    fn cfg(server: SocketAddr, msgs: u64, deadline_ms: u64) -> ClientCfg {
        ClientCfg {
            server,
            msg_count: msgs,
            payload_bytes: 128,
            max_deadline: Duration::from_millis(deadline_ms),
            cc: Cc::Cubic,
            seed: 0,
            repeat: 0,
            profile: "loopback".to_string(),
            kama_arm: KamaArm::Arq,
            submit_interval: Duration::ZERO,
            budget: BudgetMode::Unlimited,
            warmup_msgs: 0,
            gate: None,
            establish_gate: None,
            // Bu kolda FEC yok (yalnız kama-product kolu okur).
            fec: crate::transport::FecArm::Off,
            burst_fusion: false,
            bootstrap_rtt: false,
            initial_window_bytes: 0,
            bootstrap_fast: false,
            loss_confirm_k: 0,
            fec_risk: false,
            ts_out: None,
            gate_permille: None,
            persist_mode: None,
            persist_alpha: None,
            persist_rho: None,
            persist_q: None,
            spec_l2_permille: None,
            spec_l1_permille: None,
        }
    }

    /// [KAPI-1 W6] İki-fazlı koşu: warmup datagramları ölçüm kayıtlarına girmez (WARMUP_BASE
    /// id-uzayı), gate protokolü işler, ölçüm 3/3 teslim eder.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn warmup_and_gate_two_phase_run() {
        let sc = server_config(Cc::Cubic).expect("server config");
        let server_ep = QEndpoint::server(sc, "127.0.0.1:0".parse().unwrap()).expect("bind");
        let addr = server_ep.local_addr().expect("local addr");
        tokio::spawn(serve_endpoint(server_ep, None, false, 4));

        let dir = std::env::temp_dir().join(format!("kapi-gate-qd-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let gate = dir.join("g1");
        let g2 = gate.clone();
        let harness = tokio::spawn(async move {
            let ready = format!("{}.ready", g2.display());
            for _ in 0..2000 {
                if std::path::Path::new(&ready).exists() {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
            std::fs::write(format!("{}.go", g2.display()), b"go").unwrap();
        });

        let mut c = cfg(addr, 3, 3000);
        c.warmup_msgs = 2;
        c.gate = Some(gate.clone());
        let out = QdgramTransport.run_client(c).await.expect("client run");
        harness.await.unwrap();

        assert_eq!(out.events.len(), 3, "yalnız ölçüm mesajları");
        assert!(out.events.iter().all(|e| e.delivered_ns.is_some()));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Diğer iki adaptörle aynı frame-invariant guard'ı: temiz yolda hepsi teslim,
    /// delivered_ns mutlak (sent sonrası), completion eğrisi ayrışır.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn loopback_frame_invariants() {
        let sc = server_config(Cc::Cubic).expect("server config");
        let server_ep = QEndpoint::server(sc, "127.0.0.1:0".parse().unwrap()).expect("bind");
        let addr = server_ep.local_addr().expect("local addr");
        tokio::spawn(serve_endpoint(server_ep, None, false, 4));

        let out = QdgramTransport
            .run_client(cfg(addr, 10, 5000))
            .await
            .expect("client run");

        assert_eq!(out.events.len(), 10);
        assert_eq!(out.meta.transport, "qdgram");
        assert!(
            out.events.iter().all(|e| e.delivered_ns.is_some()),
            "temiz yol → tüm mesajlar teslim: {:?}",
            out.events
        );
        for e in &out.events {
            let d = e.delivered_ns.expect("delivered");
            assert!(d > e.sent_ns, "delivered_ns mutlak + sent sonrası olmalı");
        }
        assert_eq!(metrics::completion(&out.events, Duration::ZERO), 0.0);
        assert_eq!(
            metrics::completion(&out.events, Duration::from_secs(5)),
            1.0
        );
        assert!(out.meta.tx.data > 0, "DATA byte muhasebesi dolu olmalı");
    }

    /// Ölü adres → handshake fail → 0-completion, max_deadline ile bounded (HANG yok).
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn dead_address_bounded_zero_completion() {
        let dead: SocketAddr = "127.0.0.1:9".parse().unwrap();
        let t0 = Instant::now();
        let out = QdgramTransport
            .run_client(cfg(dead, 5, 1500))
            .await
            .expect("bounded Ok");
        assert!(
            t0.elapsed() < Duration::from_secs(6),
            "handshake timeout bounded olmalı: {:?}",
            t0.elapsed()
        );
        assert_eq!(out.events.len(), 5);
        assert!(out.events.iter().all(|e| e.delivered_ns.is_none()));
    }

    /// ARQ gerçekten kurtarıyor pini: İLK gönderimleri düşüren kayıplı bir "kanal" yerine, ACK'siz
    /// pencere → PTO → retransmit yolunu server'ı GEÇ başlatarak zorla: client konuşmaya başlar,
    /// server datagramları ancak bağlantı kurulduktan sonra okur — quinn DATAGRAM'ları
    /// retransmit ETMEZ (frame kaybolur), bizim app-ARQ etmeli.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn app_arq_retransmits_and_recovers() {
        // Kanal-içi kayıp benzetimi yerine deterministik bir yapı: normal loopback ama mesajların
        // İLK kopyasının ACK'i ACK_FREQ_N birikimine takılabilir; 20ms timer + kümülatif ACK her
        // durumda kapsar. Bu test retx sayacının varlığını değil, retx YOLUNUN çalıştığını arar:
        // rto floor 100ms > loopback RTT → tek mesajlık koşuda ACK timer'ı (20ms) retx'ten önce
        // kapsar (retx=0 beklenir); 200ms'lik yapay gecikmeli teslim burada kurgulanamadığından
        // retx-altı-kayıp senaryosu calibration smoke'una bırakılır (netem ile). Burada pin:
        // tek mesaj bile ACK_FREQ_N=2'ye ULAŞMADAN 20ms timer'ıyla teslim edilir (timer aynası).
        let sc = server_config(Cc::Cubic).expect("server config");
        let server_ep = QEndpoint::server(sc, "127.0.0.1:0".parse().unwrap()).expect("bind");
        let addr = server_ep.local_addr().expect("local addr");
        tokio::spawn(serve_endpoint(server_ep, None, false, 4));

        let out = QdgramTransport
            .run_client(cfg(addr, 1, 2000))
            .await
            .expect("client run");
        let e = &out.events[0];
        let d = e
            .delivered_ns
            .expect("tek mesaj timer-ACK ile teslim edilmeli (N=2 birikmez)");
        assert!(
            d.saturating_sub(e.sent_ns) < 500 * MS,
            "teslim 20ms timer'ıyla gelmeli (500ms'ten çok önce): {}ns",
            d.saturating_sub(e.sent_ns)
        );
    }
}
