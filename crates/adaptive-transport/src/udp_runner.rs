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
    let mut repair_arrival: BTreeMap<u32, u64> = BTreeMap::new(); // FR: substrat-onarım-arrival-ms (ARQ-repair
                                                                  // wall-clock; T_fec_decode<deadline<T_arq testi)
    while start.elapsed() < deadline {
        // FA: idle-erken-çıkış YALNIZ-FALLBACK (FINISH-marker gelmezse); normal-yol=FINISH-marker.
        if seen_any && last_rx.elapsed() > Duration::from_secs(8) {
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
                    src_have.entry(b).or_default().insert(e); // 0x00/0x01/0x03 = source-shard
                }
                let src_n = src_have.get(&b).map_or(0, BTreeSet::len);
                let par_n = par_have.get(&b).map_or(0, BTreeSet::len);
                let src_full = src_n >= K as usize; // tüm-source geldi (substrat/onarım tamamladı)
                if src_n + par_n >= K as usize {
                    let was_new = !block_complete.contains_key(&b);
                    block_complete.entry(b).or_insert(now_ms);
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
                let _ = sock.send_to(&v, p);
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
    let marker = format!(
        "run_id={}\npolicy={}\nreceiver_events={ev_count}\nfinalized_blocks={finalized}\npending_blocks={pending}\nledger_path={lpath}\nledger_checksum={ck:x}\nclean_shutdown={clean}\n",
        cfg.run_id, cfg.policy
    );
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
                size: UNIT as u64,
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
            let _ = s.close_fec_block_manual(a, b, cfg.fixed_r, UNIT as u64);
            close_ms.insert(b, t_ms);
        }
        // Geri-yol: SACK/RECOVERED.
        while let Ok(n) = sock.recv(&mut buf) {
            if n < 3 {
                continue;
            }
            let cnt = u16::from_be_bytes(buf[1..3].try_into().unwrap()) as usize;
            if buf[0] == 0xA0 {
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
        if workload_done && parity_drained {
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
            };
            (wk, b, e, d.unit_id)
        } else {
            (0xFDu8, u32::MAX, u16::MAX, d.unit_id)
        };
        let _ = sock.send(&enc(kind, d.seq, unit, b, e, d.size as usize));
    }
}
