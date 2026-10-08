use std::net::SocketAddr;
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream, UdpSocket};

#[cfg(feature = "http2")]
use bytes::Bytes;

use harpoon_core::config::CoreConfig;
use harpoon_core::types::endpoint::Endpoint;
use harpoon_core::types::filter::{Direction, Filter, FilterAction, FilterKind};
use harpoon_core::types::rule::Rule;

#[tokio::test]
async fn test_tcp_proxy_basic() {
    // Start echo server
    let echo_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let echo_addr = echo_listener.local_addr().unwrap();

    tokio::spawn(async move {
        loop {
            let (mut stream, _) = echo_listener.accept().await.unwrap();
            tokio::spawn(async move {
                let mut buf = [0u8; 1024];
                loop {
                    let n = stream.read(&mut buf).await.unwrap();
                    if n == 0 {
                        break;
                    }
                    stream.write_all(&buf[..n]).await.unwrap();
                }
            });
        }
    });

    let listen_addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
    // Bind to get a free port
    let tmp = TcpListener::bind(listen_addr).await.unwrap();
    let proxy_addr = tmp.local_addr().unwrap();
    drop(tmp);

    let config = CoreConfig {
        rules: vec![Rule {
            name: "tcp-test".into(),
            listen: Endpoint::tcp(proxy_addr),
            target: Endpoint::tcp(echo_addr),
            filters: vec![],
            duplicate: None,
            exporter: None,
            tls: None,
            udp_source_mode: harpoon_core::types::rule::UdpSourceMode::Proxy,
            http2: false,
            idle_timeout_secs: 30,
        }],
        ..CoreConfig::default()
    };

    let handle = harpoon_core::run(config).await.unwrap();

    // Give the proxy time to bind
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Connect through proxy
    let mut client = TcpStream::connect(proxy_addr).await.unwrap();
    client.write_all(b"hello harpoon").await.unwrap();

    let mut buf = [0u8; 64];
    let n = client.read(&mut buf).await.unwrap();
    assert_eq!(&buf[..n], b"hello harpoon");

    handle.stop();
    handle.shutdown().await;
}

#[tokio::test]
async fn test_tcp_proxy_with_drop_filter() {
    let echo_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let echo_addr = echo_listener.local_addr().unwrap();

    tokio::spawn(async move {
        loop {
            let (mut stream, _) = echo_listener.accept().await.unwrap();
            tokio::spawn(async move {
                let mut buf = [0u8; 1024];
                loop {
                    let n = stream.read(&mut buf).await.unwrap();
                    if n == 0 {
                        break;
                    }
                    stream.write_all(&buf[..n]).await.unwrap();
                }
            });
        }
    });

    let tmp = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let proxy_addr = tmp.local_addr().unwrap();
    drop(tmp);

    let config = CoreConfig {
        rules: vec![Rule {
            name: "tcp-filter-test".into(),
            listen: Endpoint::tcp(proxy_addr),
            target: Endpoint::tcp(echo_addr),
            filters: vec![Filter {
                kind: FilterKind::Substr("blocked".into()),
                direction: Direction::ClientToServer,
                action_on_match: FilterAction::Drop,
            }],
            duplicate: None,
            exporter: None,
            tls: None,
            udp_source_mode: harpoon_core::types::rule::UdpSourceMode::Proxy,
            http2: false,
            idle_timeout_secs: 30,
        }],
        ..CoreConfig::default()
    };

    let handle = harpoon_core::run(config).await.unwrap();
    tokio::time::sleep(Duration::from_millis(50)).await;

    let mut client = TcpStream::connect(proxy_addr).await.unwrap();

    // Send blocked content — should be dropped
    client.write_all(b"this is blocked data").await.unwrap();
    // Wait for the proxy to process this chunk separately
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Send allowed content
    client.write_all(b"hello world").await.unwrap();

    let mut buf = [0u8; 64];
    let n = tokio::time::timeout(Duration::from_secs(2), client.read(&mut buf))
        .await
        .expect("timeout reading from proxy")
        .unwrap();
    assert_eq!(&buf[..n], b"hello world");

    // Check stats
    let stats = handle.stats_snapshot();
    assert_eq!(stats[0].dropped_packets, 1);
    assert_eq!(stats[0].filter_matches, 1);

    handle.stop();
    handle.shutdown().await;
}

#[tokio::test]
async fn test_udp_relay_basic() {
    // Start UDP echo server
    let echo_sock = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let echo_addr = echo_sock.local_addr().unwrap();

    tokio::spawn(async move {
        let mut buf = [0u8; 65507];
        loop {
            let (n, addr) = echo_sock.recv_from(&mut buf).await.unwrap();
            echo_sock.send_to(&buf[..n], addr).await.unwrap();
        }
    });

    let tmp = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let proxy_addr = tmp.local_addr().unwrap();
    drop(tmp);

    let config = CoreConfig {
        rules: vec![Rule {
            name: "udp-test".into(),
            listen: Endpoint::udp(proxy_addr),
            target: Endpoint::udp(echo_addr),
            filters: vec![],
            duplicate: None,
            exporter: None,
            tls: None,
            udp_source_mode: harpoon_core::types::rule::UdpSourceMode::Proxy,
            http2: false,
            idle_timeout_secs: 30,
        }],
        ..CoreConfig::default()
    };

    let handle = harpoon_core::run(config).await.unwrap();
    tokio::time::sleep(Duration::from_millis(50)).await;

    let client = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    client.send_to(b"hello udp", proxy_addr).await.unwrap();

    let mut buf = [0u8; 64];
    let n = tokio::time::timeout(Duration::from_secs(2), client.recv(&mut buf))
        .await
        .expect("timeout waiting for UDP response")
        .unwrap();

    assert_eq!(&buf[..n], b"hello udp");

    // Check stats
    let stats = handle.stats_snapshot();
    assert_eq!(stats[0].packets_client_to_server, 1);
    assert_eq!(stats[0].packets_server_to_client, 1);

    handle.stop();
    handle.shutdown().await;
}

#[tokio::test]
async fn test_udp_session_multiple_clients() {
    let echo_sock = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let echo_addr = echo_sock.local_addr().unwrap();

    tokio::spawn(async move {
        let mut buf = [0u8; 65507];
        loop {
            let (n, addr) = echo_sock.recv_from(&mut buf).await.unwrap();
            echo_sock.send_to(&buf[..n], addr).await.unwrap();
        }
    });

    let tmp = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let proxy_addr = tmp.local_addr().unwrap();
    drop(tmp);

    let config = CoreConfig {
        rules: vec![Rule {
            name: "udp-multi".into(),
            listen: Endpoint::udp(proxy_addr),
            target: Endpoint::udp(echo_addr),
            filters: vec![],
            duplicate: None,
            exporter: None,
            tls: None,
            udp_source_mode: harpoon_core::types::rule::UdpSourceMode::Proxy,
            http2: false,
            idle_timeout_secs: 30,
        }],
        ..CoreConfig::default()
    };

    let handle = harpoon_core::run(config).await.unwrap();
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Two separate clients
    let client1 = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let client2 = UdpSocket::bind("127.0.0.1:0").await.unwrap();

    client1.send_to(b"client1", proxy_addr).await.unwrap();
    client2.send_to(b"client2", proxy_addr).await.unwrap();

    let mut buf1 = [0u8; 64];
    let mut buf2 = [0u8; 64];

    let n1 = tokio::time::timeout(Duration::from_secs(2), client1.recv(&mut buf1))
        .await
        .expect("timeout client1")
        .unwrap();
    let n2 = tokio::time::timeout(Duration::from_secs(2), client2.recv(&mut buf2))
        .await
        .expect("timeout client2")
        .unwrap();

    assert_eq!(&buf1[..n1], b"client1");
    assert_eq!(&buf2[..n2], b"client2");

    let stats = handle.stats_snapshot();
    assert_eq!(stats[0].active_udp_sessions, 2);

    handle.stop();
    handle.shutdown().await;
}

#[tokio::test]
async fn test_tcp_proxy_stats() {
    let echo_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let echo_addr = echo_listener.local_addr().unwrap();

    tokio::spawn(async move {
        loop {
            let (mut stream, _) = echo_listener.accept().await.unwrap();
            tokio::spawn(async move {
                let mut buf = [0u8; 1024];
                loop {
                    let n = stream.read(&mut buf).await.unwrap();
                    if n == 0 {
                        break;
                    }
                    stream.write_all(&buf[..n]).await.unwrap();
                }
            });
        }
    });

    let tmp = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let proxy_addr = tmp.local_addr().unwrap();
    drop(tmp);

    let config = CoreConfig {
        rules: vec![Rule {
            name: "stats-test".into(),
            listen: Endpoint::tcp(proxy_addr),
            target: Endpoint::tcp(echo_addr),
            filters: vec![],
            duplicate: None,
            exporter: None,
            tls: None,
            udp_source_mode: harpoon_core::types::rule::UdpSourceMode::Proxy,
            http2: false,
            idle_timeout_secs: 30,
        }],
        ..CoreConfig::default()
    };

    let handle = harpoon_core::run(config).await.unwrap();
    tokio::time::sleep(Duration::from_millis(50)).await;

    {
        let mut client = TcpStream::connect(proxy_addr).await.unwrap();
        client.write_all(b"test data 123").await.unwrap();

        let mut buf = [0u8; 64];
        let n = client.read(&mut buf).await.unwrap();
        assert_eq!(n, 13);
        // Drop client to close connection — fast-path updates stats on close
    }

    tokio::time::sleep(Duration::from_millis(100)).await;

    let stats = handle.stats_snapshot();
    assert_eq!(stats[0].bytes_client_to_server, 13);
    assert_eq!(stats[0].bytes_server_to_client, 13);

    handle.stop();
    handle.shutdown().await;
}

// ── Pipeline-native tests ──

use harpoon_core::types::pipeline::*;
use harpoon_core::types::rule::UdpSourceMode;

#[tokio::test]
async fn test_pipeline_fast_forward_tcp() {
    let echo_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let echo_addr = echo_listener.local_addr().unwrap();

    tokio::spawn(async move {
        loop {
            let (mut stream, _) = echo_listener.accept().await.unwrap();
            tokio::spawn(async move {
                let mut buf = [0u8; 1024];
                loop {
                    let n = stream.read(&mut buf).await.unwrap();
                    if n == 0 { break; }
                    stream.write_all(&buf[..n]).await.unwrap();
                }
            });
        }
    });

    let tmp = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let proxy_addr = tmp.local_addr().unwrap();
    drop(tmp);

    let pipeline = Pipeline {
        id: "pipe-ff".into(),
        name: "pipe-ff".into(),
        nodes: vec![
            Node {
                id: 1,
                label: "source".into(),
                kind: NodeKind::Source(SourceConfig {
                    endpoint: Endpoint::tcp(proxy_addr),
                    udp_source_mode: UdpSourceMode::Proxy,
                    idle_timeout_secs: 30,
                }),
            },
            Node {
                id: 2,
                label: "forward".into(),
                kind: NodeKind::Forward(ForwardConfig {
                    endpoint: Endpoint::tcp(echo_addr),
                    tcp_nodelay: true,
                }),
            },
        ],
        edges: vec![Edge { id: 1, from_node: 1, to_node: 2, from_port: None }],
    };

    let config = CoreConfig {
        pipelines: vec![pipeline],
        ..CoreConfig::default()
    };

    let handle = harpoon_core::run(config).await.unwrap();
    tokio::time::sleep(Duration::from_millis(50)).await;

    {
        let mut client = TcpStream::connect(proxy_addr).await.unwrap();
        client.write_all(b"pipeline test").await.unwrap();
        let mut buf = [0u8; 64];
        let n = client.read(&mut buf).await.unwrap();
        assert_eq!(&buf[..n], b"pipeline test");
    }

    tokio::time::sleep(Duration::from_millis(50)).await;
    let stats = handle.stats_snapshot();
    assert_eq!(stats[0].bytes_client_to_server, 13);

    handle.stop();
    handle.shutdown().await;
}

#[tokio::test]
async fn test_pipeline_linear_with_filter() {
    let echo_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let echo_addr = echo_listener.local_addr().unwrap();

    tokio::spawn(async move {
        loop {
            let (mut stream, _) = echo_listener.accept().await.unwrap();
            tokio::spawn(async move {
                let mut buf = [0u8; 1024];
                loop {
                    let n = stream.read(&mut buf).await.unwrap();
                    if n == 0 { break; }
                    stream.write_all(&buf[..n]).await.unwrap();
                }
            });
        }
    });

    let tmp = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let proxy_addr = tmp.local_addr().unwrap();
    drop(tmp);

    let pipeline = Pipeline {
        id: "pipe-lin".into(),
        name: "pipe-lin".into(),
        nodes: vec![
            Node {
                id: 1,
                label: "source".into(),
                kind: NodeKind::Source(SourceConfig {
                    endpoint: Endpoint::tcp(proxy_addr),
                    udp_source_mode: UdpSourceMode::Proxy,
                    idle_timeout_secs: 30,
                }),
            },
            Node {
                id: 2,
                label: "filter".into(),
                kind: NodeKind::Filter(FilterNodeConfig {
                    filters: vec![Filter {
                        kind: FilterKind::Substr("blocked".into()),
                        direction: Direction::ClientToServer,
                        action_on_match: FilterAction::Drop,
                    }],
                }),
            },
            Node {
                id: 3,
                label: "forward".into(),
                kind: NodeKind::Forward(ForwardConfig {
                    endpoint: Endpoint::tcp(echo_addr),
                    tcp_nodelay: true,
                }),
            },
        ],
        edges: vec![
            Edge { id: 1, from_node: 1, to_node: 2, from_port: None },
            Edge { id: 2, from_node: 2, to_node: 3, from_port: None },
        ],
    };

    let config = CoreConfig {
        pipelines: vec![pipeline],
        ..CoreConfig::default()
    };

    let handle = harpoon_core::run(config).await.unwrap();
    tokio::time::sleep(Duration::from_millis(50)).await;

    let mut client = TcpStream::connect(proxy_addr).await.unwrap();
    client.write_all(b"this is blocked data").await.unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;
    client.write_all(b"hello pipe").await.unwrap();

    let mut buf = [0u8; 64];
    let n = tokio::time::timeout(Duration::from_secs(2), client.read(&mut buf))
        .await.expect("timeout").unwrap();
    assert_eq!(&buf[..n], b"hello pipe");

    let stats = handle.stats_snapshot();
    assert_eq!(stats[0].dropped_packets, 1);

    handle.stop();
    handle.shutdown().await;
}

/// Regression test for the h2-autodetect peek stall (audit: "every TCP
/// connection waits an extra 500ms"). A server-first upstream (SMTP/FTP/SSH
/// style — greeting sent immediately on connect) is proxied through a rule in
/// the h2-autodetect path: `peek_is_h2` must time out quickly and start plain
/// forwarding, instead of stalling the full 500ms before the client ever sees
/// the server's greeting. The old 500ms peek alone blew the 400ms bound below;
/// with the short `H2_PEEK_TIMEOUT` the roundtrip passes with a wide margin.
#[cfg(feature = "http2")]
#[tokio::test]
async fn test_tcp_server_first_roundtrip_fast() {
    // Server-first upstream: greeting immediately on accept, then echo
    let upstream_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let upstream_addr = upstream_listener.local_addr().unwrap();

    tokio::spawn(async move {
        loop {
            let (mut stream, _) = upstream_listener.accept().await.unwrap();
            tokio::spawn(async move {
                if stream.write_all(b"GREETING hello-from-server\n").await.is_err() {
                    return;
                }
                let mut buf = [0u8; 1024];
                loop {
                    match stream.read(&mut buf).await {
                        Ok(0) | Err(_) => break,
                        Ok(n) => {
                            if stream.write_all(&buf[..n]).await.is_err() {
                                break;
                            }
                        }
                    }
                }
            });
        }
    });

    let tmp = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let proxy_addr = tmp.local_addr().unwrap();
    drop(tmp);

    let config = CoreConfig {
        rules: vec![Rule {
            name: "tcp-server-first".into(),
            listen: Endpoint::tcp(proxy_addr),
            target: Endpoint::tcp(upstream_addr),
            filters: vec![],
            duplicate: None,
            exporter: None,
            tls: None,
            udp_source_mode: harpoon_core::types::rule::UdpSourceMode::Proxy,
            http2: true, // rule goes through the h2-autodetect peek
            idle_timeout_secs: 30,
        }],
        ..CoreConfig::default()
    };

    let handle = harpoon_core::run(config).await.unwrap();

    // Give the proxy time to bind
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Full roundtrip: connect → read server greeting → echo back → read echo.
    let roundtrip = async {
        let mut client = TcpStream::connect(proxy_addr).await.unwrap();

        let mut buf = [0u8; 64];
        let n = client.read(&mut buf).await.unwrap();
        assert_eq!(&buf[..n], b"GREETING hello-from-server\n", "server-first greeting");

        client.write_all(b"ping").await.unwrap();
        let n = client.read(&mut buf).await.unwrap();
        assert_eq!(&buf[..n], b"ping", "echo roundtrip");
    };

    tokio::time::timeout(Duration::from_millis(400), roundtrip)
        .await
        .expect("TIMEOUT: server-first roundtrip took >400ms — h2 autodetect peek stall (H2_PEEK_TIMEOUT too large?)");

    handle.stop();
    handle.shutdown().await;
}

/// Low-level test: send H2 preface over raw TCP and verify server SETTINGS arrives.
/// This simulates what grpcio (C core) does and catches SETTINGS flush issues.
#[cfg(feature = "http2")]
#[tokio::test]
async fn test_http2_settings_arrives_raw() {
    // Upstream H2 server
    let upstream_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let upstream_addr = upstream_listener.local_addr().unwrap();

    tokio::spawn(async move {
        let (stream, _) = upstream_listener.accept().await.unwrap();
        let mut conn = h2::server::handshake(stream).await.expect("upstream handshake");
        while let Some(_) = conn.accept().await {}
    });

    // Harpoon proxy
    let tmp = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let proxy_addr = tmp.local_addr().unwrap();
    drop(tmp);

    let config = CoreConfig {
        rules: vec![Rule {
            name: "h2-raw".into(),
            listen: Endpoint::tcp(proxy_addr),
            target: Endpoint::tcp(upstream_addr),
            filters: vec![],
            duplicate: None,
            exporter: None,
            tls: None,
            udp_source_mode: harpoon_core::types::rule::UdpSourceMode::Proxy,
            http2: true,
            idle_timeout_secs: 30,
        }],
        ..CoreConfig::default()
    };

    let handle = harpoon_core::run(config).await.unwrap();
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Raw TCP client: send preface manually, then read server SETTINGS
    let mut tcp = TcpStream::connect(proxy_addr).await.unwrap();

    // Send client preface (what grpcio sends first)
    tcp.write_all(b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n").await.unwrap();

    // Send a minimal SETTINGS frame (empty SETTINGS = 9 bytes frame header)
    // Frame header: length(3) = 0x000000, type(1) = 0x04 (SETTINGS), flags(1) = 0x00, stream_id(4) = 0x00000000
    tcp.write_all(&[0x00, 0x00, 0x00, 0x04, 0x00, 0x00, 0x00, 0x00, 0x00]).await.unwrap();

    // Read server response — should contain SETTINGS frame within 2 seconds
    let mut buf = [0u8; 256];
    let n = tokio::time::timeout(Duration::from_secs(2), tcp.read(&mut buf))
        .await
        .expect("TIMEOUT: server did not send SETTINGS frame!")
        .expect("read error");

    // Server SETTINGS frame should be at least 9 bytes (frame header)
    assert!(n >= 9, "expected at least 9 bytes (SETTINGS frame header), got {n}");

    // Check it's a SETTINGS frame: type byte at offset 3 should be 0x04
    assert_eq!(buf[3], 0x04, "expected SETTINGS frame type (0x04), got 0x{:02x}", buf[3]);

    handle.stop();
    handle.shutdown().await;
}

/// Test HTTP/2 proxying through harpoon (reproduces gRPC SETTINGS timeout).
///
/// Setup: H2 client → Harpoon proxy (peek_is_h2 → http2_proxy) → H2 echo server
#[cfg(feature = "http2")]
#[tokio::test]
async fn test_http2_proxy_basic() {
    // 1. Upstream H2 server: accepts H2, echoes request path in response body
    let upstream_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let upstream_addr = upstream_listener.local_addr().unwrap();

    tokio::spawn(async move {
        let (stream, _) = upstream_listener.accept().await.unwrap();
        let mut conn = h2::server::handshake(stream).await.expect("upstream h2 handshake");
        while let Some(result) = conn.accept().await {
            let (req, mut respond) = result.expect("upstream accept");
            let path = req.uri().path().to_string();
            let response = http::Response::builder().status(200).body(()).unwrap();
            let mut send = respond.send_response(response, false).unwrap();
            send.send_data(Bytes::from(format!("echo: {path}")), true).unwrap();
        }
    });

    // 2. Harpoon proxy: simple TCP rule, HTTP/2 autodetect will kick in
    let tmp = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let proxy_addr = tmp.local_addr().unwrap();
    drop(tmp);

    let config = CoreConfig {
        rules: vec![Rule {
            name: "h2-test".into(),
            listen: Endpoint::tcp(proxy_addr),
            target: Endpoint::tcp(upstream_addr),
            filters: vec![],
            duplicate: None,
            exporter: None,
            tls: None,
            udp_source_mode: harpoon_core::types::rule::UdpSourceMode::Proxy,
            http2: true,
            idle_timeout_secs: 30,
        }],
        ..CoreConfig::default()
    };

    let handle = harpoon_core::run(config).await.unwrap();
    tokio::time::sleep(Duration::from_millis(50)).await;

    // 3. H2 client: connect through proxy, send request
    let tcp = TcpStream::connect(proxy_addr).await.unwrap();
    let (h2_client, h2_conn) = tokio::time::timeout(
        Duration::from_secs(5),
        h2::client::handshake(tcp),
    )
    .await
    .expect("H2 client handshake timed out (SETTINGS not received?)")
    .expect("H2 client handshake failed");

    tokio::spawn(async move {
        if let Err(e) = h2_conn.await {
            eprintln!("H2 client connection error: {e}");
        }
    });

    let mut h2_client = h2_client.ready().await.unwrap();

    let request = http::Request::builder()
        .method("POST")
        .uri("http://localhost/test-grpc")
        .body(())
        .unwrap();

    let (response_future, mut send_stream) = h2_client.send_request(request, false).unwrap();
    send_stream.send_data(Bytes::from_static(b"request body"), true).unwrap();

    let response = tokio::time::timeout(Duration::from_secs(5), response_future)
        .await
        .expect("response timed out")
        .expect("response error");

    assert_eq!(response.status(), http::StatusCode::OK);

    // Read response body
    let mut body = response.into_body();
    let mut response_data = Vec::new();
    while let Some(chunk) = body.data().await {
        let chunk = chunk.unwrap();
        body.flow_control().release_capacity(chunk.len()).unwrap();
        response_data.extend_from_slice(&chunk);
    }

    assert_eq!(String::from_utf8(response_data).unwrap(), "echo: /test-grpc");

    handle.stop();
    handle.shutdown().await;
}

// ── HTTP/2 large-body regression tests ──
//
// The h2 server-side Connection is the only driver of client socket reads and
// flow-control WINDOW_UPDATEs. These tests push 200KB bodies (far beyond the
// ~64KB initial HTTP/2 flow-control window) in both directions; they deadlock
// if the proxy stops driving the Connection while streaming a body.

/// 200KB request body roundtrip: client → proxy → upstream, upstream reads the
/// FULL body then echoes its length. Deadlocks if the server-side Connection
/// isn't driven while the proxy streams the request body (client exhausts its
/// flow-control window and the response HEADERS never arrive).
#[cfg(feature = "http2")]
#[tokio::test]
async fn test_http2_proxy_large_request_body() {
    const BODY: usize = 200 * 1024;

    // Upstream H2 server: per-stream handlers are spawned (like hyper does),
    // so the upstream connection driver never blocks on a stream.
    let upstream_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let upstream_addr = upstream_listener.local_addr().unwrap();

    tokio::spawn(async move {
        let (stream, _) = upstream_listener.accept().await.unwrap();
        let mut conn = h2::server::handshake(stream).await.expect("upstream h2 handshake");
        while let Some(result) = conn.accept().await {
            let (req, mut respond) = result.expect("upstream accept");
            tokio::spawn(async move {
                // Read FULL request body, then respond with its length
                let mut total = 0usize;
                let mut body = req.into_body();
                while let Some(chunk) = body.data().await {
                    let chunk = chunk.expect("upstream request chunk");
                    let _ = body.flow_control().release_capacity(chunk.len());
                    total += chunk.len();
                }
                let response = http::Response::builder().status(200).body(()).unwrap();
                let mut send = respond.send_response(response, false).unwrap();
                send.send_data(Bytes::from(format!("len: {total}")), true).unwrap();
            });
        }
    });

    let tmp = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let proxy_addr = tmp.local_addr().unwrap();
    drop(tmp);

    let config = CoreConfig {
        rules: vec![Rule {
            name: "h2-big-req".into(),
            listen: Endpoint::tcp(proxy_addr),
            target: Endpoint::tcp(upstream_addr),
            filters: vec![],
            duplicate: None,
            exporter: None,
            tls: None,
            udp_source_mode: harpoon_core::types::rule::UdpSourceMode::Proxy,
            http2: true,
            idle_timeout_secs: 30,
        }],
        ..CoreConfig::default()
    };

    let handle = harpoon_core::run(config).await.unwrap();
    tokio::time::sleep(Duration::from_millis(50)).await;

    // H2 client: send a 200KB request body in 16KB chunks (realistic framing)
    let tcp = TcpStream::connect(proxy_addr).await.unwrap();
    let (mut h2_client, h2_conn) = tokio::time::timeout(
        Duration::from_secs(5),
        h2::client::handshake(tcp),
    )
    .await
    .expect("H2 client handshake timed out")
    .expect("H2 client handshake failed");

    tokio::spawn(async move {
        let _ = h2_conn.await;
    });

    let mut h2_client = h2_client.ready().await.unwrap();

    let request = http::Request::builder()
        .method("POST")
        .uri("http://localhost/big-request")
        .body(())
        .unwrap();

    let (response_future, mut send_stream) = h2_client.send_request(request, false).unwrap();

    let payload = vec![b'x'; BODY];
    for chunk in payload.chunks(16 * 1024) {
        send_stream.send_data(Bytes::copy_from_slice(chunk), false).unwrap();
    }
    send_stream.send_data(Bytes::new(), true).unwrap();

    let response = tokio::time::timeout(Duration::from_secs(10), response_future)
        .await
        .expect("response HEADERS timed out: request body deadlocked on flow control")
        .expect("response error");

    assert_eq!(response.status(), http::StatusCode::OK);

    let mut body = response.into_body();
    let mut response_data = Vec::new();
    while let Some(chunk) = body.data().await {
        let chunk = chunk.unwrap();
        body.flow_control().release_capacity(chunk.len()).unwrap();
        response_data.extend_from_slice(&chunk);
    }

    assert_eq!(String::from_utf8(response_data).unwrap(), format!("len: {BODY}"));

    handle.stop();
    handle.shutdown().await;
}

/// 200KB response body roundtrip: upstream sends the response FIRST, then
/// drains the request. Asserts the client receives all 204800 body bytes.
#[cfg(feature = "http2")]
#[tokio::test]
async fn test_http2_proxy_large_response_body() {
    const BODY: usize = 200 * 1024;

    let upstream_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let upstream_addr = upstream_listener.local_addr().unwrap();

    tokio::spawn(async move {
        let (stream, _) = upstream_listener.accept().await.unwrap();
        let mut conn = h2::server::handshake(stream).await.expect("upstream h2 handshake");
        while let Some(result) = conn.accept().await {
            let (req, mut respond) = result.expect("upstream accept");
            tokio::spawn(async move {
                // Send the 200KB response first, then drain the request body
                let response = http::Response::builder().status(200).body(()).unwrap();
                let mut send = respond.send_response(response, false).unwrap();
                let payload = vec![b'y'; BODY];
                for chunk in payload.chunks(16 * 1024) {
                    send.send_data(Bytes::copy_from_slice(chunk), false).unwrap();
                }
                send.send_data(Bytes::new(), true).unwrap();

                let mut body = req.into_body();
                while let Some(chunk) = body.data().await {
                    let chunk = chunk.expect("upstream request chunk");
                    let _ = body.flow_control().release_capacity(chunk.len());
                }
            });
        }
    });

    let tmp = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let proxy_addr = tmp.local_addr().unwrap();
    drop(tmp);

    let config = CoreConfig {
        rules: vec![Rule {
            name: "h2-big-resp".into(),
            listen: Endpoint::tcp(proxy_addr),
            target: Endpoint::tcp(upstream_addr),
            filters: vec![],
            duplicate: None,
            exporter: None,
            tls: None,
            udp_source_mode: harpoon_core::types::rule::UdpSourceMode::Proxy,
            http2: true,
            idle_timeout_secs: 30,
        }],
        ..CoreConfig::default()
    };

    let handle = harpoon_core::run(config).await.unwrap();
    tokio::time::sleep(Duration::from_millis(50)).await;

    let tcp = TcpStream::connect(proxy_addr).await.unwrap();
    let (mut h2_client, h2_conn) = tokio::time::timeout(
        Duration::from_secs(5),
        h2::client::handshake(tcp),
    )
    .await
    .expect("H2 client handshake timed out")
    .expect("H2 client handshake failed");

    tokio::spawn(async move {
        let _ = h2_conn.await;
    });

    let mut h2_client = h2_client.ready().await.unwrap();

    let request = http::Request::builder()
        .method("POST")
        .uri("http://localhost/big-response")
        .body(())
        .unwrap();

    let (response_future, mut send_stream) = h2_client.send_request(request, false).unwrap();

    let payload = vec![b'x'; BODY];
    for chunk in payload.chunks(16 * 1024) {
        send_stream.send_data(Bytes::copy_from_slice(chunk), false).unwrap();
    }
    send_stream.send_data(Bytes::new(), true).unwrap();

    let response = tokio::time::timeout(Duration::from_secs(10), response_future)
        .await
        .expect("response HEADERS timed out")
        .expect("response error");

    assert_eq!(response.status(), http::StatusCode::OK);

    // Read the full 200KB response body
    let mut body = response.into_body();
    let mut total = 0usize;
    let read_all = tokio::time::timeout(Duration::from_secs(10), async {
        while let Some(chunk) = body.data().await {
            let chunk = chunk.expect("client response chunk");
            if !chunk.is_empty() {
                assert_eq!(chunk[0], b'y');
            }
            body.flow_control().release_capacity(chunk.len()).unwrap();
            total += chunk.len();
        }
    })
    .await;
    assert!(read_all.is_ok(), "response body read timed out at {total} bytes");
    assert_eq!(total, BODY, "client must receive all {BODY} response body bytes");

    handle.stop();
    handle.shutdown().await;
}

/// Upstream responds immediately from headers and NEVER reads the request
/// body. The client (still sending its 200KB body) must still receive the
/// response — exercises the stream-reset / release flow-control paths when
/// the proxy's body-forward loop is interrupted mid-stream.
#[cfg(feature = "http2")]
#[tokio::test]
async fn test_http2_proxy_upstream_never_reads_body() {
    const BODY: usize = 200 * 1024;

    let upstream_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let upstream_addr = upstream_listener.local_addr().unwrap();

    tokio::spawn(async move {
        let (stream, _) = upstream_listener.accept().await.unwrap();
        let mut conn = h2::server::handshake(stream).await.expect("upstream h2 handshake");
        while let Some(result) = conn.accept().await {
            let (req, mut respond) = result.expect("upstream accept");
            tokio::spawn(async move {
                // Respond without ever reading the request body; `req` is
                // dropped (resetting the stream) when this task ends
                let response = http::Response::builder().status(200).body(()).unwrap();
                let mut send = respond.send_response(response, false).unwrap();
                send.send_data(Bytes::from_static(b"ok-no-read"), true).unwrap();
                drop(req);
            });
        }
    });

    let tmp = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let proxy_addr = tmp.local_addr().unwrap();
    drop(tmp);

    let config = CoreConfig {
        rules: vec![Rule {
            name: "h2-no-read".into(),
            listen: Endpoint::tcp(proxy_addr),
            target: Endpoint::tcp(upstream_addr),
            filters: vec![],
            duplicate: None,
            exporter: None,
            tls: None,
            udp_source_mode: harpoon_core::types::rule::UdpSourceMode::Proxy,
            http2: true,
            idle_timeout_secs: 30,
        }],
        ..CoreConfig::default()
    };

    let handle = harpoon_core::run(config).await.unwrap();
    tokio::time::sleep(Duration::from_millis(50)).await;

    let tcp = TcpStream::connect(proxy_addr).await.unwrap();
    let (mut h2_client, h2_conn) = tokio::time::timeout(
        Duration::from_secs(5),
        h2::client::handshake(tcp),
    )
    .await
    .expect("H2 client handshake timed out")
    .expect("H2 client handshake failed");

    tokio::spawn(async move {
        let _ = h2_conn.await;
    });

    let mut h2_client = h2_client.ready().await.unwrap();

    let request = http::Request::builder()
        .method("POST")
        .uri("http://localhost/never-read")
        .body(())
        .unwrap();

    let (response_future, mut send_stream) = h2_client.send_request(request, false).unwrap();

    let payload = vec![b'x'; BODY];
    for chunk in payload.chunks(16 * 1024) {
        send_stream.send_data(Bytes::copy_from_slice(chunk), false).unwrap();
    }
    send_stream.send_data(Bytes::new(), true).unwrap();

    let response = tokio::time::timeout(Duration::from_secs(10), response_future)
        .await
        .expect("response timed out: upstream reset should abort body forwarding, not hang")
        .expect("response error");

    assert_eq!(response.status(), http::StatusCode::OK);

    let mut body = response.into_body();
    let mut response_data = Vec::new();
    while let Some(chunk) = body.data().await {
        let chunk = chunk.unwrap();
        body.flow_control().release_capacity(chunk.len()).unwrap();
        response_data.extend_from_slice(&chunk);
    }

    assert_eq!(String::from_utf8(response_data).unwrap(), "ok-no-read");

    handle.stop();
    handle.shutdown().await;
}
