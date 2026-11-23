#!/usr/bin/env python3
"""[PoC SON KABUL · Bulut-B] `netem` transport senaryosu v1 (#175 7096381261) koşu değerlendiricisi.

Tek komut: bir `f0_cell.sh` koşu dizininden (`<kök>/<hücre>/<kol>/rep<N>/`) beş ölçütü çıkarır;
her ölçüt için GEÇTİ / KALDI / SINANMADI ve nedeni. Ürün kodu değildir; koşu açmaz; yalnız stdlib.

    python3 -I degerlendir.py RUN_DIR [--plan PLAN] [--t0 UNIX_S] [--baseline RUN_DIR] [--json]

Kural: kayıtta olmayan alan UYDURULMAZ. Bir ölçütün gerektirdiği alan yoksa hüküm SINANMADI ve
neden satırı eksik alanı adıyla yazar (`eksik:`). Vekil değerler yalnız `bilgi:` satırında durur,
hükme girmez.

Okunan dosyalar (koşu dizinine göre):
* `alice.jsonl`, `bob.jsonl` (`f0_flows`, şema `f0a-flows-v1`): `send`, `rx`, `rcpt`, `ts`; ayrıca
  `k == "lg"` satırları (bkz. EKSİK ALAN 1).
* `alice.json`, `bob.json`: `wire` (`ack_rx` tür sayaçları, 0x17), `ptt_presses`.
* Başlangıç kaydı: `node-<rol>/transport-start.txt` ya da `kayit/node-<rol>-transport-start.txt`
  (Yerel'in düzeneği): `motor=`, `ack_biçimi=<biçim> [etkin=…]`, varsa `ack_gönderici_etkin=`,
  `ack_alıcı_etkin=`.
* Zaman sıfırı ve hız çizelgesi (öncelik): `--t0` / `--plan` > `basamak.jsonl` (`k=t0`
  `t0_unix_ns`; her `k=basamak` satırının UYGULANAN anı `komut_once_unix_ns` ve `hiz`; ilk hız
  `kuyruk.jsonl` ilk örneğindeki darboğaz tbf `options.rate`) > `sync/t0` > `t_start`; çizelge yoksa
  senaryo v1.
* Darboğaz beklemesi (birincil): paket kimliği (`pid`) taşıyan yakalamalarda paket başına
  `MID_<darboğaz> out` − `MID_ma0 in` − kendi serileşmesi (ölçülen giriş ve çıkış). Yoksa kuyruk
  serisi: `qdisc_series.jsonl` ya da `kuyruk.jsonl` (`tc -s -j` örnekleri; darboğaz arayüzündeki
  tbf'nin `backlog`'u / o anki hız).
* Darboğaz arayüzü (`--bottleneck-dev`; varsayılan: `realized_post.json` `mid_qdisc`'te tbf/fifo
  kuyruğu olan arayüz, ör. Yerel'in düzeneğinde `ifb0`; yoksa `mb0`).
* `cap/MID_<darboğaz>.jsonl` `dir == "out"` = ÖLÇÜLEN darboğaz çıkışı (IP boyu, UDP/IP 28 B
  dahil; hız birimiyle aynı). Tel ve boş hat hesabının TEK kaynağı; yoksa ilgili alt ölçüt
  SINANMADI (başka düzlemden türetilmez: kayıp darboğazdan sonra uygulanıyorsa `mb0` çıkışı yolda
  düşenleri görmez).
* `realized_post.json`: `mid_qdisc` (kuyruk düşmesi darboğaz arayüzündeki tbf/fifo sayacından).
* `t_start` (unix s): zaman çizelgesinin varsayılan sıfırı (`--t0` ile değişir).
* İsteğe bağlı `qdisc_series.jsonl` (bkz. EKSİK ALAN 2).
* `cap/B_vb0.jsonl` `dir == "out"` ve `cap/A_va0.jsonl` `dir == "in"`: Bob'un bağlı portundan
  alice'in geçici portuna giden datagramlar (alice verisinin ACK'i ve denetim; `hdr` adresleri).
  Dönüş hattı kayıpsız ve FIFO olduğundan k. çıkış = k. varış.

GECİKME AYRIŞTIRMASI (sahip 7097044569; hüküm değil, sütun): kesme ACK'i = 20 s'den sonra tavanı
≤ 6 875'e indiren ilk `lg` satırı.
  1. düşüş → kesme ACK'inin Bob'dan çıkışı (tek parça; alıcının ilk geçerli kanıt anı yoksa iç
     bölünmez ve hiçbir nedene — ör. bütçe, yol verme — atfedilmez; EKSİK ALAN 5);
  2. Bob'dan çıkış → alice'e varış (dönüş hattı: serileşme + kuyruk + 300 ms) → `lg` işlenişi;
  3. kesme → darboğaz kuyruğunun ≤ 0,7 s olup 60 s'ye dek öyle kaldığı ilk an (`qdisc_series`).
EKSİK ALAN 5 — alıcının ilk geçerli kanıt anı (aralık/alarm), ACK başına.

EKSİK ALAN 1 — `lg` satırı (bugün `f0_flows` yazmıyor): göndericinin ACK başına hat koruması
kararı, `Sender::diag_line_guard_trace` (`LineGuardAckTrace`) kaynağından, alice.jsonl içinde:
    {"k":"lg","t":<unix ms, ACK'in işlendiği an>,"pace_before":<B/s>,"pace_after":<B/s>,
     "rate_after":<B/s|null>,"raised":<bool>,"probe_failed":<bool>}
CC (2. ACK) ve hızlanma (3. ACK) ölçütleri bu satırlarla hükme bağlanır. `ts.s[].pr` pacer
KOVASININ hızıdır (`TokenBucket::rate`), hat modeli tavanı (`set_line_rate`) değildir; vekil
olarak bile hükme girmez.
EKSİK ALAN 2 — `qdisc_series.jsonl`: darboğaz kuyruğunun dönemsel örneği
    {"t_unix_ns":<ns>,"dev":"mb0","backlog_bytes":<B>}
Darboğaz beklemesi ≤ 393 ms ölçütü bununla; yoksa SINANMADI.
EKSİK ALAN 3 — alındının Bob'dan ilk çıkış anı yok: "dönüş" süresi için üst sınır
`alice rcpt − bob rx` kullanılır (gerçek dönüş ≤ bu); üst sınır > 4 s ise SINANMADI.
PTT BANT (#233): `alice.json` `wire.ptt_wire_bytes` ≤ 625 B/s × basış süresi (5 000 bit/s;
1,5 s → 937 B, 8,4 s → 5 250 B). GEÇTİ ancak şu dördü birlikte doğrulanınca: canlı sayaç açık
(`wire.live_diag`), `wire.ptt_wire_datagrams` > 0, her basışta birleşik 300 ms (`ptt_presses[]`
`bundle_ms == 300`, `coalesced == true`), telde 0x17 (`wire.ack_rx.0x17` > 0). Biri eksikse bant
SINANMADI (GEÇTİ olamaz); dördü tamam ve bayt sınırı aşılırsa KALDI.
CC ve HIZLANMA: `lg` satırı YA DA ACK tür sayacı (`wire.ack_rx`, canlı sayaç açık) yoksa SINANMADI.
"""

import argparse
import json
import pathlib
import sys

# Senaryo v1 (#175 7096381261): (an s, ileri hız B/s). 100 s'de bitiş.
PLAN_V1 = [(0.0, 12500), (20.0, 6250), (60.0, 12500), (85.0, 6250), (85.5, 12500)]
END_V1 = 100.0
T_D, T_R = 20.0, 60.0
CC_LO, CC_HI = 5625, 6875
RAISED_MIN = 11875
RL_WIN = (35.0, 58.0)
SHORT_WIN = (86.5, 98.0)
UP_END = 84.0
WAIT_MAX_MS = 393.0
TEXT_MAX_MS = 1000.0
RCPT_MAX_MS = 4000.0
PTT_ON_TIME = 0.75
MTU_WIRE = 1228
READY_SHARE = 0.90  # pencerede hazır veri gösteren `ts` örneği payı (en az)

GECTI, KALDI, SINANMADI = "GEÇTİ", "KALDI", "SINANMADI"


def read_jsonl(path):
    out = []
    if not path.is_file():
        return out
    with path.open(encoding="utf-8", errors="replace") as f:
        for line in f:
            line = line.strip()
            if not line:
                continue
            try:
                out.append(json.loads(line))
            except json.JSONDecodeError:
                continue
    return out


def read_json(path):
    """Tek JSON nesnesi; dosyada art arda birden çok nesne varsa (`>>` ile eklenmiş) SONUNCUSU."""
    if not path.is_file():
        return None
    text = path.read_text(encoding="utf-8", errors="replace")
    dec, i, last = json.JSONDecoder(), 0, None
    while True:
        while i < len(text) and text[i].isspace():
            i += 1
        if i >= len(text):
            return last
        try:
            last, i = dec.raw_decode(text, i)
        except json.JSONDecodeError:
            return last


def read_kv(path):
    kv = {}
    if path.is_file():
        for line in path.read_text(encoding="utf-8", errors="replace").splitlines():
            if "=" in line:
                k, v = line.split("=", 1)
                kv.setdefault(k.strip(), v.strip())
    return kv


def parse_rate(text):
    """`100kbit` / `50kbit` / `496bit` → B/s."""
    m = str(text).strip().lower()
    for suf, mul in (("mbit", 1_000_000), ("kbit", 1_000), ("bit", 1)):
        if m.endswith(suf):
            return int(float(m[: -len(suf)]) * mul / 8)
    raise ValueError(text)


def parse_plan(text):
    plan = []
    for part in text.split(","):
        a, b = part.split(":")
        plan.append((float(a), int(b)))
    return sorted(plan)


def rate_at(plan, t):
    r = plan[0][1]
    for a, b in plan:
        if a <= t:
            r = b
    return r


def capacity(plan, lo, hi):
    """`[lo, hi)` içinde hattın taşıyabileceği bayt."""
    pts = sorted({lo, hi} | {a for a, _ in plan if lo < a < hi})
    return sum(rate_at(plan, a) * (b - a) for a, b in zip(pts, pts[1:]))


class Run:
    def __init__(self, d, t0=None, dev=None):
        self.d = d
        self.alice = read_jsonl(d / "alice.jsonl")
        self.bob = read_jsonl(d / "bob.jsonl")
        self.alice_sum = read_json(d / "alice.json") or {}
        self.bob_sum = read_json(d / "bob.json") or {}
        self.start = {r: read_kv(d / f"node-{r}" / "transport-start.txt")
                      or read_kv(d / "kayit" / f"node-{r}-transport-start.txt") for r in ("alice", "bob")}
        self.realized_post = read_json(d / "realized_post.json")
        self.dev = dev or bottleneck_dev(self.realized_post) or "mb0"
        cap = d / "cap" / f"MID_{self.dev}.jsonl"
        self.bottleneck_capture = cap.is_file()
        self.mb0 = [p for p in read_jsonl(cap) if p.get("dir") == "out" and "t_unix_ns" in p]
        self.qdisc = read_jsonl(d / "qdisc_series.jsonl")
        self.kuyruk = read_jsonl(d / "kuyruk.jsonl")
        if not self.qdisc and self.kuyruk:
            for smp in self.kuyruk:
                for q in smp.get("q") or []:
                    if q.get("dev") == self.dev and q.get("kind") == "tbf" and "t_unix_ns" in smp:
                        self.qdisc.append({"t_unix_ns": smp["t_unix_ns"], "dev": self.dev,
                                           "backlog_bytes": q.get("backlog", 0)})
        self.basamak = read_jsonl(d / "basamak.jsonl")
        self.t0_source = "--t0"
        if t0 is None:
            b0 = next((r for r in self.basamak if r.get("k") == "t0" and "t0_unix_ns" in r), None)
            st = d / "sync" / "t0"
            ts = d / "t_start"
            if b0:
                t0, self.t0_source = b0["t0_unix_ns"] / 1e9, "basamak.jsonl t0"
            elif st.is_file():
                t0, self.t0_source = int(st.read_text().strip()) / 1e9, "sync/t0"
            elif ts.is_file():
                t0, self.t0_source = float(ts.read_text().strip()), "t_start"
            else:
                t0 = None
        self.t0 = t0  # unix s

    def packet_waits(self):
        """[(darboğaza giriş anı s, bekleme ms (serileşme hariç), giriş→çıkış ms)] — `pid` eşleşmesiyle
        (darboğaza giriş `MID_ma0 in`, çıkış `MID_<darboğaz> out`); yoksa `None`."""
        ins = {}
        for p in read_jsonl(self.d / "cap" / "MID_ma0.jsonl"):
            if p.get("dir") == "in" and p.get("pid") and "t_unix_ns" in p:
                ins.setdefault(p["pid"], p["t_unix_ns"])
        if not ins or not self.mb0 or not all("pid" in p for p in self.mb0[:50]):
            return None
        out = []
        for p in self.mb0:
            t_in = ins.get(p.get("pid"))
            if t_in is None:
                continue
            t_out = self.rel_ns(p["t_unix_ns"])
            ser = p["ip_len"] / rate_at(self.plan, t_out)
            raw = t_out - self.rel_ns(t_in)
            out.append((self.rel_ns(t_in), max(0.0, raw - ser) * 1000.0, raw * 1000.0))
        return out or None

    def realized_plan(self):
        """`basamak.jsonl`'deki uygulanan basamaklar; ilk hız kuyruk örneğindeki tbf hızı."""
        steps = [r for r in self.basamak if r.get("k") == "basamak" and r.get("rc", 0) == 0]
        if not steps or self.t0 is None:
            return None
        first = None
        for smp in self.kuyruk[:1]:
            for q in smp.get("q") or []:
                if q.get("dev") == self.dev and q.get("kind") == "tbf":
                    first = (q.get("options") or {}).get("rate")
        if first is None:
            return None
        plan = [(0.0, int(first))]
        for r in steps:
            plan.append((r["komut_once_unix_ns"] / 1e9 - self.t0, parse_rate(r["hiz"])))
        return sorted(plan)

    def rel_ms(self, unix_ms):
        return unix_ms / 1000.0 - self.t0

    def rel_ns(self, unix_ns):
        return unix_ns / 1e9 - self.t0

    def lg(self):
        rows = [r for r in self.alice if r.get("k") == "lg" and "pace_after" in r and "t" in r]
        return sorted(((self.rel_ms(r["t"]), r) for r in rows), key=lambda x: x[0])

    def ts_ready(self):
        """`ts` örnekleri: (an, yeni veri kuyruğu > 0 mı). Bob'a giden oturum."""
        out = []
        for r in self.alice:
            if r.get("k") != "ts":
                continue
            ready = None
            for s in r.get("s", []):
                rec = s.get("rec")
                if isinstance(rec, dict) and "txq" in rec:
                    ready = (ready or False) or rec["txq"] > 0
            if ready is not None:
                out.append((self.rel_ms(r["t"]), ready))
        return out

    def wire_in(self, lo, hi):
        return sum(p["ip_len"] for p in self.mb0 if lo <= self.rel_ns(p["t_unix_ns"]) < hi)


def ready_share(run, lo, hi):
    s = [ok for t, ok in run.ts_ready() if lo <= t < hi]
    return (sum(s) / len(s), len(s)) if s else (None, 0)


def combine(parts):
    """Alt ölçütlerden hüküm: biri KALDI → KALDI; biri SINANMADI → SINANMADI; hepsi GEÇTİ → GEÇTİ."""
    vs = [v for v, _ in parts]
    if KALDI in vs:
        return KALDI
    if SINANMADI in vs or not vs:
        return SINANMADI
    return GECTI


def mode_evidence(run):
    a, b = run.start["alice"], run.start["bob"]
    notes, ok = [], True
    for who, kv in (("alice", a), ("bob", b)):
        if not kv:
            return SINANMADI, [f"eksik: node-{who}/transport-start.txt"]
        fmt = (kv.get("ack_biçimi") or "").split()
        notes.append(f"{who}: motor={kv.get('motor')} ack_biçimi={kv.get('ack_biçimi')} "
                     f"gönderici={kv.get('ack_gönderici_etkin')} alıcı={kv.get('ack_alıcı_etkin')}")
        ok &= kv.get("motor") == "cc-port" and bool(fmt) and fmt[0] == "bitmap"
    w = run.alice_sum.get("wire")
    rx = (w.get("ack_rx") if isinstance(w, dict) else None) or {}
    # Bitmap kanıtı: 0x15 + 0x16 + 0x17 (pilot açıkken alıcı yalın bitmap ACK'lerini 0x17 yazar;
    # Bulut-D 7097599958 madde 2).
    bitmap_acks = sum(int(rx.get(k) or 0) for k in ("0x15", "0x16", "0x17"))
    notes.append(f"alice wire.live_diag={w.get('live_diag') if isinstance(w, dict) else None} ack_rx={rx}")
    if not ok:
        return KALDI, notes + ["kip kabul kipi değil (başlangıç kaydı cc-port + bitmap değil)"]
    if not isinstance(w, dict):
        return SINANMADI, notes + ["eksik: koşu sonu tel kanıtı (alice.json wire yok ya da null)"]
    if w.get("live_diag") is not True:
        return SINANMADI, notes + ["canlı sayaç kapalı ya da bilinmiyor (wire.live_diag ≠ true): sayaç ölçü değil"]
    if not rx:
        return SINANMADI, notes + ["eksik: ACK tür sayacı (alice.json wire.ack_rx)"]
    if bitmap_acks == 0:
        return KALDI, notes + ["canlı sayaç açık ama telde bitmap ACK yok (0x15 + 0x16 + 0x17 = 0)"]
    return GECTI, notes


def ack_type_missing(run):
    """ACK tür sayacı yoksa (ya da canlı sayaç kapalıysa) neden; varsa `None`."""
    w = run.alice_sum.get("wire") or {}
    if not w.get("ack_rx"):
        return "eksik: ACK tür sayacı (alice.json wire.ack_rx)"
    if w.get("live_diag") is not True:
        return "ACK tür sayacı ölçü değil: canlı sayaç kapalı ya da bilinmiyor (wire.live_diag ≠ true)"
    return None


def crit_cc(run, plan):
    lg = run.lg()
    if not lg:
        return SINANMADI, ["eksik: lg (ACK başına LineGuard tavanı; EKSİK ALAN 1)"]
    miss = ack_type_missing(run)
    if miss:
        return SINANMADI, [miss]
    parts, notes = [], []
    share, n = ready_share(run, T_D - 1.0, T_D)
    util = run.wire_in(T_D - 1.0, T_D) / max(1, capacity(plan, T_D - 1.0, T_D))
    if not run.bottleneck_capture:
        return SINANMADI, [f"önkoşul ölçülemedi: eksik: cap/MID_{run.dev}.jsonl (ölçülen darboğaz çıkışı)"]
    notes.append(f"önkoşul: düşüşten önceki 1 s'de tel {util:.1%}, hazır veri örneği {share} (n={n})")
    if share is None or share < READY_SHARE or util < 0.90:
        return SINANMADI, notes + ["düşüş anında hat doymuş ve veri hazır değil"]
    after = [(t, r) for t, r in lg if t >= T_D]
    if len(after) < 2:
        return SINANMADI, notes + [f"düşüşten sonra {len(after)} ACK (2 gerekli)"]
    t2, r2 = after[1]
    p2 = r2["pace_after"]
    v = GECTI if CC_LO <= p2 <= CC_HI else KALDI
    parts.append((v, None))
    notes.append(f"2. ACK: +{t2 - T_D:.2f} s, tavan {p2} (bant {CC_LO}–{CC_HI}) → {v}")
    first_ok = next((t for t, r in after if CC_LO <= r["pace_after"] <= CC_HI), None)
    notes.append("ilk bant içi ACK: " + (f"+{first_ok - T_D:.2f} s" if first_ok is not None else "yok"))
    false_cut = [(t, r["pace_after"]) for t, r in lg if t < T_D and r["pace_after"] <= CC_HI]
    fv = KALDI if false_cut else GECTI
    parts.append((fv, None))
    notes.append(f"20 s'den önce sahte kesme (tavan ≤ {CC_HI}): {false_cut[:3] or 'yok'} → {fv}")
    return combine(parts), notes


def gaps_idle_ms(run, lo, hi):
    """`[lo, hi)` içinde veri hazırken (boşluğun başındaki en yakın `ts` örneği hazır) darboğazın boş
    kaldığı süre (ms): ölçülen darboğaz çıkışında ardışık iki çıkış arası − ikincinin serileşmesi."""
    pkts = sorted((run.rel_ns(p["t_unix_ns"]), p["ip_len"]) for p in run.mb0)
    pts = [(t, n) for t, n in pkts if lo <= t < hi]
    ready = run.ts_ready()
    idle = 0.0
    for (a, _), (b, n) in zip(pts, pts[1:]):
        prior = [ok for t, ok in ready if t <= a]
        if prior and not prior[-1]:
            continue
        idle += max(0.0, (b - a) - n / rate_at(run.plan, b))
    return idle * 1000.0


def crit_rate_limit(run, plan, baseline):
    parts, notes = [], []
    lo, hi = RL_WIN
    cap = capacity(plan, lo, hi)
    wire = run.wire_in(lo, hi)
    share, n = ready_share(run, lo, hi)
    util = wire / cap if cap else 0.0
    notes.append(f"[{lo:.0f}, {hi:.0f}) s: tel {wire} B / {cap:.0f} B = {util:.1%}; hazır veri örneği "
                 f"{share if share is None else f'{share:.0%}'} (n={n})")
    if not run.mb0:
        parts.append((SINANMADI, None)); notes.append(f"eksik: cap/MID_{run.dev}.jsonl")
    elif share is None or share < READY_SHARE:
        parts.append((SINANMADI, None)); notes.append("pencerede veri hazır değil")
    else:
        v = GECTI if 0.90 <= util <= 1.0 + MTU_WIRE / cap else KALDI
        parts.append((v, None)); notes.append(f"tel %90–100 → {v}")
    # Darboğaz beklemesi: paket başına (pid) ölçülen; yoksa kuyruk serisi.
    series = [q for q in run.qdisc if q.get("dev") == run.dev and "backlog_bytes" in q]
    pw = run.packet_waits()
    if pw:
        w = sorted(x for t, x, _ in pw if lo <= t < hi)
        raw = max((r for t, _, r in pw if lo <= t < hi), default=None)
        mx = w[-1] if w else None
        v = SINANMADI if mx is None else (GECTI if mx <= WAIT_MAX_MS else KALDI)
        parts.append((v, None))
        notes.append(f"darboğaz beklemesi (paket başına pid eşleşmesi, pencerede darboğaza giren {len(w)} "
                     f"paket; serileşme hariç) en çok {mx:.1f} ms, medyan {w[len(w) // 2]:.0f} ms "
                     f"(≤ {WAIT_MAX_MS:.0f}) → {v}; bilgi: giriş→çıkış (serileşme dahil) en çok {raw:.0f} ms"
                     if w else "darboğaz beklemesi: pencerede paket yok")
        if series:
            ws = [q["backlog_bytes"] / rate_at(plan, run.rel_ns(q["t_unix_ns"])) * 1000.0
                  for q in series if lo <= run.rel_ns(q["t_unix_ns"]) < hi]
            notes.append(f"bilgi: kuyruk örneklerinden (backlog/hız) en çok {max(ws):.0f} ms" if ws else
                         "bilgi: kuyruk örneği yok")
    elif not series:
        parts.append((SINANMADI, None))
        notes.append("eksik: qdisc_series.jsonl (darboğaz beklemesi ≤ 393 ms; EKSİK ALAN 2)")
    else:
        w = [q["backlog_bytes"] / rate_at(plan, run.rel_ns(q["t_unix_ns"])) * 1000.0
             for q in series if lo <= run.rel_ns(q["t_unix_ns"]) < hi]
        mx = max(w) if w else None
        v = SINANMADI if mx is None else (GECTI if mx <= WAIT_MAX_MS else KALDI)
        parts.append((v, None)); notes.append(f"darboğaz beklemesi en çok {mx if mx is None else round(mx, 1)} ms (≤ {WAIT_MAX_MS}) → {v}")
    # Kuyrukta kayıp.
    qd = queue_drops(run.realized_post, run.dev)
    if qd is None:
        parts.append((SINANMADI, None))
        notes.append(f"kuyrukta kayıp ayrılamıyor: {run.dev}'da ayrı FIFO/TBF kuyruğu yok (netem kaybıyla tek sayaç)")
    else:
        v = GECTI if qd == 0 else KALDI
        parts.append((v, None)); notes.append(f"kuyrukta düşen {qd} (koşu boyu) → {v}")
    # Daralmada metin.
    lat = text_latencies(run)
    dar = [(i, l) for i, t, l in lat if T_D <= t < T_R]
    if not dar:
        parts.append((SINANMADI, None)); notes.append("daralmada metin yok")
    else:
        worst = max(dar, key=lambda x: x[1] if x[1] is not None else 1e18)
        bad = [x for x in dar if x[1] is None or x[1] > TEXT_MAX_MS]
        v = KALDI if bad else GECTI
        parts.append((v, None))
        notes.append(f"daralmada metin {len(dar)} adet, en kötü {worst[1]} ms (≤ 1 000) → {v}")
    # Kısa daralma sonrası boş bekleme.
    slo, shi = SHORT_WIN
    idle = gaps_idle_ms(run, slo, shi)
    sshare, _ = ready_share(run, slo, shi)
    if (not run.bottleneck_capture) or (baseline is not None and (not baseline.bottleneck_capture)):
        notes.append(f"eksik: cap/MID_{run.dev}.jsonl — boş hat süresi eksik yakalamadan ölçülemez")
        parts.append((SINANMADI, None))
    elif baseline is None:
        notes.append(f"bilgi: [{slo}, {shi}) s hat boş {idle:.0f} ms, hazır veri {sshare}")
        notes.append("eksik: --baseline (kesmesiz koşu) → kısa daralma alt ölçütü sınanmadı")
        parts.append((SINANMADI, None))
    else:
        bidle = gaps_idle_ms(baseline, slo, shi)
        v = GECTI if idle <= bidle + MTU_WIRE / 12.5 else KALDI
        parts.append((v, None))
        notes.append(f"[{slo}, {shi}) s hat boş {idle:.0f} ms, kesmesiz {bidle:.0f} ms → {v}")
    return combine(parts), notes


QUEUE_KINDS = ("tbf", "bfifo", "pfifo", "pfifo_fast")


def bottleneck_dev(realized):
    """tbf/fifo kuyruğu olan orta ad alanı arayüzü (darboğaz); `ifb*` öncelikli. Yoksa `None`."""
    if not isinstance(realized, dict):
        return None
    devs = sorted({q.get("dev") for q in realized.get("mid_qdisc") or []
                   if q.get("kind") in QUEUE_KINDS and q.get("dev")},
                  key=lambda d: (not d.startswith("ifb"), d))
    return devs[0] if devs else None


def queue_drops(realized, dev="mb0"):
    if not isinstance(realized, dict):
        return None
    total, seen = 0, False
    for q in realized.get("mid_qdisc") or []:
        if q.get("dev") == dev and q.get("kind") in QUEUE_KINDS:
            seen = True
            total += int(q.get("drops", 0))
    return total if seen else None


def crit_speedup(run, plan):
    lg = run.lg()
    if not lg:
        return SINANMADI, ["eksik: lg (ACK başına LineGuard tavanı; EKSİK ALAN 1)"]
    miss = ack_type_missing(run)
    if miss:
        return SINANMADI, [miss]
    parts, notes = [], []
    before = [r["pace_after"] for t, r in lg if t < T_R - 2.0]
    if not before or abs(before[-1] - 6250) > 625:
        return SINANMADI, [f"önkoşul: 58 s'de tavan ≈ 6 250 değil ({before[-1] if before else 'ACK yok'})"]
    share, n = ready_share(run, T_R - 1.0, T_R)
    if share is None or share < READY_SHARE:
        return SINANMADI, [f"önkoşul: toparlanmada veri hazır değil ({share}, n={n})"]
    after = [(t, r) for t, r in lg if t >= T_R]
    if len(after) < 3:
        return SINANMADI, [f"toparlanmadan sonra {len(after)} ACK (3 gerekli)"]
    k3 = next((k for k, (t, r) in enumerate(after[:3], 1) if r["pace_after"] >= RAISED_MIN), None)
    later = [(t, r["pace_after"]) for t, r in after[(k3 or 3):] if t < UP_END and r["pace_after"] < RAISED_MIN]
    v = GECTI if k3 is not None and not later else KALDI
    parts.append((v, None))
    notes.append(f"≥ {RAISED_MIN} ilk ACK: {k3 if k3 else '3 içinde yok'}; sonra altına inen: {later[:3] or 'yok'} → {v}")
    if k3 is not None:
        t_x = after[k3 - 1][0]
        lo, hi = t_x + 1.0, UP_END
        if hi - lo < 2.0:
            parts.append((SINANMADI, None)); notes.append("kararlı pencere < 2 s")
        else:
            cap = capacity(plan, lo, hi)
            util = run.wire_in(lo, hi) / cap
            sh, n2 = ready_share(run, lo, hi)
            if sh is None or sh < READY_SHARE:
                parts.append((SINANMADI, None)); notes.append(f"[{lo:.1f}, {hi:.0f}) s veri hazır değil ({sh})")
            elif not run.bottleneck_capture:
                parts.append((SINANMADI, None))
                notes.append(f"[{lo:.1f}, {hi:.0f}) s tel: eksik: cap/MID_{run.dev}.jsonl (ölçülen darboğaz çıkışı)")
            else:
                v2 = GECTI if util >= 0.90 else KALDI
                parts.append((v2, None)); notes.append(f"[{lo:.1f}, {hi:.0f}) s tel {util:.1%} (≥ %90) → {v2}")
    return combine(parts), notes


PTT_BAND_BPS = 625  # 5 000 bit/s


def crit_ptt(run):
    presses = run.bob_sum.get("ptt_presses") or []
    if not presses:
        return SINANMADI, ["basış yok (bob.json ptt_presses boş)"]
    parts, notes = [], []
    for p in presses:
        planned, ontime = p.get("frames_planned", 0), p.get("frames_on_time", 0)
        share = ontime / planned if planned else None
        v1 = SINANMADI if share is None else (GECTI if share >= PTT_ON_TIME else KALDI)
        one = p.get("ptt_started") == 1 and p.get("ptt_ended") == 1
        v2 = GECTI if one else KALDI
        parts += [(v1, None), (v2, None)]
        notes.append(f"akış {p.get('stream')}: alıcıda demet {p.get('bundle_ms')} ms; zamanında "
                     f"{ontime}/{planned} → {v1}; açılış/kapanış {p.get('ptt_started')}/{p.get('ptt_ended')} "
                     f"→ {v2}; ilk ses {p.get('first_audio_ms')} ms; oluşum→teslim "
                     f"{p.get('creation_to_delivery_ms')}")
    # Bant: dört kapı doğrulanmadan GEÇTİ olamaz.
    w = run.alice_sum.get("wire") or {}
    chosen = run.alice_sum.get("ptt_presses") or []
    gates = [
        ("canlı sayaç açık (wire.live_diag)", w.get("live_diag") is True),
        ("PTT datagramı > 0 (wire.ptt_wire_datagrams)", (w.get("ptt_wire_datagrams") or 0) > 0),
        ("her basışta birleşik 300 ms (alice ptt_presses bundle_ms=300, coalesced=true)",
         bool(chosen) and all(c.get("bundle_ms") == 300 and c.get("coalesced") is True for c in chosen)),
        ("telde 0x17 (wire.ack_rx.0x17 > 0)", ((w.get("ack_rx") or {}).get("0x17") or 0) > 0),
    ]
    failed = [g for g, ok in gates if not ok]
    secs = sum((p.get("bundles_planned") or 0) * (p.get("bundle_ms") or 0) for p in presses) / 1000.0
    if failed:
        parts.append((SINANMADI, None))
        notes.append("bant SINANMADI (GEÇTİ olamaz); doğrulanmayan: " + "; ".join(failed))
    elif "ptt_wire_bytes" not in w or secs <= 0:
        parts.append((SINANMADI, None))
        notes.append("bant SINANMADI: eksik: wire.ptt_wire_bytes ya da basış süresi (bundles_planned × bundle_ms)")
    else:
        limit = PTT_BAND_BPS * secs
        v = GECTI if w["ptt_wire_bytes"] <= limit else KALDI
        parts.append((v, None))
        notes.append(f"bant: PTT tel {w['ptt_wire_bytes']} B / {w['ptt_wire_datagrams']} datagram, basış "
                     f"{secs:.1f} s, sınır {limit:.0f} B → {v}")
    return combine(parts), notes


def text_latencies(run):
    """(sıra, gönderim anı s, gecikme ms | None) — alice `send` → bob ilk `rx`; yalnız metin sınıfları."""
    sends = {}
    for r in run.alice:
        if r.get("k") == "send" and r.get("c") not in ("S", "O", "P", "C", "F"):
            sends.setdefault((r["c"], r["i"]), r["t"])
    rx = {}
    for r in run.bob:
        if r.get("k") == "rx" and (r["c"], r.get("i")) in sends and r.get("first", True):
            rx.setdefault((r["c"], r["i"]), r["t"])
    out = []
    for key, t in sorted(sends.items(), key=lambda x: x[1]):
        lat = rx[key] - t if key in rx else None
        out.append((key, round(run.rel_ms(t), 2), None if lat is None else round(lat, 1)))
    return out


def crit_text_receipt(run):
    parts, notes = [], []
    lat = text_latencies(run)
    if not lat:
        return SINANMADI, ["metin yok"]
    bad = [(k, round(t, 2), l) for k, t, l in lat if l is None or l > TEXT_MAX_MS]
    v = KALDI if bad else GECTI
    parts.append((v, None))
    worst = max((l for _, _, l in lat if l is not None), default=None)
    notes.append(f"metin {len(lat)} adet, en kötü {worst} ms, sınır üstü/teslimsiz {bad[:5]} → {v}")
    bob_rx = {(r["c"], r["i"]): r["t"] for r in run.bob if r.get("k") == "rx" and "i" in r}
    sends = {(r["c"], r["i"]): r["t"] for r in run.alice if r.get("k") == "send"}
    ups, rtt = [], []
    for r in run.alice:
        if r.get("k") == "rcpt" and r.get("first"):
            key = (r["c"], r["i"])
            if key in bob_rx:
                ups.append((key, r["t"] - bob_rx[key]))
            if key in sends:
                rtt.append(r["t"] - sends[key])
    if not ups:
        parts.append((SINANMADI, None)); notes.append("alındı yok")
    else:
        mx = max(u for _, u in ups)
        if mx <= RCPT_MAX_MS:
            parts.append((GECTI, None))
            notes.append(f"alındı dönüş üst sınırı (alice rcpt − bob rx) en çok {mx:.0f} ms ≤ 4 000 → {GECTI}")
        else:
            parts.append((SINANMADI, None))
            notes.append(f"alındı dönüş üst sınırı {mx:.0f} ms > 4 000; gerçek dönüş ayrılamıyor "
                         "(eksik: alındının Bob ilk çıkış anı; EKSİK ALAN 3)")
    if rtt:
        notes.append(f"bilgi (hükme girmez): gidiş-dönüş en çok {max(rtt):.0f} ms, medyan "
                     f"{sorted(rtt)[len(rtt) // 2]:.0f} ms")
    f = [r for r in run.bob if r.get("k") == "rx" and r.get("c") == "F"]
    notes.append(f"bilgi: dosya bob'da {'match=' + str(f[0].get('match')) if f else 'bitmedi/yok'}")
    return combine(parts), notes


QUEUE_OK_S = 0.7


def rev_acks(run, cap_name, direction):
    """Bob bağlı portu → alice geçici portu datagramlarının anları (s, t0'a göre)."""
    hdr = next((r for r in run.alice if r.get("k") == "hdr"), None)
    if not hdr:
        return None
    try:
        a_port = int(str(hdr["alice_addr"]).rsplit(":", 1)[1])
        b_port = int(str(hdr["bob_addr"]).rsplit(":", 1)[1])
    except (KeyError, ValueError, IndexError):
        return None
    pk = read_jsonl(run.d / "cap" / cap_name)
    return sorted(run.rel_ns(p["t_unix_ns"]) for p in pk
                  if p.get("dir") == direction and p.get("sport") == b_port
                  and p.get("dport") not in (None, a_port) and "t_unix_ns" in p)


def queue_series_s(run, plan):
    """(an s, darboğaz kuyruğu s) — ölçülen backlog / o anki hız."""
    return sorted((run.rel_ns(q["t_unix_ns"]), q["backlog_bytes"] / rate_at(plan, run.rel_ns(q["t_unix_ns"])))
                  for q in run.qdisc if q.get("dev") == run.dev and "backlog_bytes" in q)


# Bob'un ACK başına an kaydı (Yerel `yerel/f0-olcum-kancalari`; ad kesinleşince eşlenir).
BOB_ACK_KINDS = ("ack", "ackx", "ack_an")
BOB_KEYS = {"kanit": ("t_kanit", "kanit", "t_evidence", "evidence_t"),
            "hazir": ("t_hazir", "hazir", "t_ready", "ready_t"),
            "cikis": ("t_cikis", "cikis", "t_wire", "wire_t")}


def bob_ack_moments(run, t_out):
    """Telde çıkışı `t_out` olan ACK'in Bob kaydındaki anları (s, t0'a göre); kayıt yoksa boş."""
    rows = [r for r in run.bob if r.get("k") in BOB_ACK_KINDS]
    if not rows:
        return {}

    def get(r, name):
        for key in BOB_KEYS[name]:
            if r.get(key) is not None:
                return run.rel_ms(r[key])
        return None

    cand = []
    for r in rows:
        c = get(r, "cikis")
        h = get(r, "hazir")
        ref = c if c is not None else h
        if ref is None or (t_out is not None and ref > t_out + 0.02):
            continue
        cand.append((abs((t_out or ref) - ref), r))
    if not cand:
        return {}
    r = min(cand, key=lambda x: x[0])[1]
    return {n: get(r, n) for n in BOB_KEYS}



def ack_identity(run, lg_row):
    """Kesme ACK'inin Bob kaydı ve tel anları, kimlikle (index → largest_pn → pid). Yoksa ({}, None, None, None)."""
    rx = next((r for r in run.alice if r.get("k") == "ack_rx" and r.get("index") == lg_row.get("index")
               and r.get("sess") == lg_row.get("sess")), None)
    if rx is None or rx.get("largest_pn") is None:
        return {}, None, None, None
    rows = [r for r in run.bob if r.get("k") in BOB_ACK_KINDS and r.get("largest_pn") == rx["largest_pn"]]
    if len(rows) != 1:
        return {}, None, None, (f"kimlik eşlemesi: largest_pn {rx['largest_pn']} için Bob ACK kaydı "
                                f"{len(rows)} adet (1 beklenir); sırayla eşlemeye düşüldü")
    r = rows[0]

    def get(name):
        for key in BOB_KEYS[name]:
            if r.get(key) is not None:
                return run.rel_ms(r[key])
        return None
    bm = {n: get(n) for n in BOB_KEYS}
    t_out = t_arr = None
    hdr = next((h for h in run.alice if h.get("k") == "hdr"), None)
    try:
        b_port = int(str(hdr["bob_addr"]).rsplit(":", 1)[1])
    except (TypeError, KeyError, ValueError, IndexError):
        b_port = None
    pid_note = ""
    if b_port is not None and bm.get("cikis") is not None:
        outs = [p for p in read_jsonl(run.d / "cap" / "B_vb0.jsonl")
                if p.get("dir") == "out" and p.get("sport") == b_port and "t_unix_ns" in p]
        near = [p for p in outs if abs(run.rel_ns(p["t_unix_ns"]) - bm["cikis"]) <= 0.02]
        if len(near) == 1:
            t_out = run.rel_ns(near[0]["t_unix_ns"])
            ins = [p for p in read_jsonl(run.d / "cap" / "A_va0.jsonl")
                   if p.get("dir") == "in" and p.get("pid") == near[0].get("pid") and "t_unix_ns" in p]
            if len(ins) == 1:
                t_arr = run.rel_ns(ins[0]["t_unix_ns"])
            else:
                pid_note = f"; Alice varışı: pid {near[0].get('pid')} cap/A_va0 in'de {len(ins)} adet (1 beklenir)"
        else:
            pid_note = f"; telde çıkış: Bob t_wire ±20 ms içinde cap/B_vb0 out {len(near)} datagram (1 beklenir)"
    return bm, t_out, t_arr, (f"kimlik eşlemesi: lg index {lg_row.get('index')} → ack_rx largest_pn "
                              f"{rx['largest_pn']} → Bob ACK index {r.get('index')} ({r.get('type')}, "
                              f"{r.get('bytes')} B, ertelemeler {r.get('defer')}){pid_note}")

def delay_breakdown(run, plan):
    notes = []
    lg = run.lg()
    cut = next(((t, r) for t, r in lg if t >= T_D and r["pace_after"] <= CC_HI), None)
    if cut is None:
        notes.append("eksik: lg (kesme ACK'i bulunamadı); parça 1–3 kesmeye göre ayrılamadı"
                     if not lg else "20 s'den sonra kesme ACK'i yok")
        series = queue_series_s(run, plan)
        if series:
            win = [(t, q) for t, q in series if T_D <= t < T_R]
            drain = next((t for i, (t, q) in enumerate(win)
                          if all(qq <= QUEUE_OK_S for _, qq in win[i:])), None)
            peak = max(win, key=lambda x: x[1]) if win else None
            notes.append("bilgi (ölçülen kuyruk, kesme anından bağımsız): düşüş → kuyruk ≤ "
                         f"{QUEUE_OK_S} s (60 s'ye dek) "
                         + (f"+{drain - T_D:.2f} s" if drain is not None else "toparlanmaya dek yok")
                         + (f"; [20, 60) s kuyruk tepesi {peak[1]:.2f} s @ {peak[0]:.2f} s" if peak else ""))
        return notes
    t_cut = cut[0]
    notes.append(f"kesme ACK'i işlendi +{t_cut - T_D:.2f} s (an {t_cut:.2f} s, tavan {cut[1]['pace_after']})")
    # Kimlikle eşleme: lg.index → ack_rx.index → largest_pn → Bob ACK kaydı; Bob t_wire → cap/B_vb0 out
    # → pid → cap/A_va0 in. Sırayla eşleme (eski yol) yalnız kimlik yoksa.
    bm, t_out, t_arr, how = ack_identity(run, cut[1])
    if how:
        notes.append(how)
    if not bm:
        arr = rev_acks(run, "A_va0.jsonl", "in")
        out = rev_acks(run, "B_vb0.jsonl", "out")
        k = None
        if not arr or not out:
            notes.append("telde çıkış/varış: eksik: cap/A_va0.jsonl, cap/B_vb0.jsonl ya da hdr adresleri")
        else:
            k = max((i for i, t in enumerate(arr) if t <= t_cut + 0.05), default=None)
            if k is None or k >= len(out) or len(out) != len(arr):
                notes.append(f"telde çıkış/varış: dönüş datagramları eşleşmiyor (çıkış {len(out)}, varış {len(arr)})")
                k = None
        t_out = out[k] if k is not None else None
        t_arr = arr[k] if k is not None else None
        bm = bob_ack_moments(run, t_out) if t_out is not None else {}
    # Beş an (sahip 7097781875): Bob'da ilk geçerli kanıt → ACK hazır → telde çıkış → Alice varışı
    # → LineGuard kesmesi. Bob'daki iki an yalnız kayıttan (`bob_ack_moments`); yoksa EKSİK.
    moments = [("düşüş", T_D), ("Bob ilk geçerli kanıt", bm.get("kanit")), ("Bob ACK hazır", bm.get("hazir")),
               ("telde çıkış", t_out if t_out is not None else bm.get("cikis")), ("Alice varışı", t_arr),
               ("LineGuard kesmesi", t_cut)]
    notes.append("an (t0'a göre, s): " + "; ".join(
        f"{n} {'EKSİK' if t is None else f'{t:.3f}'}" for n, t in moments))
    for (n1, a), (n2, b) in zip(moments, moments[1:]):
        if a is None or b is None:
            notes.append(f"aralık {n1} → {n2}: ayrıştırılamadı (eksik an)")
        else:
            notes.append(f"aralık {n1} → {n2}: {b - a:.3f} s")
    if bm.get("kanit") is None:
        missing = [n for n, t in moments if t is None]
        # Kanıt anı yoksa düşüş → çıkış tek parça; nedene atfedilmez.
        if t_out is not None:
            notes.append(f"parça 1 düşüş → Bob'dan çıkış: {t_out - T_D:.2f} s (tek parça; iç bölünmez, "
                         "nedene atfedilmez — eksik: " + ", ".join(missing) + ")")
    if t_out is not None and t_arr is not None:
        notes.append(f"parça 2 Bob'dan çıkış → alice varışı: {t_arr - t_out:.2f} s; varış → işleniş "
                     f"{t_cut - t_arr:.2f} s")
    series = queue_series_s(run, plan)
    if not series:
        notes.append("parça 3: eksik: qdisc_series.jsonl")
        return notes
    q_cut = min(series, key=lambda x: abs(x[0] - t_cut))[1]
    win = [(t, q) for t, q in series if t_cut <= t < T_R]
    drain = next((t for i, (t, q) in enumerate(win) if all(qq <= QUEUE_OK_S for _, qq in win[i:])), None)
    notes.append(f"parça 3 kesme → kuyruk ≤ {QUEUE_OK_S} s (60 s'ye dek): "
                 + (f"{drain - t_cut:.2f} s" if drain is not None else "toparlanmaya dek yok")
                 + f"; kesmede kuyruk {q_cut:.2f} s (bilgi: fikstür sınırı kesme + kuyruk = "
                   f"+{t_cut + q_cut - T_D:.2f} s)")
    return notes


def evaluate(d, plan, t0=None, baseline_dir=None, dev=None):
    run = Run(d, t0, dev)
    if run.t0 is None:
        raise SystemExit("zaman sıfırı yok (basamak.jsonl / sync/t0 / t_start): --t0 verin")
    plan_source = "--plan / senaryo v1"
    if plan is None:
        rp = run.realized_plan()
        plan, plan_source = (rp, "basamak.jsonl (uygulanan)") if rp else (PLAN_V1, "senaryo v1")
    run.plan = plan
    baseline = Run(baseline_dir, None, dev) if baseline_dir else None
    if baseline is not None:
        baseline.plan = baseline.realized_plan() or [(0.0, plan[0][1])]
    mode_v, mode_n = mode_evidence(run)
    mode_n = [f"zaman sıfırı {run.t0_source}; çizelge {plan_source}: "
              + ", ".join(f"{a:.3f} s→{b}" for a, b in plan)] + mode_n
    mode_n = mode_n + [f"darboğaz arayüzü {run.dev}; tel kaynağı cap/MID_{run.dev}.jsonl "
                       + ("(var)" if run.bottleneck_capture else "(YOK)")]
    rows = [("kip (önkoşul)", mode_v, mode_n)]
    for name, fn in (("1 CC", lambda: crit_cc(run, plan)),
                     ("3 hız sınırlama", lambda: crit_rate_limit(run, plan, baseline)),
                     ("4 hızlanma", lambda: crit_speedup(run, plan)),
                     ("2 PTT", lambda: crit_ptt(run)),
                     ("5 metin/alındı", lambda: crit_text_receipt(run))):
        v, n = fn()
        # Kip önkoşulu BÜTÜN ölçütlere uygulanır (5. dahil; Bulut-D 7097599958 madde 1).
        if mode_v != GECTI and v != SINANMADI:
            n = n + [f"kip önkoşulu {mode_v}: hüküm SINANMADI'ya çekildi (ölçü {v})"]
            v = SINANMADI
        rows.append((name, v, n))
    rows.append(("gecikme ayrıştırması", "—", delay_breakdown(run, plan)))
    return run, rows


def field_check(d, dev=None):
    """Pahalı koşudan önce: hükmü bağlayan alanlar bu kayıtta okunabiliyor mu (değer değil, varlık)."""
    run = Run(d, None, dev)
    w = run.alice_sum.get("wire") if isinstance(run.alice_sum.get("wire"), dict) else None
    rx = (w or {}).get("ack_rx") or {}
    bob_ack = [r for r in run.bob if r.get("k") in BOB_ACK_KINDS]
    has = lambda name: any(any(r.get(k) is not None for k in BOB_KEYS[name]) for r in bob_ack)
    out_b = rev_acks(run, "B_vb0.jsonl", "out")
    in_a = rev_acks(run, "A_va0.jsonl", "in")
    chosen = run.alice_sum.get("ptt_presses") or []
    rows = [
        ("transport", "başlangıç kaydı (motor, ack_biçimi) iki uçta", bool(run.start["alice"] and run.start["bob"]),
         f"alice={run.start['alice'].get('ack_biçimi')} bob={run.start['bob'].get('ack_biçimi')}"),
        ("transport", "canlı ayna açık (alice.json wire.live_diag == true)", bool(w) and w.get("live_diag") is True,
         f"wire={'yok' if w is None else 'var'} live_diag={(w or {}).get('live_diag')}"),
        ("transport", "ACK tür sayacı (wire.ack_rx)", bool(rx), str(rx)),
        ("transport", "ACK başına LineGuard tavanı (alice.jsonl k=lg)", bool(run.lg()), f"{len(run.lg())} satır"),
        ("transport", "Bob'da ilk geçerli kanıt anı (bob.jsonl ACK kaydı)", has("kanit"), f"{len(bob_ack)} ACK kaydı"),
        ("transport", "Bob'da ACK hazır anı", has("hazir"), ""),
        ("transport", "ACK telde çıkış (cap/B_vb0 out, Bob bağlı portu → Alice geçici portu)", bool(out_b),
         f"{len(out_b or [])} datagram"),
        ("transport", "Alice'e varış (cap/A_va0 in)", bool(in_a), f"{len(in_a or [])} datagram"),
        ("transport", f"ölçülen darboğaz çıkışı (cap/MID_{run.dev}.jsonl out)", run.bottleneck_capture,
         f"{len(run.mb0)} paket"),
        ("transport", f"darboğaz kuyruk serisi ({run.dev})", bool(run.qdisc), f"{len(run.qdisc)} örnek"),
        ("transport", "zaman sıfırı (basamak.jsonl / sync/t0 / t_start)", run.t0 is not None, run.t0_source),
        ("ptt", "PTT tel baytı ve datagramı (wire.ptt_wire_bytes / ptt_wire_datagrams > 0)",
         bool(w) and "ptt_wire_bytes" in w and (w.get("ptt_wire_datagrams") or 0) > 0,
         f"bayt={(w or {}).get('ptt_wire_bytes')} datagram={(w or {}).get('ptt_wire_datagrams')}"),
        ("ptt", "basışta birleşik 300 ms (alice.json ptt_presses)",
         bool(chosen) and all(c.get("bundle_ms") == 300 and c.get("coalesced") is True for c in chosen),
         str(chosen)[:120]),
        ("ptt", "telde 0x17 (wire.ack_rx.0x17 > 0)", (rx.get("0x17") or 0) > 0, f"0x17={rx.get('0x17')}"),
        ("ptt", "alıcıda basış kaydı (bob.json ptt_presses)", bool(run.bob_sum.get("ptt_presses")), ""),
    ]
    print(f"ALAN DENETİMİ {d}")
    for grp, name, ok, info in rows:
        print(f"  [{grp}] {'OKUNDU' if ok else 'EKSİK '}  {name}" + (f"  ({info})" if info else ""))
    for grp in ("transport", "ptt"):
        miss = [n for g, n, ok, _ in rows if g == grp and not ok]
        print(f"  {grp}: " + ("HAZIR" if not miss else f"HAZIR DEĞİL — eksik {len(miss)}: " + "; ".join(miss)))
    print("  not: alındının Bob'dan çıkış anı okuyucuda yok (alan adı gelince eklenir); yokken alındı "
          "dönüşü üst sınırla, > 4 s ise SINANMADI.")
    return rows


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("run_dir", type=pathlib.Path)
    ap.add_argument("--plan", help="an:B/s,… (varsayılan senaryo v1)")
    ap.add_argument("--t0", type=float, help="zaman çizelgesinin sıfırı (unix s); varsayılan t_start")
    ap.add_argument("--baseline", type=pathlib.Path, help="kesmesiz koşu dizini (kısa daralma)")
    ap.add_argument("--bottleneck-dev", help="darboğaz arayüzü (varsayılan: realized_post'tan; yoksa mb0)")
    ap.add_argument("--json", action="store_true")
    ap.add_argument("--alan-denetimi", action="store_true",
                    help="yalnız hükmü bağlayan alanların varlığını denetle (pahalı koşudan önce)")
    a = ap.parse_args(argv)
    if a.alan_denetimi:
        field_check(a.run_dir, a.bottleneck_dev)
        return 0
    plan = parse_plan(a.plan) if a.plan else None
    run, rows = evaluate(a.run_dir, plan, a.t0, a.baseline, a.bottleneck_dev)
    if a.json:
        print(json.dumps({"run_dir": str(a.run_dir), "t0": run.t0, "plan": run.plan,
                          "rows": [{"olcut": n, "hukum": v, "neden": notes} for n, v, notes in rows]},
                         ensure_ascii=False, indent=1))
    else:
        print(f"KOŞU {a.run_dir}  t0={run.t0}")
        for n, v, notes in rows:
            print(f"{n:<18} {v}")
            for x in notes:
                print(f"    - {x}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
