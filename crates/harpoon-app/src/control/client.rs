use std::path::Path;

use anyhow::{Context, Result};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;

use super::proto::{Request, Response};

pub struct ControlClient {
    stream: UnixStream,
}

impl ControlClient {
    pub async fn connect(socket_path: &Path) -> Result<Self> {
        let stream = UnixStream::connect(socket_path)
            .await
            .with_context(|| {
                format!(
                    "cannot connect to harpoon daemon at {}. Is it running?",
                    socket_path.display()
                )
            })?;
        Ok(Self { stream })
    }

    pub async fn send(&mut self, request: Request) -> Result<Response> {
        let msg = serde_json::to_vec(&request)?;
        self.stream
            .write_all(&(msg.len() as u32).to_be_bytes())
            .await?;
        self.stream.write_all(&msg).await?;

        let mut len_buf = [0u8; 4];
        self.stream.read_exact(&mut len_buf).await?;
        let resp_len = u32::from_be_bytes(len_buf) as usize;

        let mut resp_buf = vec![0u8; resp_len];
        self.stream.read_exact(&mut resp_buf).await?;

        let response: Response = serde_json::from_slice(&resp_buf)?;
        Ok(response)
    }
}
