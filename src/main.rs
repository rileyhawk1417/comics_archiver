use clap::{arg, Parser};
use std::fs::File;
use std::io::{self, BufReader};
use walkdir::WalkDir;
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
//TODO: Remove generic argument
fn main() -> io::Result<()> {
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
    //compressor(xz_encoder, out);

    for entry in WalkDir::new(args.input_dir)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if let Some(extension) = entry.path().extension() {
            if extension == "rs" {
                //println!("File name: {:?}", entry.path().file_name())
                //NOTE: Research on Result<(), Error>
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
    /*
    Command::new("CBZ Compressor")
        .version("1.0")
        .author("Your Name")
        .about("Compresses CBZ files into an LZMA-compressed archive")
        .arg(
            Arg::new("input-dir")
                .short('i')
                .long("input-dir")
                .value_name("DIRECTORY")
                .help("Sets the input directory containing CBZ files")
                .required(true),
        )
        .arg(
            Arg::new("output-file")
                .short('o')
                .long("output-file")
                .value_name("FILE")
                .help("Sets the output .xz file")
                .required(true),
        )
        .get_matches();
    */
    //TODO: look for a guide that can help with this
    /*
    let values = Args::parse();
    println!("Input Dir: {}", values[0].name);
    println!("Output File: {}", values[1].name);
    */

    /*
        let input_dir = Args::parse("input-dir").unwrap();
        let output_file = matches.value_of("output-file").unwrap();
    */

    // Create the output .xz file
    /*
    let output_file = File::create(output_file)?;
    let mut encoder = XzEncoder::new(output_file, 9);

    // Traverse the directory to find .cbz files
    for entry in WalkDir::new(input_dir).into_iter().filter_map(|e| e.ok()) {
        if let Some(extension) = entry.path().extension() {
            if extension == "cbz" {
                let mut file = File::open(entry.path())?;

                // Copy the content of the .cbz file to the encoder
                io::copy(&mut file, &mut encoder)?;
            }
        }
    }

    // Finish encoding and flush the output
    encoder.finish()?;
    */

    //xz_encoder.finish()?;
    println!("Compression complete. Output: ");
    Ok(())
}
