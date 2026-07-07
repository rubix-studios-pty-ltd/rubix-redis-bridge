use super::hash::TokenHash;

#[test]
fn sha256() {
    let token_hash = TokenHash::sha256("abc");

    assert_eq!(
        token_hash.config_value(),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
}

#[test]
fn hmac_sha256() {
    let token_hash = TokenHash::hmac_sha256("key", "The quick brown fox jumps over the lazy dog");

    assert_eq!(
        token_hash.config_value(),
        "f7bc83f430538424b13298e6aa6fb143ef4d59a14946175997479dbc2d1a3cd8"
    );
}

#[test]
fn hmac_sha256_parses() {
    let token_hash = TokenHash::hmac_sha256_parse(
        "F7BC83F430538424B13298E6AA6FB143EF4D59A14946175997479DBC2D1A3CD8",
    )
    .unwrap();

    assert_eq!(
        token_hash.config_value(),
        "f7bc83f430538424b13298e6aa6fb143ef4d59a14946175997479dbc2d1a3cd8"
    );
}
