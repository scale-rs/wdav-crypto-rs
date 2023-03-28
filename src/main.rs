use dav_server::warp::dav_dir;
use std::net::SocketAddr;

#[tokio::main]
async fn main() {
    env_logger::init();
    let dir = ".";
    let addr: SocketAddr = ([0, 0, 0, 0], 8080).into();
    println!("listening on {:?} serving {}", addr, dir);
    let warp_dav = dav_dir(dir, true, true);
    warp::serve(warp_dav).run(addr).await;
}
