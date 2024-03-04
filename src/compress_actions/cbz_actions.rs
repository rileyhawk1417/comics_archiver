use rayon::prelude::*;
use std::io::{self, BufReader, BufWriter, Cursor, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use tokio::fs::File as AsyncFile;
use tokio::io::{AsyncBufRead, AsyncRead, AsyncReadExt, AsyncSeekExt, BufReader as AsyncBufReader};
use tokio::runtime::Runtime;
use zip::{write::FileOptions, CompressionMethod, ZipArchive, ZipWriter};

/// Extract files from `.cbz` archive.
/// Then return archive entries.
/// * `cbz_file`: `.cbz file`
pub async fn extract_dir_and_files_from_cbz(cbz_file: &str) -> io::Result<Vec<(Vec<u8>, PathBuf)>> {
    let mut entries = Vec::new();
    let file = match AsyncFile::open(cbz_file.as_ref()).await {
        Ok(f) => f,
        Err(e) => return Err(e),
    };
    let file = file.into_std().await;
    let file = io::BufReader::new(file);

    let mut zip_file = ZipArchive::new(file)?;

    for idx in 0..zip_file.len() {
        let mut inner_file = zip_file.by_index(idx)?;
        let file_name = inner_file.name().to_owned();
        let file_path = Path::new(&file_name);
        if file_path.is_dir() {
            continue;
        }
        let mut file_contents = Vec::new();
        inner_file.read_to_end(&mut file_contents)?;
        entries.push((file_contents, file_path.to_owned()));
    }
    Ok(entries)
}
