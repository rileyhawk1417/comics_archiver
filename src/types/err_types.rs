use std::io;

// Custom Error type
#[derive(Debug)]
pub enum CompressionError {
    IoError(io::Error),
    UnsupportedFileType,
    WalkDirError(walkdir::Error),
}
