use std::iter;
use std::time::Duration;
use teamy_broker::broker::synthetic_credential_fixture;
use teamy_broker::platform::pipe_security;
use teamy_broker::protocol::CAPABILITIES;
use teamy_broker::protocol::Hello;
use teamy_broker::protocol::Request;
use teamy_broker::protocol::Response;
use teamy_broker::protocol::VERSION;
use teamy_broker::protocol::wire;
use teamy_broker::sdk::Client;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::sync::oneshot;
use tokio::time;

async fn stalled_exchange(authorize: bool, response: u8) {
    let endpoint = format!(
        r"\\.\pipe\teamy-phone-control-test-{}",
        uuid::Uuid::new_v4()
    );
    let mut pipe = pipe_security::create(&endpoint, true).unwrap();
    let (seen, wait) = oneshot::channel();
    let server = tokio::spawn(async move {
        pipe.connect().await.unwrap();
        wire::write(
            &mut pipe,
            &Hello {
                version: VERSION,
                instance_id: uuid::Uuid::new_v4().to_string(),
                capabilities: CAPABILITIES
                    .iter()
                    .map(|s| (*s).into())
                    .chain(iter::once("persistent-no-vault-phone-v1".into()))
                    .collect(),
            },
        )
        .await
        .unwrap();
        let _: Hello = wire::read(&mut pipe).await.unwrap();
        let request: Request = wire::read(&mut pipe).await.unwrap();
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
                wire::write(&mut pipe, &Response::code("phone-status"))
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
            time::timeout(Duration::from_secs(2), pipe.read(&mut byte))
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
    let result = time::timeout(Duration::from_secs(3), async {
        if authorize {
            client
                .phone_authorize_once(Request {
                    operation: "phone-authorize-fixture".into(),
                    acquire: Some(synthetic_credential_fixture::request()),
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
