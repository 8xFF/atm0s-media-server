use futures::AsyncRead as _;
use media_server_protocol::record::{SessionRecordHeader, SessionRecordRow};
use surf::Body;
use tokio::io::{AsyncRead, AsyncReadExt};

pub struct BodyWrap {
    body: Body,
}

impl BodyWrap {
    pub async fn get_uri(uri: &str) -> Result<Self, surf::Error> {
        let body = surf::get(uri).await?.take_body();
        Ok(Self { body })
    }
}

impl AsyncRead for BodyWrap {
    fn poll_read(mut self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>, buf: &mut tokio::io::ReadBuf<'_>) -> std::task::Poll<std::io::Result<()>> {
        let mut tmp_buf = [0; 1500];
        match std::pin::Pin::new(&mut self.body).poll_read(cx, &mut tmp_buf[0..buf.remaining()]) {
            std::task::Poll::Ready(Ok(size)) => {
                buf.put_slice(&tmp_buf[0..size]);
                std::task::Poll::Ready(Ok(()))
            }
            std::task::Poll::Ready(Err(e)) => std::task::Poll::Ready(Err(std::io::Error::new(std::io::ErrorKind::BrokenPipe, ""))),
            std::task::Poll::Pending => std::task::Poll::Pending,
        }
    }
}

async fn read_util_full<R: AsyncRead + Unpin>(source: &mut R, buf: &mut [u8]) -> std::io::Result<()> {
    let mut read_len = 0;
    while read_len < buf.len() {
        let part = source.read_exact(&mut buf[read_len..]).await?;
        read_len += part;
    }
    assert_eq!(read_len, buf.len());
    Ok(())
}

pub struct RecordChunkReader<R> {
    source: R,
    buf: [u8; 1500],
    header: SessionRecordHeader,
}

impl<R: AsyncRead + Unpin> RecordChunkReader<R> {
    pub async fn new(mut source: R) -> std::io::Result<Self> {
        let header_len = source.read_u32().await?;
        log::info!("header len {header_len}");
        let mut buf = [0; 1500];
        read_util_full(&mut source, &mut buf[0..header_len as usize]).await?;
        let header = SessionRecordHeader::read_from(&buf[0..header_len as usize])?;
        Ok(Self { source, buf, header })
    }

    pub async fn pop(&mut self) -> std::io::Result<Option<SessionRecordRow>> {
        let chunk_len = self.source.read_u32().await?;
        log::debug!("chunk len {chunk_len}");
        read_util_full(&mut self.source, &mut self.buf[0..chunk_len as usize]).await?;
        let event = SessionRecordRow::read_from(&self.buf[0..chunk_len as usize])?;
        Ok(Some(event))
    }
}
