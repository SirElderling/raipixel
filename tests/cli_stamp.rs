//! AI-directed same-cel stamping: frozen source, copy semantics and rollback.
use raipixel::{Limits, Work, png};
use std::sync::atomic::{AtomicU64, Ordering};
use std::{fs, path::PathBuf, process::Command};

static NEXT: AtomicU64 = AtomicU64::new(0);

struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        let id = NEXT.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!("raipixel-stamp-{}-{id}", std::process::id()));
        fs::create_dir(&root).unwrap();
        Self(root)
    }
    fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
    fn run(&self, args: &[&str]) -> std::process::Output {
        Command::new(env!("CARGO_BIN_EXE_raipixel"))
            .current_dir(&self.0)
            .args(args)
            .output()
            .unwrap()
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn pixels(path: &std::path::Path) -> Vec<[u8; 4]> {
    png::decode(
        &fs::read(path).unwrap(),
        Limits::default(),
        &mut Work::default(),
    )
    .unwrap()
    .image
    .pixels()
    .to_vec()
}

#[test]
fn overlapping_copy_stamps_from_frozen_original_and_keeps_hidden_rgb() {
    let root = Scratch::new();
    fs::write(
        root.path("job.json"),
        r##"{"interface":1,"semantics":"1.0","ops":[
          {"op":"asset.create","name":"icon","size":[4,1]},
          {"op":"layer.create","asset":"icon","name":"paint"},
          {"op":"frame.create","asset":"icon","name":"base"},
          {"op":"cel.create","target":"icon/paint/base"},
          {"op":"grid","targets":["icon/paint/base"],"rows":["ABCD"],
           "symbols":{"A":"#01020300","B":"#040506FF",
                      "C":"#07080980","D":"#0A0B0CFF"}},
          {"op":"stamp","targets":["icon/paint/base"],"source":"icon/paint/base",
           "rect":[0,0,3,1],"at":[1,0],"mode":"copy"}
        ],"exports":[{"name":"game","kind":"frame","targets":["icon/base"]}]}"##,
    )
    .unwrap();
    let output = root.run(&["run", "job.json", "--out", "result"]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let expected = [[1, 2, 3, 0], [1, 2, 3, 0], [4, 5, 6, 255], [7, 8, 9, 128]];
    assert_eq!(pixels(&root.path("result/images/000000.png")), expected);
    assert_eq!(
        pixels(&root.path("result/artifacts/game/image.png")),
        expected
    );
}

#[test]
fn source_over_uses_integer_alpha_rule_and_rejected_stamps_do_not_publish() {
    let root = Scratch::new();
    fs::write(
        root.path("create.json"),
        r##"{"interface":1,"semantics":"1.0","ops":[
          {"op":"asset.create","name":"icon","size":[2,1]},
          {"op":"layer.create","asset":"icon","name":"paint"},
          {"op":"frame.create","asset":"icon","name":"base"},
          {"op":"cel.create","target":"icon/paint/base"},
          {"op":"grid","targets":["icon/paint/base"],"rows":["BR"],
           "symbols":{"B":"#0000FFFF","R":"#FF000080"}}
        ]}"##,
    )
    .unwrap();
    let output = root.run(&["run", "create.json", "--out", "source"]);
    assert!(output.status.success());
    fs::write(
        root.path("edit.json"),
        r#"{"interface":1,"semantics":"1.0","ops":[
          {"op":"stamp","targets":["icon/paint/base"],"source":"icon/paint/base",
           "rect":[1,0,1,1],"at":[0,0]}
        ]}"#,
    )
    .unwrap();
    let output = root.run(&[
        "run",
        "edit.json",
        "--project",
        "source",
        "--out",
        "revised",
    ]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        pixels(&root.path("revised/images/000000.png")),
        [[128, 0, 127, 255], [255, 0, 0, 128]]
    );

    for (index, source) in [
        r#"{"op":"stamp","targets":["icon/paint/base"],"source":"icon/paint/base","rect":[1,0,2,1],"at":[0,0]}"#,
        r#"{"op":"stamp","targets":["icon/paint/base"],"source":"icon/paint/base","rect":[0,0,2,1],"at":[1,0]}"#,
        r#"{"op":"stamp","targets":["icon/paint/base"],"source":"other/layer/frame","at":[0,0]}"#,
        r#"{"op":"stamp","targets":["icon/paint/base"],"source":"icon/paint/base","mode":"multiply"}"#,
    ]
    .iter()
    .enumerate()
    {
        let job = format!(
            r#"{{"interface":1,"semantics":"1.0","ops":[{source}]}}"#
        );
        fs::write(root.path("invalid.json"), job).unwrap();
        let out = format!("failed-{index}");
        let result = root.run(&[
            "run", "invalid.json", "--project", "source", "--out", &out,
        ]);
        assert!(!result.status.success(), "invalid stamp {index} succeeded");
        assert!(!root.path(&out).exists());
    }
    assert_eq!(
        pixels(&root.path("source/images/000000.png")),
        [[0, 0, 255, 255], [255, 0, 0, 128]]
    );
}
