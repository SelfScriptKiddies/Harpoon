use std::time::UNIX_EPOCH;

use bytes::{BufMut, BytesMut};
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;
#[cfg(unix)]
use tokio::net::UnixStream;
use tokio::sync::mpsc;
use tracing;

use crate::types::event::{Event, EventKind};
use crate::types::rule::ExporterKind;

const FRAME_VERSION: u8 = 0x01;

pub fn encode_event(event: &Event) -> BytesMut {
    let mut buf = BytesMut::new();

    buf.put_u8(FRAME_VERSION);

    let (kind_id, rule_name, extra) = match &event.kind {
        EventKind::IncomingData { rule, src, len } => {
            (0x01u8, rule.as_str(), format!("{src}:{len}"))
        }
        EventKind::OutgoingData { rule, dst, len } => {
            (0x02, rule.as_str(), format!("{dst}:{len}"))
        }
        EventKind::FilterMatch {
            rule,
            filter_index,
        } => (0x03, rule.as_str(), format!("{filter_index}")),
        EventKind::FilterDrop {
            rule,
            filter_index,
        } => (0x04, rule.as_str(), format!("{filter_index}")),
        EventKind::UdpSessionCreated { rule, client } => {
            (0x05, rule.as_str(), format!("{client}"))
        }
        EventKind::UdpSessionTimeout { rule, client } => {
            (0x06, rule.as_str(), format!("{client}"))
        }
        EventKind::TcpConnectionOpened { rule, client } => {
            (0x07, rule.as_str(), format!("{client}"))
        }
        EventKind::TcpConnectionClosed { rule, client } => {
            (0x08, rule.as_str(), format!("{client}"))
        }
        EventKind::ExporterError { rule, detail } => (0x09, rule.as_str(), detail.clone()),
        EventKind::RuleActivated { rule } => (0x0A, rule.as_str(), String::new()),
    };

    let ts_millis = event
        .timestamp
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;

    buf.put_u8(kind_id);
    buf.put_u64(ts_millis);

    let name_bytes = rule_name.as_bytes();
    buf.put_u16(name_bytes.len() as u16);
    buf.put_slice(name_bytes);

    let extra_bytes = extra.as_bytes();
    buf.put_u16(extra_bytes.len() as u16);
    buf.put_slice(extra_bytes);

    buf
}

fn write_length_prefixed_frame(payload: &BytesMut) -> BytesMut {
    let mut frame = BytesMut::with_capacity(4 + payload.len());
    frame.put_u32(payload.len() as u32);
    frame.put_slice(payload);
    frame
}

pub async fn run_exporter(kind: ExporterKind, mut rx: mpsc::Receiver<Event>) {
    match kind {
        ExporterKind::Uds { path } => {
            run_uds_exporter(&path, &mut rx).await;
        }
        ExporterKind::TcpFramed { addr } => {
            run_tcp_framed_exporter(addr, &mut rx).await;
        }
    }
}

#[cfg(unix)]
async fn run_uds_exporter(path: &std::path::Path, rx: &mut mpsc::Receiver<Event>) {
    let mut stream: Option<UnixStream> = None;

    while let Some(event) = rx.recv().await {
        if stream.is_none() {
            match UnixStream::connect(path).await {
                Ok(s) => stream = Some(s),
                Err(e) => {
                    tracing::warn!(path = %path.display(), error = %e, "uds exporter connect failed");
                    continue;
                }
            }
        }

        let payload = encode_event(&event);
        let frame = write_length_prefixed_frame(&payload);

        if let Some(ref mut s) = stream {
            if let Err(e) = s.write_all(&frame).await {
                tracing::warn!(error = %e, "uds exporter write failed, reconnecting");
                stream = None;
            }
        }
    }
}

async fn run_tcp_framed_exporter(addr: std::net::SocketAddr, rx: &mut mpsc::Receiver<Event>) {
    let mut stream: Option<TcpStream> = None;

    while let Some(event) = rx.recv().await {
        if stream.is_none() {
            match TcpStream::connect(addr).await {
                Ok(s) => stream = Some(s),
                Err(e) => {
                    tracing::warn!(%addr, error = %e, "tcp exporter connect failed");
                    continue;
                }
            }
        }

        let payload = encode_event(&event);
        let frame = write_length_prefixed_frame(&payload);

        if let Some(ref mut s) = stream {
            if let Err(e) = s.write_all(&frame).await {
                tracing::warn!(error = %e, "tcp exporter write failed, reconnecting");
                stream = None;
            }
        }
    }
}
