//! H-FEC-ORDER — sahte shard `open()`'dan geçemediği için decoder'a HİÇ girmez; yeterli geçerli
//! shard ile kurtarma çalışır (INV-FEC-1: authenticate-before-decode).
//!
//! Decoder burada sahtedir (çağrı sayar, `K` farklı shard'la birleştirir). Sınanan şey FEC
//! matematiği değil, seam'in decoder'ın ÖNÜNDEKİ kapı olmasıdır.

use kama_contracts::s3::PacketProtection;

use crate::s3_support::{established, session, tamper};
use crate::target::S3Target;

/// Kurtarma için gereken shard sayısı.
const K: usize = 4;
const SHARD_BODY: usize = 8;

/// Sahte decoder: her beslemeyi sayar; `K` farklı shard gelince bloğu sırayla birleştirir.
#[derive(Default)]
struct CountingDecoder {
    calls: usize,
    shards: [Option<Vec<u8>>; K],
}

impl CountingDecoder {
    fn feed(&mut self, frame: &[u8]) {
        self.calls += 1;
        let idx = usize::from(frame[0]);
        if idx < K {
            self.shards[idx] = Some(frame[1..].to_vec());
        }
    }

    fn reconstruct(&self) -> Option<Vec<u8>> {
        let mut out = Vec::with_capacity(K * SHARD_BODY);
        for s in &self.shards {
            out.extend_from_slice(s.as_ref()?);
        }
        Some(out)
    }
}

/// Alıcı hattı: yalnız `open()`'dan geçen çerçeve decoder'a verilir.
fn receive<P: PacketProtection>(p: &mut P, dec: &mut CountingDecoder, wire: &[u8]) {
    if let Some(o) = p.open(wire) {
        dec.feed(&o.frame);
    }
}

pub fn forged_shard_never_reaches_decoder<T: S3Target>() {
    let mut p = established::<T>();
    let ctx = session(p.h);
    let block: Vec<u8> = (0..(K * SHARD_BODY) as u8).collect();
    let wires: Vec<Vec<u8>> = block
        .chunks(SHARD_BODY)
        .enumerate()
        .map(|(i, body)| {
            let mut frame = vec![i as u8];
            frame.extend_from_slice(body);
            p.a.seal(ctx, &frame).expect("seal").bytes
        })
        .collect();

    let mut dec = CountingDecoder::default();
    // Sahte shard'lar: her geçerli shard'ın bozulmuş kopyası.
    for w in &wires {
        receive(&mut p.b, &mut dec, &tamper(w));
    }
    assert_eq!(dec.calls, 0, "sahte shard decoder'a girdi");
    assert!(dec.reconstruct().is_none());

    // K geçerli shard (aralarına bir sahte daha karışmış) → kurtarma başarılı.
    for (i, w) in wires.iter().enumerate() {
        if i == 1 {
            receive(&mut p.b, &mut dec, &tamper(w));
        }
        receive(&mut p.b, &mut dec, w);
    }
    assert_eq!(dec.calls, K, "decoder yalnız geçerli shard'ları gördü");
    assert_eq!(dec.reconstruct(), Some(block));
}
