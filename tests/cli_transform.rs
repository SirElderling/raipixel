//! End-to-end agent-issued transform, exact PNG sample preservation and rollback.
use raipixel::{Limits, Work, png};
use std::{fs, path::Path, process::Command};

fn run(base: &Path, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_raipixel"))
        .current_dir(base)
        .args(args)
        .output()
        .unwrap()
}

#[test]
fn json_transform_round_trip_and_rejected_kind_preserve_source() {
    let root = std::env::temp_dir().join(format!(
        "raipixel-transform-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    fs::create_dir(&root).unwrap();
    fs::write(
        root.join("create.json"),
        r##"{"interface":1,"semantics":"1.0","ops":[
          {"op":"asset.create","name":"sprite","size":[3,2]},
          {"op":"layer.create","asset":"sprite","name":"paint"},
          {"op":"frame.create","asset":"sprite","name":"base"},
          {"op":"cel.create","target":"sprite/paint/base"},
          {"op":"grid","targets":["sprite/paint/base"],
           "rows":["ABC","DEF"],
           "symbols":{"A":"#11223300","B":"#445566FF","C":"#77889980",
                      "D":"#AABBCC00","E":"#DDEEFF01","F":"#102030FE"}}
        ]}"##,
    )
    .unwrap();
    let first = run(&root, &["run", "create.json", "--out", "first"]);
    assert!(
        first.status.success(),
        "{}",
        String::from_utf8_lossy(&first.stderr)
    );
    fs::write(
        root.join("edit.json"),
        r#"{"interface":1,"semantics":"1.0","ops":[
          {"op":"transform","targets":["sprite/paint/base"],"kind":"flip-x"},
          {"op":"transform","targets":["sprite/paint/base"],"kind":"flip-y"}
        ],"exports":[{"name":"game","kind":"frame","targets":["sprite/base"]}]}"#,
    )
    .unwrap();
    let second = run(
        &root,
        &["run", "edit.json", "--project", "first", "--out", "second"],
    );
    assert!(
        second.status.success(),
        "{}",
        String::from_utf8_lossy(&second.stderr)
    );
    let source = png::decode(
        &fs::read(root.join("first/images/000000.png")).unwrap(),
        Limits::default(),
        &mut Work::default(),
    )
    .unwrap()
    .image;
    let output = png::decode(
        &fs::read(root.join("second/artifacts/game/image.png")).unwrap(),
        Limits::default(),
        &mut Work::default(),
    )
    .unwrap()
    .image;
    assert_eq!(
        output.pixels(),
        &[
            [16, 32, 48, 254],
            [221, 238, 255, 1],
            [170, 187, 204, 0],
            [119, 136, 153, 128],
            [68, 85, 102, 255],
            [17, 34, 51, 0]
        ]
    );
    assert_eq!(source.pixel(0, 0), Some([17, 34, 51, 0]));

    fs::write(
        root.join("invalid.json"),
        r#"{"interface":1,"semantics":"1.0","ops":[
          {"op":"transform","targets":["sprite/paint/base"],"kind":"rotate-90"}
        ]}"#,
    )
    .unwrap();
    let failed = run(
        &root,
        &["run", "invalid.json", "--project", "first", "--out", "bad"],
    );
    assert!(!failed.status.success());
    assert!(!root.join("bad").exists());
    fs::remove_dir_all(root).unwrap();
}
