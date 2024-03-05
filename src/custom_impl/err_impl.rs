use std::fmt;
use std::io;

#[derive(Debug)]
pub enum CompressionError {
    IoError(io::Error),
    UnsupportedFileType,
    WalkDirError(walkdir::Error),
}

impl From<io::Error> for CompressionError {
    fn from(err: io::Error) -> Self {
        CompressionError::IoError(err)
    }
}

impl From<walkdir::Error> for CompressionError {
    fn from(err: walkdir::Error) -> Self {
        CompressionError::WalkDirError(err)
    }
}

impl fmt::Display for CompressionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CompressionError::IoError(err) => write!(f, "I/O Error: {}", err),
            CompressionError::UnsupportedFileType => write!(f, "Unsupported File Type!"),
            CompressionError::WalkDirError(err) => write!(f, "Failed to find directory: {}", err),
        }
    }
}
