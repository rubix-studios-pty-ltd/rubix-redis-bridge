use base64::Engine;
use redis::Value as RedisValue;
use serde::ser::{SerializeMap, SerializeSeq};
use serde::{Serialize, Serializer};
use tracing::warn;

pub(crate) struct RedisJson<'a> {
    value: &'a RedisValue,
    base64_encoding: bool,
}

impl<'a> RedisJson<'a> {
    pub(crate) fn new(value: &'a RedisValue, base64_encoding: bool) -> Self {
        Self {
            value,
            base64_encoding,
        }
    }
}

impl Serialize for RedisJson<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serialize_redis(self.value, self.base64_encoding, serializer)
    }
}

fn serialize_redis<S>(
    value: &RedisValue,
    base64_encoding: bool,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    match value {
        RedisValue::Nil => serializer.serialize_none(),
        RedisValue::Int(value) => serializer.serialize_i64(*value),
        RedisValue::BulkString(bytes) => serialize_bytes(bytes, base64_encoding, serializer),
        RedisValue::Array(values) => serialize_array(values, base64_encoding, serializer),
        RedisValue::SimpleString(value) => serialize_string(value, base64_encoding, serializer),
        RedisValue::Okay => serializer.serialize_str("OK"),
        RedisValue::Map(entries) => serialize_map(entries, base64_encoding, serializer),
        RedisValue::Set(values) => serialize_array(values, base64_encoding, serializer),
        RedisValue::Double(value) => serialize_f64(*value, serializer),
        RedisValue::Boolean(value) => serializer.serialize_bool(*value),
        RedisValue::BigNumber(value) => serializer.serialize_str(&value.to_string()),

        RedisValue::Attribute { data, attributes } => {
            let mut map = serializer.serialize_map(Some(3))?;
            map.serialize_entry("type", "attribute")?;
            map.serialize_entry("data", &RedisJson::new(data, base64_encoding))?;
            map.serialize_entry("attributes", &RedisMap::new(attributes, base64_encoding))?;
            map.end()
        }

        RedisValue::VerbatimString { format, text } => {
            let mut map = serializer.serialize_map(Some(3))?;
            map.serialize_entry("type", "verbatim_string")?;
            map.serialize_entry("format", &format!("{format:?}"))?;
            map.serialize_entry("value", text)?;
            map.end()
        }

        RedisValue::Push { kind, data } => {
            let mut map = serializer.serialize_map(Some(3))?;
            map.serialize_entry("type", "push")?;
            map.serialize_entry("kind", &format!("{kind:?}"))?;
            map.serialize_entry("data", &RedisArray::new(data, base64_encoding))?;
            map.end()
        }

        RedisValue::ServerError(error) => {
            let mut map = serializer.serialize_map(Some(2))?;
            map.serialize_entry("type", "server_error")?;
            map.serialize_entry("error", &error.to_string())?;
            map.end()
        }

        _ => {
            warn!("unsupported RedisValue variant encountered");

            let mut map = serializer.serialize_map(Some(1))?;
            map.serialize_entry("type", "unsupported_redis_value")?;
            map.end()
        }
    }
}

fn serialize_f64<S>(value: f64, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    if value.is_finite() {
        return serializer.serialize_f64(value);
    }

    let mut map = serializer.serialize_map(Some(2))?;
    map.serialize_entry("type", "double")?;
    map.serialize_entry("value", &value.to_string())?;
    map.end()
}

fn serialize_bytes<S>(bytes: &[u8], base64_encoding: bool, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    if base64_encoding {
        return serializer.serialize_str(&base64::engine::general_purpose::STANDARD.encode(bytes));
    }

    match std::str::from_utf8(bytes) {
        Ok(value) => serializer.serialize_str(value),
        Err(_) => {
            let mut map = serializer.serialize_map(Some(3))?;
            map.serialize_entry("type", "binary")?;
            map.serialize_entry("encoding", "base64")?;
            map.serialize_entry(
                "value",
                &base64::engine::general_purpose::STANDARD.encode(bytes),
            )?;
            map.end()
        }
    }
}

fn serialize_string<S>(value: &str, base64_encoding: bool, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    if base64_encoding && value != "OK" {
        return serializer
            .serialize_str(&base64::engine::general_purpose::STANDARD.encode(value.as_bytes()));
    }

    serializer.serialize_str(value)
}

struct RedisArray<'a> {
    values: &'a [RedisValue],
    base64_encoding: bool,
}

impl<'a> RedisArray<'a> {
    fn new(values: &'a [RedisValue], base64_encoding: bool) -> Self {
        Self {
            values,
            base64_encoding,
        }
    }
}

impl Serialize for RedisArray<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serialize_array(self.values, self.base64_encoding, serializer)
    }
}

fn serialize_array<S>(
    values: &[RedisValue],
    base64_encoding: bool,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let mut seq = serializer.serialize_seq(Some(values.len()))?;

    for value in values {
        seq.serialize_element(&RedisJson::new(value, base64_encoding))?;
    }

    seq.end()
}

struct RedisMap<'a> {
    entries: &'a [(RedisValue, RedisValue)],
    base64_encoding: bool,
}

impl<'a> RedisMap<'a> {
    fn new(entries: &'a [(RedisValue, RedisValue)], base64_encoding: bool) -> Self {
        Self {
            entries,
            base64_encoding,
        }
    }
}

impl Serialize for RedisMap<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serialize_map(self.entries, self.base64_encoding, serializer)
    }
}

fn serialize_map<S>(
    entries: &[(RedisValue, RedisValue)],
    base64_encoding: bool,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    if let Some(keys) = object_keys(entries, base64_encoding) {
        let mut map = serializer.serialize_map(Some(entries.len()))?;

        for ((_, value), key) in entries.iter().zip(keys) {
            map.serialize_entry(&key, &RedisJson::new(value, base64_encoding))?;
        }

        return map.end();
    }

    let mut map = serializer.serialize_map(Some(2))?;
    map.serialize_entry("type", "map")?;
    map.serialize_entry("value", &RedisMapPairs::new(entries, base64_encoding))?;
    map.end()
}

struct RedisMapPairs<'a> {
    entries: &'a [(RedisValue, RedisValue)],
    base64_encoding: bool,
}

impl<'a> RedisMapPairs<'a> {
    fn new(entries: &'a [(RedisValue, RedisValue)], base64_encoding: bool) -> Self {
        Self {
            entries,
            base64_encoding,
        }
    }
}

impl Serialize for RedisMapPairs<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut seq = serializer.serialize_seq(Some(self.entries.len()))?;

        for (key, value) in self.entries {
            seq.serialize_element(&RedisPair {
                key,
                value,
                base64_encoding: self.base64_encoding,
            })?;
        }

        seq.end()
    }
}

struct RedisPair<'a> {
    key: &'a RedisValue,
    value: &'a RedisValue,
    base64_encoding: bool,
}

impl Serialize for RedisPair<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut seq = serializer.serialize_seq(Some(2))?;
        seq.serialize_element(&RedisJson::new(self.key, self.base64_encoding))?;
        seq.serialize_element(&RedisJson::new(self.value, self.base64_encoding))?;
        seq.end()
    }
}

fn object_keys(entries: &[(RedisValue, RedisValue)], base64_encoding: bool) -> Option<Vec<String>> {
    let mut seen = std::collections::HashSet::with_capacity(entries.len());
    let mut keys = Vec::with_capacity(entries.len());

    for (key, _) in entries {
        let object_key = object_key(key, base64_encoding)?;

        if !seen.insert(object_key.clone()) {
            return None;
        }

        keys.push(object_key);
    }

    Some(keys)
}

fn object_key(value: &RedisValue, base64_encoding: bool) -> Option<String> {
    match value {
        RedisValue::Nil => Some("null".to_string()),
        RedisValue::Int(value) => Some(value.to_string()),
        RedisValue::Boolean(value) => Some(value.to_string()),
        RedisValue::SimpleString(value) => {
            if base64_encoding && value != "OK" {
                return Some(base64::engine::general_purpose::STANDARD.encode(value.as_bytes()));
            }

            Some(value.clone())
        }
        RedisValue::BulkString(bytes) => {
            if base64_encoding {
                return Some(base64::engine::general_purpose::STANDARD.encode(bytes));
            }

            std::str::from_utf8(bytes).ok().map(ToOwned::to_owned)
        }
        RedisValue::BigNumber(value) => Some(value.to_string()),
        RedisValue::Double(value) if value.is_finite() => Some(value.to_string()),
        _ => None,
    }
}
