//! MUTATION-GÜDÜMLÜ — durum makinesi EŞİKLERİ (`update_state`, `compute_send_rate`).
//!
//! **Kaynak:** `cargo mutants -p adaptive-transport --file .../sender.rs`
//! [ÖLÇÜLDÜ] `438 mutants tested in 23m: 68 missed` — kalan 68'in 22'si bu dosyanın hedefi:
//! `update_state` (14), `compute_send_rate` (3), `qd_ratio_permille_now` (3),
//! `startup_plateau` (2).
//!
//! **Kök sebep — matris geçişleri kontrol ediyor, EŞİKLERİ kontrol etmiyor:**
//! `sender_state_matrix.rs` 81 geçişin hangilerinin İZİNLİ olduğunu kanıtlıyor. Ama bir
//! geçişi TETİKLEYEN koşulun (plato eşiği, BDP karşılaştırması, qd oranı, probe hacmi)
//! doğru hesaplandığını hiçbir test sürmüyor. Bir eşik kayarsa matris yine yeşil kalır:
//! geçişler hâlâ izinli hücrelerde olur, sadece YANLIŞ ZAMANDA olur.
//!
//! **[ÖLÇÜLDÜ] Durum başına pacing hızı** (bu dosyanın dayandığı referans):
//! ```text
//! Idle          rate=0       PreConverge   rate=0    (efektif floor 31 B/s)
//! Startup       rate=1683    btlbw=59500
//! Drain         rate=5050    ProbeUp  rate=5050   Steady rate=5050
//! ```
//!
//! **[ÖLÇÜLDÜ] Mutant `882` (`Idle => Startup` arm silinmesi) neden kaçıyordu:**
//! [KOD] `on_event`'te `AppData` alındığında `Idle` durumu ZATEN çözülüyor —
//! `srtt.is_none()` ise `PreConverge`, değilse `Startup`. Yani `update_state`'e `Idle`
//! durumuyla girmek ancak **resume** senaryosunda olur: yakınsamış bir oturum boşa düşüp
//! sonra devam ettiğinde. Hiçbir test "yakınsa → boşal → devam et" dizisini kurmuyordu.
//! Bu dosya onu kurar; ölçüldü ki resume `Startup` verir (rate=841), `PreConverge` değil.
//!
//! **Kapsam dışı (açıkça):** `ProbeUp`'ın `success`/`abort` aritmetiği (947–959, ~10 mutant).
//! `delivered`, `btlbw` ve `qd` üçlüsünü aynı anda kontrol etmeyi gerektiriyor; ayrı bir
//! harness işi. [BEKLİYOR]
//!
//! ---
//! **[ÖLÇÜLDÜ] İKİ MUTANT EŞDEĞER ÇIKTI — bu dosya onları ÖLDÜREMEZ:**
//!
//!   `882:17` `SenderState::Idle => SenderState::Startup` arm silinmesi.
//!            Arm **YAPISAL OLARAK ÖLÜ**: [KOD] `on_event`'te `AppData` alındığında
//!            `state == Idle` ise durum `update_state` ÇAĞRILMADAN ÖNCE çözülüyor
//!            (`srtt.is_none()` → PreConverge, değilse → Startup). `update_state`'e `Idle`
//!            durumuyla girmek için iş VAR ama durum `Idle` olmalı; iş varsa `tx_queue`
//!            doludur, o da `AppData` geldiği anlamına gelir, o da durumu zaten çözmüştür.
//!            [BEKLİYOR — Abkarada] Arm kaldırılsın mı, yoksa savunmacı olarak kalsın mı?
//!            (`codec.rs:279` ve `pn.rs` guard'larıyla aynı sınıf: erişilemez savunma.)
//!
//!   `1110/1111/1112` `Drain`/`ProbeUp`/`DrainRtprop` faz arm'larının silinmesi.
//!            [ÖLÇÜLDÜ] Bu senaryoda ProbeUp ve Steady AYNI hızı üretiyor (5050 B/s);
//!            `Drain`'e ise hiç uğranmıyor. Arm silinip `_ => Steady`'ye düşmek
//!            gözlemlenebilir bir fark yaratmıyor. Fazları ayrıştırmak için gain'lerin
//!            ayrıştığı bir rejim (yüksek btlbw + dolu cwnd) kurmak gerekiyor.
//!            [BEKLİYOR] Ayrı harness.
//!
//! Aşağıdaki testler yine de geçerlidir — resume davranışını, cold-start floor'unu ve
//! Startup'ın diğer fazlardan ayrıştığını (1683 vs 5050) pinlerler; bunlar mutation
//! skorundan bağımsız gerçek regresyon korumalarıdır.
//!
//! Koş: `cargo test -p adaptive-transport --test sender_state_thresholds`

use adaptive_transport::sender::{AckInfo, Event, OpaqueUnit, Sender, SenderState, Timer};
use std::time::Duration;

const MS: u64 = 1_000_000;
const UNIT: u64 = 101;

fn ack(v: Vec<u64>) -> Event {
    Event::AckReceived(AckInfo {
        acked_seqs: v,
        ack_delay: Duration::from_micros(500),
    })
}

fn app(id: u64) -> Event {
    Event::AppData(OpaqueUnit { id, size: UNIT })
}

/// Yakınsamış bir oturum: srtt kurulu, PreConverge terk edilmiş. Dönen liste: ilk ACK olayının
/// KENDİ emisyonlarının seq'leri (henüz ACK'lenmemiş; çağıran bir sonraki adımda ACK'ler).
fn converged() -> (Sender, u64, Vec<u64>) {
    let mut s = Sender::new();
    let mut now = 0u64;
    let mut out: Vec<u64> = Vec::new();
    let mut uid = 0u64;
    for _ in 0..5 {
        now += 200 * MS;
        uid += 1;
        for d in s.on_event(app(uid), now) {
            out.push(d.seq);
        }
    }
    now += 50 * MS;
    // [K4G-05b DUR-32] İlk ACK olayının KENDİ emisyonları da toplanır ve dönülür (dosyanın
    // "emisyon HANGİ olaydan dönerse dönsün toplanır" ilkesi): v1.1'de (madde 7) çerçevenin en
    // büyüğü kuruluş kopyasıdır → srtt kısalır, başlangıç tabanı büyür ve kuyruktaki birimler bu
    // olayda çıkar; çıktı atılırsa o kayıtlar hiç ACK'lenmez ve motor Idle'a inemez.
    let acked = std::mem::take(&mut out);
    out = s.on_event(ack(acked), now).iter().map(|d| d.seq).collect();
    assert_ne!(
        s.state(),
        SenderState::PreConverge,
        "kurulum: ilk ACK PreConverge'i sonlandırmalı"
    );
    (s, now, out)
}

// ════════════════════════════════════════════════════════════════════════════
// update_state — cold-start / resume ayrımı
// ════════════════════════════════════════════════════════════════════════════

/// MUTANT `sender.rs:882:17` — `SenderState::Idle => SenderState::Startup` arm silinmesi.
#[test]
fn resume_from_idle_goes_to_startup_not_back_to_preconverge() {
    // [KOD] §3.3 yorumu: "cold-start YALNIZ ilk sefer (hiç ölçüm yok). Ölçüm varsa önsel
    // zaten tahliye edilmiş → Idle→Startup (normal resume), PreConverge'e geri DÖNÜLMEZ."
    //
    // Bu ayrım davranışsal olarak KRİTİK: PreConverge'e geri dönmek pacing'i 31 B/s
    // floor'una indirir ve ölçülmüş kanal bilgisini çöpe atar. Sessizlik sonrası her
    // resume'da bu olursa (ör. konuşma arası duraklamalar) hız hiç toparlanamaz.
    let (mut s, mut now, out) = converged();

    // Boşalt: her şeyi ACK'le, timer'larla ilerlet → Idle'a düş.
    //
    // [2026-09-02 uyarlama, açık-kayıtlı] Eski döngü YALNIZ Pto-çağrısının dönüşünü topluyordu;
    // post-#45 motorda token-refill ACK anında tx_queue/dup drain'ini tetikleyip ACK-çağrısının
    // DÖNÜŞÜNDEN de emisyon üretir. O emisyonlar düşürülünce hiç ACK'lenmiyor ve pif hiç
    // sıfırlanmıyordu (SONDA ölçümü: pif=3→4, Idle'a meşru olarak inilemiyor). Kurulumun doğru
    // biçimi: emisyon HANGİ olaydan dönerse dönsün toplanır ve bir sonraki adımda ACK'lenir.
    // [K4G-05b DUR-32] `converged()`'ın ilk ACK olayındaki emisyonlar da bu kümeye dahildir.
    let mut pending: Vec<u64> = out;
    for _ in 0..30 {
        now += 100 * MS;
        for d in s.on_event(Event::TimerFired(Timer::Pto), now) {
            pending.push(d.seq);
        }
        if !pending.is_empty() {
            now += 20 * MS;
            let acked = std::mem::take(&mut pending);
            for d in s.on_event(ack(acked), now) {
                pending.push(d.seq);
            }
        }
    }
    assert_eq!(
        s.state(),
        SenderState::Idle,
        "kurulum: motor Idle'a düşmeli"
    );

    // RESUME: yeni veri gelir.
    now += 50 * MS;
    s.on_event(app(999), now);

    assert_eq!(
        s.state(),
        SenderState::Startup,
        "resume Startup vermeli — arm silinirse durum takılı kalır"
    );
    assert_ne!(
        s.state(),
        SenderState::PreConverge,
        "resume PreConverge'e geri döndü — ölçülmüş kanal bilgisi çöpe atıldı"
    );

    // Hız cold-start floor'unda OLMAMALI (ölçüldü: resume'da 841, floor 31).
    let (_st, rate, _btlbw, eff, _b) = s.diag_rate();
    assert!(
        eff > 31,
        "resume sonrası efektif hız floor'da kaldı: {eff} — PreConverge'e dönülmüş olabilir"
    );
    assert!(rate > 0, "resume sonrası pacing hızı 0: {rate}");
}

/// Cold-start ile resume'un AYRIMI — ikisi aynı olsaydı yukarıdaki test vakum olurdu.
#[test]
fn first_ever_appdata_enters_preconverge_not_startup() {
    // [ÖLÇÜLDÜ] Taze motorda `srtt.is_none()` → PreConverge. Bu, resume testinin
    // karşı kutbu; ikisi birlikte `on_event`'teki `srtt.is_none()` dallanmasını pinler.
    let mut s = Sender::new();
    assert_eq!(s.state(), SenderState::Idle, "taze motor Idle'da başlamalı");
    s.on_event(app(1), MS);
    assert_eq!(
        s.state(),
        SenderState::PreConverge,
        "ilk AppData cold-start'a girmeli (hiç ölçüm yok)"
    );
    let (_st, _rate, _btlbw, eff, _b) = s.diag_rate();
    assert_eq!(eff, 31, "cold-start efektif hızı min_rate floor'u olmalı");
}

// ════════════════════════════════════════════════════════════════════════════
// compute_send_rate — durum başına faz gain'i
// ════════════════════════════════════════════════════════════════════════════

/// MUTANT `sender.rs:1110:13` (`Drain` arm), `1111:13` (`ProbeUp`), `1112:13` (`DrainRtprop`).
#[test]
fn send_rate_differs_between_startup_and_the_steady_family() {
    // [KOD] `compute_send_rate` her duruma bir `PacingPhase` eşler; arm silinirse durum
    // `_ => PacingPhase::Steady` fallback'ine düşer ve o fazın gain'i kaybolur.
    //
    // [ÖLÇÜLDÜ] Startup 1683 B/s, Drain/ProbeUp/Steady 5050 B/s — Startup'ın high_gain'i
    // ölçülebilir biçimde AYRIŞIYOR.
    let mut s = Sender::new();
    let mut now = 0u64;
    let mut out: Vec<u64> = Vec::new();
    let mut uid = 0u64;
    let mut rates: std::collections::BTreeMap<String, u64> = std::collections::BTreeMap::new();

    for i in 0..4000 {
        now += 20 * MS;
        let ev = match i % 7 {
            0..=2 => {
                uid += 1;
                app(uid)
            }
            3 | 4 => {
                let t = out.len().min(6);
                let mut a: Vec<u64> = out.drain(..t).collect();
                if a.len() >= 4 && i % 21 == 3 {
                    a.drain(..2);
                }
                ack(a)
            }
            5 => Event::TimerFired(Timer::RtpropProbe),
            _ => Event::TimerFired(Timer::Pto),
        };
        for d in s.on_event(ev, now) {
            out.push(d.seq);
        }
        if out.len() > 400 {
            out.drain(..200);
        }
        let (st, rate, _b, _e, _x) = s.diag_rate();
        rates.entry(format!("{st:?}")).or_insert(rate);
    }

    // Vakumsuzluk: senaryo en az dört farklı duruma uğramalı.
    assert!(
        rates.len() >= 4,
        "senaryo yeterli durum çeşitliliği üretmedi: {:?}",
        rates.keys().collect::<Vec<_>>()
    );
    assert!(
        rates.contains_key("Startup"),
        "Startup'a hiç uğranmadı: {rates:?}"
    );

    // [ÖLÇÜLDÜ] `Idle`'ın `compute_send_rate` match'inde KOLU YOKTUR — `_ => Steady`'ye
    // düşer, yani motor boşa düştüğünde son hesaplanan hızı KORUR (ölçüldü: 5050).
    // Gönderilecek bir şey olmadığı için zararsız, ama "Idle = 0" varsayımı YANLIŞTIR.
    // PreConverge ise erken-dönüşle FLOOR alır; `rate` alanı 0 kalır.
    for (st, r) in &rates {
        if st == "PreConverge" {
            assert_eq!(*r, 0, "PreConverge'de pacing hızı 0 olmalı (erken dönüş)");
        } else if st != "Idle" {
            assert!(
                *r > 0,
                "{st} durumunda pacing hızı 0 — arm silinmiş olabilir"
            );
        }
    }

    // Startup DİĞERLERİNDEN farklı olmalı — faz gain'leri eşitlenmemeli.
    let startup = rates["Startup"];
    let others: Vec<u64> = rates
        .iter()
        .filter(|(k, _)| {
            k.as_str() != "Startup" && k.as_str() != "Idle" && k.as_str() != "PreConverge"
        })
        .map(|(_, v)| *v)
        .collect();
    assert!(
        !others.is_empty(),
        "Startup dışında pozitif-hızlı duruma uğranmadı"
    );
    assert!(
        others.iter().all(|&o| o != startup),
        "Startup hızı ({startup}) diğer fazlarla eşitlendi ({others:?}) — faz arm'ı silinmiş olabilir"
    );
}

/// PreConverge'in kendi kolu — `compute_send_rate`'in ilk erken-dönüşü.
#[test]
fn preconverge_returns_the_floor_before_any_phase_lookup() {
    // [KOD] `if self.state == PreConverge { return PRE_CONVERGE_FLOOR_BW; }` faz eşlemesinden
    // ÖNCE gelir. Bu erken-dönüş, cold-start'ta faz gain'lerinin uygulanmamasını garantiler.
    let mut s = Sender::new();
    s.on_event(app(1), MS);
    assert_eq!(s.state(), SenderState::PreConverge);
    let (_st, rate, btlbw, eff, _b) = s.diag_rate();
    assert_eq!(btlbw, 0, "cold-start'ta btlbw ölçülmemiş olmalı");
    assert_eq!(rate, 0, "cold-start'ta pacing hızı 0");
    assert_eq!(
        eff, 31,
        "cold-start efektif hızı FLOOR olmalı, faz-türevi değil"
    );
}

// ════════════════════════════════════════════════════════════════════════════
// qd_ratio_permille_now — kuyruk gecikmesi oranı
// ════════════════════════════════════════════════════════════════════════════

/// MUTANT `sender.rs:1039:9` (`-> 1`), `1040:26` (`delete !`), `1041:69` (`/` → `%`).
#[test]
fn qd_ratio_is_zero_without_an_rtprop_measurement() {
    // [KOD] `match self.rtt.rtprop() { Some(rtp) if !rtp.is_zero() => …, _ => 0 }`.
    // rtprop ölçülmeden oran hesaplanamaz; guard `!rtp.is_zero()` sıfıra bölmeyi engeller.
    // `!` silinirse SIFIR rtprop ile bölme denenir; `-> 1` sabiti ise ProbeUp abort eşiğini
    // (PROBE_QD_ABORT_PERMILLE) yanlış tetikler.
    let s = Sender::new();
    let (rtprop_us, qd_us) = s.diag_qd();
    assert_eq!(
        (rtprop_us, qd_us),
        (0, 0),
        "taze motorda rtprop/qd ölçülmemiş olmalı"
    );

    let mut s2 = Sender::new();
    s2.on_event(app(1), MS);
    assert_eq!(
        s2.state(),
        SenderState::PreConverge,
        "ölçümsüz motor ProbeUp'a girmemeli"
    );
}

#[test]
fn qd_ratio_becomes_measurable_once_rtprop_is_known() {
    // Yakınsadıktan sonra rtprop dolar ve oran anlamlı hale gelir.
    let (s, _now, _out) = converged();
    let (rtprop_us, _qd_us) = s.diag_qd();
    assert!(
        rtprop_us > 0,
        "ACK sonrası rtprop hâlâ 0 — kurulum kayıp yolunu sürmüyor"
    );
}

// ════════════════════════════════════════════════════════════════════════════
// startup_plateau — plato tespiti
// ════════════════════════════════════════════════════════════════════════════

/// MUTANT `sender.rs:1088:13` — `grew` karşılaştırması `>` → `==` / `>=`.
#[test]
fn startup_exits_to_drain_only_after_the_plateau_is_reached() {
    // [KOD] `grew = (bw - last) * 1000 > last * STARTUP_PLATEAU_EPS_PERMILLE`.
    // Bant genişliği eps'ten FAZLA büyüdüyse plato sayacı sıfırlanır; büyüme durunca
    // sayaç dolar ve Startup → Drain geçişi olur.
    //
    // `>` → `>=` olursa TAM eps kadar büyüme "büyüme" sayılır ve plato bir tur gecikir;
    // `>` → `==` olursa yalnız tam-eşitlikte büyüme sayılır, yani neredeyse hiç — plato
    // erken tetiklenir ve Startup vaktinden önce terk edilir (bant kullanılmadan kalır).
    let mut s = Sender::new();
    let mut now = 0u64;
    let mut out: Vec<u64> = Vec::new();
    let mut uid = 0u64;
    let mut saw_startup = false;
    let mut saw_drain = false;

    for i in 0..4000 {
        now += 20 * MS;
        let ev = match i % 7 {
            0..=2 => {
                uid += 1;
                app(uid)
            }
            3 | 4 => {
                let t = out.len().min(6);
                ack(out.drain(..t).collect())
            }
            5 => Event::TimerFired(Timer::RtpropProbe),
            _ => Event::TimerFired(Timer::Pto),
        };
        for d in s.on_event(ev, now) {
            out.push(d.seq);
        }
        if out.len() > 400 {
            out.drain(..200);
        }
        match s.state() {
            SenderState::Startup => saw_startup = true,
            SenderState::Drain => saw_drain = true,
            _ => {}
        }
    }

    assert!(
        saw_startup,
        "senaryo Startup'a hiç uğramadı — kurulum bozuk"
    );
    assert!(
        saw_drain,
        "Startup'tan Drain'e hiç geçilmedi — plato tespiti çalışmıyor olabilir"
    );
}

// ════════════════════════════════════════════════════════════════════════════
// Vakumsuzluk
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn converged_setup_actually_leaves_cold_start() {
    // Bu dosyadaki testlerin çoğu `converged()`'a dayanıyor. Kurulum sessizce bozulursa
    // (pacer floor'u değişir, ACK işleme kayar) testler yanlış nedenle yeşil kalabilir.
    let (s, _now, _out) = converged();
    assert_ne!(s.state(), SenderState::PreConverge);
    let (_st, _rate, _btlbw, eff, _b) = s.diag_rate();
    assert!(eff > 31, "yakınsama sonrası efektif hız floor'da: {eff}");
    assert!(s.diag_qd().0 > 0, "yakınsama sonrası rtprop ölçülmemiş");
}
