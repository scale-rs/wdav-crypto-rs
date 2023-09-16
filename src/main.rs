use std::io;
use tmpwdav_1_q0047082::server;

const _MOCKABLE_IN_DEBUG_ONLY: () = {
    #[cfg(all(not(debug_assertions), feature = "mockable"))]
    if true {
        panic!("Use `mockable` (and related) feature only in debug build.");
    }
};

#[tokio::main]
pub async fn main() -> io::Result<()> {
    server::main().await
}
