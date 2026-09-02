# DR-CS-1 (MINIMAL BOOTSTRAP v7) — DUR-SON RAPORU

**Tarih:** 2026-07-24 · **Branch:** `d1-sprint5c` · **PR:** #44 (DRAFT) · **Feature:** `experimental-
cold-start` (DEFAULT-OFF). **Dayanak:** Abkarada F2B-wire-codepoint-ONAYI (0x19/0x1A dar-amendman).

## HÜKÜM (iki-satır; karar-§8-tavanı gereği)

> **SENDER-SIDE COLD-START PROGRESS MECHANISM VALIDATED — PRODUCT RECEIVER PATIENCE CONTRACT
> UNIMPLEMENTED.**

Gerekçe (karar-§8 + F0-tespit-(ii)): M6 ürün-alıcı-patience-host'u YOK (per-blok-assembler inşa-
edilmemiş; patience YALNIZ harness-rx-loop'ta + spec-sözleşmede). Tam `COLD-START PROGRESS FIX
VALIDATED` hükmü, ürün-alıcı-patience-sözleşmesi uygulanmadan verilemez. Mekanizmanın SENDER-tarafı
(probe→echo→bootstrap→mevcut-PTO/ARQ) uçtan-uca-doğrulandı.

---

## SON-SUNUM (28 çıktı)

**1. Commit hash'leri:** spec=`666aa0c` · registry/codec=`f175a14` · F1=`4eca1ec` · F2A=`c0662f3` ·
F2B=`196bcd4` · F3=`ab8759a` · F0-doc=`27d7165` · durum=`03a722c` · DUR-SON=(bu).

**2. Commit-bazında dosyalar:** spec (kama-noise-wire-tdd §2.56/§2.57 + engine-CANCELLED) · schema
(inner_kind 0x19/0x1A + Control::CsProbe/CsEcho) · noise-wire (control-codec + retry + dr_cs1_wire.rs)
· sender.rs (Event::Tick/CsProbeEcho + on_event_batch + M2/M3/M5 + CsTimerKind/CsTerminal) · loss.rs
(next_time_loss_deadline_ns READ-ONLY) · udp_runner.rs (adapter + 0x19/0x1A + M6-patience) ·
dr_cs1_acceptance.rs (21-test) · bench (wildcard-arm).

**3. Production diff:** flag-OFF byte-for-byte (INV-CS-5 kimlik-kanıtlı). Spec-amendman DAR (0x19/0x1A
allowlist + layout; mevcut-codepoint anlam/layout/parser DEĞİŞMEDİ). Registry/codec-genişleme (yeni-
variant; mevcut-frame'ler byte-özdeş — g8-golden). **Not: bu, "üretim-wire-format-değişmez" kuralının
karar-§1-onaylı DAR-istisnasıdır.**

**4. Feature/default:** `experimental-cold-start` + `experimental-fec-budget` DEFAULT-OFF; default-flip
YAPILMADI. Flag yalnız-LOCAL.

**5. PR:** #44 DRAFT; merge YOK.

**6. Cold-start progress-invariant:** ilk-eligible-send-sonrası ACK-bağımsız-Duplicate ≤2×first_delay;
state-ilerleme ACK-kapılı (F0/FP). DR-CS-1 buna probe→echo-bootstrap-yolu-EKLER (srtt'nin ikinci-meşru-
kaynağı; §0.2-a).

**7. Cold-start senaryo-matrisi:** DI-1 (65/300/1200) · DI-1A · DI-1C · DI-2 · DI-4 · DI-8 · DI-11 +
F3: DI-3 · DI-7 · DI-12A-D + N8-spot + F1-atomiklik (DI-12E/F/G) + F2A (DI-1B/DI-6B). 21-in-process +
7-wire-codec + W-1/W-4-faithful-wire.

**8. First-source-loss:** DI-1 (in-process) + **W-1 (faithful-wire; RTT100): block-0 esi0-loss →
complete@5498ms** (probe→echo→bootstrap→ARQ; decode_via=0/repair_kind=§9-ARQ). BASELINE (FP flag-OFF):
90s-içinde-ASLA (rx-8s-terk). ⇒ **T_W1_BOUND=30s PRE-DECLARED; 5.5s ≪ 30s → W-1 PASS.**

**9. First-ACK-loss:** DI-2 (spurious-retransmit BEKLENEN; §2.4-a; induced-bytes-EXACT-1200; yön-teşhisi-
iddia-YOK).

**10. PTO arm/fire:** (d)-BootstrapPto — bootstrap-sonrası MEVCUT time-threshold makinesi (eşik-KURAL-
DEĞİŞMEDİ; loss.rs-READ-ONLY-türev) `next_wakeup_ns`'e-bağlı (EK4-A2 yaşam-döngüsü); fire=mevcut-Pto-
core-yolu (INV-CS-4 yeni-retransmit-yolu-YOK). C2: overdue-aynı-batch / exposed-deadline-Tick-only
(ek-network-event-YOK; DI-1/1A/1C).

**11. Pacing/dispatch:** probe emisyon-pass'i schedule-SONU (DATA-önce-mühürlü-sıra + TokenBucket
DOKUNULMADI); floor-31B/s'de-token-feasible (DI-6B).

**12. Cold-start-liveness-hükmü:** SENDER-SIDE VALIDATED (yukarıda).

**13. Faithful-harness-topolojisi:** two-netns rootless (FQ; §7-sıra-precondition).

**14. Env-manifest:** `data/dr-cs1/faithful-acceptance.txt` + FQ-ENV.

**15. RTT-acceptance:** 6/6 PASS (100/1000/2500 configured-exact; F4-önkoşul-KARŞILANDI).

**16. Forward/reverse-yön-pin:** FQ-acceptance (fwd-only/rev-only 501ms).

**17. Cleanup:** host-veth-sızıntı-YOK (FQ).

**18. Frozen-deadline (T_W1_BOUND):** PRE-DECLARED=30s (geometrik-kopya-only-yol-~61s'ten VE 90s-
timeout'tan-KÜÇÜK; formül-üstü-generous-tavan). Ölçülen-5.5s 6×-altında → post-hoc-fitting-kaygısı-yok.
**DÜRÜST-NOT:** ayrı-kalibrasyon-dosyası-hash+timestamp-ritüeli hafif-tutuldu (sonuç-bound'un-6×-altında;
tam-ritüel W-2/W-3-genişletmesinde).

**19. Registry-uniqueness (§6.1):** g1 — 0x19/0x1A benzersiz + 0x12=RECOVERED-kilit (runtime-assert-yaşıyor).

**20. Runtime-registry-assert (§6.2):** g2 — classify ≡ authoritative (256-uzay-tam-tarama: {0x01,0x02}∪
{0x10..0x1A}).

**21. Exact-encode/decode (§6.3):** g3 — golden-body + sealed-len PROBE=44B/ECHO=43B (ölçüm; encode-length-
testi) + roundtrip; g3b malformed/truncated/reserved-subtype/nonzero-pad/trailing → FAIL-CLOSED.

**22. Unknown-codepoint (§6.4):** g4 — 0x1B-reserved-reject + bozuk-frame-sonrası-session-YAŞAR (eski-peer-
safe-ignore-tabanı; wire-safety-PASS).

**23. Replay (§6.5):** g5 — pn-window-aynı-bytes-reject + control_id-dedup (Fresh→DuplicateAckOnly) +
RetryClass=NoRetry.

**24. Echo-rate-limit (§6.6):** harness T_ECHO_MIN=500ms + cs_echo_suppressed-sayaç (ledger-eşleşme).

**25. Accounting (§6.7 / C4):** cold_probe_tx_bytes (INV-CS-2 176B) · cold_echo_tx_bytes (alıcı-defteri;
W-1: 43B/echo) · bootstrap_induced_retransmit_bytes (DI-2: EXACT-1200; birim-başına-bir-kez).

**26. Sealed-control-compat (§6.8):** g8 — RECOVERED-golden DEĞİŞMEDİ + noise-wire-TAM-suite + integration
(RECOVERED==0x12-runtime-assert) + schema + engine YEŞİL.

**27. M5-terminal (INV-CS-3):** ResolvedByDataAck/ProbeEcho/TimedOut/PostTimeoutRecovery (epoch-bir-terminal
+ ayrı-PostTimeout; DI-3/7/12A-D). M6-patience: EK4-A1-formül (1200B>75s→S-A-105s; harness).

**28. Clean-link (W-4; spurious-maliyet):** RTT100 + RTT2500 temiz → **cs_probe_rx=0** (data-ACK<3s →
sıfır-spurious-probe; sıfır-maliyet). ⇒ mekanizma "gerektiğinde-yardım-eder (5.5s), gerekmeyince-bedava".

---

## KOŞULMAYAN / KISMİ (dürüstlük — sessiz-tavan-yok)

- **W-2 (exact-once ACK-drop wire)** ve **W-3 (blackout-heal 3-boyut wire)** ve **DI-2B (sustained-reverse-
  blackout wire)** ve **DI-10 (RX-cap wire)** ve **tam-mixed-version-wire-matrisi**: KOŞULMADI (mekanizma
  in-process-21-test + wire-codec-7-test + W-1-decisive + W-4-clean ile-doğrulandı; wire-matrisin-tamamı
  ayrı-koşu). Registry/parser mixed-version-SINIF-özelliği g4'te-kanıtlı (safe-ignore=frame-reject+session-
  yaşar).
- **T_W1_BOUND tam-kalibrasyon-freeze-ritüeli** (ayrı-dosya-hash): hafif (sonuç-6×-altında).
- **Ürün-alıcı-patience** (M6 Seçenek-2): harness+spec-sözleşme; ürün-host-YOK → hüküm-tavanı-SENDER-SIDE.

## DEĞİŞMEYENLER
bw.rs/BtlBw · send_rate-loss · INV-RATE-1 · TokenBucket · SACK-V2 · qd · ZAMAN-MODELİ · ARQ-kuralları/
eşikleri · geometrik-kopya · loss-ledger · ack_confidence · üretim-fuzzy-config · LeftoverOnly · mevcut-
codepoint-layout'ları — DOKUNULMADI. PR-draft; merge/default-flip YASAK.

## SONRAKİ (onaya-tabi)
W-2/W-3/DI-10/DI-2B/tam-mixed-version-wire-matrisi + ürün-alıcı-patience-implementasyonu (per-blok-
assembler) → tam `COLD-START PROGRESS FIX VALIDATED` hükmü. Ayrıca SENSOR-DR (srtt+deadline-slack;
FU-observability) bağımsız-bekliyor.
