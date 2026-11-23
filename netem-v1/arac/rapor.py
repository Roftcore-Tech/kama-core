#!/usr/bin/env python3
"""PR yorumu üretici (senaryo v1 taban). kullanım: rapor.py <v1 kökü> > yorum.md"""
import json, os, sys
V = sys.argv[1]
def n(x, nd=0):
    if x is None: return "–"
    s = f"{x:,.{nd}f}".replace(",", " ").replace(".", ",")
    return s
T = {}; C = {}
for t in (1, 2, 3):
    T[t] = json.load(open(f"{V}/{t}/analiz-veri.json"))
    p = f"{V}/kesmesiz-{t}/analiz-veri.json"
    if os.path.exists(p): C[t] = json.load(open(p))
S3 = lambda f: " / ".join(f(T[t]) for t in (1, 2, 3))
def band(v): return v is not None and 5625 <= v <= 6875
# hükümler
cc = {}; hs = {}; hz = {}; kd = {}
for t, d in T.items():
    cc[t] = "GEÇTİ" if (band(d["cc"]["tavan_a2"]) and d["cc"]["pre20_min"] >= 11875) else "KALDI"
    w = d["hs"]["w"]; tx = d["hs"]["texts_narrow"]
    ok = 90 <= w["sunulan_yuzde"] <= 100 and w["kuyrukta_dusen"] == 0 and w["bekleme_enc"] <= 393 and all(x is not None and x <= 1000 for x in tx)
    hs[t] = "GEÇTİ" if ok else "KALDI"
    z = d["hz"]
    if not band(z["pre58"]): hz[t] = "SINANMADI"
    else:
        ok = z["t_up"] is not None and z["b3"] is not None and z["t_up"] <= z["b3"] + 0.15 and z["kalici"] and z["w"] and z["w"]["sunulan_yuzde"] >= 90
        hz[t] = "GEÇTİ" if ok else "KALDI"
    if t in C:
        c = C[t]
        pre_ok = d["hz"]["tavan_84"] >= 11875 and c["hz"]["tavan_84"] >= 11875
        if not pre_ok: kd[t] = "SINANMADI"
        else: kd[t] = "GEÇTİ" if d["hs"]["w_short"]["bekleme_medyan"] <= c["hs"]["w_short"]["bekleme_medyan"] + 86 else "KALDI"
    else: kd[t] = "SINANMADI"
def daralma_tavan(d):
    a2 = d["cc"]["a2"]; vals = [d["cc"]["tavan_a2"]] + [x[2] * 12500.0 / 12214 for x in d["chg"] if a2 + 0.15 < x[0] < 60]
    return min(vals), max(vals)
o = []
P = o.append
P("**Yerel (ölçüm): netem taban koşusu — #250 öncesi, entegrasyon dalı. Senaryo v1, tohum 1–3, iki süreç, PTT'siz. Kuyruk düzeltmesinin \"önce\" kolu; CC ve hızlanmanın ilk netem ölçümü.**")
P("")
P("**Hükümler (ön kayıtlı pencereler; tohum 1 / 2 / 3). Tek hücre, üç tohum: hedefin kapandığını göstermez.**")
P(f"- **CC: {cc[1]} / {cc[2]} / {cc[3]}.** 20 s'den sonraki 2. ACK'te tavan {S3(lambda d: n(d['cc']['tavan_a2']))} B/s (bant 5 625–6 875). İlk geçerli örneğe gerçek süre: basamaktan {S3(lambda d: n(d['cc']['first_in']-20.0, 2))} s sonra. 20 s'den önce düşüş yok (tavan en az {S3(lambda d: n(d['cc']['pre20_min']))}).")
P(f"- **Hız sınırlama [35, 58): {hs[1]} / {hs[2]} / {hs[3]}.** Sunulan / kapasite %{S3(lambda d: n(d['hs']['w']['sunulan_yuzde'],1))}; kuyrukta düşen {S3(lambda d: n(d['hs']['w']['kuyrukta_dusen']))}; darboğaz beklemesi medyan {S3(lambda d: n(d['hs']['w']['bekleme_medyan']))} ms, en çok {S3(lambda d: n(d['hs']['w']['bekleme_enc']))} ms (eşik ≈ 393); daralmada metin ≤ 1 s {S3(lambda d: str(sum(1 for x in d['hs']['texts_narrow'] if x is not None and x<=1000))+'/'+str(len(d['hs']['texts_narrow'])))}.")
P(f"- **Hızlanma: {hz[1]} / {hz[2]} / {hz[3]}.** Önkoşul (58 s'de tavan) {S3(lambda d: n(d['hz']['pre58']))} B/s. Tavanın ≥ 11 875 olduğu an: {S3(lambda d: n(d['hz']['t_up'],2) if d['hz']['t_up'] and d['hz']['t_up']<84 else 'olmadı')} s; kararlı pencerede sunulan / kapasite %{S3(lambda d: n(d['hz']['w']['sunulan_yuzde'],1) if d['hz']['w'] else '–')}.")
if C:
    P(f"- **Kısa daralma sonrası [86,5, 98) (kesmesiz kolla karşılaştırma): {kd[1]} / {kd[2]} / {kd[3]}.** Bekleme medyanı taban {S3(lambda d: n(d['hs']['w_short']['bekleme_medyan']))} ms; kesmesiz {' / '.join(n(C[t]['hs']['w_short']['bekleme_medyan']) if t in C else '–' for t in (1,2,3))} ms.")
else:
    P("- **Kısa daralma sonrası [86,5, 98): SINANMADI** (kesmesiz karşılaştırma kolu yok).")
P("@@OZET@@")
P("")
P("**Kimlik**")
P("- Ürün kodu: `origin/yerel/poc-entegrasyon` @ `73715edbede79bd17b5cc11408947b65f6b7b1fd` (aday `1a4f9efc` + #251). #250 yok. İkili: `f0_flows` release, `--locked`, sha256 `d9c74ce941b209b2d387ef82c613855e4d4cb79d57acdaac8f55b9dd25c36802`; başlangıç kaydında `çekirdek_git=73715edbede79bd17b5cc11408947b65f6b7b1fd`.")
P("- Komut (tohum başına bir kez; tek kilit altında): `flock <kilit> env V1_SEED=<1|2|3> HDIR=<scratch düzenek> BIN_F0A_ADAY=<ikili> bash scripts/ktp2/f0/f0_cell.sh v1_taban.cell <kök>`")
P("- bob: `env KAMA_TRANSPORT_ENGINE=cc-port KAMA_ACK_FORMAT=bitmap KAMA_LINK_RATES=62,12500 KAMA_PILOT_RECORD=1 f0_flows --role bob <ortak>`")
P("- alice: `env KAMA_TRANSPORT_ENGINE=cc-port KAMA_ACK_FORMAT=bitmap KAMA_LINK_RATES=12500,62 KAMA_PILOT_RECORD=1 f0_flows --role alice <ortak>`")
P("- ortak: `--scenario dosyali --file-bytes 1200000 --file-at-ms 0 --warmup-texts 0 --texts 49 --text-at-ms 2000 --text-every-ms 2000 --text-bytes 64 --sigs 0 --ptt-at-ms none --karsi off --window-s 2 --receipt-grace-s 30 --connect-timeout-s 30 --probe-ms 100 --live-diag on`")
P("- Düzenek: `f0_cell.sh` değişmedi (`14cffdad…`). `kapi3_harness.sh` scratch kopya = depo dosyası + 53 satır (iki kademeli ileri hat, kuyruk örnekleyicisi, basamak uygulayıcısı, kayıt kopyacısı); `pkt_capture.py` + 3 satır. Araç kanıtı: 7096775628. sudo yok.")
P("")
P("**Kip ve kip kanıtı**")
P("- Motor cc-port, ACK bitmap, PTT yok, yön hızı profili aynalı (12 500 / 62).")
P("- Başlangıç kaydı (üç tohumda aynı): alice `motor=cc-port`, `ack_biçimi=bitmap etkin=evet`, `yön_hızları=yukarı 12500 B/s, aşağı 62 B/s`, `koruma=geri-bildirim-aç`; bob `motor=cc-port`, `ack_biçimi=bitmap etkin=hayır (geri bildirim aç profil yok: telde v2)`, `yukarı 62 / aşağı 12500`. Bob satırı #251'in 7096536885'te yazılan kayıt kusurudur: bu baş kaydı yalnız gönderici yönünden hesaplıyor.")
P(f"- Tel: tür baytı mühürlü, telde okunamaz; bu başta ACK tür sayacı da çıktıda yok (eksik kanca 1). **Dolaylı tel kanıtı:** Alice'in tavanı, ACK/denetim akışından bir datagram geldikten ≤ 160 ms sonra {S3(lambda d: str(len(d['trig'])))} kez değişti; bu datagramların {S3(lambda d: str(d['non14'])+'/'+str(len(d['trig'])))}'inin IP boyu 0x14 ile bağdaşmıyor (0x14 = 65 + 4k B; görülen boylar ör. 70, 72, 75, 76, 78, 80, 86). Tavanı yalnız ACK günceller; 0x15/0x16 boyu 67 + bitmap. 0x15 ile 0x16 boydan ayrılamaz. 0x14 boyuyla bağdaşan datagramların türü kanıtlanamıyor.")
P("- Bu kanıt tür sayacı değildir. Koordinatör tür sayacını şart koşarsa aşağıdaki hükümler \"sınanmadı\" okunur ve koşu, sayaç kancası olan başta yinelenir.")
P("")
P("**Hat (orta ad alanı; `tc -d qdisc show`, tohum 1, koşu başında, aynen)**")
P("```")
P(open(f"{V}/1/v1_taban_t1/aday/rep1/tc-d-qdisc-show.txt").read().rstrip().replace("\t", "  "))
P("```")
P("- İleri: `ma0` girişi → `ifb0` [tbf 1: + pfifo 10: 64 paket] (darboğaz, IP boyu) → `mb0` çıkışı netem 20: (300 ms + %20, `seed` = tohum). **Kayıp darboğazdan SONRA** (araç kanıtında ölçüldü). #233'teki tek netem dumanında kayıp darboğazdan önceydi; o iz bununla eşdeğer değil. Fikstürle bayt bayt karşılaştırma yapılmadı: kayıp deseni sıralı çekirdek RNG'si.")
P("- Dönüş: `ma0` çıkışında tek netem, 62 B/s, 300 ms, kayıpsız.")
P("- Hız planı (alice `sync/t0` saatine bağlı): 12 500 → 20 s 6 250 → 60 s 12 500 → 85,0 s 6 250 → 85,5 s 12 500 → 100 s bitiş. Basamak: `tc qdisc change dev ifb0 root handle 1: stab overhead -14 linklayer ethernet tbf rate <50kbit|100kbit> burst 1200 limit 64`.")
P("")
P("**Hız basamakları ve kuyruk sürekliliği (ürün koşularında; aynı kuyruk `tbf 1:` / `pfifo 10:`)**")
P("")
P("| tohum | basamak | uygulandığı an τ (komut öncesi → sonrası, s) | hedeften sapma | ÖNCE (8 ms önce): hız / backlog / drops | SONRA (≈ 3 ms sonra): hız / backlog / drops | arada varan / çıkan |")
P("|---|---|---|---|---|---|---|")
for t in (1, 2, 3):
    for b in T[t]["bas"]:
        P(f"| {t} | {n(b['ofset'],1)} s → {n(b['hiz'])} | {n(b['t_once'],6)} → {n(b['t_sonra'],6)} | +{n(b['sapma_ms'],3)} ms | {n(b['once']['hiz'])} / {b['once']['q']} p, {n(b['once']['b'])} B / {b['once']['drops']} | {n(b['sonra']['hiz'])} / {b['sonra']['q']} p, {n(b['sonra']['b'])} B / {b['sonra']['drops']} | {b['varan']} / {b['cikan']} |")
P("")
allb = [b for t in T for b in T[t]["bas"]]
P(f"- {len(allb)} basamağın hepsinde: rc = 0; handle aynı; drops farkı 0; backlog farkı = arada varan − çıkan. Kuyruk silinmedi, yeniden başlamadı. **Koşular teşhis etiketli değil.**")
P("- Örnekleyici her koşuda `tc -s -j qdisc show`, 50 ms; her örnekte aynı qdisc kümesi. Kova yan etkisi: her basamakta en çok 1 200 B hız sınırından erken çıkar (araç kanıtı).")
P("- Hız telde: kuyruk doluyken ardışık çıkış aralığı 1 074 B için 171,7–172,0 ms (6 250 B/s) ve 85,8–86,0 ms (12 500 B/s). Tohum 2'de 85,0–85,5 s basamağı uygulandı (qdisc kaydı) ama telde iz bırakmadı: o 0,5 s'de sunulan 2 148 B, kapasite 3 125 B.")
P("")
P("**Kirli koşu denetimi (ölçüt koşudan önce yazıldı, K1–K11):** üç koşu temiz.")
P(f"- Yakalama kaybı 0 (beş düzlem); kol başı sayaçlar 0; darboğazdan UDP dışı paket geçmedi; iki süreç ayrı ağ ad alanında; `listener_drops` = 0, `jsonl_errors` = 0; basamak sapması ≤ 0,013 ms; örnek aralığı en çok {S3(lambda d: n(d['gap_max'],1))} ms.")
P(f"- Konak yükü (1 dk) en çok {S3(lambda d: n(d['load_max'],2))} (eşik 6,0). **Tohum 3:** τ ≈ 60–100 s'de yük 1,8 → 5,66; kaynak bu görevin dışında, koşu başında ve sonunda cargo/rustc yok. Eşiğin altında; ama sahibin \"ölçümle ağır iş üst üste binmez\" kuralına yakın. Ölçüm araçlarında iz yok (yakalama kaybı 0, basamak sapması 0,005–0,009 ms).")
P("")
P("**Ölçüm (tohum 1 / 2 / 3)**")
P("")
P("| ölçü | tohum 1 | tohum 2 | tohum 3 |")
P("|---|---|---|---|")
def row(name, f): P(f"| {name} | " + " | ".join(f(T[t]) for t in (1, 2, 3)) + " |")
row("darboğaza sunulan (A çıkışı, hat öncesi) paket / IP B", lambda d: f"{n(d['a_out'])} / {n(d['a_out_b'])}")
row("darboğazdan çıkan paket", lambda d: n(d["ifb"]))
row("KUYRUKTA düşen (yakalama; `tbf` drops)", lambda d: f"{d['kd']}; {d['tbf_drops']}")
row("HATTA düşen, rastgele kayıp (yakalama; `netem` drops)", lambda d: f"{d['hd']}; {d['netem_drops']}")
row("gerçekleşen kayıp (hatta düşen / darboğazdan çıkan)", lambda d: f"%{n(d['loss_pct'],2)}")
row("kuyruk en çok (paket / B, τ)", lambda d: f"{d['qmax']['p']} / {n(d['qmax']['b'])}, {n(d['qmax']['t'],2)} s")
row("darboğaz beklemesi en çok (çıkış τ)", lambda d: f"{n(d['sj_max']['ms'])} ms, {n(d['sj_max']['t'],2)} s")
row("20 s daralması: ilk ACK adayı / 2. ACK adayı τ", lambda d: f"{n(d['cc']['a1'],2)} / {n(d['cc']['a2'],2)} s")
row("tavanın 5 625–6 875'e girdiği an (basamaktan sonra)", lambda d: f"{n(d['cc']['first_in'],2)} s (+{n(d['cc']['first_in']-20,2)} s)")
row("daralma boyunca tavan (2. ACK → 60 s) en az–en çok", lambda d: "–".join(n(x) for x in daralma_tavan(d)))
row("bekleme tepe → ≤ 393 ms'ye ilk iniş (basamaktan sonra)", lambda d: f"{n(d['hs']['pk'][1])} ms → {n(d['hs']['first_ok']-20,2)} s")
row("[20, 60) içinde > 393 ms beklemenin son görüldüğü an", lambda d: f"{n(d['hs']['last_bad'],2)} s")
row("gönderimin en uzun durduğu aralık (veri akışı)", lambda d: f"{n(d['pause'][0]*1000)} ms ({n(d['pause'][1],2)} → {n(d['pause'][2],2)} s)")
row("[35, 58) sunulan / çıkan (kapasiteye oran)", lambda d: f"%{n(d['hs']['w']['sunulan_yuzde'],1)} / %{n(d['hs']['w']['cikan_yuzde'],1)}")
row("[35, 58) bekleme medyan / p95 / en çok", lambda d: f"{n(d['hs']['w']['bekleme_medyan'])} / {n(d['hs']['w']['bekleme_p95'])} / {n(d['hs']['w']['bekleme_enc'])} ms")
row("daralmada (20–60 s) metin teslim: ≤ 1 s / en çok", lambda d: f"{sum(1 for x in d['hs']['texts_narrow'] if x is not None and x<=1000)}/{len(d['hs']['texts_narrow'])} / {n(max(x for x in d['hs']['texts_narrow'] if x is not None))} ms")
row("58 s'de tavan", lambda d: n(d["hz"]["pre58"]))
row("60 s'den sonra 1. / 3. ACK adayı τ", lambda d: f"{n(d['hz']['b1'],2)} / {n(d['hz']['b3'],2)} s")
row("tavan ≥ 11 875 anı; 84 s'ye dek kalıcı mı", lambda d: (f"{n(d['hz']['t_up'],2)} s; evet" if (d['hz']['t_up'] and d['hz']['t_up'] < 84 and d['hz']['kalici']) else f"olmadı (60 s sonrası en çok {n(d['hz']['tavan_max_after60'])})"))
row("[60, 85) sunulan / kapasite", lambda d: f"%{n(d['hz']['w6085']['sunulan_yuzde'],1)}")
row("kararlı pencere [tavan anı + 1 s, 84) sunulan / kapasite", lambda d: f"%{n(d['hz']['w']['sunulan_yuzde'],1)}" if d['hz']['w'] else "pencere yok")
row("[86,5, 98) sunulan / kapasite; bekleme medyan / en çok", lambda d: f"%{n(d['hs']['w_short']['sunulan_yuzde'],1)}; {n(d['hs']['w_short']['bekleme_medyan'])} / {n(d['hs']['w_short']['bekleme_enc'])} ms")
row("[70, 84) (kesme öncesi) bekleme medyan / en çok", lambda d: f"{n(d['hs']['w_ref']['bekleme_medyan'])} / {n(d['hs']['w_ref']['bekleme_enc'])} ms")
if C:
    P("| kesmesiz kol [86,5, 98): sunulan / kapasite; bekleme medyan / en çok | " + " | ".join((f"%{n(C[t]['hs']['w_short']['sunulan_yuzde'],1)}; {n(C[t]['hs']['w_short']['bekleme_medyan'])} / {n(C[t]['hs']['w_short']['bekleme_enc'])} ms" if t in C else "–") for t in (1, 2, 3)) + " |")
    P("| kesmesiz kol: 84 s'de tavan; kirli denetimi | " + " | ".join((f"{n(C[t]['hz']['tavan_84'])}; {'temiz' if C[t]['clean'] else 'KİRLİ'}" if t in C else "–") for t in (1, 2, 3)) + " |")
row("metin: teslim / gönderilen; medyan / en çok; ≤ 1 s", lambda d: f"{d['texts_del']}/{d['texts_sent']}; {n(d['text']['med'])} / {n(d['text']['mx'])} ms; {d['text']['le1']}/{d['texts_sent']}")
row("alındı dönüş bacağı: medyan / en çok; ≤ 4 s", lambda d: f"{n(d['rc_']['ret_med'])} / {n(d['rc_']['ret_max'])} ms; {d['rc_']['ret_le4']}/{d['texts_sent']}")
row("alındı gidiş-dönüş (hükme girmez): medyan / en çok; > 4 s", lambda d: f"{n(d['rc_']['rt_med'])} / {n(d['rc_']['rt_max'])} ms; {d['rc_']['rt_gt4']}")
row("dosya, Bob motor kabulü: 20 / 60 / 85 s / bitiş (kB; 1 200 kB'lik dosya)", lambda d: f"{n(d['file']['t20']/1000)} / {n(d['file']['t60']/1000)} / {n(d['file']['t85']/1000)} / {n(d['file']['end']/1000)}")
row("ACK/denetim akışı A'ya varan datagram / B; ACK adayı (≥ 65 B)", lambda d: f"{d['ack_flow']} / {n(d['ack_flow_b'])}; {d['ack_cand']}")
row("alice son sonda: tel gövde B / yeniden iletim B / parite / ileri kopya", lambda d: f"{n(d['alice_last']['txb'])} / {n(d['alice_last']['rtxb'])} / {d['alice_last']['par']} / {d['alice_last']['dup']}")
P("")
P("**Evre özeti (sunulan % · çıkan % · kuyruk en çok p · bekleme medyan / en çok ms)**")
P("")
names = ["[0, 20) 12 500", "[20, 35) 6 250 geçiş", "[35, 58) 6 250 kararlı", "[58, 60)", "[60, 64) 12 500 geçiş", "[64, 84) 12 500", "[84, 86,5) kısa daralma", "[86,5, 98)"]
P("| evre | tohum 1 | tohum 2 | tohum 3 |"); P("|---|---|---|---|")
for i, nm in enumerate(names):
    P(f"| {nm} | " + " | ".join(f"%{n(T[t]['phases'][i]['sunulan_yuzde'],0)} · %{n(T[t]['phases'][i]['cikan_yuzde'],0)} · {T[t]['phases'][i]['kuyruk_enc_p']} · {n(T[t]['phases'][i]['bekleme_medyan'])} / {n(T[t]['phases'][i]['bekleme_enc'])}" for t in (1, 2, 3)) + " |")
P("")
P("@@BULGU@@")
P("")
P("<details><summary>Ham iz: 1 s zaman çizelgesi (üç tohum) ve tavan / ACK olayları</summary>")
P("")
P("Sütunlar: s · kapasite B · darboğaza sunulan B (hat öncesi, A çıkışı, IP boyu) · darboğazdan çıkan B · kuyruk paket son/en çok (50 ms `tc -s` örnekleri) · darboğaz beklemesi ms medyan/en çok (paket başına: `ifb0` çıkışı − `ma0` girişi) · kuyrukta düşen · hatta düşen · ACK/denetim varışı τ:IP B (A girişi) · tavan B/s hat-eşdeğeri en az–en çok (alice sondası `pr` × 12500 / 12214, 100 ms) · metin teslim ms · alındı dönüş / gidiş-dönüş ms · dosya kB (Bob motor kabulü).")
for t in (1, 2, 3):
    P(""); P(f"Tohum {t}:"); P("```"); P(open(f"{V}/{t}/analiz-siki.txt").read().rstrip()); P("```")
    P(f"Tohum {t} tavan değişimleri (τ: B/s hat-eşdeğeri): " + "; ".join(f"{n(x[0],2)}: {n(x[1]*12500/12214)} → {n(x[2]*12500/12214)}" for x in T[t]["chg"]))
P("")
P("</details>")
P("")
P("@@SON@@")
print("\n".join(o))
