// H-HANDLE (tip yarısı): HubHandle, SessionHandle'ın yerine konamaz.
use kama_contracts::s3::{HubHandle, ProtectionContext};

fn main() {
    let hub = HubHandle::from_raw(7);
    // `Session` bir `SessionHandle` ister; wire routing tutamacı oturum bağlamı OLAMAZ.
    let _ctx = ProtectionContext::Session(hub);
}
