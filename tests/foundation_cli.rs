//! End-to-end executable Foundation regressions. No external test dependencies.
use raipixel::{png, Image, Limits, Work};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);

struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        let id = NEXT.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "raipixel-cli-{}-{id}", std::process::id()
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn path(&self, suffix: &str) -> PathBuf {
        self.0.join(suffix)
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn run(args: &[&Path]) -> std::process::Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_raipixel"));
    for arg in args {
        command.arg(arg);
    }
    command.output().unwrap()
}
fn word(arg: &'static str) -> &'static Path {
    Path::new(arg)
}
fn decode(path: &Path) -> Image {
    png::decode(
        &fs::read(path).unwrap(),
        Limits::default(),
        &mut Work::default(),
    )
    .unwrap()
    .image
}

#[test]
fn single_cel_can_be_authored_saved_reopened_and_corrected_without_changing_source() {
    let scratch = Scratch::new();
    let job = scratch.path("create.json");
    let out = scratch.path("first");
    fs::write(
        &job,
        r##"{"interface":1,"semantics":"1.0","ops":[
            {"op":"asset.create","name":"icon","size":[4,3],"pivot":[2,2]},
            {"op":"layer.create","asset":"icon","name":"source"},
            {"op":"frame.create","asset":"icon","name":"base"},
            {"op":"cel.create","target":"icon/source/base"},
            {"op":"grid","targets":["icon/source/base"],"rows":["abc.","aaaa","...."],
                "symbols":{"a":"#12003400","b":"#112233FF","c":"#ABCDEF80","." :"#00000000"}},
            {"op":"draw","targets":["icon/source/base"],"shape":"line","points":[[0,2],[3,2]],"color":"#010203FF"}
        ],"exports":[{"name":"preview","kind":"frame","targets":["icon/base"],"scale":2}]}"##,
    )
    .unwrap();
    let result = run(&[word("run"), job.as_path(), word("--out"), out.as_path()]);
    assert!(result.status.success(), "{}", String::from_utf8_lossy(&result.stderr));
    assert_eq!(decode(&out.join("images/000000.png")).pixel(0, 0), Some([18, 0, 52, 0]));
    assert_eq!(decode(&out.join("images/000000.png")).pixel(2, 0), Some([171, 205, 239, 128]));
    let preview = decode(&out.join("artifacts/preview/image.png"));
    assert_eq!(preview.size(), [8, 6]);
    assert_eq!(preview.pixel(5, 0), Some([171, 205, 239, 128]));

    let changed_job = scratch.path("change.json");
    let second = scratch.path("second");
    fs::write(
        &changed_job,
        r##"{"interface":1,"semantics":"1.0","ops":[
            {"op":"draw","targets":["icon/source/base"],"shape":"pixel",
             "at":[1,0],"color":"#AABBCC00"}
        ],"exports":[{"name":"png","kind":"frame","targets":["icon/base"]}]}"##,
    )
    .unwrap();
    let result = run(&[
        word("run"), changed_job.as_path(), word("--project"), out.as_path(),
        word("--out"), second.as_path(),
    ]);
    assert!(result.status.success(), "{}", String::from_utf8_lossy(&result.stderr));
    let original = decode(&out.join("images/000000.png"));
    let edited = decode(&second.join("images/000000.png"));
    assert_eq!(original.pixel(1, 0), Some([17, 34, 51, 255]));
    assert_eq!(edited.pixel(1, 0), Some([170, 187, 204, 0]));
    for y in 0..3 {
        for x in 0..4 {
            if [x, y] != [1, 0] {
                assert_eq!(original.pixel(x, y), edited.pixel(x, y));
            }
        }
    }
    let manifest = fs::read(second.join("project.json")).unwrap();
    assert!(manifest.ends_with(b"\n"));
    let parsed = raipixel::json::parse(
        &manifest, raipixel::json::Limits::default(), &mut Work::default()
    );
    assert!(parsed.is_ok());
}

#[test]
fn input_failure_or_dry_run_does_not_publish_a_directory() {
    let scratch = Scratch::new();
    let job = scratch.path("bad.json");
    let out = scratch.path("not-created");
    fs::write(
        &job,
        r#"{"interface":1,"semantics":"1.0","ops":[
            {"op":"asset.create","name":"x","size":[2,2]},
            {"op":"unknown.method"}
        ]}"#,
    )
    .unwrap();
    let failed = run(&[word("run"), job.as_path(), word("--out"), out.as_path()]);
    assert!(!failed.status.success());
    assert!(!out.exists());
    assert!(String::from_utf8_lossy(&failed.stderr).contains("E_CAPABILITY"));

    fs::write(
        &job,
        r#"{"interface":1,"semantics":"1.0","ops":[
            {"op":"asset.create","name":"x","size":[2,2]},
            {"op":"layer.create","asset":"x","name":"l"},
            {"op":"frame.create","asset":"x","name":"f"},
            {"op":"cel.create","target":"x/l/f"}
        ]}"#,
    )
    .unwrap();
    let dry = run(&[
        word("run"), job.as_path(), word("--out"), out.as_path(), word("--dry-run")
    ]);
    assert!(dry.status.success(), "{}", String::from_utf8_lossy(&dry.stderr));
    assert!(!out.exists());
}

#[test]
fn external_png_import_retains_transparent_rgb_and_only_replaces_requested_sample() {
    let scratch = Scratch::new();
    let original = Image::from_pixels(
        2,
        1,
        &[[91, 82, 73, 0], [10, 20, 30, 255]],
        Limits::default(),
        &mut Work::default(),
    )
    .unwrap();
    let mut bytes = Vec::new();
    png::encode(&original, &mut bytes, &mut Work::default()).unwrap();
    fs::write(scratch.path("input.png"), bytes).unwrap();
    let job = scratch.path("correct.json");
    fs::write(
        &job,
        r##"{"interface":1,"semantics":"1.0","ops":[
            {"op":"asset.create","name":"icon","size":[2,1]},
            {"op":"layer.create","asset":"icon","name":"source"},
            {"op":"frame.create","asset":"icon","name":"base"},
            {"op":"png.import","target":"icon/source/base","file":"input.png"},
            {"op":"draw","targets":["icon/source/base"],"shape":"pixel",
             "at":[1,0],"color":"#DEADBEEF"}
        ]}"##,
    )
    .unwrap();
    let out = scratch.path("output");
    let result = run(&[word("run"), job.as_path(), word("--out"), out.as_path()]);
    assert!(result.status.success(), "{}", String::from_utf8_lossy(&result.stderr));
    let edited = decode(&out.join("images/000000.png"));
    assert_eq!(edited.pixel(0, 0), Some([91, 82, 73, 0]));
    assert_eq!(edited.pixel(1, 0), Some([222, 173, 190, 239]));
}

#[test]
fn unsupported_snapshot_content_is_rejected_instead_of_discarded() {
    let scratch = Scratch::new();
    let job = scratch.path("initial.json");
    let first = scratch.path("first");
    fs::write(&job, r#"{"interface":1,"semantics":"1.0","ops":[
        {"op":"asset.create","name":"x","size":[1,1]},
        {"op":"layer.create","asset":"x","name":"l"},
        {"op":"frame.create","asset":"x","name":"f"},
        {"op":"cel.create","target":"x/l/f"}
    ]}"#).unwrap();
    assert!(run(&[word("run"), job.as_path(), word("--out"), first.as_path()]).status.success());
    let manifest = first.join("project.json");
    let content = fs::read_to_string(&manifest).unwrap();
    fs::write(&manifest, content.replace(r#""palettes":[]"#, r#""palettes":[{"name":"keep","entries":[]}]"#)).unwrap();
    let next = scratch.path("next");
    fs::write(&job, r#"{"interface":1,"semantics":"1.0","ops":[]}"#).unwrap();
    let result = run(&[
        word("run"), job.as_path(), word("--project"), first.as_path(),
        word("--out"), next.as_path()
    ]);
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("E_CAPABILITY"));
    assert!(!next.exists());
}
