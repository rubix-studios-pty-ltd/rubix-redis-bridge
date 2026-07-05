use num_bigint::BigInt;
use redis::{PushKind, Value as RedisValue, VerbatimFormat};
use serde_json::{Value, json};

use super::redis_value::RedisJson;

fn bulk(value: &str) -> RedisValue {
    RedisValue::BulkString(value.as_bytes().to_vec())
}

fn json_value(value: RedisValue) -> Value {
    serde_json::to_value(RedisJson::new(&value, false)).unwrap()
}

fn json_value_base64(value: RedisValue) -> Value {
    serde_json::to_value(RedisJson::new(&value, true)).unwrap()
}

#[test]
fn nil() {
    assert_eq!(json_value(RedisValue::Nil), Value::Null);
}

#[test]
fn int() {
    assert_eq!(json_value(RedisValue::Int(42)), json!(42));
}

#[test]
fn bulk_string_utf8() {
    assert_eq!(json_value(bulk("hello")), json!("hello"));
}

#[test]
fn bulk_string_binary() {
    assert_eq!(
        json_value(RedisValue::BulkString(vec![0, 159, 146, 150])),
        json!({
            "type": "binary",
            "encoding": "base64",
            "value": "AJ+Slg=="
        })
    );
}

#[test]
fn bulk_string_base64() {
    assert_eq!(json_value_base64(bulk("hello")), json!("aGVsbG8="));
}

#[test]
fn simple_string() {
    assert_eq!(
        json_value(RedisValue::SimpleString("PONG".to_string())),
        json!("PONG")
    );
}

#[test]
fn simple_string_base64() {
    assert_eq!(
        json_value_base64(RedisValue::SimpleString("PONG".to_string())),
        json!("UE9ORw==")
    );
}

#[test]
fn okay() {
    assert_eq!(json_value(RedisValue::Okay), json!("OK"));
}

#[test]
fn array() {
    assert_eq!(
        json_value(RedisValue::Array(vec![
            bulk("one"),
            RedisValue::Int(2),
            RedisValue::Nil,
        ])),
        json!(["one", 2, null])
    );
}

#[test]
fn set_array() {
    assert_eq!(
        json_value(RedisValue::Set(vec![bulk("a"), bulk("b")])),
        json!(["a", "b"])
    );
}

#[test]
fn map_json_object() {
    assert_eq!(
        json_value(RedisValue::Map(vec![
            (bulk("a"), RedisValue::Int(1)),
            (bulk("b"), bulk("two")),
        ])),
        json!({
            "a": 1,
            "b": "two"
        })
    );
}

#[test]
fn map_non_object_key() {
    assert_eq!(
        json_value(RedisValue::Map(vec![(
            RedisValue::Array(vec![bulk("key")]),
            bulk("value"),
        )])),
        json!({
            "type": "map",
            "value": [
                [
                    ["key"],
                    "value"
                ]
            ]
        })
    );
}

#[test]
fn map_duplicate_keys() {
    assert_eq!(
        json_value(RedisValue::Map(vec![
            (bulk("a"), RedisValue::Int(1)),
            (bulk("a"), RedisValue::Int(2)),
        ])),
        json!({
            "type": "map",
            "value": [
                ["a", 1],
                ["a", 2]
            ]
        })
    );
}

#[test]
fn double_map_keys() {
    assert_eq!(
        json_value(RedisValue::Map(vec![
            (RedisValue::Double(1.0), bulk("one")),
            (RedisValue::Double(-0.0), bulk("negative-zero")),
            (RedisValue::Double(1.25e3), bulk("exponent")),
        ])),
        json!({
            "1.0": "one",
            "-0.0": "negative-zero",
            "1250.0": "exponent"
        })
    );
}

#[test]
fn attribute() {
    assert_eq!(
        json_value(RedisValue::Attribute {
            data: Box::new(bulk("payload")),
            attributes: vec![(bulk("ttl"), RedisValue::Int(60))],
        }),
        json!({
            "type": "attribute",
            "data": "payload",
            "attributes": {
                "ttl": 60
            }
        })
    );
}

#[test]
fn finite_double() {
    assert_eq!(json_value(RedisValue::Double(1.25)), json!(1.25));
}

#[test]
fn nan_double_tagged() {
    assert_eq!(
        json_value(RedisValue::Double(f64::NAN)),
        json!({
            "type": "double",
            "value": "NaN"
        })
    );
}

#[test]
fn infinite_double_tagged() {
    assert_eq!(
        json_value(RedisValue::Double(f64::INFINITY)),
        json!({
            "type": "double",
            "value": "inf"
        })
    );

    assert_eq!(
        json_value(RedisValue::Double(f64::NEG_INFINITY)),
        json!({
            "type": "double",
            "value": "-inf"
        })
    );
}

#[test]
fn boolean() {
    assert_eq!(json_value(RedisValue::Boolean(true)), json!(true));
    assert_eq!(json_value(RedisValue::Boolean(false)), json!(false));
}

#[test]
fn verbatim_string() {
    assert_eq!(
        json_value(RedisValue::VerbatimString {
            format: VerbatimFormat::Text,
            text: "hello".to_string(),
        }),
        json!({
            "type": "verbatim_string",
            "format": "Text",
            "value": "hello"
        })
    );
}

#[test]
fn big_number() {
    let value = BigInt::parse_bytes(b"123456789012345678901234567890", 10).unwrap();

    assert_eq!(
        json_value(RedisValue::BigNumber(value)),
        json!("123456789012345678901234567890")
    );
}

#[test]
fn push() {
    assert_eq!(
        json_value(RedisValue::Push {
            kind: PushKind::Message,
            data: vec![bulk("channel"), bulk("payload")],
        }),
        json!({
            "type": "push",
            "kind": "Message",
            "data": ["channel", "payload"]
        })
    );
}
