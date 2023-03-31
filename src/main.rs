use std::ffi::OsString;
use std::net::{IpAddr, SocketAddr};

use clap::Parser;
use dav_server::warp::dav_dir;

#[derive(Debug, Parser)]
#[command(name = "wdav")]
#[command(about = "Quick start a webdav server", long_about = None)]
struct Cli {
    #[arg(short, long, default_value = ".", help = "Attach to webdav root")]
    folder: OsString,
    #[arg(short, long, default_value = "0.0.0.0", help = "Address of listen")]
    address: OsString,
    #[arg(short, long, default_value_t = 8080, help = "Port of listen")]
    port: u16,
}

#[tokio::main]
async fn main() {
    let args = Cli::parse();
    match args {
        Cli {
            folder,
            address,
            port,
        } => {
            let cd = std::env::current_dir().unwrap();
            let path = folder.to_string_lossy().to_string();
            let addr: IpAddr = address.to_string_lossy().to_string().parse().unwrap();
            let addr = SocketAddr::new(addr, port);
            println!("current dir : {}", cd.to_string_lossy().to_string());
            println!("listening on {} serving {}", addr, path);
            let warp_dav = dav_dir(path, true, true);
            warp::serve(warp_dav).run(addr).await;
        }
    }
}
