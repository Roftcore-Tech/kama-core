# A2-F5 PRE-REGISTRATION — PLATEAU + PRODUCT W-3-65 RERUN [FROZEN]

**Statü:** Koşulardan ÖNCE dondurulur (Abkarada full-scale-directive §11; §5-disiplini). Test-matrisi,
tekrar, flood-miktarları, beklenen-zincirler, plateau-sınırları hash+timestamp ile sabit; sonuca-göre
DEĞİŞTİRİLMEZ. Testler deterministik-birim (gerçek ReceiverHost + gerçek Sender; netem AYRI —
self-hosted-nightly). [CAL]-SEALED sınırlar freeze-r2 + CAL-10.

## 1. PLATEAU (deterministik defter/sayaç-assert; RSS-model-tabanı)

```text
F-A PROBE-FLOOD:
  girdi:   N_flood = 100_000 probe (aynı+farklı probe_seq)
  beklenti: cold-ledger-delta = 0 (probe cold-deftere DOKUNMAZ) · working-set SABİT
            (cs_echo_count = N_flood; retained_bytes = 0)

F-B COLD-BLOCK-FLOOD:
  girdi:   çok-bloklu authenticated-shard seli (K'ya-ULAŞMAYAN incomplete bloklar)
  beklenti: session cold-retained ≤ 480 KiB (RX_COLD_RETAINED_BYTE_CAP_PER_SESSION) ·
            global cold-retained ≤ 15 MiB (GLOBAL_COLD_BYTE_CAP) · typed-reject-sayacı > 0
            (cap-aşımı) · plateau (defter-sayaç == model-tavanı; büyüme-durur)

F-C CONTROL-FLOOD:
  girdi:   N_flood = 100_000 farklı-control-id (ileri + geri/window-altı)
  beklenti: dedup-bitmap SABİT (ControlDedupWindow O(1) working-set; büyüme=0) ·
            below_window_count > 0 (window-altı görünür) · tombstone ≤ 64

TOPLAM-INVARIANT: cold-retained + bounded-side-state ≤ 16 MiB (her-t'de).
```

## 2. PRODUCT W-3-65 RERUN (gerçek ReceiverHost + Sender; ≥3 tekrar; B1-amendmanlı sınıf-ayrımı)

```text
65B-SINIFI (bootstrap-heal; ACK-BAĞIMSIZ):
  zincir: Sender bootstrap → giveup@60s → CsTerminal::TimedOut (epoch-BİR-kez) →
          receiver §11 forward-recovery RECOVERED → Sender.on_event(RecoveredControl) →
          PostTimeoutNotice{cause: RecoveredControl} · PostTimeoutRecovery-terminal-YOK-mecburiyeti-
          KALDIRILDI (B1: 65B ACK-bağımsız) · CsTerminal İKİNCİ-terminal ÜRETMEZ · N8 BtlBw-bit-özdeş

300B-SINIFI (DATA-ACK-clocked; kontrol-hücresi):
  zincir: TimedOut → DATA-ACK-heal → PostTimeoutNotice{cause: DataAckClocked} (PostTimeoutRecovery-
          terminal korunur)

CAP-STRESS:
  girdi:  RX_COLD_OPEN_BLOCK_CAP (8) üzeri incomplete-blok
  beklenti: typed-rejection (Dropped "cold cap exceeded") + bounded-memory (retained ≤ cap)

Tekrar: her-hücre ≥3 (deterministik → aynı-sonuç; sim-determinizm bit-özdeş).
Instrumentation: take_post_timeout_notice / take_cs_terminal / global_cold().res / IngestOutcome.
Test-only harness sonucu PRODUCT-acceptance yerine GEÇMEZ (gerçek ReceiverHost/Sender kullanılır).
```

## 3. YASAK (§12-uyum)
1200B-decisive / liveness-solved / recovery-impossible / FEC-ARQ-hükmü YASAK. ARQ-vs-FEC-karşılaştırma
kurulmaz. A1 SEALED. Hüküm §12 üç-formattan-biri.

## PROVENANCE
```text
sha256: 32ada1ad2b5628c49ed7ac2c810b6f48c551a23d2acc6b32836b16958f52b3ff (§PROVENANCE-sha256-satırı hariç gövde;
        doğrulama: grep -v '^sha256: ' <bu-dosya> | sha256sum)
tarih : 2026-07-30T18:47:40Z
```
