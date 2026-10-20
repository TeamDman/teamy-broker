use std::time::Duration;
use teamy_broker::sdk::Client;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;

async fn stalled_exchange(authorize: bool, response: u8) {
    let endpoint = format!(
        r"\\.\pipe\teamy-phone-control-test-{}",
        uuid::Uuid::new_v4()
    );
    let mut pipe = teamy_broker::platform::pipe_security::create(&endpoint, true).unwrap();
    let (seen, wait) = tokio::sync::oneshot::channel();
    let server = tokio::spawn(async move {
        pipe.connect().await.unwrap();
        teamy_broker::protocol::wire::write(
            &mut pipe,
            &teamy_broker::protocol::Hello {
                version: teamy_broker::protocol::VERSION,
                instance_id: uuid::Uuid::new_v4().to_string(),
                capabilities: teamy_broker::protocol::CAPABILITIES
                    .iter()
                    .map(|s| (*s).into())
                    .chain(std::iter::once("persistent-no-vault-phone-v1".into()))
                    .collect(),
            },
        )
        .await
        .unwrap();
        let _: teamy_broker::protocol::Hello =
            teamy_broker::protocol::wire::read(&mut pipe).await.unwrap();
        let request: teamy_broker::protocol::Request =
            teamy_broker::protocol::wire::read(&mut pipe).await.unwrap();
        request.validate().unwrap();
        assert_eq!(
            request.operation,
            if authorize {
                "phone-authorize-fixture"
            } else {
                "phone-status"
            }
        );
        match response {
            1 => pipe.write_all(&[0, 0]).await.unwrap(),
            2 => {
                teamy_broker::protocol::wire::write(
                    &mut pipe,
                    &teamy_broker::protocol::Response::code("phone-status"),
                )
                .await
                .unwrap();
                pipe.write_u32(100).await.unwrap();
                pipe.write_all(b"{").await.unwrap();
            }
            _ => {}
        }
        seen.send(()).unwrap();
        let mut byte = [0];
        assert_eq!(
            tokio::time::timeout(Duration::from_secs(2), pipe.read(&mut byte))
                .await
                .unwrap()
                .unwrap(),
            0,
            "deadline must release the exchange pipe"
        );
    });
    let client = Client::new(None)
        .unwrap()
        .with_endpoint(endpoint)
        .with_deadlines(Duration::from_secs(1), Duration::from_millis(200));
    let result = tokio::time::timeout(Duration::from_secs(3), async {
        if authorize {
            client
                .phone_authorize_once(teamy_broker::protocol::Request {
                    operation: "phone-authorize-fixture".into(),
                    acquire: Some(teamy_broker::broker::synthetic_credential_fixture::request()),
                    lease_id: None,
                })
                .await
        } else {
            client.phone_control("phone-status").await
        }
    })
    .await
    .unwrap();
    assert_eq!(result.unwrap_err().to_string(), "phone exchange deadline");
    wait.await.unwrap();
    server.await.unwrap();
}

#[tokio::test]
async fn phone_control_total_deadline_bounds_stalled_and_partial_responses() {
    for response in 0..3 {
        stalled_exchange(false, response).await;
    }
}

#[tokio::test]
async fn phone_authorization_total_deadline_bounds_stalled_and_partial_responses() {
    for response in 0..3 {
        stalled_exchange(true, response).await;
    }
}
