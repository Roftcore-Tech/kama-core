#!/usr/bin/env python3
"""[M4-fizik-siniri] İdeal ARQ / ideal FEC+ARQ gecikme modeli — netem FIFO kanalı (Monte Carlo).

Model kodu; ürün kodu değil. Etiket: [türetilmiş] (girdiler [kod]/[hücre dosyası]).

Kanal (iki yön ayrı netem qdisc; REV = FWD):
  kayıp: enqueue'da Bernoulli q (kaybolan paket serileştirme tüketmez)
  rate + jitter: tts = max(last_tts, t + d_j) + boy/C   (last_tts > t ise), değilse t + d_j + boy/C
  (sch_netem.c netem_enqueue rate kolu; scripts/ktp2/f0/b/fizik.py:fifo_owd ile aynı model)

Politikalar (gönderen sunum takviminde, kuyruksuz; pencere/pacer YOK = ideal gönderen):
  oracle : kaybolan kopya gönderiminden RTT0 = 2d + ser_data + ser_ack sonra yeniden gönderilir
           (geri besleme kaybı/gecikmesi yok; fizik.py "ideal ARQ" + FIFO jitter + hat işgali).
  rfc    : her varışta ACK (tam SACK kümesi), ACK ters hatta kayıplı/FIFO; gönderen RFC 9002
           paket eşiği 3 / zaman eşiği 9/8·max(srtt, latest_rtt) + PTO (srtt + 4·rttvar).
           (Kama loss.rs:14-21 ile aynı eşikler; Kama alıcısı 2 DATA'da bir ACK'ler — burada her
           varışta: ideal sınır.)
  fec    : rfc + blok K birim dolunca (birim K−1 gönderildiği AN) R onarım (boy E + başlık);
           alıcı blokta ≥ K parça (veri ∪ onarım) olunca eksik birimleri teslim eder (kurtarılan
           birim ACK'te teyitli sayılır → ARQ durur). Kısmi son blok: onarım YOK (Kama runtime.rs
           yalnız K dolunca kapatır; kısmi kapanış yalnız döngü sonunda).
"""
import heapq
import math
import random

L234 = 42
TAG = 16          # bench IdentitySeam (seam.rs:105)
HDR_DATA = 11     # wire.rs:17
HDR_FEC = 12      # wire.rs:104
ACK_BODY0 = 15    # ack.rs:204


def data_l2(p):
    return L234 + TAG + HDR_DATA + p


def rep_l2(e):
    return L234 + TAG + HDR_FEC + e


ACK_L2 = L234 + TAG + ACK_BODY0


class Link:
    __slots__ = ("C", "d", "s", "q", "rng", "last", "tx_bytes", "tx_pkts")

    def __init__(self, C, d, s, q, rng):
        self.C, self.d, self.s, self.q, self.rng = C, d, s, q, rng
        self.last = -1e18
        self.tx_bytes = 0
        self.tx_pkts = 0

    def send(self, t, size):
        self.tx_bytes += size
        self.tx_pkts += 1
        if self.q > 0 and self.rng.random() < self.q:
            return None
        dj = self.rng.gauss(self.d, self.s) if self.s > 0 else self.d
        if dj < 0:
            dj = 0.0
        ser = size / self.C
        if self.last > t:
            tts = max(self.last, t + dj) + ser
        else:
            tts = t + dj + ser
        self.last = tts
        return tts


def run(C, d, sig, q, I, M, pay, policy, K=8, R=2, E=1024, seed=1, ack_every=1, close_partial=False):
    """Tek koşu. Dönüş: (gecikme listesi [s] M öğe — teslim edilmeyen None, ileri L2 B, süre s)."""
    rng = random.Random(seed)
    fwd = Link(C, d, sig, q, rng)
    rev = Link(C, d, sig, q, rng)
    s_data = data_l2(pay)
    s_rep = rep_l2(E)
    rtt0 = 2 * d + s_data / C + ACK_L2 / C
    ev = []
    cnt = [0]

    def push(t, kind, a=None):
        cnt[0] += 1
        heapq.heappush(ev, (t, cnt[0], kind, a))

    offer = [i * I for i in range(M)]
    deliv = [None] * M
    for i in range(M):
        push(offer[i], "OFFER", i)
    # gönderen
    seq_next = [0]
    seq_info = {}          # seq -> (unit or -1-block for repair, send_t)
    outstanding = {}       # seq -> unit (data only, unacked & undeclared)
    unit_acked = [False] * M
    largest = [-1]
    srtt = [rtt0]
    rttvar = [rtt0 / 2]
    latest = [rtt0]
    timer_set = set()
    # alıcı
    log = []               # ('s', seq) / ('u', unit)
    block_cnt = {}
    block_seen = {}
    block_done = set()
    proc = [0]
    since_ack = [0]
    fec_on = policy == "fec"
    nblocks_full = M // K

    def tx_data(t, u):
        s = seq_next[0]
        seq_next[0] += 1
        seq_info[s] = (u, t)
        arr = fwd.send(t, s_data)
        if policy == "oracle":
            if arr is None:
                push(t + rtt0, "ORTX", u)
            else:
                push(arr, "ARR", (s, u, False))
            return
        outstanding[s] = u
        if arr is not None:
            push(arr, "ARR", (s, u, False))
        push(t + srtt[0] + 4 * rttvar[0], "PTO", s)

    def tx_repair(t, b, r):
        s = seq_next[0]
        seq_next[0] += 1
        seq_info[s] = (-1 - b, t)
        arr = fwd.send(t, s_rep)
        if arr is not None:
            push(arr, "ARR", (s, -1 - b, True))

    def thr():
        return 9.0 / 8.0 * max(srtt[0], latest[0])

    def declare(t, s):
        u = outstanding.pop(s, None)
        if u is None:
            return
        if not unit_acked[u]:
            tx_data(t, u)

    while ev:
        t, _, kind, a = heapq.heappop(ev)
        if kind == "OFFER":
            u = a
            tx_data(t, u)
            if fec_on and u % K == K - 1:
                b = u // K
                for r in range(R):
                    tx_repair(t, b, r)
            if fec_on and close_partial and u == M - 1 and (u % K) != K - 1:
                b = u // K
                for r in range(R):
                    tx_repair(t, b, r)
        elif kind == "ORTX":
            u = a
            if deliv[u] is None:
                tx_data(t, u)
        elif kind == "ARR":
            s, u, is_rep = a
            if u >= 0:
                if deliv[u] is None:
                    deliv[u] = t
                    log.append(("u", u))
                b = u // K
                shard = u % K
            else:
                b = -1 - u
                shard = K + (s % 1000003)  # onarım parçası: ayrık kimlik (her onarım farklı)
            if fec_on:
                seen = block_seen.setdefault(b, set())
                if shard not in seen:
                    seen.add(shard)
                    if b not in block_done and len(seen) >= K and (b < nblocks_full or close_partial):
                        block_done.add(b)
                        for v in range(b * K, min(M, b * K + K)):
                            if deliv[v] is None:
                                deliv[v] = t
                                log.append(("u", v))
            if policy == "oracle":
                continue
            log.append(("s", s))
            since_ack[0] += 1
            if since_ack[0] >= ack_every:
                since_ack[0] = 0
                arr = rev.send(t, ACK_L2)
                if arr is not None:
                    push(arr, "ACK", len(log))
        elif kind == "ACK":
            n = a
            new_largest = False
            for k in range(proc[0], n):
                typ, x = log[k]
                if typ == "u":
                    unit_acked[x] = True
                else:
                    outstanding.pop(x, None)
                    if x > largest[0]:
                        largest[0] = x
                        new_largest = True
                        smp = t - seq_info[x][1]
                        latest[0] = smp
                        rttvar[0] = 0.75 * rttvar[0] + 0.25 * abs(srtt[0] - smp)
                        srtt[0] = 0.875 * srtt[0] + 0.125 * smp
            if n > proc[0]:
                proc[0] = n
            # kurtarılan birimin kopyaları artık gereksiz
            for s in [s for s, u in outstanding.items() if unit_acked[u]]:
                outstanding.pop(s, None)
            L = largest[0]
            th = thr()
            for s in sorted(x for x in outstanding if x < L):
                if L - s >= 3 or t - seq_info[s][1] >= th:
                    declare(t, s)
                elif s not in timer_set:
                    timer_set.add(s)
                    push(seq_info[s][1] + th, "LT", s)
        elif kind == "LT":
            s = a
            timer_set.discard(s)
            if s in outstanding and s < largest[0]:
                if t - seq_info[s][1] >= thr() - 1e-9:
                    declare(t, s)
                else:
                    timer_set.add(s)
                    push(seq_info[s][1] + thr(), "LT", s)
        elif kind == "PTO":
            s = a
            if s in outstanding:
                u = outstanding[s]
                if not unit_acked[u]:
                    declare(t, s)
    lat = [None if deliv[i] is None else deliv[i] - offer[i] for i in range(M)]
    dur = max([x for x in deliv if x is not None] + [offer[-1]])
    return lat, fwd.tx_bytes, dur


def pct(vals, qq):
    v = sorted(vals)
    if not v:
        return None
    if len(v) == 1:
        return v[0]
    pos = qq * (len(v) - 1)
    lo = math.floor(pos)
    hi = min(lo + 1, len(v) - 1)
    return v[lo] + (v[hi] - v[lo]) * (pos - lo)
