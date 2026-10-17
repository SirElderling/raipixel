//! Native CLI smoke test: bare relative job paths, editable reopen, export and rollback.
use raipixel::{Limits, Work, png};
use std::{fs, path::Path, process::Command};

fn invoke(base: &Path, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_raipixel"))
        .current_dir(base)
        .args(args)
        .output()
        .expect("launch raipixel")
}

#[test]
fn single_cel_jobs_reopen_with_exact_transparent_rgb_and_no_partial_publication() {
    let root = std::env::temp_dir().join(format!(
        "raipixel-cli-test-{}-{:?}",
        std::process::id(),
        std::thread::current().id(),
    ));
    fs::create_dir(&root).expect("unique fixture directory");
    let first = r##"{
      "interface":1,"semantics":"1.0",
      "ops":[
        {"op":"asset.create","name":"icon","size":[2,2]},
        {"op":"layer.create","asset":"icon","name":"paint"},
        {"op":"frame.create","asset":"icon","name":"base"},
        {"op":"cel.create","target":"icon/paint/base"},
        {"op":"grid","targets":["icon/paint/base"],"rows":["AB","BA"],
         "symbols":{"A":"#AA0000FF","B":"#00112200"}}
      ],
      "exports":[{"name":"first","kind":"frame","targets":["icon/base"]}]
    }"##;
    fs::write(root.join("first.json"), first).unwrap();
    let result = invoke(&root, &["run", "first.json", "--out", "initial"]);
    assert!(
        result.status.success(),
        "first job failed: {}",
        String::from_utf8_lossy(&result.stderr)
    );

    let bytes = fs::read(root.join("initial/artifacts/first/image.png")).unwrap();
    let initial = png::decode(&bytes, Limits::default(), &mut Work::default())
        .unwrap()
        .image;
    assert_eq!(initial.size(), [2, 2]);
    assert_eq!(
        initial.pixels(),
        &[
            [170, 0, 0, 255],
            [0, 17, 34, 0],
            [0, 17, 34, 0],
            [170, 0, 0, 255],
        ]
    );

    let updated = r##"{
      "interface":1,"semantics":"1.0",
      "ops":[
        {"op":"draw","targets":["icon/paint/base"],
         "shape":"pixel","at":[1,1],"color":"#01020300"}
      ],
      "exports":[{"name":"edited","kind":"frame","targets":["icon/base"]}]
    }"##;
    fs::write(root.join("edit.json"), updated).unwrap();
    let result = invoke(
        &root,
        &["run", "edit.json", "--project", "initial", "--out", "edited"],
    );
    assert!(
        result.status.success(),
        "reopen/edit failed: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    let reopened = png::decode(
        &fs::read(root.join("edited/artifacts/edited/image.png")).unwrap(),
        Limits::default(),
        &mut Work::default(),
    )
    .unwrap()
    .image;
    assert_eq!(reopened.pixel(0, 0), Some([170, 0, 0, 255]));
    assert_eq!(reopened.pixel(1, 0), Some([0, 17, 34, 0]));
    assert_eq!(reopened.pixel(0, 1), Some([0, 17, 34, 0]));
    assert_eq!(reopened.pixel(1, 1), Some([1, 2, 3, 0]));

    let dry = invoke(&root, &["run", "edit.json", "--project", "initial", "--out", "dry", "--dry-run"]);
    assert!(dry.status.success());
    assert!(!root.join("dry").exists(), "dry-run published an output");

    fs::write(root.join("invalid.json"), r#"{"interface":1,"semantics":"1.0","ops":[{"op":"unknown"}]}"#)
        .unwrap();
    let rejected = invoke(&root, &["run", "invalid.json", "--out", "rejected"]);
    assert!(!rejected.status.success());
    assert!(!root.join("rejected").exists(), "invalid job published output");

    fs::remove_dir_all(&root).unwrap();
}
