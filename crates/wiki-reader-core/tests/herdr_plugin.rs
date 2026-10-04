//! Contract checks for the shipped plugin; never links to a live Herdr session.

fn manifest() -> toml::Table {
    toml::from_str(include_str!(
        "../../../integrations/herdr/herdr-plugin.toml"
    ))
    .unwrap()
}

fn argv(value: &toml::Value) -> Vec<&str> {
    value["command"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect()
}

#[test]
fn manifest_offers_text_tier_plugin_panes_and_a_split_launcher() {
    let manifest = manifest();
    assert_eq!(manifest["id"].as_str(), Some("wiki-reader"));
    assert_eq!(manifest["min_herdr_version"].as_str(), Some("0.9.0"));
    assert_eq!(manifest["platforms"].as_array().unwrap().len(), 2);
    let panes = manifest["panes"].as_array().unwrap();
    assert_eq!(panes.len(), 2);
    assert_eq!(panes[0]["id"].as_str(), Some("reader-overlay"));
    assert_eq!(panes[0]["placement"].as_str(), Some("overlay"));
    let popup = &panes[1];
    assert_eq!(popup["id"].as_str(), Some("reader-popup"));
    assert_eq!(popup["placement"].as_str(), Some("popup"));
    assert_eq!(popup["width"].as_str(), Some("80%"));
    assert_eq!(popup["height"].as_str(), Some("80%"));
    for pane in panes {
        assert_eq!(argv(pane), ["wiki-reader", "--herdr-context"]);
    }
    let actions = manifest["actions"].as_array().unwrap();
    let ids: Vec<_> = actions.iter().map(|a| a["id"].as_str().unwrap()).collect();
    assert_eq!(ids, ["open", "open-overlay", "open-popup"]);
    assert_eq!(
        argv(&actions[0]),
        ["wiki-reader", "--herdr-split"],
        "the default action opens an ordinary pane, which can draw images"
    );
}

#[cfg(unix)]
#[test]
fn plugin_pane_actions_use_inherited_binary_and_propagate_failure() {
    use std::os::unix::fs::PermissionsExt;
    use std::process::Command;

    let manifest = manifest();
    let actions = manifest["actions"].as_array().unwrap();
    assert_eq!(actions.len(), 3);
    for (action, id, entrypoint) in [
        (&actions[1], "open-overlay", "reader-overlay"),
        (&actions[2], "open-popup", "reader-popup"),
    ] {
        assert_eq!(action["id"].as_str(), Some(id));
        let argv = argv(action);
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
            format!("plugin\npane\nopen\n--plugin\nwiki-reader\n--entrypoint\n{entrypoint}\n")
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
}
