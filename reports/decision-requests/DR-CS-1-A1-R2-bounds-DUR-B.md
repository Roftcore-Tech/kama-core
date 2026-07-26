# DUR-B — W-2B-R2 [CAL]-BOUNDS UYGULANABİLİRLİĞİ (CAP=64 / T_MAX=30s)

**Tip:** DUR-B (bekle-onay; dar) · **Tarih:** 2026-07-26 · **Faz:** R2-harness-doğrulama sonrası · **Branch:** `d1-sprint5c`
**Dayanak:** Abkarada R2-kararı (A3+B1) + R2 pre-registration (anchor `fe119b4`). **Production-code-SIFIR** korundu.

> **ÇERÇEVE:** ARQ tüm kollarda; DR-CS-1 cold-start LIVENESS. R2 harness (`baea738`) ON/OFF-matched ledger'ı ölçüyor.

---

## 0. ÖZET
R2-harness İZOLE-doğrulaması, R2 pre-reg'e devraldığım eski-W-2B [CAL]-bound'larının (**CAP=64**, **T_MAX=30s**)
R2'nin AMACIYLA (doğal-production-recovery-path'i ölçmek) **MEKANİSTİK-ÇELİŞTİĞİNİ** gösterdi. §5 gereği donmuş
pre-reg'i tek-taraflı değiştiremem → dar-DUR-B.

## 1. ÖLÇÜLEN (izole harness-doğrulama; ON, RTT100, w-units=8, suppress_data_acks + no-settle-exit)
Full-ACK-starvation'da doğal-recovery = **YAVAŞ FLOOR-RATE byte-tabanlı dispatch** (BtlBw hiç-ilklenmez —
ACK-gerektirir; cold-start-bootstrap YALNIZ srtt-kurar, BtlBw-DEĞİL):
```
size 65B   → completion=14.1s  · recovery_path=FirstFlightNoRecovery · spurious-dup(§11)@~16s
size 300B  → completion=61.2s  · recovery_path=FirstFlightNoRecovery
size 1200B → NoRecovery (90s-ufukta tamamlanmadı; floor-rate ~250s-gerektirir)
```
**Cold-start marjinal-etkisi: NÖTR** (ON 14.1s ≈ OFF 14.7s @65B; ikisi de floor-rate; BtlBw-yok).

## 2. ÇELİŞKİ
- **CAP=64:** 64 bastırılmış-SACK @50ms-cadence = **~3.2s**'te suppression'ı-bırakır → sender ACK-alır →
  BtlBw-ilklenir → blok HIZLI-tamamlanır. Bu **"FULL ACK-STARVATION"-DEĞİL**, ~3.2s-partial-starvation
  (≈ W-2C, Abkarada'nın-AYRI/opsiyonel-bıraktığı). CAP=64 doğal-recovery'yi KESER.
- **T_MAX=30s:** 300B-completion (61s) > 30s → T_MAX'i-zorlarsam 300B "bounded-out" olur; oysa doğal-recovery
  61s'te GERÇEKLEŞİR (ölçülebilir).
- Bu bound'lar eski-W-2B-hipotezi (probe→echo→PTO≤13s) için tasarlanmıştı — o hipotez ÇÜRÜDÜ (ForwardRecoveryDup
  DUR-B). Yeni-recovery YAVAŞ olduğundan bound'lar artık uygunsuz.

## 3. KARAR SEÇENEKLERİ (Abkarada; en-küçük-amendman)
- **A (öneri):** R2 için CAP=64/T_MAX=30s'i "release+FAIL" olarak UYGULAMA (eski-W-2B-artefaktı). R2-karakterizasyonu:
  arrival-triggered-release (doğal) + **boyut-ölçekli OBSERVATION-horizon** (65B:25s · 300B:80s · 1200B:45s-NoRecovery).
  `suppressed_ack_count` + `completion_time` METRİK olarak raporlanır (FAIL-değil). suppressed_ack büyük-olabilir
  (uzun-suppression-doğal-maliyeti; bounded=horizon). Bu, Abkarada'nın "doğal-recovery'yi-ölç" niyetine UYAR.
- **B:** CAP=64'ü TUT → ~3.2s-partial-starvation → farklı-senaryo (≈W-2C). "Full starvation" DEĞİL.
- **C:** CAP'i saf-güvenlik-bound'u yap (ör. observation-horizon-eşdeğeri; erken-release-YOK); T_MAX yerine
  boyut-ölçekli-observation-horizon. (A'nın-varyantı; CAP-safety-korunur ama recovery-kesilmez.)

## 4. RUNTIME NOTU (bilgi)
54-koşu × boyut-ölçekli-horizon (25+80+45s) × 3RTT × 2feature × 3rep ≈ **46 dk** (+W-3-65 ~6dk +drain). Kabul
edilirse arka-planda-koşulur.

## 5. DUR-B TALEBİ
A/B/C kararı bekliyor. Onaysız R2-matris KOŞULMAZ (§5). Harness HAZIR (`baea738`; ON/OFF-ledger-doğrulandı);
yalnız bound-çerçevesi karara-bağlı. W-3-65 (B1; bu-çelişkiden-BAĞIMSIZ) karar-beklerken koşulabilir mi — Abkarada.
