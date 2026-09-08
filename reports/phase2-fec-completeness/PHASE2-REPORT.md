# FAZ-2 — FEC SPEC-TAMLIĞI · KAPANIŞ

FEC **araştırması KAPALI**. Bu faz yalnız denetimle kanıtlanmış spec↔kod uyuşmazlıklarını kapatır:
kalibrasyon yok, parametre araması yok, yeni tahminci/kontrolcü/kod ailesi yok.
Her kalem: **SPEC → kapsama testi → en küçük düzeltme → yeşil test**.

## KALEM-1 — Kısmi/son blok kodlanmıyordu (Model-A known-zero)

**Spec (fec-core §6 L205-216, NORMATİF):** `known_zero_count = K_shape − K_actual`; **gönderici
zero-slot'ları ENCODE'a katar, WIRE'a göndermez**; **alıcı** `K_actual`'ı öğrenip known-zero
shard'ları decoder'a bilinen-source olarak enjekte eder. Zorunlu POC: `K_shape=8 × K_actual<K`.

**Kod (öncesi):** blok YALNIZ tam dolunca kapanıyordu (`cur_block.len() == k_shape`); alıcı
`reconstruct(p, &recv, 0)` ile sabit sıfır known-zero veriyordu. Sonuç: akışın **son** bloğu ve
K'ya ulaşmayan **kısa akışlar** FEC korumasından tamamen yoksundu.

**Düzeltme:**
- *Gönderici:* akış sonunda kısmi blok zero-dolgu ile kapatılır (`fec_partial_blocks_closed` sayacı);
  `repair_store` artık blok→(K_actual, repairs) tutar ve **K_actual tel başlığında taşınır**.
- *Alıcı:* `known_zero = K_shape − K_actual`; K_actual otoritesi **repair başlığıdır** (v0 telinde
  source sembolleri DATA çerçevesiyle gider ve K_actual taşımaz).
- *Ek kusur (aynı yolda bulundu):* kurtarılan chunk'ın birim-id eşlemesi `asm.k` (=K_actual)
  kullanıyordu; doğrusu `k_shape`'tir (gönderici blok_id'yi `id / k_shape` ile atar). Kısmi blokta
  bu, kurtarılan chunk'ı yanlış birime bağlardı.

**Test (spec'in zorunlu POC'u):** `partial_block_below_k_is_coded_and_recovers_model_a` —
K_shape=8, K_actual=5, bir chunk KALICI silinir → mesajların tamamı FEC ile kurtarılıp teslim edilir
(`fec_partial_blocks_closed == 1`, `fec_recovered_units ≥ 1`).

## KALEM-2 — §8.5 bant-türevli parity clamp'i bağlı değildi

**Spec (engine §8.5 L658-661, NORMATİF):** `parity_final = min(fuzzy_parity, budget_cap, profile_cap)`,
`budget_cap = f(BtlBw)`.

**Kod (öncesi):** `budget::budget_cap_permille` / `budget::parity_final` uygulanmıştı ama **hiçbir
ürün çağıranı yoktu** (yalnız `tv_emergency` testinden). **Düzeltme:** arbiter ÇIKIŞINA uygulandı —
tek-arbiter invaryantı korunur (ikinci otorite değil, üst sınır; yalnız küçültür).

**İki uç durum, sessizce seçilmedi:**
1. **Kanıt yokken clamp yok.** `btlbw()` örnek yokken 0 döner; 0'ı "bütçe yok" okumak FEC'i kuruluş
   fazında sessizce kapatırdı. BtlBw yoksa üst sınır profil tavanıdır.
2. **SPEC-İÇİ ÇELİŞKİ (açık kalem):** formül `budget_ref = 125 kB/s` fixture'ıyla BtlBw < 125 B/s'te
   `cap = 0‰` verir; ama §11 (NORMATİF) **tam o köşede** "minimal parity shard" ister ve ürünün
   MIN_RATE tabanı 31 B/s'tir. Literal okuma, transport'un birincil hedef rejiminde (sub-kbps)
   FEC'i tamamen kapatır. **Karar (kalibrasyon değil):** clamp yalnız küçülttüğünde uygulanır;
   parity'yi **sıfırlayacaksa uygulanmaz** ve `parity_budget_would_zero` sayacıyla görünür olur.
   Sıfırlama vakasının politikası **owner kararıdır** (kapanış raporunda açık kalem).

**Ürün-yolu kanıtı** (netem %20 kayıp, 1 mbit): `clamp=4165` karar bağlandı; parity 46 → 33 repair
(−%28), `would_zero=0` (1 mbit'te çelişki köşesine girilmiyor — öngörüyle uyumlu).

## KALEM-3 — Kaybolan repair ARQ kuyruğuna giriyordu (ESI namespace çakışması)

**Spec:** repair'ler proaktif ve **ACK-bağımsızdır** (fec-core §4/§6); engine §9 ARQ **orijinal
DATA**'yı kapsar. Kaybolan repair "eksik kalan redundans"tır, yeniden iletilecek yük değil.

**Kod (öncesi):** `queue_losses` `Duplicate` dışındaki TÜM köken'leri `pending_lost`'a itiyordu —
**FecParity dâhil**. Parity `unit.id = esi` taşır; ESI, kaynak birim-id'lerinden **farklı bir
namespace**tir. Ürün runtime'ı retransmit ederken `store.get(&unit_id)` ile **yanlış (ilk) kaynak
birimi** gönderirdi. Aynı namespace tehlikesi fate-estimator tarafında zaten tanınmıştı
(`origin == Data` koşulu), ARQ tarafında tanınmamıştı. **FEC ürün default'una alındığı için bu
kusur canlıydı.**

**Düzeltme:** FecParity kökenli kayıp `pending_lost`'a girmez (inflight düşürme ve tombstone
korunur; `repair_loss_not_arqed` sayacıyla görünür).

**Ürün-yolu kanıtı:** netem %20 kayıp koşusunda `repair_not_arqed=5` — düzeltme öncesi bunlar
5 yanlış DATA retransmit'i olurdu.

**Kapsam notu (dürüstlük):** `kama-engine/tests/n8_v2.rs` iptal yolunu PARITY girdileri üzerinden
ölçüyordu (kendi kurulum yorumu: "parity'ler pending_lost'a girer") — yani düzeltilen kusura
dayanıyordu. Test yeni semantiğe göre güncellendi (parity ARQ'ya girmez → iptal edilecek girdi yok).
**Açık kapsam boşluğu:** RECOVERED'ın **kaynak** birim retransmit'ini iptal etmesi için doğrudan bir
test kurmayı deneyip güvenilir hâle getiremedim; tahmine dayalı assertion yazmaktansa testi
kaldırdım. Mekanizma kodda korunuyor (`pending_lost.retain`), ama **doğrudan testi yok** — backlog.

## KALEM-4 — Emergency MinimalParity spec'in tersini yapıyordu

**Spec (engine §11 L771-772 / TDD §5.6 L584-585, NORMATİF):** acil durumda davranış FRAMING'e bağlıdır:
`Tiny` → zaman-aralıklı D× kopya; **`SegmentBlock` → minimal parity shard** (düşük r/k, ACK-bağımsız).

**Kod (öncesi):** çağıran `Framing::Tiny`'yi SABİT veriyordu → `Framing::SegmentBlock` ve
`RecoveryAction::MinimalParity` hiçbir yerden çağrılmıyordu (denetim: "nowhere"); latch ayrıca
acil durumda parity'yi koşulsuz 0'a düşürüyordu ("temporary safety interlock").

**Düzeltme:** framing FEC blok-framing'inin aktifliğinden türetilir (`emergency_framing()`); latch
`RecoveryAction::MinimalParity { permille }` geldiğinde onu TÜKETİR (oran `EmergencyConfig::fixture()`
değeridir — kalibrasyon YAPILMADI). FEC kapalıyken davranış birebir aynıdır (Tiny → kopya, parity 0).

**Test:** `kalem4_acil_framing_fec_aktifligine_bagli` — FEC kapalıyken minimal-parity kolu seçilemez;
framing eşlemesi (`Tiny→Duplication`, `SegmentBlock→MinimalParity{>0}`) doğrudan pinlenir.

## Kapı

fmt · clippy `--all-features` ve `--no-default-features` 0 · PR test kümesi (workspace, ağır
entegrasyon panelleri hariç) yeşil · kama-engine 4 binary yeşil · aktivasyon koşusu ürün profilinde
(`profile=kama-v1-full`) sayaçları üretiyor.

**FEC RESEARCH CLOSED · FEC IMPLEMENTATION COMPLETENESS CLOSED** (bir açık owner kalemi:
§8.5 sıfırlama köşesinin politikası; bir backlog kalemi: RECOVERED→kaynak-iptal doğrudan testi).
