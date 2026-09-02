# DR-CS-1 F4-CLOSURE — PLAN + TEST ENVANTERİ (FC0)

**Tarih:** 2026-07-25 · **Branch:** `d1-sprint5c` · **Başlangıç:** `f22e644` · **PR:** #44 (DRAFT).
**Amaç:** F4 kabul-matrisinin EKSİK hücrelerini tamamla + bilimsel-doğru closure. **Yeni-mekanizma/
tasarım/codepoint/recovery-yolu YOK.**

## 1. HÜCRE ENVANTERİ (mevcut durum)

| Hücre | Durum (f22e644) | Eksik altyapı |
|---|---|---|
| W-1 (frozen-bound) | KISMİ — 30s-generous-tavan, 5.5s ölçüldü (RTT100/1200B) | formal-calibration+freeze-artifact; 65/300B; OFF-vs-ON tablosu |
| W-2 (exact-once ACK-drop) | YOK | test-only first-SACK-suppress hook + rx-dup-counter + app-delivery=1 |
| DI-2B (reverse-blackout) | YOK | reverse-100%-loss netem + probe-timing/timeout-assertion |
| W-3 (blackout-heal 3-boyut) | YOK | --unit-bytes knob + boundary-restore-profilleri + zaman-çizelgesi |
| DI-10 (rx-caps) | YOK | gerçek-cap-türetimi + typed-diag + adversarial (cap+1) |
| Mixed-version (5-satır) | KISMİ — g4 (unknown-safe-ignore) | 5-satır tam-matris (wire_safety + feature_effectiveness) codec-seviyesi |

## 2. MEVCUT TEST-ONLY HOOK'LAR (envanter-kontrolü)

- Echo-suppress: `cs_echo_suppressed` sayacı VAR (T_ECHO_MIN-rate-limit; udp_runner.rs:158/212). W-2
  için AYRI hook gerekir (SACK-suppress; ACK-drop ≠ echo-rate-limit).
- Unit-size: `UNIT=1200` SABİT (const; udp_runner.rs:17). → `--unit-bytes` knob gerekir (W-3 65/300/1200).
- Rx-dup-counter: YOK. → block-decode'da (block,esi) zaten-src_have'de-ise-say (W-2 kriter-8).
- Terminal-drain: `take_cs_terminal` harness'e-BAĞLI-DEĞİL. → tx-batch-sonrası-drain + marker-terminal.
- Rx-cold-caps: mevcut-map'ler w_units-bounded (blok-indeksli); DI-10 = gerçek-sınır-türet + typed-diag.

## 3. EKLENECEK ALTYAPI (test-only; production-yol-etkisiz; hepsi cfg-gated)

1. `--unit-bytes N` (RunnerCfg.unit_bytes; default 1200; OpaqueUnit.size + enc-size + parity-size).
2. `--drop-first-sack` (test-only; rx İLK-SACK-emisyonunu exact-once-suppress; drop-ledger).
3. rx `cs_dup_deliveries` sayacı (block,esi zaten-src_have'de → duplicate-delivery; W-2 kriter-8).
4. tx marker: `cs_terminal` (take_cs_terminal-drain → ResolvedBy*/TimedOut/PostTimeoutRecovery).
5. rx `--cold-block-cap N` + typed-diag (cap-aşımı → RXCAP-marker; sessiz-eviction-yok; DI-10).
6. Provenance-blok (her-marker'a): git-commit/dirty/flag/kernel/netns/tc/RTT/seed/timestamp/acceptance.

## 4. PROVENANCE FORMATI (her faithful-koşu; §2-zorunlu)
```
git_commit · dirty_tree · feature_flag · kernel · netns_topology · tc_netem · configured_RTT/loss/rate
· random_seed · start_timestamp · runner_version · acceptance_6of6
```
Acceptance 6/6 geçmezse koşu = `INVALID RUN — NO EVIDENCE` (kabul-tablosuna-girmez).

## 5. COMMIT PLANI
FC0(plan) → plumbing(test-only) → FC1(W-2) → FC2(DI-2B) → FC3(W-3) → FC4(DI-10) → FC5(mixed-version)
→ FC6(calibration+freeze+W-1-rerun) → FC7(closure-final + hüküm).

## 6. BLOCKER LİSTESİ (FC0)
- Mixed-version "old-receiver": harness-raw-UDP AEAD-sealed-DEĞİL → 0x19-misinterpret harness-artefaktı.
  ÇÖZÜM: mixed-version PRODUCTION-codec-seviyesinde (kama-noise-wire classify/decode; g4-genişletme) —
  faithful-production-yolu bu; harness-raw-wire safe-ignore-modellemez. (Tasarım-kararı-değil; kapsam-içi.)
- Yeni tasarım-gereksinimi görülürse → DUR-B (exact-blocker + dosya/satır + en-küçük-amendman).

## HÜKÜM (FC0): doc/test-envanter tamamlandı; tasarım-kararı-gerektiren-yeni-durum YOK → devam.
