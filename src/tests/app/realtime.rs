use crate::app::validate_channel;

#[test]
fn accepts_supported_realtime_channel() {
    assert!(validate_channel("tenant/events:updates", 64).is_ok());
}

#[test]
fn rejects_empty_realtime_channel() {
    assert!(validate_channel("", 64).is_err());
}

#[test]
fn rejects_oversized_realtime_channel() {
    assert!(validate_channel("channel", 6).is_err());
}

#[test]
fn rejects_realtime_protocol_delimiters() {
    for channel in ["one,two", "one\ntwo", "one\rtwo", "one\0two"] {
        assert!(validate_channel(channel, 64).is_err());
    }
}
