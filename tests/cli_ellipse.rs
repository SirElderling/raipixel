//! Agent-issued ellipse drawing preserves exact RGBA in PNG, snapshot and reopen.
use raipixel::{Limits, Work, png};
use std::{fs, path::Path, process::Command};

fn run(root: &Path, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_raipixel"))
        .current_dir(root)
        .args(args)
        .output()
        .expect("run raipixel")
}

#[test]
fn ellipse_draw_corrects_an_existing_cel_and_reopens_without_changing_source() {
    let root = std::env::temp_dir().join(format!(
        "raipixel-ellipse-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    fs::create_dir(&root).unwrap();
    fs::write(
        root.join("create.json"),
        r##"{"interface":1,"semantics":"1.0","ops":[
          {"op":"asset.create","name":"icon","size":[7,7]},
          {"op":"layer.create","asset":"icon","name":"paint"},
          {"op":"frame.create","asset":"icon","name":"base"},
          {"op":"cel.create","target":"icon/paint/base"},
          {"op":"draw","shape":"rect","rect":[0,0,7,7],
           "targets":["icon/paint/base"],"color":"#10203000"}
        ]}"##,
    )
    .unwrap();
    let create = run(&root, &["run", "create.json", "--out", "initial"]);
    assert!(
        create.status.success(),
        "{}",
        String::from_utf8_lossy(&create.stderr)
    );
    fs::write(
        root.join("ellipse.json"),
        r##"{"interface":1,"semantics":"1.0","ops":[
          {"op":"draw","shape":"ellipse","filled":true,"rect":[0,0,7,7],
           "targets":["icon/paint/base"],"color":"#A1B2C300"},
          {"op":"draw","shape":"ellipse","filled":false,"rect":[0,0,7,7],
           "targets":["icon/paint/base"],"color":"#405060FF"}
        ],"exports":[{"name":"game","kind":"frame","targets":["icon/base"]}]}"##,
    )
    .unwrap();
    let edit = run(
        &root,
        &["run", "ellipse.json", "--project", "initial", "--out", "edited"]
    );
    assert!(
        edit.status.success(),
        "{}",
        String::from_utf8_lossy(&edit.stderr)
    );
    let bytes = fs::read(root.join("edited/artifacts/game/image.png")).unwrap();
    let image = png::decode(&bytes, Limits::default(), &mut Work::default())
        .unwrap()
        .image;
    assert_eq!(image.size(), [7, 7]);
    assert_eq!(image.pixel(3, 3), Some([161, 178, 195, 0]));
    assert_eq!(image.pixel(3, 0), Some([64, 80, 96, 255]));
    assert_eq!(image.pixel(0, 0), Some([16, 32, 48, 0]));
    assert_eq!(image.pixel(3, 1), Some([161, 178, 195, 0]));
    let original = png::decode(
        &fs::read(root.join("initial/images/000000.png")).unwrap(),
        Limits::default(),
        &mut Work::default(),
    )
    .unwrap()
    .image;
    assert_eq!(original.pixel(3, 3), Some([16, 32, 48, 0]));

    fs::write(
        root.join("reopen.json"),
        r#"{"interface":1,"semantics":"1.0","ops":[],
        "exports":[{"name":"game","kind":"frame","targets":["icon/base"]}]}"#,
    )
    .unwrap();
    let reopen = run(
        &root,
        &["run", "reopen.json", "--project", "edited", "--out", "reopened"]
    );
    assert!(
        reopen.status.success(),
        "{}",
        String::from_utf8_lossy(&reopen.stderr)
    );
    assert_eq!(
        bytes,
        fs::read(root.join("reopened/artifacts/game/image.png")).unwrap()
    );

    fs::write(
        root.join("invalid.json"),
        r##"{"interface":1,"semantics":"1.0","ops":[
          {"op":"draw","shape":"rect","filled":false,"rect":[0,0,2,2],
           "targets":["icon/paint/base"],"color":"#AABBCCFF"}
        ]}"##,
    )
    .unwrap();
    let invalid = run(
        &root,
        &["run", "invalid.json", "--project", "edited", "--out", "rejected"]
    );
    assert!(!invalid.status.success());
    assert!(!root.join("rejected").exists());
    fs::remove_dir_all(root).unwrap();
}
