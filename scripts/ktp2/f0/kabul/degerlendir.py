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
* `node-alice/transport-start.txt`, `node-bob/transport-start.txt`: `motor=`, `ack_biçimi=`,
  `ack_gönderici_etkin=`, `ack_alıcı_etkin=`.
* Darboğaz arayüzü (`--bottleneck-dev`; varsayılan: `realized_post.json` `mid_qdisc`'te tbf/fifo
  kuyruğu olan arayüz, ör. Yerel'in düzeneğinde `ifb0`; yoksa `mb0`).
* `cap/MID_<darboğaz>.jsonl` `dir == "out"` = darboğazdan çıkan ileri tel (IP boyu, UDP/IP 28 B
  dahil; hız birimiyle aynı). Bu dosya yoksa ve darboğaz `mb0` değilse `cap/MID_mb0.jsonl` çıkışı
  kullanılır: kayıp darboğazdan SONRA uygulanıyorsa yolda düşenleri görmez, değer ALT SINIRDIR
  (alt sınır ≥ %90 ise GEÇTİ; değilse SINANMADI).
* `realized_post.json`: `mid_qdisc` (kuyruk düşmesi darboğaz arayüzündeki tbf/fifo sayacından).
* `t_start` (unix s): zaman çizelgesinin varsayılan sıfırı (`--t0` ile değişir).
* İsteğe bağlı `qdisc_series.jsonl` (bkz. EKSİK ALAN 2).

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
EKSİK ALAN 4 — PTT şeridine özel tel baytı yok: PTT bant ölçütü SINANMADI.
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
        self.start = {r: read_kv(d / f"node-{r}" / "transport-start.txt") for r in ("alice", "bob")}
        self.realized_post = read_json(d / "realized_post.json")
        self.dev = dev or bottleneck_dev(self.realized_post) or "mb0"
        cap = d / "cap" / f"MID_{self.dev}.jsonl"
        # Darboğaz çıkışı yakalanmadıysa mb0 çıkışı: kayıp darboğazdan sonraysa alt sınır.
        self.wire_lower_bound = not cap.is_file() and self.dev != "mb0"
        if self.wire_lower_bound:
            cap = d / "cap" / "MID_mb0.jsonl"
        self.mb0 = [p for p in read_jsonl(cap) if p.get("dir") == "out" and "t_unix_ns" in p]
        self.qdisc = read_jsonl(d / "qdisc_series.jsonl")
        if t0 is None:
            ts = d / "t_start"
            t0 = float(ts.read_text().strip()) if ts.is_file() else None
        self.t0 = t0  # unix s

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
        notes.append(f"{who}: motor={kv.get('motor')} ack_biçimi={kv.get('ack_biçimi')} "
                     f"gönderici={kv.get('ack_gönderici_etkin')} alıcı={kv.get('ack_alıcı_etkin')}")
        ok &= kv.get("motor") == "cc-port" and kv.get("ack_biçimi") == "bitmap"
    rx = (run.alice_sum.get("wire") or {}).get("ack_rx") or {}
    bitmap_acks = rx.get("0x15", 0) + rx.get("0x16", 0)
    notes.append(f"alice ack_rx={rx}")
    if not rx:
        return SINANMADI, notes + ["eksik: alice.json wire.ack_rx"]
    if not ok or bitmap_acks == 0:
        return KALDI, notes + ["kip kabul kipi değil (cc-port + bitmap, telde 0x15/0x16 > 0)"]
    return GECTI, notes


def crit_cc(run, plan):
    lg = run.lg()
    if not lg:
        return SINANMADI, ["eksik: lg (ACK başına LineGuard tavanı; EKSİK ALAN 1)"]
    parts, notes = [], []
    share, n = ready_share(run, T_D - 1.0, T_D)
    util = run.wire_in(T_D - 1.0, T_D) / max(1, capacity(plan, T_D - 1.0, T_D))
    if run.wire_lower_bound and util < 0.90:
        return SINANMADI, [f"önkoşul: düşüşten önceki 1 s'de tel ALT SINIR {util:.1%} (eksik: cap/MID_{run.dev}.jsonl)"]
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
    """`[lo, hi)` içinde darboğazın boş kaldığı süre (ms): ardışık çıkışlar arası boşluk − serileşme."""
    pts = sorted(run.rel_ns(p["t_unix_ns"]) for p in run.mb0 if lo <= run.rel_ns(p["t_unix_ns"]) < hi)
    sizes = {run.rel_ns(p["t_unix_ns"]): p["ip_len"] for p in run.mb0}
    idle = 0.0
    for a, b in zip(pts, pts[1:]):
        ser = sizes[b] / 12500.0
        idle += max(0.0, (b - a) - ser)
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
    elif run.wire_lower_bound:
        v = GECTI if 0.90 <= util else SINANMADI
        parts.append((v, None))
        notes.append(f"tel ALT SINIR (mb0 çıkışı; eksik: cap/MID_{run.dev}.jsonl) %90 → {v}; üst sınır sınanmadı")
    else:
        v = GECTI if 0.90 <= util <= 1.0 + MTU_WIRE / cap else KALDI
        parts.append((v, None)); notes.append(f"tel %90–100 → {v}")
    # Darboğaz beklemesi.
    series = [q for q in run.qdisc if q.get("dev") == run.dev and "backlog_bytes" in q]
    if not series:
        parts.append((SINANMADI, None))
        notes.append("eksik: qdisc_series.jsonl (darboğaz beklemesi ≤ 393 ms; EKSİK ALAN 2)")
    else:
        w = [q["backlog_bytes"] / rate_at(plan, run.rel_ns(q["t_unix_ns"])) * 1000.0
             for q in series if lo <= run.rel_ns(q["t_unix_ns"]) < hi]
        mx = max(w) if w else None
        v = SINANMADI if mx is None else (GECTI if mx <= WAIT_MAX_MS else KALDI)
        parts.append((v, None)); notes.append(f"darboğaz beklemesi en çok {mx} ms (≤ {WAIT_MAX_MS}) → {v}")
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
    if run.wire_lower_bound or (baseline is not None and baseline.wire_lower_bound):
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
            elif run.wire_lower_bound and util < 0.90:
                parts.append((SINANMADI, None))
                notes.append(f"[{lo:.1f}, {hi:.0f}) s tel ALT SINIR {util:.1%} < %90 (eksik: cap/MID_{run.dev}.jsonl)")
            else:
                v2 = GECTI if util >= 0.90 else KALDI
                parts.append((v2, None)); notes.append(f"[{lo:.1f}, {hi:.0f}) s tel {util:.1%} (≥ %90) → {v2}")
    return combine(parts), notes


def crit_ptt(run):
    presses = run.bob_sum.get("ptt_presses") or []
    if not presses:
        return SINANMADI, ["basış yok (bob.json ptt_presses boş)"]
    parts, notes = [], []
    rx = (run.alice_sum.get("wire") or {}).get("ack_rx") or {}
    v0 = GECTI if rx.get("0x17", 0) > 0 else KALDI
    parts.append((v0, None)); notes.append(f"telde 0x17 = {rx.get('0x17', 0)} → {v0}")
    for p in presses:
        planned, ontime = p.get("frames_planned", 0), p.get("frames_on_time", 0)
        share = ontime / planned if planned else None
        v1 = SINANMADI if share is None else (GECTI if share >= PTT_ON_TIME else KALDI)
        one = p.get("ptt_started") == 1 and p.get("ptt_ended") == 1
        v2 = GECTI if one else KALDI
        v3 = GECTI if p.get("bundle_ms") == 300 else KALDI
        parts += [(v1, None), (v2, None), (v3, None)]
        notes.append(f"akış {p.get('stream')}: demet {p.get('bundle_ms')} ms → {v3}; zamanında "
                     f"{ontime}/{planned} → {v1}; açılış/kapanış {p.get('ptt_started')}/{p.get('ptt_ended')} "
                     f"→ {v2}; ilk ses {p.get('first_audio_ms')} ms; oluşum→teslim "
                     f"{p.get('creation_to_delivery_ms')}")
    parts.append((SINANMADI, None))
    notes.append("eksik: PTT şeridine özel tel baytı (bant 1,5 s ≤ 937 B, 8,4 s ≤ 5 250 B; EKSİK ALAN 4)")
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
        out.append((key, run.rel_ms(t), lat))
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


def evaluate(d, plan, t0=None, baseline_dir=None, dev=None):
    run = Run(d, t0, dev)
    if run.t0 is None:
        raise SystemExit("t_start yok: --t0 verin")
    baseline = Run(baseline_dir, None, dev) if baseline_dir else None
    mode_v, mode_n = mode_evidence(run)
    mode_n = mode_n + [f"darboğaz arayüzü {run.dev}; tel kaynağı "
                       + ("cap/MID_mb0.jsonl (ALT SINIR)" if run.wire_lower_bound else f"cap/MID_{run.dev}.jsonl")]
    rows = [("kip (önkoşul)", mode_v, mode_n)]
    for name, fn in (("1 CC", lambda: crit_cc(run, plan)),
                     ("3 hız sınırlama", lambda: crit_rate_limit(run, plan, baseline)),
                     ("4 hızlanma", lambda: crit_speedup(run, plan)),
                     ("2 PTT", lambda: crit_ptt(run)),
                     ("5 metin/alındı", lambda: crit_text_receipt(run))):
        v, n = fn()
        if mode_v != GECTI and name != "5 metin/alındı" and v != SINANMADI:
            n = n + [f"kip önkoşulu {mode_v}: hüküm SINANMADI'ya çekildi (ölçü {v})"]
            v = SINANMADI
        rows.append((name, v, n))
    return run, rows


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("run_dir", type=pathlib.Path)
    ap.add_argument("--plan", help="an:B/s,… (varsayılan senaryo v1)")
    ap.add_argument("--t0", type=float, help="zaman çizelgesinin sıfırı (unix s); varsayılan t_start")
    ap.add_argument("--baseline", type=pathlib.Path, help="kesmesiz koşu dizini (kısa daralma)")
    ap.add_argument("--bottleneck-dev", help="darboğaz arayüzü (varsayılan: realized_post'tan; yoksa mb0)")
    ap.add_argument("--json", action="store_true")
    a = ap.parse_args(argv)
    plan = parse_plan(a.plan) if a.plan else PLAN_V1
    run, rows = evaluate(a.run_dir, plan, a.t0, a.baseline, a.bottleneck_dev)
    if a.json:
        print(json.dumps({"run_dir": str(a.run_dir), "t0": run.t0, "plan": plan,
                          "rows": [{"olcut": n, "hukum": v, "neden": notes} for n, v, notes in rows]},
                         ensure_ascii=False, indent=1))
    else:
        print(f"KOŞU {a.run_dir}  t0={run.t0}  plan={plan}")
        for n, v, notes in rows:
            print(f"{n:<18} {v}")
            for x in notes:
                print(f"    - {x}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
