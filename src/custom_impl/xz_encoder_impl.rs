use liblzma::write::XzEncoder;
use std::io::Write;
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::io::{AsyncWrite, Result};

pub struct AsyncXzEncoder<W: AsyncWrite + Unpin> {
    encoder: XzEncoder<W>,
}

impl<W: AsyncWrite + Unpin> AsyncXzEncoder<W> {
    pub fn new(writer: W, level: u32) -> Self {
        let encoder = XzEncoder::new(writer, level);
        Self { encoder }
    }
}

impl<W: AsyncWrite + Unpin> AsyncWrite for AsyncXzEncoder<W> {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize>> {
        Pin::new(&mut self.encoder).poll_write(cx, buf)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<()>> {
        Pin::new(&mut self.encoder).poll_flush(cx)
    }

    fn poll_close(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<()>> {
        Pin::new(&mut self.encoder).poll_close(cx)
    }
}
