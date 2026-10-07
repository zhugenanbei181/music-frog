use super::*;
use infiltrator_contract::dns_query::DnsRecordType;
use mockito::Server;
use serde_json::{Value, json};

fn payload() -> Value {
    json!({"Status": 0, "Question": [{"Name": "music.test.", "Qtype": 16, "Qclass": 1}],
        "TC": false, "RD": true, "RA": true, "AD": false, "CD": true,
        "Answer": [{"name": "music.test.", "type": 16, "TTL": 42, "data": "\"original {ttl}\""}],
        "Authority": [{"name": "test.", "type": 2, "TTL": 600, "data": "ns.test."}],
        "Additional": [{"name": "ns.test.", "type": 1, "TTL": 0, "data": "192.0.2.53"}]})
}
fn request() -> DnsQueryRequest {
    DnsQueryRequest {
        name: "music.test".into(),
        record_type: DnsRecordType::Txt,
    }
}
#[tokio::test]
async fn loopback_controller_query_sends_exact_identity_auth_and_preserves_every_response_section()
{
    let mut server = Server::new_async().await;
    let expected = server
        .mock("GET", "/dns/query?name=music.test&type=TXT")
        .match_header("authorization", "Bearer fixture-only")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(payload().to_string())
        .create_async()
        .await;
    let client = MihomoClient::new(&server.url(), Some("fixture-only".into())).unwrap();
    let response = DnsQueryPort::query(&client, &request()).await.unwrap();
    expected.assert_async().await;
    assert_eq!(
        response.questions[0],
        DnsQuestion {
            name: "music.test.".into(),
            record_type: 16,
            class: 1
        }
    );
    assert_eq!(response.answers[0].data, "\"original {ttl}\"");
    assert_eq!(response.answers[0].ttl, 42);
    assert_eq!(response.authority[0].data, "ns.test.");
    assert_eq!(response.additional[0].ttl, 0);
    assert!(
        response.recursion_desired && response.recursion_available && response.checking_disabled
    );
    assert!(!response.truncated && !response.authenticated_data);
}
#[tokio::test]
async fn malformed_or_unrelated_controller_answers_and_http_refusals_keep_typed_failure_identity() {
    let mut server = Server::new_async().await;
    let client = MihomoClient::new(&server.url(), None).unwrap();
    for (status, body, code) in [
        (401, "{}", ErrorCode::Authentication),
        (403, "{}", ErrorCode::Permission),
        (400, "{}", ErrorCode::InvalidInput),
        (500, "{}", ErrorCode::Network),
        (200, "not JSON", ErrorCode::InvalidState),
        (200, "{}", ErrorCode::InvalidState),
    ] {
        let expected = server
            .mock("GET", "/dns/query?name=music.test&type=TXT")
            .with_status(status)
            .with_header("content-type", "application/json")
            .with_body(body)
            .create_async()
            .await;
        let failure = Failure::from(DnsQueryPort::query(&client, &request()).await.unwrap_err());
        assert_eq!(failure.code, code);
        expected.assert_async().await;
        expected.remove_async().await;
    }
    for mismatch in [
        json!([{"Name":"another.test.","Qtype":16,"Qclass":1}]),
        json!([{"Name":"music.test.","Qtype":1,"Qclass":1}]),
        json!([]),
    ] {
        let mut answer = payload();
        answer["Question"] = mismatch;
        let expected = server
            .mock("GET", "/dns/query?name=music.test&type=TXT")
            .with_status(200)
            .with_body(answer.to_string())
            .create_async()
            .await;
        assert_eq!(
            Failure::from(DnsQueryPort::query(&client, &request()).await.unwrap_err()).code,
            ErrorCode::InvalidState
        );
        expected.assert_async().await;
        expected.remove_async().await;
    }
    let no_call = server
        .mock("GET", "/dns/query")
        .expect(0)
        .create_async()
        .await;
    let bad = DnsQueryRequest {
        name: "a&name=injected".into(),
        record_type: DnsRecordType::A,
    };
    assert_eq!(
        Failure::from(DnsQueryPort::query(&client, &bad).await.unwrap_err()).code,
        ErrorCode::InvalidInput
    );
    no_call.assert_async().await;
}
#[tokio::test]
async fn a_negative_dns_response_and_omitted_empty_sections_are_observed_facts_not_transport_errors()
 {
    let mut server = Server::new_async().await;
    let mut answer = payload();
    answer["Status"] = json!(3);
    answer.as_object_mut().unwrap().remove("Answer");
    answer.as_object_mut().unwrap().remove("Additional");
    let expected = server
        .mock("GET", "/dns/query?name=music.test&type=TXT")
        .with_status(200)
        .with_body(answer.to_string())
        .create_async()
        .await;
    let client = MihomoClient::new(&server.url(), None).unwrap();
    let response = DnsQueryPort::query(&client, &request()).await.unwrap();
    expected.assert_async().await;
    assert_eq!(response.status, 3);
    assert!(response.answers.is_empty() && response.additional.is_empty());
    assert_eq!(response.authority[0].record_type, 2);
}
