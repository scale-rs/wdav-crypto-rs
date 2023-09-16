#[cfg_attr(feature = "mock_fs", mockall_double::double)]
pub use fs_mockable::FileSystem;
//#[cfg(test)]
//pub use fs_mockable::FileSystem as ProductionFileSystem;

pub use fs_mockable::UnmockFileSystem;

use std::path::Path;

mod fs_mockable;

/// Require `path` leaf part not to be `..`.
fn file_name_leaf(path: &Path) -> String {
    path.file_name()
        .expect("The path must not be `..`")
        .to_string_lossy()
        .to_string()
}
