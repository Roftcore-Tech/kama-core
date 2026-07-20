# DR-1 — §2.57 control count-bounds sayısal-tanımsız → max-control sealed_len üst-sınırı hesaplanamaz

- **Kaynak:** N7 (wire-sizing/PMTU) UYUM-raporu.
- **Tür:** spec-boşluğu ([CAL]-deferred), **spec-HATASI-DEĞİL** — KURAL-2 gereği kod-değişikliği/spec-değişikliği ÖNERİLMEZ; karar Abkarada'nın.
- **Statü:** AÇIK — Abkarada-kararı bekliyor. **FEC-codec'i BLOKLAMAZ** (sadece control-frame max-boyutu / N7-PMTU-tam-tablosu için gerekli).

## Gözlem

§2.6/§2.57 byte-layout FEC-shard için **tam-kapalı**: sealed_len(EXTENDED-FEC) = `37 + E`, deterministik (bkz.
`reports/fec-poc/pmtu-profile-table.md`). Ancak **control-frame'lerin** sealed_len ÜST-SINIRI hesaplanamıyor çünkü
§2.57 gövdelerindeki tekrar-sayıları sayısal-sınırsız/[CAL]:

- `ACK`/`SACK`: wire-PN-range sayısı (`range_count`) — "count'lar-profile-bounded" (§2.7 satır-317) ama sayısal-değer YOK.
- `RECOVERED (0x12)`: `esi_range_count u16_be` + `[esi_start, esi_end] × count` — count üst-sınırı tanımsız (u16-teorik-max 65535 → 4×65535 B saçma-büyük, PMTU-aşar).

Sonuç: "her-control-datagram PMTU-aşmaz" (K-FEC-MTU-1 control-tarafı) bugün-KANITLANAMAZ; max-control sealed_len açık.

## Karar-talebi (Abkarada)

Aşağıdaki count-tavanlarına **sayısal-değer** ver (profile-bounded [CAL] → sabit), ki max-control sealed_len üst-sınırı
kapansın ve control-frame'ler PMTU-içinde-kalsın:

1. `ACK`/`SACK` `range_count` max (öneri-aralığı: tek-datagram-PMTU'ya-sığacak, örn ≤ 32 range → ~132 B gövde).
2. `RECOVERED` `esi_range_count` max (öneri: ≤ 16 range).
3. Tavan-aşımında davranış: **çok-datagram'a-böl** mü yoksa **reject/coalesce** mi?

## Etki

- Karar-VERİLENE-kadar: FEC-shard-tarafı tam-hesaplı (etkilenmez); control-frame max-boyut-tablosu N7'de AÇIK-bırakıldı.
- Karar-sonrası: schema-crate control-DTO'larına count-cap sabitleri + `sealed_len(control)` üst-sınır-testi eklenir (D1).
