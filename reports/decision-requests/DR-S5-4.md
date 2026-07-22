# DR-S5-4 — sender-RECOVERED-wiring: retransmit-cancel + retention-release + recovered→confirmed_logical [ONAYLANDI + UYGULANDI]

> **COMMIT-K-ADDENDUM (authority-continuity-turu):** on_recovered_control yazım-kümesi (i)(ii)(iii)'e
> **(iv) pending_fec parity-CANCEL** eklendi (blok-decode-edildi → kuyruklu-parity-anlamsız; block_id-
> bazlı-retain; sayaç `parity_canceled_total` → diag `canceled_r_total`). Bu-DR'ın "SALT-LOGICAL"
> beyanındaki yazım-kümesi-genişlemesidir — bw_sampler/btlbw/credit/pacer'a SIFIR-satır-ilkesi ve
> N8-bit-özdeşlik AYNEN-korunur (emisyon-kuyruğu kredi-yolu-değildir; iptal=gelecek-gönderim-yok-olur).
> Kanıt: emission_continuity.rs::t66 (+çift-sayım-pini). Ayrıntı: reports/d1-exit/dr-1a-authority-continuity.md.

- **Sprint:** 5d-3 (cerrahi; §2-protokol). **Statü:** ONAYLANDI (Abkarada, 2026-07-21) — **§4 SEÇENEK-A**
  (diag_recovered_cancels + ConfirmSource-marker; send_generation-semantiği-dokunulmadı; B=ileride-ayrı-alt-DR).
  Üç-uygulama-şartı karşılandı: (1) N8-bit-özdeşlik-testi commit'te (`dr_s5_4_n8_recovered_does_not_change_btlbw`),
  (2) block_id/esi→unit_id-çevrimi engine'de (`kama-engine/src/recovered_map.rs`; sender mevcut unit_id-cancel-paraleli),
  (3) diag_recovered_cancels + ConfirmSource M1-erişilebilir (public accessor + take_confirmed_logical 3-tuple).
- **Amaç (K2-kapanışı):** sender RECOVERED-control'ü işlesin → FEC'in ANA-avantajını (kurtardığını-retransmit-etmeme)
  AÇ. Bu **M1-ön-koşulu** (bağlanmadan M1 = FEC-arm ana-avantajı-kapalı → karşılaştırma geçersiz).
- **Hassasiyet:** DR-S5-3'ten DAHA-hassas. S5-3 = ACK-yolu (BtlBw-kredi-yolunun-YANINDA, tek-satır SALT-LOGICAL).
  Bu = RECOVERED-yolu, ve **recovered'ın BtlBw'ye HİÇ-yolu-olmaması-gereken** yer. N8-değmezlik burada mutlak.
- **Non-regression:** `reports/d1-exit/baseline-pre-5c.md` {A+B+C} + N8-bit-özdeşlik (aşağıda §3).

## 0. ÇAĞRAN + KOMŞULUK-HARİTASI + bw.rs-YASAK-SATIR-LİSTESİ

**RECOVERED-handler sender'da NEREYE giriyor?** Sender'da control-event YOK (`Event` = AppData/AckReceived/TimerFired,
sender.rs:84; `on_event` dispatch 503-522). Yani **RECOVERED-giriş-noktası YENİ** — additive `Event::RecoveredControl`
+ ayrı `on_recovered_control` metodu (on_ack_received'dan **AYRI** metod; on_timer/on_ack_received ile simetrik).
**Bu, N8-değmezliğin yapısal-tabanı:** RECOVERED-işleme on_ack_received'ın bw.rs-bloklarından FARKLI-metotta.

**bw.rs KOMŞU-SATIRLAR (güncel, DR-S5-3 sonrası; KESİN-YASAK — on_recovered_control HİÇBİRİNE değmez):**
```
555-575  [Y3/K6R mühürlü] W_BTLBW delivered-EAI türetimi (last_sample_bw@570; btlbw.set_window@575)   ← YASAK
577-582  [S-FAZ-0]        W_qd delivered-EAI saati (rtt.set_qd_window@582)                             ← YASAK
604-605  bw_sampler.on_ack + btlbw.update (inflight-ACK BtlBw kredisi)                                 ← YASAK
619-620  bw_sampler.on_ack + btlbw.update (tombstone kol-1 BW-örneği)                                  ← YASAK
633-634  bw_sampler.credit_delivered + diag_delivered_credited (tombstone kol-2 delivered-kredi)       ← YASAK
~1530    track_sent → bw_sampler.on_sent (emit-anı BW-örnek; §1532 loss.on_sent yanı)                  ← YASAK
```
**BEYAN:** on_recovered_control bu-6-bloğun HİÇBİRİNE tek-satır-eklemez/okumaz. RECOVERED yalnız pending_lost/
dup_queue/confirmed_logical (SALT-LOGICAL) yazar. Diff bu-satırlara-değerse → İHLAL → DUR-sor.

**Recovered-cancel TokenBucket/pacing-yoluna KOMŞU MU? → HAYIR (kanıtlı):**
- retransmit-cancel = `pending_lost.retain(|r| r.unit.id != recovered)` — saf VecDeque-filtre. **pacing-BAĞIMSIZ.**
  Pacer yalnız retransmit-**ATTEMPT**'inde (`plan_arq_retransmits`: `pacer.try_send(rec.unit.size)` @plan_arq
  içinde) okunur; **cancel entry'yi kaldırır → attempt HİÇ-olmaz** → TokenBucket'a değmez.
- retention-release = `dup_queue.retain(|d| d.unit.id != recovered)` — MEVCUT-mekanizma @707-709'un (acked-cancel)
  BİREBİR-paraleli (RECOVERED tetiklemesiyle). pacing-bağımsız.
- confirmed_logical = `record_*` (DR-S5-3 mekanizması @1239; SALT-LOGICAL, bw'ye-değmez).
- track_sent (bw_sampler.on_sent @~1530) recovered-cancel tarafından **ÇAĞRILMAZ** (cancel = non-emit).

## 1. Dokunulacak dosya:satır (±3-bağlam)

**(a) Event enum (84)** — additive variant:
```
84  pub enum Event {
      AppData(OpaqueUnit) / AckReceived(AckInfo) / TimerFired(Timer)
      + RecoveredControl(RecoveredInfo)   ← YENİ (engine block_id/esi→unit_id çevirip verir)
```
**(b) on_event dispatch (521-522)** — additive arm:
```
521  Event::AckReceived(ack) => self.on_ack_received(&ack, now_ns),
522  Event::TimerFired(timer) => self.on_timer(timer, now_ns),
    + Event::RecoveredControl(info) => self.on_recovered_control(&info, now_ns),   ← YENİ
```
**(c) Sender struct (~351-352)** — additive alanlar (DR-S5-3 defterlerinin yanı):
```
351  sent_logical: BTreeMap<u64, SrLogicalRef>,
352  confirmed_logical: VecDeque<(u64, SrLogicalRef)>,
    + diag_recovered_cancels: u64   ← K2 M1-signal (retransmit-suppress sayacı)
    (+ confirmed_logical entry ConfirmSource-marker; §4)
```
**(d) YENİ metod on_recovered_control** — on_ack_received'dan AYRI, bw.rs-bloklarına UZAK.

## 2. Değişiklik diff-taslağı

- **(a) Tipler:** `struct RecoveredInfo { units: Vec<RecoveredUnit> }`, `struct RecoveredUnit { unit_id: u64, block_id: u32, esi: u16 }`
  (engine üretir — block_id/esi↔unit_id eşlemesi engine'de, SoC; sender native-unit_id ile çalışır).
- **(b) Metod (cancel-only; N8-firewall):**
  ```rust
  fn on_recovered_control(&mut self, info: &RecoveredInfo, _now_ns: u64) -> Vec<TransmitDecision> {
      for ru in &info.units {
          // (i) retransmit-CANCEL: kurtarılan-unit pending_lost'tan çıkar (retransmit hiç-olmaz).
          let before = self.pending_lost.len();
          self.pending_lost.retain(|r| r.unit.id != ru.unit_id);
          self.diag_recovered_cancels += (before - self.pending_lost.len()) as u64; // K2/M1
          // (ii) retention-RELEASE: bekleyen forward-dup kopyaları iptal (MEVCUT @708 paraleli).
          self.dup_queue.retain(|d| d.unit.id != ru.unit_id);
          // (iii) confirmed_logical: recovered-logical teyidi (SALT-LOGICAL; RECOVERED-marker).
          self.record_recovered_logical(ru.block_id, ru.esi); // bw_sampler/btlbw'ye DOKUNMAZ
      }
      Vec::new() // cancel yeni-emit ÜRETMEZ (schedule_transmit çağrılmaz — saf-state)
  }
  ```
- **(c) record_recovered_logical:** confirmed_logical'a RECOVERED-kaynaklı-teyit yazar (DR-S5-3 bounded-cap aynen).
  **bw_sampler/btlbw/credit_delivered'e SIFIR-satır** (S5-3 record_confirmed_logical ile aynı-disiplin).

## 3. N8-DEĞMEZLİK KANITI (S5-3-N8-spot'un RECOVERED-versiyonu; ZORUNLU, bu-commit'te)

**İki-koşu bit-özdeşlik testi** (S5-3 `dr_s5_3_mapping_does_not_change_btlbw` kalıbı, RECOVERED için):
```
warmup_recovered(fires: bool) -> (btlbw, recovered_cancels, confirmed_logical_len):
  Aynı DATA/loss senaryosunu sür (pending_lost dolu). fires=true → on_recovered_control(units) çağrılır;
  fires=false → NO-OP. İki-koşu:
    ASSERT btlbw_fires == btlbw_noop      (BİT-ÖZDEŞ — recovered BtlBw'ye SIFIR-etki)
    ASSERT recovered_cancels_fires > 0    (fires: retransmit-cancel gerçekleşti)
    ASSERT confirmed_logical_fires > noop (fires: recovered→logical-defter DOLDU)
```
**Hüküm-kriteri:** recovered→confirmed_logical GİRER + retransmit-CANCEL olur AMA **BtlBw bit-özdeş** →
credit-yolu AÇILMIYOR (yalnız-logical-defter + retransmit-cancel + retention-release). Bit-özdeşlik-kırılırsa → İHLAL.
Diff-net: recovered-satırları yalnız pending_lost/dup_queue/confirmed_logical/diag_recovered_cancels; btlbw.update-YOK.

## 4. send_generation-DÜZELMESİ (K2-kapanışı) — retransmit-cancel M1-görünürlüğü

**K2-tespiti:** send_generation transport-core'da retransmit'te ARTMAZ (sabit-0; SendRecordHint yalnız FEC-emit'te);
retransmit-diskriminatörü retx_count. **Sorun:** M1'de "FEC-kurtardığını-retransmit-etmiyor" ölçülebilir-olmalı.

**Öneri (ölçülebilir-cancel; send_generation-semantiği-korunarak):** retransmit-suppress'in M1-sinyali =
- **`diag_recovered_cancels`** (birincil): kurtarma-yüzünden-iptal-edilen retransmit-sayısı. M1 doğrudan ölçer:
  "recovered-cancel > 0 ∧ o unit'ler retx_count-artmadı" → FEC-arm retransmit-etmedi (ARQ-arm ederdi).
- **confirmed_logical `ConfirmSource` marker** (WireAck | Recovered): recovered-confirmed vs wire-ACK-confirmed
  AYRIMI → M1 "recovered-teslim" ile "retransmit-teslim"i sayar. Recovered-entry send_generation=ORİJİNAL
  (retransmit-olmadığı-için gen-artmaz) → "gen-0'da-teslim, retransmit-YOK" = FEC-kurtardı kanıtı.

**send_generation-hakkında-DÜRÜST-NOT:** send_generation PER-EMİSYON'dur; iptal-edilen-retransmit bir NON-emisyon →
send_generation onu doğrudan-taşımaz. M1-kanıtı = **suppress-edilen-retransmit** (`diag_recovered_cancels` + o unit'in
retx_count-artmaması). Eğer Abkarada send_generation'ı per-LOGICAL-unit-generation'a (0=orijinal, 1=ilk-retx)
yeniden-tanımlamak isterse — bu AYRI-alt-karar (send_generation-semantik-değişimi); bu-DR onu ZORUNLU-kılmaz,
`diag_recovered_cancels` + ConfirmSource yeterli-M1-metriği verir. **Karar:** (A) yalnız-counter+marker [ÖNERİLEN] veya
(B) +send_generation-per-logical-redefinisyon [daha-geniş-dokunuş]. Onayında belirt.

## 5. Non-regression kanıt-planı (AYNI COMMIT'te)

1. **A:** adaptive-transport exact-count (+N8-bit-özdeşlik-testi + on_recovered-birim-testleri kadar artış; mevcut-126 AYNEN).
   Diğer-crate'ler değişmez (RecoveredInfo sender-içi; engine-tarafı ayrı-adımda-bağlanabilir).
2. **B:** desen1_sim 5-aralık-pin PASS (RECOVERED-yolu ACK/rate-yolunu-değiştirmediği-için AYNEN).
3. **C:** FEC-off yapısal korunur (RecoveredControl-event yalnız engine-çağırınca; guard-OFF etkilenmez).
4. **N8-SPOT (§3):** btlbw_fires == btlbw_noop bit-özdeş — **bu-commit'in-çıkış-şartı.**
5. fmt/clippy temiz.

## 6. KAPSAM-SINIRI

Bu-DR yalnız **sender-tarafı RECOVERED-cancel/release/logical-defter + N8-değmezlik**. Engine-tarafı (kama-engine)
RECOVERED-control→RecoveredUnit-çevirisi (block_id/esi→unit_id) + Event-besleme **ayrı-adım** (5d-3-yeşil-sonrası veya
aynı-turda-engine-testi). N8-yeniden-doğrulama-v2 (muhasebe-yarısı-bağlı + güvenlik-korundu, canlı-pipeline) DR-S5-4-
yeşil-SONRASI (5d-çıkış-şartı). send_generation-per-logical-redefinisyonu (§4-B seçilirse) ayrı-alt-DR.
