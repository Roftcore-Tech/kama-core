//! [KAPI-1 W2] Packet-atomic bütçe governor'ü (r5 §D) — İKİ transporta ÖZDEŞ tek implementasyon.
//!
//! Normatif kurallar (r5 D.4 + KAPI-1 §2.3):
//! - **Packet-atomic:** `kalan < ip_len` → paket kanala enjekte EDİLMEZ; partial/overshoot YOK.
//! - **Pacing-nötr:** governor ASLA geciktirmez ve kuyruk tutmaz. Ret = enjeksiyon yok; transportun
//!   kendi kurtarması bunu kanal-silinmesiyle eşdeğer görür (ön-kayıtlı semantik).
//! - **Yön-başına:** her uç kendi egress'ini yönetir (client=fwd, server=rev); aktarım YOK.
//! - **DÜZLEM-A handshake muafiyeti:** `arm()` öncesi her şey serbestçe geçer ve AYRI sayılır
//!   (`pre_arm_*`); bütçe yalnız arm sonrası uygulanır. Kama'da arm=t0 (handshake yok); quinn'de
//!   arm=connection-established (W6 barrier'ı bunu t=0'a rafine edecek).
//! - Aritmetik tam-sayı (token'lar byte·ns cinsinden u128) — float/RNG yok, deterministik.
//!
//! `ip_len` = udp_payload + [`IPV4_UDP_OVERHEAD`] (IP-düzeyi bütçe, r5 H "IP-layer observed bytes").

use serde::{Deserialize, Serialize};

/// IPv4 (20) + UDP (8) başlık ek yükü — bütçe IP-düzeyinde sayar (r5 D.2/H).
pub const IPV4_UDP_OVERHEAD: u64 = 28;

const NS_PER_S: u128 = 1_000_000_000;

/// Bütçe biçimi (r5 D.1: workload-ayrık).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BudgetMode {
    /// Bütçe yok (governor saf sayaç gibi davranır; hiçbir şey reddedilmez).
    #[default]
    Unlimited,
    /// burst: one-shot kümülatif hard-cap `B = C×D×u/8` (kanal-türevli, transport-BAĞIMSIZ).
    Burst {
        /// Toplam enjeksiyon tavanı (IP byte).
        cap_bytes: u64,
    },
    /// paced: token-bucket — `refill = C×u/8`, `bucket` = ön-kayıtlı burst-allowance (D6: 2 s ×
    /// refill), `ceiling = C×W×u/8` kümülatif tavan. Kova DOLU başlar (allowance semantiği).
    Paced {
        /// Dolum hızı (IP byte / s).
        refill_bytes_per_s: u64,
        /// Kova boyu (IP byte) — anlık patlama izni.
        bucket_bytes: u64,
        /// Kümülatif tavan (IP byte); `u64::MAX` = tavansız.
        ceiling_bytes: u64,
    },
}

impl BudgetMode {
    /// Rapor/manifest etiketi.
    #[must_use]
    pub fn label(&self) -> String {
        match self {
            Self::Unlimited => "unlimited".to_string(),
            Self::Burst { cap_bytes } => format!("burst:{cap_bytes}"),
            Self::Paced {
                refill_bytes_per_s,
                bucket_bytes,
                ceiling_bytes,
            } => format!("paced:{refill_bytes_per_s}/{bucket_bytes}/{ceiling_bytes}"),
        }
    }
}

/// RunMeta'ya gömülen sayaç fotoğrafı (r5 D.5 metrikleri buradan türer). Eski JSONL → default.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct BudgetSnapshot {
    /// Mod etiketi ("unlimited" | "burst:N" | "paced:r/b/c"; eski kayıtlarda boş).
    pub mode: String,
    /// Arm sonrası enjekte edilen IP byte (netem'in sonra düşürdükleri DAHİL — r5 H).
    pub injected_bytes: u64,
    /// Arm sonrası enjekte edilen paket sayısı.
    pub injected_pkts: u64,
    /// Reddedilen (enjekte edilmeyen) IP byte.
    pub refused_bytes: u64,
    /// Reddedilen paket sayısı.
    pub refused_pkts: u64,
    /// İlk ret anı ns (budget_exhaustion_time, r5 D.5); 0 = hiç ret olmadı.
    pub first_refusal_ns: u64,
    /// Arm ÖNCESİ geçen IP byte (DÜZLEM-A handshake muafiyeti — bütçeye girmez, gizlenmez).
    pub pre_arm_bytes: u64,
    /// Arm öncesi paket sayısı.
    pub pre_arm_pkts: u64,
}

/// Tek-yön governor. Paylaşım gerekiyorsa (quinn socket sarmalı) `Arc<Mutex<_>>` ile sar.
#[derive(Debug)]
pub struct BudgetGovernor {
    mode: BudgetMode,
    armed: bool,
    /// Paced token durumu: byte·ns (tam-sayı; kova DOLU başlar).
    token_bns: u128,
    last_refill_ns: u64,
    injected_bytes: u64,
    injected_pkts: u64,
    refused_bytes: u64,
    refused_pkts: u64,
    first_refusal_ns: Option<u64>,
    pre_arm_bytes: u64,
    pre_arm_pkts: u64,
}

impl BudgetGovernor {
    #[must_use]
    pub fn new(mode: BudgetMode) -> Self {
        let token_bns = match mode {
            BudgetMode::Paced { bucket_bytes, .. } => u128::from(bucket_bytes) * NS_PER_S,
            _ => 0,
        };
        Self {
            mode,
            armed: false,
            token_bns,
            last_refill_ns: 0,
            injected_bytes: 0,
            injected_pkts: 0,
            refused_bytes: 0,
            refused_pkts: 0,
            first_refusal_ns: None,
            pre_arm_bytes: 0,
            pre_arm_pkts: 0,
        }
    }

    /// Bütçe uygulamasını başlat (DÜZLEM-A t=0). Öncesi trafiği `pre_arm_*`'a sayılmıştır.
    /// İkinci çağrı no-op (idempotent). Kova dolu kalır; refill saati buradan akar.
    pub fn arm(&mut self, now_ns: u64) {
        if !self.armed {
            self.armed = true;
            self.last_refill_ns = now_ns;
        }
    }

    /// Test-gözlemi (ürün yolu arm-durumunu sorgulamaz; W6 barrier'ı yalnız `arm()` çağırır).
    #[cfg(test)]
    #[must_use]
    pub fn is_armed(&self) -> bool {
        self.armed
    }

    fn refill(&mut self, now_ns: u64) {
        if let BudgetMode::Paced {
            refill_bytes_per_s,
            bucket_bytes,
            ..
        } = self.mode
        {
            let dt = now_ns.saturating_sub(self.last_refill_ns);
            self.token_bns = (self.token_bns + u128::from(dt) * u128::from(refill_bytes_per_s))
                .min(u128::from(bucket_bytes) * NS_PER_S);
            self.last_refill_ns = now_ns;
        }
    }

    /// Tek paket izni (packet-atomic). `true` → paket enjekte edilebilir (sayıldı);
    /// `false` → enjekte ETME (ret sayıldı). Asla bekletmez.
    pub fn permit(&mut self, ip_len: u64, now_ns: u64) -> bool {
        self.permit_n(ip_len, 1, now_ns)
    }

    /// Çok-paketlik atomik izin (yalnız savunmacı GSO yolu — sarmalayıcı
    /// `max_transmit_segments()=1` ile bunu pratikte tekile indirger). Ya hepsi ya hiçbiri.
    pub fn permit_n(&mut self, ip_len_total: u64, pkts: u64, now_ns: u64) -> bool {
        if !self.armed {
            self.pre_arm_bytes = self.pre_arm_bytes.saturating_add(ip_len_total);
            self.pre_arm_pkts = self.pre_arm_pkts.saturating_add(pkts);
            return true;
        }
        let granted = match self.mode {
            BudgetMode::Unlimited => true,
            BudgetMode::Burst { cap_bytes } => {
                self.injected_bytes.saturating_add(ip_len_total) <= cap_bytes
            }
            BudgetMode::Paced { ceiling_bytes, .. } => {
                self.refill(now_ns);
                self.injected_bytes.saturating_add(ip_len_total) <= ceiling_bytes
                    && self.token_bns >= u128::from(ip_len_total) * NS_PER_S
            }
        };
        if granted {
            if let BudgetMode::Paced { .. } = self.mode {
                self.token_bns -= u128::from(ip_len_total) * NS_PER_S;
            }
            self.injected_bytes = self.injected_bytes.saturating_add(ip_len_total);
            self.injected_pkts = self.injected_pkts.saturating_add(pkts);
        } else {
            self.refused_bytes = self.refused_bytes.saturating_add(ip_len_total);
            self.refused_pkts = self.refused_pkts.saturating_add(pkts);
            self.first_refusal_ns.get_or_insert(now_ns);
        }
        granted
    }

    #[must_use]
    pub fn snapshot(&self) -> BudgetSnapshot {
        BudgetSnapshot {
            mode: self.mode.label(),
            injected_bytes: self.injected_bytes,
            injected_pkts: self.injected_pkts,
            refused_bytes: self.refused_bytes,
            refused_pkts: self.refused_pkts,
            first_refusal_ns: self.first_refusal_ns.unwrap_or(0),
            pre_arm_bytes: self.pre_arm_bytes,
            pre_arm_pkts: self.pre_arm_pkts,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// r5 D.4 packet-atomic: kalan < ip_len → paket TAMAMEN reddedilir; partial düşüm yok.
    #[test]
    fn burst_cap_is_packet_atomic() {
        let mut g = BudgetGovernor::new(BudgetMode::Burst { cap_bytes: 100 });
        g.arm(0);
        assert!(g.permit(60, 10)); // 60/100
        assert!(!g.permit(60, 20), "kalan 40 < 60 → atomik ret");
        assert!(
            g.permit(40, 30),
            "40'lık paket hâlâ sığar (overshoot yok, açlık da yok)"
        );
        assert!(!g.permit(1, 40), "cap doldu");
        let s = g.snapshot();
        assert_eq!((s.injected_bytes, s.injected_pkts), (100, 2));
        assert_eq!((s.refused_bytes, s.refused_pkts), (61, 2));
        assert_eq!(
            s.first_refusal_ns, 20,
            "budget_exhaustion_time = İLK ret anı"
        );
    }

    /// Paced: kova dolu başlar (burst-allowance), tam-sayı refill, bucket tavanı, ceiling durdurur.
    #[test]
    fn paced_refill_bucket_and_ceiling() {
        let mut g = BudgetGovernor::new(BudgetMode::Paced {
            refill_bytes_per_s: 1000,
            bucket_bytes: 500,
            ceiling_bytes: 1200,
        });
        g.arm(0);
        assert!(
            g.permit(500, 0),
            "kova dolu başlar → allowance anında kullanılabilir"
        );
        assert!(
            !g.permit(100, 0),
            "kova boş → aynı anda ikinci paket reddedilir (bekletme YOK)"
        );
        // 100 ms sonra: 100 B birikti; 100'lük paket geçer, 101'lik geçmezdi.
        assert!(g.permit(100, 100_000_000));
        assert!(!g.permit(1, 100_000_000), "token'lar tüketildi");
        // 1 s sonra: refill 1000 ama kova 500'e KIRPILIR.
        assert!(g.permit(500, 1_100_000_000));
        assert!(!g.permit(200, 1_100_000_000));
        // Kümülatif 1100/1200 → 150'lik paket ceiling'i aşar (token olsa bile).
        assert!(
            !g.permit(150, 3_000_000_000),
            "ceiling kümülatif tavanı atomik uygular"
        );
        assert!(g.permit(100, 3_000_000_000), "100 tam sığar (1200'e)");
        let s = g.snapshot();
        assert_eq!(s.injected_bytes, 1200);
    }

    /// DÜZLEM-A muafiyeti: arm öncesi HER ŞEY geçer ve ayrı sayılır; bütçeden düşmez.
    #[test]
    fn pre_arm_traffic_is_exempt_and_separately_counted() {
        let mut g = BudgetGovernor::new(BudgetMode::Burst { cap_bytes: 100 });
        assert!(!g.is_armed());
        assert!(g.permit(10_000, 0), "arm öncesi (handshake) serbest");
        g.arm(50);
        g.arm(60); // idempotent
        assert!(g.is_armed());
        assert!(
            g.permit(100, 100),
            "bütçe arm'dan itibaren TAM (handshake düşmedi)"
        );
        let s = g.snapshot();
        assert_eq!((s.pre_arm_bytes, s.pre_arm_pkts), (10_000, 1));
        assert_eq!(s.injected_bytes, 100);
    }

    /// Unlimited: hiçbir şey reddedilmez; sayaçlar yine tutulur (delivered-vs-byte eğrisi için).
    #[test]
    fn unlimited_counts_but_never_refuses() {
        let mut g = BudgetGovernor::new(BudgetMode::Unlimited);
        g.arm(0);
        assert!(g.permit(u64::MAX, 0));
        assert!(g.permit(1, 1));
        let s = g.snapshot();
        assert_eq!(s.refused_pkts, 0);
        assert_eq!(s.injected_pkts, 2);
        assert_eq!(s.mode, "unlimited");
    }

    /// Savunmacı GSO yolu: çok-segmentli transmit ya bütünüyle girer ya bütünüyle reddedilir.
    #[test]
    fn multi_segment_permit_is_all_or_nothing() {
        let mut g = BudgetGovernor::new(BudgetMode::Burst { cap_bytes: 100 });
        g.arm(0);
        assert!(
            !g.permit_n(120, 3, 0),
            "3 segment toplam 120 > 100 → hiçbiri girmez"
        );
        assert!(g.permit_n(90, 3, 0));
        let s = g.snapshot();
        assert_eq!((s.injected_bytes, s.injected_pkts), (90, 3));
        assert_eq!((s.refused_bytes, s.refused_pkts), (120, 3));
    }
}
