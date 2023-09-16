use std::io;
use tmpwdav_1_q0047082::server;

const _NOT_MOCKABLE: () = {
    #[cfg(feature = "mockable")]
    panic!(
        "Use `mockable` (and related) feature only when testing (and only through `test-binary`)."
    );
};

#[tokio::main]
pub async fn main() -> io::Result<()> {
    server::main().await
}
