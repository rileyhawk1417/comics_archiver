use async_trait::async_trait;
use liblzma::write::XzEncoder;
use std::io::{self, Write};
use tokio::fs::File;
use tokio::io::{AsyncWriteExt, BufWriter};
async fn write_to_file_async(
    filename: &str,
    data: &[u8],
) -> Result<(), Box<dyn std::error::Error>> {
    // Open the file for writing asynchronously
    let file = match File::create(filename).await {
        Ok(f) => f,
        Err(err) => {
            eprintln!("Failed to create output file: {}", err);
            return Err(err.into());
        }
    };

    let file_2 = match std::fs::File::create(filename) {
        Ok(f) => f,
        Err(err) => {
            eprintln!("Failed to create output file: {}", err);
            return Err(err.into());
        }
    };

    // Create a buffered writer
    let mut buf_writer = BufWriter::new(file_2);
    let mut file_buff = std::io::BufWriter::new(file_2);

    // Create an XzEncoder to compress the data
    let mut xz_encoder = XzEncoder::new(file_buff, 6);

    // Write the compressed data to the file
    //xz_encoder.write_all(data).await?;

    // Flush the writer to ensure all data is written to the file
    //xz_encoder.finish().await?;

    Ok(())
}
