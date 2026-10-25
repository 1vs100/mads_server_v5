//! Full request sequence against a running server.

use std::path::Path;

use massive_proto::crypto::{MacAddress, MacKey};
use massive_proto::ids::{HostName, Port, SkuName, SkuVersion, ZoneName};
use massived::capture::{self, Direction};
use massived::config::{Advertise, Config};
use massived::decode::{self, Input};
use massived::handler::Handler;
use massived::probe::{self, ProbeError, ProbeOptions};
use massived::server::Server;

fn write_config(dir: &Path, mac_policy: &str) -> Config {
    std::fs::create_dir(dir.join("content")).unwrap();
    std::fs::write(dir.join("content/a.bin"), b"texture bytes").unwrap();
    std::fs::write(dir.join("content/b.bin"), b"video bytes").unwrap();
    let text = format!(
        r#"
[protocol]
mac_policy = "{mac_policy}"

[capture]
path = "capture.jsonl"

[content]
directory = "content"

[[asset]]
id = 1
file = "a.bin"
asset_type = 0

[[asset]]
id = 2
file = "b.bin"
asset_type = 0

[[zone]]
name = "Lobby_PT"
  [[zone.order]]
  order_id = 7
  assets = [1, 2]
  [[zone.placement]]
  ie_name = "Text_1"
  ie_id = 1
  media_type = 1
  assets = [1]
  [[zone.placement]]
  ie_name = "Video15_1"
  ie_id = 2
  media_type = 0x401
  rotation_type = 0x10
  assets = [2]
"#
    );
    let path = dir.join("massived.toml");
    std::fs::write(&path, text).unwrap();
    Config::from_file(&path).unwrap()
}

fn advertise() -> Advertise {
    Advertise {
        host: HostName::new("127.0.0.1").unwrap(),
        port: Port(1),
    }
}

fn options(target: std::net::SocketAddr, zone: &str) -> ProbeOptions {
    ProbeOptions {
        target,
        zone: ZoneName::new(zone).unwrap(),
        mac_address: "00:11:22:33:44:55".parse::<MacAddress>().unwrap(),
        sku_name: SkuName::new("mgs_1_vs_100_x360").unwrap(),
        sku_version: SkuVersion::new("edge").unwrap(),
    }
}

#[tokio::test]
async fn probe_passes_and_capture_decodes() {
    let dir = tempfile::tempdir().unwrap();
    let config = write_config(dir.path(), "enforce");
    let server = Server::bind(&config, &["127.0.0.1:0".parse().unwrap()], &advertise())
        .await
        .unwrap();
    let target = server.local_addrs()[0];
    let (stop_tx, stop_rx) = tokio::sync::oneshot::channel::<()>();
    let serving = tokio::spawn(server.serve(async {
        let _ = stop_rx.await;
    }));

    let report = probe::run(&options(target, "Lobby_PT")).await.unwrap();
    assert_eq!(report.zone.maos.len(), 2);
    assert_eq!(report.zone.orders[0].assets.len(), 2);
    assert_eq!(report.downloads, 2);
    assert!(
        report
            .session
            .zones
            .contains(&ZoneName::new("Lobby_PT").unwrap())
    );

    let err = probe::run(&options(target, "Unknown_Zone"))
        .await
        .unwrap_err();
    assert!(matches!(err, ProbeError::ZoneNotAnnounced { .. }), "{err}");

    stop_tx.send(()).unwrap();
    serving.await.unwrap();

    let records = capture::read(&dir.path().join("capture.jsonl")).unwrap();
    let inbound = records
        .iter()
        .filter(|(r, _)| r.direction == Direction::In)
        .count();
    assert_eq!(inbound, records.len() - inbound);
    // locate, open, player update, enter zone, two downloads, impression,
    // heartbeat, statistics, close; then locate and open of the second run
    assert_eq!(inbound, 12);

    let mut out = Vec::new();
    decode::run(
        &Input::Capture(dir.path().join("capture.jsonl")),
        &MacKey::retail(),
        &mut out,
    )
    .unwrap();
    let text = String::from_utf8(out).unwrap();
    assert!(text.contains("EnterZoneResponse"), "{text}");
    assert!(!text.contains("INVALID"), "{text}");
    assert!(!text.contains("rejected by client rules"), "{text}");
}

#[tokio::test]
async fn bad_mac_is_refused_when_enforced() {
    let dir = tempfile::tempdir().unwrap();
    let config = write_config(dir.path(), "enforce");
    let server = Server::bind(&config, &["127.0.0.1:0".parse().unwrap()], &advertise())
        .await
        .unwrap();
    let target = server.local_addrs()[0];
    let (stop_tx, stop_rx) = tokio::sync::oneshot::channel::<()>();
    let serving = tokio::spawn(server.serve(async {
        let _ = stop_rx.await;
    }));

    let request = massive_proto::Request::Heartbeat(massive_proto::request::HeartbeatRequest {
        massive_player_id: massive_proto::ids::MassivePlayerId(1),
        session_id: massive_proto::ids::SessionId(1),
        time: massive_proto::ids::ClientTime(0),
    });
    let body = request.encode(&MacKey::new(b"wrong key".to_vec())).unwrap();
    let mut message = massive_proto::transport::request_head(
        massive_proto::transport::Method::Post,
        "/impsrv/5/heartbeat",
        "127.0.0.1",
        target.port(),
        body.len(),
    )
    .into_bytes();
    message.extend_from_slice(&body);
    let mut stream = tokio::net::TcpStream::connect(target).await.unwrap();
    let response = massived::http::exchange(&mut stream, &message)
        .await
        .unwrap();
    assert_eq!(response.code, 400);

    stop_tx.send(()).unwrap();
    serving.await.unwrap();
}

#[tokio::test]
async fn serves_on_caller_bound_blocking_listener() {
    let dir = tempfile::tempdir().unwrap();
    let config = write_config(dir.path(), "enforce");
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let target = listener.local_addr().unwrap();
    let advertise = Advertise {
        host: HostName::new("127.0.0.1").unwrap(),
        port: Port(target.port()),
    };
    let handler = Handler::new(&config, &advertise).unwrap();
    let server = Server::with_std_listeners(&config, handler, vec![listener])
        .await
        .unwrap();
    let (stop_tx, stop_rx) = tokio::sync::oneshot::channel::<()>();
    let serving = tokio::spawn(server.serve(async {
        let _ = stop_rx.await;
    }));

    let report = probe::run(&options(target, "Lobby_PT")).await.unwrap();
    assert_eq!(report.downloads, 2);

    stop_tx.send(()).unwrap();
    serving.await.unwrap();
}
