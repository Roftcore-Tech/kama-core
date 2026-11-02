# K4G-05b NETEM-7 — ön-kayıt (sonuçlardan ÖNCE)

Tarih: 2026-10-25 (gece). Dayanak: CEVAP 15 v3 §2.3 ("netem kapılarının (7 profil) değişmemesi yanlışlanabilir TAHMİNDİR … kapılar kayarsa dur, raporla") ve §2.4 (son aday kapısında "netem 7 profil").

## 0. Neden yeni koşucu

Depoda koşturulabilir bir "7 profil regresyon kapısı" yok. Gecelik `hostile` shard'ları yalnız aktivasyon kanıtı üretiyor (`nightly.yml` [FAZ-5 KAPISI] yorumu). `kapi_calrun.sh` stub kolu (`--transport kama`) koşturuyor ve ürün değil. `phase5_cell.sh` ürün kolunu koşturuyor ama katalogu T0–T7; ADR-18'in 7 profilinden yalnız p1, p2 ve p5'i içeriyor.

Bu yüzden `~/kama-k4g-work/tools/netem7_cell.sh` yazıldı:

- `phase5_cell.sh` deseninden türetildi (barrier, rc dosyası, kol sırasını döndürme);
- `kapi3_harness.sh` ile 3-netns kuruyor;
- profil dizeleri `kapi_manifest.py:40-55`'ten VERBATIM;
- p6 pencere planı `:110`'dan VERBATIM.

Yeni bozulma modeli yok.

## 1. Kollar ve sabitler

- **Kollar** (aynı netns, aynı koşucu, dönüşümlü):
  - taban = `origin/main` @ `1b1425d2`;
  - aday = 05b son aday SHA'sı (koşu anında `cell.json`'a yazılır).
- **İkili:** her iki kol `cargo build --release --locked -p bench --bin bench`, depo toolchain'i 1.96.0, kendi worktree'sinde, ayrı target.
- **Transport:** `--transport kama-product`, sunucu `--payload 256`.
- **Geometri, p6 dışında:** phase5 sözleşme geometrisi, 100 × 256 B @ 50 ms, D = 20 s.
- **Geometri, p6:** 100 × 256 B @ 1000 ms (kapi CLASS_PLAN "I" paced aralığı), D = 20 s.
  - Gerekçe: 50 ms'de sunum 5 s'de biter, 30–60 s penceresi hiçbir mesaja değmez.
  - Pencere planı her tekrarda istemci başlamadan hemen önce baştan oynatılır.
- **N:** kol × profil başına 20 tekrar. Kol sırası her tekrarda döner.
- **Paralellik:** profiller arasında en çok 3 shard; profil içinde seri.
- **Ortam:** koşu sırasında başka cargo süreci yok.

## 2. Metrikler ve karar kuralı

- **M1 (birincil):** koşu başına D = 20 s içinde teslim oranı (`phase5_cell_report.py` `comp()` ile aynı formül).
- **M2 (ikincil):** koşu başına teslim edilenlerin gecikme medyanı.
- **Test:** iki yanlı, kesin, bağ-düzeltmeli Mann–Whitney. RNG yok. α = 0.05 / 7.
- **KAYMA(M1)** ⇔ |medyan(aday) − medyan(taban)| ≥ 0.05 **ve** p < α.
- **KAYMA(M2)** ⇔ medyan oranı ∉ [0.8, 1.25] **ve** p < α.
- **Yön:** iki yönlü sayılır. Tahmin "değişmez" olduğu için iyileşme de tahmini yanlışlar.
- **Altyapı arızası:** olay içermeyen ya da `out.jsonl` üretmeyen koşu.
  - N'e sayılmaz, raporlanır.
  - Kol-profil başına 2'den fazlaysa ALTYAPI-DUR.
- **Hüküm cümleleri:**
  - Herhangi bir KAYMA, EKSİK ya da ALTYAPI-DUR → **DUR**, sahibe rapor, merge yok.
  - Aksi hâlde: "yedi profilde ön-kayıtlı kayma saptanmadı (N = 20/kol)".
- **Bu ölçümün kapsamı dışında:**
  - Kama ↔ rakip kıyası ve performans iddiası yok (ADR-hostile-validation-contract: n < 30 = smoke).
  - "Kapı geçti" sözü kullanılmaz; kapı sözleri sahibindir.
- Sessiz yeniden koşu yok. Düşen shard düşük kalır ve raporlanır.

## 3. Tahminler

- **Sahibin tahmini (CEVAP 15 v3 §2.4):** netem kapıları değişmez.
- **Orkestratör tahmini:** yedi profilin hiçbirinde KAYMA yok. Gerekçe:
  - 05b değişiklikleri (B′ karşılık hakkı, K3 ARQ payı rezervi, DUR-40 yaş tavanı) yalnız token yetmezliği ya da ≥ RTO_MAX bekleyen kayıt varken devreye girer;
  - 100–200 kbit/s'de ve 50 kbit/s'de çağrı başına token birikimi yüksektir.
- **Sapma beklenen yerler:**
  - **p6_blackout:** 30 s kesinti = RTO_MAX (30 s) sınırı; DUR-40'ın yaş tavanı tam bu eşikte tetiklenebilir.
  - **p3_loss95:** kopya sınıfı en yoğun burada.
- **Sapma çıkarsa sorgulama sırası:** önce enstrüman, sonra türetim, sonra kod. Test yeşile boyanmaz.

## 4. Araç doğrulaması (sonuç taşımaz)

Koşudan önce `netem7_cell.sh` smoke edilir. Smoke'ta iki kol da TABAN ikilisidir: aday ikilisi sonuç vermeden araç doğrulanır. Smoke çıktısı ayrı dizinde tutulur ve analize girmez.

Kontroller:

- barrier;
- `realized_*.txt`;
- p6 `trace.plan.log` (30 s ve 60 s geçişleri);
- `manifest.json` profil kimliği;
- `rc`.
