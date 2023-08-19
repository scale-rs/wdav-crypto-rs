use dav_server::{fakels::FakeLs, localfs::LocalFs, DavHandler};
use std::net::{IpAddr, SocketAddr};
use std::os::unix::prelude::PermissionsExt;
use std::{env, fs, io};
use warp::Filter;

const WDAV_SYMLINKS: &'static str = "/tmp/wdav_symlinks";
const WDAV_SYMLINKS_WRITE: &'static str = "/tmp/wdav_symlinks/write";
const WDAV_SYMLINKS_READ: &'static str = "/tmp/wdav_symlinks/read";
const WDAV_DIRS: &'static str = "/tmp/wdav_dirs";

/// User read & execute, but no write, permission.
const ACL_U_RX: u32 = 0o500;
/// User read, write & execute permission.
const ACL_U_WRX: u32 = 0o700;

#[tokio::main]
async fn main() -> io::Result<()> {
    let port = env::var("PORT").unwrap_or("8080".to_string());
    let port = port.parse::<u16>().unwrap();

    let cd = std::env::current_dir().unwrap();
    let ip: IpAddr = "127.0.0.1".parse().unwrap();
    let addr = SocketAddr::new(ip, port);
    println!("current dir : {}", cd.to_string_lossy().to_string());

    fs::create_dir_all(WDAV_SYMLINKS)?;
    {
        let mut perms = fs::metadata(WDAV_DIRS)?.permissions();
        perms.set_mode(ACL_U_RX); // rwx

        fs::set_permissions(WDAV_DIRS, perms)?;
        // @TODO
        // - chmod a-w /tmp/wdav
        // - create folders through admin interface only
        // - create folders as symlinks
    }
    let mut dav_filter = {
        // No symlinks by default:
        //
        //let warp_dav = dav_server::warp::dav_dir(base, true, true);

        // With symlinks allowed: Content of symlinked directories IS served, but such directories
        // themselves are not shown. That is excellent for our need-to-know-based ACL.
        //
        // In GNOME open the WebDAV directory with: nautilus dav://127.0.0.1:4201/subdir-here
        let dav_builder = DavHandler::builder()
            .filesystem(LocalFs::new(WDAV_SYMLINKS, false, false, false))
            .locksystem(FakeLs::new())
            .autoindex(true);
        //.strip_prefix("/wdav");

        /*if index_html {
            builder = builder.indexfile("index.html".to_string())
        }*/
        let dav_handler = dav_builder.build_handler();
        dav_server::warp::dav_handler(dav_handler)
    };

    //warp::serve(warp_dav).run(addr).await;

    let admin = warp::path!("admin").map(|| "Hello, World - no slash!");
    let admin_more = warp::path!("admin" / ..).map(|| "Hello, World - slash!");

    let routes = warp::any().and(admin.or(admin_more).or(dav_filter));
    let _ = warp::get();

    println!("listening on {}.", addr);
    warp::serve(routes).run(addr).await;
    Ok(())
}
