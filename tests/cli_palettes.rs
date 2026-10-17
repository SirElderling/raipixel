//! End-to-end palette state, symbolic color resolution and lossless native reopen.
use raipixel::{Limits, Work, png};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);

struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        let next = NEXT.fetch_add(1, Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("raipixel-palette-{}-{next}", std::process::id()));
        fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn file(&self, path: &str) -> PathBuf {
        self.0.join(path)
    }

    fn run(&self, args: &[&str]) -> std::process::Output {
        Command::new(env!("CARGO_BIN_EXE_raipixel"))
            .current_dir(&self.0)
            .args(args)
            .output()
            .expect("launch raipixel")
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn decode(path: &Path) -> Vec<[u8; 4]> {
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
fn named_palette_resolves_opaque_colors_and_survives_native_round_trip() {
    let scratch = Scratch::new();
    fs::write(
        scratch.file("create.json"),
        r##"{"interface":1,"semantics":"1.0","ops":[
          {"op":"palette.set","name":"metal","entries":[
            {"name":"edge","rgb":"#112233"},
            {"name":"steel","rgb":"#445566"}]},
          {"op":"asset.create","name":"icon","size":[3,2],"palette":"metal"},
          {"op":"layer.create","asset":"icon","name":"paint"},
          {"op":"frame.create","asset":"icon","name":"base"},
          {"op":"cel.create","target":"icon/paint/base"},
          {"op":"grid","targets":["icon/paint/base"],"rows":["ABS","BAB"],
           "symbols":{"A":"@metal/edge","B":"@metal/steel","S":"#01020300"}},
          {"op":"draw","shape":"pixel","targets":["icon/paint/base"],
           "at":[2,1],"color":"@metal/edge"}
        ],"exports":[{"name":"game","kind":"frame","targets":["icon/base"]}]}"##,
    )
    .unwrap();

    let first = scratch.run(&["run", "create.json", "--out", "original"]);
    assert!(
        first.status.success(),
        "{}",
        String::from_utf8_lossy(&first.stderr)
    );
    let expected = [
        [17, 34, 51, 255],
        [68, 85, 102, 255],
        [1, 2, 3, 0],
        [68, 85, 102, 255],
        [17, 34, 51, 255],
        [17, 34, 51, 255],
    ];
    assert_eq!(
        decode(&scratch.file("original/artifacts/game/image.png")),
        expected
    );
    assert_eq!(
        decode(&scratch.file("original/images/000000.png")),
        expected
    );
    let snapshot_before = fs::read(scratch.file("original/project.json")).unwrap();
    let snapshot_text = String::from_utf8_lossy(&snapshot_before);
    assert!(snapshot_text.contains(r#""name":"metal""#));
    assert!(snapshot_text.contains(r##""rgb":"#112233""##));
    assert!(snapshot_text.contains(r##""rgb":"#445566""##));

    // Replacing a palette does not retroactively alter stored pixels.
    fs::write(
        scratch.file("replace.json"),
        r##"{"interface":1,"semantics":"1.0","ops":[
          {"op":"palette.set","name":"metal","replace":true,"entries":[
            {"name":"edge","rgb":"#778899"},
            {"name":"steel","rgb":"#AABBCC"}]},
          {"op":"draw","shape":"pixel","targets":["icon/paint/base"],
           "at":[0,0],"color":"@metal/edge"}
        ],"exports":[{"name":"game","kind":"frame","targets":["icon/base"]}]}"##,
    )
    .unwrap();
    let next = scratch.run(&[
        "run",
        "replace.json",
        "--project",
        "original",
        "--out",
        "revised",
    ]);
    assert!(
        next.status.success(),
        "{}",
        String::from_utf8_lossy(&next.stderr)
    );
    let mut changed = expected;
    changed[0] = [119, 136, 153, 255];
    assert_eq!(decode(&scratch.file("revised/images/000000.png")), changed);
    assert_eq!(
        decode(&scratch.file("revised/artifacts/game/image.png")),
        changed
    );
    assert_eq!(
        fs::read(scratch.file("original/project.json")).unwrap(),
        snapshot_before,
        "source project was modified during reopen"
    );

    // A no-op reopen preserves both palette state and canonical PNG output.
    fs::write(
        scratch.file("reopen.json"),
        r#"{"interface":1,"semantics":"1.0","ops":[]}"#,
    )
    .unwrap();
    let reopened = scratch.run(&[
        "run",
        "reopen.json",
        "--project",
        "revised",
        "--out",
        "reopened",
    ]);
    assert!(
        reopened.status.success(),
        "{}",
        String::from_utf8_lossy(&reopened.stderr)
    );
    assert_eq!(
        fs::read(scratch.file("revised/project.json")).unwrap(),
        fs::read(scratch.file("reopened/project.json")).unwrap()
    );
    assert_eq!(
        fs::read(scratch.file("revised/images/000000.png")).unwrap(),
        fs::read(scratch.file("reopened/images/000000.png")).unwrap()
    );
}

#[test]
fn invalid_palette_commands_never_publish_and_preserve_source_snapshot() {
    let scratch = Scratch::new();
    let prefix = r##"{"interface":1,"semantics":"1.0","ops":["##;
    let suffix = "]}";
    let failures = [
        r##"{"op":"palette.set","name":"p","entries":[{"name":"a","rgb":"#010203"},{"name":"a","rgb":"#040506"}]}"##,
        r##"{"op":"palette.set","name":"p","entries":[{"name":"a","rgb":"#010203FF"}]}"##,
        r##"{"op":"palette.set","name":"p","replace":true,"entries":[{"name":"a","rgb":"#010203"}]}"##,
        r##"{"op":"palette.set","name":"p","entries":[{"name":"a","rgb":"#010203"}]},{"op":"palette.set","name":"p","entries":[{"name":"b","rgb":"#040506"}]}"##,
        r##"{"op":"asset.create","name":"x","size":[2,2],"palette":"missing"}"##,
        r##"{"op":"asset.create","name":"x","size":[2,2]},{"op":"layer.create","asset":"x","name":"l"},{"op":"frame.create","asset":"x","name":"f"},{"op":"cel.create","target":"x/l/f"},{"op":"draw","targets":["x/l/f"],"shape":"pixel","at":[0,0],"color":"@missing/edge"}"##,
    ];
    for (i, operations) in failures.iter().enumerate() {
        let job = format!("{prefix}{operations}{suffix}");
        fs::write(scratch.file("bad.json"), job).unwrap();
        let out = format!("rejected-{i}");
        let result = scratch.run(&["run", "bad.json", "--out", &out]);
        assert!(
            !result.status.success(),
            "invalid palette fixture {i} was accepted"
        );
        assert!(!scratch.file(&out).exists(), "invalid job was published");
    }
}
