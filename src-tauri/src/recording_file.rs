use objc2::rc::Retained;
use objc2_foundation::{NSError, NSFileManager, NSString, NSURL};
use std::path::Path;

fn url(path: &Path) -> Retained<NSURL> {
    NSURL::fileURLWithPath(&NSString::from_str(&path.to_string_lossy()))
}

fn reason(error: Retained<NSError>) -> &'static str {
    match error.code() {
        4 => "not_found",
        516 => "exists",
        _ => "failed",
    }
}

pub fn trash(path: &Path) -> Result<(), &'static str> {
    NSFileManager::defaultManager().trashItemAtURL_resultingItemURL_error(&url(path), None).map_err(reason)
}

pub fn rename(from: &Path, to: &Path) -> Result<(), &'static str> {
    NSFileManager::defaultManager().moveItemAtURL_toURL_error(&url(from), &url(to)).map_err(reason)
}
