use crate::entry::{EntriesMap, Entry};
#[cfg(not(feature = "mock_fs"))]
use crate::{DIRS, SYMLINKS_READ, SYMLINKS_WRITE};
#[cfg(not(feature = "mock_fs"))]
use std::fs as std_fs;
use std::io;
use std::path::{Path, PathBuf};

pub struct FileSystem {}

/// Functions that we implement for [FileSystem] but we don't neeed/want to mock them. Hence, we'll
/// have same implementation for production (no mock) and for tests (mock).
pub trait UnmockFileSystem {
    fn get_entries(&self) -> io::Result<EntriesMap>;
}

// #[cfg(not(feature = "mock_fs"))]
#[cfg_attr(feature = "mock_fs", mockall::automock)]
impl FileSystem {
    /// Return the target - but as-is, NOT canonical!
    ///
    /// This function could be generic, like `read_link_full<P: AsRef<Path>>(path: P)`. However,
    /// https://docs.rs/mockall/latest/mockall/#generic-methods would then require the generic type
    /// to be `'static`!
    pub fn read_link_full(&self, path: &PathBuf) -> String {
        let link = std_fs::read_link(path).expect("Expecting {path} to be a symlink.");
        link.as_os_str().to_string_lossy().to_string()
    }

    pub fn exists(&self, path: &Path) -> bool {
        let target_exists = path.try_exists();
        matches!(target_exists, Ok(true))
    }

    pub fn get_primaries(&self) -> io::Result<EntriesMap> {
        let dirs = std_fs::read_dir(DIRS)?;

        let mut entries = EntriesMap::new();
        for dir_entry in dirs {
            let entry = Entry::new_under_dirs(dir_entry?.path());
            entries.insert(entry.name().to_owned(), entry);
        }
        Ok(entries)
    }

    /// Call on result of [get_primaries].
    pub fn get_secondaries_read(&self, mut primaries: EntriesMap) -> io::Result<EntriesMap> {
        let secondaries = std_fs::read_dir(SYMLINKS_READ)?;
        let mut entries = EntriesMap::new();

        for secondary in secondaries {
            let path = secondary?.path();
            let name = crate::fs::file_name_leaf(&path);

            let primary = primaries.remove(&name);
            let new_entry = if let Some(primary) = primary {
                primary.and_readable_symlink(self, path)
            } else {
                Entry::new_under_readable_symlinks(&path)
            };

            entries.insert(name, new_entry);
        }
        Ok(entries)
    }

    /// Call on result of [get_secondaries_read].
    fn get_secondaries_write(&self, mut secondaries_read: EntriesMap) -> io::Result<EntriesMap> {
        let secondaries_write = std_fs::read_dir(SYMLINKS_WRITE)?;
        let mut entries = EntriesMap::new();

        for secondary_write in secondaries_write {
            let path = secondary_write?.path();
            let name = crate::fs::file_name_leaf(&path);

            let secondary_read = secondaries_read.remove(&name);
            let new_entry = if let Some(secondary_read) = secondary_read {
                secondary_read.and_writable_symlink(path)
            } else {
                Entry::new_under_writable_symlinks(&path)
            };

            entries.insert(name, new_entry);
        }
        Ok(entries)
    }
}

/*#[cfg(feature = "mock_fs")]
mockall::mock! {
    pub FileSystem {
        pub fn read_link_full(&self, path: &PathBuf) -> String;
        pub fn exists(&self, path: &Path) -> bool;
        pub fn get_primaries(&self) -> io::Result<EntriesMap>;
        pub fn get_secondaries_read(&self, mut primaries: EntriesMap) -> io::Result<EntriesMap>;
        fn get_secondaries_write(&self, mut secondaries_read: EntriesMap) -> io::Result<EntriesMap>;
    }
}*/

mod unmock {
    #[cfg_attr(feature = "mock_fs", mockall_double::double)]
    use super::FileSystem;

    use super::UnmockFileSystem;
    use crate::entry::EntriesMap;
    use std::io;

    impl UnmockFileSystem for FileSystem {
        fn get_entries(&self) -> io::Result<EntriesMap> {
            let primaries = self.get_primaries()?;
            let secondaries_read = self.get_secondaries_read(primaries)?;
            self.get_secondaries_write(secondaries_read)
        }
    }
}
