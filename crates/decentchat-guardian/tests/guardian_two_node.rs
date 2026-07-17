//! Bounded real-network coverage for Guardian ticket import and Willow replication.
//!
//! Ignored in the default suite because it requires permission to bind local UDP
//! sockets. CI or developers can run it with:
//! `cargo test -p decentchat-guardian --test guardian_two_node -- --ignored`.

use std::process::Command;
use std::time::Duration;

use decentchat_guardian::{GuardianNode, GuardianNodeConfig, RoomSession, SessionConfig};

fn node_config(path: &std::path::Path) -> GuardianNodeConfig {
    let mut config = GuardianNodeConfig::new(path);
    config.local_only = true;
    config
}

fn session_config() -> SessionConfig {
    SessionConfig {
        projection_interval: Duration::from_millis(50),
        join_timeout: Duration::from_secs(15),
        ..Default::default()
    }
}

async fn drive_until(
    left: &mut RoomSession,
    right: &mut RoomSession,
    predicate: impl Fn(&RoomSession, &RoomSession) -> bool,
) {
    tokio::time::timeout(Duration::from_secs(15), async {
        loop {
            let _ = tokio::join!(left.process_event(), right.process_event());
            if predicate(left, right) {
                break;
            }
        }
    })
    .await
    .expect("Guardian replicas did not converge before the deadline");
}

#[tokio::test]
#[ignore = "requires local UDP sockets"]
async fn ticket_import_bidirectional_history_presence_and_isolation() {
    let left_dir = tempfile::tempdir().unwrap();
    let right_dir = tempfile::tempdir().unwrap();
    let left_node = GuardianNode::open(node_config(left_dir.path()))
        .await
        .unwrap();
    let right_node = GuardianNode::open(node_config(right_dir.path()))
        .await
        .unwrap();

    let (mut left, _left_events) = left_node
        .create_room("shared", session_config())
        .await
        .unwrap();
    left.set_username("alice".into()).await.unwrap();
    let history = left.send_message("before join".into()).await.unwrap();
    let ticket = left.share_ticket().await.unwrap();

    let (mut right, _right_events) = right_node
        .join_room(&ticket, session_config())
        .await
        .unwrap();
    right.set_username("bob".into()).await.unwrap();
    drive_until(&mut left, &mut right, |left, right| {
        right
            .state()
            .messages
            .iter()
            .any(|message| message.id == history.id)
            && left
                .state()
                .members
                .values()
                .any(|member| member.nickname.as_deref() == Some("bob"))
    })
    .await;

    let reply = right.send_message("from bob".into()).await.unwrap();
    drive_until(&mut left, &mut right, |left, _| {
        left.state()
            .messages
            .iter()
            .any(|message| message.id == reply.id)
    })
    .await;

    let (mut isolated, _isolated_events) = left_node
        .create_room("isolated", session_config())
        .await
        .unwrap();
    let isolated_message = isolated.send_message("private".into()).await.unwrap();
    assert!(
        !left
            .state()
            .messages
            .iter()
            .any(|message| message.id == isolated_message.id)
    );

    right.leave().await.unwrap();
    tokio::time::timeout(Duration::from_secs(15), async {
        loop {
            let _ = left.process_event().await;
            if left
                .state()
                .members
                .get(&right_node.node_id())
                .is_some_and(|member| member.offline)
            {
                break;
            }
        }
    })
    .await
    .expect("graceful leave did not replicate");

    let _ = left.leave().await;
    let _ = isolated.leave().await;
    left_node.shutdown().await.unwrap();
    right_node.shutdown().await.unwrap();

    // Guardian holds redb resources for the lifetime of its process. Exercise a
    // true process restart so operating-system teardown releases those handles.
    let persistence_dir = tempfile::tempdir().unwrap();
    run_helper("persistence_writer_helper", persistence_dir.path());
    run_helper("persistence_reader_helper", persistence_dir.path());
}

fn run_helper(name: &str, data_dir: &std::path::Path) {
    let status = Command::new(std::env::current_exe().unwrap())
        .arg(name)
        .arg("--ignored")
        .env("DECENTCHAT_GUARDIAN_TEST_DIR", data_dir)
        .status()
        .unwrap();
    assert!(status.success(), "restart helper {name} failed");
}

#[tokio::test]
#[ignore = "subprocess helper"]
async fn persistence_writer_helper() {
    let Some(path) = std::env::var_os("DECENTCHAT_GUARDIAN_TEST_DIR") else {
        return;
    };
    let node = GuardianNode::open(node_config(std::path::Path::new(&path)))
        .await
        .unwrap();
    std::fs::write(
        std::path::Path::new(&path).join("expected-node-id"),
        node.node_id().to_hex(),
    )
    .unwrap();
    let (mut room, _events) = node
        .create_room("persistent", session_config())
        .await
        .unwrap();
    room.send_message("survives restart".into()).await.unwrap();
    let _ = room.leave().await;
    node.shutdown().await.unwrap();
}

#[tokio::test]
#[ignore = "subprocess helper"]
async fn persistence_reader_helper() {
    let Some(path) = std::env::var_os("DECENTCHAT_GUARDIAN_TEST_DIR") else {
        return;
    };
    let path = std::path::Path::new(&path);
    let expected_node_id = std::fs::read_to_string(path.join("expected-node-id")).unwrap();
    let node = GuardianNode::open(node_config(path)).await.unwrap();
    assert_eq!(node.node_id().to_hex(), expected_node_id);
    let (mut room, _events) = node
        .create_room("persistent", session_config())
        .await
        .unwrap();
    assert!(
        room.state()
            .messages
            .iter()
            .any(|message| message.content == "survives restart")
    );
    let _ = room.leave().await;
    node.shutdown().await.unwrap();
}
