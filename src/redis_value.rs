use base64::Engine;
use redis::Value as RedisValue;
use serde_json::{Map, Number, Value, json};
use tracing::warn;

pub fn encode_redis_value(value: RedisValue, base64_encoding: bool) -> Value {
    match value {
        RedisValue::Nil => Value::Null,
        RedisValue::Int(value) => json!(value),
        RedisValue::BulkString(value) => encode_bytes(value, base64_encoding),
        RedisValue::Array(values) => encode_array(values, base64_encoding),
        RedisValue::SimpleString(value) => encode_simple_string(value, base64_encoding),
        RedisValue::Okay => json!("OK"),

        RedisValue::Map(entries) => encode_map(entries, base64_encoding),

        RedisValue::Attribute { data, attributes } => json!({
            "type": "attribute",
            "data": encode_redis_value(*data, base64_encoding),
            "attributes": encode_map(attributes, base64_encoding),
        }),

        RedisValue::Set(values) => Value::Array(
            values
                .into_iter()
                .map(|value| encode_redis_value(value, base64_encoding))
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

        other => {
            let _ = other;

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
            .map(|value| encode_redis_value(value, base64_encoding))
            .collect(),
    )
}

fn encode_map(entries: Vec<(RedisValue, RedisValue)>, base64_encoding: bool) -> Value {
    let mut object = Map::new();
    let mut pairs = Vec::with_capacity(entries.len());
    let mut can_encode_as_object = true;

    for (key, value) in entries {
        let encoded_key = encode_redis_value(key, base64_encoding);
        let encoded_value = encode_redis_value(value, base64_encoding);

        if let Some(object_key) = json_to_object(&encoded_key) {
            if object.insert(object_key, encoded_value.clone()).is_some() {
                can_encode_as_object = false;
            }
        } else {
            can_encode_as_object = false;
        }

        pairs.push(Value::Array(vec![encoded_key, encoded_value]));
    }

    if can_encode_as_object {
        Value::Object(object)
    } else {
        json!({
            "type": "map",
            "value": pairs,
        })
    }
}

fn json_to_object(value: &Value) -> Option<String> {
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

fn encode_big_number<T: ToString>(value: T) -> Value {
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

fn encode_simple_string(value: String, base64_encoding: bool) -> Value {
    if base64_encoding && value != "OK" {
        return json!(base64::engine::general_purpose::STANDARD.encode(value.as_bytes()));
    }

    json!(value)
}
