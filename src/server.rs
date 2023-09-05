use crate::entry;
use crate::{ADMIN, READ, SYMLINKS, SYMLINKS_READ, SYMLINKS_WRITE, WRITE};
use askama::Template;
use dav_server::{self, fakels::FakeLs, localfs::LocalFs, DavMethod};
pub use entry::Entry;
use http::{uri::Uri, StatusCode};
use std::collections::HashMap;
use std::fmt::Debug;
use std::fs::{self};
use std::io;
use std::path::Path;
use warp::http::{self};
use warp::{redirect, reject::Reject, reject::Rejection, reply, Filter};

use crate::DEFAULT_PORT;
use crate::DIRS;
use crate::ENV_DATA_KEY;
use crate::ENV_PORT;
use crate::ENV_SALT;
use dav_server::DavConfig;
use dav_server::DavHandler;
use dav_server::DavMethodSet;
use std::env;
use std::net::IpAddr;
use std::net::SocketAddr;
use warp;
use warp::http::HeaderValue;
use warp::reject;
use warp::Reply;

pub(crate) fn dav_config(
    prefix_segment: impl core::fmt::Display,
    dir_path: impl AsRef<Path>,
    methods: DavMethodSet,
) -> DavConfig {
    // No symlinks by default. The following disables symlinks:
    //
    //let warp_dav = dav_server::warp::dav_dir(base, true, true);

    // LocalFs::new(...) enables symlinks. With symlinks allowed: Content of symlinked directories
    // IS served, but such directories themselves are not shown by default. That's documented, see
    // dav_server::DavConfig::hide_symlinks(...).
    //
    // That is excellent for our need-to-know-based ACL.
    //
    // In GNOME open the WebDAV directory with: nautilus dav://127.0.0.1:4201/subdir-here
    DavHandler::builder()
        .filesystem(LocalFs::new(dir_path, false, false, false))
        //--- @TODO SYMLINKS_READ to a param
        .locksystem(FakeLs::new())
        .autoindex(true) //@TODO
        .indexfile("index.html")
        .methods(methods)
        //.strip_prefix("/".to_owned() + prefix_segment)
        .strip_prefix(format!("/{}", prefix_segment))
}

#[derive(Debug)]
pub(crate) struct Rej<T>(T)
where
    T: Debug + Sized + Send + Sync;

unsafe impl<T> Send for Rej<T> where T: Debug + Sized + Send + Sync {}

unsafe impl<T> Sync for Rej<T> where T: Debug + Sized + Send + Sync {}

impl<T> Reject for Rej<T> where T: Debug + Sized + Send + Sync + 'static {}

pub(crate) fn redirect_see_other<L>(location: L) -> Result<impl Reply, Rejection>
where
    HeaderValue: TryFrom<L>,
    <HeaderValue as TryFrom<L>>::Error: Into<http::Error>,
{
    // Content-type', 'text/html')
    //warp::reply::with::headers(headers)
    Ok(warp::reply::with_status(
        warp::reply::with_header(warp::reply(), "Location:", location),
        StatusCode::SEE_OTHER,
    ))
    /*
    Ok(warp::reply::with_header(
        warp::reply::with_status(warp::reply(), StatusCode::SEE_OTHER),
        "Location:",
        location,
    ))
    */
}

/// Directory entries, mapped by their (potentially lossy) names.
#[derive(Template)]
#[template(path = "admin_list.html")]
pub(crate) struct AdminListTemplate {
    pub(crate) entries: HashMap<String, Entry>,
}

// Thanks to https://blog.logrocket.com/template-rendering-in-rust
pub(crate) type WebResult<T> = std::result::Result<T, Rejection>;

pub(crate) async fn admin_list() -> WebResult<impl Reply> {
    let entries = entry::get_entries().map_err(|e| reject::custom(Rej(e)))?;

    let template = AdminListTemplate { entries };
    let res = template.render().map_err(|e| reject::custom(Rej(e)))?;
    Ok(reply::html(res))
}

pub(crate) async fn admin_add(dir_name: String) -> Result<impl Reply, Rejection> {
    let dir_result = fs::create_dir(format!("{DIRS}/{dir_name}"));
    if let Err(e) = dir_result {
        return Err(reject::custom(Rej(e)));
    }
    //redirect_see_other(format!("/{ADMIN}"))
    Ok(redirect::see_other(
        format!("/{ADMIN}").parse::<Uri>().expect("Admin UR"),
    ))
}

pub(crate) async fn admin_remove_write(dir_name: String) -> Result<impl Reply, Rejection> {
    let dir_result = fs::create_dir(format!("{DIRS}/{dir_name}"));
    if let Err(e) = dir_result {
        return Err(reject::custom(Rej(e)));
    }
    // @TODO replace with redirect::see_other(...):
    redirect_see_other(format!("/{ADMIN}"))
}

pub(crate) async fn main() -> io::Result<()> {
    let port = env::var(ENV_PORT).unwrap_or(DEFAULT_PORT.to_string());
    let port = port.parse::<u16>().unwrap();

    let salt = env::var(ENV_SALT).expect("Requiring SALT env variable.");
    let data_key = env::var(ENV_DATA_KEY).expect("Requiring 'data key', formerly known as 'project key'. It should be passed automatically by Deta on both Deta platform and local `space dev`.");

    let ip: IpAddr = "127.0.0.1".parse().unwrap();
    let addr = SocketAddr::new(ip, port);

    fs::create_dir_all(DIRS)?;
    fs::create_dir_all(SYMLINKS)?;
    fs::create_dir_all(SYMLINKS_READ)?;
    fs::create_dir_all(SYMLINKS_WRITE)?;

    let dav_read_filter = {
        // DavMethodSet::add(&mut self, DavMethod) is ugly. And there is no direct method to
        // add/merge/union two instances of DavMethodSet. But, for now, the following:
        let mut read_only = DavMethodSet::HTTP_RO;
        read_only.add(DavMethod::PropFind);

        let dav_handler = dav_config(READ, SYMLINKS_READ, read_only).build_handler();
        dav_server::warp::dav_handler(dav_handler)
    };

    let dav_write_filter = {
        // The following is impractical/redundant, but it's currently the only portable/correct way.
        let mut read_write = DavMethodSet::WEBDAV_RW;
        // Based on source of DavMethodSet::HTTP_RW:
        read_write.add(DavMethod::Get);
        read_write.add(DavMethod::Head);
        read_write.add(DavMethod::Options);
        read_write.add(DavMethod::Put);

        let dav_handler = dav_config(WRITE, SYMLINKS_WRITE, read_write).build_handler();
        dav_server::warp::dav_handler(dav_handler)
    };

    let admin_list = warp::path(ADMIN)
        .and(warp::path::end())
        .and_then(admin_list);

    // HTTP POST with URL parameters is unusual, but easy to handle & test
    let admin_add = warp::post()
        .and(warp::body::content_length_limit(1024 * 16))
        .and(warp::path!("admin" / "add" / String))
        .and_then(admin_add);

    let routes = warp::any().and(
        admin_list
            .or(admin_add)
            .or(warp::path(READ).and(dav_read_filter))
            .or(warp::path(WRITE).and(dav_write_filter)),
    );

    println!("listening on {}.", addr);
    warp::serve(routes).run(addr).await;
    Ok(())
}
