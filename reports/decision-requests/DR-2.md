# DR-2 — §0.7-B PRE_CONVERGE/KEYSTONE ↔ bounded-idempotent-INIT-flight-machine katman-özdeşleştirmesi

- **Kaynak:** N10 (flight-machine) UYUM-raporu.
- **Tür:** spec-netlik-sorusu (kod↔spec katman-uyuşmazlığı). KURAL-2 gereği spec-değiştirilmedi; karar Abkarada'nın.
- **Statü:** AÇIK — Abkarada-kararı bekliyor. **FEC-codec'i BLOKLAMAZ** (handshake/INIT-survival ayrı-katman, D1-noise-wire).

## Gözlem

§0.7-B, "survival = bounded-idempotent-INIT-retransmission-flight-machine" kararını PRE_CONVERGE + KEYSTONE ile
İLİŞKİLENDİREN bir "BAĞ" satırı içeriyor (MADDE-5 cached-byte-for-byte-INIT-retransmit). Kod-envanteri (N10) bu-ikisinin
**farklı-katman** olduğunu gösteriyor:

- **PRE_CONVERGE** (`sender.rs:1290-1303`, `pacer.rs:75-80`): congestion-control **soğuk-başlangıç** — DATA-birim geometrik-kopya,
  ×2-backoff, **sayı-tavanı YOK** (`sender.rs:1296` "sayı-tavanı YOK"), ACK-gelince-iptal, her-kopya **YENİ-seq** taşır
  (`sender.rs:1290`). MADDE-5'in "cached-byte-for-byte / aynı-init_counter / bounded" kuralının **TERSİ**.
- **KEYSTONE** (`sender.rs:53`): yalnız `[K-KEYSTONE]` yorum-etiketi — Noise-AEAD-probe kodu **YOK**.
- **SESSION_INIT / handshake / crypto:** iskelet (`crypto/src/lib.rs:1-3`). Cached-INIT-store, retry-sabitleri,
  anti-amplification = grep-0.

Yani §0.7-B'nin (B)-flight-machine'i **kodda-yok** ve mevcut-PRE_CONVERGE onun-uygulaması-DEĞİL (amaç+kural-zıt).

## Karar-talebi (Abkarada)

§0.7-B "BAĞ"-satırının niyeti hangisi:

1. **(a) Ayrı-katman:** INIT-flight-machine (bounded, cached-byte-for-byte, anti-amplification) D1'de **SIFIRDAN** yazılır;
   PRE_CONVERGE sadece-analojik-emsal (CC-cold-start), özdeş-DEĞİL. → N10-raporu bu-yorumla-yazıldı.
2. **(b) Yeniden-kullan:** PRE_CONVERGE-makinesi INIT-survival'a genişletilir (bounded-cap + cached-INIT + same-counter
   eklenir) — bu-durumda "sayı-tavanı-YOK" DATA-CC-davranışı INIT-yolunda değişmeli.

Öneri: **(a)** — INIT-datagram-survival ile DATA-CC-cold-start güvenlik-modeli+idempotency-gereksinimi farklı;
karıştırma amplification/replay-riski taşır.

## Etki

- Karar FEC-codec'i etkilemez (ayrı-katman). D1-noise-wire INIT-flight-machine tasarımını yönlendirir.
- (a) seçilirse §0.7-B "BAĞ"-satırı D1-notunda "analoji, özdeşlik-değil" diye netleştirilir (spec-hata-düzeltmesi-DEĞİL).
