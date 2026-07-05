use redis::Value as RedisValue;
use serde_json::json;

use super::redis_response::{
    CommandResponse, PipelineResponse, RedisResponse, TransactionResponse,
};

fn bulk(value: &str) -> RedisValue {
    RedisValue::BulkString(value.as_bytes().to_vec())
}

#[test]
fn command() {
    let result = bulk("pong");

    assert_eq!(
        serde_json::to_value(CommandResponse {
            result: &result,
            base64_encoding: false,
        })
        .unwrap(),
        json!({
            "result": "pong"
        })
    );
}

#[test]
fn command_base64() {
    let result = bulk("pong");

    assert_eq!(
        serde_json::to_value(CommandResponse {
            result: &result,
            base64_encoding: true,
        })
        .unwrap(),
        json!({
            "result": "cG9uZw=="
        })
    );
}

#[test]
fn pipeline_result() {
    let items = vec![RedisResponse::Result(RedisValue::Okay)];

    assert_eq!(
        serde_json::to_value(PipelineResponse {
            items: &items,
            base64_encoding: false,
        })
        .unwrap(),
        json!([
            {
                "result": "OK"
            }
        ])
    );
}

#[test]
fn pipeline_error() {
    let items = vec![RedisResponse::Error("ERR wrong type".to_string())];

    assert_eq!(
        serde_json::to_value(PipelineResponse {
            items: &items,
            base64_encoding: false,
        })
        .unwrap(),
        json!([
            {
                "error": "ERR wrong type"
            }
        ])
    );
}

#[test]
fn pipeline_mixed() {
    let items = vec![
        RedisResponse::Result(bulk("one")),
        RedisResponse::Error("ERR wrong type".to_string()),
        RedisResponse::Result(RedisValue::Int(3)),
    ];

    assert_eq!(
        serde_json::to_value(PipelineResponse {
            items: &items,
            base64_encoding: false,
        })
        .unwrap(),
        json!([
            {
                "result": "one"
            },
            {
                "error": "ERR wrong type"
            },
            {
                "result": 3
            }
        ])
    );
}

#[test]
fn transaction() {
    let values = vec![bulk("one"), RedisValue::Int(2), RedisValue::Okay];

    assert_eq!(
        serde_json::to_value(TransactionResponse {
            values: &values,
            base64_encoding: false,
        })
        .unwrap(),
        json!([
            {
                "result": "one"
            },
            {
                "result": 2
            },
            {
                "result": "OK"
            }
        ])
    );
}
