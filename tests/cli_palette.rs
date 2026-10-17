//! Named palette ownership and editable-state round trip through the agent CLI.
use raipixel::{Limits, Work, png};
use std::{fs, path::Path, process::Command};

fn run(dir: &Path, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_raipixel"))
        .current_dir(dir)
        .args(args)
        .output()
        .expect("execute raipixel")
}

fn rgba(dir: &Path, path: &str) -> Vec<[u8; 4]> {
    let raw = fs::read(dir.join(path)).unwrap();
    png::decode(&raw, Limits::default(), &mut Work::default())
        .unwrap()
        .image
        .pixels()
        .to_vec()
}

#[test]
fn symbolic_colors_survive_snapshot_reopen_and_palette_replace_does_not_recolor() {
    let dir = std::env::temp_dir().join(format!(
        "raipixel-palette-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    fs::create_dir(&dir).unwrap();
    let first = r##"{"interface":1,"semantics":"1.0","ops":[
      {"op":"palette.set","name":"metal","entries":[
        {"name":"dark","rgb":"#010203"},
        {"name":"light","rgb":"#AABBCC"}]},
      {"op":"asset.create","name":"weapon","size":[2,2],"palette":"metal"},
      {"op":"layer.create","asset":"weapon","name":"blade"},
      {"op":"frame.create","asset":"weapon","name":"base"},
      {"op":"cel.create","target":"weapon/blade/base"},
      {"op":"grid","targets":["weapon/blade/base"],"rows":["DL",".."],
       "symbols":{"D":"@metal/dark","L":"@metal/light",".":"#00000000"}}
    ],"exports":[{"name":"preview","kind":"frame","targets":["weapon/base"]}]}"##;
    fs::write(dir.join("first.json"), first).unwrap();
    let output = run(&dir, &["run", "first.json", "--out", "first"]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let expected = [
        [1, 2, 3, 255],
        [170, 187, 204, 255],
        [0, 0, 0, 0],
        [0, 0, 0, 0],
    ];
    assert_eq!(rgba(&dir, "first/artifacts/preview/image.png"), expected);
    let manifest = fs::read_to_string(dir.join("first/project.json")).unwrap();
    assert!(manifest.contains(r#""palette":"metal""#));
    assert!(manifest.contains(r##""rgb":"#AABBCC""##));

    let change = r##"{"interface":1,"semantics":"1.0","ops":[
      {"op":"palette.set","name":"metal","replace":true,"entries":[
        {"name":"dark","rgb":"#090807"},{"name":"light","rgb":"#AABBCC"}]}
    ]}"##;
    fs::write(dir.join("change.json"), change).unwrap();
    let output = run(
        &dir,
        &[
            "run",
            "change.json",
            "--project",
            "first",
            "--out",
            "second",
        ],
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    // Palette replacement only changes future references; stored samples are exact.
    assert_eq!(rgba(&dir, "second/images/000000.png"), expected);
    let draw = r##"{"interface":1,"semantics":"1.0","ops":[
      {"op":"draw","targets":["weapon/blade/base"],"shape":"pixel",
       "at":[0,0],"color":"@metal/dark"},
      {"op":"draw","targets":["weapon/blade/base"],"shape":"pixel",
       "at":[0,1],"color":"@metal/dark"}
    ],"exports":[{"name":"modified","kind":"frame","targets":["weapon/base"]}]}"##;
    fs::write(dir.join("draw.json"), draw).unwrap();
    let output = run(
        &dir,
        &["run", "draw.json", "--project", "second", "--out", "third"],
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let mut updated = expected;
    updated[0] = [9, 8, 7, 255];
    updated[2] = [9, 8, 7, 255];
    assert_eq!(rgba(&dir, "third/artifacts/modified/image.png"), updated);

    let error = r##"{"interface":1,"semantics":"1.0","ops":[
      {"op":"grid","targets":["weapon/blade/base"],"rows":["!"],
       "symbols":{"!":"@metal/missing"}}
    ]}"##;
    fs::write(dir.join("error.json"), error).unwrap();
    let output = run(
        &dir,
        &[
            "run",
            "error.json",
            "--project",
            "third",
            "--out",
            "invalid",
        ],
    );
    assert!(!output.status.success());
    assert!(!dir.join("invalid").exists());
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn malformed_palette_rejected_without_publishing_any_snapshot() {
    let dir = std::env::temp_dir().join(format!(
        "raipixel-palette-errors-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    fs::create_dir(&dir).unwrap();
    for (index, name, entries) in [
        (
            0,
            "metal",
            r##"[{"name":"a","rgb":"#AABBCC"},{"name":"a","rgb":"#010203"}]"##,
        ),
        (1, "metal", r##"[{"name":"a","rgb":"#AABBCC00"}]"##),
        (2, "metal", "[]"),
    ] {
        let job = format!(
            r#"{{"interface":1,"semantics":"1.0","ops":[
            {{"op":"palette.set","name":"{name}","entries":{entries}}}
            ]}}"#
        );
        let file = format!("job-{index}.json");
        let dest = format!("rejected-{index}");
        fs::write(dir.join(&file), job).unwrap();
        let output = run(&dir, &["run", &file, "--out", &dest]);
        assert!(!output.status.success());
        assert!(!dir.join(dest).exists());
    }
    fs::remove_dir_all(dir).unwrap();
}
