use redis::Value;
use serde::ser::{SerializeMap, SerializeSeq};
use serde::{Serialize, Serializer};

use super::redis_value::RedisJson;

pub(crate) enum RedisResponse {
    Result(Value),
    Error(String),
}

pub(crate) struct CommandResponse<'a> {
    pub(crate) result: &'a Value,
    pub(crate) base64_encoding: bool,
}

impl Serialize for CommandResponse<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut map = serializer.serialize_map(Some(1))?;
        map.serialize_entry("result", &RedisJson::new(self.result, self.base64_encoding))?;
        map.end()
    }
}

pub(crate) struct PipelineResponse<'a> {
    pub(crate) items: &'a [RedisResponse],
    pub(crate) base64_encoding: bool,
}

impl Serialize for PipelineResponse<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut seq = serializer.serialize_seq(Some(self.items.len()))?;

        for item in self.items {
            seq.serialize_element(&PipelineItem {
                item,
                base64_encoding: self.base64_encoding,
            })?;
        }

        seq.end()
    }
}

struct PipelineItem<'a> {
    item: &'a RedisResponse,
    base64_encoding: bool,
}

impl Serialize for PipelineItem<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self.item {
            RedisResponse::Result(value) => {
                let mut map = serializer.serialize_map(Some(1))?;
                map.serialize_entry("result", &RedisJson::new(value, self.base64_encoding))?;
                map.end()
            }
            RedisResponse::Error(error) => {
                let mut map = serializer.serialize_map(Some(1))?;
                map.serialize_entry("error", error)?;
                map.end()
            }
        }
    }
}

pub(crate) struct TransactionResponse<'a> {
    pub(crate) values: &'a [Value],
    pub(crate) base64_encoding: bool,
}

impl Serialize for TransactionResponse<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut seq = serializer.serialize_seq(Some(self.values.len()))?;

        for value in self.values {
            seq.serialize_element(&CommandResponse {
                result: value,
                base64_encoding: self.base64_encoding,
            })?;
        }

        seq.end()
    }
}
