# Yerel ham izler — netem taban koşuları (senaryo v1)

Yalnız ölçüm çıktısı; birleştirilmez. Ürün kodu: `yerel/poc-entegrasyon` @ `73715edbede79bd17b5cc11408947b65f6b7b1fd` (aday `1a4f9efc` + #251). #250 (kuyruk tahmini) YOK.

- Kip: iki `f0_flows` süreci (ayrı ağ ad alanları), cc-port, `KAMA_ACK_FORMAT=bitmap`, yön hızı profili aynalı (12 500 / 62), PTT'siz.
- Senaryo: `netem-v1/netem-senaryo-v1.md` (koordinatör, #175 7096381261): 12 500 → 20 s 6 250 → 60 s 12 500 → 85,0–85,5 s 6 250 → 100 s; dosya 1,2 MB; metin 2 s'de bir.
- Hat: ileri darboğaz `ifb0` (tbf + pfifo 64, IP boyu) → `mb0` netem (300 ms + %20, `seed` = tohum): **kayıp darboğazdan SONRA**. Dönüş `ma0` tek netem 62 B/s, 300 ms, kayıpsız. Hız basamağında aynı kuyruk korunur (araç kanıtı: #250 yorum 7096775628; `netem-v1/sonda/`).
- El sıkışma ve sağlama dosyaları netem DIŞINDA (Unix soketi / dosya).
- Kayıp deseni sıralı çekirdek RNG'si; fikstürün anahtarlı kaybıyla satır satır karşılaştırılmaz.

## Dizinler
- `netem-v1/1`, `2`, `3`: taban koşuları, tohum 1–3. `v1_taban_t<N>/aday/rep1/` altında: `alice.json(l)`, `bob.json(l)` (f0_flows kayıtları; `ts.s[]` canlı ayna örnekleri), `kuyruk.jsonl` (darboğaz kuyruğu örnekleyicisi, ≤ 100 ms), `basamak.jsonl` (hız basamağının uygulandığı an ve öncesi/sonrası sayaçlar), `tc-d-qdisc-show.txt`, `realized_pre/post.json`, `cap/` (tel yakalamaları: hat öncesi/sonrası), `kayit/` (düğüm başlangıç ve pilot özetleri), `cmd.txt`.
- `netem-v1/kesmesiz-1`: aynı senaryo, hız basamağı YOK, tohum 1 (kısa daralma ölçütünün karşılaştırma kolu).
- `netem-v1/arac`: koşu ve analiz betikleri, hücre dosyaları, scratch düzenek (`hdir/`: depo `kapi3_harness.sh` + iki kademeli hat, örnekleyici, basamak uygulayıcısı).
- `netem-v1/sonda`: araç kanıtı (hız, kuyruk 64, kayıp sırası, basamakta kuyruk sürekliliği).
- `analiz-*.txt/json`, `uclu.*`, `metin-tablo.txt`: Yerel ölçüm ajanının YARIM KALAN ön hesapları. **Doğrulanmadı; kaynak sayılmaz.** Ayrıştırma Bulut-A ve Bulut-B'de.

## Bu kayıtta OLMAYAN alanlar (hükmü bağlar)
- ACK başına LineGuard tavanı (`lg` satırı) yok: `ts.s[].pr` pacer kovasının hızıdır, hat tavanı değil. CC ve hızlanma hükmü bu kayıttan verilemez.
- ACK tür sayacı yok (kip tel kanıtı dolaylı).
- Bob'da ilk geçerli kanıtın oluştuğu an yok: "düşüş → ACK çıkışı" süresi kanıt bekleme / bütçe / dolu dönüşe yol verme olarak **ayrıştırılamaz**.
- Alındının Bob'dan çıkış anı yok.
Bu alanlar `yerel/f0-olcum-kancalari` dalında ekleniyor.

## Kancalı koşu — `netem-v1/kancali-1` (2026-11-23, tohum 1, tek koşu)

Yalnız ölçüm çıktısı; birleştirilmez. Ürün kodu: `yerel/f0-olcum-kancalari` @ `e1012cf2048c1f8c485a16429fb5edfa64cf94c1` (taban #233 hazır SHA `125b9afa` + salt gözlem kancaları). Yukarıdaki "OLMAYAN alanlar" bu dizinde VAR; üstteki taban dizinleri (`1`, `2`, `3`, `kesmesiz-1`) değişmedi.

- İkili: `f0_flows`, sha256 `adf8f5add5f4eae5ea9d3b2c5c2ab1b959b79f1fd037d3f0add32be0b0e64a03`. `KAMA_GIT_SHA` verilmeden derlenmiş: başlangıç kaydında `çekirdek_git=bilinmiyor (derlemede KAMA_GIT_SHA verilmedi)` (`kayit/node-*-transport-start.txt`). SHA bağı: derleme ağacı `e1012cf2`'de ve temizdi; ikili o ağacın hedef dizininden alındı. Yeniden derlenmedi.
- Düzenek tohum 1 taban koşusuyla aynı: aynı hücre dosyası (`netem-v1/arac/v1_taban.cell`, sha256 `2a90bbba…`, DEĞİŞMEDEN), aynı `hdir/`, aynı koşucu. `--live-diag on` taban hücresinde zaten vardı; tek fark ikili (ve çıktı kökü). PTT yok.
- Hat: ileri `ifb0` tbf + pfifo 64 → `mb0` netem 300 ms + %20 (`seed 1`), kayıp darboğazdan SONRA; dönüş `ma0` netem 62 B/s + 300 ms. Hız planı `20:50kbit 60:100kbit 85.0:50kbit 85.5:100kbit`.
- Kirli ölçütü (`netem-v1/kirli-olcut.txt`, K1–K11): hepsi tuttu (`1/analiz-ozet.txt` sonu); yineleme yapılmadı. Ön kayıt: `on-kayit.txt` (koşudan önce).

### Dosyalar
- `1/v1_taban_t1/aday/rep1/`: taban dizinleriyle aynı düzen. Yeni satır türleri: `bob.jsonl` `ack` (Bob'un alıcı motorundan çıkan her ACK: `t`, `t_evidence`, `t_ready`, `t_wire`, `t_detect`, `t_alarm`, `t_first_try`, `t_first_defer`, `alarm`, `rise`, `wait_open`, `defer{}`, `defer_alarm{}`, `largest_pn`, `index`, `type`, `bytes`); `alice.jsonl` `ack_rx` (varan ACK: `t`, `at_ns`, `largest_pn`, `index`) ve `lg` (ACK başına hat koruması kararı: `decision`, `pace_before`, `pace_after`, `rate_*`). Eşleme: `ack.largest_pn` + sıra ↔ `ack_rx.largest_pn`; `ack_rx.at_ns` = `lg.at_ns`.
- `cap/`: paket yakalamaları, satır başına `pid` = UDP gövdesi özeti (düzlemler arası eşleme). ACK datagramı: `B_vb0` `out` (Bob arayüz çıkışı) → `MID_mb0` `in` → `MID_ma0` `out` (dönüş netem'inden çıkış) → `A_va0` `in` (Alice arayüz girişi). ACK'in yakalamadaki karşılığı boy (çerçeve `bytes` + 50 = IP boyu) + zaman (`t_wire` ± 5 ms, tek aday) ile bulundu; 23/23 tek aday.
- `1/ack-tablo.md`, `1/ack-tablo.json`, `1/ack-tablo-denetim.txt`: ACK başına tek satır (`arac/ack_tablo.py`); anlar ilgili hız basamağı komutunun başlangıcına göre ms. `1/ozet.txt`: ilk tavan kesmesinin dökümü, kuyruk, metin, basamak sayaçları (`arac/ozet.py`).
- `1/analiz-*.txt/json`: taban koşularındaki `netem-v1/arac/analiz.py`'nin bu koşudaki çıktısı.
- `sha-ham.txt`: ham dosyaların sha256'ları.
- Tablolar ve özet Yerel ölçüm adımının çıktısıdır; **bağımsız doğrulanmadı**. Yorum ve ayrıştırma içermez; ayrıştırma Bulut-A ve Bulut-B'de.

### Bu kayıtta OLMAYAN / sınırlı alanlar
- Bob'un 23. ACK'inin (tel çıkışı τ = 99,899 s) Alice satırı yok: koşu τ = 100,004 s'de bitti.
- `t_wire` = `send_to` dönüşü; çekirdek/arayüz sonrası anlar yalnız yakalamadan. Alice'te çekirdeğe varış anı üründe tutulmuyor; `ack_rx.t` motorun işleme anı (yakalama ile farkı ≤ 0,17 ms).
- `t_evidence` yalnız kanıtla bırakılan alarm ACK'lerinde dolu (6/23); öbürlerinde `null` (boş bırakıldı, neden tahmin edilmedi).
- [85,0, 85,5) s daralmasında darboğaza sunulan trafik 0 B (Alice'in veri akışı τ = 84,72 → 86,98 s arasında gönderim yapmadı): kısa dip dolu hatta uygulanmadı.
- Alice'in kendi `ack` satırları (52 adet, 0x14; alındı yönünün ACK'leri) tabloda yok.

Makine yolları `<scratch>`, `<wt>`, `<home>` ile değiştirildi.

## #258 hedefli önce/sonra — `netem-v1/k258-1` (2026-11-25, tohum 1; koşu 1 + bir yineleme)

Yalnız ölçüm çıktısı; birleştirilmez. Yama: PR #258, `yerel/gonderici-dongu` @ `911f2c6dd1cc1bfcebb30e655f2e3c13760ff41a` (ölçüm kancaları `cd15a3ae` + yoklama hükmü kuralı + ACK başına gözlem alanları). Önce = `netem-v1/kancali-1/1/` (yeniden koşulmadı; ikilisi `e1012cf2`; `e1012cf2` → `cd15a3ae` farkı yalnız belge yorumu satırları).

- İkili: hazır `f0_flows`, sha256 `6d6a10c739ccf29a35daa1eeea0ef90879804bd20cc712fd558f611ae204ce41`; derlenmedi. İki ucun başlangıç kaydında `çekirdek_git=911f2c6dd1cc1bfcebb30e655f2e3c13760ff41a` (`kayit/node-*-transport-start.txt`).
- Düzenek önce koşusuyla aynı: aynı hücre dosyası (`netem-v1/arac/v1_taban.cell`, sha256 `2a90bbba…`), aynı `hdir/`, aynı koşucu (cell.env: runner `14cffdad…`, harness `59fc4a0d…`), tohum 1, PTT yok. Tek fark ikili (ve çıktı kökü).
- Ön kontrol (`onkontrol/`, netem'siz loopback): başlangıç kaydı SHA'sı, Alice'e varan tel türü 0x16, `lg` satırında yeni anahtarlar (`front_wait_ns`, `by_bytes`, `by_delay`, `backlog_bytes`, `extra_backlog_bytes`, `pair_bound_wire`, `verdict`). Loopback'te `verdict` 6/6 `null` idi; alt alanları netem koşularının `lg` satırlarında görüldü.
- Ön kayıt: `on-kayit.txt` (beklenen sayılar PR gövdesinden; çözümleme tanımları; koşu 1'den önce). Yineleme kararı aynı dosyanın ekinde, yinelemeden önce.

### Kirli denetimi (K1–K11, `netem-v1/kirli-olcut.txt`)
- `1/` (koşu 1) ve `1-yineleme/` (koşu 2): ikisinde de K2 ve K8 TUTMADI, öbür dokuz ölçüt tuttu. Ön kayıtlı kural gereği tek yineleme yapıldı; yineleme de aynı iki ölçütte kaldı; başka koşu yok.
- K2: `kosu.log` "bitiş" satırında `cargo_rustc=1`. Sayılan süreç aynı kilidi bekleyen başka bir işin derlemesi; `rustc` başlangıcı kilidin bırakıldığı saniye (19:10:26Z / 19:20:08Z), ölçüm penceresinin kapanışı (`t_end`) 19:10:25,564Z / 19:20:07,635Z.
- K8: darboğazdan geçen UDP dışı 1 paket (ICMP, 99 B), τ = 100,09 → 100,20 s / τ = 100,07 s; koşu bitişi τ = 100,005 s.
- Yük (1 dk ortalaması, örnekleyici): koşu 1 1,13–1,64; koşu 2 3,48–5,38 (eşik 6,0).

### Dosyalar
- `1/`, `1-yineleme/`: `kancali-1/1/` ile aynı düzen (`v1_taban_t1/aday/rep1/` ham kayıtlar, `cap/`, `kuyruk.jsonl`, `basamak.jsonl`, `kayit/`; `ack-tablo.*`, `ozet.txt`, `analiz-*`). Ek: `k258-ek.txt/json` (`arac/k258_ek.py`): hüküm ACK'leri, `lg` satırı başına yazılan kuyruk ↔ gerçek darboğaz kuyruğu ↔ dönüş sıra beklemesi, gönderici susmaları, önce/sonra girdileri.
- `once/k258-ek.txt/json`: aynı aracın önce koşusunun ham dosyalarındaki çıktısı (önce kaydında `backlog_bytes` / `verdict` alanı yok; tutma, gönderim boşluğundan türetildi).
- `tablolar.md`: üç koşunun yan yana tabloları (`arac/yorum_tablo.py`). `sha-ham.txt`: ham dosyaların sha256'ları.
- `arac/`: `kos.sh`, `kos_yineleme.sh`, `bekle_kos.sh`, `onkontrol.sh`, `k258_ek.py`, `yorum_tablo.py`, `yayin_hazirla.sh`.
- Tablolar Yerel ölçüm adımının çıktısıdır; **bağımsız doğrulanmadı**. Neden ataması ve öneri içermez.

### Sınırlar
- Tek tohum; iki koşu da kirli ölçütünün iki maddesinde kaldı (yukarıda).
- Aynı tohum koşuları paket düzeyinde aynı yapmıyor: Alice arayüz çıkışı dizisi (boy + an ± 50 ms) önce ↔ koşu 1'de 146., önce ↔ koşu 2'de 423., koşu 1 ↔ koşu 2'de 146. pakette ayrışıyor.
- `tablolar.md`'deki "yalnız cephe ölçüsüyle sonuç" sütunu kayıttaki girdilerden hesaptır, ikinci bir koşu değildir.
- Kesmesiz karşılaştırma kolu ve PTT yok.

Makine yolları `<scratch>`, `<wt>`, `<home>`, `<scratch-kesik>` ile değiştirildi.
