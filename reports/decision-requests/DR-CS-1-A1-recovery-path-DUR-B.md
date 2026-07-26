# DUR-B — DR-CS-1 TUR-A1: RECOVERY-PATH PRE-REGISTRATION ÇELİŞKİSİ (W-2B + W-3-65)

**Tip:** DUR-B (bekle-onay) · **Tarih:** 2026-07-26 · **Faz:** F-A1-1/F-A1-2 arası · **Branch:** `d1-sprint5c`
**Dayanak:** TUR-A1 talimatı v2 (`docs/tasks/dr-cs-1-tur-a1-talimati-v2.md`) + pre-registration
(`reports/d1-exit/DR-CS-1-A1-pre-registration.md`, anchor `a635efa`). **Production-code-SIFIR** korundu.

> **ÇERÇEVE:** ARQ tüm kollarda aktiftir; DR-CS-1 cold-start LIVENESS DR'ı. Bu DUR-B bir ölçüm-bulgusudur.

---

## 0. ÖZET
F-A1-1 harness'ı tamamlandı (`9cba9cf`; test-only) ve iki hücrenin İZOLE-doğrulaması, **pre-registered
beklenen-zincirlerle ÇELİŞEN** tutarlı bir mekanizma-bulgusu ortaya çıkardı. §5 gereği sonuca-bakıp-beklenti-
değiştirmek YASAK; §0.1 gereği mekanizma-yeniden-yorumu Abkarada-kararı ister → **DUR-B**.

**Kök bulgu (her iki hücrede ortak):** Küçük-workload + bastırılmış-feedback rejiminde motor, kurtarımı
**§11 forward-recovery + RECOVERED-control (ACK-BAĞIMSIZ)** yoluyla yapar; pre-registered varsayılan
**§9-ARQ-retransmit + DATA-ACK** yolunu KULLANMAZ.

---

## 1. BULGU-A — W-2B: retransmit_path = ForwardRecoveryDup (ExistingPtoArq DEĞİL)

**İzole koşu (faithful; RTT100/65B/w8; --suppress-data-acks + --no-settle-exit):**
```
w2b_transport_duplicate_rx_count = 1       (bir re-send ulaştı)
w2b_retransmit_rx_first          = 16194ms#unit0
w2b_pto_wire                     = none    (§9-ARQ Retransmit HİÇ ateşlemedi)
w2b_dup_wire                     = 16195ms#unit0   (§11 Duplicate)
w2b_retransmit_path              = ForwardRecoveryDup     ← ÖLÇÜLEN (varsayım-değil)
w2b_srtt_source                  = ProbeEcho
w2b_ack_accepted_before_bootstrap_count = 0
w2b_application_delivery_count    = 8
w2b_application_duplicate_delivery_count = 0    (dedup temiz)
finalized_blocks                 = 1            (self-heal OLDU)
```

**Mekanistik gerekçe (DETERMİNİSTİK, boyuttan-bağımsız):** §9-ARQ retransmit **SACK-REAKTİFTİR** (SACK-gap/
NACK ile tetiklenir). W-2B TÜM DATA-ACK/SACK'i bastırdığı için §9-ARQ **ATEŞLEYEMEZ**. Geriye tek re-send
yolu §11 forward-recovery (ACK-bağımsız, time-threshold loss-deadline ile) kalır → 16.2s'te (cadence-floor
W_loss≈16.8s) Duplicate. **Pre-registered `retransmit_path = ExistingPtoArq` bu senaryoda MEKANİSTİK-İMKÂNSIZ.**

**Pre-registration çelişkisi:** talimat §2.4 `retransmit_path = ExistingPtoArq` + "pto_wire sender-kanıtı"
(§4 F-A1-4-ii). Ölçüm: pto_wire=none, path=ForwardRecoveryDup. Zincir olduğu-gibi KAPANAMAZ.

---

## 2. BULGU-B — W-3-65: heal OLUYOR ama PostTimeoutRecovery ATEŞLEMİYOR

**İzole koşu (faithful; restore=61, --no-settle-exit, OBSERVATION_HORIZON=120s, RX_TEST_PATIENCE=130s):**
```
receiver_events  = 9
finalized_blocks = 1        ← BLOK HEAL OLDU (F4'ün UNOBSERVED'i ÇÖZÜLDÜ; settle-fix işe yaradı)
pending_blocks   = 0
cs_terminals     = TimedOut@60000ms    ← PostTimeoutRecovery YOK
```

**Karşılaştırma (F4 verisi):** 300B/1200B restore=61 → PostTimeoutRecovery@63.9s/83.2s (ATEŞLEDİ). 65B → YOK.
**Gerekçe:** 65B tüm-pencere cwnd'e-sığar → restore-sonrası tek-burst heal, **ACK-bağımsız** forward-recovery/
RECOVERED yoluyla tamamlanır → tx'e DATA-ACK ulaşmadan blok kapanır → PostTimeoutRecovery (DATA-ACK-tetikli)
ateşlemez. 300/1200B ACK-clocked heal → DATA-ACK → PostTimeoutRecovery.

**Pre-registration çelişkisi:** talimat §1 + pre-reg §2 beklenen-zincir "...→ DATA-ACK → PostTimeoutRecovery".
Ölçüm: heal-var (finalized=1) ama terminal=TimedOut (PostTimeoutRecovery-yok). Zincir olduğu-gibi 3/3-PASS-veremez.

---

## 3. ORTAK KÖK-NEDEN
Her iki pre-registered zincir de motorun kurtarımı **§9-ARQ-retransmit + DATA-ACK** ile yaptığını VARSAYDI.
Ölçüm: bu rejimlerde motor **§11 forward-recovery + RECOVERED-control (ACK-bağımsız)** kullanıyor. Bu, önceki
mimari-bulguyla tutarlı (M1: "RECOVER_FWD-dup"; [[kama-vs-quinn-v1]] çevresi). **Motor-defekti-DEĞİL** — sağlıklı
ACK-bağımsız kurtarım; ama pre-registered ARQ-ACK-yolu-varsayımı bu rejimde geçersiz.

---

## 4. KARAR SEÇENEKLERİ (Abkarada; §0.1 en-küçük-amendman)

**W-2B için:**
- **A1 (öneri):** `retransmit_path` beklentisini `ExistingPtoArq`'tan **ÖLÇÜLEN-path**'e amend et (bu senaryoda
  ForwardRecoveryDup). W-2B amacı (self-heal + dedup + spurious-cost ÖLÇÜMÜ) KORUNUR; yalnız path-etiketi
  ölçüme-uyar. Yeni pre-registration (yeni hash+timestamp) + 27-koşu path-dağılımını RAPORLAR.
- **A2:** senaryoyu §9-ARQ'yu tetikleyecek şekilde revize et (kısmi-suppression: SACK-gap bırak → reaktif-ARQ
  ateşler). Ama bu "TAM ACK-starvation" niteliğini değiştirir (farklı deney).
- **A3:** W-2B'yi "ACK-starvation altında kurtarım-yolu KARAKTERİZASYONU" olarak yeniden-çerçevele (path'i
  iddia-etmeden ölç); FULL yerine "CHARACTERIZED" hükmü.

**W-3-65 için:**
- **B1 (öneri):** beklenen-terminal'i "heal-gözlendi (finalized=1)" olarak amend et; PostTimeoutRecovery'yi
  ACK-clocked-rejim (300/1200B) için TUT, 65B için "ACK-bağımsız-heal → terminal=TimedOut + finalized=1"
  kabul-et. Kabul-kriteri: finalized=1 (heal) + no-busy-loop + post-condition. (in-process F3-D PostTimeoutRecovery
  yolunu zaten pinliyor; bu faithful-65B-rejimi ayrı-karakterde.)
- **B2:** forward-recovery'yi kapat (test-only) → ARQ+ACK heal'i zorla → PostTimeoutRecovery ateşler. Ama bu
  motorun-doğal-yolunu maskeler (daha-az-faithful).

---

## 5. DUR-B TALEBİ
Yukarıdaki A/B seçenekleri Abkarada-kararı bekliyor. **Onaysız devam YOK** (§5: sonuca-bakıp-beklenti-
değiştirmek YASAK). Karar gelince: yeni pre-registration (gerekirse) + F-A1-2 tam-matris (W-3-65 3-rep +
W-2B 27-koşu) + F-A1-3/4/5. Harness HAZIR (`9cba9cf`); yalnız beklenti-çerçevesi karara bağlı.

**Not:** W-3-65 harness-fix'i (--no-settle-exit + 120s) F4'ün UNOBSERVED'ini teknik-olarak ÇÖZDÜ (finalized=1);
açık-olan yalnız terminal-sınıfı-beklentisi (B1/B2).
