use crate::err_impl::CompressionError;
use image::ImageOutputFormat;
use indicatif::{ProgressBar, ProgressStyle};
use std::io::{self, Cursor, Read, Write};
use std::path::{Path, PathBuf};
use tokio::fs::File as AsyncFile;
use zip::{write::FileOptions, CompressionMethod, ZipArchive, ZipWriter};

/// Extract files from `.cbz` archive.
///
/// Return file names and attach them together with path
/// * `cbz_file`: `.cbz file`
/// Return `<Vec(Vec<u8>, PathBuf)>` | (file_data, file_path)
pub async fn extract_from_cbz<P1: AsRef<Path>>(
    cbz_file: P1,
    //cbz_file: Arc<impl AsRef<Path> + Send + Sync>,
) -> io::Result<Vec<(String, Vec<u8>, PathBuf)>> {
    let mut entries = Vec::new();
    let file = match AsyncFile::open(cbz_file.as_ref()).await {
        Ok(f) => f,
        Err(e) => return Err(e),
    };
    let file = file.into_std().await;
    let file = io::BufReader::new(file);

    let mut zip_file = ZipArchive::new(file)?;

    println!("Unpacking: {}", cbz_file.as_ref().to_str().unwrap());
    for idx in 0..zip_file.len() {
        let mut inner_file = zip_file.by_index(idx)?;
        let file_name = inner_file.name().to_owned();
        let file_path = Path::new(&file_name);
        if file_path.is_dir() {
            continue;
        }
        let mut file_contents = Vec::new();
        inner_file.read_to_end(&mut file_contents)?;
        let archive_file_name = cbz_file
            .as_ref()
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .to_string();
        entries.push((archive_file_name, file_contents, file_path.to_owned()));
    }
    println!(
        "Fetched files from: {}",
        cbz_file.as_ref().to_str().unwrap()
    );
    Ok(entries)
}

/// Compress Image with `image` crate.
///
/// Compress image into a new file with 90% quality on Jpeg format.
/// * `image_data` - Vec<u8> image data.
/// Return `Vec<u8>` compressed image data
pub fn img_compressor(image_data: Vec<u8>) -> Result<Vec<u8>, CompressionError> {
    let spinner = ProgressBar::new_spinner();
    spinner.set_style(
        ProgressStyle::with_template("Image compression: {spinner}")
            .unwrap()
            .progress_chars("#>-"),
    );
    let mut compressed_data = Vec::new();
    let img = image::load_from_memory(&image_data).expect("Failed to load image!");
    img.write_to(
        &mut Cursor::new(&mut compressed_data),
        ImageOutputFormat::Jpeg(90),
    )
    .expect("Failed to compress image!");
    spinner.finish_and_clear();
    Ok(compressed_data)
}

/// Compress directory and files to `.cbz` archive.
///
/// * `file_contents`: `Vec<(Vec<u8>, PathBuf)>`
/// file_contents = (file_data, file_path)
/// Return `Vec<u8>>` zip archive.
pub fn compress_to_cbz(
    file_contents: Vec<(&String, Vec<u8>, &PathBuf)>,
) -> io::Result<(String, Vec<u8>)> {
    let mut zip_buffer = Vec::new();
    let mut archive_name: String = String::new();
    if let Some(file_name) = file_contents.first() {
        archive_name = file_name.0.to_string();
    }

    {
        let mut zip_writer = ZipWriter::new(Cursor::new(&mut zip_buffer));
        //NOTE: Can't copy type `File` directly from one to another.
        let options = FileOptions::default()
            .compression_method(CompressionMethod::Deflated)
            .compression_level(Some(9))
            .unix_permissions(0o755);
        for file_path in &file_contents {
            zip_writer
                .start_file(file_path.2.to_owned().to_string_lossy(), options)
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
            let _ = zip_writer.write_all(&file_path.1);
        }
    }
    //NOTE: Adds another layer of compression not necessary.
    // NOTE: Will probably remove this later.
    /*
        let mut lzma_compressed = Vec::new();
        {
            let mut encoder = XzEncoder::new(zip_buffer.as_slice(), 9);
            encoder.read_to_end(&mut lzma_compressed)?;
        }
    */
    println!("Repack done for: {}!", archive_name);
    Ok((archive_name.to_string(), zip_buffer))
}
