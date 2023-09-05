use super::{ReadAndOrWriteIncorrectKind, SecondaryIncorrectKind};
use crate::fs::FileSystem;
use crate::{SYMLINKS_READ, SYMLINKS_WRITE};
#[cfg(test)]
use mockall::automock;
use std::path::PathBuf;

/// This is private, so as to hide the implementation (enum variants) and make it future-proof.
#[derive(Debug)]
enum EntryImpl {
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

/// Directory entry immediately below either [DIRS], and/or [SYMLINKS_READ] and/or [SYMLINKS_WRITE].
#[repr(transparent)]
#[derive(Debug)]
pub struct Entry {
    entry_impl: EntryImpl,
}

// Can't use `#[cfg_attr(test, automock)]`, because when `fs::fs_mockable` calls
// `and_readable_symlink`, it would pass `FileSystem` instead of `MockFileSystem`.
#[cfg_attr(test, automock)]
impl Entry {
    fn new(entry_impl: EntryImpl) -> Self {
        Self { entry_impl }
    }
    pub(crate) fn is_ok_and_complete(&self) -> bool {
        match self.entry_impl {
            EntryImpl::PrimaryAndReadOnly { .. } | EntryImpl::PrimaryAndReadWrite { .. } => true,
            _ => false,
        }
    }
    pub(crate) fn is_readable(&self) -> bool {
        self.is_ok_and_complete()
    }
    pub(crate) fn is_writable(&self) -> bool {
        matches!(self.entry_impl, EntryImpl::PrimaryAndReadWrite { .. })
    }
    pub(crate) fn name(&self) -> &str {
        match &self.entry_impl {
            EntryImpl::PrimaryOnly { name }
            | EntryImpl::PrimaryAndReadOnly { name }
            | EntryImpl::PrimaryAndReadWrite { name, .. }
            | EntryImpl::PrimaryAndReadAndOrWriteIncorrect { name, .. }
            | EntryImpl::PrimaryNonDir { name, .. }
            | EntryImpl::SecondaryIncorrect { name, .. } => &name,
        }
    }
    pub(crate) fn write_name(&self) -> &str {
        match &self.entry_impl {
            EntryImpl::PrimaryAndReadWrite {
                name: _,
                write_name: write,
            } => &write,
            _ => unreachable!(
                "Can be called only on ReadWrite variant, but it was invoked on {:?}.",
                self
            ),
        }
    }

    pub(crate) fn new_under_dirs(path: PathBuf) -> Self {
        let name = path.to_string_lossy().to_string();
        if path.is_dir() {
            Self::new(EntryImpl::PrimaryOnly { name })
        } else {
            Self::new(EntryImpl::PrimaryNonDir { name, path })
        }
    }

    pub(crate) fn and_readable_symlink(self, fs: &FileSystem, path: PathBuf) -> Self {
        if let EntryImpl::PrimaryOnly { name } = self.entry_impl {
            return Self::new(if path.is_symlink() {
                let target = fs.read_link_full(&path);
                if target == format!("{SYMLINKS_READ}/{name}") {
                    EntryImpl::PrimaryAndReadOnly { name }
                } else {
                    let is_orphan = !fs.exists(&path);

                    EntryImpl::PrimaryAndReadAndOrWriteIncorrect {
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
                EntryImpl::PrimaryAndReadAndOrWriteIncorrect {
                    name,
                    kind: ReadAndOrWriteIncorrectKind::PrimaryAndReadIncorrect {
                        read: SecondaryIncorrectKind::NonSymlink {
                            is_dir: path.is_dir(),
                        },
                        write: None,
                    },
                }
            });
        }
        panic!(
            "Expected variant PrimaryOnly, but called on variant {:?}.",
            self
        );
    }

    fn _new_under_symlinks(path: &PathBuf, fs: &FileSystem, is_read: bool) -> Self {
        let name = super::file_name_leaf(path);

        Self::new(if path.is_symlink() {
            let target = fs.read_link_full(path);
            let is_orphan = fs.exists(path);
            EntryImpl::SecondaryIncorrect {
                name,
                is_read,
                kind: SecondaryIncorrectKind::OrphanOrDifferentSymlink { target, is_orphan },
            }
        } else {
            let is_dir = path.is_dir();
            EntryImpl::SecondaryIncorrect {
                name,
                is_read,
                kind: SecondaryIncorrectKind::NonSymlink { is_dir },
            }
        })
    }

    pub(crate) fn new_under_readable_symlinks(path: &PathBuf) -> Self {
        let fs = loop {};
        Self::_new_under_symlinks(path, &fs, true)
    }

    pub(crate) fn and_writable_symlink(self, path: PathBuf) -> Self {
        let fs: FileSystem = loop {};
        // @TODO hash!!!!:
        let write_name = self.name().to_owned();

        if let EntryImpl::PrimaryAndReadOnly { name } = self.entry_impl {
            return Self::new(if path.is_symlink() {
                let target = fs.read_link_full(&path);
                if target == format!("{SYMLINKS_WRITE}/{write_name}") {
                    EntryImpl::PrimaryAndReadWrite { name, write_name }
                } else {
                    let is_orphan = !fs.exists(&path);

                    EntryImpl::PrimaryAndReadAndOrWriteIncorrect {
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
                EntryImpl::PrimaryAndReadAndOrWriteIncorrect {
                    name,
                    kind: ReadAndOrWriteIncorrectKind::PrimaryAndReadOkButWriteIncorrect {
                        write_name,
                        write: SecondaryIncorrectKind::NonSymlink {
                            is_dir: path.is_dir(),
                        },
                    },
                }
            });
        } else if let EntryImpl::PrimaryOnly { name } = self.entry_impl {
            return Self::new(if path.is_symlink() {
                let target = fs.read_link_full(&path);
                if target == format!("{SYMLINKS_WRITE}/{write_name}") {
                    EntryImpl::PrimaryAndReadAndOrWriteIncorrect {
                        name,
                        kind: ReadAndOrWriteIncorrectKind::PrimaryAndWriteOnly { write_name },
                    }
                } else {
                    let is_orphan = !fs.exists(&path);

                    EntryImpl::PrimaryAndReadAndOrWriteIncorrect {
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
                EntryImpl::PrimaryAndReadAndOrWriteIncorrect {
                    name,
                    kind: ReadAndOrWriteIncorrectKind::PrimaryAndWriteOnlyAndIncorrect {
                        write_name,
                        write: SecondaryIncorrectKind::NonSymlink {
                            is_dir: path.is_dir(),
                        },
                    },
                }
            });
        }
        panic!(
            "Expected variant PrimaryAndReadOnly or PrimaryOnly, but called on variant {:?}.",
            self
        );
    }

    pub(crate) fn new_under_writable_symlinks(path: &PathBuf) -> Self {
        let fs = loop {};
        Self::_new_under_symlinks(path, &fs, false)
    }
}
