use crate::{SYMLINKS_READ, SYMLINKS_WRITE};
use std::fs::{self, DirEntry};
use std::{path::Path, path::PathBuf};

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
pub enum SecondaryIncorrectKind {
    OrphanOrDifferentSymlink { target: String, is_orphan: bool },
    NonSymlink { is_dir: bool },
}

pub type WriteNameAndKind = (
    String, /*write_name*/
    Result<(), SecondaryIncorrectKind>,
);

#[derive(Debug)]
pub enum ReadAndOrWriteIncorrectKind {
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
pub enum Entry {
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
    pub fn is_ok_and_complete(&self) -> bool {
        match self {
            Self::PrimaryAndReadOnly { .. } | Self::PrimaryAndReadWrite { .. } => true,
            _ => false,
        }
    }
    pub fn is_readable(&self) -> bool {
        self.is_ok_and_complete()
    }
    pub fn is_writable(&self) -> bool {
        matches!(self, Self::PrimaryAndReadWrite { .. })
    }
    pub fn name(&self) -> &str {
        match self {
            Self::PrimaryOnly { name }
            | Self::PrimaryAndReadOnly { name }
            | Self::PrimaryAndReadWrite { name, .. }
            | Self::PrimaryAndReadAndOrWriteIncorrect { name, .. }
            | Self::PrimaryNonDir { name, .. }
            | Self::SecondaryIncorrect { name, .. } => &name,
        }
    }
    pub fn write_name(&self) -> &str {
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

    pub fn new_under_dirs(entry: DirEntry) -> Self {
        let path = entry.path();
        let name = path.to_string_lossy().to_string();
        if path.is_dir() {
            Self::PrimaryOnly { name }
        } else {
            Self::PrimaryNonDir { name, path }
        }
    }

    pub fn and_readable_symlink(self, entry: DirEntry) -> Self {
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

    pub fn _new_under_symlinks(path: PathBuf, is_read: bool) -> Self {
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

    pub fn new_under_readable_symlinks(path: PathBuf) -> Self {
        Self::_new_under_symlinks(path, true)
    }

    pub fn and_writable_symlink(self, entry: DirEntry) -> Self {
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

    pub fn new_under_writable_symlinks(path: PathBuf) -> Self {
        Self::_new_under_symlinks(path, false)
    }
}
