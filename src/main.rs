use clap::{arg, Parser};
use comics_archiver::cbz_actions::{
    compress_dir_and_files_to_cbz, compress_images_with_img, extract_dir_and_files_from_cbz,
};
use comics_archiver::err_impl::CompressionError;
use humantime::format_duration;
use indicatif::{HumanBytes, MultiProgress, ProgressBar, ProgressState, ProgressStyle};
use liblzma::write::XzEncoder;
use rayon::iter::{IntoParallelRefIterator, IntoParallelRefMutIterator, ParallelIterator};
use std::fs::File;
use std::io::{self, BufReader, Cursor, Write};
use std::path::{Path, PathBuf};
use std::process;
use std::sync::{Arc, Mutex};
use std::time::Instant;
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

fn cbz_file_count(file_dir: Arc<impl AsRef<Path> + Send + Sync>) -> u64 {
    let mut file_count = 0;

    for entry in WalkDir::new(file_dir.as_ref())
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if let Some(ext) = entry.path().extension() {
            if ext == "cbz" {
                file_count += 1;
            }
        }
    }
    file_count as u64
}

/// Get the paths of the files from given path
///
/// * `source_dir`: <Arc> type value
/// Return `Vec<PathBuf>` : file list, else Return fail.
fn cbz_file_list(
    source_dir: Arc<impl AsRef<Path> + Send + Sync>,
) -> Result<Vec<PathBuf>, CompressionError> {
    let mut discovered_entries = Vec::new();

    for entry in WalkDir::new(source_dir.as_ref())
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if let Some(ext) = entry.path().extension() {
            if ext == "cbz" {
                discovered_entries.push(entry.path().to_owned());
            }
        }
    }
    Ok(discovered_entries)
}

//TODO: Move this to another file then write the decompression logic.
/// Define compress action
/// Compress the given files, then return
/// the list of included files for compression & output file size.
/// * `dir_path`: Directory with cbz files.
/// * `output_file`: Name of output file.
async fn compress_action<'a, P2: AsRef<Path>>(
    dir_path: Arc<impl AsRef<Path> + Send + Sync + 'static>,
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

    let dir_clone = &dir_path.clone();
    let total_files = cbz_file_count(dir_path.clone());

    let multi_pb = MultiProgress::new();
    let pb = multi_pb.add(ProgressBar::new(total_files as u64));
    //    let mut progress = 0;

    let mut encoder = XzEncoder::new(out_file.into_std().await, 9);
    //  let percent_completion = (progress as f32 / total_files as f32) * 100.0;
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
    println!("Begin the process...");
    let raw_files = tokio::spawn(async {
        let raw_file_list = cbz_file_list(dir_path).unwrap();
        let mut extracted_files = Vec::new();
        for filez in raw_file_list {
            let data = match extract_dir_and_files_from_cbz(filez).await {
                Ok(f) => f,
                Err(err) => {
                    eprintln!("Failed due to: {}", err);
                    return Err(CompressionError::IoError(err));
                }
            };
            //TODO: Figure out filter conditions.
            if !extracted_files.contains(&data) {
                extracted_files.push(data);
            }
        }
        Ok(extracted_files)
    });
    /*
    * NOTE: This block does work though
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
                //NOTE: Chunking was the right answer :) 4 allows it to at least repack all files
                //nicely.
                for file_data in cbz_entries.chunks(4) {
                    if let Some(s) = file_data.first() {
                        let compressed_img = match compress_images_with_img(s.1.clone()) {
                            Ok(img) => img,
                            Err(err) => {
                                eprintln!("Error compressing image! : {}", err);
                                return Err(err);
                            }
                        };
                        compressed_list.push((s.0.clone(), compressed_img, s.2.clone()));
                    };
                }
            }
        }
        */
    /*
    for (file_name, _, file_path) in &compressed_list {
        println!(
            "file_name: {}, compressed_list: {}",
            file_name,
            file_path.to_str().unwrap()
        );
    }
    */
    /*
    let mut size: usize = 0;
    if let Some(f) = compressed_list.first() {
        size = f.1.len();
    }
    */
    //BUG: Being called too many times without waiting for unpacking to finish.
    let mut raw_data: Vec<Vec<(String, Vec<u8>, PathBuf)>> = raw_files.await.unwrap().unwrap();
    let pb_imgs = multi_pb.insert_after(&pb, ProgressBar::new(raw_data.len() as u64));
    let mutex_data = Arc::new(Mutex::new(raw_data.clone()));
    pb_imgs.set_message("Compressing images...");
    raw_data.par_iter_mut().for_each(|imgs| {
        let mut raw_data = mutex_data.lock().unwrap();
        for inner_items in imgs.iter_mut() {
            let img_1 = inner_items.1.clone();
            inner_items.1 = compress_images_with_img(img_1).expect("Failed to compress!");
            pb_imgs.inc(1);
        }
    });
    pb_imgs.finish_with_message("Finished compressing images!");

    //Loop inside all cbz archives.
    let final_compression: Vec<Result<(String, Vec<u8>), CompressionError>> = raw_data
        .par_iter()
        .map(
            |files| match compress_dir_and_files_to_cbz(files.to_vec()) {
                Ok(complete) => Ok(complete),
                Err(err) => {
                    return Err(CompressionError::IoError(err));
                }
            },
        )
        .collect();

    let tmp_output_path = format!(
        "{}/{}",
        dir_clone.as_ref().as_ref().to_str().unwrap(),
        "tmp"
    );
    tokio::fs::create_dir_all(tmp_output_path.clone()).await?;

    for items in final_compression {
        let _ = if let Ok(item) = items {
            let tmp_file_path = format!("{}/{}", tmp_output_path, item.0);
            match tokio::fs::write(tmp_file_path, item.1).await {
                Ok(done) => done,

                Err(err) => {
                    eprintln!("Error writing cbz file! : {}", err);
                    return Err(CompressionError::IoError(err));
                }
            };
        };
    }
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


        println!("before repacking: {}", HumanBytes(size as u64));
        println!("after repacking: {}", HumanBytes(data.1.len() as u64));
    }

        */
    pb.finish_with_message("Compression done!");
    //encoder.try_finish()?;
    compressed_size = encoder.total_out();
    Ok((compressed_list.clone(), compressed_size))
}
#[tokio::main]
async fn main() {
    let args = Args::parse();
    let output_file = args.output_file.clone();
    let input_dir = Arc::new(args.input_dir);
    let time_taken = Instant::now();
    let cbz_files_list = cbz_file_list(input_dir).unwrap();
    for files in cbz_files_list {
        println!("Filename: {}", files.file_name().unwrap().to_str().unwrap());
    }

    //TODO: Refactor the code to handle directories.
    /*
     * TODO: Refactoring on how the code/logic behaves.
     * NOTE: The below snippets are meant to be in `process_chapters`.
     * Which will be one function handling all the data processing & file IO.
     * As it goes over chapters.
     * NOTE: Get name of img, img_data, metadata
     * let extracted_data: Vec<String, Vec<u8>, Vec<Path>> = extract_files(AsRef<Path>).await
     * let optimize_images: Vec<String, Vec<u8>, Vec<Path>> = optimize_imgs(extracted_data.await)
     * let compressed_data: Vec<String, Vec<u8>> = repack_files(optimize_images)
     * write_to_disk(compressed_data).await
     * */
    /*
        match compress_action(input_dir, args.output_file).await {
            Ok(compressed) => {
                println!("Compression done for: ");
                for file in compressed.0 {
                    //println!("{}", file.1.to_str().unwrap());
                }
                println!("New compressed file name: {}", output_file);
                println!("New file size: {}", compressed.1);
                println!(
                    "Total time taken for compression: {}",
                    format_duration(time_taken.elapsed()).to_string()
                );
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
    */
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
