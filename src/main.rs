use clap::{arg, Parser};
use std::error::Error;
use std::fmt;
use std::fs::File;
use std::io::{self, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process;
use walkdir::{DirEntry, WalkDir};
use xz2::write::XzEncoder;

#[derive(Parser, Debug)]
#[command(
    name = "Comic Archiver",
    version = "0.1.0",
    about = "Archiver to compress cbz files",
    long_about = "Compress your manga .cbz files with max settings"
)]
struct Args {
    #[arg(short, long)]
    #[arg(short, long)]
    input_dir: String,

    #[arg(short, long)]
    output_file: String,
}
// Custom Error type
#[derive(Debug)]
enum CompressionError {
    IoError(io::Error),
    UnsupportedFileType,
    WalkDirError(walkdir::Error),
}

// Define implementation
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

/// Define compress worker
fn compress_worker<P2: AsRef<Path>>(
    source_path: DirEntry,
    out_file: P2,
) -> Result<u64, CompressionError> {
    let input_dir = File::open(source_path.path());
    let mut in_file = match input_dir {
        Ok(in_file) => in_file,
        Err(err) => {
            eprintln!("Failed due to: {}", err);
            return Err(CompressionError::IoError(err));
        }
    };
    let output_file = File::create(&out_file);
    let out = match output_file {
        Ok(out) => out,
        Err(err) => {
            eprintln!("Failed to create output file: {}", err);
            return Err(CompressionError::IoError(err));
        }
    };

    let mut encoder = XzEncoder::new(out, 9);
    let meta = format!(
        "{}:{}\n",
        source_path.path().file_name().unwrap().to_str().unwrap(),
        source_path.metadata()?.len()
    );
    let _ = encoder.write_all(meta.as_bytes());
    io::copy(&mut in_file, &mut encoder)?;
    encoder.try_finish()?;
    Ok(encoder.total_out())
}

//TODO: Move this to another file then write the decompression logic.
/// Define compress action
/// Compress the given files, then return
/// the list of included files for compression & output file size.
/// * `dir_path`: Directory with cbz files.
/// * `output_file`: Name of output file.
fn compress_action<P1: AsRef<Path>, P2: AsRef<Path>>(
    dir_path: P1,
    output_file: P2,
) -> Result<(Vec<PathBuf>, u64), CompressionError> {
    let mut compressed_list: Vec<PathBuf> = Vec::new();
    let compressed_size: u64;
    let out_file = match File::create(output_file) {
        Ok(out) => out,
        Err(err) => {
            eprintln!("Fail!");
            return Err(CompressionError::IoError(err));
        }
    };

    let mut encoder = XzEncoder::new(out_file, 9);
    for entry in WalkDir::new(dir_path).into_iter().filter_map(|e| e.ok()) {
        /*
        * NOTE: This only compresses individual files not grabbing everything then compressing.
        if val.file_type().is_file() && val.path().extension().map_or(false, |ext| ext == "rs") {
            let file = val.path().to_path_buf();
            compressed_size = compress_worker(val, &output_file)?;
            compressed_list.push(file);
        }
        */

        if let Some(ext) = entry.path().extension() {
            if ext == "cbz" {
                let file = File::open(entry.path());
                compressed_list.push(entry.path().to_path_buf());
                let curr_file = match file {
                    Ok(curr_file) => curr_file,
                    Err(err) => {
                        eprintln!("Failed to create file: {}", err);
                        return Err(CompressionError::IoError(err));
                    }
                };
                let metadata = format!(
                    "{}:{}\n",
                    entry.path().file_name().unwrap().to_str().unwrap(),
                    entry.metadata()?.len()
                );
                encoder.write_all(metadata.as_bytes())?;
                io::copy(&mut io::BufReader::new(curr_file), &mut encoder)?;
            }
        }
    }
    encoder.try_finish()?;
    compressed_size = encoder.total_out();
    Ok((compressed_list, compressed_size))
}

fn main() {
    let args = Args::parse();
    let output_file = args.output_file.clone();
    match compress_action(args.input_dir, args.output_file) {
        Ok(compressed) => {
            println!("Compression done for: ");
            for file in compressed.0 {
                println!("{}", file.display());
            }
            println!("New compressed file name: {}", output_file);
            println!("New file size: {}", compressed.1);
        }
        Err(CompressionError::IoError(err)) => {
            eprintln!("I/O Error: {}", err);
            process::exit(1);
        }

        Err(CompressionError::UnsupportedFileType) => {
            eprintln!("Unsupported File Type only CBZ files are supported");
            process::exit(1);
        }

        Err(CompressionError::WalkDirError(err)) => {
            eprintln!("Failed to find files in directory: {}", err);
            process::exit(1);
        }
    }
}

// This function works fine but hangs since it doesnt exit
fn main_working() -> io::Result<()> {
    let args = Args::parse();
    let out_file_path = args.output_file;
    let output_file = File::create(out_file_path);
    let out = match output_file {
        Ok(out) => out,
        Err(err) => {
            eprintln!("Error opening stream: {}", err);
            return Err(err);
        }
    };

    let mut xz_encoder = XzEncoder::new(out, 9);

    for entry in WalkDir::new(args.input_dir)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if let Some(extension) = entry.path().extension() {
            if extension == "rs" {
                let file_buff = File::open(entry.path())?;
                let file_reader = BufReader::new(file_buff);
                let _ = match io::copy(&mut BufReader::new(file_reader), &mut xz_encoder) {
                    Ok(complete) => {
                        println!(
                            "Compression complete! About {} bytes were written",
                            complete
                        );
                        let _ = xz_encoder.try_finish();
                        Ok(())
                    }
                    Err(err) => Err(err),
                };
            }
        }
    }
    xz_encoder.finish()?;

    println!("Compression complete. Output: ");
    Ok(())
}
