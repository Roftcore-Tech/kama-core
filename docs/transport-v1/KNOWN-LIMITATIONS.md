# Kama Transport v1 — BİLİNEN SINIRLAR

Her sınır bir kimlik, kanıt ve gelecek yön taşır. Backlog dilekleri sınır DEĞİLDİR; burada
yalnız ölçülmüş ya da kod-doğrulanmış sınırlar yer alır.

## LIM-001 — Sıfır-geri-besleme kuruluşu

**İfade:** Kama v1, ilk kuruluş sırasında hiç kullanılabilir geri-besleme elde edilemezse
ilerlemeyi garanti etmez. ACK yok → RTT örneği yok → yeniden-iletim zamanlayıcısı kurulamaz →
akış ilk pencerede (`initial_window_bytes` 2 400 B) kalır.
**Kanıt:** T1b (çift-yön %80 kayıp): 18/30 koşu `n=0/0 conf=0‰ Unknown`, 1 698 B / 25 600 B
gönderildi, `retx = 0`; T1c'de 30'da 1 koşu aynı imza.
**Önemli ayrım:** BOZUK geri-besleme (çoğu ACK kayıp, bazıları dönüyor) DESTEKLENİR ve T6 ile
güçlü doğrulanmıştır (dönüş %80 kayıp → 1.000). Sınır yalnız *sıfır* ACK'li başlangıçtır.
**Gelecek:** `bootstrap_rtt` / `bootstrap_fast_ladder` sonraki sürümde yeniden değerlendirilebilir;
v1 sevk davranışı DEĞİLDİR. ADR-zero-ack-bootstrap; EVOLUTION §9.1.

## LIM-002 — FEC'in nedensel katkısı kurulmamış

**İfade:** FEC açık ve bazı rejimlerde katılıyor (T2a 19, T6 3, T7 4.5 parity); ancak Faz-5,
Kama'nın ölçülen hostile üstünlüklerinin (T1a, T6) NEDENİ olarak FEC'i kurmaz — T1a'da
`parity = 0`. Üstünlük ARQ + mesaj-granülü teslim + veri-öncelikli kuruluş + yönlü geri-besleme
ile atfedilir.
**Yasak çıkarım:** "FEC etkisiz." T3a'da aynı mekanizma KORUYUCU çıktı (quinn'den %43 az tel).
**Kanıt:** PHASE5 §19, ADR-fec-v1-role. **Gelecek:** EVOLUTION §9.2/§9.4.

## LIM-003 — FEC öneri → maddeleşme gözlenebilirliği (Faz-6'da DARALTILDI)

**İfade:** Aktivasyon manifesti (`ActivationSummary`) "hiç önerilmedi" ile "önerildi ama
maddeleşmedi"yi ayırt edemiyordu (ikisi `parity=0 ∧ denied=0`). Faz-6'da salt-gözlem sayacı
`FecActuationDiag::rounded_to_zero_count` eklendi (öneri > 0, baskılama yok, `planned_r = 0`).
**Kalan sınır:** bu sayaç `FecActuationDiag`'ta; benchmark manifest şeması `v1` DEĞİŞMEDİ
(şema donmuş), yani sayaç kampanya manifestlerinde değil, `diag_fec_actuation()` API'sinde ve
stderr `fec2=[...]`/`clamp` satırlarında okunur. Manifest şemasına eklenmesi v1.x işidir.
**Kanıt:** PHASE5 "BULGU FEC-1"; `tests/sender_diag_contract.rs`.

## LIM-004 — Dönüş-bant asimetrisi kapsamı

**İfade:** T5 fiziksel asimetriyi uyguladı (1 Mbit / 64 kbit) ama yük dönüş yolunu doyurmadı
(ACK 281 B/s ↔ kapasite 8 000 B/s, %3.5). Doygun/doyuma-yakın dönüş bandındaki davranış
güçlü doğrulanmamıştır; Kama runtime'ı bu hücrede asimetriyi gözlemedi (T4 ile aynı tanı).
**Sözleşme şartı nasıl karşılandı:** yönlü gözlem şartı T6 ile (kayıp asimetrisi) karşılandı.
**Gelecek:** ACK-ağır geometri ya da daha dar dönüş kapasitesiyle doyuma yaklaşan hücre
(EVOLUTION §9.3). v1 iddiaları geriye dönük değiştirilmez.

## LIM-005 — TCP tel muhasebesi

**İfade:** Harness yalnız uygulama katmanı byte'larını sayar; çekirdek TCP retransmit/ACK/başlık
trafiği görünmez. TCP'ye karşı **tam-tel maliyet** iddiası kurulamaz. Kama ↔ quinn tel kıyası
tam harness-gözlemlidir (metodoloji §10).
**Kanıt:** Faz-4 TCP kolu beyanı; manifest `wire_accounting_note`; `fairness` "declared" farkı.

## LIM-006 — §11 acil-durum yolu saha-doğrulanmamış; §8.5 sayaçları emergency'yi görmez

**İfade:** §11 MinimalParity kablolu ve birim-testli (`tv_emergency`), ama T0–T7'nin 330 Kama
koşusunun hiçbirinde `RecoverFwd` girişi yok (`recover_fwd_entries = 0`); T6'da health Dead
iken `ack_confidence = 1000‰` raporlandı. Ayrıca emergency dalı `budget::parity_final`'i baypas
eder → `would_zero`/`clamped` sayaçları emergency parity'yi SAYMAZ; emergency duplication (Tiny)
hiçbir bütçe kısıtından geçmez.
**Kanıt:** ADR-minimal-parity-exception §Evidence; Faz-5 manifest taraması.
**Gelecek:** EVOLUTION §9.6 (ack_confidence'ın o köşede neden LOW'a düşmediğinin ölçümü).

## LIM-007 — Gecelik CI kapısı hedef runner'da koşmadı

**İfade:** Faz-4 gecelik hostile/manifest adımı hosted CI'da hiç çalışmadı (`NETEM_RUNNER`
tanımsız; dal `main`'e merge edilmemiş). Kanıt yerel doğrulanmış harness'tan.
**Kanıt/eylem:** `reports/transport-v1/NIGHTLY-INFRA-WAIVER.md` (sahip adımları listeli).
INFRA sınıfı; transport arızası değil.

## LIM-008 — ~1 ms'den kısa akışta PARAM_SET hiç gönderilmez (FEC bağlanmaz)

**İfade:** İstemci PARAM_SET'i ana döngünün `rtprop` interval'inin ilk tick'inde gönderir
(`crates/client-core/src/runtime.rs`, `rtprop.tick()` kolu). tokio zamanlayıcı çarkı 1 ms
çözünürlüklüdür; "anında" ilk tick pratikte ≈1 ms sonra ateşler. Tüm akış (sunum + teslim teyidi)
bundan önce biterse ana döngü PARAM gönderilmeden çıkar; drain döngüsünde `rtprop` kolu yoktur.
O akışta repair shard'ları tele çıkar ama sunucu eşi bağlamaz (`param_bound_peers = 0`); kod
okumasına göre shard'lar `pre_param_fec`'te (sınırlı, `PRE_PARAM_FEC_CAP`) bekler ve kullanılmaz.
**Kapsam:** yalnız alt-milisaniye RTT + kayıpsız yol (loopback/aynı-host). RTT ≥ ms olan her
yolda akış ilk tick'ten uzun yaşar; ilan edilmiş hostile zarfın DIŞINDA bir köşedir.
**Kanıt:** hosted CI `core-main` 35466488294 (2026-09-19): akış 0.34 ms ve 0.97 ms, `fec=736 B`,
`ctrl=0`, `param_confirmed=0`, sunucu `param_bound_peers=0`, `param_rejects=0`. Bağımsız tokio
1.52.3 sondası (current_thread, 200 deneme): ilk tick 0.93–1.07 ms. Yerel 60/60 koşuda akış
51–82 ms, PARAM her seferinde bağlandı.
**v1'de neden değişmiyor:** runtime donmuş (ADR-transport-v1-freeze); PARAM gönderim anını
değiştirmek runtime davranış değişikliğidir. Test (`bench::kama_product`) ön koşulu sunum
aralığıyla kurar, runtime'a dokunmaz.
**Gelecek:** v1.x — PARAM_SET'in ilk DATA'dan önce (tick beklemeden) gönderilmesi adayı.

## Sınır OLMAYANLAR (açıkça)

* T0'daki 33 tail probe / 100 mesaj: toplam tel quinn'in %4 üstünde — tasarım takası, patoloji değil.
* T2a'daki `parity_denied ≫ parity_emitted` (1 560 ↔ 19): koruyucu tahkim (teslim tavanda,
  en düşük blackout, zaten +%16 tel) — Faz-5 A şıkkı; karşı-olgusal ispatlanmamış, desteklenmiş.
