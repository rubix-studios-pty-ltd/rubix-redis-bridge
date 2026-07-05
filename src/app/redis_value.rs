use base64::Engine;
use redis::Value as RedisValue;
use serde_json::{Map, Number, Value, json};
use tracing::warn;

pub fn encode_value(value: RedisValue, base64_encoding: bool) -> Value {
    match value {
        RedisValue::Nil => Value::Null,
        RedisValue::Int(value) => json!(value),
        RedisValue::BulkString(value) => encode_bytes(value, base64_encoding),
        RedisValue::Array(values) => encode_array(values, base64_encoding),
        RedisValue::SimpleString(value) => encode_string(value, base64_encoding),
        RedisValue::Okay => json!("OK"),

        RedisValue::Map(entries) => encode_map(entries, base64_encoding),

        RedisValue::Attribute { data, attributes } => json!({
            "type": "attribute",
            "data": encode_value(*data, base64_encoding),
            "attributes": encode_map(attributes, base64_encoding),
        }),

        RedisValue::Set(values) => Value::Array(
            values
                .into_iter()
                .map(|value| encode_value(value, base64_encoding))
                .collect(),
        ),

        RedisValue::Double(value) => encode_f64(value),
        RedisValue::Boolean(value) => json!(value),

        RedisValue::VerbatimString { format, text } => json!({
            "type": "verbatim_string",
            "format": format!("{format:?}"),
            "value": text,
        }),

        RedisValue::BigNumber(value) => encode_big_number(value),

        RedisValue::Push { kind, data } => json!({
            "type": "push",
            "kind": format!("{kind:?}"),
            "data": encode_array(data, base64_encoding),
        }),

        RedisValue::ServerError(error) => json!({
            "type": "server_error",
            "error": error.to_string(),
        }),

        _ => {
            warn!("unsupported RedisValue variant encountered");

            json!({
                "type": "unsupported_redis_value",
            })
        }
    }
}

fn encode_array(values: Vec<RedisValue>, base64_encoding: bool) -> Value {
    Value::Array(
        values
            .into_iter()
            .map(|value| encode_value(value, base64_encoding))
            .collect(),
    )
}

fn encode_map(entries: Vec<(RedisValue, RedisValue)>, base64_encoding: bool) -> Value {
    if can_encode_object(&entries, base64_encoding) {
        return Value::Object(encode_object(entries, base64_encoding));
    }

    let pairs = entries
        .into_iter()
        .map(|(key, value)| {
            Value::Array(vec![
                encode_value(key, base64_encoding),
                encode_value(value, base64_encoding),
            ])
        })
        .collect::<Vec<_>>();

    json!({
        "type": "map",
        "value": pairs,
    })
}

fn can_encode_object(entries: &[(RedisValue, RedisValue)], base64_encoding: bool) -> bool {
    let mut keys = std::collections::HashSet::with_capacity(entries.len());

    for (key, _) in entries {
        let encoded_key = encode_value(key.clone(), base64_encoding);
        let Some(object_key) = encode_object_key(&encoded_key) else {
            return false;
        };

        if !keys.insert(object_key) {
            return false;
        }
    }

    true
}

fn encode_object(
    entries: Vec<(RedisValue, RedisValue)>,
    base64_encoding: bool,
) -> Map<String, Value> {
    entries
        .into_iter()
        .filter_map(|(key, value)| {
            let encoded_key = encode_value(key, base64_encoding);
            let object_key = encode_object_key(&encoded_key)?;
            Some((object_key, encode_value(value, base64_encoding)))
        })
        .collect()
}

fn encode_object_key(value: &Value) -> Option<String> {
    match value {
        Value::String(value) => Some(value.clone()),
        Value::Number(value) => Some(value.to_string()),
        Value::Bool(value) => Some(value.to_string()),
        Value::Null => Some("null".to_string()),
        Value::Array(_) | Value::Object(_) => None,
    }
}

fn encode_f64(value: f64) -> Value {
    match Number::from_f64(value) {
        Some(number) => Value::Number(number),
        None => json!({
            "type": "double",
            "value": value.to_string(),
        }),
    }
}

fn encode_big_number(value: impl ToString) -> Value {
    json!(value.to_string())
}

fn encode_bytes(bytes: Vec<u8>, base64_encoding: bool) -> Value {
    if base64_encoding {
        return json!(base64::engine::general_purpose::STANDARD.encode(&bytes));
    }

    match String::from_utf8(bytes) {
        Ok(value) => json!(value),
        Err(error) => {
            let bytes = error.into_bytes();

            json!({
                "type": "binary",
                "encoding": "base64",
                "value": base64::engine::general_purpose::STANDARD.encode(&bytes),
            })
        }
    }
}

fn encode_string(value: String, base64_encoding: bool) -> Value {
    if base64_encoding && value != "OK" {
        return json!(base64::engine::general_purpose::STANDARD.encode(value.as_bytes()));
    }

    json!(value)
}
