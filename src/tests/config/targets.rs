use crate::config::parse_file_targets;

#[test]
fn parses_file_targets() {
    let data = r#"
        {
          "version": 1,
          "targets": [
            {
              "rrb_id": "primary_redis",
              "connection_string": "redis://default:password@redis:6379",
              "operation_limit": 100,
              "connection_shards": 8,
              "tokens": [
                {
                  "id": "primary_app",
                  "hash": "f7bc83f430538424b13298e6aa6fb143ef4d59a14946175997479dbc2d1a3cd8"
                }
              ]
            }
          ]
        }
        "#;

    let targets = parse_file_targets(data, Some("hash-key")).unwrap();

    assert_eq!(targets.len(), 1);
    assert_eq!(targets[0].rrb_id, "primary_redis");
    assert_eq!(targets[0].operation_limit, 100);
    assert_eq!(targets[0].connection_shards, 8);
    assert_eq!(targets[0].tokens.len(), 1);
    assert!(targets[0].tokens[0].enabled);
}

#[test]
fn rejects_file_mode_without_hash() {
    let data = r#"
        {
          "version": 1,
          "targets": [
            {
              "rrb_id": "primary_redis",
              "connection_string": "redis://default:password@redis:6379",
              "tokens": [
                {
                  "id": "primary_app",
                  "hash": "f7bc83f430538424b13298e6aa6fb143ef4d59a14946175997479dbc2d1a3cd8"
                }
              ]
            }
          ]
        }
        "#;

    assert!(parse_file_targets(data, None).is_err());
    assert!(parse_file_targets(data, Some("hash-key")).is_ok());
}

#[test]
fn rejects_duplicate_token() {
    let data = r#"
        {
          "version": 1,
          "targets": [
            {
              "rrb_id": "primary_redis",
              "connection_string": "redis://default:password@redis:6379",
              "tokens": [
                {
                  "id": "primary_app",
                  "hash": "f7bc83f430538424b13298e6aa6fb143ef4d59a14946175997479dbc2d1a3cd8"
                },
                {
                  "id": "secondary_app",
                  "hash": "f7bc83f430538424b13298e6aa6fb143ef4d59a14946175997479dbc2d1a3cd8"
                }
              ]
            }
          ]
        }
        "#;

    assert!(parse_file_targets(data, Some("hash-key")).is_err());
}

#[test]
fn defaults_connection_shards() {
    let data = r#"
        {
          "version": 1,
          "targets": [
            {
              "rrb_id": "primary_redis",
              "connection_string": "redis://default:password@redis:6379",
              "tokens": [
                {
                  "id": "primary_app",
                  "hash": "f7bc83f430538424b13298e6aa6fb143ef4d59a14946175997479dbc2d1a3cd8"
                }
              ]
            }
          ]
        }
        "#;

    let targets = parse_file_targets(data, Some("hash-key")).unwrap();

    assert_eq!(targets[0].connection_shards, 4);
}

#[test]
fn rejects_zero_connection_shards() {
    let data = r#"
        {
          "version": 1,
          "targets": [
            {
              "rrb_id": "primary_redis",
              "connection_string": "redis://default:password@redis:6379",
              "connection_shards": 0,
              "tokens": [
                {
                  "id": "primary_app",
                  "hash": "f7bc83f430538424b13298e6aa6fb143ef4d59a14946175997479dbc2d1a3cd8"
                }
              ]
            }
          ]
        }
        "#;

    assert!(parse_file_targets(data, Some("hash-key")).is_err());
}
