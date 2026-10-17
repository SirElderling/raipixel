//! Agent-instruction integration: original-sample recolor, keyed alpha and snapshot reopen.
use raipixel::{Limits, Work, png};
use std::{fs, path::Path, process::Command};

fn run(root: &Path, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_raipixel"))
        .current_dir(root)
        .args(args)
        .output()
        .expect("launch raipixel")
}

#[test]
fn mapped_colors_and_alpha_edits_survive_png_snapshot_round_trip() {
    let root = std::env::temp_dir().join(format!(
        "raipixel-color-test-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    fs::create_dir(&root).unwrap();
    let job = r##"{
        "interface":1,"semantics":"1.0","ops":[
          {"op":"asset.create","name":"icon","size":[3,2]},
          {"op":"layer.create","asset":"icon","name":"base"},
          {"op":"frame.create","asset":"icon","name":"base"},
          {"op":"cel.create","target":"icon/base/base"},
          {"op":"grid","targets":["icon/base/base"],"rows":["AB.","CBA"],
           "symbols":{"A":"#01020300","B":"#040506C8","C":"#07080964",".":"#00000000"}},
          {"op":"palette.map","targets":["icon/base/base"],
           "pairs":[["#010203","#040506"],["#040506","#070809"]]},
          {"op":"alpha","targets":["icon/base/base"],"kind":"key",
           "color":"#070809","region":[0,0,3,1]},
          {"op":"alpha","targets":["icon/base/base"],"kind":"threshold",
           "threshold":128,"region":[0,1,3,1]}
        ],
        "exports":[{"name":"png","kind":"frame","targets":["icon/base"]}]
    }"##;
    fs::write(root.join("job.json"), job).unwrap();
    let result = run(&root, &["run", "job.json", "--out", "one"]);
    assert!(
        result.status.success(),
        "job failed: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    let bytes = fs::read(root.join("one/artifacts/png/image.png")).unwrap();
    let image = png::decode(&bytes, Limits::default(), &mut Work::default())
        .unwrap()
        .image;
    assert_eq!(
        image.pixels(),
        &[
            [4, 5, 6, 0], [7, 8, 9, 0], [0, 0, 0, 0],
            [7, 8, 9, 0], [7, 8, 9, 255], [4, 5, 6, 0]
        ]
    );

    let reopen = r#"{"interface":1,"semantics":"1.0","ops":[],
      "exports":[{"name":"reopened","kind":"frame","targets":["icon/base"]}]}"#;
    fs::write(root.join("reopen.json"), reopen).unwrap();
    let result = run(
        &root,
        &["run", "reopen.json", "--project", "one", "--out", "two"]
    );
    assert!(
        result.status.success(),
        "reopen failed: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    let other = fs::read(root.join("two/artifacts/reopened/image.png")).unwrap();
    assert_eq!(bytes, other, "reopening modified deterministic PNG bytes");

    let bad = r##"{"interface":1,"semantics":"1.0",
        "ops":[{"op":"palette.map","targets":["icon/base/base"],
        "pairs":[["#010203","#040506"],["#010203","#070809"]]}]}"##;
    fs::write(root.join("duplicate.json"), bad).unwrap();
    let result = run(
        &root,
        &["run", "duplicate.json", "--project", "one", "--out", "rejected"]
    );
    assert!(!result.status.success());
    assert!(!root.join("rejected").exists());
    assert_eq!(
        fs::read(root.join("one/artifacts/png/image.png")).unwrap(),
        bytes
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn rgba_pair_and_alpha_key_match_full_sample_without_cascading() {
    let root = std::env::temp_dir().join(format!(
        "raipixel-rgba-test-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    fs::create_dir(&root).unwrap();
    let job = r##"{"interface":1,"semantics":"1.0","ops":[
      {"op":"asset.create","name":"icon","size":[2,1]},
      {"op":"layer.create","asset":"icon","name":"paint"},
      {"op":"frame.create","asset":"icon","name":"base"},
      {"op":"cel.create","target":"icon/paint/base"},
      {"op":"grid","targets":["icon/paint/base"],"rows":["AB"],
       "symbols":{"A":"#01020300","B":"#010203FF"}},
      {"op":"palette.map","targets":["icon/paint/base"],
       "preserve_alpha":false,"pairs":[["#01020300","#070809FE"]]},
      {"op":"alpha","targets":["icon/paint/base"],"kind":"key",
       "color":"#070809FD","match":"rgba"}
    ],"exports":[{"name":"png","kind":"frame","targets":["icon/base"]}]}"##;
    fs::write(root.join("job.json"), job).unwrap();
    let result = run(&root, &["run", "job.json", "--out", "edited"]);
    assert!(
        result.status.success(),
        "job failed: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    let image = png::decode(
        &fs::read(root.join("edited/artifacts/png/image.png")).unwrap(),
        Limits::default(),
        &mut Work::default(),
    )
    .unwrap()
    .image;
    assert_eq!(image.pixels(), &[[7, 8, 9, 254], [1, 2, 3, 255]]);
    fs::remove_dir_all(root).unwrap();
}
