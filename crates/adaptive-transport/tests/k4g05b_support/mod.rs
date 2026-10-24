//! K4G-05b test düzeneği — `tests/k4g05b_*.rs` dosyalarının ortak parçaları (`mod k4g05b_support;`).
//! Cargo `tests/` altındaki alt dizini ayrı test hedefi saymaz.
//!
//! Kaynak (K4G-05b notu Ek B.2 "Düzenek"): P0 `k4g_p0_engine.rs` düzeneği — ürün kurulumu
//! `product_sender` (`ClientSession::new` + `runtime.rs:1290-1351` aynası), `Sim` ağ düzeneği
//! (tek yön 200 ms, gerçek `Receiver`, 20 ms ACK zamanlayıcısı, isteğe bağlı FIFO darboğaz) ve P0
//! `RuntimeMirror` (eşlenen seq'in ACK'inde siler = bugünkü runtime) — davranışı DEĞİŞMEDEN
//! taşındı (`RuntimeMirror::on_ack` yalnız sildiği birimleri döndürür). Eklenenler: 05c-model teyit
//! aynası [`ConfirmMirror`] (yalnız `take_confirmed_units()` çıktısıyla siler), her motor olayından
//! sonra BİR KEZ teyit drenajı (Ek B.2.2 drenaj sözleşmesi), gözlenebilir sahiplik [`owner_of`]
//! (üç kabul koşulunun (i) vekili), olay başına gözlem kaydı [`Watch`], iki yönlü kesinti penceresi
//! ve dönüş yolu açıklık süzgeci. Motor zamanı sahte saattir (ns sayacı); gerçek uyku yoktur.
#![allow(dead_code)] // Her test dosyası düzeneğin bir alt kümesini kullanır.

use adaptive_transport::ack::AckFrame;
use adaptive_transport::emergency::EmergencyConfig;
use adaptive_transport::pacer::DEFAULT_BURST_CAP;
use adaptive_transport::receiver::{Receiver, RxDatagram, RxKind};
use adaptive_transport::sender::{
    AckInfo, BootstrapRttConfig, Event, FecActuationMode, FecLossRiskConfig, FecStructurePolicy,
    InitialWindowConfig, LossConfirmHorizon, OpaqueUnit, Sender, SenderState, SrLogicalRef, Timer,
    TransmitDecision, TransmitKind, K_MAP,
};
use adaptive_transport::tail_recovery::TailRecoveryConfig;
use std::collections::{HashMap, HashSet, VecDeque};
use std::time::Duration;

pub const MS: u64 = 1_000_000;
pub const SEC: u64 = 1_000_000_000;
/// Ürün pompası: runtime.rs:65 `PACE_TICK` = 1 ms; :1903-1905 `Timer::Pto`.
pub const PACE_TICK_NS: u64 = MS;
/// runtime.rs:67 `RTPROP_TICK` = 200 ms; :1924 `Timer::RtpropProbe`.
pub const RTPROP_TICK_NS: u64 = 200 * MS;
/// Alıcı ACK zamanlayıcısı: runtime.rs:69 `ACK_TICK` = 20 ms; :2740-2769 `on_ack_timer`.
pub const ACK_TICK_NS: u64 = 20 * MS;
/// Alıcı epizot kotası: runtime.rs:257 `quota_r: 4`; :2486-2488 `set_quota_r`.
pub const RX_QUOTA_R: u32 = 4;
/// Yapı-gate seviyesi: runtime.rs:241 `STRUCTURE_GATE_LEVEL_PERMILLE`, :279.
pub const STRUCTURE_GATE_LEVEL_PERMILLE: u32 = 250;
/// Ürün FEC bloğu: profil 0x05 k_shape 8.
pub const FEC_K: u64 = 8;
/// Alıcıya giden FEC başlığındaki k_actual (tam blok).
pub const FEC_K_ACTUAL: u8 = 8;
/// Profil 0x05 r_code_max 4 → ESI ∈ 0..4.
pub const FEC_R: u16 = 4;
/// Tek yön yayılım (Ek B.2 "Düzenek": tek yön 200 ms).
pub const ONE_WAY_NS: u64 = 200 * MS;
/// SACK tel boyu (receiver.rs:83 `SACK_WIRE_BYTES`).
pub const SACK_WIRE_BYTES: u64 = 65;
/// sender.rs `K_ARQ_SHARE` (özel sabit; değer koddan — P0 ile aynı yöntem).
pub const K_ARQ_SHARE: usize = 4;
/// sender.rs `BURST_WINDOW_NS` (özel sabit; değer koddan).
pub const BURST_WINDOW_NS: u64 = 40 * MS;
/// D2 `min_rate` (budget.rs:29; Ek B.2 "D2 simgeleri") — B/s.
pub const MIN_RATE_BPS: u64 = 31;

/// RTO_MAX = ürün `validated_v1().max_backoff_pto_ns` (30 s; client-core freeze testiyle pinli).
pub fn rto_max_ns() -> u64 {
    TailRecoveryConfig::validated_v1().max_backoff_pto_ns
}

/// Ürün kuyruk-probu bütçesi (`validated_v1().max_tail_probes` = 3).
pub fn probe_budget() -> u64 {
    u64::from(TailRecoveryConfig::validated_v1().max_tail_probes)
}

/// D_max: RECOVER_FWD kopya faktörünün tavanı (`EmergencyConfig::fixture().dup3_factor` = 3).
pub fn d_max() -> u64 {
    u64::from(EmergencyConfig::fixture().dup3_factor)
}

/// K_MAP (birim başına iletim kaydı tavanı; `sender::K_MAP`).
pub fn k_map() -> u64 {
    K_MAP as u64
}

// ════════════════════════════════════════════════════════════════════════════════════════════
// Ürün yapılandırmasının aynası (P0 `product_sender`, aynen)
// ════════════════════════════════════════════════════════════════════════════════════════════

/// `ClientSession::new` + `client_task` kurulumunun Sender API'siyle aynası (P0 ile aynı).
pub fn product_sender() -> Sender {
    // client-core/src/lib.rs:89-92 — ClientSession::new: Sender::new + validated_v1 kuyruk-probu.
    let mut s = Sender::new();
    s.set_tail_recovery_config(TailRecoveryConfig::validated_v1());
    // runtime.rs:1293-1295 — cfg.fec_structure_policy = FecStructurePolicy::default().
    s.set_fec_structure_policy(FecStructurePolicy::default());
    // runtime.rs:1298-1310 — cfg.bootstrap_rtt = false → BootstrapRttConfig::default().
    s.set_bootstrap_rtt(BootstrapRttConfig::default());
    // runtime.rs:1313 — set_cs_probe_lane(cfg.bootstrap_rtt = false).
    s.set_cs_probe_lane(false);
    // runtime.rs:1315-1317 — LossConfirmHorizon::CadenceLegacy.
    s.set_loss_confirm_horizon(LossConfirmHorizon::CadenceLegacy);
    // runtime.rs:1319 — FecLossRiskConfig::bounded_v1().
    s.set_fec_loss_risk(FecLossRiskConfig::bounded_v1());
    // runtime.rs:1321-1323 — Some(STRUCTURE_GATE_LEVEL_PERMILLE).
    s.set_structure_gate_level(Some(STRUCTURE_GATE_LEVEL_PERMILLE));
    // runtime.rs:1330-1334 — INITIAL_WINDOW_BYTES = pacer::DEFAULT_BURST_CAP.
    s.set_initial_window(InitialWindowConfig {
        bytes: DEFAULT_BURST_CAP,
    });
    // runtime.rs:1340-1351 — cfg.fec = Some(0x05, manual_r None) → Controller.
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

/// Ürün + statik-r FEC kolu (P0 ile aynı; `close_fec_block_manual` ile blok kapanışı).
pub fn product_sender_static_r() -> Sender {
    let mut s = product_sender();
    let mode = s.set_fec_actuation_mode(FecActuationMode::Manual { enabled: true });
    assert!(mode.is_ok(), "statik-r kipi kabul edilmeli");
    s
}

/// Ürün kurulumu, kuyruk-probu KAPALI (D1 kol 2/3 ve H: prob kapsamı dışında sahiplenme).
pub fn product_sender_probe_off() -> Sender {
    let mut s = product_sender();
    s.set_tail_recovery_config(TailRecoveryConfig {
        enabled: false,
        ..TailRecoveryConfig::validated_v1()
    });
    s
}

/// §3.3-B ilk kopya gecikmesi: 3/2 × (boy + PC_SACK_WIRE) / PRE_CONVERGE_FLOOR_BW (P0 ile aynı).
pub fn preconverge_first_copy_delay_ns(size: u64) -> u64 {
    let rtt_floor = (size + 65) * SEC / 31;
    rtt_floor * 3 / 2
}

// ════════════════════════════════════════════════════════════════════════════════════════════
// Kayıt
// ════════════════════════════════════════════════════════════════════════════════════════════

/// Motorun ürettiği bir karar; `probe` = aynı olayda gerçek kuyruk-probu emisyonu oldu.
#[derive(Clone, Copy, Debug)]
pub struct Emit {
    pub t: u64,
    pub d: TransmitDecision,
    pub probe: bool,
}

pub fn carries_unit(k: TransmitKind) -> bool {
    matches!(
        k,
        TransmitKind::Data | TransmitKind::Retransmit | TransmitKind::Duplicate
    )
}

pub fn is_reemission(k: TransmitKind) -> bool {
    matches!(k, TransmitKind::Retransmit | TransmitKind::Duplicate)
}

/// Motoru tek olayla sürer, kararları sırayla kaydeder (runtime.rs:1455-1460 `drive!` aynası).
pub fn drive_sender(
    s: &mut Sender,
    ev: Event,
    now: u64,
    log: &mut Vec<Emit>,
) -> Vec<TransmitDecision> {
    let probes_before = s.tail_probe_diag_ext().1;
    let out = s.on_event(ev, now);
    let probe = s.tail_probe_diag_ext().1 > probes_before;
    log.extend(out.iter().map(|&d| Emit { t: now, d, probe }));
    out
}

pub fn data_seq(log: &[Emit], unit: u64) -> Option<u64> {
    log.iter()
        .find(|e| e.d.kind == TransmitKind::Data && e.d.unit_id == unit)
        .map(|e| e.d.seq)
}

/// Birimi taşıyan bütün emisyonlar (Data / Retransmit / Duplicate), sırayla.
pub fn unit_tx(log: &[Emit], unit: u64) -> Vec<Emit> {
    log.iter()
        .filter(|e| e.d.unit_id == unit && carries_unit(e.d.kind))
        .copied()
        .collect()
}

pub fn fmt_tx(list: &[Emit]) -> String {
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

// ════════════════════════════════════════════════════════════════════════════════════════════
// İki runtime aynası
// ════════════════════════════════════════════════════════════════════════════════════════════

/// P0 runtime aynası (DEĞİŞMEDEN): depo kaydı runtime.rs:1836, `carries_unit` eşlemesi
/// :1461-1471, store-miss = bayat karar :1543-1548, ACK budaması yalnız `store.remove` başarılıysa
/// :1736-1743. Tek ek: `on_ack` sildiği birimleri döndürür (davranış aynı).
#[derive(Default)]
pub struct RuntimeMirror {
    pub store: HashSet<u64>,
    pub seq_to_unit: HashMap<u64, u64>,
    pub unit_seqs: HashMap<u64, Vec<u64>>,
    pub stale_decisions: u64,
}

impl RuntimeMirror {
    pub fn offer(&mut self, unit: u64) {
        self.store.insert(unit);
    }

    pub fn on_decision(&mut self, d: &TransmitDecision) {
        if !carries_unit(d.kind) {
            return;
        }
        self.seq_to_unit.insert(d.seq, d.unit_id);
        self.unit_seqs.entry(d.unit_id).or_default().push(d.seq);
        if !self.store.contains(&d.unit_id) {
            self.stale_decisions += 1;
        }
    }

    /// ACK'lenen seq'lerden eşlenen birimleri siler; silinen birimleri döndürür.
    pub fn on_ack(&mut self, acked: &[u64]) -> Vec<u64> {
        let mut deleted = Vec::new();
        for s in acked {
            let Some(&unit) = self.seq_to_unit.get(s) else {
                continue;
            };
            if self.store.remove(&unit) {
                deleted.push(unit);
                for old in self.unit_seqs.remove(&unit).unwrap_or_default() {
                    self.seq_to_unit.remove(&old);
                }
            }
        }
        deleted
    }

    pub fn has_payload(&self, unit: u64) -> bool {
        self.store.contains(&unit)
    }
}

/// 05c-model teyit aynası (Ek B.2 "İki runtime aynası"): payload YALNIZ `take_confirmed_units()`
/// çıktısındaki birim için silinir; her yeniden emisyonda payload aranır (store-miss sayacı); her
/// birimin teyit sayısı tutulur (tam bir kez beklenir).
#[derive(Default)]
pub struct ConfirmMirror {
    pub store: HashSet<u64>,
    pub stale_decisions: u64,
    pub confirm_counts: HashMap<u64, u32>,
    /// Teyit çıktısında olup depoda bulunmayan birim (çift teyit ya da hiç sunulmamış birim).
    pub unknown_confirms: u64,
}

impl ConfirmMirror {
    pub fn offer(&mut self, unit: u64) {
        self.store.insert(unit);
    }

    pub fn on_decision(&mut self, d: &TransmitDecision) {
        if carries_unit(d.kind) && !self.store.contains(&d.unit_id) {
            self.stale_decisions += 1;
        }
    }

    pub fn on_confirmed(&mut self, units: &[u64]) {
        for &u in units {
            *self.confirm_counts.entry(u).or_default() += 1;
            if !self.store.remove(&u) {
                self.unknown_confirms += 1;
            }
        }
    }

    pub fn has_payload(&self, unit: u64) -> bool {
        self.store.contains(&unit)
    }

    pub fn confirms(&self, unit: u64) -> u32 {
        self.confirm_counts.get(&unit).copied().unwrap_or(0)
    }
}

// ════════════════════════════════════════════════════════════════════════════════════════════
// Gözlenebilir sahiplik (üç kabul koşulunun (i) vekili)
// ════════════════════════════════════════════════════════════════════════════════════════════

/// Teyitsiz bir birimin o anki gözlenebilir sahibi (Ek B.2 "Üç kabul koşulu testi" (i):
/// `diag_dup_queue_units` + prob kapsamı + ARQ planı; madde 4 (c) H penceresi).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Owner {
    /// Kopya kuyruğunda kaydı var (tek seferlik ya da geometrik).
    Copy,
    /// Kayıp ilan edilmiş, ARQ planında (`pending_lost`).
    Arq,
    /// Kuyruk-probu kapsamı etkin (prob açık, bütçe tükenmemiş) ve uçuşta kaydı var.
    Probe,
    /// Prob kapsamı dışında, uçuşta kaydı var ve motor bir uyanma vadesi açıklıyor
    /// (H sahiplenme vadesi EK4-A2 7. kaynak).
    HWindow,
}

/// `unit`'in gözlenebilir sahibi; `None` = sahipsiz (INV-NO-ORPHAN ihlali adayı).
pub fn owner_of(s: &Sender, unit: u64) -> Option<Owner> {
    if s.diag_dup_queue_units().binary_search(&unit).is_ok() {
        return Some(Owner::Copy);
    }
    let r = s.diag_unit_recovery(unit);
    if r.pending_lost_retx.is_some() {
        return Some(Owner::Arq);
    }
    if r.inflight_records == 0 {
        return None;
    }
    let t = s.diag_recovery_timers();
    if t.tail_enabled && t.tail_probe_index < t.tail_max_probes {
        return Some(Owner::Probe);
    }
    s.next_wakeup_ns().map(|_| Owner::HWindow)
}

/// Bir motor olayının (`schedule_transmit` turunun) kaydı.
#[derive(Clone, Debug)]
pub struct TurnRec {
    pub t: u64,
    /// Olay `Event::AppData` mıydı (tur başı BURST_CAP'i yeni DATA başıyla hesaplanır).
    pub is_app: bool,
    pub out: Vec<TransmitDecision>,
    /// Tur sonu pacing hızı, token ve BURST_CAP (`diag_pacer`).
    pub rate_after: u64,
    pub tokens_after: u64,
    pub cap_after: u64,
    /// Tur sonu durum.
    pub state_after: SenderState,
    /// Tur sonu gönderilmemiş DATA sayısı (`diag_recovery_timers().tx_queue_len`).
    pub backlog_after: u32,
}

/// Olay başına gözlem: teyit drenajı + iki ayna + izlenen birimlerin sahipliği.
#[derive(Default)]
pub struct Watch {
    pub rt: RuntimeMirror,
    pub cm: ConfirmMirror,
    /// `(an, birim)` — teyit çıktısı, giriş sırasıyla.
    pub confirmed: Vec<(u64, u64)>,
    pub confirmed_set: HashSet<u64>,
    pub sent: HashSet<u64>,
    /// Sahipliği her olayda denetlenen birimler.
    pub units: Vec<u64>,
    /// `(an, birim)` — teyitsiz, gönderilmiş ama gözlenebilir sahibi olmayan birim.
    pub orphans: Vec<(u64, u64)>,
    /// `(an, birim, canlı kayıt)` — birim başına kayıt K_MAP'i aştı.
    pub kmap_over: Vec<(u64, u64, u32)>,
    pub last_owner: HashMap<u64, Option<Owner>>,
    /// `(an, birim, yeni sahip)` — sahip değişimleri (rapor).
    pub owner_changes: Vec<(u64, u64, Option<Owner>)>,
    /// `(an, durum)` — durum değişimleri.
    pub state_changes: Vec<(u64, SenderState)>,
    /// `(an, birim)` — P0 aynasının teyit çıktısında OLMAYAN birimi silmesi.
    pub p0_unconfirmed_deletions: Vec<(u64, u64)>,
    /// `birim → (ilk kez kopya kuyruğunda görüldüğü an, ondan önceki son açıklanan uyanma vadesi)`.
    pub adopted_at: HashMap<u64, (u64, Option<u64>)>,
    last_wakeup: HashMap<u64, Option<u64>>,
    /// `(an, birim, vade)` — prob kapsamı dışında, kopya kuyruğuna ve ARQ'ya girmemiş birim için
    /// olay sonunda açıklanan uyanma vadesi YOK ya da geçmiş (H-sahiplenmesi kaçırıldı).
    pub h_overdue: Vec<(u64, u64, Option<u64>)>,
    /// `(an, kopya kaydı, kopyası olan birim)` — kopya işi D_max × birim sayısını aştı.
    pub copy_work_over: Vec<(u64, u32, usize)>,
    /// `Some` ise her olay [`TurnRec`] olarak kaydedilir.
    pub turns: Option<Vec<TurnRec>>,
}

impl Watch {
    pub fn offer(&mut self, unit: u64) {
        self.rt.offer(unit);
        self.cm.offer(unit);
    }

    /// Her motor olayından sonra BİR KEZ (drenaj sözleşmesi): teyit çıktısı → 05c aynası; kararlar
    /// → iki ayna; durum; izlenen birimlerin sahipliği ve kayıt tavanı.
    pub fn after_event(
        &mut self,
        s: &mut Sender,
        now: u64,
        out: &[TransmitDecision],
        is_app: bool,
    ) {
        let confirmed = s.take_confirmed_units();
        self.cm.on_confirmed(&confirmed);
        for u in confirmed {
            self.confirmed.push((now, u));
            self.confirmed_set.insert(u);
        }
        for d in out {
            self.rt.on_decision(d);
            self.cm.on_decision(d);
            if carries_unit(d.kind) {
                self.sent.insert(d.unit_id);
            }
        }
        let state = s.state();
        if self.state_changes.last().is_none_or(|&(_, st)| st != state) {
            self.state_changes.push((now, state));
        }
        for &u in &self.units {
            if !self.sent.contains(&u) || self.confirmed_set.contains(&u) {
                continue;
            }
            let owner = owner_of(s, u);
            if owner.is_none() {
                self.orphans.push((now, u));
            }
            if self.last_owner.get(&u) != Some(&owner) {
                self.last_owner.insert(u, owner);
                self.owner_changes.push((now, u, owner));
            }
            match owner {
                Some(Owner::Copy) => {
                    let before = self.last_wakeup.get(&u).copied().flatten();
                    self.adopted_at.entry(u).or_insert((now, before));
                }
                Some(Owner::HWindow) | None => {
                    let w = s.next_wakeup_ns();
                    if w.is_none_or(|w| w <= now) {
                        self.h_overdue.push((now, u, w));
                    }
                    self.last_wakeup.insert(u, w);
                }
                Some(Owner::Arq | Owner::Probe) => {}
            }
            let r = s.diag_unit_recovery(u);
            let live = r.inflight_records + r.tombstone_records;
            if u64::from(live) > k_map() {
                self.kmap_over.push((now, u, live));
            }
        }
        if !self.units.is_empty() {
            let queued = s.diag_recovery_timers().dup_queue_len;
            let with_copies = s.diag_dup_queue_units().len();
            if u64::from(queued) > d_max() * with_copies as u64 {
                self.copy_work_over.push((now, queued, with_copies));
            }
        }
        if let Some(turns) = self.turns.as_mut() {
            let (rate_after, tokens_after, cap_after) = s.diag_pacer();
            turns.push(TurnRec {
                t: now,
                is_app,
                out: out.to_vec(),
                rate_after,
                tokens_after,
                cap_after,
                state_after: state,
                backlog_after: s.diag_recovery_timers().tx_queue_len,
            });
        }
    }

    /// P0 aynasının bu ACK'te sildiği birimler teyit çıktısında mı (Ek B.2 (ii-a)).
    pub fn check_deletions(&mut self, now: u64, deleted: &[u64]) {
        for &u in deleted {
            if !self.confirmed_set.contains(&u) {
                self.p0_unconfirmed_deletions.push((now, u));
            }
        }
    }

    /// İlk RecoverFwd girişi (`t_from` ve sonrası).
    pub fn first_state_at_or_after(&self, state: SenderState, t_from: u64) -> Option<u64> {
        self.state_changes
            .iter()
            .find(|&&(t, st)| t >= t_from && st == state)
            .map(|&(t, _)| t)
    }
}

// ════════════════════════════════════════════════════════════════════════════════════════════
// Yalnız-gönderen sürücü (ACK'ler elle; P0 `Drv` + gözlem)
// ════════════════════════════════════════════════════════════════════════════════════════════

/// Ağ yok; ACK çerçevesi testte kurulur (P0 ile aynı).
pub struct Drv {
    pub s: Sender,
    pub now: u64,
    pub log: Vec<Emit>,
    pub w: Watch,
}

impl Drv {
    pub fn new(s: Sender) -> Self {
        Self {
            s,
            now: 0,
            log: Vec::new(),
            w: Watch::default(),
        }
    }

    pub fn ev(&mut self, ev: Event) -> Vec<TransmitDecision> {
        let is_app = matches!(ev, Event::AppData(_));
        let out = drive_sender(&mut self.s, ev, self.now, &mut self.log);
        self.w.after_event(&mut self.s, self.now, &out, is_app);
        out
    }

    pub fn app(&mut self, id: u64, size: u64) {
        self.w.offer(id);
        self.ev(Event::AppData(OpaqueUnit { id, size }));
    }

    pub fn ack(&mut self, seqs: Vec<u64>) {
        self.ack_with_delay(seqs, Duration::ZERO);
    }

    pub fn ack_with_delay(&mut self, seqs: Vec<u64>, ack_delay: Duration) {
        let deleted = self.w.rt.on_ack(&seqs);
        self.ev(Event::AckReceived(AckInfo {
            acked_seqs: seqs,
            ack_delay,
        }));
        self.w.check_deletions(self.now, &deleted);
    }

    pub fn tick(&mut self) -> Vec<TransmitDecision> {
        self.ev(Event::TimerFired(Timer::Pto))
    }

    /// `now`'ı `step` adımlarla `t_end`'e dek ilerletir; her adımda `Timer::Pto`.
    pub fn pump_to(&mut self, t_end: u64, step: u64) {
        while self.now + step <= t_end {
            self.now += step;
            self.tick();
        }
    }

    pub fn data_seq(&self, unit: u64) -> Option<u64> {
        data_seq(&self.log, unit)
    }

    pub fn kind_seqs(&self, kind: TransmitKind) -> Vec<(u64, u64)> {
        self.log
            .iter()
            .filter(|e| e.d.kind == kind)
            .map(|e| (e.d.seq, e.d.unit_id))
            .collect()
    }

    pub fn unit_tx(&self, unit: u64) -> Vec<Emit> {
        unit_tx(&self.log, unit)
    }
}

// ════════════════════════════════════════════════════════════════════════════════════════════
// Ağ düzeneği: iki yönlü yol + gerçek Receiver (P0 `Sim` + gözlem + kesinti)
// ════════════════════════════════════════════════════════════════════════════════════════════

/// Tek yönlü yol: gönderim anına bağlı yayılım gecikmesi + isteğe bağlı FIFO darboğaz (P0).
pub struct Path {
    pub delay: fn(u64) -> u64,
    pub rate_bps: Option<u64>,
    free_at: u64,
}

impl Path {
    pub fn new(delay: fn(u64) -> u64, rate_bps: Option<u64>) -> Self {
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

pub fn one_way_fixed(_t: u64) -> u64 {
    ONE_WAY_NS
}

fn insert_sorted<T>(q: &mut VecDeque<(u64, u64, T)>, arr: u64, order: u64, item: T) {
    let pos = q.partition_point(|e| (e.0, e.1) <= (arr, order));
    q.insert(pos, (arr, order, item));
}

pub type DropRule = Box<dyn FnMut(&TransmitDecision) -> bool>;

fn in_window(w: Option<(u64, u64)>, t: u64) -> bool {
    w.is_some_and(|(a, b)| (a..b).contains(&t))
}

/// Ağ simülasyonu (P0). Tik sırası: (1) ileri yol varışları alıcıya, (2) alıcı ACK zamanlayıcısı,
/// (3) dönüş yolu varışları motora; ardından test uygulama olaylarını ve `timers()` ile ürün
/// pompasını sürer. Ek: `outage` [başlangıç, bitiş) penceresinde ileri yolda gönderim anı, dönüş
/// yolunda ACK üretim anı düşer; `ret_open` verilirse dönüş yolu yalnız açık anlarda taşır.
pub struct Sim {
    pub s: Sender,
    pub rx: Receiver,
    pub now: u64,
    pub step: u64,
    pub fwd: Path,
    pub ret: Path,
    fwd_q: VecDeque<(u64, u64, RxDatagram)>,
    ret_q: VecDeque<(u64, u64, AckFrame)>,
    order: u64,
    pub drop_fwd: DropRule,
    pub outage: Option<(u64, u64)>,
    pub ret_open: Option<fn(u64) -> bool>,
    pub log: Vec<Emit>,
    pub max_seq: Option<u64>,
    pub max_acked: Option<u64>,
    pub last_ack_at: Option<u64>,
    pub w: Watch,
}

impl Sim {
    pub fn new(s: Sender, step: u64, fwd: Path, ret: Path) -> Self {
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
            outage: None,
            ret_open: None,
            log: Vec::new(),
            max_seq: None,
            max_acked: None,
            last_ack_at: None,
            w: Watch::default(),
        }
    }

    pub fn drive(&mut self, ev: Event) {
        let is_app = matches!(ev, Event::AppData(_));
        let out = drive_sender(&mut self.s, ev, self.now, &mut self.log);
        self.w.after_event(&mut self.s, self.now, &out, is_app);
        for d in out {
            self.emit(d);
        }
    }

    fn emit(&mut self, d: TransmitDecision) {
        if !carries_unit(d.kind) && d.kind != TransmitKind::FecParity {
            return; // CsProbe: motor seq'i değil; ürün yolunda kapalı (runtime.rs:1313).
        }
        self.max_seq = Some(self.max_seq.map_or(d.seq, |m| m.max(d.seq)));
        if (self.drop_fwd)(&d) || in_window(self.outage, self.now) {
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

    pub fn app(&mut self, id: u64, size: u64) {
        self.w.offer(id);
        self.drive(Event::AppData(OpaqueUnit { id, size }));
    }

    /// Ürün pompası: her tikte `Timer::Pto`, 200 ms'de bir `Timer::RtpropProbe` (P0).
    pub fn timers(&mut self) {
        self.drive(Event::TimerFired(Timer::Pto));
        if self.now.is_multiple_of(RTPROP_TICK_NS) {
            self.drive(Event::TimerFired(Timer::RtpropProbe));
        }
    }

    fn send_ack(&mut self, a: AckFrame, t: u64) {
        if in_window(self.outage, t) || self.ret_open.is_some_and(|open| !open(t)) {
            return;
        }
        let arr = self.ret.arrival(t, SACK_WIRE_BYTES);
        self.order += 1;
        insert_sorted(&mut self.ret_q, arr, self.order, a);
    }

    pub fn advance(&mut self) {
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
                let deleted = self.w.rt.on_ack(&acked);
                self.drive(Event::AckReceived(AckInfo {
                    acked_seqs: acked,
                    ack_delay: a.ack_delay_duration(),
                }));
                self.w.check_deletions(now, &deleted);
            }
        }
    }

    pub fn unit_tx(&self, unit: u64) -> Vec<Emit> {
        unit_tx(&self.log, unit)
    }
}
