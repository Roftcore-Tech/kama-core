# DR-S5-3 — sender.rs: send-record ↔ ACK-eşleme (full_wire_pn → logical_ref) [ONAY BEKLİYOR]

- **Sprint:** 5c (cerrahi; §2-protokol). **Statü:** ÖNERİ — Abkarada-onayı-öncesi UYGULANMAZ.
- **Amaç:** N8-YENİDEN-DOĞRULAMA-ÖN-KOŞULU. ACK-neyi-onayladığı deterministik-logical-olmadan "recovered≠ACK"
  anlamlı-test-edilemez. Bu-DR: wire-ACK → hangi-logical-paket(block_id/esi)-teyit-edildi kaydı.
- **Non-regression:** `reports/d1-exit/baseline-pre-5c.md` {A+B+C}. §D-metodoloji KİLİTLİ.

## 0. ÇAĞIRAN + bw.rs-KOMŞULUK (Abkarada-şartı — grep OnAckReceived/acked)

`on_ack_received` çağıranı: `sender.rs:510` `Event::AckReceived(ack) => self.on_ack_received(&ack, now_ns)`.
**bw.rs-KOMŞU-SATIRLAR (bu-dokunuşun-YANINDA; KESİN-YASAK — DEĞİLMEYECEK):**
```
553-565  [Y3 mühürlü] W_BTLBW delivered-EAI-türetimi (last_sample_bw; btlbw.set_window)   ← YASAK
566-575  [S-FAZ-0] W_qd delivered-EAI-saati                                               ← YASAK
590-591  let sample = self.bw_sampler.on_ack(...); self.btlbw.update(sample, ...)         ← YASAK (inflight-ACK BtlBw-kredisi)
605-606  bw_sampler.on_ack + btlbw.update (tombstone kol-1)                               ← YASAK
619-620  self.bw_sampler.credit_delivered(...); diag_delivered_credited += ...            ← YASAK (delivered-kredi)
```
**BEYAN:** DR-S5-3 bu-5-bloğun HİÇBİRİNE tek-satır-eklemez/değiştirmez. Eşleme YALNIZ ayrı-logical-deftere yazar.
Eğer diff bu-satırlara-değerse → İHLAL → DUR-sor.

## 1. Dokunulacak dosya:satır (±3-bağlam)

**(a) Sender struct (~296)** — logical-defter alanları (yeni; mevcut-alanlar-değişmez):
```
295  inflight_bytes: u64,
296  inflight: BTreeMap<u64, InflightPacket>,
... (buraya: sent_logical + confirmed_logical eklenir)
```
**(b) plan_fec_parity (DR-S5-1'de-eklenen, ~1230)** — emit-anında seq→logical kaydı:
```
        let seq = self.track_sent(OpaqueUnit{...}, now_ns, 0, TransmitKind::FecParity);
        // (buraya: self.sent_logical.insert(seq, SrLogicalRef::Fec{block_id,esi}); )
        out.push(TransmitDecision { ... send_record: Some(...) });
```
**(c) on_ack_received loop (587-596)** — teyit-anında logical-defter güncelleme (BtlBw-satırlarına-DEĞMEDEN):
```
587  if let Some(p) = self.inflight.remove(&seq) {
588      self.inflight_bytes = self.inflight_bytes.saturating_sub(p.unit.size);
589      acked_units.push(p.unit.id);   // ← MEVCUT logical-cancel (§3.3-B); YENİ-satır BUNUN-YANINA
590      let sample = self.bw_sampler.on_ack(now_ns, p.bw_sample);   // ← YASAK-BÖLGE, DEĞİLMEZ
591      self.btlbw.update(sample, now_ns, sample.delivered > 0);    // ← YASAK-BÖLGE, DEĞİLMEZ
```

## 2. Değişiklik diff-taslağı

- **(a)** Sender'a iki-alan:
  ```rust
  /// wire-seq → logical-ref (yalnız FEC/control-emisyonu; DATA burada YOK — mevcut unit.id-cancel yeterli).
  sent_logical: BTreeMap<u64, SrLogicalRef>,
  /// wire-ACK ile teyit-edilen logical-paketler (§0.5 send-record defteri; ARQ-cancel/retention için).
  /// **SALT-LOGICAL — BtlBw/delivered'e beslenmez** (N8: recovered≠ACK ayrımının deterministik-tabanı).
  confirmed_logical: Vec<(u64, SrLogicalRef)>,
  ```
  Sender::new: `sent_logical: BTreeMap::new(), confirmed_logical: Vec::new()`.
- **(b)** plan_fec_parity: `track_sent`-sonrası **tek-satır** `self.sent_logical.insert(seq, SrLogicalRef::Fec { block_id: p.block_id, esi: p.esi });`
- **(c)** on_ack_received loop, `acked_units.push(p.unit.id);`-sonrası **tek-satır** (BtlBw-satırlarından-ÖNCE veya-SONRA
  FARK-ETMEZ — sıra-bağımsız, BtlBw'yi-okumaz/yazmaz):
  ```rust
  if let Some(lref) = self.sent_logical.remove(&seq) {
      self.confirmed_logical.push((seq, lref)); // SALT-LOGICAL; bw_sampler/btlbw'ye DOKUNMAZ
  }
  ```
- **(d)** Public: `pub fn take_confirmed_logical(&mut self) -> Vec<(u64, SrLogicalRef)>` (engine ARQ-cancel/retention okur)
  + diag: `pub fn diag_confirmed_logical_count(&self) -> usize`.

## 3. Hangi mühürlü-davranışa KOMŞU (titiz)

- **KOMŞU-AMA-DEĞİLMİYOR (KESİN-YASAK):** `bw_sampler.on_ack`/`btlbw.update`/`credit_delivered`/`last_sample_bw`/
  W_BTLBW/W_qd/delivered-EAI (§0 satır-listesi). **SealedPacketMeta-lookup ACK'in-BtlBw-yolunu DEĞİŞTİRMEZ** —
  yalnız hangi-logical-paketin-teyit-edildiğini ayrı-deftere-yazar. `acked_units`(unit.id-cancel) MEVCUT-mantık, dokunulmaz.
- **INV-RATE-1 korunur:** eşleme rate-hesabına GİRMEZ; send_rate hâlâ yalnız-BtlBw'den (loss/logical→rate-etkisi-YOK).
- **N8 temeli:** confirmed_logical wire-ACK-teyididir; RECOVERED(control) AYRI-yol (bu-defter-onunla-karışmaz →
  recovered≠ACK deterministik-ayrılabilir; N8-testi bunu-doğrular).

## 4. Non-regression kanıt-planı (AYNI COMMIT'te)

1. A: adaptive-transport exact-count (+yeni-test kadar artış; mevcut-122 AYNEN); diğer-crate'ler değişmez.
2. B: desen1_sim 5-aralık-pin PASS (özellikle qd/btlbw — ACK-yolu-değişmediği-için AYNEN).
3. C: FEC-off yapısal korunur.
4. **N8-SPOT (bu-commit):** wire-ACK confirmed_logical-üretir AMA BtlBw-örneği MEVCUT-davranışla-AYNI (btlbw
   confirmed_logical-eklenmeden-öncekiyle-özdeş — yeni-satır BtlBw'yi-değiştirmez assert). Tam-N8 (emisyon-canlı) S5-3-sonrası.
5. fmt/clippy temiz.

## 5. Kapsam-sınırı

Bu-DR yalnız **wire-ACK→logical-eşleme-defteri**. RECOVERED-control-işleme (sender-tarafı) + tam-N8-yeniden-doğrulama
(emisyon-canlı-pipeline: INV-RATE-1 + recovered≠ACK) DR-S5-3-yeşil-SONRASI, ayrı-adım (5c-çıkış-şartı).
