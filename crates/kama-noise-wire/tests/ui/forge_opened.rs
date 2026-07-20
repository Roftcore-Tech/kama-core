// OpenedTransportShard'ı doğrudan struct-literal ile üretmek DERLENMEMELİ (alanlar private + _proof).
fn main() {
    let _ = kama_noise_wire::OpenedTransportShard {
        // alanlar private → E0451 / private-field
    };
}
