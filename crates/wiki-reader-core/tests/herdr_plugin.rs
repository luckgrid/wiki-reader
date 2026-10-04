//! Contract checks for the shipped plugin; never links to a live Herdr session.

fn manifest() -> toml::Table {
    toml::from_str(include_str!(
        "../../../integrations/herdr/herdr-plugin.toml"
    ))
    .unwrap()
}

#[test]
fn popup_manifest_uses_context_launcher() {
    let manifest = manifest();
    assert_eq!(manifest["id"].as_str(), Some("wiki-reader"));
    assert_eq!(manifest["min_herdr_version"].as_str(), Some("0.9.0"));
    assert_eq!(manifest["platforms"].as_array().unwrap().len(), 2);
    let panes = manifest["panes"].as_array().unwrap();
    assert_eq!(panes.len(), 1);
    let pane = &panes[0];
    assert_eq!(pane["id"].as_str(), Some("reader"));
    assert_eq!(pane["placement"].as_str(), Some("popup"));
    assert_eq!(pane["width"].as_str(), Some("80%"));
    assert_eq!(pane["height"].as_str(), Some("80%"));
    let command: Vec<_> = pane["command"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert_eq!(command, ["wiki-reader", "--herdr-context"]);
}

#[cfg(unix)]
#[test]
fn action_uses_inherited_binary_and_propagates_failure() {
    use std::os::unix::fs::PermissionsExt;
    use std::process::Command;

    let manifest = manifest();
    let actions = manifest["actions"].as_array().unwrap();
    assert_eq!(actions.len(), 1);
    assert_eq!(actions[0]["id"].as_str(), Some("open"));
    let argv: Vec<_> = actions[0]["command"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    let dir = tempfile::tempdir().unwrap();
    // Spaces and shell metacharacters must stay part of the binary path.
    let binary = dir.path().join("herdr binary; not a shell command");
    std::fs::write(&binary, "#!/bin/sh\nprintf '%s\\n' \"$@\"\nexit 7\n").unwrap();
    std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o700)).unwrap();
    let output = Command::new(argv[0])
        .args(&argv[1..])
        .env("HERDR_BIN_PATH", &binary)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(7));
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "plugin\npane\nopen\n--plugin\nwiki-reader\n--entrypoint\nreader\n"
    );
    let status = Command::new(argv[0])
        .args(&argv[1..])
        .env("HERDR_BIN_PATH", dir.path().join("missing"))
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .unwrap();
    assert!(!status.success());
}
