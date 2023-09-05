use crate::{DIRS, SYMLINKS_READ, SYMLINKS_WRITE};
#[cfg_attr(test, double)]
pub use entry_mockable::Entry;
#[cfg(test)]
use mockall_double::double;
use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::Path;

mod entry_mockable;

/// Require `path` leaf part not to be `..`.
fn file_name_leaf(path: &Path) -> String {
    assert!(false);
    path.file_name()
        .expect("The path must not be `..`")
        .to_string_lossy()
        .to_string()
}

#[derive(Debug)]
pub(crate) enum SecondaryIncorrectKind {
    OrphanOrDifferentSymlink { target: String, is_orphan: bool },
    NonSymlink { is_dir: bool },
}

pub(crate) type WriteNameAndKind = (
    String, /*write_name*/
    Result<(), SecondaryIncorrectKind>,
);

#[derive(Debug)]
pub(crate) enum ReadAndOrWriteIncorrectKind {
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

pub(crate) type EntriesMap = HashMap<String, Entry>;
