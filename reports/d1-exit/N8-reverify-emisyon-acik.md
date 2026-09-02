# N8-YENİDEN-DOĞRULAMA (FEC-emisyon AÇIKKEN) — Sprint-5c çıkış-şartı (1/2)

> **HÜKÜM: D0-N8 uyumu (INV-RATE-1 + recovered≠ACK/BtlBw) FEC-emisyon AÇIKKEN de KORUNUYOR.**
> İhlal-yolu ilk-kez fiziksel-mümkün (DR-S5-1 emisyonu açtı) — ölçüldü, ihlal-YOK.
> Saf-doğrulama: mühürlü-koda (bw.rs/send_rate/delivered-EAI/W_BTLBW) DOKUNULMADI; yalnız test-eklendi.

## 0. NEDEN — "kapalıyken-imkansız" → "açıkken-de-olmuyor"

D0-N8 kanıtı FEC-emisyonu KAPALIYKEN alınmıştı: `TransmitKind::FecParity` emit-edilmiyordu →
ihlal-yolu (FEC-shard-akışı, RECOVERED-üretimi) YAPISAL-İMKANSIZDI. DR-S5-1 emisyonu AÇTI →
ihlal-yolu ilk-kez FİZİKSEL-MÜMKÜN. Bu rapor, invariant'ın açıkken-de-tutulduğunun canlı-kanıtıdır.

İki ayrı invariant, iki ayrı test:
```
INV-RATE-1    : loss-artışı → send_rate DÜŞMEZ (send_rate yalnız BtlBw'den; loss-signal'den DEĞİL)
recovered≠ACK : FEC-decode başarısı (RECOVERED) → BtlBw'ye/delivered-EAI'ye KREDİ VERMEZ
```

## 1. TEST-1 — INV-RATE-1, emisyon-AÇIK (canlı sender)

**Konum:** `crates/adaptive-transport/tests/n8_reverify_emit.rs` ·
`n8_inv_rate_1_loss_does_not_lower_send_rate_emit_on`

**Kurulum:** `Sender::new()` + `set_fec_enabled(true)` (emisyon-AÇIK; her 4. round bir repair-shard akıtılır).
80 round sabit-kanal-BW (UNIT=1200 B / 10ms). İki koşu, AYNI kanal-BW:
- **(a) loss=0** — tüm emitted-seq ACK'lenir.
- **(b) loss≈%14** — `seq % 7 == 3` olan wire-seq'ler ACK'LENMEZ (kayıp = ACK-yokluğu → gap-detection → recovery-kolu).

**Ölçüm (deterministik, iki-koşu tekrarlanabilir):**

| Koşu | btlbw | send_rate (pacer.rate) | loss_pkt | fec_emit | acked |
|---|---|---|---|---|---|
| loss=0 | **240000** | **240000** | 4 | 19 | 99 |
| loss≈%14 | **240000** | **240000** | 63 | 11 | 82 |

- **send_rate BİT-ÖZDEŞ (240000 == 240000)** ve **btlbw BİT-ÖZDEŞ** — loss'un send_rate'e etkisi **SIFIR**.
- Kayıp gerçek + emisyon gerçek: loss-arm `loss_pkt=63` (kayıpsız-arm 4'e karşı) + `acked=82<99`; `fec_emit=11,19>0`.
- **NOT (benign):** emisyon-AÇIK her iki arm'da az-sayıda gap-detection üretir (FEC-parity DATA'dan ayrı-pace →
  geçici inflight-seq boşluğu; kayıpsız-arm=4). Kayıp-arm BELİRGİN-daha-fazla (63); bu gürültü send_rate'i etkilemez.

**HÜKÜM (INV-RATE-1):** ✅ Loss send_rate'i DÜŞÜRMEZ — emisyon-AÇIK + gerçek-%14-kayıp iken send_rate bit-özdeş.
Bu, DR-S5-3 N8-spot'unun (mapping-fires-vs-noop bit-özdeş) canlı-pipeline + emisyon-AÇIK + gerçek-kayıp
versiyonudur. Karşı-örnek (loss→rate-coupling) send_rate'i ~0.86× yapıp bit-özdeşliği KIRARDI → test FAIL olurdu.

**Deterministik-çekirdek (destekleyici):** `tv_src.rs::tv_src_inv_rate_1a_loss_does_not_change_rate` — `loss`,
rate-motoru `evaluate()`'in parametresi bile DEĞİL; canlı-yolda loss=ACK-yokluğu yalnız `pending_lost`/recovery-kolunu
besler, `btlbw.update`'e ASLA girmez.

## 2. TEST-2 — recovered ≠ ACK/BtlBw, emisyon-AÇIK (canlı pipeline)

**Konum:** `crates/kama-integration/tests/pipeline.rs` · `n8_recovered_is_logical_delivery_not_wire_credit`

**Kurulum:** Profil K8R4E64; 5-chunk app-bytes. Source esi {1,3} WIRE'da HİÇ teslim-edilmez (2 kayıp);
repair'ler geçer → FEC-decode kayıp-2-source'u kurtarır (RECOVERED üretilir).

**ASSERT-2a — recovered LOGICAL-teslim AMA WIRE-teslim DEĞİL → kredi-tabanına girmez:**
- `receiver_open_decode` → app-bytes TAM (recovered ile logical-teslim başarılı).
- Kredi-tabanı (wire-teslim source) = **{0,2,4}** (3 shard, yalnız wire'dan gelenler → BtlBw/delivered besler).
- recovered {1,3} bu-tabanın **DIŞINDA** — repair-decode'dan geldi, wire'dan gelmedi → **BtlBw'ye bit-eklemez**.

**ASSERT-2b — RECOVERED-control ile ACK AYRI-defter/AYRI-codepoint:**
- `control_produces_bandwidth_credit(RECOVERED) == false` — RECOVERED wire-katmanında BtlBw-credit ÜRETMEZ.
- `RECOVERED.codepoint()==0x12` ≠ `ACK.codepoint()==0x10` — yapısal olarak KARIŞAMAZ
  (recovered → retention-release; ACK → wire-kredi/confirmed_logical).

**HÜKÜM (recovered≠ACK):** ✅ FEC-decode başarısı BtlBw'ye/delivered-EAI'ye kredi VERMEZ.
Kredi yalnız wire-teslim-shard'lardan (engine `on_ack_received` → `bw_sampler`/`btlbw` yolu); recovered oraya GİRMEZ.

## 3. SINIR-BULGUSU (ihlal DEĞİL — güçlendirme + ertelenmiş-yol beyanı)

Kullanıcının ASSERT-2a metni "recovered-source `confirmed_logical`-defterine girer (logical-teslim)" der.
**Sender-tarafı RECOVERED-control-İŞLEME (→ `confirmed_logical`) henüz BAĞLI DEĞİL** — DR-S5-3 §5'te açıkça
ertelendi ("RECOVERED-control-işleme sender-tarafı ... ayrı-adım"). Yani:
- **Güvenlik-yarısı ("BtlBw'ye bit-eklemez") KANITLI** — hem yapısal (recovered'ın `btlbw.update`'e YOLU YOK; kredi
  yalnız wire-ACK'ten) hem runtime (Test-2 + `control_produces_bandwidth_credit=false`). recovered≠BtlBw **sağlanıyor**.
- **Muhasebe-yarısı ("recovered → logical-defter") ertelenmiş** — sender henüz RECOVERED işlemediği için recovered
  şu-an `confirmed_logical`'a GİRMEZ. Bu, N8-güvenliğini GÜÇLENDİRİR (recovered'ın BtlBw'ye hiçbir yolu yok), yeni-bir
  invariant-ihlali AÇMAZ. Sender-RECOVERED→retention-release/logical-defter wiring'i **5c-sonrası ayrı-DR** (kapsamı
  DR-S5-3 §5 çizdi).

**Sonuç:** N8-güvenlik-invariant'ı (recovered kredi-vermez) TAM; muhasebe-tarafı (recovered→logical-defter) gelecekte
bağlanacak, o-zaman `confirmed_logical`'ın BtlBw'den ayrı-tutulduğu (DR-S5-3 N8-spot) o-yol-için-de tekrar-doğrulanır.

## 4. NON-REGRESSION {A+B+C} (bu-adımda; test-only, mühürlü-koda-dokunulmadı)

| Gate | Baseline (pre-5c → S5-3) | Bu-adım | Durum |
|---|---|---|---|
| A: kama-noise-wire | 90 | **90** | eşit |
| A: kama-fec-codec | 25 | **25** | eşit |
| A: kama-transport-schema | 9 | **9** | eşit |
| A: kama-integration | 9 | **10** (+1 Test-2) | mevcut-korundu, +1 |
| A: adaptive-transport | 125 | **126** (+1 Test-1) | mevcut-korundu, +1 |
| B: desen1_sim 5-aralık-pin | PASS | **PASS** | eşit |
| C: FEC-off yapısal (guard-OFF no-emit) | PASS | **PASS** | eşit |
| fmt --check · clippy -D warnings | temiz | **temiz** | eşit |

**KESİN-YASAK dokunulmadı:** bw.rs delivered-EAI/last_sample_bw · send_rate-loss-modeli · ZAMAN-MODELİ · SACK-V2 ·
TokenBucket · PreConverge-CC · qd-canlanma · W_BTLBW. İki-test yalnız public-API sürer + diag OKUR + yeni control-literal kurar.

## 5. KALAN (Sprint-5c çıkış-şartı 2/2)

- **RETRANSMIT-test** (yeni-PN + yeni-nonce; INIT-cached-istisnası hariç; pipeline-üstünde) — kullanıcı ayrı-yönlendirir.
- İkisi-yeşil → **5c KAPANIR** → 5d (PARAM-bootstrap + admission).
