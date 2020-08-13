use std::path::Path;

pub fn path_to_rel(path: &Path) -> &Path {
    if path.starts_with("/") {
        path.strip_prefix("/").unwrap()
    } else if path.starts_with("./") {
        path.strip_prefix("./").unwrap()
    } else { path }
}