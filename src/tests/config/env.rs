use crate::config::parse_csv;

#[test]
fn parses_csv_commands() {
    let commands = parse_csv("get, Set , DEL");

    assert!(commands.contains("GET"));
    assert!(commands.contains("SET"));
    assert!(commands.contains("DEL"));
}
