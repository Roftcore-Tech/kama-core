# DR-S5-2 — receiver.rs: FecShard-admission-köprüsü + RxKind-genişlik-düzeltmesi [ONAY BEKLİYOR]

- **Sprint:** 5c (cerrahi; §2-protokol). **Statü:** ÖNERİ — Abkarada-onayı-öncesi UYGULANMAZ.
- **Non-regression referansı:** `reports/d1-exit/baseline-pre-5c.md` (A+B+C). ⚠ Metodoloji §D-onayına bağlı.

## 0. ÖN-KONTROL SONUCU (Abkarada-şartı) — RxKind::FecShard çağıran-listesi

`grep -rn RxKind::FecShard crates/adaptive-transport/` → **3 site, hiçbiri mühürlü-yola değmez:**
- `receiver.rs:39` — enum TANIMI (değiştirilecek).
- `receiver.rs:227` — buffer-yolu `RxKind::FecShard { .. }` (`..`-pattern → alan-genişliğinden BAĞIMSIZ; değişmez).
- `receiver.rs:648-650` — TEST-constructor (yeni-genişliğe güncellenecek).
- **Alan-erişimi:** yalnız `shard_index` @ test:650. src/-içinde RxKind-alan-erişimi YOK.
- **Mühürlü-komşu-çağıran YOK:** RxKind::Data-teslim / SACK-V2 / bw.rs bu-tipe DEĞMEZ. → GÜVENLİ, DUR-gerekmedi.

## 1. Dokunulacak dosya:satır (±3-bağlam)

**(a) receiver.rs:35-49** — RxKind::FecShard genişlikleri (N8/N3-bulgusu: schema §2.6 ile uyumsuz):
```
35  pub enum RxKind {
36      Data,
38      /// FEC_SHARD — block/shard; reconstruction güvenlik katmanında ...
39      FecShard {
41          block_id: u64,     // schema §2.6: block_id u32
43          shard_index: u16,  // schema: esi u16 (ad-uyumu)
45          k: u16,            // schema: k_actual u8 (K_shape≤255)
47          r: u16,            // schema: R_code_max — repair ayrı
48      },
49  }
```
**(b) receiver.rs:225-232** — FecShard-buffer-yolu (şu-an teslim-etmez; admission-köprüsü YOK):
```
225  let (delivered, is_shard) = match d.kind {
226      RxKind::Data => (vec![d.unit_id], false),
227      RxKind::FecShard { .. } => {
228          // Shard buffer'lanır + ACK'lenir; RS-decode ... GÜVENLİK katmanında
229          // (authenticate-before-decode, INV-FEC-1). Transport birimi burada teslim ETMEZ.
230          (Vec::new(), true)
231      }
232  };
```

## 2. Değişiklik diff-taslağı

- **(a) RxKind-genişlik:** `block_id u64→u32`, `k u16→k_actual u8`, `shard_index→esi` (ad+u16-korunur), repair-ayrımı
  `is_repair: bool` (schema-source/repair). Schema `LogicalShardHeader` ile BİRE-BİR eşlenir (tek-otorite §2.6).
  ⚠ RxDatagram/RxKind byte-wire-DEĞİL (transport-iç-tip) → mühürlü-22B-envelope-etkilenmez.
- **(b) admission-köprüsü:** FecShard-kolu, opened-authenticated-shard'ı (noise-wire `OpenedTransportShard` →
  `InnerPlain::FecShard`) fec-codec-admission-buffer'ına yönlendirir (verify-before-decode: yalnız-AEAD-geçmiş-shard;
  INV-FEC-1). Teslim hâlâ-YOK (reconstruction-tamamlanınca ayrı-yol). ACK-yolu (SACK-V2) DEĞİŞMEZ.

## 3. Hangi mühürlü-davranışa KOMŞU

- **KOMŞU (dokunmuyorum):** `RxKind::Data` teslim-yolu + SACK-V2-frame/ack-tetikleme (KESİN-YASAK) + is_shard→
  RxState::FecDecode geçişi. Yalnız FecShard-KOLUNUN-içi genişletilir; Data-teslim + ACK-üretimi BYTE-DEĞİŞMEZ.
- **INV-FEC-1 korunur:** authenticate-before-decode — admission-köprüsü YALNIZ-noise-wire-open'dan-gelen (sealed-type)
  shard'ı kabul eder; forged-shard decoder'a-ULAŞAMAZ.
- **DOKUNMUYORUM (KESİN-YASAK):** SACK-V2/ZAMAN-MODELİ/qd. RxKind-genişlik transport-iç-tip (wire-byte-değil).

## 4. Non-regression kanıt-planı (AYNI COMMIT'te)

1. baseline-A: exact-test-count AYNEN (özellikle adaptive-transport 118 — RxKind-genişlik-değişimi mevcut-testleri-bozmamalı).
2. `fec_shard_buffered_not_delivered` (receiver.rs:643) — hâlâ-PASS (veya yeni-admission'a-göre güncellenir: shard buffer'lanır+ACK'lenir, teslim-YOK).
3. baseline-B/C: desen1 aralık-pin + FEC-off-yapısal.
4. SACK/Data-teslim spot: mixed Data+FecShard datagram-dizisinde Data-teslimi + ACK-tetiği DEĞİŞMEZ.
5. fmt/clippy temiz.

## 5. Kapsam-sınırı

Bu-DR **RxKind-genişlik + admission-köprüsü-iskeleti** (opened-shard→fec-buffer). Tam-reconstruction→app-teslim
akışı + decoder-admission-runtime-guard 5d/5c-devamı. N8-tam-yeniden-doğrulama emisyon-açık-pipeline'la 5c-çıkışında.
