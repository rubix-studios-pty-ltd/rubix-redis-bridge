use anyhow::bail;
use serde_json::Value;

pub(crate) fn json_to_arg(value: &Value, max_arg_bytes: usize) -> anyhow::Result<Vec<u8>> {
    let bytes = match value {
        Value::String(value) => value.as_bytes().to_vec(),
        Value::Number(value) => value.to_string().into_bytes(),
        Value::Bool(value) => value.to_string().into_bytes(),
        Value::Null => bail!("Invalid Redis argument. Null values are not supported."),
        Value::Array(_) | Value::Object(_) => {
            bail!("Invalid Redis argument. Nested arrays and objects are not supported.")
        }
    };

    if bytes.len() > max_arg_bytes {
        bail!(
            "Redis argument is too large. Maximum allowed bytes per argument: {}.",
            max_arg_bytes
        );
    }

    Ok(bytes)
}
