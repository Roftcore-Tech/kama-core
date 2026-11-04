//! K4G P0 · MOTOR şeridi — tabanda (main `3439fccd`) bulguları gösteren belirlenimli testler.
//!
//! DÜZELTME YOK. Dayanak: CEVAP PAKETİ 9 (2026-10-24) §2 rev4 tablosu ve §4 P0 listesi; dış
//! inceleme (2026-10-23) §3, §5, §6.1-6.4; bağımsız doğrulama `verify-wf_f84f4d0e.json`
//! anahtarları D1, ESI, BURST, RTT, KAYNAK. Kural (C9): düzeltme yalnız tabanda kırmızı test
//! varsa yazılır; yanlışlanan madde nota "yanlışlandı: <test>" diye girer.
//!
//! ```text
//! test                                         bulgu    tabanda   ana atıf
//! p0_d1_orphan_after_probe_budget              D1       KIRMIZI   sender.rs:5794-5796, :5822, :3166
//! p0_esi_parity_ack_cancels_unrelated_dup      ESI      KIRMIZI   sender.rs:3027, :3043, :3153-3155
//! p0_esi_probe_targets_parity                  ESI      KIRMIZI   sender.rs:3227-3238, :5764-5803
//! p0_burst_cap_clamp_starves_recovery          BURST    KIRMIZI   sender.rs:2310-2325, pacer.rs:70-73
//! p0_class_share_saturated_data_starves_copy   PAY      KIRMIZI   sender.rs:4229-4251, :5932-5948
//! p0_stale_decision_after_confirm              KAYNAK   KIRMIZI   sender.rs:3371-3444, :5632-5684
//! p0_rtt_path_change_converges                 RTT      YEŞİL     sender.rs:3010-3150, rtt.rs:142-169
//! ```
//!
//! Her kırmızı test iki aşamada düşer: önce `FİKSTÜR` iddiaları (senaryo kusuru gerçekten kurdu
//! mu), sonra kusur iddiası. "FİKSTÜR GEÇERSİZ" ile düşen test kusuru SINAMAMIŞTIR; o sonuç
//! kırmızı sayılmaz, fikstür düzeltilir.
//!
//! Ürün yolu: `product_sender`, `ClientSession::new` (client-core/src/lib.rs:89-92) ve
//! `client_task` kurulumunu (client-core/src/runtime.rs:1290-1351) Sender API'siyle aynen uygular;
//! `adaptive-transport` client-core'a bağımlı olamaz, her adımın kaynağı yorumdadır. Dosyada cargo
//! feature kapısı yoktur; ürün `adaptive-transport`'u varsayılan özelliklerle derler
//! (client-core/Cargo.toml:24), bu testler de varsayılan derlemede koşar. Alıcı tarafı gerçek
//! `Receiver`'dır (quota 4: runtime.rs:257, :2486-2488; 20 ms ACK zamanlayıcısı: runtime.rs:69,
//! :2740-2769); ACK'ler ürün gibi `acked_full` çıktısıyla diff'siz beslenir (runtime.rs:1723-1760).
//! Motor zamanı sahte saattir (ns sayacı); gerçek uyku yoktur.
//!
//! Koş: `CARGO_BUILD_JOBS=2 cargo test -p adaptive-transport --test k4g_p0_engine -- --nocapture --test-threads=1`

use adaptive_transport::ack::AckFrame;
use adaptive_transport::emergency::EmergencyLevel;
use adaptive_transport::pacer::DEFAULT_BURST_CAP;
use adaptive_transport::receiver::{Receiver, RxDatagram, RxKind};
use adaptive_transport::sender::{
    AckInfo, BootstrapRttConfig, Event, FecActuationMode, FecLossRiskConfig, FecStructurePolicy,
    InitialWindowConfig, LossConfirmHorizon, OpaqueUnit, Sender, SenderState, SrLogicalRef, Timer,
    TransmitDecision, TransmitKind,
};
use adaptive_transport::tail_recovery::TailRecoveryConfig;
use std::collections::{HashMap, HashSet, VecDeque};
use std::time::Duration;

const MS: u64 = 1_000_000;
const SEC: u64 = 1_000_000_000;
/// Ürün pompası: runtime.rs:65 `PACE_TICK` = 1 ms; :1903-1905 `Timer::Pto`.
const PACE_TICK_NS: u64 = MS;
/// runtime.rs:67 `RTPROP_TICK` = 200 ms; :1924 `Timer::RtpropProbe`.
const RTPROP_TICK_NS: u64 = 200 * MS;
/// Alıcı ACK zamanlayıcısı: runtime.rs:69 `ACK_TICK` = 20 ms; :2740-2769 `on_ack_timer`.
const ACK_TICK_NS: u64 = 20 * MS;
/// Alıcı epizot kotası: runtime.rs:257 `quota_r: 4`; :2486-2488 `set_quota_r`.
const RX_QUOTA_R: u32 = 4;
/// Yapı-gate seviyesi: runtime.rs:241 `STRUCTURE_GATE_LEVEL_PERMILLE`, :279.
const STRUCTURE_GATE_LEVEL_PERMILLE: u32 = 250;
/// Ürün FEC bloğu: profil 0x05 (runtime.rs:229) k_shape 8 (kama-fec-codec/src/profile.rs:86-90).
const FEC_K: u64 = 8;
/// Alıcıya giden FEC başlığındaki k_actual (tam blok).
const FEC_K_ACTUAL: u8 = 8;
/// Profil 0x05 r_code_max 4 (profile.rs:89) → ESI ∈ 0..4 (sender.rs:4806-4813).
const FEC_R: u16 = 4;
/// Tek yön yayılım (K4G-05b notu Ek B.2: "tek yön 200 ms").
const ONE_WAY_NS: u64 = 200 * MS;
/// SACK tel boyu (sender.rs:1440 `EAI_SACK_WIRE_BYTES`; receiver.rs:83).
const SACK_WIRE_BYTES: u64 = 65;
/// sender.rs:1479 `K_ARQ_SHARE` (özel sabit; değer koddan).
const K_ARQ_SHARE: usize = 4;
/// sender.rs:1507 `BURST_WINDOW_NS` (özel sabit; değer koddan).
const BURST_WINDOW_NS: u64 = 40 * MS;

// ════════════════════════════════════════════════════════════════════════════════════════════
// Ürün yapılandırmasının aynası
// ════════════════════════════════════════════════════════════════════════════════════════════

/// `ClientSession::new` + `client_task` kurulumunun Sender API'siyle aynası.
///
/// Sapma (beyanlı): runtime'ın FEC blok mandalı (runtime.rs:1849-1880) burada çağrılmaz. Controller
/// kipinde parite yalnız o mandalla kuyruğa girer; temiz yolda ürün de r=0 planlar (runtime.rs:
/// 269-270). Parite kolu ESI testlerinde `product_sender_static_r` ile ayrıca sürülür.
fn product_sender() -> Sender {
    // client-core/src/lib.rs:89-92 — ClientSession::new: Sender::new + validated_v1 kuyruk-probu.
    let mut s = Sender::new();
    s.set_tail_recovery_config(TailRecoveryConfig::validated_v1());
    // runtime.rs:1293-1295 — cfg.fec_structure_policy = FecStructurePolicy::default() (:263).
    s.set_fec_structure_policy(FecStructurePolicy::default());
    // runtime.rs:1298-1310 — cfg.bootstrap_rtt = false (:261) → BootstrapRttConfig::default().
    s.set_bootstrap_rtt(BootstrapRttConfig::default());
    // runtime.rs:1313 — set_cs_probe_lane(cfg.bootstrap_rtt = false).
    s.set_cs_probe_lane(false);
    // runtime.rs:1315-1317 — LossConfirmHorizon::CadenceLegacy (:276).
    s.set_loss_confirm_horizon(LossConfirmHorizon::CadenceLegacy);
    // runtime.rs:1319 — FecLossRiskConfig::bounded_v1() (:277).
    s.set_fec_loss_risk(FecLossRiskConfig::bounded_v1());
    // runtime.rs:1321-1323 — Some(STRUCTURE_GATE_LEVEL_PERMILLE) (:279, :241).
    s.set_structure_gate_level(Some(STRUCTURE_GATE_LEVEL_PERMILLE));
    // runtime.rs:1326-1328 — fusion_policy None (:275) → setter çağrılmaz.
    // runtime.rs:1330-1334 — INITIAL_WINDOW_BYTES = pacer::DEFAULT_BURST_CAP (:237, :260).
    s.set_initial_window(InitialWindowConfig {
        bytes: DEFAULT_BURST_CAP,
    });
    // runtime.rs:1340-1351 — cfg.fec = Some(0x05, manual_r None) (:271-274) → Controller.
    let mode = s.set_fec_actuation_mode(FecActuationMode::Controller);
    assert!(
        mode.is_ok(),
        "taze Sender FEC kipini kabul etmeli (runtime.rs:1349-1351)"
    );
    assert!(
        s.tail_recovery_enabled(),
        "ürün kuyruk-probu açık olmalı (lib.rs:91)"
    );
    s
}

/// Ürün + statik-r FEC kolu: runtime.rs:1340-1344 `manual_r: Some(r)` → `Manual { enabled: true }`,
/// blok kapanışı runtime.rs:1849-1854 `close_fec_block_manual`. Parite emisyon noktası
/// (sender.rs:5236-5286) ve ACK yolları (sender.rs:3019-3043) Controller kipiyle ortaktır; fark
/// yalnız r'nin kim tarafından seçildiğidir (Controller kipinde parite PreConverge'de kapalıdır).
fn product_sender_static_r() -> Sender {
    let mut s = product_sender();
    let mode = s.set_fec_actuation_mode(FecActuationMode::Manual { enabled: true });
    assert!(mode.is_ok(), "statik-r kipi kabul edilmeli");
    s
}

/// §3.3-B ilk kopya gecikmesi: 3/2 × (boy + PC_SACK_WIRE) / PRE_CONVERGE_FLOOR_BW saniye
/// (sender.rs:6019-6023; sabitler :1429, :1500-1502).
fn preconverge_first_copy_delay_ns(size: u64) -> u64 {
    let rtt_floor = (size + 65) * SEC / 31;
    rtt_floor * 3 / 2
}

// ════════════════════════════════════════════════════════════════════════════════════════════
// Kayıt + runtime aynası
// ════════════════════════════════════════════════════════════════════════════════════════════

/// Motorun ürettiği bir karar; `probe` = aynı olayda gerçek kuyruk-probu emisyonu oldu
/// (`tail_probe_diag_ext().1` arttı).
#[derive(Clone, Copy, Debug)]
struct Emit {
    t: u64,
    d: TransmitDecision,
    probe: bool,
}

fn carries_unit(k: TransmitKind) -> bool {
    matches!(
        k,
        TransmitKind::Data | TransmitKind::Retransmit | TransmitKind::Duplicate
    )
}

fn is_reemission(k: TransmitKind) -> bool {
    matches!(k, TransmitKind::Retransmit | TransmitKind::Duplicate)
}

/// Motoru tek olayla sürer, kararları sırayla kaydeder (runtime.rs:1455-1460 `drive!` aynası).
fn drive_sender(s: &mut Sender, ev: Event, now: u64, log: &mut Vec<Emit>) -> Vec<TransmitDecision> {
    let probes_before = s.tail_probe_diag_ext().1;
    let out = s.on_event(ev, now);
    let probe = s.tail_probe_diag_ext().1 > probes_before;
    log.extend(out.iter().map(|&d| Emit { t: now, d, probe }));
    out
}

fn data_seq(log: &[Emit], unit: u64) -> Option<u64> {
    log.iter()
        .find(|e| e.d.kind == TransmitKind::Data && e.d.unit_id == unit)
        .map(|e| e.d.seq)
}

fn fmt_tx(list: &[Emit]) -> String {
    list.iter()
        .map(|e| {
            format!(
                "{}ms:{:?}#{}{}",
                e.t / MS,
                e.d.kind,
                e.d.seq,
                if e.probe { "(prob)" } else { "" }
            )
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// client-core runtime'ının birim deposu ve seq→birim tablolarının AYNASI (salt test muhasebesi):
/// depo kaydı runtime.rs:1836, `carries_unit` eşlemesi :1461-1471, store-miss = bayat karar
/// :1543-1548, ACK budaması yalnız `store.remove` başarılıysa :1736-1743.
#[derive(Default)]
struct RuntimeMirror {
    store: HashSet<u64>,
    seq_to_unit: HashMap<u64, u64>,
    unit_seqs: HashMap<u64, Vec<u64>>,
    stale_decisions: u64,
}

impl RuntimeMirror {
    fn offer(&mut self, unit: u64) {
        self.store.insert(unit);
    }

    fn on_decision(&mut self, d: &TransmitDecision) {
        if !carries_unit(d.kind) {
            return;
        }
        self.seq_to_unit.insert(d.seq, d.unit_id);
        self.unit_seqs.entry(d.unit_id).or_default().push(d.seq);
        if !self.store.contains(&d.unit_id) {
            self.stale_decisions += 1;
        }
    }

    fn on_ack(&mut self, acked: &[u64]) {
        for s in acked {
            let Some(&unit) = self.seq_to_unit.get(s) else {
                continue;
            };
            if self.store.remove(&unit) {
                for old in self.unit_seqs.remove(&unit).unwrap_or_default() {
                    self.seq_to_unit.remove(&old);
                }
            }
        }
    }

    fn has_payload(&self, unit: u64) -> bool {
        self.store.contains(&unit)
    }

    fn entries_for(&self, unit: u64) -> usize {
        self.seq_to_unit.values().filter(|&&u| u == unit).count()
    }
}

// ════════════════════════════════════════════════════════════════════════════════════════════
// Yalnız-gönderen sürücü (ACK'ler elle; seq düzeyinde kol hedefleme)
// ════════════════════════════════════════════════════════════════════════════════════════════

/// Ağ yok; ACK çerçevesi testte kurulur. Gerçek alıcı yalnız teslim edilen seq'leri ACK'lerdi;
/// `acked_full` çıktısı burada verilen kümeyle aynıdır.
struct Drv {
    s: Sender,
    now: u64,
    log: Vec<Emit>,
    rt: RuntimeMirror,
}

impl Drv {
    fn new(s: Sender) -> Self {
        Self {
            s,
            now: 0,
            log: Vec::new(),
            rt: RuntimeMirror::default(),
        }
    }

    fn ev(&mut self, ev: Event) -> Vec<TransmitDecision> {
        let out = drive_sender(&mut self.s, ev, self.now, &mut self.log);
        for d in &out {
            self.rt.on_decision(d);
        }
        out
    }

    fn app(&mut self, id: u64, size: u64) {
        self.rt.offer(id);
        self.ev(Event::AppData(OpaqueUnit { id, size }));
    }

    fn ack(&mut self, seqs: Vec<u64>) {
        self.rt.on_ack(&seqs);
        self.ev(Event::AckReceived(AckInfo {
            acked_seqs: seqs,
            ack_delay: Duration::ZERO,
        }));
    }

    /// `now`'ı `step` adımlarla `t_end`'e dek ilerletir; her adımda `Timer::Pto`.
    fn pump_to(&mut self, t_end: u64, step: u64) {
        while self.now + step <= t_end {
            self.now += step;
            self.ev(Event::TimerFired(Timer::Pto));
        }
    }

    fn data_seq(&self, unit: u64) -> Option<u64> {
        data_seq(&self.log, unit)
    }

    fn kind_seqs(&self, kind: TransmitKind) -> Vec<(u64, u64)> {
        self.log
            .iter()
            .filter(|e| e.d.kind == kind)
            .map(|e| (e.d.seq, e.d.unit_id))
            .collect()
    }
}

// ════════════════════════════════════════════════════════════════════════════════════════════
// Ağ düzeneği: iki yönlü yol + gerçek Receiver
// ════════════════════════════════════════════════════════════════════════════════════════════

/// Tek yönlü yol: gönderim anına bağlı yayılım gecikmesi + isteğe bağlı FIFO darboğaz.
struct Path {
    delay: fn(u64) -> u64,
    rate_bps: Option<u64>,
    free_at: u64,
}

impl Path {
    fn new(delay: fn(u64) -> u64, rate_bps: Option<u64>) -> Self {
        Self {
            delay,
            rate_bps,
            free_at: 0,
        }
    }

    fn arrival(&mut self, t: u64, size: u64) -> u64 {
        let depart = match self.rate_bps {
            Some(r) => {
                let start = self.free_at.max(t);
                self.free_at = start + size * SEC / r;
                self.free_at
            }
            None => t,
        };
        depart + (self.delay)(t)
    }
}

fn one_way_fixed(_t: u64) -> u64 {
    ONE_WAY_NS
}

fn insert_sorted<T>(q: &mut VecDeque<(u64, u64, T)>, arr: u64, order: u64, item: T) {
    let pos = q.partition_point(|e| (e.0, e.1) <= (arr, order));
    q.insert(pos, (arr, order, item));
}

type DropRule = Box<dyn FnMut(&TransmitDecision) -> bool>;

/// Ağ simülasyonu. Tik sırası: (1) ileri yol varışları kendi varış anlarıyla alıcıya, (2) alıcı
/// ACK zamanlayıcısı, (3) dönüş yolu varışları motora; ardından test uygulama olaylarını ve
/// `timers()` ile ürün pompasını sürer.
struct Sim {
    s: Sender,
    rx: Receiver,
    now: u64,
    step: u64,
    fwd: Path,
    ret: Path,
    fwd_q: VecDeque<(u64, u64, RxDatagram)>,
    ret_q: VecDeque<(u64, u64, AckFrame)>,
    order: u64,
    drop_fwd: DropRule,
    log: Vec<Emit>,
    max_seq: Option<u64>,
    max_acked: Option<u64>,
    last_ack_at: Option<u64>,
    rt: Option<RuntimeMirror>,
}

impl Sim {
    fn new(s: Sender, step: u64, fwd: Path, ret: Path, mirror: bool) -> Self {
        let mut rx = Receiver::new();
        rx.set_quota_r(RX_QUOTA_R);
        Self {
            s,
            rx,
            now: 0,
            step,
            fwd,
            ret,
            fwd_q: VecDeque::new(),
            ret_q: VecDeque::new(),
            order: 0,
            drop_fwd: Box::new(|_d: &TransmitDecision| false),
            log: Vec::new(),
            max_seq: None,
            max_acked: None,
            last_ack_at: None,
            rt: mirror.then(RuntimeMirror::default),
        }
    }

    fn drive(&mut self, ev: Event) {
        let out = drive_sender(&mut self.s, ev, self.now, &mut self.log);
        for d in out {
            self.emit(d);
        }
    }

    fn emit(&mut self, d: TransmitDecision) {
        if !carries_unit(d.kind) && d.kind != TransmitKind::FecParity {
            return; // CsProbe: motor seq'i değil; ürün yolunda kapalı (runtime.rs:1313).
        }
        self.max_seq = Some(self.max_seq.map_or(d.seq, |m| m.max(d.seq)));
        if let Some(rt) = self.rt.as_mut() {
            rt.on_decision(&d);
        }
        if (self.drop_fwd)(&d) {
            return;
        }
        // Alıcı paritesi FEC-shard olarak görür (runtime.rs:3026-3037); diğerleri DATA.
        let kind = match (d.kind, d.send_record) {
            (TransmitKind::FecParity, Some(h)) => {
                let SrLogicalRef::Fec { block_id, esi } = h.logical_ref;
                RxKind::FecShard {
                    block_id,
                    esi,
                    k_actual: FEC_K_ACTUAL,
                    is_repair: true,
                }
            }
            _ => RxKind::Data,
        };
        let arr = self.fwd.arrival(self.now, d.size);
        self.order += 1;
        let dg = RxDatagram {
            seq: d.seq,
            unit_id: d.unit_id,
            kind,
        };
        insert_sorted(&mut self.fwd_q, arr, self.order, dg);
    }

    fn app(&mut self, id: u64, size: u64) {
        if let Some(rt) = self.rt.as_mut() {
            rt.offer(id);
        }
        self.drive(Event::AppData(OpaqueUnit { id, size }));
    }

    /// Ürün pompası: her tikte `Timer::Pto` (runtime.rs:1903-1905), 200 ms'de bir
    /// `Timer::RtpropProbe` (runtime.rs:1924).
    fn timers(&mut self) {
        self.drive(Event::TimerFired(Timer::Pto));
        if self.now.is_multiple_of(RTPROP_TICK_NS) {
            self.drive(Event::TimerFired(Timer::RtpropProbe));
        }
    }

    fn send_ack(&mut self, a: AckFrame, t: u64) {
        let arr = self.ret.arrival(t, SACK_WIRE_BYTES);
        self.order += 1;
        insert_sorted(&mut self.ret_q, arr, self.order, a);
    }

    fn advance(&mut self) {
        self.now += self.step;
        let now = self.now;
        // (1) İleri yol varışları (runtime.rs:2933-2940, :3026-3037).
        while self.fwd_q.front().is_some_and(|e| e.0 <= now) {
            if let Some((arr, _, dg)) = self.fwd_q.pop_front() {
                let out = self.rx.on_datagram(dg, arr);
                if let Some(a) = out.ack {
                    self.send_ack(a, arr);
                }
            }
        }
        // (2) Alıcı ACK zamanlayıcısı (runtime.rs:2740-2769).
        if now.is_multiple_of(ACK_TICK_NS) {
            if let Some(a) = self.rx.on_ack_timer(now) {
                self.send_ack(a, now);
            }
        }
        // (3) Dönüş yolu varışları → motor: acked_full diff'siz + ack_delay (runtime.rs:1729-1760).
        while self.ret_q.front().is_some_and(|e| e.0 <= now) {
            if let Some((_, _, a)) = self.ret_q.pop_front() {
                let expected = self.max_seq.map_or(0, |m| m + 1);
                let acked = a.acked_full(expected);
                if let Some(&m) = acked.iter().max() {
                    self.max_acked = Some(self.max_acked.map_or(m, |x| x.max(m)));
                    self.last_ack_at = Some(now);
                }
                if let Some(rt) = self.rt.as_mut() {
                    rt.on_ack(&acked);
                }
                self.drive(Event::AckReceived(AckInfo {
                    acked_seqs: acked,
                    ack_delay: a.ack_delay_duration(),
                }));
            }
        }
    }
}

/// "Uygunluk" vekilleri: kamuya açık gözlemcilerle görülebilen sahipler.
#[derive(Debug)]
#[allow(dead_code)] // Alanlar yalnız `{:?}` raporunda okunur.
struct OwnerProbe {
    t_ms: u64,
    state: SenderState,
    emergency: EmergencyLevel,
    pending_lost: u64,
    probes_left: u64,
    next_wakeup_ms: Option<u64>,
    inflight_bytes: u64,
}

fn owner_probe(s: &Sender, now: u64, budget: u64) -> OwnerProbe {
    OwnerProbe {
        t_ms: now / MS,
        state: s.state(),
        emergency: s.emergency_level(),
        pending_lost: s.diag_loss().2,
        probes_left: budget.saturating_sub(s.tail_probe_diag_ext().1),
        next_wakeup_ms: s.next_wakeup_ns().map(|w| w / MS),
        inflight_bytes: s.inflight_bytes(),
    }
}

fn assert_fixture(test: &str, fixture: &[String]) {
    assert!(
        fixture.is_empty(),
        "{test}: FİKSTÜR GEÇERSİZ (kusur sınanmadı; sonuç kırmızı SAYILMAZ):\n{}",
        fixture.join("\n")
    );
}

// ════════════════════════════════════════════════════════════════════════════════════════════
// 1 · D1
// ════════════════════════════════════════════════════════════════════════════════════════════

/// D1 — prob bütçesi tükenince sahipsiz kalan Reliable birim. Tabanda beklenen: KIRMIZI.
///
/// Neden kırmızı (kod): kuyruk-probu eski kaydı emekliye ayırıp yenisini `Retransmit` kökenli izler
/// (sender.rs:5794-5796); validated_v1 bütçesi 3'tür ve tükenince zamanlayıcı `Idle` döner
/// (tail_recovery.rs:80, :221-224); bütçe yalnız uçuşu küçülten ACK ile yenilenir (sender.rs:
/// 3165-3172); zaman eşiği daha büyük bir onaylı seq ister (loss.rs:692); RECOVER_FWD sessizlik kolu
/// yalnız `Data` kökenli kaydı sahiplenir (sender.rs:5822). Gerçek alıcı sessiz dönemde de SACK
/// tekrarlar (receiver.rs:93-94 `ACK_CADENCE_FLOOR_NS` ≈ 8.39 s); bayat SACK `last_ack_ns`'i tazeler
/// (sender.rs:2971) ama uçuşu küçültmez, bu yüzden ack_confidence düşmez (sender.rs:6133-6150) ve
/// prob bütçesi de yenilenmez. Sonuç: 3. probdan sonra 600 s boyunca kurban yeniden gönderilmez.
///
/// Fikstür (C9 §2 rev4 D1): RTT önce kurulur (PreConverge çıkışı sender.rs:3926-3933); kurban,
/// son ısınma ACK'inin işlendiği tikte (taze ACK) en yüksek seq ile gönderilir; kurbanın bütün
/// iletimleri ileri yolda düşer, başka trafik yoktur, dolayısıyla kurbandan sonra ACK'lenen seq
/// yoktur; yeni uygulama verisi yoktur (kusur oturum düzeyindedir: başka birimin ACK'i bütçeyi
/// yenilerdi). Her kurban emisyonunda runtime aynasında payload mevcuttur. Pompa 10 ms (üründe
/// 1 ms): prob vadeleri ≥ 1 s (base PTO, tail_recovery.rs:155-165) olduğundan 10 ms çözünürlük
/// vadeyi en çok 10 ms kaydırır; 600 s × 1 ms = 6·10⁵ olay debug derlemede gereksiz süredir.
/// Uygunluk (gözlenebilir sahip vekilleri) ve emisyon (gerçek `TransmitDecision`) ayrı raporlanır;
/// kopya kuyruğu sahipliği kamu API'siyle gözlenemez (kanca önerisi raporda).
#[test]
fn p0_d1_orphan_after_probe_budget() {
    const TEST: &str = "p0_d1_orphan_after_probe_budget";
    const UNIT: u64 = 300;
    const WARM: u64 = 4;
    const STEP: u64 = 10 * MS;
    const HORIZON: u64 = 600 * SEC;
    let budget = u64::from(TailRecoveryConfig::validated_v1().max_tail_probes);
    // Ürün birim kimlikleri 0'dan ardışıktır (runtime.rs:1391, :1807-1808) → kurban = WARM.
    let victim: u64 = WARM;
    let mut sim = Sim::new(
        product_sender(),
        STEP,
        Path::new(one_way_fixed, None),
        Path::new(one_way_fixed, None),
        true,
    );
    sim.drop_fwd =
        Box::new(move |d: &TransmitDecision| d.unit_id == victim && carries_unit(d.kind));
    let mut fixture: Vec<String> = Vec::new();

    // Faz 1 — ısınma: RTT kurulur, uçuş boşalır; kurban taze ACK'in işlendiği tikte girer.
    for id in 0..WARM {
        sim.app(id, UNIT);
    }
    sim.timers();
    let mut t_inject: Option<u64> = None;
    while sim.now < 10 * SEC {
        sim.advance();
        let fresh_ack = sim.last_ack_at == Some(sim.now);
        if fresh_ack && sim.s.inflight_bytes() == 0 && sim.s.state() != SenderState::PreConverge {
            t_inject = Some(sim.now);
            sim.app(victim, UNIT);
            sim.timers();
            break;
        }
        sim.timers();
    }
    assert!(
        t_inject.is_some(),
        "{TEST}: FİKSTÜR GEÇERSİZ: 10 s içinde ısınma ACK'leri uçuşu boşaltmadı / PreConverge'den çıkılmadı"
    );
    let t0 = t_inject.unwrap_or_default();
    let srtt_at_inject = sim.s.path_timing_snapshot().srtt_ns;
    if srtt_at_inject.is_none() {
        fixture.push("kurban girerken srtt yok (RTT önce kurulmalı)".to_string());
    }
    let victim_seq = sim
        .log
        .iter()
        .find(|e| e.t == t0 && e.d.kind == TransmitKind::Data && e.d.unit_id == victim)
        .map(|e| e.d.seq);
    if victim_seq.is_none() {
        fixture.push(format!(
            "kurban taze ACK tikinde (t={} ms) gönderilmedi; 'taze ACK'ten hemen sonra' sağlanmadı",
            t0 / MS
        ));
    }
    if victim_seq.is_some_and(|vs| sim.log.iter().any(|e| e.d.seq > vs && e.t <= t0)) {
        fixture.push("kurban gönderildiği anda en yüksek seq değil".to_string());
    }

    // Faz 2 — 600 s; yeni uygulama verisi YOK.
    let end = t0 + HORIZON;
    let mut at_exhaust: Option<OwnerProbe> = None;
    while sim.now < end {
        sim.advance();
        sim.timers();
        if at_exhaust.is_none() && sim.s.tail_probe_diag_ext().1 >= budget {
            at_exhaust = Some(owner_probe(&sim.s, sim.now, budget));
        }
    }
    let at_end = owner_probe(&sim.s, sim.now, budget);

    let victim_tx: Vec<Emit> = sim
        .log
        .iter()
        .filter(|e| e.d.unit_id == victim && carries_unit(e.d.kind))
        .copied()
        .collect();
    let probes: Vec<Emit> = victim_tx
        .iter()
        .filter(|e| e.probe && e.d.kind == TransmitKind::Retransmit)
        .copied()
        .collect();
    let probes_sent = sim.s.tail_probe_diag_ext().1;
    if probes_sent != budget || probes.len() as u64 != budget {
        fixture.push(format!(
            "prob bütçesi tükenmedi: prob_sent={probes_sent}, kurban prob emisyonu={}, bütçe={budget}",
            probes.len()
        ));
    }
    let t_last_probe = probes.last().map_or(u64::MAX, |e| e.t);
    if let (Some(vs), Some(ma)) = (victim_seq, sim.max_acked) {
        if ma >= vs {
            fixture.push(format!(
                "kurbandan (seq {vs}) sonra ACK'lenen seq var (en büyük ACK'li seq {ma})"
            ));
        }
    }
    let (payload_ok, stale) = sim.rt.as_ref().map_or((false, u64::MAX), |r| {
        (r.has_payload(victim), r.stale_decisions)
    });
    if !payload_ok || stale != 0 {
        fixture.push(format!(
            "runtime aynasında kurban payload'u her emisyonda mevcut olmalı (mevcut={payload_ok}, store-miss={stale})"
        ));
    }
    let post: Vec<Emit> = victim_tx
        .iter()
        .filter(|e| e.t > t_last_probe)
        .copied()
        .collect();
    let first_post = post.first().map(|e| e.t.saturating_sub(t_last_probe) / MS);

    println!(
        "[P0-D1] kurban u{victim} t0={} ms srtt0={:?} ms; iletimler: {}",
        t0 / MS,
        srtt_at_inject.map(|v| v / MS),
        fmt_tx(&victim_tx)
    );
    println!(
        "[P0-D1] EMİSYON: 3. probdan sonra yeniden gönderim={} (ilki +{first_post:?} ms), ufuk 600 s",
        post.len()
    );
    println!("[P0-D1] UYGUNLUK (bütçe tükenince): {at_exhaust:?}");
    println!("[P0-D1] UYGUNLUK (600 s sonunda): {at_end:?}");
    println!(
        "[P0-D1] UYGUNLUK notu: kopya kuyruğu (dup_queue) sahipliği kamu API'siyle gözlenemez; \
         kanca önerisi Sender::diag_dup_queue_units() (salt-okuma). Kod okuması: kurbanı kopya \
         kuyruğuna ekleyen yol bu senaryoda yok (sender.rs:3393-3399, :5693-5697, :5822-5832, \
         :5843-5847, :6008-6012, :6024-6028)."
    );

    assert_fixture(TEST, &fixture);
    assert!(
        !post.is_empty(),
        "D1 KUSURU: prob bütçesi ({budget}) {} ms'de tükendi; sonraki {} s boyunca kurban u{victim} \
         HİÇ yeniden gönderilmedi. Son kaydı Retransmit kökenli (sender.rs:5796), RECOVER_FWD \
         sessizlik kolu yalnız Data kökenli kaydı sahiplenir (sender.rs:5822), later-ACK yok \
         (loss.rs:692), bütçe yalnız uçuşu küçülten ACK ile yenilenir (sender.rs:3166); bayat SACK \
         ilerleme değildir. INV-NO-ORPHAN ihlali.",
        t_last_probe / MS,
        end.saturating_sub(t_last_probe) / SEC
    );
}

// ════════════════════════════════════════════════════════════════════════════════════════════
// 2 · ESI — parite ACK'i ilgisiz veri biriminin kopya işini iptal eder
// ════════════════════════════════════════════════════════════════════════════════════════════

#[derive(Default)]
struct ArmOutcome {
    fixture: Vec<String>,
    defect: Vec<String>,
}

/// Uçuş kolu: blok 0'ın 8 kaynak birimi (kimlik 0..7) PreConverge'de gönderilir ve §3.3-B
/// geometrik kopya alır (sender.rs:6014-6028); kaynaklar kaybolur, parite (ESI 0..3) teslim edilip
/// UÇUŞTAN ACK'lenir. Parite RTT örneği vermez (sender.rs:3034), PreConverge sürer. Kimliği ESI ile
/// çakışan 0..3'ün kopyaları iptal edilir; 4..7 aynı koşuda kontroldür.
fn esi_inflight_arm() -> ArmOutcome {
    const UNIT: u64 = 100;
    let mut o = ArmOutcome::default();
    let mut d = Drv::new(product_sender_static_r());
    for id in 0..FEC_K {
        d.app(id, UNIT);
    }
    let auth = d.s.open_fec_block_authority();
    let closed = d.s.close_fec_block_manual(auth, 0, FEC_R, UNIT);
    if closed != Ok(FEC_R) {
        o.fixture.push(format!(
            "statik-r blok kapanışı {closed:?} döndü (beklenen Ok({FEC_R}))"
        ));
    }
    d.pump_to(MS, MS); // parite emisyonu: plan_fec_parity (sender.rs:5187-5232)
    let parity = d.kind_seqs(TransmitKind::FecParity);
    if parity.len() != usize::from(FEC_R) || parity.iter().map(|p| p.1).ne(0..u64::from(FEC_R)) {
        o.fixture.push(format!(
            "parite ESI 0..{FEC_R} olarak gönderilmedi: {parity:?}"
        ));
    }
    if (0..FEC_K).any(|id| d.data_seq(id).is_none()) {
        o.fixture
            .push("blok 0'ın 8 kaynak birimi t=0'da gönderilmedi (IW 2400 B)".to_string());
    }
    d.pump_to(400 * MS, 10 * MS);
    if d.s.state() != SenderState::PreConverge {
        o.fixture
            .push("parite ACK'inden önce PreConverge'de olunmalı".to_string());
    }
    // Kaynaklar kayıp, parite teslim edildi → ACK yalnız parite seq'lerini taşır.
    d.ack(parity.iter().map(|p| p.0).collect());
    if d.s.diag_rtt_sample_count() != 0 || d.s.state() != SenderState::PreConverge {
        o.fixture.push(format!(
            "parite ACK'i RTT örneği vermemeli ve PreConverge sürmeli (örnek={}, state={:?})",
            d.s.diag_rtt_sample_count(),
            d.s.state()
        ));
    }
    let t_ack = d.now;
    let horizon = t_ack + 2 * preconverge_first_copy_delay_ns(UNIT) + SEC;
    d.pump_to(horizon, 10 * MS);
    let dups = |id: u64| {
        d.log
            .iter()
            .filter(|e| e.t > t_ack && e.d.kind == TransmitKind::Duplicate && e.d.unit_id == id)
            .count()
    };
    let colliding: Vec<(u64, usize)> = (0..u64::from(FEC_R)).map(|id| (id, dups(id))).collect();
    let control: Vec<(u64, usize)> = (u64::from(FEC_R)..FEC_K).map(|id| (id, dups(id))).collect();
    println!(
        "[P0-ESI uçuş] parite(seq,esi)={parity:?}; ACK t={} ms; ufuk {} s; Duplicate sayısı \
         ESI-çakışan {colliding:?} · kontrol {control:?}",
        t_ack / MS,
        horizon / SEC
    );
    if control.iter().any(|c| c.1 == 0) {
        o.fixture.push(format!(
            "kontrol birimlerinin (ESI ile çakışmayan) kopyası çıkmadı: {control:?}"
        ));
    }
    for &(id, n) in &colliding {
        if n == 0 {
            o.defect.push(format!(
                "[uçuş kolu] u{id}: parite (esi {id}) ACK'i uçuştan acked_units'e girdi \
                 (sender.rs:3019-3027; parite unit.id = esi, sender.rs:5253-5261), \
                 dup_queue.retain (sender.rs:3153-3155) u{id}'nin bekleyen kopyasını iptal etti; \
                 u{id} hiç teyit edilmediği hâlde {} s boyunca kopyası gönderilmedi",
                (horizon - t_ack) / SEC
            ));
        }
    }
    o
}

/// Tombstone kolu: blok 0'ın 8 kaynağı ve 4 paritesi PreConverge'de gider; blok 1'in ilk 4
/// birimi (kimlik 8..11) arkadan gelir. 0..4 kaynakları ve parite düşer; 5..11 ACK'lenir → RTT
/// kurulur, paket eşiği (sender.rs:3157-3162) parite seq'lerini kayıp ilan edip tombstone'a koyar
/// (sender.rs:3434-3435, :3446-3468). Parite geç gelir: geç ACK tombstone'a çarpar
/// (sender.rs:3038-3043). Kimliği ESI ile çakışan 0..3'ün PreConverge kopyaları iptal edilir;
/// u4 aynı koşuda kontroldür.
fn esi_tombstone_arm() -> ArmOutcome {
    const UNIT: u64 = 100;
    const LOST_DATA: u64 = 5; // u0..u4 kaynakları düşer
    let mut o = ArmOutcome::default();
    let mut d = Drv::new(product_sender_static_r());
    for id in 0..FEC_K {
        d.app(id, UNIT);
    }
    let auth = d.s.open_fec_block_authority();
    let closed = d.s.close_fec_block_manual(auth, 0, FEC_R, UNIT);
    if closed != Ok(FEC_R) {
        o.fixture.push(format!(
            "statik-r blok kapanışı {closed:?} döndü (beklenen Ok({FEC_R}))"
        ));
    }
    d.pump_to(MS, MS);
    let parity = d.kind_seqs(TransmitKind::FecParity);
    d.pump_to(2 * MS, MS);
    for id in FEC_K..FEC_K + 4 {
        d.app(id, UNIT);
    }
    let delivered: Vec<u64> = (LOST_DATA..FEC_K + 4)
        .filter_map(|id| d.data_seq(id))
        .collect();
    if parity.len() != usize::from(FEC_R) || delivered.len() as u64 != FEC_K + 4 - LOST_DATA {
        o.fixture.push(format!(
            "gönderim kurulumu eksik: parite={parity:?}, teslim edilecek veri seq={delivered:?}"
        ));
    }
    d.pump_to(300 * MS, 10 * MS);
    d.ack(delivered.clone());
    if d.s.state() == SenderState::PreConverge {
        o.fixture
            .push("teslim ACK'i PreConverge'i sonlandırmalı (RTT örneği)".to_string());
    }
    if d.s.diag_repair_loss_not_arqed() != u64::from(FEC_R) {
        o.fixture.push(format!(
            "parite kayıp ilanıyla tombstone'a girmeliydi: repair_loss_not_arqed={} (beklenen {FEC_R})",
            d.s.diag_repair_loss_not_arqed()
        ));
    }
    d.pump_to(SEC, 10 * MS);
    let hits_before = d.s.diag_tombstone().1;
    // Geç parite: alıcının birikimli ACK'i artık parite seq'lerini de taşır.
    let late: Vec<u64> = parity
        .iter()
        .map(|p| p.0)
        .chain(delivered.iter().copied())
        .collect();
    d.ack(late);
    let hits = d.s.diag_tombstone().1.saturating_sub(hits_before);
    if hits != u64::from(FEC_R) {
        o.fixture.push(format!(
            "geç parite ACK'i tombstone'a çarpmalıydı: hit={hits} (beklenen {FEC_R})"
        ));
    }
    let t_ack = d.now;
    // Kopyalar t=0'da vadelendi: vade = ilk kopya gecikmesi; RECOVER_FWD ancak son ACK + ≥14 s.
    let horizon = preconverge_first_copy_delay_ns(UNIT) + 2 * SEC;
    d.pump_to(horizon, 10 * MS);
    let dups = |id: u64| {
        d.log
            .iter()
            .filter(|e| e.t > t_ack && e.d.kind == TransmitKind::Duplicate && e.d.unit_id == id)
            .count()
    };
    let colliding: Vec<(u64, usize)> = (0..u64::from(FEC_R)).map(|id| (id, dups(id))).collect();
    let control = (u64::from(FEC_R), dups(u64::from(FEC_R)));
    println!(
        "[P0-ESI tombstone] parite(seq,esi)={parity:?}; geç ACK t={} ms (tombstone hit={hits}); \
         ufuk {} ms; Duplicate sayısı ESI-çakışan {colliding:?} · kontrol {control:?}",
        t_ack / MS,
        horizon / MS
    );
    if control.1 == 0 {
        o.fixture.push(format!(
            "kontrol birimi u{} (ESI ile çakışmaz, kayıp) kopyası çıkmadı",
            control.0
        ));
    }
    for &(id, n) in &colliding {
        if n == 0 {
            o.defect.push(format!(
                "[tombstone kolu] u{id}: kayıp ilan edilmiş parite (esi {id}) geç ACK'i tombstone'dan \
                 acked_units'e girdi (sender.rs:3038-3043), dup_queue.retain (sender.rs:3153-3155) \
                 teyitsiz u{id}'nin kopyasını iptal etti"
            ));
        }
    }
    o
}

/// ESI — parite ACK'i aynı kimlikli veri biriminin bekleyen kopya işini iptal eder. Tabanda: KIRMIZI.
///
/// Neden kırmızı (kod): parite kaydı `OpaqueUnit.id = esi` taşır (sender.rs:5253-5261); uçuş kolu
/// (sender.rs:3027) ve tombstone kolu (sender.rs:3043) kökene bakmadan bu kimliği `acked_units`'e
/// koyar; `dup_queue.retain` (sender.rs:3153-3155) aynı kimlikli bütün kopya kayıtlarını siler.
/// ESI uzayı 0..R (profil 0x05'te 0..4) oturumun ilk veri kimlikleriyle (0..3) çakışır.
///
/// Fikstür (C9 §2 ESI): parite ESI = veri UnitId 0..3; uçuş ve tombstone kolları ayrı ayrı; her
/// kolda ESI ile çakışmayan kayıp birim(ler) aynı koşuda kontroldür (tabanda kopyası çıkar).
/// Ürün sapması: parite statik-r kolu ile planlanır (`product_sender_static_r`).
#[test]
fn p0_esi_parity_ack_cancels_unrelated_dup() {
    const TEST: &str = "p0_esi_parity_ack_cancels_unrelated_dup";
    let a = esi_inflight_arm();
    let b = esi_tombstone_arm();
    let fixture: Vec<String> = a
        .fixture
        .iter()
        .map(|m| format!("[uçuş kolu] {m}"))
        .chain(b.fixture.iter().map(|m| format!("[tombstone kolu] {m}")))
        .collect();
    assert_fixture(TEST, &fixture);
    let defect: Vec<String> = a.defect.into_iter().chain(b.defect).collect();
    assert!(
        defect.is_empty(),
        "ESI KUSURU (parite ACK'i ilgisiz veri kopyasını iptal etti):\n{}",
        defect.join("\n")
    );
}

// ════════════════════════════════════════════════════════════════════════════════════════════
// 3 · ESI — kuyruk-probu pariteyi hedefler ve Retransmit olarak yeniden sınıflar
// ════════════════════════════════════════════════════════════════════════════════════════════

/// ESI — en eski uçuş kaydı parite iken kuyruk-probu pariteyi hedefler. Tabanda: KIRMIZI.
///
/// Neden kırmızı (kod): prob hedefi kökene bakmadan `inflight`'ın en küçük seq'idir
/// (sender.rs:3227-3238; tail_recovery.rs:199-216 filtresiz kabul eder); `emit_tail_probe` kaydın
/// `unit`'ini okur (sender.rs:5765), pariteyi tombstone'a emekliye ayırır (:5794) ve
/// `TransmitKind::Retransmit`, `unit_id = esi` olarak yeniden yayar (:5796-5803). Runtime bunu
/// kaynak birim sayar (runtime.rs:1461-1471), teyitli birimin deposu yoksa bayat karar
/// (runtime.rs:1543-1548) ve kalıcı tablo girdisi üretir.
///
/// Fikstür: blok 0'ın 8 kaynağı + 4 paritesi; 0.3 s'de 8 kaynağın hepsi ACK'lenir (RTT kurulur),
/// uçuşta yalnız parite kalır (`inflight_bytes` = 4 × boy). Sonraki 60 s'de kimliği 0..7 olan
/// (hepsi teyitli) birim için hiçbir Retransmit/Duplicate kararı çıkmamalıdır. Ürün sapması:
/// statik-r parite kolu.
#[test]
fn p0_esi_probe_targets_parity() {
    const TEST: &str = "p0_esi_probe_targets_parity";
    const UNIT: u64 = 100;
    let mut fixture: Vec<String> = Vec::new();
    let mut d = Drv::new(product_sender_static_r());
    for id in 0..FEC_K {
        d.app(id, UNIT);
    }
    let auth = d.s.open_fec_block_authority();
    let closed = d.s.close_fec_block_manual(auth, 0, FEC_R, UNIT);
    if closed != Ok(FEC_R) {
        fixture.push(format!(
            "statik-r blok kapanışı {closed:?} döndü (beklenen Ok({FEC_R}))"
        ));
    }
    d.pump_to(MS, MS);
    let parity = d.kind_seqs(TransmitKind::FecParity);
    let data: Vec<u64> = (0..FEC_K).filter_map(|id| d.data_seq(id)).collect();
    if parity.len() != usize::from(FEC_R) || data.len() as u64 != FEC_K {
        fixture.push(format!(
            "gönderim kurulumu eksik: veri seq={data:?}, parite={parity:?}"
        ));
    }
    d.pump_to(300 * MS, 10 * MS);
    d.ack(data);
    if d.s.state() == SenderState::PreConverge {
        fixture.push("kaynak ACK'i PreConverge'i sonlandırmalı".to_string());
    }
    let inflight = d.s.inflight_bytes();
    if inflight != u64::from(FEC_R) * UNIT {
        fixture.push(format!(
            "teyitten sonra uçuşta yalnız parite kalmalı: inflight_bytes={inflight} (beklenen {})",
            u64::from(FEC_R) * UNIT
        ));
    }
    let t_conf = d.now;
    d.pump_to(t_conf + 60 * SEC, 10 * MS);
    let offending: Vec<Emit> = d
        .log
        .iter()
        .filter(|e| e.t > t_conf && is_reemission(e.d.kind) && e.d.unit_id < FEC_K)
        .copied()
        .collect();
    let by_probe = offending
        .iter()
        .filter(|e| e.probe && e.d.kind == TransmitKind::Retransmit)
        .count();
    println!(
        "[P0-ESI prob] parite(seq,esi)={parity:?}; teyit t={} ms; teyitten sonra 0..7 kimlikli \
         yeniden-emisyon: {} (prob kaynaklı {by_probe}); prob_sent={}; runtime aynası: \
         store-miss={}, kalıcı seq→birim girdisi={}",
        t_conf / MS,
        fmt_tx(&offending),
        d.s.tail_probe_diag_ext().1,
        d.rt.stale_decisions,
        d.rt.seq_to_unit.len()
    );
    assert_fixture(TEST, &fixture);
    assert!(
        offending.is_empty(),
        "ESI KUSURU (prob hedefi): uçuşta yalnız parite varken kuyruk-probu pariteyi hedefledi \
         (sender.rs:3227-3238) ve onu Retransmit, unit_id = esi olarak yeniden yaydı \
         (sender.rs:5764-5803); bu kimlikler teyitli veri birimleridir (runtime aynasında \
         store-miss={}). Kararlar: {}",
        d.rt.stale_decisions,
        fmt_tx(&offending)
    );
}

// ════════════════════════════════════════════════════════════════════════════════════════════
// 4 · BURST — tur başı BURST_CAP kırpması büyük kurtarma birimini aç bırakır
// ════════════════════════════════════════════════════════════════════════════════════════════

const BURST_LINK_BPS: u64 = 1064;
const BURST_WARM_UNIT: u64 = 100;
const BURST_WARM_END: u64 = 10 * SEC;
const BURST_SMALL: u64 = 37;
const BURST_GAP: u64 = 200 * MS;
const BURST_T_V: u64 = 20 * SEC;

#[derive(Debug, Default)]
#[allow(dead_code)] // Bazı alanlar yalnız `{:?}` raporunda okunur.
struct BurstReport {
    v_id: Option<u64>,
    t_v_sent: Option<u64>,
    /// V gönderildiği tikteki birikimli kayıp-ilanı sayacı (`diag_loss().0 + .1`).
    loss_cum_at_v: Option<u64>,
    /// V'nin kayıp ilanı: birikimli sayaç arttı ya da `pending_lost` ≥ 1 (aynı çağrıda ilan +
    /// ARQ olursa `pending_lost` çağrı sonunda boştur — post-call residue kuralı).
    t_loss: Option<u64>,
    rate_at_loss: u64,
    rate_max_after_loss: u64,
    tokens_max_after_loss: u64,
    pending_ticks: u64,
    ticks_after_loss: u64,
    state_at_v: Option<SenderState>,
    startup_floor_at_v: u64,
    reemissions: Vec<(u64, TransmitKind)>,
}

/// Doğrulayıcının modeli: darboğaz 1064 B/s (40 ms pencere ≈ 42 B), tek yön 200 ms. 0-10 s ısınma
/// (100 B / 100 ms; btlbw darboğazı ölçer), sonra 37 B küçük DATA / 200 ms (varış aralığı
/// < ~0.95 s). `T_V`'de `big` boyunda V birimi girer; V'nin YALNIZ ilk (Data) iletimi düşer.
/// Ürün pompası 1 ms: kırpma her olayda yeniden hesaplandığından (sender.rs:2234-2236,
/// :2310-2325) pompa çözünürlüğü korunur.
fn burst_run(big: u64, horizon: u64) -> BurstReport {
    let mut sim = Sim::new(
        product_sender(),
        PACE_TICK_NS,
        Path::new(one_way_fixed, Some(BURST_LINK_BPS)),
        Path::new(one_way_fixed, None),
        false,
    );
    let mut r = BurstReport::default();
    let mut next_id: u64 = 0;
    let mut scan = 0usize;
    loop {
        let now = sim.now;
        if now == BURST_T_V {
            let v = next_id;
            next_id += 1;
            sim.drop_fwd = Box::new(move |d: &TransmitDecision| {
                d.kind == TransmitKind::Data && d.unit_id == v
            });
            r.state_at_v = Some(sim.s.state());
            r.startup_floor_at_v = sim.s.startup_pacing_floor_bps();
            sim.app(v, big);
            r.v_id = Some(v);
        }
        let gap = if now < BURST_WARM_END {
            100 * MS
        } else {
            BURST_GAP
        };
        if now.is_multiple_of(gap) {
            let size = if now < BURST_WARM_END {
                BURST_WARM_UNIT
            } else {
                BURST_SMALL
            };
            sim.app(next_id, size);
            next_id += 1;
        }
        sim.timers();
        if let Some(v) = r.v_id {
            for e in &sim.log[scan..] {
                if e.d.unit_id != v || !carries_unit(e.d.kind) {
                    continue;
                }
                if e.d.kind == TransmitKind::Data {
                    r.t_v_sent = r.t_v_sent.or(Some(e.t));
                } else {
                    r.reemissions.push((e.t, e.d.kind));
                }
            }
        }
        scan = sim.log.len();
        let (loss_pkt, loss_time, pending) = sim.s.diag_loss();
        let loss_cum = loss_pkt + loss_time;
        let (rate, tokens, _cap) = sim.s.diag_pacer();
        if r.t_v_sent.is_some() && r.loss_cum_at_v.is_none() {
            r.loss_cum_at_v = Some(loss_cum);
        }
        let declared = r.loss_cum_at_v.is_some_and(|c| loss_cum > c || pending > 0);
        if r.t_loss.is_none() && declared {
            r.t_loss = Some(now);
            r.rate_at_loss = rate;
        }
        if r.t_loss.is_some() {
            r.rate_max_after_loss = r.rate_max_after_loss.max(rate);
            r.tokens_max_after_loss = r.tokens_max_after_loss.max(tokens);
            r.ticks_after_loss += 1;
            if pending > 0 {
                r.pending_ticks += 1;
            }
        }
        if now >= horizon {
            break;
        }
        sim.advance();
    }
    r
}

/// BURST — tur başı BURST_CAP kırpması bekleyen büyük kurtarma gönderimini aç bırakır. Tabanda: KIRMIZI.
///
/// Neden kırmızı (kod): tur başı cap = clamp(send_rate × 40 ms, max(tx başı, iw), max(tx başı,
/// 2·MTU, iw)) (sender.rs:2310-2325); formülde ARQ planı, prob hedefi ve kopya başı yoktur.
/// `set_burst_cap` token'ı yeni cap'e kırpar (pacer.rs:70-73); refill eski cap'le yapılır, kırpma
/// sonra gelir (sender.rs:2234-2236, :2324-2325). Her küçük DATA varışında token ≈ 42 B'ye
/// kırpılır; iki varış arasında biriken kredi 1050 B'ye yetişmez (≈ 0.95 s gerekir). ARQ
/// `try_send` reddinde `break` eder (sender.rs:5656-5661) ve uyanma da yoktur
/// (pacer.rs:116 `size > burst_cap ⇒ None`).
///
/// Fikstür (C9 §2 BURST; doğrulayıcı modeli 37 B / 42 B / 1050 B, 600 s): V kayıp ilan edilir
/// (pending_lost ≥ 1), kayıp anında hız sunulan DATA yükünün en az 2 katıdır (DATA doymamış: aksi
/// hâlde düzeltilmiş taban da açlık gösterir, DUR-9 b sınıfı), kayıptan sonra hız kırpma rejimindedir
/// (iki küçük DATA arasında biriken kredi < 1050 B). Kontrol kolu: kurtarma birimi DATA başı
/// boyunda (37 B) iken tabanda 60 s içinde yeniden gönderilir.
#[test]
fn p0_burst_cap_clamp_starves_recovery() {
    const TEST: &str = "p0_burst_cap_clamp_starves_recovery";
    const BIG: u64 = 1050;
    const HORIZON: u64 = 600 * SEC;
    let main = burst_run(BIG, HORIZON);
    let control = burst_run(BURST_SMALL, BURST_T_V + 60 * SEC);
    let offered = BURST_SMALL * SEC / BURST_GAP;
    let window = main.rate_max_after_loss * BURST_WINDOW_NS / SEC;
    let reach = window.max(BURST_SMALL) - BURST_SMALL + main.rate_max_after_loss * BURST_GAP / SEC;
    println!("[P0-BURST] ana kol (V={BIG} B): {main:?}");
    println!(
        "[P0-BURST] iki küçük DATA arasında azami kredi ≈ {reach} B (R_max={} B/s); sunulan DATA {offered} B/s",
        main.rate_max_after_loss
    );
    println!("[P0-BURST] kontrol kolu (V={BURST_SMALL} B): {control:?}");
    let mut fixture: Vec<String> = Vec::new();
    if main.t_v_sent.is_none() {
        fixture.push("V'nin ilk (Data) iletimi hiç çıkmadı".to_string());
    }
    if main.t_loss.is_none() {
        fixture.push("V kayıp ilan edilmedi (pending_lost hiç ≥ 1 olmadı)".to_string());
    }
    if main.rate_at_loss < 2 * offered {
        fixture.push(format!(
            "kayıp anında hız {} B/s < 2 × sunulan {offered} B/s: DATA doymuş; B1 düzeltmesi de \
             kurtaramaz (sınıflar arası sıra, DUR-9 b) → bu fikstür B1'i sınamaz",
            main.rate_at_loss
        ));
    }
    if reach >= BIG {
        fixture.push(format!(
            "kırpma rejimi dışında: R_max={} B/s ile iki küçük DATA arasında {reach} B kredi birikebilir ≥ {BIG} B",
            main.rate_max_after_loss
        ));
    }
    if control.reemissions.is_empty() {
        fixture.push(format!(
            "kontrol kolu (kurtarma birimi {BURST_SMALL} B) tabanda yeniden gönderilmedi: {:?}",
            control.reemissions
        ));
    }
    assert_fixture(TEST, &fixture);
    assert!(
        !main.reemissions.is_empty(),
        "BURST KUSURU: V ({BIG} B) {} ms'de kayıp ilan edildi, sonraki {} s boyunca HİÇ yeniden \
         gönderilmedi; kayıptan sonra gözlenen azami token {} B < {BIG} B. Tur başı cap tabanı yalnız \
         tx başıdır (sender.rs:2322-2325), set_burst_cap kredisi kırpar (pacer.rs:72); ARQ başı \
         tabana girmez.",
        main.t_loss.map_or(0, |t| t / MS),
        HORIZON.saturating_sub(main.t_loss.unwrap_or(HORIZON)) / SEC,
        main.tokens_max_after_loss
    );
}

// ════════════════════════════════════════════════════════════════════════════════════════════
// 5 · Sınıflar arası pay — doygun DATA, vadesi gelmiş eski kopyayı aç bırakır
// ════════════════════════════════════════════════════════════════════════════════════════════

struct Turn {
    t: u64,
    out: Vec<TransmitDecision>,
    rate: u64,
    tokens: u64,
}

/// PAY — pace'i dolduran DATA, vadesi gelmiş eski kopyayı aç bırakır. Tabanda: KIRMIZI.
///
/// Neden kırmızı (kod): planlama sırası DATA → ARQ → prob → kuruluş probu → kopya kuyruğu → parite
/// (sender.rs:4236-4255); `drain_new_data` token yettikçe sürer (sender.rs:5977-5996), kopya
/// drenajı ilk token reddinde `break` eder (sender.rs:5943-5948). Tek pay mekanizması RECOVER_ARQ'ya
/// özgüdür: `pending_lost` doluyken her `K_ARQ_SHARE` (= 4, sender.rs:1479) turda bir ARQ DATA'dan
/// önce denenir (sender.rs:4229-4235). Kopya kuyruğu için eşdeğeri yoktur.
///
/// Fikstür (C9 §2 "Sınıflar arası pay"): U (kimlik 4, en yüksek seq) PreConverge'de gönderilir ve
/// §3.3-B kopyası t≈7.98 s'ye vadelenir (sender.rs:6014-6028); 0.3 s'de diğer birimler ACK'lenir
/// (RTT kurulur, U kayıp ilan edilmez, durum RecoverArq değildir: pay kuralı durumdan bağımsız
/// okunmuştur). 7 s'de 200 birimlik DATA birikimi girer. Tur = 200 ms'de bir `Timer::Pto`: her tur
/// en az bir birimlik kredi taşır (hız ≥ boy / 200 ms) ve DATA turu doldurur (tur sonu token < boy);
/// 1 ms pompada turların çoğu kredisizdir ve "K'inci tur" yalnız eşik geçiş rastlantısına bağlı
/// kalırdı. Kural: vadeden sonraki ilk `K_ARQ_SHARE` turdan birinde kopya, aynı turdaki DATA'dan
/// önce hizmet almalıdır.
#[test]
fn p0_class_share_saturated_data_starves_copy() {
    const TEST: &str = "p0_class_share_saturated_data_starves_copy";
    const UNIT: u64 = 100;
    const BACKLOG: u64 = 200;
    const TURN: u64 = 200 * MS;
    const END: u64 = 14 * SEC;
    let mut fixture: Vec<String> = Vec::new();
    let mut d = Drv::new(product_sender());
    for id in 0..4 {
        d.app(id, UNIT);
    }
    let u: u64 = 4;
    d.app(u, UNIT);
    let warm: Vec<u64> = (0..4).filter_map(|id| d.data_seq(id)).collect();
    if warm.len() != 4 || d.data_seq(u).is_none() {
        fixture.push("t=0 ilk uçuşu eksik (IW 2400 B)".to_string());
    }
    let copy_due = preconverge_first_copy_delay_ns(UNIT);
    d.pump_to(300 * MS, 10 * MS);
    d.ack(warm);
    if d.s.state() == SenderState::PreConverge {
        fixture.push("ısınma ACK'i PreConverge'i sonlandırmalı".to_string());
    }
    d.pump_to(7 * SEC, 10 * MS);
    if d.s.diag_loss().2 != 0 {
        fixture.push("U kayıp ilan edilmemeli (en yüksek seq, later-ACK yok)".to_string());
    }
    if d.log
        .iter()
        .any(|e| e.d.kind == TransmitKind::Duplicate && e.d.unit_id == u)
    {
        fixture.push("U'nun kopyası vadeden önce çıktı".to_string());
    }
    let t_backlog = d.now;
    for i in 0..BACKLOG {
        d.app(5 + i, UNIT);
    }
    let mut turns: Vec<Turn> = Vec::new();
    while d.now + TURN <= END {
        d.now += TURN;
        let out = d.ev(Event::TimerFired(Timer::Pto));
        let (rate, tokens, _cap) = d.s.diag_pacer();
        turns.push(Turn {
            t: d.now,
            out,
            rate,
            tokens,
        });
    }
    let due: Vec<&Turn> = turns
        .iter()
        .filter(|t| t.t >= copy_due)
        .take(K_ARQ_SHARE)
        .collect();
    let is_copy = |x: &TransmitDecision| x.kind == TransmitKind::Duplicate && x.unit_id == u;
    if due.len() < K_ARQ_SHARE {
        fixture.push(format!("vade sonrası yalnız {} tur ölçüldü", due.len()));
    }
    for t in &due {
        let grant = t
            .out
            .iter()
            .any(|x| x.kind == TransmitKind::Data || is_copy(x));
        if !grant {
            fixture.push(format!(
                "{} ms turu kredi almadı (hiç emisyon yok)",
                t.t / MS
            ));
        }
        if t.tokens >= UNIT {
            fixture.push(format!(
                "{} ms turunda pace dolu değil: tur sonu token {} ≥ {UNIT}",
                t.t / MS,
                t.tokens
            ));
        }
        if t.rate < UNIT * SEC / TURN {
            fixture.push(format!(
                "{} ms turunda hız {} B/s < {} B/s: tur başına bir birimlik kredi yok",
                t.t / MS,
                t.rate,
                UNIT * SEC / TURN
            ));
        }
    }
    let last_due = due.last().map_or(0, |t| t.t);
    let data_sent = d
        .log
        .iter()
        .filter(|e| e.t >= t_backlog && e.t <= last_due && e.d.kind == TransmitKind::Data)
        .count() as u64;
    if data_sent >= BACKLOG {
        fixture.push("ölçüm penceresinde DATA birikimi tükendi (doygunluk yok)".to_string());
    }
    let served_at = due
        .iter()
        .find(|t| {
            let copy = t.out.iter().position(is_copy);
            let data = t.out.iter().position(|x| x.kind == TransmitKind::Data);
            copy.is_some_and(|p| data.is_none_or(|q| p < q))
        })
        .map(|t| t.t);
    let first_any = turns
        .iter()
        .find(|t| t.out.iter().any(is_copy))
        .map(|t| t.t / MS);
    let per_turn: Vec<(u64, usize, bool, u64)> = due
        .iter()
        .map(|t| {
            (
                t.t / MS,
                t.out
                    .iter()
                    .filter(|x| x.kind == TransmitKind::Data)
                    .count(),
                t.out.iter().any(is_copy),
                t.tokens,
            )
        })
        .collect();
    println!(
        "[P0-PAY] kopya vadesi {} ms; vade sonrası ilk {K_ARQ_SHARE} tur (t_ms, DATA, kopya, tur sonu token): \
         {per_turn:?}; kopyanın ilk hizmeti (herhangi tur) {first_any:?} ms; DATA birikimi 7 s'de {BACKLOG}",
        copy_due / MS
    );
    assert_fixture(TEST, &fixture);
    assert!(
        served_at.is_some(),
        "PAY KUSURU: vadesi {} ms'de gelen U kopyası, pace'i dolduran DATA altında vade sonrası ilk \
         {K_ARQ_SHARE} turun hiçbirinde DATA'dan önce hizmet almadı (ilk hizmet: {first_any:?} ms). \
         Kopya drenajı DATA'dan sonra koşar ve ilk token reddinde durur (sender.rs:4236-4251, \
         :5943-5948); K_ARQ_SHARE payı yalnız RECOVER_ARQ+pending_lost içindir (sender.rs:4229-4235).",
        copy_due / MS
    );
}

// ════════════════════════════════════════════════════════════════════════════════════════════
// 6 · KAYNAK — teyit sonrası bayat motor kararı
// ════════════════════════════════════════════════════════════════════════════════════════════

/// KAYNAK — teyit edilmiş birim için motor yeni ARQ/kopya/prob kararı üretir. Tabanda: KIRMIZI.
///
/// Neden kırmızı (kod): ACK yolunda teyitli birim yalnız kopya kuyruğundan silinir
/// (sender.rs:3152-3155); `queue_losses` (sender.rs:3371-3444) ve `plan_arq_retransmits`
/// (sender.rs:5632-5684) teyitli birimi süzmez. Birimin teyitten sonra uçuşta kalan iletimi later-ACK
/// ile kayıp ilan edilir ve ARQ onu yeniden gönderir. Runtime'da bu karar store-miss olarak
/// bayatlar (runtime.rs:1543-1548) ve seq→birim girdisi kalıcı olur (runtime.rs:1469-1470,
/// :1736-1743) — runtime tablo sızıntısının motor kaynağı (doğrulama KAYNAK maddesi).
///
/// Fikstür: RTT kurulur; u4..u9 gönderilir, u4'ün orijinali eksik kalacak şekilde 5..9 ACK'lenir →
/// u4 paket eşiğiyle kayıp ilan edilir ve ARQ yeniden gönderir; ardından u4'ün ORİJİNAL seq'i geç
/// ACK'lenir (tombstone kol-2) → u4 teyitlidir, runtime aynasında payload silinir. Sonra u10..u14
/// gönderilip ACK'lenir (prob vadesinden önce) → u4'ün uçuştaki yeniden gönderimi kayıp ilan edilir.
/// Teyitten sonra u4 için hiçbir Retransmit/Duplicate kararı çıkmamalıdır.
#[test]
fn p0_stale_decision_after_confirm() {
    const TEST: &str = "p0_stale_decision_after_confirm";
    const UNIT: u64 = 100;
    let mut fixture: Vec<String> = Vec::new();
    let mut d = Drv::new(product_sender());
    for id in 0..4 {
        d.app(id, UNIT);
    }
    d.pump_to(300 * MS, 10 * MS);
    let warm: Vec<u64> = (0..4).filter_map(|id| d.data_seq(id)).collect();
    d.ack(warm);
    if d.s.state() == SenderState::PreConverge {
        fixture.push("ısınma ACK'i PreConverge'i sonlandırmalı".to_string());
    }
    d.pump_to(500 * MS, 10 * MS);
    let u: u64 = 4;
    for id in 4..10 {
        d.app(id, UNIT);
    }
    d.pump_to(800 * MS, 10 * MS);
    let u_seq = d.data_seq(u);
    let tail: Vec<u64> = (5..10).filter_map(|id| d.data_seq(id)).collect();
    if u_seq.is_none() || tail.len() != 5 {
        fixture.push(format!(
            "u4..u9 gönderimi eksik: u4={u_seq:?}, u5..u9={tail:?}"
        ));
    }
    let loss_before = d.s.diag_loss().0;
    d.ack(tail.clone());
    d.pump_to(850 * MS, 10 * MS);
    if d.s.diag_loss().0 <= loss_before {
        fixture.push("u4 paket eşiğiyle kayıp ilan edilmedi".to_string());
    }
    let retx1 = d
        .log
        .iter()
        .find(|e| e.t >= 800 * MS && e.d.kind == TransmitKind::Retransmit && e.d.unit_id == u)
        .copied();
    if retx1.is_none() {
        fixture.push("u4 için ilk ARQ yeniden gönderimi çıkmadı".to_string());
    }
    d.pump_to(900 * MS, 10 * MS);
    let hits_before = d.s.diag_tombstone().1;
    let late: Vec<u64> = u_seq.into_iter().chain(tail.iter().copied()).collect();
    d.ack(late);
    if d.s.diag_tombstone().1 <= hits_before {
        fixture.push("u4'ün geç orijinal ACK'i tombstone'a çarpmadı".to_string());
    }
    if d.rt.has_payload(u) {
        fixture.push(
            "runtime aynasında u4 payload'u teyitte silinmeliydi (runtime.rs:1736-1744)"
                .to_string(),
        );
    }
    let t_conf = d.now;
    d.pump_to(920 * MS, 10 * MS);
    for id in 10..15 {
        d.app(id, UNIT);
    }
    d.pump_to(1_100 * MS, 10 * MS);
    let more: Vec<u64> = (10..15).filter_map(|id| d.data_seq(id)).collect();
    if more.len() != 5 {
        fixture.push(format!("u10..u14 gönderimi eksik: {more:?}"));
    }
    d.ack(more);
    d.pump_to(3 * SEC, 10 * MS);
    let stale: Vec<Emit> = d
        .log
        .iter()
        .filter(|e| e.t > t_conf && e.d.unit_id == u && is_reemission(e.d.kind))
        .copied()
        .collect();
    let labels: Vec<String> = stale
        .iter()
        .map(|e| {
            let path = match (e.d.kind, e.probe) {
                (TransmitKind::Retransmit, true) => "kuyruk-probu (sender.rs:5758-5805)",
                (TransmitKind::Retransmit, false) => "ARQ (sender.rs:5647-5683; ilan :3436-3444)",
                _ => "kopya (sender.rs:5932-5972)",
            };
            format!("{} ms {:?} seq {} ← {path}", e.t / MS, e.d.kind, e.d.seq)
        })
        .collect();
    println!(
        "[P0-KAYNAK] u4 teyit t={} ms; ilk ARQ {:?}; teyitten sonra u4 kararları: {labels:?}; \
         runtime aynası store-miss={}, u4'e işaret eden kalıcı seq→birim girdisi={}",
        t_conf / MS,
        retx1.map(|e| (e.t / MS, e.d.seq)),
        d.rt.stale_decisions,
        d.rt.entries_for(u)
    );
    assert_fixture(TEST, &fixture);
    assert!(
        stale.is_empty(),
        "KAYNAK KUSURU: u4 {} ms'de geç orijinal ACK ile teyit edildi, motor sonra u4 için {} yeni \
         karar üretti: {labels:?}. queue_losses/plan_arq_retransmits teyitli birimi süzmüyor \
         (sender.rs:3371-3444, :5632-5684); runtime'da store-miss={} ve kalıcı tablo girdisi={}.",
        t_conf / MS,
        stale.len(),
        d.rt.stale_decisions,
        d.rt.entries_for(u)
    );
}

// ════════════════════════════════════════════════════════════════════════════════════════════
// 7 · RTT — kalıcı yol değişimi 0.4 s → 2 s
// ════════════════════════════════════════════════════════════════════════════════════════════

const RTT_T_CHANGE: u64 = 60 * SEC;
const RTT_NEW_NS: u64 = 2 * SEC;
const RTT_OLD_NS: u64 = 400 * MS;

/// Yol değişimi: gönderim anı `RTT_T_CHANGE`'den önceyse tek yön 200 ms, sonra 1 s (RTT 2 s).
fn path_change_delay(t: u64) -> u64 {
    if t < RTT_T_CHANGE {
        RTT_OLD_NS / 2
    } else {
        RTT_NEW_NS / 2
    }
}

/// RTT — kalıcı yol değişimi (0.4 s → 2 s) altında srtt yakınsar. Tabanda beklenen: YEŞİL
/// (rev4'te de yeşil kalmalı; C9 §4).
///
/// Neden yeşil (kod): örnek, çerçevenin en büyük yeni-onaylı `Data` kaydından alınır
/// (sender.rs:3010-3037, :3130-3141); alıcı ACK'i hızlı verdiğinden ack_delay küçüktür
/// (receiver.rs:403-405, :459-464); kuantum döngüsü yakınsamayı kanal zamanına bağlar
/// (rtt.rs:142-169, T = rtprop/4, sender.rs:3005-3007). Değişimden sonra yeni örneklerin hepsi
/// ≈ 2 s olduğundan srtt birkaç örnekte banda girer.
///
/// Fikstür: 200 B / 100 ms DATA, kimliği ≡ 3 (mod 10) olan birimlerin ilk iletimi düşer (ARQ
/// yeniden gönderir → yeniden gönderimli trafik; kuyruk probları da çıkar), 60 s'de yol kalıcı
/// olarak 2 s'ye çıkar, koşu 180 s. Pompa 10 ms: örnekler ACK olaylarında alınır, 10 ms çözünürlük
/// örneğe ≤ 10 ms nicemleme katar (bandın ‰5'i). Yakınsama ölçütü: değişimden önce srtt
/// 0.4 s ± %20; değişimden sonra ≥ 32 örnek kabul edilir; srtt en geç değişim + 60 s'de
/// |srtt − 2 s| < %20 bandına girer ve değişim + 60 s ile 180 s arasındaki her 1 s kontrol
/// noktasında bantta kalır.
#[test]
fn p0_rtt_path_change_converges() {
    const TEST: &str = "p0_rtt_path_change_converges";
    const UNIT: u64 = 200;
    const GAP: u64 = 100 * MS;
    const END: u64 = 180 * SEC;
    const STEP: u64 = 10 * MS;
    const SETTLE: u64 = 60 * SEC;
    const MIN_SAMPLES: u32 = 32;
    let mut sim = Sim::new(
        product_sender(),
        STEP,
        Path::new(path_change_delay, None),
        Path::new(path_change_delay, None),
        false,
    );
    sim.drop_fwd =
        Box::new(|d: &TransmitDecision| d.kind == TransmitKind::Data && d.unit_id % 10 == 3);
    let mut next_id: u64 = 0;
    let mut cps: Vec<(u64, Option<u64>, u32)> = Vec::new();
    loop {
        if sim.now.is_multiple_of(GAP) && sim.now < END {
            sim.app(next_id, UNIT);
            next_id += 1;
        }
        sim.timers();
        if sim.now.is_multiple_of(SEC) {
            cps.push((
                sim.now,
                sim.s.path_timing_snapshot().srtt_ns,
                sim.s.diag_rtt_sample_count(),
            ));
        }
        if sim.now >= END {
            break;
        }
        sim.advance();
    }
    let in_band = |srtt: Option<u64>, center: u64| {
        srtt.is_some_and(|v| (center * 8 / 10..=center * 12 / 10).contains(&v))
    };
    let before = cps.iter().rev().find(|c| c.0 < RTT_T_CHANGE).copied();
    let last = cps.last().copied();
    let samples_after = match (before, last) {
        (Some(b), Some(l)) => l.2.saturating_sub(b.2),
        _ => 0,
    };
    let retx_after = sim
        .log
        .iter()
        .filter(|e| e.t >= RTT_T_CHANGE && e.d.kind == TransmitKind::Retransmit)
        .count();
    let t_enter = cps
        .iter()
        .find(|c| c.0 >= RTT_T_CHANGE && in_band(c.1, RTT_NEW_NS))
        .map(|c| c.0);
    let out_of_band: Vec<(u64, Option<u64>)> = cps
        .iter()
        .filter(|c| c.0 >= RTT_T_CHANGE + SETTLE && !in_band(c.1, RTT_NEW_NS))
        .map(|c| (c.0 / SEC, c.1.map(|v| v / MS)))
        .collect();
    let trace: Vec<(u64, Option<u64>)> = cps
        .iter()
        .filter(|c| c.0 >= RTT_T_CHANGE - 2 * SEC && c.0 <= RTT_T_CHANGE + 20 * SEC)
        .map(|c| (c.0 / SEC, c.1.map(|v| v / MS)))
        .collect();
    println!(
        "[P0-RTT] değişim öncesi srtt={:?} ms; değişimden sonra kabul edilen örnek={samples_after}; \
         Retransmit={retx_after}; banda giriş={:?} s; iz (s, srtt_ms): {trace:?}",
        before.and_then(|b| b.1).map(|v| v / MS),
        t_enter.map(|t| t / SEC)
    );
    let mut fixture: Vec<String> = Vec::new();
    let before_ok = before.is_some_and(|b| in_band(b.1, RTT_OLD_NS));
    if !before_ok {
        fixture.push(format!(
            "değişimden önce srtt 0.4 s ± %20 değil: {:?} ms",
            before.and_then(|b| b.1).map(|v| v / MS)
        ));
    }
    if retx_after == 0 {
        fixture.push(
            "değişimden sonra yeniden gönderim yok (yeniden gönderimli trafik kurulmadı)"
                .to_string(),
        );
    }
    if samples_after < MIN_SAMPLES {
        fixture.push(format!(
            "değişimden sonra yalnız {samples_after} RTT örneği kabul edildi (< {MIN_SAMPLES})"
        ));
    }
    assert_fixture(TEST, &fixture);
    assert!(
        t_enter.is_some_and(|t| t <= RTT_T_CHANGE + SETTLE),
        "RTT: yol 0.4 s → 2 s kalıcı arttıktan sonra srtt {} s içinde [1.6 s, 2.4 s] bandına girmedi \
         (giriş={:?} s); iz: {trace:?}",
        SETTLE / SEC,
        t_enter.map(|t| t / SEC)
    );
    assert!(
        out_of_band.is_empty(),
        "RTT: değişim + {} s sonrasında srtt bant dışına çıktı (s, srtt_ms): {out_of_band:?}",
        SETTLE / SEC
    );
}
