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
          {"op":"transform","targets":["sprite/paint/base"],"kind":"rotate-45"}
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

#[test]
fn json_quarter_turn_and_reopen_preserve_every_rgba_sample() {
    let root = std::env::temp_dir().join(format!(
        "raipixel-quarter-turn-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    fs::create_dir(&root).unwrap();
    fs::write(
        root.join("square.json"),
        r##"{"interface":1,"semantics":"1.0","ops":[
          {"op":"asset.create","name":"square","size":[3,3]},
          {"op":"layer.create","asset":"square","name":"paint"},
          {"op":"frame.create","asset":"square","name":"base"},
          {"op":"cel.create","target":"square/paint/base"},
          {"op":"grid","targets":["square/paint/base"],
           "rows":["ABC","DEF","GHI"],
           "symbols":{
             "A":"#01020300","B":"#02030410","C":"#030405FF",
             "D":"#04050640","E":"#05060780","F":"#06070800",
             "G":"#070809FF","H":"#08090A08","I":"#090A0B00"
           }}
        ]}"##,
    )
    .unwrap();
    let first = run(&root, &["run", "square.json", "--out", "original"]);
    assert!(
        first.status.success(),
        "{}",
        String::from_utf8_lossy(&first.stderr)
    );
    fs::write(
        root.join("turn.json"),
        r#"{"interface":1,"semantics":"1.0","ops":[
          {"op":"transform","targets":["square/paint/base"],"kind":"rotate-90"}
        ],"exports":[{"name":"game","kind":"frame","targets":["square/base"]}]}"#,
    )
    .unwrap();
    let turn = run(
        &root,
        &[
            "run",
            "turn.json",
            "--project",
            "original",
            "--out",
            "clockwise",
        ],
    );
    assert!(
        turn.status.success(),
        "{}",
        String::from_utf8_lossy(&turn.stderr)
    );
    let bytes = fs::read(root.join("clockwise/artifacts/game/image.png")).unwrap();
    let output = png::decode(&bytes, Limits::default(), &mut Work::default())
        .unwrap()
        .image;
    let original = png::decode(
        &fs::read(root.join("original/images/000000.png")).unwrap(),
        Limits::default(),
        &mut Work::default(),
    )
    .unwrap()
    .image;
    let expected = [
        [7, 8, 9, 255],
        [4, 5, 6, 64],
        [1, 2, 3, 0],
        [8, 9, 10, 8],
        [5, 6, 7, 128],
        [2, 3, 4, 16],
        [9, 10, 11, 0],
        [6, 7, 8, 0],
        [3, 4, 5, 255],
    ];
    assert_eq!(output.pixels(), &expected);
    fs::write(
        root.join("reverse.json"),
        r#"{"interface":1,"semantics":"1.0","ops":[
          {"op":"transform","targets":["square/paint/base"],"kind":"rotate-270"}
        ],"exports":[{"name":"game","kind":"frame","targets":["square/base"]}]}"#,
    )
    .unwrap();
    let reversed = run(
        &root,
        &[
            "run",
            "reverse.json",
            "--project",
            "clockwise",
            "--out",
            "recovered",
        ],
    );
    assert!(
        reversed.status.success(),
        "{}",
        String::from_utf8_lossy(&reversed.stderr)
    );
    let recovered = png::decode(
        &fs::read(root.join("recovered/artifacts/game/image.png")).unwrap(),
        Limits::default(),
        &mut Work::default(),
    )
    .unwrap()
    .image;
    assert_eq!(recovered, original, "quarter-turns must be exact inverses");
    assert_eq!(
        fs::read(root.join("original/images/000000.png")).unwrap(),
        fs::read(root.join("recovered/images/000000.png")).unwrap(),
        "snapshot PNG output must be deterministic"
    );
    fs::remove_dir_all(root).unwrap();
}
