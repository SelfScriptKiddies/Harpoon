use std::net::SocketAddr;
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream, UdpSocket};

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
            idle_timeout_secs: 30,
        }],
        ..CoreConfig::default()
    };

    let handle = harpoon_core::run(config).await.unwrap();
    tokio::time::sleep(Duration::from_millis(50)).await;

    let mut client = TcpStream::connect(proxy_addr).await.unwrap();
    client.write_all(b"test data 123").await.unwrap();

    let mut buf = [0u8; 64];
    let n = client.read(&mut buf).await.unwrap();
    assert_eq!(n, 13);

    tokio::time::sleep(Duration::from_millis(50)).await;

    let stats = handle.stats_snapshot();
    assert_eq!(stats[0].bytes_client_to_server, 13);
    assert_eq!(stats[0].bytes_server_to_client, 13);
    assert_eq!(stats[0].packets_client_to_server, 1);
    assert_eq!(stats[0].packets_server_to_client, 1);

    handle.stop();
    handle.shutdown().await;
}
