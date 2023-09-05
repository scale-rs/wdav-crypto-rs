#[cfg_attr(test, double)]
pub(crate) use fs_mockable::FileSystem;
#[cfg(test)]
use mockall_double::double;
use std::path::Path;

mod fs_mockable;

/// Require `path` leaf part not to be `..`.
fn file_name_leaf(path: &Path) -> String {
    path.file_name()
        .expect("The path must not be `..`")
        .to_string_lossy()
        .to_string()
}
