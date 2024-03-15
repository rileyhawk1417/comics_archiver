use clap::{arg, Parser};
use comics_archiver::cbz_actions::{compress_to_cbz, extract_from_cbz, img_compressor};
use comics_archiver::err_impl::CompressionError;
use humantime::format_duration;
use indicatif::{ProgressBar, ProgressStyle};
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use std::time::Instant;
use walkdir::WalkDir;

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

fn time_formatter(given_time: String) -> String {
    let mut hours = 0;
    let mut mins = 0;
    let mut secs = 0;
    for part in given_time.split_whitespace() {
        match part.chars().last() {
            Some('h') => hours = part.trim_end_matches('h').parse().unwrap_or(0),
            Some('m') => mins = part.trim_end_matches('m').parse().unwrap_or(0),
            Some('s') => secs = part.trim_end_matches('s').parse().unwrap_or(0),
            _ => {}
        }
    }

    let formatted_time = format!("{}h {}m {}s", hours, mins, secs);
    formatted_time
}

/*
* NOTE: Just to silence warnings but I plan to use it.
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
*/

/// Get the paths of the files from given path
///
/// * `source_dir`: <Arc> type value
/// Return `Vec<PathBuf>` : file list, else Return fail.
fn cbz_file_list(
    source_dir: Arc<impl AsRef<Path> + Send + Sync>,
) -> Result<Vec<PathBuf>, CompressionError> {
    let mut discovered_entries: Vec<PathBuf> = Vec::new();

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

async fn process_chapters(
    file_path: String,
    dir_path: &Arc<String>,
) -> Result<String, CompressionError> {
    let processed_imgs = Arc::new(Mutex::new(HashSet::new()));
    let extracted_files: Vec<(String, Vec<u8>, PathBuf)> = extract_from_cbz(file_path).await?;

    let pb = ProgressBar::new_spinner();
    let optimized_images: Vec<(&String, Vec<u8>, &PathBuf)> = extracted_files
        .par_iter()
        .filter_map(|(name, img_blob, img_path)| {
            let mut locked_processed_imgs = processed_imgs.lock().unwrap();
            pb.set_style(
                ProgressStyle::with_template("Compressing Images: {spinner} \n")
                    .unwrap()
                    .progress_chars("#>-"),
            );
            pb.enable_steady_tick(Duration::new(0, 100));
            //NOTE: Maybe change this to a match later..
            if !locked_processed_imgs.contains(&(name, img_path)) {
                let _ = &mut locked_processed_imgs.insert((name, img_path));
                let shrunk_img = img_compressor(img_blob.clone()).unwrap();
                pb.inc(1);
                Some((name, shrunk_img, img_path))
            } else {
                //
                None
            }
        })
        .collect::<Vec<_>>();
    pb.finish_and_clear();

    let repacked_archive: (String, Vec<u8>) = compress_to_cbz(optimized_images).unwrap();

    let tmp_output_path = format!("{}/{}", dir_path, "tmp");
    let tmp_output_file = format!("{}/{}", tmp_output_path, repacked_archive.0.to_owned());
    let _ = tokio::fs::create_dir_all(tmp_output_path.clone()).await;
    match tokio::fs::write(tmp_output_file.clone(), repacked_archive.1).await {
        Ok(done) => done,
        Err(err) => {
            eprintln!("Failed to write repacked archive to disk!: {}", err);
            return Err(CompressionError::IoError(err));
        }
    }
    let compression_done = format!("Chapter {} compressed!", tmp_output_file.clone());
    Ok(compression_done)
}

/// Wrapper to run rayon tasks asynchronously
///
/// * `task`: [TODO:parameter]
async fn run_rayon_task_async<F, Fut, R>(task: F) -> R
where
    F: FnOnce() -> Fut + Send + 'static,
    Fut: std::future::Future<Output = R> + Send + 'static,
    R: Send + 'static,
{
    task().await
}

#[tokio::main]
async fn main() {
    let args = Args::parse();
    //let output_file = args.output_file.clone();
    let input_dir = Arc::new(args.input_dir);
    let time_taken = Instant::now();
    //let in_clone = input_dir.clone();
    let cbz_files_list = cbz_file_list(input_dir.clone()).unwrap();

    let finish_msg = format!("Finished compression for: {}", input_dir.clone());
    let overall_progress = ProgressBar::new_spinner();
    overall_progress.set_style(
        ProgressStyle::with_template("Processing chapters: {spinner}")
            .unwrap()
            .progress_chars("#>-"),
    );
    overall_progress.enable_steady_tick(Duration::new(0, 100));
    /*
    for files in cbz_files_list {
        //NOTE: Run `process_chapters in here for each chapter`
        println!("Filename: {}", files.file_name().unwrap().to_str().unwrap());
    }
    */
    let processed_set = Arc::new(Mutex::new(HashSet::new()));
    //NOTE: Type Vec<impl Future<Output = String>>
    let chapter_funcs = cbz_files_list
        .par_iter()
        .filter_map(move |chapter| {
            /* NOTE: block_on works fine */
            let chapter = chapter.to_str().unwrap().to_owned();
            let in_dir = Arc::clone(&input_dir);
            let mut processed_set_locked = processed_set.lock().unwrap();
            if !processed_set_locked.contains(&chapter) {
                let _ = &mut processed_set_locked.insert(chapter.to_string());
                Some(run_rayon_task_async(move || async move {
                    //NOTE: There is a bottle neck in memory, more like high memory consumption....
                    process_chapters(chapter, &in_dir)
                        .await
                        .expect("Failed to process chapters")
                }))
            } else {
                //
                None
            }
        })
        .collect::<Vec<_>>();
    for files in chapter_funcs {
        overall_progress.inc(1);
        let value = files.await.to_string();
        println!("{}", value);
    }

    overall_progress.finish_with_message(finish_msg);

    let time_taken = time_taken.elapsed();
    println!(
        "Time taken: {}",
        time_formatter(format_duration(time_taken).to_string())
    );
    overall_progress.finish();

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
