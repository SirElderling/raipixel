//! Read-only validation reports are machine-readable and never rewrite snapshots.
use std::{fs, path::Path, process::Command};

fn invoke(root: &Path, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_raipixel"))
        .current_dir(root)
        .args(args)
        .output()
        .expect("launch raipixel")
}

#[test]
fn validate_reports_palette_violations_without_changing_existing_files() {
    let root = std::env::temp_dir().join(format!(
        "raipixel-validate-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    fs::create_dir(&root).unwrap();
    let job = r##"{"interface":1,"semantics":"1.0","ops":[
      {"op":"palette.set","name":"allowed","entries":[
        {"name":"dark","rgb":"#010203"}]},
      {"op":"asset.create","name":"icon","size":[2,1],"palette":"allowed"},
      {"op":"layer.create","asset":"icon","name":"paint"},
      {"op":"frame.create","asset":"icon","name":"base"},
      {"op":"cel.create","target":"icon/paint/base"},
      {"op":"draw","targets":["icon/paint/base"],"shape":"pixel","at":[0,0],
       "color":"@allowed/dark"}
    ]}"##;
    fs::write(root.join("initial.json"), job).unwrap();
    let made = invoke(&root, &["run", "initial.json", "--out", "first"]);
    assert!(made.status.success(), "{}", String::from_utf8_lossy(&made.stderr));
    let first = invoke(&root, &["validate", "first"]);
    assert!(first.status.success(), "{}", String::from_utf8_lossy(&first.stderr));
    let report = String::from_utf8_lossy(&first.stdout);
    assert!(report.contains(r#""passed":true"#));
    assert!(report.contains(r#""name":"palette","passed":true"#));

    fs::write(
        root.join("edit.json"),
        r##"{"interface":1,"semantics":"1.0","ops":[
           {"op":"draw","targets":["icon/paint/base"],"shape":"pixel",
            "at":[1,0],"color":"#090807"}],"checks":["structure"]}"##,
    )
    .unwrap();
    let edit = invoke(
        &root,
        &["run", "edit.json", "--project", "first", "--out", "changed"]
    );
    assert!(edit.status.success(), "{}", String::from_utf8_lossy(&edit.stderr));
    let before = fs::read(root.join("changed/images/000000.png")).unwrap();
    let check = invoke(&root, &["validate", "changed"]);
    assert_eq!(check.status.code(), Some(3), "{}", String::from_utf8_lossy(&check.stderr));
    assert!(check.stderr.is_empty());
    let report = String::from_utf8_lossy(&check.stdout);
    assert!(report.contains(r#""ok":false"#));
    assert!(report.contains(r#""code":"E_STYLE""#));
    assert!(report.contains(r#""at":[1,0]"#));
    assert!(report.contains(r#""error_count":1"#));
    assert_eq!(fs::read(root.join("changed/images/000000.png")).unwrap(), before);

    fs::write(
        root.join("export.json"),
        r#"{"interface":1,"semantics":"1.0","ops":[],"checks":["structure"],
            "exports":[{"name":"png","kind":"frame","targets":["icon/base"]}]}"#,
    )
    .unwrap();
    let forbidden = invoke(
        &root,
        &["run", "export.json", "--project", "changed", "--out", "forbidden"],
    );
    assert!(!forbidden.status.success());
    assert!(!root.join("forbidden").exists());
    assert_eq!(fs::read(root.join("changed/images/000000.png")).unwrap(), before);

    fs::write(
        root.join("query.json"),
        r#"{"asset":"icon","checks":["structure","bounds"]}"#,
    )
    .unwrap();
    let check = invoke(&root, &["validate", "changed", "--query", "query.json"]);
    assert!(check.status.success(), "{}", String::from_utf8_lossy(&check.stderr));
    let report = String::from_utf8_lossy(&check.stdout);
    assert!(report.contains(r#""error_count":0"#));
    assert!(!report.contains(r#""name":"palette""#));

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn validation_query_rejects_unsupported_and_duplicate_checks() {
    let root = std::env::temp_dir().join(format!(
        "raipixel-validate-errors-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    fs::create_dir(&root).unwrap();
    fs::write(
        root.join("initial.json"),
        r#"{"interface":1,"semantics":"1.0","ops":[
           {"op":"asset.create","name":"x","size":[1,1]},
           {"op":"layer.create","asset":"x","name":"l"},
           {"op":"frame.create","asset":"x","name":"f"},
           {"op":"cel.create","target":"x/l/f"}
        ]}"#,
    )
    .unwrap();
    assert!(invoke(&root, &["run", "initial.json", "--out", "snapshot"]).status.success());
    for (index, query) in [
        r#"{"checks":["made_up"]}"#,
        r#"{"checks":["structure","structure"]}"#,
        r#"{"checks":[]}"#,
    ].iter().enumerate() {
        fs::write(root.join(format!("{index}.json")), query).unwrap();
        let output = invoke(
            &root,
            &["validate", "snapshot", "--query", &format!("{index}.json")],
        );
        assert!(!output.status.success());
        assert!(output.stdout.is_empty(), "invalid request emitted a success report");
    }
    fs::remove_dir_all(root).unwrap();
}
