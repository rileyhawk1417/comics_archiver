use image::codecs::jpeg::JpegEncoder;
use image::{DynamicImage, GenericImageView, ImageFormat, ImageOutputFormat};
use indicatif::ProgressBar;
use liblzma::bufread::XzEncoder;
use liblzma::write::XzDecoder;
use rayon::prelude::*;
use std::io::{self, Cursor, Read, Write};
use std::path::{Path, PathBuf};
use tokio::fs::File as AsyncFile;
use tokio::io::{AsyncBufRead, AsyncRead, AsyncReadExt, AsyncSeekExt, BufReader as AsyncBufReader};
use zip::{write::FileOptions, CompressionMethod, ZipArchive, ZipWriter};

/// Extract files from `.cbz` archive.
///
/// Extract files and attach them together with path
/// * `cbz_file`: `.cbz file`
/// Return `<Vec(Vec<u8>, PathBuf)>` | (file_data, file_path)
pub async fn extract_dir_and_files_from_cbz<P1: AsRef<Path>>(
    cbz_file: P1,
) -> io::Result<Vec<(String, Vec<u8>, PathBuf)>> {
    let mut entries = Vec::new();
    let file = match AsyncFile::open(cbz_file.as_ref()).await {
        Ok(f) => f,
        Err(e) => return Err(e),
    };
    let file = file.into_std().await;
    let file = io::BufReader::new(file);

    let mut zip_file = ZipArchive::new(file)?;
    let pb = ProgressBar::new(zip_file.len() as u64);

    for idx in 0..zip_file.len() {
        let mut inner_file = zip_file.by_index(idx)?;
        let file_name = inner_file.name().to_owned();
        let file_path = Path::new(&file_name);
        if file_path.is_dir() {
            continue;
        }
        let mut file_contents = Vec::new();
        pb.inc(1);
        inner_file.read_to_end(&mut file_contents)?;
        entries.push((
            cbz_file
                .as_ref()
                .file_name()
                .to_owned()
                .unwrap()
                .to_str()
                .unwrap()
                .to_string(),
            file_contents,
            file_path.to_owned(),
        ));
    }
    Ok(entries)
}

// NOTE: This does not work well for images.
// liblzma doesnt work well for image compression.
pub fn compress_images_with_lzma(image_data: Vec<u8>) -> io::Result<Vec<u8>> {
    let mut compressed_data = Vec::new();
    let mut input_cursor = Cursor::new(image_data);
    {
        let mut encoder = liblzma::write::XzEncoder::new(&mut compressed_data, 9);
        let _ = io::copy(&mut input_cursor, &mut encoder);
        encoder.finish()?;
    }
    Ok(compressed_data)
}

/// Compress Image with `image` crate.
///
/// Compress image into a new file with 90% quality on Jpeg format.
/// * `image_data` - Vec<u8> image data.
/// Return `Vec<u8>` compressed image data
pub fn compress_images_with_img(image_data: Vec<u8>) -> io::Result<Vec<u8>> {
    let mut compressed_data = Vec::new();
    let img = image::load_from_memory(&image_data).expect("Failed to load image!");
    img.write_to(
        &mut Cursor::new(&mut compressed_data),
        ImageOutputFormat::Jpeg(90),
    )
    .expect("Failed to compress image!");
    Ok(compressed_data)
}

//NOTE: Will probably remove this later.
pub fn decompress_images_with_img(image_data: Vec<u8>) -> io::Result<Vec<u8>> {
    let mut compressed_data = Vec::new();
    let img = image::load_from_memory(&image_data).expect("Failed to load image!");
    img.write_to(
        &mut Cursor::new(&mut compressed_data),
        //NOTE: 90% still looks okay in the images.
        ImageOutputFormat::Jpeg(90),
    )
    .expect("Failed to decompress image!");
    Ok(compressed_data)
}

//NOTE: Will probably remove this later.
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

/*
        let data = match compress_dir_and_files_to_cbz(compressed_list.clone()).await {
            Ok(complete) => complete,
            Err(err) => {
                eprintln!("Error repacking files! : {}", err);
                return Err(CompressionError::IoError(err));
            }
        };
        match tokio::fs::write("packaged_zip_optimised.cbz", data).await {
            Ok(done) => done,

            Err(err) => {
                eprintln!("Error writing cbz file! : {}", err);
                return Err(CompressionError::IoError(err));
            }
        };
*/

/// Compress directory and files to `.cbz` archive.
///
/// * `file_contents`: `Vec<(Vec<u8>, PathBuf)>`
/// file_contents = (file_data, file_path)
/// Return `Vec<u8>>` zip archive.
pub fn compress_dir_and_files_to_cbz(
    file_contents: Vec<(String, Vec<u8>, PathBuf)>,
) -> io::Result<(String, Vec<u8>)> {
    let mut zip_buffer = Vec::new();
    let mut archive_name: String = String::new();
    let mut vec_length: usize = 0;
    if let Some(file_name) = file_contents.get(0) {
        vec_length = file_name.1.len();
        archive_name = file_name.0.to_string();
    }

    let pb = ProgressBar::new(vec_length as u64);
    /*
        let repacked_cbz = match std::fs::File::create(&file_contents[0].0) {
            Ok(f) => f,
            Err(err) => {
                eprintln!("Failed to create archive: {}", file_contents[0].0);
                return Err(err);
            }
        };
    */
    {
        //NOTE: Fallback to vectors if file doesnt work.
        let mut zip_writer = ZipWriter::new(Cursor::new(&mut zip_buffer));
        //NOTE: Can't copy type `File` directly from one to another.
        //let mut zip_writer = ZipWriter::new(&repacked_cbz);
        let options = FileOptions::default()
            .compression_method(CompressionMethod::Deflated)
            .compression_level(Some(9))
            .unix_permissions(0o755);
        for file_path in &file_contents {
            zip_writer
                .start_file(file_path.2.to_owned().to_string_lossy(), options)
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
            let _ = zip_writer.write_all(&file_path.1);
            pb.inc(1);
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
    //zip_buffer.push(repacked_cbz);
    pb.finish_with_message("Done repacking archives!");
    Ok((archive_name.to_string(), zip_buffer))
}
