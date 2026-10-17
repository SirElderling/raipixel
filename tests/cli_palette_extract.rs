//! End-to-end AI palette extraction with deterministic names and exact pixels.
use raipixel::{Image, Limits, Work, png};
use std::{fs, path::PathBuf, process::Command};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        let i = NEXT.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "raipixel-extract-{}-{i}",
            std::process::id()
        ));
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
            .unwrap()
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn colors(path: &std::path::Path) -> Vec<[u8; 4]> {
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
fn palette_extract_counts_visible_rgb_orders_names_and_survives_reopen() {
    let root = Scratch::new();
    fs::write(
        root.file("job.json"),
        r##"{"interface":1,"semantics":"1.0","ops":[
          {"op":"asset.create","name":"icon","size":[3,2]},
          {"op":"layer.create","asset":"icon","name":"paint"},
          {"op":"frame.create","asset":"icon","name":"base"},
          {"op":"cel.create","target":"icon/paint/base"},
          {"op":"grid","targets":["icon/paint/base"],"rows":["AB.","BBC"],
           "symbols":{"A":"#EF11AA00","B":"#112233FF",
                      "C":"#01020380",".":"#00000000"}},
          {"op":"palette.extract","source":"icon/paint/base","name":"observed"},
          {"op":"draw","targets":["icon/paint/base"],"shape":"pixel",
           "at":[0,0],"color":"@observed/c000"}
        ]}"##,
    )
    .unwrap();
    let first = root.run(&["run", "job.json", "--out", "initial"]);
    assert!(
        first.status.success(),
        "{}",
        String::from_utf8_lossy(&first.stderr)
    );
    let manifest = fs::read_to_string(root.file("initial/project.json")).unwrap();
    assert!(manifest.contains(r##""name":"c000","rgb":"#112233""##));
    assert!(manifest.contains(r##""name":"c001","rgb":"#010203""##));
    assert!(!manifest.contains(r##""rgb":"#EF11AA""##));
    assert_eq!(
        colors(&root.file("initial/images/000000.png")),
        [
            [17, 34, 51, 255],
            [17, 34, 51, 255],
            [0, 0, 0, 0],
            [17, 34, 51, 255],
            [17, 34, 51, 255],
            [1, 2, 3, 128]
        ]
    );

    fs::write(
        root.file("edit.json"),
        r##"{"interface":1,"semantics":"1.0","ops":[
          {"op":"draw","targets":["icon/paint/base"],"shape":"pixel",
           "at":[2,0],"color":"@observed/c001"}
        ],"exports":[{"name":"game","kind":"frame","targets":["icon/base"]}]}"##,
    )
    .unwrap();
    let next = root.run(&[
        "run", "edit.json", "--project", "initial", "--out", "reopened",
    ]);
    assert!(
        next.status.success(),
        "{}",
        String::from_utf8_lossy(&next.stderr)
    );
    let pixels = colors(&root.file("reopened/artifacts/game/image.png"));
    assert_eq!(pixels[2], [1, 2, 3, 255]);
    assert_eq!(pixels[5], [1, 2, 3, 128]);
    assert!(root.file("initial/project.json").exists());
}

#[test]
fn palette_extract_rejects_more_colors_than_limit_without_publication() {
    let root = Scratch::new();
    let pixels: Vec<_> = (0..257u16)
        .map(|i| [(i & 255) as u8, (i >> 8) as u8, 17, 255])
        .collect();
    let image = Image::from_pixels(
        257, 1, &pixels, Limits::default(), &mut Work::default(),
    ).unwrap();
    let mut encoded = Vec::new();
    png::encode(&image, &mut encoded, &mut Work::default()).unwrap();
    fs::write(root.file("input.png"), encoded).unwrap();
    fs::write(
        root.file("job.json"),
        r#"{"interface":1,"semantics":"1.0","ops":[
          {"op":"asset.create","name":"icon","size":[257,1]},
          {"op":"layer.create","asset":"icon","name":"paint"},
          {"op":"frame.create","asset":"icon","name":"base"},
          {"op":"png.import","target":"icon/paint/base","file":"input.png"},
          {"op":"palette.extract","source":"icon/paint/base",
           "name":"too_many","max_colors":256}
        ]}"#,
    ).unwrap();
    let result = root.run(&["run", "job.json", "--out", "rejected"]);
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("E_PALETTE_TOO_MANY"));
    assert!(!root.file("rejected").exists());
}
