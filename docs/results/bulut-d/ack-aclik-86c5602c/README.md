# Bütçeli ACK'in dar dönüş hattında aç kalması: ham kanıt ve karşı testin ön kaydı

Bulut-D, 2026-11-23. Bulgu sahip tarafından kabul edildi. Düzeltme Yerel'in alanında
(`Receiver::yield_to_uplink`); bu dalda ürün kodu yok, yalnız ham kayıt var.

## Ölçülen ağaç ve komut

- Aday `cand/asym-combined` @ `86c5602caa18ff592c201e23bd0946d17e43a779`.
- Üstüne yalnız test içi zaman damgası eklendi: [`olcum-yamasi.diff`](olcum-yamasi.diff), yalnız
  `crates/kama-integration/tests/ack_ref_wire_size.rs`. Bu yama commit edilmedi; ürün davranışını
  değiştirmez, yalnız `eprintln!` ve koşu sonunda en çok 60 s bekleme ekler.
- Komut (debug, CI'daki `ack reference` işiyle aynı derleme):

  ```
  git apply docs/results/bulut-d/ack-aclik-86c5602c/olcum-yamasi.diff
  cargo test --locked -p kama-integration --test ack_ref_wire_size \
    ack_ref_wire_size_link_profile -- --exact --nocapture
  ```

- Kol: yön profili açık (Alice 12 500 / 62, Bob 62 / 12 500 B/s). İleri yönde %30 tohumlu kayıp.
  Alice 30 metin yolluyor; Bob her metne bir AMP alındısı yazıyor (Bob'un 62 B/s yukarı hattı).
  ACK V2 (0x14), bütçe %25 (`RETURN_BUDGET_PERMIL` = 250).

Satır biçimi:
- `PROBE session_up bob_log=<adres>`: Bob'un ACK kaydının adresi. Testin `ack_mühürlenen`
  sayısı yalnız bu kaydı sayar.
- `PROBE ack_seal log=<adres> len=<çerçeve B> t=<s>`: mühürlenen her ACK çerçevesi, oturumun
  kurulduğu andan itibaren.
- `PROBE texts_done`: 30 metnin hepsi Bob'a teslim edildi.
- `PROBE first_bob_ack_after_texts`: yalnız seri B; metinlerden sonra Bob'un ilk ACK'ine dek
  geçen süre (en çok 60 s beklenir).

## Ham sonuç: 28 koşu

| seri | koşu | uç ayrımı | Bob'un ACK'leri (s) | hüküm |
|---|---|---|---|---|
| ön (damgasız) | 6 | yok | `ack_mühürlenen=1` × 6 | geçti |
| ön (iki uç karışık) | 4 | yok | `ack_mühürlenen=1` × 4 | geçti |
| A ([`ham/A-terminal.txt`](ham/A-terminal.txt)) | 6 | var | 1,147–1,160 × 5; **koşu 1: yok** | koşu 1 `vakum: ACK mühürlenmedi` |
| B ([`ham/B01.txt`](ham/B01.txt)…[`B12`](ham/B12.txt)) | 12 | var | 1,147–1,388 × 11; **B01: 33,925** | geçti (60 s bekleme ile) |

- Uçları ayrılan 18 koşunun 16'sında Bob tek ACK'ini 1,147–1,388 s'de mühürlüyor (79 B,
  16 aralık), sonra koşu sonuna dek ACK yok.
- 2/18 koşuda (A1, B01) Bob 6,2 s'lik pencerede hiç ACK mühürlemiyor. İkisinde de Alice'in
  ilk ACK'i +0,5 s kaymış (0,528 / 0,535 s; diğerlerinde 0,010–0,241 s).
- **B01'de Bob'un ilk ACK'i 33,925 s'de çıkıyor, yani 30 metnin teslim edilmesinden
  27,698 s sonra.** Aynı aralıkta Alice ≈ 1,14 s'de bir ACK mühürlüyor (12 500 B/s yönü).
- CI kırmızısı: koşu 45135106628, iş 137559496250 (`ack_mühürlenen=0`). Bu evrenin aynısı.
- Seri A'nın PROBE satırları stderr'den terminale yazıldı; dosyaya terminal çıktısından aynen
  aktarıldı. Seri B'nin dosyaları komutun ham çıktısıdır.

Aralıklı test aynı biçimde yeniden koşulmayacak (sahip, 2026-11-23). Karşı test aşağıda
deterministik kurulur.

## Mekanizma (koddan)

- `Receiver::yield_to_uplink` (`crates/adaptive-transport/src/receiver.rs:835–841`):
  `uplink_yield` açıkken alarm dışı her ACK denemesi `true` döner ve yol verir. Bekleme süresi
  sınırsız.
- `uplink_yield` her denemede `tx.control_should_yield()` ile kurulur
  (`crates/client-core/src/runtime.rs:5984`, `:6109`).
- 30 alındı × ≈ 70 B, 62 B/s'de ≈ 30 s hat ediyor. İlk ACK denemesi dolu hatta denk gelirse ACK,
  kuyruk boşalana dek çıkmıyor; 33,9 s bununla tutarlı.
- "Tek yol ver" kuralı (`:842–848`) yalnız alarm ACK'ine uygulanıyor.

## Var olan sözleşme sınırı

`feedback_horizon_ns(62)` (`receiver.rs:114`) şöyle tanımlı: "bir birimin teyidi en geç bu ufuk
ve yol RTT'si kadar sonra gelir"; gönderici uçuş tabanını bu varsayımla kuruyor.

- en büyük ACK = 65 + 4 × 16 = 129 B;
- bütçe = 62 × 250 / 1000 = 15 B/s (tamsayı bölme);
- ufuk = 129 / 15 + 129 / 62 = 8,600 + 2,081 = **10,681 s**.

B01'deki 33,9 s bu ufku üç kat aşıyor. Bütçe aralığı (129 / 15 = 8,6 s), Bulut-B'nin V2
benzetiminde ölçtüğü ACK aralığıyla (8,55–8,60 s, #245) aynı.

## Karşı testin ön kaydı (düzeltmeden önce; yazıcı Yerel)

Düzeltme aşağıdakilerin hepsini aynı anda sağlamalı. Biri bozulursa yama reddedilir.

**K1 · ACK, dolu dönüş hattında süresiz beklemez.**
- Düzenek deterministik olmalı; aralıklı `ack_ref_wire_size_link_profile` kullanılmaz.
  Alıcının ilk ACK denemesinden itibaren dönüş hattı sürekli veriyle dolu (her denemede
  `control_should_yield() == true`) ve ACK isteyen varışlar sürüyor.
- İddia: ACK isteyen ilk varıştan, ve sonra her ACK'ten itibaren, bir sonraki ACK en geç
  `feedback_horizon_ns(return_bw)` içinde mühürlenir (62 B/s'de 10,681 s). Yeni sabit yok.
- Bugünkü kodda kırmızı olmalı (B01: 33,9 s). Düzeltme çıkarılınca kırmızıya dönmeli.

**K2 · alındı ≤ 4 s korunur.**
- Mevcut ürün kabul ölçütü. Bütün ürün hücreleri, tohum 1–3. Özellikle oturum başı
  (`receiver.rs:830–833`).
- Bilinen ret örneği: alarm dışı ACK'e aynı "tek yol ver" kuralı uygulandığında oturum başında
  ACK'ler hattı arka arkaya doldurmuş ve alındı dönüşü **2,2 → 6,9 s**'ye çıkmıştı (ürün testi
  `asym_product_outcome`, tohum 2). Bu yüzden o kural tek başına çözüm değildir.
- Ret: herhangi bir hücrede ya da tohumda alındı > 4 s.

**K3 · ACK bütçesi %25 kalır.**
- Her pencerede Bob'un ACK tel baytı ≤ 0,25 × dönüş hattı × süre + kova tavanı (2 × 129 B).
- Dinamik bütçe, %50 ya da yeni ACK türü yok.

**K4 · Verinin önüne en çok bir denetim datagramı geçer.**
- Mevcut kural (`receiver.rs:819–820`). Hattın boşaldığı her anda ACK, verinin önüne en çok bir
  datagram geçer.

**Ölçü:**
- Benzetimde ya da birim sürücüsünde mühür anı.
- Ürün hücresinde Web'in mevcut izinden Bob'un İZ-ACK mühür anları ile dönüş yönünün İZ-HAT
  satırları (Bob → Alice, bütün türler). Yeni koşu yalnız düzeltme dalı için Web'in ölçüm
  kuyruğuna verilir.
