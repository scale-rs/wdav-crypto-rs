use std::io;
use tmpwdav_1_q0047082::server;

#[tokio::main]
pub async fn main() -> io::Result<()> {
    server::main().await
}
