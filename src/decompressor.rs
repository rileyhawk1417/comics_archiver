use std::error::Error;
use std::fs::{self, File};
use std::io::{self, BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::process;

use walkdir::WalkDir;
use xz2::read::XzDecoder;

#[derive(Debug)]
enum DecompressionError {
    IoError(io::Error),
    InvalidMetadata,
}

impl From<io::Error> for DecompressionError {
    fn from(err: io::Error) -> Self {
        DecompressionError::IoError(err)
    }
}

fn decompress_cbz_file<P: AsRef<Path>>(
    compressed_file_path: P,
    output_dir: P,
) -> Result<(), DecompressionError> {
    let compressed_file = File::open(&compressed_file_path)?;
    let mut decoder = XzDecoder::new(BufReader::new(compressed_file));

    // Read compressed data and metadata
    let mut buffer = String::new();
    decoder.read_to_string(&mut buffer)?;

    // Split by separator (e.g., newline) to separate metadata and compressed data
    let parts: Vec<&str> = buffer.split('\n').collect();

    for part in parts {
        // Parse metadata (replace with your own parsing logic)
        let metadata_parts: Vec<&str> = part.split(':').collect();
        if metadata_parts.len() == 2 {
            let file_name = metadata_parts[0];
            let file_size: usize = metadata_parts[1]
                .parse()
                .map_err(|_| DecompressionError::InvalidMetadata)?;

            // Create the output file with the original file name
            let output_file_path = output_dir.as_ref().join(file_name);
            let mut output_file = File::create(output_file_path)?;

            // Read and write the compressed data for each file
            io::copy(&mut decoder, &mut output_file)?;
        }
    }

    Ok(())
}

fn main() {
    let compressed_file_path = Path::new("output.cbz.xz");
    let output_dir = Path::new("output_directory");

    match decompress_cbz_file(&compressed_file_path, &output_dir) {
        Ok(()) => {
            println!("Decompression successful!");
        }
        Err(err) => {
            eprintln!("Error: {:?}", err);
            process::exit(1);
        }
    }
}
