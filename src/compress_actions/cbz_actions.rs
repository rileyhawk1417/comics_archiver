use liblzma::write::{XzDecoder, XzEncoder};
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
pub async fn extract_dir_and_files_from_cbz<P1: AsRef<Path>>(
    cbz_file: P1,
) -> io::Result<Vec<(Vec<u8>, PathBuf)>> {
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
        //NOTE: To verify the files are being printed;
        //println!("{:?}: {}", file_name, file_path.to_str().unwrap());
        entries.push((file_contents, file_path.to_owned()));
    }
    Ok(entries)
}

pub async fn compress_images_with_lzma(image_data: Vec<u8>) -> io::Result<Vec<u8>> {
    let mut compressed_data = Vec::new();
    let mut input_cursor = Cursor::new(image_data);
    {
        let mut encoder = XzEncoder::new(&mut compressed_data, 9);
        let _ = io::copy(&mut input_cursor, &mut encoder);
        encoder.finish()?;
    }
    Ok(compressed_data)
}

pub async fn decompress_images_with_lzma(image_data: Vec<u8>) -> io::Result<Vec<u8>> {
    let mut decompressed_data = Vec::new();
    let mut input_cursor = Cursor::new(image_data);
    {
        let mut decoder = XzDecoder::new(&mut decompressed_data);
        let _ = io::copy(&mut input_cursor, &mut decoder);
        decoder.finish()?;
    }
    Ok(decompressed_data)
}

pub async fn compress_dir_and_files_to_cbz(
    file_contents: Vec<(Vec<u8>, PathBuf)>,
) -> io::Result<Vec<u8>> {
    let mut zip_buffer = Vec::new();
    {
        let mut zip_writer = ZipWriter::new(Cursor::new(&mut zip_buffer));
        let options = FileOptions::default()
            .compression_method(CompressionMethod::Stored)
            .unix_permissions(0o644);
        for (file, file_path) in file_contents {
            let file_name = file_path.to_str().unwrap();
            let mut file = zip_writer
                .start_file(file_path, options)
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        }
    }
    Ok(zip_buffer)
}
