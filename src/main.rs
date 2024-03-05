use clap::{arg, Parser};
use comics_archiver::cbz_actions::{
    compress_dir_and_files_to_cbz, compress_images_with_img, compress_images_with_lzma,
    extract_dir_and_files_from_cbz,
};
use comics_archiver::err_impl::CompressionError;
use indicatif::{HumanBytes, ProgressBar, ProgressState, ProgressStyle};
use liblzma::write::XzEncoder;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use std::cmp::min;
use std::error::Error;
use std::fmt;
use std::fs::File;
use std::io::{self, BufReader, Cursor, Write};
use std::path::{Path, PathBuf};
use std::process;
use tokio::fs::File as AsyncFile;
use tokio::io::{copy as Async_Copy, AsyncReadExt, AsyncWriteExt, BufWriter};
use walkdir::{DirEntry, WalkDir};

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
async fn compress_action<P1: AsRef<Path>, P2: AsRef<Path>>(
    dir_path: P1,
    output_file: P2,
) -> Result<(Vec<(String, Vec<u8>, PathBuf)>, u64), CompressionError> {
    let mut compressed_list: Vec<PathBuf> = Vec::new();
    let compressed_size: u64;
    let out_file = match AsyncFile::create(output_file).await {
        Ok(out) => out,
        Err(err) => {
            eprintln!("Fail!");
            return Err(CompressionError::IoError(err));
        }
    };

    let total_files = WalkDir::new(dir_path.as_ref())
        .follow_links(true)
        .into_iter()
        .filter_map(|e| e.ok())
        .count();
    let pb = ProgressBar::new(total_files as u64);
    let mut progress = 0;

    let mut encoder = XzEncoder::new(out_file.into_std().await, 9);
    let percent_completion = (progress as f32 / total_files as f32) * 100.0;
    let mut compressed_list = Vec::new();
    //NOTE: Parallelism idea?
    /*
    let mut entries: Result<Vec<DirEntry>, _> = WalkDir::new(dir_path)
        .into_iter()
        .filter_map(|entry| {
            let entry = entry.unwrap();
            if entry.path().extension().map_or(false, |ext| ext == "cbz") {
                Some(entry.path().to_owned())
            } else {
                None
            }
        })
        .try_fold(Vec::new(), |mut acc, path| {
            acc.push(entry);
            Ok(acc)
        });
    let res = entries
        .par_iter()
        .for_each(|x| match extract_dir_and_files_from_cbz(x).await {
            Ok(f) => f,
            Err(err) => {
                eprintln!("Failed due to: {}", err);
                return Err(CompressionError::IoError(err));
            }
        });
    for (_, file_path) in res {
        println!(": {}", file_path.to_str().unwrap());
    }
    */
    let dir_clone = dir_path.as_ref();
    for entry in WalkDir::new(&dir_path).into_iter().filter_map(|e| e.ok()) {
        if let Some(ext) = entry.path().extension() {
            if ext == "cbz" {
                //NOTE: Takes time to extract.
                let cbz_entries = match extract_dir_and_files_from_cbz(entry.path()).await {
                    Ok(f) => f,
                    Err(err) => {
                        eprintln!("Failed due to: {}", err);
                        return Err(CompressionError::IoError(err));
                    }
                };

                //NOTE: If it doesnt work out remove chunks * clones
                //EDIT: This does help free up some performance, problem is its writing more than
                //once?
                //NOTE: Chunking was the right answer :)
                for file_data in cbz_entries.chunks(4) {
                    if let Some(s) = file_data.first() {
                        let compressed_img = match compress_images_with_img(s.1.clone()) {
                            Ok(img) => img,
                            Err(err) => {
                                eprintln!("Error compressing image! : {}", err);
                                return Err(CompressionError::IoError(err));
                            }
                        };
                        compressed_list.push((s.0.clone(), compressed_img, s.2.clone()));
                    };
                }
            }
        }

        for (file_name, _, file_path) in &compressed_list {
            println!(
                "file_name: {}, compressed_list: {}",
                file_name,
                file_path.to_str().unwrap()
            );
        }
        let mut size: usize = 0;
        if let Some(f) = compressed_list.first() {
            size = f.1.len();
        }
        let data = match compress_dir_and_files_to_cbz(compressed_list.clone()) {
            Ok(complete) => complete,
            Err(err) => {
                eprintln!("Error repacking files! : {}", err);
                return Err(CompressionError::IoError(err));
            }
        };

        let tmp_output_path = format!("{}/{}", dir_clone.to_str().unwrap(), "tmp");
        tokio::fs::create_dir_all(&tmp_output_path).await?;

        let tmp_file_path = format!("{}/{:?}", tmp_output_path, data.0);
        /*
                match optimized_file.write_all(&data.1).await {
                    Ok(done) => done,
                    Err(err) => {
                        eprintln!("Failed to copy repacked file: {}", err);
                    }
                }
        */
        match tokio::fs::write(tmp_file_path, &data.1).await {
            Ok(done) => done,

            Err(err) => {
                eprintln!("Error writing cbz file! : {}", err);
                return Err(CompressionError::IoError(err));
            }
        };
        pb.inc(1);
        /*
                match tokio::io::copy(&mut Cursor::new(data.1), &mut optimized_file).await {
                    Ok(_done) => {
                        println!("Copying Done!");
                    }
                    Err(err) => {
                        eprintln!("Failed to copy repacked file: {}", err);
                    }
                };
        */

        /*
                //TODO: MAke this async
                match encoder.write_all(&data[..]) {
                    Ok(_done) => {
                        println!("Writing to file...: ");
                        encoder.try_finish()?;
                    }
                    Err(err) => {
                        eprintln!("Error repacking files! : {}", err);
                        return Err(CompressionError::IoError(err));
                    }
                };
        */

        // Using it directly with no error handling works fine
        //let cbz_entries = extract_dir_and_files_from_cbz(entry.path()).await?;

        // Print cbz_entries on screen for testing
        /*
                for (file_name, file_path) in cbz_entries {
                    println!("{:?}: {}", file_name, file_path.to_str().unwrap());
                }
        */
        //TODO: fetch entries and print
        /*
        * NOTE: This only compresses individual files not grabbing everything then compressing.
        if val.file_type().is_file() && val.path().extension().map_or(false, |ext| ext == "rs") {
            let file = val.path().to_path_buf();
            compressed_size = compress_worker(val, &output_file)?;
            compressed_list.push(file);
        }
        */

        /*
                progress += 1;
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
                        println!("{}", entry.path().file_name().unwrap().to_str().unwrap());
                        io::copy(&mut io::BufReader::new(curr_file), &mut encoder)?;
                    }

                    print!("\r[");
                    let percent_chars = (percent_completion / 2.0) as usize;
                    for _ in 0..percent_chars {
                        print!("=");
                    }
                    for _ in percent_chars..50 {
                        print!(" ");
                    }
                    print!("] {:.2}% ", percent_completion);
                    io::stdout().flush().unwrap();
                }

        */

        println!("before repacking: {}", HumanBytes(size as u64));
        println!("after repacking: {}", HumanBytes(data.1.len() as u64));
    }
    pb.finish_with_message("Compression done!");
    //encoder.try_finish()?;
    compressed_size = encoder.total_out();
    Ok((compressed_list.clone(), compressed_size))
}
#[tokio::main]
async fn main() {
    let args = Args::parse();
    let output_file = args.output_file.clone();
    match compress_action(args.input_dir, args.output_file).await {
        Ok(compressed) => {
            println!("Compression done for: ");
            for file in compressed.0 {
                //println!("{}", file.1.to_str().unwrap());
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
