use rayon::prelude::*;
use std::io::{self, BufReader, BufWriter, Cursor, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use tokio::fs::File as AsyncFile;
use tokio::io::{AsyncBufRead, AsyncRead, AsyncReadExt, AsyncSeekExt, BufReader as AsyncBufReader};
use tokio::runtime::Runtime;
use zip::{write::FileOptions, CompressionMethod, ZipArchive, ZipWriter};

struct AsyncBufReaderAdapter(tokio::fs::File);

/// Adapter to satisfy BufReader from tokio
/// Since ZipArchive doesn't implement AsyncReadExt
impl Read for AsyncBufReaderAdapter {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let rt = Runtime::new().unwrap();
        let res: io::Result<usize> = rt.block_on(async move {
            let mut buf_read = AsyncBufReader::new(&mut self.0);
            let mut data = vec![0; buf.len()];
            let bytes_read: usize = buf_read.read(&mut data).await?;
            buf[..bytes_read].copy_from_slice(&data[..bytes_read]);
            Ok(bytes_read)
        });
        match res {
            Ok(bytes_read) => Ok(bytes_read),
            Err(err) => Err(Into::into(err)),
        }
    }
}
//BUG: Seek is struggling with rt block on
/// Adapter to satisfy BufReader from tokio
/// Since ZipArchive doesn't implement AsyncSeekExt
impl Seek for AsyncBufReaderAdapter {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        let rt = Runtime::new().unwrap();
        let result = rt.block_on(async {
            return self.0.seek(pos).await;
        });
        match result {
            Ok(bytes_read) => Ok(bytes_read),
            Err(err) => Err(Into::into(err)),
        }
    }
}

/// Extract files from `.cbz` archive.
/// Then return archive entries.
/// * `cbz_file`: `.cbz file`
pub async fn extract_dir_and_files_from_cbz(cbz_file: &str) -> io::Result<Vec<(Vec<u8>, PathBuf)>> {
    let mut entries = Vec::new();
    let file = AsyncFile::open(cbz_file).await?;
    let mut zip_archive_adapter = AsyncBufReaderAdapter(file);
    let mut zip_file = ZipArchive::new(&mut zip_archive_adapter).unwrap();

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
