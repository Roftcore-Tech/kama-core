# netem-v1/k258-2 — PR #258 GÜNCEL BAŞ (`b0659064c31dd0074372536c7bfb1f6935506032`) hedefli gerçek hat koşusu

Yalnız ölçüm çıktısı; birleştirilmez. Neden ataması ve öneri yok; tablolar bağımsız doğrulanmadı.

- Senaryo v1, tohum 1, PTT'siz; düzenek `netem-v1/k258-1` ile aynı (hücre, koşucu, düzenek özetleri `on-kayit.txt`'te). Tek koşu (`1/`).
- İkili: hazır `f0_flows`, sha256 `e56a2a1919815992e37d99958965c439edb1e6213a7ef876d2c7b75c4f63d87c`; derleme yapılmadı.
- Kirli denetimi: ön kayıtta düzeltilen ölçütle (K2 ve K8 yalnız ölçüm penceresi [t_start, t_end] içinde) TEMİZ; yineleme yok.
  `1/kirli-pencere.txt` (K2p, K8p), `1/analiz-ozet.txt` (öbür ölçütler; aynı dosyadaki K2 ve K8 satırları ESKİ tanımdır).
- Karşılaştırma sütunları yeniden koşulmadı: önce = `netem-v1/kancali-1/1`, 911f2c6d koşu 1 ve 2 = `netem-v1/k258-1/{1,1-yineleme}`.

Dosyalar:
- `on-kayit.txt` — ön kayıt (koşudan önce) + koşu sonrası ek. `onkontrol/` — netem'siz ön kontrol.
- `1/v1_taban_t1/aday/rep1/` — ham kayıtlar (`alice.jsonl`: `lg`, `ack_rx`; `bob.jsonl`: `ack`), `kuyruk.jsonl`, `basamak.jsonl`, `cap/` beş düzlem, `tc-d-qdisc-show.txt`, `kayit/`, `cmd.txt`.
- `1/k258-ek.txt/json` — (a)–(c)'nin tam hâli, her `lg` satırı. `1/kapsama.txt/json` — hüküm satırı başına aralığın uçları ↔ yoklama penceresi (türetme; tanım `arac/kapsama.py` başında).
- `1/yukselis.txt` — `lg.raised` satırlarında alıcının çift aralığı dökümü; `1/eski-kural.txt` — kayıttaki girdilerle üç kuralın hesabı. İkisi koşudan SONRA eklendi (ön kayıtta yok).
- `1/ack-tablo.md/json`, `1/ozet.txt`, `1/analiz-*`, `1/kosu.log`, `1/k2-ornek.txt` (konak gözlemcisi: 0,5 s'de bir cargo/rustc süreç sayısı).
- `tablolar.md` — (a) bu koşu; (b)–(d) dört koşu yan yana (önce | 911f2c6d koşu 1 | 911f2c6d koşu 2 | b0659064).
- `arac/` — kullanılan betikler. `arac-deneme/` — `kapsama.py` ve `kirli_pencere.py`'nin koşudan önce 911f2c6d koşularının ham dosyalarındaki deneme çıktıları.
- `sha-ham.txt` — yerel ham dosyaların özetleri (yol temizliğinden önce; yol içeren dosyalarda yayımlanan kopyanın özeti farklıdır).

Yol temizliği: makine yolları `<scratch>`, `<wt>`, `<home>` ile değiştirildi (`arac/yayin_hazirla.sh`).
