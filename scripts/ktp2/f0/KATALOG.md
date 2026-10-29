# KTP2-F0 ortak ölçüm kataloğu (orkestratör; bütün şeritler bunu kullanır)

Dayanak: `ISEMRI-KTP2-F0-2026-10-29` §1 (birincil hedef ağ) ve §3. Bu dosya ortak profil kimliklerini, kolları ve koşu kurallarını tanımlar. Şeritler kendi hücre dosyalarını `cells/<şerit>/` altında bu kimliklerle yazar.

## 1. Kollar

| kol | SHA (origin) | ikili | not |
|---|---|---|---|
| `taban` | `ktp2/f0-taban` (main `a858f3fc` + yalnız ölçüm kodu) | `$BIN_TABAN` | bugünkü ürün motoru |
| `aday` | `ktp2/f0-kol-05b` (`49864510` + aynı ölçüm kodu, merge) | `$BIN_ADAY` | 05b adayı; yeni geliştirme yok |
| `quinn` | taban ikilisinin quinn kolu | `$BIN_TABAN` | BİRİNCİL = taşıma-ACK teslim tanımlı kol (`--rival-delivery transport-ack`); `scripts/ktp2/f0/arms_rakip.sh` `rakip_arms` (Ajan D). Mesaj başına bağımsız uni-stream; teslim = `SendStream::stopped`; BBR; el sıkışma tavanı `--handshake-timeout-s` |
| `tcp` | taban ikilisinin TCP kolu | `$BIN_TABAN` | BİRİNCİL = taşıma-ACK kolu: teslim = `snd_una` (`/proc/self/net/tcp`). Tek bağlantı, cubic (makinede `tcp_bbr` yüklü değil), raporda yazılır |
| `quinn_rpl`, `tcp_rpl` | eski kollar (uygulama yanıtıyla teslim) | `$BIN_TABAN` | YALNIZ betimsel. Uygulama yanıtı ileri yönde mesaj başına ek bir ACK paketi yükler (Ajan D, D-1) |

**Bench Kama kolunun kapsamı (beyan, Ajan D D-9 ve Ajan B B-K7).** `bench --transport kama-product` motor ve ürün runtime'ıdır, ama:
- kripto yok (`IdentitySeam`); kurulum maliyeti yapısal olarak sıfır;
- zarf 16 B, ürün 22 B;
- S1 `Reorder` sıralaması yok; teslim motorun sırasız teslimidir.

Bu yüzden bench kıyasları Kama için ÜST SINIRDIR. Gerçek ürün yolu (Noise, 22 B zarf, oturum geneli sıralama, kama-app) Ajan A'nın `f0_flows` düzeneğinde ölçülür.

- Ürün kodunun davranışı değişmez.
- Ölçüm kodu (bench, örnekler, betikler) iki Kama koluna da AYNI commit'lerle girer. Örnekler: `crates/bench`, `crates/kama-integration/examples`, `scripts/ktp2/f0`.
- Davranış değiştirmeyen salt-okunur sayaç gerekirse iki kola birden eklenir ve commit mesajında `[salt-okunur sayaç]` diye belirtilir.

## 2. Profil kimlikleri

İki yön simetrik, aksi yazılmadıkça. Netem dizesi her yöne AYNEN `tc qdisc replace dev <dev> root netem <dize>` olarak girer.

Kimlik biçimi `<hız>_<gecikme>_<kayıp>`.

| eksen | değer | netem parçası | anlam |
|---|---|---|---|
| hız | `r25` | `rate 200bit` | 25 B/s (0,2 kbit/s) |
| | `r1k` | `rate 8kbit` | 1 kB/s (8 kbit/s) |
| | `r12k` | `rate 100kbit` | 12,5 kB/s (100 kbit/s) |
| gecikme | `d300` | `delay 300ms 60ms distribution normal` | RTT ≈ 600 ms, tek yön σ 60 ms |
| | `d1000` | `delay 1000ms 200ms distribution normal` | RTT ≈ 2 s, tek yön σ 200 ms |
| kayıp (birincil) | `l0` `l10` `l20` `l30` | `loss N%` | rastgele, bağımsız |
| kayıp (stres) | `l50` `l80` | `loss N%` | yalnız raporlanır; karar yönlendirmez |

Dizeler:
- Sıra her zaman şudur: `delay … distribution normal loss N% rate X`. `l0`'da `loss` parçası yazılmaz.
- netem `limit` varsayılan değerinde bırakılır (1000 paket). Gecikme hattındaki paketler de bu sınıra sayılır; daha küçük bir limit, yüksek RTT'de yapay düşüşe yol açardı.

Bilinen netem davranışları (beyan):
- `rate` varken jitter paketleri yeniden sıralamaz. "Değişken gecikme" sırayı korur.
  - Sırayı koruyan jitter, ortalama gecikmeyi profil değerinin üstüne çıkarır: bir paket, önündekinden önce çıkamaz.
  - Örnek (Ajan B, `fizik.py`): r12k/d1000/65 B'de RTT 2,02 s yerine ≈ 2,37 s.
  - Bu yüzden motor srtt'si iki ölçüyle kıyaslanır: profil RTT'si ve sıra-koruyan model RTT'si.
- netem `rate`, paketin L2 dahil boyunu sayar: Ethernet 14 + IP 20 + UDP 8 = 42 B/paket. Kapasite muhasebesinde bu ayrı sütundur. Gerçek değer koşu başına `realized_pre/post.json` qdisc sayaçlarından doğrulanır.
- Kayıp tohumsuzdur: çekirdek RNG'si hücre başına rastgele. Koşular eşlenemez. Birim = koşu.

## 3. Koşu kuralları

- `f0_cell.sh` kollar arasında sırayı her tekrarda döndürür.
- Sessiz yeniden koşu yok. Düşen koşu (`rc ≠ 0`, olay yok) düşük kalır ve raporlanır.
- Hüküm için N ≥ 30. N < 30 olan hücre yalnız betimseldir ve öyle etiketlenir.
- Her hücre için koşular arası dağılım raporlanır: medyan, IQR, min–maks.
- Koşuları yalnız orkestratör başlatır (`f0_grid.sh`). Şeritler hücre dosyası ve tahmini koşu süresi verir.
- cargo yalnız orkestratörde koşar.
- Öncelik: birincil hücreler önce. Stres hücreleri birincillerden sonra ve N ≥ 10 ile, betimsel olarak.
