**Yerel (ölçüm): araç kanıtı — kuyruk zaman çizelgesi ve hız basamağında kuyruk sürekliliği. Yalnız sonda trafiği; ürün koşusu değil, hüküm yok. Geçme koşulu (koşudan önce yazıldı) sağlandı: hız basamağı kuyruğu koruyor.**

**Düzenek.** `f0_cell.sh` değişmedi (sha256 `14cffdad…`). `HDIR` scratch kopyaya bakıyor: `kapi3_harness.sh` = depo dosyası (ilk 233 satır bayt-aynı, `38dcea4f…`) + sona eklenen 53 satır; `pkt_capture.py` + 3 satır (paket kimliği: UDP gövdesinin özeti). Depoya girmedi; sudo yok (rootless, üç ağ ad alanı).

**İleri hat iki kademe; kayıp darboğazdan SONRA (ölçüldü).**
`ns-A va0 → orta: ma0 girişi → ifb0 [tbf 1: + pfifo 10: 64 paket] (darboğaz) → mb0 çıkışı netem 20: (300 ms + %20, tohumlu) → ns-B`. Dönüş: `ma0` çıkışında tek netem (değişmedi).

```
ip link add ifb0 type ifb; ip link set ifb0 up
tc qdisc add dev ifb0 root handle 1: stab overhead -14 linklayer ethernet tbf rate 100kbit burst 1200 limit 64
tc qdisc add dev ifb0 parent 1:1 handle 10: pfifo limit 64
tc qdisc add dev ma0 handle ffff: ingress
tc filter add dev ma0 parent ffff: protocol ip matchall action mirred egress redirect dev ifb0
tc qdisc replace dev mb0 root handle 20: netem delay 300ms loss 20% seed 1 limit 10000
tc qdisc replace dev ma0 root netem delay 300ms rate 496bit -14 limit 64
# hız basamağı (kuyruk yerinde kalır):
tc qdisc change dev ifb0 root handle 1: stab overhead -14 linklayer ethernet tbf rate 50kbit burst 1200 limit 64
tc qdisc change dev ifb0 root handle 1: stab overhead -14 linklayer ethernet tbf rate 100kbit burst 1200 limit 64
```
`tc -d qdisc show` (orta ad alanı, koşu başında, aynen):
```
qdisc netem 8008: dev ma0 root refcnt 13 limit 64 delay 300ms rate 496bit packetoverhead -14 seed 15905914021246086606
qdisc ingress ffff: dev ma0 parent ffff:fff1 ----------------
qdisc netem 20: dev mb0 root refcnt 13 limit 10000 delay 300ms loss 20% seed 1
qdisc tbf 1: dev ifb0 root refcnt 2 rate 100Kbit burst 1200b/1 mpu 0b limit 64b linklayer ethernet
 linklayer ethernet overhead -14
qdisc pfifo 10: dev ifb0 parent 1:1 limit 64p
```

**(i) Kuyruk zaman çizelgesi.** Orta ad alanında `tc -s -j qdisc show` her 50 ms: 374 örnek, aralık medyan 50,0 ms, en büyük 50,5 ms (backlog bayt/paket, drops, overlimits, sent; her örnekte konak yükü). Her örnekte darboğaz aynı qdisc: `tbf 1:` kökte, `pfifo 10:` `1:1` altında, limit 64 paket. Kol başında bütün sayaçlar 0.

**(ii) Hız basamağı, kuyruk doluyken** (sonda: 1000 B IP paketler; 60 + 20 paket; iki tekrar, sayılar tekrar 1 / tekrar 2):

| basamak | uygulandığı an (t0'dan; komut öncesi → sonrası) | hedeften sapma | ÖNCE (komuttan 8,0 ms önce) backlog / drops / sent | SONRA (komuttan ≈ 3 ms sonra) backlog / drops / sent | arada çıkan paket | sonraki çıkış aralığı (medyan) |
|---|---|---|---|---|---|---|
| 12 500 → 6 250 | 1 650,012 → 1 654,229 ms / 1 650,011 → 1 654,099 ms | +0,012 / +0,011 ms | 41 p, 41 000 B / 0 / 19 | 40 p, 40 000 B / 0 / 20 | 1 | 160,004 / 159,999 ms (n = 15) → 6 249,8 / 6 250,1 B/s |
| 6 250 → 12 500 | 4 150,008 → 4 153,855 ms / 4 150,008 → 4 153,523 ms | +0,008 / +0,008 ms | 45 p, 45 000 B / 0 / 35 | 45 p, 45 000 B / 0 / 35 | 0 | 79,985 / 79,992 ms (n = 44) → 12 502,3 / 12 501,2 B/s |

Basamak öncesi aralık 79,987 / 79,986 ms (n = 18). Backlog ve drops `tbf 1:` ile `pfifo 10:` kayıtlarında aynı.

Geçme koşulu (önceden): (a) handle değişmedi — evet (`1:` / `10:`, 374 örneğin hepsinde); (b) backlog sıfırlanmadı — 41 → 40 (arada 1 paket çıktı), 45 → 45; (c) drop sayacı artmadı — 0 → 0; (d) kuyruktaki paketler yeni hızla çıktı — 80 → 160 → 80 ms; (e) basamak yüzünden kaybolan paket = sunulan 80 − kuyrukta düşen 0 − hatta düşen 18 − teslim 62 = **0**.

**Basamağın yan etkisi (ölçüldü, sınır):** `tbf change` kovayı doldurur (1 200 B). Her basamakta en çok bir paket hız sınırından erken çıkar: basamak 1'de komuttan 4,57 ms sonra bir paket çıktı, sonraki aralık 128,2 ms (160 yerine); basamak 2'de ilk aralık 64,1 ms (80 yerine). Sonraki aralıklar tam.

**Aynı sonda turunda doğrulananlar:**
- Hız (L2 14 B düzeltmesi `stab overhead -14`, IP boyu): ileri 12 502 B/s ve 6 250 B/s; dönüş 8 × 48 B, aralık 774,19 ms = 62,00 B/s.
- Gecikme: darboğaz çıkışından B'ye medyan 300,13 ms (299,94–300,46).
- FIFO 64: boş hatta 80 paketlik patlama → 1 + 64 çıktı, 15 kuyrukta düştü (`tbf` drops 15).
- Kayıp: darboğazdan çıkan 145 paketin 31'i hatta düştü (%21,4; `netem 20:` drops 31). Tohum belirlenimli: iki tekrarda teslim kümesi aynı (114 / 114). Sıralı çekirdek RNG'si; fikstürün anahtarlı deseniyle aynı değil.
- **Kayıp sırası:** B'de ardışık varış aralığı 79,97 ms; kayıp atlayan aralık, atlanan paket başına 80,02 ms. Düşen paket hat süresini harcadı: kayıp darboğazdan sonra. (#233'teki tek netem dumanında tersiydi: kayıp darboğazdan önce. O izle bu düzeneğin izi eşdeğer sayılmaz.)
- Zaman çapası: basamak uygulayıcısı koşunun `sync/t0` değerini (Unix ns; `f0_flows` aynı dosyayı yazar) okur; komut hedef anından +0,008–0,012 ms sonra başladı, 3,5–4,2 ms sürdü.
- Yakalama kaybı 0 (beş düzlem); konak yükü en çok 0,58.

**Sınırlar.** Darboğaz `ifb0`'da (giriş yönlendirmesi), tek netem değil; kova 1 200 B (boş hattan sonraki ilk paket serileşmeden çıkar). `limit 64` yalnız darboğaz kuyruğunu sayar; gecikme hattı ayrı (limit 10 000). El sıkışma ve sağlama dosyaları bu düzenekte hat dışı (ürün koşularında geçerli; sonda bunları kullanmaz).

🤖 Generated with [Claude Code](https://claude.com/claude-code)
