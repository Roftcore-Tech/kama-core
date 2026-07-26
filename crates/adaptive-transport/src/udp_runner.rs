//! Commit-AB — POLICY-AWARE UDP BENCH-RUNNER (feature-gated; ÜRETİM-YOLUNA-BAĞLI-DEĞİL; root'suz —
//! root-yalnız-dış-netns/tc-katmanında). GERÇEK-UdpSocket + GERÇEK-Sender-budget-yolu; default-policy=
//! LeftoverOnly; product/admin-activation-YOK. Çıktı: blok-ledger-CSV (pos-şema-alt-kümesi) + run-özeti
//! + canonical-config-dump (config-hash + build-sürümü). Exit: 0=başarı, 2=timeout, 3=arg-hatası.
#![allow(clippy::missing_errors_doc)]

use crate::budget_kernel::PolicyParams;
use crate::sender::{
    AckInfo, BudgetPolicy, Event, OpaqueUnit, RecoveredInfo, RecoveredUnit, Sender, SrLogicalRef,
    Timer, TransmitKind,
};
use std::collections::{BTreeMap, BTreeSet};
use std::io::Write as _;
use std::net::UdpSocket;
use std::time::{Duration, Instant};

pub const UNIT: usize = 1200;
const K: u64 = 8;

/// FC-closure DI-10 — cold-state incomplete-block cap kararı (SR-AUDIT-001 bounds-before-alloc).
/// `open_incomplete` = şu-an-açık (tamamlanmamış, en-az-bir-shard'lı) blok-sayısı. Cap>0 ve
/// `open_incomplete >= cap` ise YENİ-blok reddedilir (slot ALLOCATE edilmez; mevcut-bloklar
/// EVICT-EDİLMEZ → sessiz-eviction-yok; reddin-kendisi typed-diag ile sayılır). Cap==0 = sınırsız.
/// Saf-fonksiyon (RNG-yok; aynı-girdi→aynı-çıktı) → determinist unit-test edilebilir.
#[must_use]
pub fn cold_admit_new_block(open_incomplete: usize, cap: u32) -> bool {
    cap == 0 || open_incomplete < cap as usize
}

/// TUR-A1 W-2B (harness-gözlem): TransmitDecision-listesini tara.
/// probe_wire=ilk CsProbe wire-emission; pto_wire=Retransmit emission'ları (§9-ARQ reaktif yolu);
/// dup_wire=Duplicate emission'ları (§11 forward-recovery, ACK-bağımsız yolu). unit_id base-göreli
/// (rx ile eşleşir). retransmit_path yapısal-varsayım DEĞİL: hangi-kind re-send-ettiği GÖZLENİR.
/// Salt-gözlem; mekanizmaya DOKUNMAZ (production-code-DEĞİL).
#[cfg(feature = "experimental-cold-start")]
fn w2b_scan_tx(
    out: &[crate::sender::TransmitDecision],
    t_ms: u64,
    base: u64,
    probe_wire: &mut Option<u64>,
    pto_wire: &mut Vec<(u64, u64)>,
    dup_wire: &mut Vec<(u64, u64)>,
) {
    use crate::sender::TransmitKind;
    for d in out {
        match d.kind {
            TransmitKind::CsProbe => {
                probe_wire.get_or_insert(t_ms);
            }
            TransmitKind::Retransmit => {
                pto_wire.push((t_ms, d.unit_id.saturating_sub(base)));
            }
            TransmitKind::Duplicate => {
                dup_wire.push((t_ms, d.unit_id.saturating_sub(base)));
            }
            _ => {}
        }
    }
}

#[derive(Debug, Clone)]
pub struct RunnerCfg {
    pub policy: String, // leftover|p15|f15 (+param-alanları-CLI'dan)
    pub ent_share: u32,
    pub hc_share: u32,
    pub ent_cap: u64,
    pub hc_cap: u64,
    pub ticket_cap: u32, // bilgi-amaçlı (kernel-AdmissionCaps-sabit-8; dump'ta-görünür)
    pub t_floor_mult: u32,
    pub op_horizon_ms: u64,
    pub ext_horizon_ms: u64,
    pub run_id: String,
    pub seed: u64,
    pub out_dir: String,
    pub w_units: u64,
    pub peer: String,
    pub bind: String,
    pub fixed_r: u16,     // FD-oracle: sabit-r (0..=4; close_fec_block_manual-r)
    pub emit_fuzzy: bool, // FC: fuzzy-event-dataset yaz (additive; decision-değişmez)
    pub deficit: u16, // FK: TEST-ONLY exact-source-erasure (rx-ingress; per-block-ilk-N-source-esi düşür)
    pub deficit_from_block: u32, // FK: deficit YALNIZ b>=bu (warmup; ilk-paket-kaybı-cold-start-deadlock önle)
    pub drop_repair: bool, // FK: FEC-IZOLASYON — silinen-esi'nin substrat-onarımını (§9-ARQ 0x01 + §11-dup 0x03)
    // da düşür → blok YALNIZ first-flight (kalan-source + parity) ile tamamlanabilir (saf-FEC-decodability testi).
    pub tx_settle_ms: u64, // FR: workload+parity-drain-sonrası SACK/ARQ-round-trip-settle (yüksek-RTT için ≥birkaç-RTT)
    pub unit_bytes: u64,   // FC-closure: DATA-birim-boyutu (W-3 65/300/1200B; default 1200=UNIT)
    pub drop_first_sack: bool, // FC-closure W-2 TEST-ONLY: İLK-SACK-emisyonunu exact-once-suppress
    pub cold_block_cap: u32, // FC-closure DI-10: rx incomplete-block hard-cap (0=sınırsız/UNIT-default)
    // ── TUR-A1 test-only knob'ları (harness; PRODUCTION-CODE-DEĞİL) ──
    pub no_settle_exit: bool, // A1: tx settle-exit'i TAMAMEN kapat (W-3-65: post-blackout heal 120s-ufka-kadar gözlenir)
    pub suppress_data_acks: bool, // A1 W-2B: rx TÜM matching DATA-ACK/SACK'i bastır (echo 0x1A HER ZAMAN geçer);
                                  // arrival-triggered release: ilk transport-duplicate DATA arrival'da (PTO-retransmit) suppression bırakılır.
}

impl RunnerCfg {
    #[must_use]
    pub fn canonical_dump(&self) -> String {
        format!(
            "policy={};ent_share={};hc_share={};ent_cap={};hc_cap={};ticket_cap={};t_floor_mult={};op={};ext={};seed={};w={}",
            self.policy, self.ent_share, self.hc_share, self.ent_cap, self.hc_cap,
            self.ticket_cap, self.t_floor_mult, self.op_horizon_ms, self.ext_horizon_ms,
            self.seed, self.w_units
        )
    }
    #[must_use]
    pub fn config_hash(&self) -> u64 {
        let mut h = 0xcbf2_9ce4_8422_2325u64;
        for b in self.canonical_dump().bytes() {
            h ^= u64::from(b);
            h = h.wrapping_mul(0x0000_0100_0000_01B3);
        }
        h
    }
    fn to_policy(&self) -> Result<BudgetPolicy, String> {
        let mut p = PolicyParams::defaults();
        p.share_permille = self.ent_share;
        p.hardcap_share_permille = self.hc_share;
        p.entitlement_cap_bytes = self.ent_cap;
        p.hardcap_cap_bytes = self.hc_cap;
        p.floor_mult_permille = self.t_floor_mult;
        match self.policy.as_str() {
            "leftover" => Ok(BudgetPolicy::LeftoverOnly),
            "p15" => Ok(BudgetPolicy::ProtectedShareExperimental(p)),
            "f15" | "f10" | "f20" => Ok(BudgetPolicy::FloorCapExperimental(p)),
            other => Err(format!("bilinmeyen-policy: {other}")),
        }
    }
}

/// DR-CS-1 M1 ADAPTER (EK5-ii — NORMATİF; teslim-edilen-kod): poll-iterasyonu başına saat BİR-KEZ
/// örneklenir; o iterasyonda toplanan TÜM inbound aynı `now_ns` ile damgalanır ve TEK
/// `on_event_batch` çağrısına gider → aynı-ns olaylar ayrı-batch'lere DÜŞEMEZ (atomiklik-inşaatla).
/// Flag-OFF'ta derlenmez (legacy per-event yol birebir; INV-CS-5).
#[cfg(feature = "experimental-cold-start")]
pub struct CsBatchAdapter {
    now_ns: u64,
    events: Vec<Event>,
}

#[cfg(feature = "experimental-cold-start")]
impl CsBatchAdapter {
    #[must_use]
    pub fn new(now_ns: u64) -> Self {
        Self {
            now_ns,
            events: Vec::new(),
        }
    }
    pub fn push(&mut self, ev: Event) {
        self.events.push(ev);
    }
    /// İterasyon-damgası (tüm-inbound bu-değerle işlenir).
    #[must_use]
    pub fn stamp(&self) -> u64 {
        self.now_ns
    }
    /// Tek-batch teslimi: (damga, olaylar).
    #[must_use]
    pub fn drain(self) -> (u64, Vec<Event>) {
        (self.now_ns, self.events)
    }
}

/// Datagram: [kind u8][seq u64][unit u64][block u32][esi u16] + dolgu (size-temsili).
fn enc(kind: u8, seq: u64, unit: u64, b: u32, e: u16, size: usize) -> Vec<u8> {
    let mut v = vec![0u8; size.max(23)];
    v[0] = kind;
    v[1..9].copy_from_slice(&seq.to_be_bytes());
    v[9..17].copy_from_slice(&unit.to_be_bytes());
    v[17..21].copy_from_slice(&b.to_be_bytes());
    v[21..23].copy_from_slice(&e.to_be_bytes());
    v
}

/// RX-tarafı: alınanları-say + kümülatif-SACK + MDS-decode → RECOVERED bildir. Root'suz.
pub fn run_rx(cfg: &RunnerCfg, deadline: Duration) -> Result<(), String> {
    let sock = UdpSocket::bind(&cfg.bind).map_err(|e| e.to_string())?;
    sock.set_read_timeout(Some(Duration::from_millis(20))).ok();
    let start = Instant::now();
    let mut acked: BTreeSet<u64> = BTreeSet::new();
    let mut par_have: BTreeMap<u32, BTreeSet<u16>> = BTreeMap::new(); // distinct-parity-shard esi (kind 0x02)
    let mut recovered: Vec<(u32, u16)> = Vec::new();
    let mut peer: Option<std::net::SocketAddr> = None;
    let mut buf = [0u8; 2048];
    let mut last_sack = Instant::now();
    let mut last_rx = Instant::now(); // erken-çıkış: ilk-paketten-sonra-5s-sessizlik → bitir
    let mut seen_any = false;
    let mut rx_start: Option<Instant> = None;
    let mut block_complete: BTreeMap<u32, u64> = BTreeMap::new(); // block → complete-ms (K-source-hazır)
    let mut block_first: BTreeMap<u32, u64> = BTreeMap::new(); // block → ilk-görülme-ms (open-proxy)
    let mut src_have: BTreeMap<u32, BTreeSet<u16>> = BTreeMap::new(); // yalnız-source-esi (<K)
    let mut finish_seen = 0u32; // FA: sender-FINISH-marker sayacı (temiz-bitiş)
    let mut ev_count = 0u64;
    let mut dropped: BTreeSet<(u32, u16)> = BTreeSet::new(); // FK: ingress-düşürülen source
    let mut deficit_applied: BTreeMap<u32, u16> = BTreeMap::new(); // block→uygulanan-deficit
    let mut first_repair: BTreeMap<u32, u64> = BTreeMap::new(); // FJ: block→ilk-parity-arrival-ms
    let mut decode_via: BTreeMap<u32, u8> = BTreeMap::new(); // block→0=substrat(src-full),1=FEC(parity)
    let mut repair_kind: BTreeMap<u32, u8> = BTreeMap::new(); // FK: silinen-esi'yi hangi-substrat kurtardı
                                                              // (0x01=§9-ARQ, 0x03=§11-dup, 0=none/FEC-only)
    #[cfg(feature = "experimental-cold-start")]
    let mut cs_probe_rx: u64 = 0; // DR-CS-1: gelen-probe sayacı
    #[cfg(feature = "experimental-cold-start")]
    let mut cs_echo_tx: u64 = 0; // üretilen-echo (alıcı-defteri; INV-CS-2/C4)
    #[cfg(feature = "experimental-cold-start")]
    let mut cs_echo_tx_bytes: u64 = 0; // cold_echo_tx_bytes = echo×43B
    #[cfg(feature = "experimental-cold-start")]
    let mut cs_echo_suppressed: u64 = 0; // T_ECHO_MIN-suppress (ledger-eşleşme-testi DI-13C)
    #[cfg(feature = "experimental-cold-start")]
    let mut cs_last_echo: Option<Instant> = None;
    #[cfg(feature = "experimental-cold-start")]
    let mut cs_dup_deliveries: u64 = 0; // FC-closure W-2: (block,esi) zaten-src_have'de → duplicate-teslim
    #[cfg(feature = "experimental-cold-start")]
    let mut cs_first_sack_dropped = false; // W-2 exact-once: ilk-SACK-suppress-uygulandı-mı
    #[cfg(feature = "experimental-cold-start")]
    let mut cs_rxcap_diag: u64 = 0; // DI-10: cold-block-cap-aşımı typed-diag sayacı (sessiz-eviction-yok)
                                    // ── TUR-A1 W-2B-R2 faithful ACK-starvation ledger (rx-tarafı; suppress_data_acks-gated; HER-İKİ-build ──
                                    // ON/OFF matched-arm karşılaştırması için GENEL (feature'a-değil suppress_data_acks-runtime-flag'ine bağlı;
                                    // default-false → flag-OFF byte-özdeş). Cold-start-özel alanlar (probe/echo/srtt) tx-tarafında ON-only.
    let mut w2b_suppression_start: Option<u64> = None; // ilk-DATA-ACK/SACK bastırma anı (rel-ms)
    let mut w2b_suppressed_ack_count: u64 = 0; // bastırılan matching-ACK/SACK sayısı
    let mut w2b_transport_dup_rx: u64 = 0; // (block,esi) zaten-src_have → transport-duplicate DATA arrival
    let mut w2b_app_delivery: u64 = 0; // block-completion'da app-teslim (birim başına BİR kez)
    let w2b_app_dup_delivery: u64 = 0; // uygulama-katmanı çift-teslim: was_new-guard + dedup → yapısal 0 (§2.5)
    let mut w2b_released: bool = false; // arrival-triggered release uygulandı mı
    let mut w2b_release_time: Option<u64> = None; // suppression_release (rel-ms)
    let mut w2b_released_ack_wire: Option<u64> = None; // release-sonrası İLK SACK wire-emission (rel-ms)
    let mut w2b_retransmit_rx: Vec<(u64, u64)> = Vec::new(); // (rel-ms, unit_id) transport-duplicate DATA arrival'ları (POST-completion spurious re-send)
    let mut w2b_retransmit_rx_count: u64 = 0;
    // R3 (Abkarada-düzeltme-2): TRIGGER/RECOVERY AYRIMI. completion_trigger_kind = block-0'ı TAMAMLAYAN
    // shard'ın wire-kind'ı (0x00=FirstFlight · 0x01=ExistingPtoArq · 0x03=ForwardRecoveryDup · 0x02=OtherKind).
    // Bu tek-başına recovery-yolu-DEĞİL (first-flight completion recovery-değildir); recovery-olayları AYRI.
    let mut w2b_completion_kind: Option<u8> = None; // completion_trigger_kind (block-0 tamamlayan shard)
    let mut w2b_recovery_events_before_completion: u64 = 0; // block-0 completion-ÖNCESİ re-send/recovery olayları
    let mut w2b_first_recovery_kind: Option<u8> = None; // block-0 İLK recovery-event'inin wire-kind'ı (0x01/0x02/0x03)
    let mut w2b_first_recovery_time: Option<u64> = None; // block-0 İLK recovery-event zamanı
                                                         // R3 (Abkarada-düzeltme-3): AMPİRİK ilk-dispatch-oranı (protokol-cwnd DEĞİL). empirical_initial_dispatch =
                                                         // ilk-uçuş-burst (stall-gap >1500ms öncesi block-0 distinct-source-shard) × unit_bytes. Salt-gözlem; sender-diff GEREKMEZ.
    let mut w2b_ff_burst_units: u64 = 0; // block-0 ilk-uçuş-burst distinct-source-shard sayısı
    let mut w2b_ff_ended = false; // ilk-uçuş-burst bitti mi (stall-gap gözlendi)
    let mut w2b_last_src_arrival: Option<u64> = None; // block-0 son source-shard arrival (gap-tespiti)
                                                      // R3 (Abkarada A+C-bounds): ACK_CAP_HARD = runaway-güvenlik-sigortası (release-DEĞİL; aşımda ABORT→INVALID).
                                                      // Formül-türevi: ceil(GLOBAL_WATCHDOG_MS/SACK_CADENCE_MS) × 2 = ceil(150000/50)×2 = 6000. Normal-hücrede
                                                      // erişilemez (en-uzun-horizon 85s → ~1700 suppressed). suppressed_ack SATURATING (overflow-yok).
    const W2B_SACK_CADENCE_MS: u64 = 50;
    const W2B_ACK_CAP_HARD: u64 = 6000;
    let mut w2b_hard_cap_hit = false; // ACK_CAP_HARD aşıldı mı → run INVALID (DUR-B-tetiği)
    let mut repair_arrival: BTreeMap<u32, u64> = BTreeMap::new(); // FR: substrat-onarım-arrival-ms (ARQ-repair
                                                                  // wall-clock; T_fec_decode<deadline<T_arq testi)
                                                                  // DR-CS-1 M6 (EK4-A1; S-A profil-türevi patience): cold-start-flag'iyle idle-terk-eşiği
                                                                  // RX_COLD_ABANDON'a-yükseltilir (max-birim-1200B'de 75s HİÇBİR-profilde-geçerli-değil,
                                                                  // F0-tespit-(viii): 102-105s). Ürün-alıcı-patience-host'u-YOK (harness-sözleşme; hüküm-tavanı
                                                                  // SENDER-SIDE). Flag-OFF'ta 8s-korunur (INV-CS-5 kimliği).
    #[cfg(feature = "experimental-cold-start")]
    let idle_abandon = Duration::from_secs(105); // S-A: max(102-104.4, 105-tavan)
    #[cfg(not(feature = "experimental-cold-start"))]
    let idle_abandon = Duration::from_secs(8);
    while start.elapsed() < deadline {
        // FA: idle-erken-çıkış YALNIZ-FALLBACK (FINISH-marker gelmezse); normal-yol=FINISH-marker.
        if seen_any && last_rx.elapsed() > idle_abandon {
            break;
        }
        if finish_seen >= 3 {
            break; // FINISH-marker ≥3-onaylı (UDP-kaybına-karşı) → temiz-drain-bitti
        }
        if let Ok((n, from)) = sock.recv_from(&mut buf) {
            if n >= 1 && buf[0] == 0xFE {
                // FA: sender-FINISH-marker (workload-tamam) → ACK-FINISH + drain-onayı.
                peer = Some(from);
                seen_any = true;
                finish_seen += 1;
                last_rx = Instant::now();
                let _ = sock.send_to(&[0xFEu8], from);
                continue;
            }
            // DR-CS-1 F2B: CS-probe (0x19) → idempotent-stateless ECHO (0x1A; §2.57-temsil; 43B=
            // ECHO_WIRE_MAX-sınıfı) + T_ECHO_MIN=500ms/akış rate-limit (suppress-sayaçlı).
            // seen_count/arrival_gap YALNIZ-LEDGER (karar-girdisi-değil). Per-probe-allocation-YOK.
            #[cfg(feature = "experimental-cold-start")]
            if n >= 4 && buf[0] == 0x19 {
                cs_probe_rx += 1;
                seen_any = true;
                let gap_ms = last_rx.elapsed().as_millis() as u32;
                last_rx = Instant::now();
                let allow =
                    cs_last_echo.is_none_or(|t: Instant| t.elapsed() >= Duration::from_millis(500));
                if allow {
                    cs_last_echo = Some(Instant::now());
                    let mut e = [0u8; 43];
                    e[0] = 0x1A; // CTRL_CS_ECHO
                    e[1] = 0x01; // subtype
                    e[2..4].copy_from_slice(&buf[2..4]); // probe_seq (yansıtılan)
                    e[4..8].copy_from_slice(&(ev_count as u32).to_be_bytes()); // seen_count (ledger)
                    e[8..12].copy_from_slice(&gap_ms.to_be_bytes()); // arrival_gap (ledger)
                    let _ = sock.send_to(&e, from);
                    cs_echo_tx += 1;
                    cs_echo_tx_bytes += 43;
                } else {
                    cs_echo_suppressed += 1;
                }
                continue;
            }
            if n >= 23 {
                peer = Some(from);
                seen_any = true;
                ev_count += 1;
                last_rx = Instant::now();
                let seq = u64::from_be_bytes(buf[1..9].try_into().unwrap());
                let kind = buf[0];
                let b = u32::from_be_bytes(buf[17..21].try_into().unwrap());
                let e = u16::from_be_bytes(buf[21..23].try_into().unwrap());
                let now_ms = rx_start
                    .get_or_insert_with(Instant::now)
                    .elapsed()
                    .as_millis() as u64;
                block_first.entry(b).or_insert(now_ms);
                // FK: TEST-ONLY exact-source-erasure — ilk-uçuş-source-esi<deficit DÜŞER (ingress;
                // FEC-decode/ARQ-öncesi). Retransmit (kind=Retransmit=0x01 ⇒ enc'de-değil; wire-kind
                // datagram[0]) ingress-drop-EDİLMEZ (ARQ-kurtarır). İlk-uçuş=Data(0x00)/Parity(0x02).
                let target_esi = e < K as u16
                    && cfg.deficit > 0
                    && b >= cfg.deficit_from_block
                    && e < cfg.deficit;
                // İlk-uçuş-source (0x00) ERASE: gerçek-silinme (rx'e-hiç-girmez).
                if target_esi && kind == 0x00 && !dropped.contains(&(b, e)) {
                    dropped.insert((b, e));
                    deficit_applied.entry(b).or_insert(0);
                    *deficit_applied.get_mut(&b).unwrap() += 1;
                    continue; // ingress-drop
                }
                // Substrat-onarımı (§9-ARQ=0x01 / §11-dup=0x03) silinen-esi için:
                if target_esi && dropped.contains(&(b, e)) && (kind == 0x01 || kind == 0x03) {
                    if cfg.drop_repair {
                        continue; // FEC-IZOLASYON: substrat-onarımını da düşür → yalnız-FEC tamamlayabilir
                    }
                    // HİBRİT: onarımı geçir + hangi-substrat-arm + NE ZAMAN kurtardı (ilk-onarım-arrival;
                    // wall-clock ARQ-repair-latency = Phase-C T_arq_repair_arrival girdisi).
                    repair_kind.entry(b).or_insert(kind);
                    repair_arrival.entry(b).or_insert(now_ms);
                }
                acked.insert(seq);
                // SINIFLANDIRMA WIRE-KIND'A göre (esi-büyüklüğüne DEĞİL): parity-esi 0-tabanlı
                // (close_fec_block_manual `for esi in 0..r`) → source-esi ile ÇAKIŞIR. MDS-decode
                // ayrı-küme sayar: distinct = |source-esi| + |parity-shard|. K-distinct → recoverable.
                if kind == 0x02 {
                    par_have.entry(b).or_default().insert(e);
                    first_repair.entry(b).or_insert(now_ms); // ilk-parity-arrival (FJ)
                } else {
                    // FC-closure W-2: (block,esi) zaten-varsa DUPLICATE-teslim (spurious-retransmit'in
                    // alıcı-dedup'u; app-teslimi-block-completion-ile-BİR-kez). DI-10: incomplete-block
                    // cold-cap (aşımda typed-diag; SESSİZ-EVICTION-YOK — yeni-blok-kabul-edilmez-sayılır).
                    // DI-10: YENİ-blok mı? (henüz-src-shard-yok VE tamamlanmamış). Cap-kararı YALNIZ
                    // yeni-blok-slot-tahsisi-öncesi (bounds-before-alloc); mevcut-blok/duplicate cap'e-takılmaz.
                    #[cfg(feature = "experimental-cold-start")]
                    {
                        let is_new_block = src_have.get(&b).is_none_or(BTreeSet::is_empty)
                            && !block_complete.contains_key(&b);
                        if cfg.cold_block_cap > 0 && is_new_block {
                            let open_incomplete = src_have
                                .iter()
                                .filter(|(bb, s)| !block_complete.contains_key(bb) && !s.is_empty())
                                .count();
                            if !cold_admit_new_block(open_incomplete, cfg.cold_block_cap) {
                                cs_rxcap_diag += 1; // typed-diag (sessiz-DEĞİL); yeni-slot ALLOCATE-edilmez.
                                continue; // bounds-before-alloc: reddedilir; mevcut-bloklar EVICT-edilmez.
                            }
                        }
                    }
                    let sh = src_have.entry(b).or_default();
                    let is_dup = sh.contains(&e);
                    #[cfg(feature = "experimental-cold-start")]
                    if is_dup {
                        cs_dup_deliveries += 1;
                    }
                    // TUR-A1 W-2B-R2 (GENEL; her-iki-build; suppress_data_acks-gated): (block,esi) zaten-src_have
                    // → transport-duplicate DATA arrival (spurious re-send). unit_id = base-göreli block*K+esi
                    // (tx-eşleşir). ARRIVAL-TRIGGERED RELEASE: İLK duplicate'te suppression bırakılır (§2.1).
                    if is_dup && cfg.suppress_data_acks {
                        let unit_id = u64::from(b) * K + u64::from(e);
                        w2b_transport_dup_rx += 1;
                        w2b_retransmit_rx.push((now_ms, unit_id));
                        w2b_retransmit_rx_count += 1;
                        // ARRIVAL-TRIGGERED RELEASE: İLK transport-duplicate'te suppression bırakılır (§2.1).
                        if !w2b_released {
                            w2b_released = true;
                            w2b_release_time = Some(now_ms);
                        }
                    }
                    // R3 (düzeltme-2): block-0 completion-ÖNCESİ RECOVERY-OLAYLARI (re-send: 0x01/0x02/0x03).
                    // FirstFlight (0x00) recovery-DEĞİL. first_recovery_event + count.
                    if cfg.suppress_data_acks && b == 0 && !block_complete.contains_key(&0) {
                        if matches!(kind, 0x01..=0x03) {
                            w2b_recovery_events_before_completion += 1;
                            if w2b_first_recovery_kind.is_none() {
                                w2b_first_recovery_kind = Some(kind);
                                w2b_first_recovery_time = Some(now_ms);
                            }
                        }
                        // R3 (düzeltme-3): ilk-uçuş-burst = stall-gap ÖNCESİ block-0 YENİ-source-shard sayısı
                        // (ampirik effective-initial-cwnd). GAP eşiği = 1500ms (floor-rate re-send'leri ayırır).
                        if !is_dup && (e as u64) < K {
                            if let Some(prev) = w2b_last_src_arrival {
                                if now_ms.saturating_sub(prev) > 1500 {
                                    w2b_ff_ended = true;
                                }
                            }
                            if !w2b_ff_ended {
                                w2b_ff_burst_units += 1;
                            }
                            w2b_last_src_arrival = Some(now_ms);
                        }
                    }
                    sh.insert(e); // 0x00/0x01/0x03 = source-shard
                }
                let src_n = src_have.get(&b).map_or(0, BTreeSet::len);
                let par_n = par_have.get(&b).map_or(0, BTreeSet::len);
                let src_full = src_n >= K as usize; // tüm-source geldi (substrat/onarım tamamladı)
                if src_n + par_n >= K as usize {
                    let was_new = !block_complete.contains_key(&b);
                    block_complete.entry(b).or_insert(now_ms);
                    // TUR-A1 W-2B-R2 (GENEL): app-teslim block-completion'da BİR kez (was_new-guard → K-source
                    // birim teslim edilir). was_new=false (duplicate re-entry) → TEKRAR-teslim YOK (dedup) →
                    // app_dup_delivery ARTMAZ (yapısal 0; §2.5). recovery_path = TAMAMLAYAN-shard'ın wire-kind'ı.
                    if cfg.suppress_data_acks && was_new {
                        w2b_app_delivery += K;
                        if b == 0 && w2b_completion_kind.is_none() {
                            w2b_completion_kind = Some(kind); // completion_trigger_kind (block-0 tamamlayan shard)
                        }
                    }
                    if was_new {
                        // decode_via: src_full → substrat(0; erasure onarımla/tam-source doldu);
                        // aksi → FEC(1; parity K-distinct'i first-flight'ta tamamladı, source-eksik).
                        decode_via.insert(b, u8::from(!src_full));
                        // eksik-source-esi'yi RECOVERED-kontrolüyle tx'e bildir (ARQ-iptal): src_have'a
                        // YAZMA (src_full-etiketini bozmasın) — yalnız kontrol-mesajı.
                        if let Some(sh) = src_have.get(&b) {
                            for miss in 0..K as u16 {
                                if !sh.contains(&miss)
                                    && !recovered.iter().any(|&(bb, ee)| bb == b && ee == miss)
                                {
                                    recovered.push((b, miss));
                                }
                            }
                        }
                    }
                }
            }
        }
        if last_sack.elapsed() >= Duration::from_millis(50) {
            if let Some(p) = peer {
                // SACK: [0xA0][count u16][seq...]; RECOVERED: [0xA1][count][block u32+esi u16...]
                let tail: Vec<u64> = acked.iter().rev().take(64).copied().collect();
                let mut v = vec![0xA0u8];
                v.extend((tail.len() as u16).to_be_bytes());
                for q in &tail {
                    v.extend(q.to_be_bytes());
                }
                let sack_ms = rx_start
                    .as_ref()
                    .map_or(0, |s| s.elapsed().as_millis() as u64);
                // FC-closure W-2A (drop_first_sack) COLD-START-gated; W-2B-R2 (suppress_data_acks) GENEL.
                #[cfg(feature = "experimental-cold-start")]
                let first_sack = {
                    let f = cfg.drop_first_sack && !cs_first_sack_dropped && !tail.is_empty();
                    if f {
                        cs_first_sack_dropped = true;
                    }
                    f
                };
                #[cfg(not(feature = "experimental-cold-start"))]
                let first_sack = false;
                // TUR-A1 W-2B-R2 (GENEL; her-iki-build): arrival-triggered release'e kadar TÜM matching-SACK
                // bastırılır (probe-echo 0x1A ayrı-yolda; hiç-bastırılmaz). Release-sonrası ilk-SACK geçer.
                let w2b_suppress = cfg.suppress_data_acks && !w2b_released && !tail.is_empty();
                if w2b_suppress {
                    w2b_suppression_start.get_or_insert(sack_ms);
                    // A+C: suppressed_ack SATURATING diagnostic (release-tetiği-DEĞİL). ACK_CAP_HARD =
                    // runaway-güvenlik: aşımda ABORT→INVALID (release-DEĞİL); normal-hücrede erişilemez.
                    if w2b_suppressed_ack_count < W2B_ACK_CAP_HARD {
                        w2b_suppressed_ack_count += 1;
                    } else {
                        w2b_hard_cap_hit = true; // saturate + runaway-flag (DUR-B/INVALID)
                    }
                    let _ = W2B_SACK_CADENCE_MS; // formül-türevi (pre-reg gerekçesi; last_sack cadence=50ms)
                } else if cfg.suppress_data_acks && w2b_released && !tail.is_empty() {
                    w2b_released_ack_wire.get_or_insert(sack_ms); // release-sonrası İLK SACK wire-emit
                }
                let suppress = first_sack || w2b_suppress;
                if !suppress {
                    let _ = sock.send_to(&v, p);
                }
                if !recovered.is_empty() {
                    let mut r = vec![0xA1u8];
                    let take: Vec<_> = recovered.drain(..).take(100).collect();
                    r.extend((take.len() as u16).to_be_bytes());
                    for (b, e) in take {
                        r.extend(b.to_be_bytes());
                        r.extend(e.to_be_bytes());
                    }
                    let _ = sock.send_to(&r, p);
                }
            }
            last_sack = Instant::now();
        }
    }
    // FA: RECEIVER-GRACEFUL-FINALIZE — ledger + checksum + done-marker (SIGTERM-değil, temiz-yol).
    std::fs::create_dir_all(&cfg.out_dir).map_err(|e| e.to_string())?;
    let op = cfg.op_horizon_ms;
    let mut body = String::from(
        "run_id,block,first_ms,complete_ms,rel_ms,dl_op,deficit_applied,decode_via,first_repair_ms,repair_kind,arq_repair_ms\n",
    );
    let (mut finalized, mut pending) = (0u64, 0u64);
    for (b, first) in &block_first {
        let (comp, rel, dlop) = match block_complete.get(b) {
            Some(&c) => {
                finalized += 1;
                (c as i64, (c - first) as i64, u8::from((c - first) <= op))
            }
            None => {
                pending += 1;
                (-1, -1, 0)
            }
        };
        let da = deficit_applied.get(b).copied().unwrap_or(0);
        let dv = decode_via.get(b).map_or(-1i8, |&v| v as i8); // -1=incomplete,0=substrat,1=FEC
        let fr = first_repair.get(b).map_or(-1i64, |&v| v as i64);
        let rk = repair_kind.get(b).copied().unwrap_or(0); // 0=none/FEC-only,1=§9-ARQ,3=§11-dup
        let ar = repair_arrival.get(b).map_or(-1i64, |&v| v as i64); // substrat-onarım-arrival (rel rx_start)
        body.push_str(&format!(
            "{},{b},{first},{comp},{rel},{dlop},{da},{dv},{fr},{rk},{ar}\n",
            cfg.run_id
        ));
    }
    // FNV-1a checksum (ledger-integrity; deterministik).
    let mut ck = 0xcbf2_9ce4_8422_2325u64;
    for by in body.bytes() {
        ck ^= u64::from(by);
        ck = ck.wrapping_mul(0x0000_0100_0000_01B3);
    }
    let lpath = format!("{}/{}-rxledger.csv", cfg.out_dir, cfg.run_id);
    std::fs::write(&lpath, &body).map_err(|e| e.to_string())?;
    // FA: done-marker (orchestrator-BAŞARI-teyidi; marker-yoksa run≠başarılı).
    let clean = finish_seen >= 3; // FINISH-onaylı-mı (yoksa idle-fallback)
    #[allow(unused_mut)]
    let mut marker = format!(
        "run_id={}\npolicy={}\nreceiver_events={ev_count}\nfinalized_blocks={finalized}\npending_blocks={pending}\nledger_path={lpath}\nledger_checksum={ck:x}\nclean_shutdown={clean}\n",
        cfg.run_id, cfg.policy
    );
    // TUR-A1 W-3-65 (B1): recovery_observation — block-0'ın erased-esi'sini HANGİ substrat kurtardı
    // (repair_kind: 0x01=§9-ARQ-retransmit, 0x03=§11-forward-recovery-control/dup). deficit>0 (loss) rejiminde.
    if cfg.deficit > 0 {
        let ro = match repair_kind.get(&0) {
            Some(0x01) => "RecoveredByArqRetransmit",
            Some(0x03) => "RecoveredByForwardRecoveryControl",
            Some(_) => "RecoveredByOtherSubstrate",
            None => "FirstFlightOrFecOrUnrecovered",
        };
        marker.push_str(&format!("recovery_observation={ro}\n"));
    }
    #[cfg(feature = "experimental-cold-start")]
    marker.push_str(&format!(
        "cs_probe_rx={cs_probe_rx}\ncs_echo_tx={cs_echo_tx}\ncs_echo_tx_bytes={cs_echo_tx_bytes}\ncs_echo_suppressed={cs_echo_suppressed}\ncs_dup_deliveries={cs_dup_deliveries}\ncs_first_sack_dropped={cs_first_sack_dropped}\ncs_rxcap_diag={cs_rxcap_diag}\n"
    ));
    // TUR-A1 W-2B-R2 rx-tarafı ledger (GENEL; her-iki-build → ON/OFF matched-arm; clock-domain: tek-host
    // CLOCK_MONOTONIC). feature_state compile-time; completion_time = block-0 ilk-completion (rx-block-ts).
    if cfg.suppress_data_acks {
        #[cfg(feature = "experimental-cold-start")]
        let feature_state = "ON";
        #[cfg(not(feature = "experimental-cold-start"))]
        let feature_state = "OFF";
        let retransmit_first = w2b_retransmit_rx
            .first()
            .map_or_else(|| "none".to_string(), |(t, u)| format!("{t}ms#unit{u}"));
        let completed = block_complete.get(&0).copied();
        let completion_time = completed.map_or_else(|| "none".to_string(), |v| format!("{v}ms"));
        // R3 (düzeltme-2): completion_trigger_kind = block-0'ı TAMAMLAYAN shard'ın wire-kind'ı (recovery-yolu
        // DEĞİL — first-flight completion recovery-değildir). Ayrı alanlar: first_recovery_event + count.
        let completion_trigger_kind = match w2b_completion_kind {
            Some(0x00) => "FirstFlight".to_string(),
            Some(0x01) => "ExistingPtoArq".to_string(),
            Some(0x03) => "ForwardRecoveryDup".to_string(),
            Some(0x02) => "OtherExistingProductionKind[FecParity]".to_string(),
            Some(k) => format!("OtherExistingProductionKind[{k:#04x}]"),
            None => "NoCompletionWithinHorizon".to_string(),
        };
        let first_recovery_kind = match w2b_first_recovery_kind {
            Some(0x01) => "ExistingPtoArq".to_string(),
            Some(0x02) => "FecParity".to_string(),
            Some(0x03) => "ForwardRecoveryDup".to_string(),
            Some(k) => format!("OtherKind[{k:#04x}]"),
            None => "none".to_string(),
        };
        let first_recovery_time =
            w2b_first_recovery_time.map_or_else(|| "none".to_string(), |v| format!("{v}ms"));
        // recovery_contributed_to_completion: completion-öncesi recovery-event VARSA veya completing-shard
        // bir-recovery-kind (0x01/0x02/0x03) ise TRUE. FirstFlight-completion + recovery-event-yok → FALSE.
        let completing_is_recovery = matches!(w2b_completion_kind, Some(0x01..=0x03));
        let recovery_contributed =
            w2b_recovery_events_before_completion > 0 || completing_is_recovery;
        // recovery_path: YALNIZ gerçekten-recovery-gözlendiyse (aksi "none"; düzeltme-2 disiplini).
        let recovery_path = if recovery_contributed {
            completion_trigger_kind.clone()
        } else {
            "none".to_string()
        };
        // R3 (Abkarada-düzeltme-2): AMPİRİK ilk-dispatch metriği — sender'ın PROTOKOL-İÇİ cwnd-değişkeni
        // DEĞİL; faithful-wire'da stall-gap(1500ms)-öncesi gözlenen ilk-dispatch miktarı. W-1'in "cwnd"iyle
        // KARIŞTIRILMAMALI. Alan-adı: empirical_initial_dispatch_bytes (eski effective_initial_cwnd_bytes yerine).
        let total_workload = cfg.w_units * cfg.unit_bytes;
        let eff_dispatch = w2b_ff_burst_units * cfg.unit_bytes;
        let ratio = if eff_dispatch > 0 {
            format!("{:.3}", total_workload as f64 / eff_dispatch as f64)
        } else {
            "inf".to_string()
        };
        marker.push_str(&format!(
            "w2b_clock_domain=CLOCK_MONOTONIC-single-host\nw2b_feature_state={feature_state}\nw2b_workload_units={}\nw2b_unit_bytes={}\nw2b_total_workload_bytes={total_workload}\nw2b_empirical_initial_dispatch_bytes={eff_dispatch}\nw2b_first_flight_burst_units={}\nw2b_workload_to_effective_dispatch_ratio={ratio}\nw2b_completion_trigger_kind={completion_trigger_kind}\nw2b_completion_time={completion_time}\nw2b_first_recovery_event_kind={first_recovery_kind}\nw2b_first_recovery_event_time={first_recovery_time}\nw2b_recovery_events_before_completion_count={}\nw2b_recovery_contributed_to_completion={recovery_contributed}\nw2b_recovery_path={recovery_path}\nw2b_suppression_start={}\nw2b_suppressed_ack_count={w2b_suppressed_ack_count}\nw2b_ack_cap_hard_hit={w2b_hard_cap_hit}\nw2b_transport_duplicate_rx_count={w2b_transport_dup_rx}\nw2b_application_delivery_count={w2b_app_delivery}\nw2b_application_duplicate_delivery_count={w2b_app_dup_delivery}\nw2b_retransmit_rx_first={retransmit_first}\nw2b_retransmit_rx_count={w2b_retransmit_rx_count}\nw2b_suppression_release={}\nw2b_released_ack_wire={}\n",
            cfg.w_units,
            cfg.unit_bytes,
            w2b_ff_burst_units,
            w2b_recovery_events_before_completion,
            w2b_suppression_start.map_or_else(|| "none".to_string(), |v| format!("{v}ms")),
            w2b_release_time.map_or_else(|| "none".to_string(), |v| format!("{v}ms")),
            w2b_released_ack_wire.map_or_else(|| "none".to_string(), |v| format!("{v}ms")),
        ));
    }
    std::fs::write(
        format!("{}/{}-rxdone.marker", cfg.out_dir, cfg.run_id),
        marker,
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// TX-tarafı: GERÇEK-Sender (budget-path) + gerçek-zaman-event-döngüsü. Ledger+özet-CSV yazar.
#[allow(clippy::too_many_lines)]
pub fn run_tx(cfg: &RunnerCfg, deadline: Duration) -> Result<(), String> {
    let sock = UdpSocket::bind(&cfg.bind).map_err(|e| e.to_string())?;
    sock.connect(&cfg.peer).map_err(|e| e.to_string())?;
    sock.set_read_timeout(Some(Duration::from_millis(1))).ok();
    let mut s = Sender::new();
    if cfg.policy != "off" {
        s.set_manual_fec_enabled(true).map_err(|e| e.to_string())?;
        s.set_budget_policy(cfg.to_policy()?)
            .map_err(|e| format!("{e:?}"))?;
    }
    let start = Instant::now();
    let now_ns = |st: &Instant| -> u64 { st.elapsed().as_nanos() as u64 };
    let mut app = 0u64;
    let mut emitted_in_block: BTreeMap<u32, u64> = BTreeMap::new();
    let mut open_ms: BTreeMap<u32, u64> = BTreeMap::new();
    let mut close_ms: BTreeMap<u32, u64> = BTreeMap::new();
    let mut par_emit: BTreeMap<u32, u64> = BTreeMap::new();
    let mut acked_seen: BTreeSet<u64> = BTreeSet::new();
    let mut buf = [0u8; 4096];
    let mut last_tick = Instant::now();
    let base = 1_000_000u64;
    // FC: canonical fuzzy-event dataset (additive; existing-diag-poll; decision-DEĞİŞMEZ).
    let mut fuzzy_rows = String::new();
    let mut last_ev_idx = u64::MAX;
    let mut settle_since: Option<u64> = None; // FK: workload+parity-drained-settle takibi
    #[cfg(feature = "experimental-cold-start")]
    let mut cs_terminal_log: Vec<String> = Vec::new(); // FC-closure: M5-terminal-drain (INV-CS-3)
                                                       // ── TUR-A1 W-2B tx-tarafı ledger izleyicileri (harness-gözlem; sender-diag'lardan türev) ──
    #[cfg(feature = "experimental-cold-start")]
    let mut w2b_probe_wire: Option<u64> = None; // ilk CsProbe wire-emission (t_ms)
    #[cfg(feature = "experimental-cold-start")]
    let mut w2b_echo_rx: Option<u64> = None; // ilk echo (0x1A) kabul (t_ms)
    #[cfg(feature = "experimental-cold-start")]
    let mut w2b_bootstrap_srtt_time: Option<u64> = None; // srtt_source ilk Some(ProbeEcho) olduğu iterasyon
    #[cfg(feature = "experimental-cold-start")]
    let mut w2b_pto_wire: Vec<(u64, u64)> = Vec::new(); // (t_ms, unit_id-base-göreli) Retransmit (§9-ARQ)
    #[cfg(feature = "experimental-cold-start")]
    let mut w2b_dup_wire: Vec<(u64, u64)> = Vec::new(); // (t_ms, unit_id) Duplicate (§11 forward-recovery)
    #[cfg(feature = "experimental-cold-start")]
    let mut w2b_ack_before_bootstrap: u64 = 0; // bootstrap-ÖNCESİ kabul-edilen ACK (W-2B'de 0 beklenir)
    while start.elapsed() < deadline {
        let t_ms = start.elapsed().as_millis() as u64;
        // DR-CS-1 M1 ADAPTER (EK5-ii): saat iterasyon-başına BİR-KEZ örneklenir; bu-iterasyonun
        // TÜM inbound'u aynı-damgayla TEK on_event_batch çağrısına gider (flag-ON yolu).
        #[cfg(feature = "experimental-cold-start")]
        let mut cs_adapter = CsBatchAdapter::new(now_ns(&start));
        // FC: fuzzy-event-yakalama (event_index-ilerlediğinde tek-satır; salt-gözlem).
        if cfg.emit_fuzzy {
            if let Some(fi) = s.diag_fusion_input_snapshot() {
                if fi.event_index != last_ev_idx {
                    last_ev_idx = fi.event_index;
                    let dec = s.last_decision();
                    let (_st, btlbw, srtt_us, rate, inflight) = s.diag_rate();
                    fuzzy_rows.push_str(&format!(
                        "{},{},{},{},{},{},{},{},{},{},{},{},{},{},{:?},{}\n",
                        cfg.run_id,
                        cfg.policy,
                        fi.event_index,
                        t_ms,
                        fi.loss_windowed_ppm,
                        fi.qd_ratio_permille,
                        fi.jitter_ppm,
                        fi.ack_confidence_permille,
                        fi.bandwidth_bps,
                        btlbw,
                        srtt_us,
                        rate,
                        inflight,
                        dec.fec_parity_ratio_permille,
                        dec.mode,
                        u8::from(fi.no_rule)
                    ));
                }
            }
        }
        // app + tick (inline):
        if app < cfg.w_units && t_ms >= app * 40 {
            let ev = Event::AppData(OpaqueUnit {
                id: base + app,
                size: cfg.unit_bytes,
            });
            app += 1;
            #[cfg(feature = "experimental-cold-start")]
            cs_adapter.push(ev);
            #[cfg(not(feature = "experimental-cold-start"))]
            {
                let out = s.on_event(ev, now_ns(&start));
                emit(
                    &sock,
                    &out,
                    &mut emitted_in_block,
                    &mut open_ms,
                    &mut par_emit,
                    t_ms,
                    base,
                );
            }
        }
        if last_tick.elapsed() >= Duration::from_millis(10) {
            #[cfg(feature = "experimental-cold-start")]
            cs_adapter.push(Event::TimerFired(Timer::Pto));
            #[cfg(not(feature = "experimental-cold-start"))]
            {
                let out = s.on_event(Event::TimerFired(Timer::Pto), now_ns(&start));
                emit(
                    &sock,
                    &out,
                    &mut emitted_in_block,
                    &mut open_ms,
                    &mut par_emit,
                    t_ms,
                    base,
                );
            }
            last_tick = Instant::now();
        }
        // Blok-tamam → otorite-bağlı-close (gerçek-budget-yolu).
        let ready: Vec<u32> = emitted_in_block
            .iter()
            .filter(|&(_, &c)| c >= K)
            .map(|(&b, _)| b)
            .collect();
        for b in ready {
            emitted_in_block.remove(&b);
            let a = s.open_fec_block_authority();
            let _ = s.close_fec_block_manual(a, b, cfg.fixed_r, cfg.unit_bytes);
            close_ms.insert(b, t_ms);
        }
        // Geri-yol: SACK/RECOVERED.
        while let Ok(n) = sock.recv(&mut buf) {
            if n < 3 {
                continue;
            }
            // DR-CS-1 F2B: CS-echo (0x1A) → Event::CsProbeEcho (yalnız flag-ON; adapter-batch).
            #[cfg(feature = "experimental-cold-start")]
            if n >= 4 && buf[0] == 0x1A {
                let pseq = u16::from_be_bytes(buf[2..4].try_into().unwrap());
                if cfg.suppress_data_acks {
                    w2b_echo_rx.get_or_insert(t_ms); // W-2B: ilk echo kabul (bootstrap-kaynağı ProbeEcho)
                }
                cs_adapter.push(Event::CsProbeEcho { probe_seq: pseq });
                continue;
            }
            let cnt = u16::from_be_bytes(buf[1..3].try_into().unwrap()) as usize;
            if buf[0] == 0xA0 {
                // TUR-A1 W-2B: bootstrap-ÖNCESİ SACK kabul edildiyse say (assert=0 beklenir; rx-suppress
                // gereği tx bootstrap-öncesi hiç-SACK-almamalı → srtt_source=ProbeEcho garantisi).
                #[cfg(feature = "experimental-cold-start")]
                if cfg.suppress_data_acks && w2b_bootstrap_srtt_time.is_none() {
                    w2b_ack_before_bootstrap += 1;
                }
                let mut new = Vec::new();
                for i in 0..cnt.min((n - 3) / 8) {
                    let q = u64::from_be_bytes(buf[3 + i * 8..11 + i * 8].try_into().unwrap());
                    if acked_seen.insert(q) {
                        new.push(q);
                    }
                }
                if !new.is_empty() {
                    let ev = Event::AckReceived(AckInfo {
                        acked_seqs: new,
                        ack_delay: Duration::ZERO,
                    });
                    #[cfg(feature = "experimental-cold-start")]
                    cs_adapter.push(ev);
                    #[cfg(not(feature = "experimental-cold-start"))]
                    {
                        let out = s.on_event(ev, now_ns(&start));
                        emit(
                            &sock,
                            &out,
                            &mut emitted_in_block,
                            &mut open_ms,
                            &mut par_emit,
                            t_ms,
                            base,
                        );
                    }
                }
            } else if buf[0] == 0xA1 {
                let mut units = Vec::new();
                for i in 0..cnt.min((n - 3) / 6) {
                    let b = u32::from_be_bytes(buf[3 + i * 6..7 + i * 6].try_into().unwrap());
                    let e = u16::from_be_bytes(buf[7 + i * 6..9 + i * 6].try_into().unwrap());
                    units.push(RecoveredUnit {
                        unit_id: base + u64::from(b) * K + u64::from(e),
                        block_id: b,
                        esi: e,
                    });
                }
                let ev = Event::RecoveredControl(RecoveredInfo { units });
                #[cfg(feature = "experimental-cold-start")]
                cs_adapter.push(ev);
                #[cfg(not(feature = "experimental-cold-start"))]
                {
                    s.on_event(ev, now_ns(&start));
                }
            }
        }
        // DR-CS-1 M1 FLUSH (EK5-E1): iterasyonun tüm-inbound'u TEK atomik batch'te; sonrasında
        // hâlâ-due-varsa boş-batch (Tick). Flag-OFF'ta derlenmez (legacy per-event yol yukarıda).
        #[cfg(feature = "experimental-cold-start")]
        {
            let (bn, bev) = cs_adapter.drain();
            if !bev.is_empty() {
                let out = s.on_event_batch(bn, bev);
                if cfg.suppress_data_acks {
                    w2b_scan_tx(
                        &out,
                        t_ms,
                        base,
                        &mut w2b_probe_wire,
                        &mut w2b_pto_wire,
                        &mut w2b_dup_wire,
                    );
                }
                emit(
                    &sock,
                    &out,
                    &mut emitted_in_block,
                    &mut open_ms,
                    &mut par_emit,
                    t_ms,
                    base,
                );
            }
            let now2 = now_ns(&start);
            if s.next_wakeup_ns().is_some_and(|w| w <= now2) {
                let out = s.on_event_batch(now2, Vec::new()); // Tick
                if cfg.suppress_data_acks {
                    w2b_scan_tx(
                        &out,
                        t_ms,
                        base,
                        &mut w2b_probe_wire,
                        &mut w2b_pto_wire,
                        &mut w2b_dup_wire,
                    );
                }
                emit(
                    &sock,
                    &out,
                    &mut emitted_in_block,
                    &mut open_ms,
                    &mut par_emit,
                    t_ms,
                    base,
                );
            }
            // TUR-A1 W-2B: bootstrap_srtt_time — srtt_source ilk Some(ProbeEcho) olduğu an (diag-türev).
            if cfg.suppress_data_acks && w2b_bootstrap_srtt_time.is_none() {
                let (src, _, _, _) = s.diag_cs_bootstrap();
                if src.is_some() {
                    w2b_bootstrap_srtt_time = Some(t_ms);
                }
            }
            // FC-closure: M5-terminal drain (INV-CS-3; take_cs_terminal tek-tüketim). Son-terminal
            // marker'a (ResolvedByDataAck/ProbeEcho/TimedOut/PostTimeoutRecovery).
            if let Some(term) = s.take_cs_terminal() {
                cs_terminal_log.push(format!("{term:?}@{t_ms}ms"));
            }
        }
        std::thread::sleep(Duration::from_millis(2));
        // FK: tx-loop-süresi op_horizon'DAN-AYRI (op_horizon=rx-recovery-deadline). tx workload-sent
        // + parity/ARQ-drain (enqueued==emitted) + 4s-idle-settle (round-trip'ler-tamamlansın) sonra-
        // biter; ext_horizon-cap. (Önceki-bug: tx op_horizon'da-erken-çıkıp parity-emit-etmiyordu.)
        let dfa = s.diag_fec_actuation();
        // workload_done = app-enqueue-bitti VE tüm-bloklar-KAPANDI (=tüm-source DISPATCH-edildi). İkincisi
        // şart: yüksek-RTT'de dispatch ACK-clocked → enqueue'dan-geri-kalır; yalnız-enqueue'ya-baksak
        // settle-erken-tetiklenir + dispatch-edilmemiş-bloklar-hiç-ulaşmaz (FR-bug: RTT1s'te 18/30-blok-kayıp).
        let all_blocks_closed = (close_ms.len() as u64) >= cfg.w_units / K;
        let workload_done = app >= cfg.w_units && all_blocks_closed;
        let parity_drained = dfa.enqueued_r_total == dfa.emitted_r_total;
        // FC-closure (DI-2B faithfulness): armed-VE-henüz-terminal-OLMAYAN cold-start-epizodu = BEKLEYEN
        // sender-işi. settle-exit'i bloklar → sender-tarafı give-up-terminal'i (60s) HER payload-boyutunda
        // gözlemlenebilir. Aksi: küçük-workload (65B tüm-pencere ilk-cwnd'e-sığar → workload_done@t0) blackout-
        // altında give-up'tan-ÖNCE settle-çıkar (terminals=none artefaktı). Bu HARNESS-gözlem-kapısı; mekanizma
        // (give-up-timer) DEĞİŞMEZ; flag-OFF-binary byte-özdeş (cfg-gated).
        #[cfg(feature = "experimental-cold-start")]
        let cs_pending = {
            let (_, _, _, _, armed, disarmed) = s.diag_cold_start();
            armed && !disarmed
        };
        #[cfg(not(feature = "experimental-cold-start"))]
        let cs_pending = false;
        // TUR-A1: --no-settle-exit → settle-exit TAMAMEN kapalı (W-3-65: post-blackout heal
        // OBSERVATION_HORIZON=120s'e-kadar gözlenmeli; küçük-workload settle-erken-çıkışı YASAK).
        if workload_done && parity_drained && !cs_pending && !cfg.no_settle_exit {
            settle_since = settle_since.or(Some(t_ms));
        } else {
            settle_since = None;
        }
        if let Some(sd) = settle_since {
            // settle_ms: workload+parity-drain-SONRASI SACK/ARQ-round-trip'lere-verilen-süre. Yüksek-RTT'de
            // ARQ-repair ~1.5×RTT → settle ≥ birkaç-RTT olmalı (faithful-harness). Default 4000 (loopback).
            if t_ms.saturating_sub(sd) > cfg.tx_settle_ms {
                break;
            }
        }
    }
    // FA: WORKLOAD-tamam → FINISH-marker (0xFE) tekrar-gönder + rx-ACK-bekle (SIGTERM-yerine-graceful).
    let fin_start = Instant::now();
    let mut fin_acked = false;
    while fin_start.elapsed() < Duration::from_secs(3) {
        let _ = sock.send(&[0xFEu8; 1]);
        if let Ok(n) = sock.recv(&mut buf) {
            if n >= 1 && buf[0] == 0xFE {
                fin_acked = true;
                // Birkaç-kez-daha-gönder (rx ≥3-FINISH-görsün) sonra-çık.
                for _ in 0..5 {
                    let _ = sock.send(&[0xFEu8; 1]);
                    std::thread::sleep(Duration::from_millis(20));
                }
                break;
            }
        }
        std::thread::sleep(Duration::from_millis(30));
    }
    let _ = fin_acked; // best-effort; rx idle-fallback'i de temiz-finalize-eder
                       // Çıktılar.
    std::fs::create_dir_all(&cfg.out_dir).map_err(|e| e.to_string())?;
    if cfg.emit_fuzzy && !fuzzy_rows.is_empty() {
        let hdr = "run_id,policy,event_index,t_ms,loss_windowed_ppm,qd_ratio_permille,jitter_ppm,ack_confidence_permille,bandwidth_bps,btlbw,srtt_us,pacing_rate,inflight_bytes,fuzzy_ratio_permille,mode,no_rule\n";
        std::fs::write(
            format!("{}/{}-fuzzy.csv", cfg.out_dir, cfg.run_id),
            format!("{hdr}{fuzzy_rows}"),
        )
        .map_err(|e| e.to_string())?;
    }
    let d = s.diag_fec_actuation();
    let b = s.diag_budget();
    let mut lf = std::fs::File::create(format!("{}/{}-ledger.csv", cfg.out_dir, cfg.run_id))
        .map_err(|e| e.to_string())?;
    writeln!(lf, "run_id,block,open_ms,close_ms,par_emit").ok();
    for (blk, o) in &open_ms {
        writeln!(
            lf,
            "{},{blk},{o},{},{}",
            cfg.run_id,
            close_ms.get(blk).copied().unwrap_or(0),
            par_emit.get(blk).copied().unwrap_or(0)
        )
        .ok();
    }
    let mut sf = std::fs::File::create(format!("{}/{}-summary.csv", cfg.out_dir, cfg.run_id))
        .map_err(|e| e.to_string())?;
    writeln!(
        sf,
        "run_id,config_hash,policy,enq,emit,canc,waste,ndisp,rdisp,tk_adm,tk_on,tk_late,tk_miss"
    )
    .ok();
    writeln!(
        sf,
        "{},{:x},{},{},{},{},{},{},{},{},{},{},{}",
        cfg.run_id,
        cfg.config_hash(),
        cfg.policy,
        d.enqueued_r_total,
        d.emitted_r_total,
        d.canceled_r_total,
        b.reservation_waste_total,
        b.new_data_displaced,
        b.retx_displaced,
        b.tk_admitted,
        b.tk_on_time,
        b.tk_late,
        b.tk_missed_unique
    )
    .ok();
    std::fs::write(
        format!("{}/{}-config.txt", cfg.out_dir, cfg.run_id),
        format!("{}\nhash={:x}\n", cfg.canonical_dump(), cfg.config_hash()),
    )
    .ok();
    // FC-closure: tx-side cold-start terminal-log (M5; INV-CS-3 sonuç-sınıfları).
    #[cfg(feature = "experimental-cold-start")]
    {
        let (srtt_src, _, induced, _timed) = s.diag_cs_bootstrap();
        let (emitted, tx_bytes, wrapped, _pend, armed, disarmed) = s.diag_cold_start();
        let terminals = if cs_terminal_log.is_empty() {
            "none".to_string()
        } else {
            cs_terminal_log.join(",")
        };
        let mut body = format!(
            "cs_probe_emitted={emitted}\ncs_probe_tx_bytes={tx_bytes}\ncs_probe_seq_wrapped={wrapped}\ncs_armed={armed}\ncs_disarmed={disarmed}\ncs_terminals={terminals}\nbootstrap_induced_retransmit_bytes={induced}\n"
        );
        // TUR-A1 W-2B tx-tarafı ledger (25-alan'ın tx-alanları; harness-gözlem + sender-diag türev).
        // retransmit_path=ExistingPtoArq + new_retransmit_path_count=0 yapısaldır: harness YALNIZ mevcut
        // TransmitKind::Retransmit (§9-ARQ) yolunu emit eder; yeni-retransmit-yolu KODDA YOKTUR.
        if cfg.suppress_data_acks {
            let fmt_list = |v: &[(u64, u64)]| -> String {
                if v.is_empty() {
                    "none".to_string()
                } else {
                    v.iter()
                        .map(|(t, u)| format!("{t}ms#unit{u}"))
                        .collect::<Vec<_>>()
                        .join(";")
                }
            };
            let pto_list = fmt_list(&w2b_pto_wire);
            let dup_list = fmt_list(&w2b_dup_wire);
            let srtt_source = match srtt_src {
                Some(crate::sender::CsSrttSource::ProbeEcho) => "ProbeEcho",
                Some(crate::sender::CsSrttSource::DataAck) => "DataAck",
                None => "none",
            };
            // retransmit_path GÖZLENEN (yapısal-varsayım-DEĞİL): re-send hangi-kind ile yapıldı ölç.
            // new_retransmit_path_count=0 yapısaldır (kodda YALNIZ §9-Retransmit + §11-Duplicate yolları var;
            // "yeni" bir retransmit-yolu YOK). Bu, talimat-varsayımı (ExistingPtoArq) ile GERÇEĞİ karşılaştırır.
            let retransmit_path = if !w2b_pto_wire.is_empty() {
                "ExistingPtoArq"
            } else if !w2b_dup_wire.is_empty() {
                "ForwardRecoveryDup"
            } else {
                "NoTxResendObserved"
            };
            body.push_str(&format!(
                "w2b_clock_domain=CLOCK_MONOTONIC-single-host\nw2b_probe_wire={}\nw2b_echo_rx={}\nw2b_bootstrap_srtt_time={}\nw2b_pto_wire={pto_list}\nw2b_dup_wire={dup_list}\nw2b_srtt_source={srtt_source}\nw2b_ack_accepted_before_bootstrap_count={w2b_ack_before_bootstrap}\nw2b_retransmit_path={retransmit_path}\nw2b_new_retransmit_path_count=0\nw2b_terminal_class={terminals}\n",
                w2b_probe_wire.map_or_else(|| "none".to_string(), |v| format!("{v}ms")),
                w2b_echo_rx.map_or_else(|| "none".to_string(), |v| format!("{v}ms")),
                w2b_bootstrap_srtt_time.map_or_else(|| "none".to_string(), |v| format!("{v}ms")),
            ));
        }
        std::fs::write(format!("{}/{}-cstx.txt", cfg.out_dir, cfg.run_id), body).ok();
    }
    Ok(())
}

fn emit(
    sock: &UdpSocket,
    out: &[crate::sender::TransmitDecision],
    eib: &mut BTreeMap<u32, u64>,
    open_ms: &mut BTreeMap<u32, u64>,
    par_emit: &mut BTreeMap<u32, u64>,
    t_ms: u64,
    base: u64,
) {
    for d in out {
        // DR-CS-1 M2 (F2B; AMENDMAN-onaylı-codepoint): CS-probe — otoritatif §2.57-layout'un
        // harness-plaintext-temsili (kind=inner_kind=0x19; body: subtype+probe_seq+pad; 44B =
        // PROBE_WIRE_EXACT boyut-sınıfı; AEAD/outer-yeri sıfır-pad). Üretim-sealed-yolu noise-
        // wire-codec'te (dr_cs1_wire.rs-kanıtlı); harness alan-düzenini-birebir-taşır.
        #[cfg(feature = "experimental-cold-start")]
        if d.kind == TransmitKind::CsProbe {
            let mut v = vec![0u8; d.size as usize]; // 44B
            v[0] = 0x19; // CTRL_CS_PROBE (DR-CS-1-amendman; eski-0x12-CANCELLED)
            v[1] = 0x01; // subtype: cold-start
            v[2..4].copy_from_slice(&(d.seq as u16).to_be_bytes()); // probe_seq
            let _ = sock.send(&v);
            continue;
        }
        let (kind, b, e, unit) = if d.kind == TransmitKind::FecParity {
            let h = d.send_record.expect("hint");
            let SrLogicalRef::Fec { block_id, esi } = h.logical_ref;
            *par_emit.entry(block_id).or_insert(0) += 1;
            (2u8, block_id, esi, d.unit_id)
        } else if d.unit_id >= base {
            let i = d.unit_id - base;
            let (b, e) = ((i / K) as u32, (i % K) as u16);
            if d.kind == TransmitKind::Data {
                *eib.entry(b).or_insert(0) += 1;
                open_ms.entry(b).or_insert(t_ms);
            }
            // FK: wire-kind — §9-ARQ-retransmit(0x01) ile §11-forward-recovery-duplication(0x03) AYRI
            // kodlanır (rx substratı-doğru-atfetsin: reaktif-ARQ vs ACK-bağımsız-proaktif-dup).
            let wk = match d.kind {
                TransmitKind::Data => 0x00u8,
                TransmitKind::Retransmit => 0x01,
                TransmitKind::Duplicate => 0x03,
                TransmitKind::FecParity => 0x02, // yukarıda-ele-alındı (ulaşılmaz)
                #[cfg(feature = "experimental-cold-start")]
                TransmitKind::CsProbe => 0x19, // yukarıda-ele-alındı (ulaşılmaz)
            };
            (wk, b, e, d.unit_id)
        } else {
            (0xFDu8, u32::MAX, u16::MAX, d.unit_id)
        };
        let _ = sock.send(&enc(kind, d.seq, unit, b, e, d.size as usize));
    }
}
