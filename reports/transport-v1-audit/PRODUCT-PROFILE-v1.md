# KAMA TRANSPORT v1 — RESMİ ÜRÜN PROFİLİ

Tek doğruluk kaynağı: `client_core::KamaV1Profile` (crates/client-core/src/runtime.rs).
`RuntimeConfig::default()` buna devreder; benchmark kolları da buradan başlar. Pin: `crates/client-core/tests/product_profile_v1.rs`.
**Yalnız bu profille koşan bir ölçüm "Kama v1" adıyla raporlanabilir** (bench `KP-MANIFEST profile=kama-v1-full`, sapma listesi boş).

## Açık mekanizmalar (v1 kapsamı)

| mekanizma | değer / koşul | kanıt |
|---|---|---|
| ARQ + selective retransmit | her zaman; `pending_lost` → `plan_arq_retransmits` | KP-ACTIVATED `arq_retx>0` |
| Forward duplication (§3.3-B / §11) | PreConverge kopyası + RecoverFwd D× | `dup>0` |
| Tail-recovery (KAMA-TAIL-V1) | `TailRecoveryConfig::validated_v1()` (ClientSession kurar) | manifest `tail=ON`; **ateşleme sayacı yok → telemetri borcu** |
| Kuruluş probu (DR-EST-1) | default ON, PreConverge, token-muaf | `establish_probes_tx` |
| İlk-uçuş penceresi [IW] | 2400 B (`DEFAULT_BURST_CAP`) | manifest `iw=2400B` |
| FEC — blok kodlama | **profil 0x05**: K=8, R_max=4, E=1024; birim 1013 B | manifest `fec=ON adaptive p0x05` |
| FEC — aktüasyon | Controller (adaptif); parity oranını arbiter belirler | temizde parity 0, kayıpta >0 |
| Yapı-gate (canonical kol) | seviye 250‰ (r=2 @ K=8) | burst'te `gate_opened>0` |
| Spekülatif kol | `bounded_v1`: L1/L2 250-250, persistence P2 (Q95×1.25), C4 guard G1 (rp ≥ 300‰) | `spec_levels`, `arbiter` |
| Tek FEC arbiter | `max(confirmed, structure, speculative)`; toplama yok | `arbiter=[...]` |
| KURULUŞ-GUARD | PreConverge'de arbiter çıkışı 0 (üç kol) | `preconv_parity_blocked` |
| Kayıp defteri | Suspected/Confirmed/Spurious; confirm ufku `CadenceLegacy` | `ledger_peak_ppm`, `spurious` |
| Batched ACK/SACK | ACK_FRAME_V2, `ACK_FREQ_N=2` + timer, `MAX_ACK_RANGES=16` | `rx_bytes`, `acked` |
| Fragmenter | **KAPALI** (`max_message_bytes: None`) — app tavanı 1013 B | — |

## Kapalı mekanizmalar ve NEDENİ

| mekanizma | default | neden |
|---|---|---|
| DR-CS-3 bootstrap-RTT merdiveni | `bootstrap_rtt: false` | KALİBRE EDİLMEMİŞ; flip ölçüm ister (DR-CS-3 §9) |
| Kalibre füzyon politikası | `fusion_policy: None` | opt-in ölçüm kapısı (DR-FUSION-POLICY-1) |
| `FecStructurePolicy::BurstFusion` | `Q18Iid` | kalibre edilmemiş; burst→ARQ sevk davranışı |
| Fragmenter | `max_message_bytes: None` | açık opt-in; her DATA'ya +11 B prefix = wire görünür değişiklik |
| Kanal-türevli confirm ufku | `CadenceLegacy` | ölçüm kolu; düzeltme = ölçülen `return_bw` (açık WIRE) |
| `experimental-cold-start` (F2A) | cargo feature OFF | DR-CS-3 merdiveniyle duplicate; ürün anahtarına bağlı |
| `experimental-fec-budget` | cargo feature OFF | budget_kernel kalibre edilmemiş |
| `diag-sched-telemetry` | cargo feature OFF | salt gözlem; ürün davranışını değiştirmez |

## Bilinen sınırlar (v1 kapsamı DIŞINDA, açıkça beyan)

1. **Kripto seam yok:** üretimdeki tek `SecuritySeam` `IdentitySeam` (şifreleme yok). Gerçek Noise
   seam Sprint-6/S2'de. INV-FEC-1'in "authenticate-before-decode" sırası seam konumuyla yapısal olarak
   korunuyor ama kripto henüz takılı değil.
2. **Dönüş yolu ölçülmüyor:** `return_bw` sabit prior (31 B/s), ACK decimation yok, `seen_count`
   tüketicisi yok → ileri/dönüş kaybı ayrımı YOK. Asimetrik yol ve bozulmuş dönüş yolu rejimlerinde
   mimari eksik (bkz. KAPSAMA MATRİSİ tema A).
3. **Kısmi/son blok kodlanmıyor:** K=8 dolmayan blok parity almaz (`k_actual = k_shape` sabit).
4. **FEC symbol boyu dağıtım parametresidir:** repair her zaman E=1024 B. Birim boyu ≪ E olan
   dağıtımlarda parity ek yükü orantısız büyür (256 B yükte ~%94). Bkz. ACTIVATION-PROOF C-1.
