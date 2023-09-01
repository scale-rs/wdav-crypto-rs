use askama::Template;
/// This does use Syn + Parse crate, which increase build times. But Askama's attribute (procedural)
/// macro uses those anyway.
use const_format::formatcp;
use core::panic;
use dav_server::{
    fakels::FakeLs, localfs::LocalFs, DavConfig, DavHandler, DavMethod, DavMethodSet,
};
use http::uri::Uri;
use std::collections::HashMap;
use std::fmt::Debug;
use std::fs::{self, DirEntry};
use std::net::{IpAddr, SocketAddr};
use std::{env, io, path::Path, path::PathBuf};
use warp::http::{self, HeaderValue, StatusCode};
use warp::{redirect, reject, reject::Reject, reject::Rejection, reply, Filter, Reply};

/// Environment variable name that contains the port number assigned by Deta.Space.
const ENV_PORT: &'static str = "PORT";
const DEFAULT_PORT: &'static str = "8080";

/// Environment variable name that contains private key ("data key", formerly known as "project
/// key", generated by Deta.Space. (See also
/// <https://deta.space/docs/en/build/fundamentals/data-storage#manual-setup>).
const ENV_DATA_KEY: &'static str = "DETA_PROJECT_KEY";

/// Environment variable name that contains "salt", so that users whom you give write hashes can't
/// brute-force your Deta.Space private key.
const ENV_SALT: &'static str = "SALT";

// Directory names here don't have a trailing slash.
//
const TMP: &'static str = "/tmp";
const DIRS: &'static str = formatcp!("{TMP}/wdav_dirs");

// Leading URL "segments" (top level directories). Warp requires them NOT to contain any slash.
const READ: &'static str = "read";
const WRITE: &'static str = "write";
const ADMIN: &'static str = "admin";
const ADD: &'static str = "add";

// Directories containing symlinks. These constants could use `const_format` crate. But that
// involves quote + syn = long build times. TODO reconsider because of Tokio, or don't use Tokio
// attrib. macro.
const SYMLINKS: &'static str = "/tmp/wdav_symlinks";
const SYMLINKS_WRITE: &'static str = formatcp!("{SYMLINKS}/{WRITE}");
const SYMLINKS_READ: &'static str = formatcp!("{SYMLINKS}/{READ}");

const CLEANUP_IN_PROGRESS: &'static str = formatcp!("{SYMLINKS}/CLEANUP_IN_PROGRESS");

const EMPTY_NAME: String = String::new();

fn dav_config(
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
struct Rej<T>(T)
where
    T: Debug + Sized + Send + Sync;

unsafe impl<T> Send for Rej<T> where T: Debug + Sized + Send + Sync {}
unsafe impl<T> Sync for Rej<T> where T: Debug + Sized + Send + Sync {}
impl<T> Reject for Rej<T> where T: Debug + Sized + Send + Sync + 'static {}

fn redirect_see_other<L>(location: L) -> Result<impl Reply, Rejection>
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

/// Require `path` leaf part not to be `..`.
fn file_name_leaf(path: &Path) -> String {
    path.file_name()
        .expect("The path must not be `..`")
        .to_string_lossy()
        .to_string()
}

/// Return the target - but as-is, NOT canonical!
fn read_link_full<P: AsRef<Path>>(path: &P) -> String {
    let link = fs::read_link(path).expect("Expecting {path} to be a symlink.");
    link.as_os_str().to_string_lossy().to_string()
}

fn exists(path: &Path) -> bool {
    let target_exists = path.try_exists();
    matches!(target_exists, Ok(true))
}

#[derive(Debug)]
enum SecondaryIncorrectKind {
    OrphanOrDifferentSymlink { target: String, is_orphan: bool },
    NonSymlink { is_dir: bool },
}

type WriteNameAndKind = (
    String, /*write_name*/
    Result<(), SecondaryIncorrectKind>,
);

#[derive(Debug)]
enum ReadAndOrWriteIncorrectKind {
    PrimaryAndReadIncorrect {
        read: SecondaryIncorrectKind,
        write: Option<WriteNameAndKind>,
    },
    PrimaryAndReadOkButWriteIncorrect {
        write_name: String,
        write: SecondaryIncorrectKind,
    },
    PrimaryAndWriteOnly {
        write_name: String,
    },
    PrimaryAndWriteOnlyAndIncorrect {
        write_name: String,
        write: SecondaryIncorrectKind,
    },
}

/// Dir entry immediately below either [DIRS], and/or [SYMLINKS_READ] and/or [SYMLINKS_WRITE].
#[derive(Debug)]
enum Entry {
    PrimaryOnly {
        name: String,
    },
    PrimaryAndReadOnly {
        name: String,
    },
    PrimaryAndReadWrite {
        name: String,
        // Write symlink (hash-based) source name
        write_name: String,
    },
    PrimaryAndReadAndOrWriteIncorrect {
        name: String,
        kind: ReadAndOrWriteIncorrectKind,
    },
    PrimaryNonDir {
        name: String,
        path: PathBuf,
    },

    SecondaryIncorrect {
        name: String,
        /// Whether it's under [SYMLINKS_READ]. Otherwise it's under [SYMLINKS_WRITE].
        is_read: bool,
        kind: SecondaryIncorrectKind,
    }, /*,
       SecondaryReadOrphanSymlink {
           name: String,
           target: String,
       },
       SecondaryReadNonSymlink {
           name: String,
           is_dir: bool,
       },
       SecondaryWriteOrphanSymlink {
           name: String,
           target: String,
       },
       SecondaryWriteNonSymlink {
           name: String,
           is_dir: bool,
       },*/
}

impl Entry {
    fn is_ok_and_complete(&self) -> bool {
        match self {
            Self::PrimaryAndReadOnly { .. } | Self::PrimaryAndReadWrite { .. } => true,
            _ => false,
        }
    }
    fn is_readable(&self) -> bool {
        self.is_ok_and_complete()
    }
    fn is_writable(&self) -> bool {
        matches!(self, Self::PrimaryAndReadWrite { .. })
    }
    fn name(&self) -> &str {
        match self {
            Self::PrimaryOnly { name }
            | Self::PrimaryAndReadOnly { name }
            | Self::PrimaryAndReadWrite { name, .. }
            | Self::PrimaryAndReadAndOrWriteIncorrect { name, .. }
            | Self::PrimaryNonDir { name, .. }
            | Self::SecondaryIncorrect { name, .. } => &name,
        }
    }
    fn write_name(&self) -> &str {
        match self {
            Self::PrimaryAndReadWrite {
                name: _,
                write_name: write,
            } => &write,
            _ => unreachable!(
                "Can be called only on ReadWrite variant, but it was invoked on {:?}.",
                self
            ),
        }
    }

    fn new_under_dirs(entry: DirEntry) -> Self {
        let path = entry.path();
        let name = path.to_string_lossy().to_string();
        if path.is_dir() {
            Self::PrimaryOnly { name }
        } else {
            Self::PrimaryNonDir { name, path }
        }
    }

    fn and_readable_symlink(self, entry: DirEntry) -> Self {
        let path = entry.path();

        if let Self::PrimaryOnly { name } = self {
            return if path.is_symlink() {
                let target = read_link_full(&path);
                if target == format!("{SYMLINKS_READ}/{name}") {
                    Self::PrimaryAndReadOnly { name }
                } else {
                    let is_orphan = !exists(&path);

                    Self::PrimaryAndReadAndOrWriteIncorrect {
                        name,
                        kind: ReadAndOrWriteIncorrectKind::PrimaryAndReadIncorrect {
                            read: SecondaryIncorrectKind::OrphanOrDifferentSymlink {
                                target,
                                is_orphan,
                            },
                            write: None,
                        },
                    }
                }
            } else {
                Self::PrimaryAndReadAndOrWriteIncorrect {
                    name,
                    kind: ReadAndOrWriteIncorrectKind::PrimaryAndReadIncorrect {
                        read: SecondaryIncorrectKind::NonSymlink {
                            is_dir: path.is_dir(),
                        },
                        write: None,
                    },
                }
            };
        }
        panic!(
            "Expected variant PrimaryOnly, but called on variant {:?}.",
            self
        );
    }

    fn _new_under_symlinks(path: PathBuf, is_read: bool) -> Self {
        let name = file_name_leaf(&path);

        if path.is_symlink() {
            let target = read_link_full(&path);
            let is_orphan = exists(&path);
            Self::SecondaryIncorrect {
                name,
                is_read,
                kind: SecondaryIncorrectKind::OrphanOrDifferentSymlink { target, is_orphan },
            }
        } else {
            let is_dir = path.is_dir();
            Self::SecondaryIncorrect {
                name,
                is_read,
                kind: SecondaryIncorrectKind::NonSymlink { is_dir },
            }
        }
    }

    fn new_under_readable_symlinks(path: PathBuf) -> Self {
        Self::_new_under_symlinks(path, true)
    }

    fn and_writable_symlink(self, entry: DirEntry) -> Self {
        // @TODO hash!!!!:
        let write_name = self.name().clone().to_owned();
        let path = entry.path();

        if let Self::PrimaryAndReadOnly { name } = self {
            return if path.is_symlink() {
                let target = read_link_full(&path);
                if target == format!("{SYMLINKS_WRITE}/{write_name}") {
                    Self::PrimaryAndReadWrite { name, write_name }
                } else {
                    let is_orphan = !exists(&path);

                    Self::PrimaryAndReadAndOrWriteIncorrect {
                        name,
                        kind: ReadAndOrWriteIncorrectKind::PrimaryAndReadOkButWriteIncorrect {
                            write: SecondaryIncorrectKind::OrphanOrDifferentSymlink {
                                target,
                                is_orphan,
                            },
                            write_name,
                        },
                    }
                }
            } else {
                Self::PrimaryAndReadAndOrWriteIncorrect {
                    name,
                    kind: ReadAndOrWriteIncorrectKind::PrimaryAndReadOkButWriteIncorrect {
                        write_name,
                        write: SecondaryIncorrectKind::NonSymlink {
                            is_dir: path.is_dir(),
                        },
                    },
                }
            };
        } else if let Self::PrimaryOnly { name } = self {
            return if path.is_symlink() {
                let target = read_link_full(&path);
                if target == format!("{SYMLINKS_WRITE}/{write_name}") {
                    Self::PrimaryAndReadAndOrWriteIncorrect {
                        name,
                        kind: ReadAndOrWriteIncorrectKind::PrimaryAndWriteOnly { write_name },
                    }
                } else {
                    let is_orphan = !exists(&path);

                    Self::PrimaryAndReadAndOrWriteIncorrect {
                        name,
                        kind: ReadAndOrWriteIncorrectKind::PrimaryAndWriteOnlyAndIncorrect {
                            write_name,
                            write: SecondaryIncorrectKind::OrphanOrDifferentSymlink {
                                target,
                                is_orphan,
                            },
                        },
                    }
                }
            } else {
                Self::PrimaryAndReadAndOrWriteIncorrect {
                    name,
                    kind: ReadAndOrWriteIncorrectKind::PrimaryAndWriteOnlyAndIncorrect {
                        write_name,
                        write: SecondaryIncorrectKind::NonSymlink {
                            is_dir: path.is_dir(),
                        },
                    },
                }
            };
        }
        panic!(
            "Expected variant PrimaryAndReadOnly or PrimaryOnly, but called on variant {:?}.",
            self
        );
    }

    fn new_under_writable_symlinks(path: PathBuf) -> Self {
        Self::_new_under_symlinks(path, false)
    }
}

/// Directory entries, mapped by their (potentially lossy) names.
#[derive(Template)]
#[template(path = "admin_list.html")]
struct AdminListTemplate {
    entries: HashMap<String, Entry>,
}

// Thanks to https://blog.logrocket.com/template-rendering-in-rust
type WebResult<T> = std::result::Result<T, Rejection>;

async fn admin_list() -> WebResult<impl Reply> {
    let dirs = fs::read_dir(DIRS).map_err(|e| reject::custom(Rej(e)))?;

    let mut entries = HashMap::<String, Entry>::new();
    for dir_entry in dirs {
        match dir_entry {
            Ok(entry) => {
                let entry = Entry::new_under_dirs(entry);
                entries.insert(entry.name().to_owned(), entry);
            }
            Err(err) => return Err(reject::custom(Rej(err))),
        }
    }

    if false {
        let dir = loop {} as DirEntry;
        dir.path().as_os_str().is_ascii();
        dir.path().symlink_metadata();
        dir.path().read_link();
        dir.path().display();
        //(dir.path() as Debug).
        dir.path().to_string_lossy();
        if let Ok(metadata) = dir.metadata() {
            //metadata.is_symlink()
        }
        if let Ok(file_type) = dir.file_type() {
            if file_type.is_dir() {}
        }
    }

    let template = AdminListTemplate { entries };
    let res = template.render().map_err(|e| reject::custom(Rej(e)))?;
    Ok(reply::html(res))
}

async fn admin_add(dir_name: String) -> Result<impl Reply, Rejection> {
    let dir_result = fs::create_dir(format!("{DIRS}/{dir_name}"));
    if let Err(e) = dir_result {
        return Err(reject::custom(Rej(e)));
    }
    //redirect_see_other(format!("/{ADMIN}"))
    Ok(redirect::see_other(
        format!("/{ADMIN}").parse::<Uri>().expect("Admin UR"),
    ))
}

async fn admin_remove_write(dir_name: String) -> Result<impl Reply, Rejection> {
    let dir_result = fs::create_dir(format!("{DIRS}/{dir_name}"));
    if let Err(e) = dir_result {
        return Err(reject::custom(Rej(e)));
    }
    // @TODO replace with redirect::see_other(...):
    redirect_see_other(format!("/{ADMIN}"))
}

#[tokio::main]
async fn main() -> io::Result<()> {
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
