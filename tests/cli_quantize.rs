//! Agent-origin nearest-palette quantization; exact PNG and saved-state assertions.
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
fn agent_quantization_is_exact_and_saved_palette_remains_editable() {
    let root = std::env::temp_dir().join(format!(
        "raipixel-quantize-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    fs::create_dir(&root).unwrap();
    fs::write(
        root.join("create.json"),
        r##"{"interface":1,"semantics":"1.0","ops":[
            {"op":"palette.set","name":"ramp","entries":[
                {"name":"dark","rgb":"#000000"},
                {"name":"light","rgb":"#0A0000"}]},
            {"op":"asset.create","name":"icon","size":[3,2],"palette":"ramp"},
            {"op":"layer.create","asset":"icon","name":"paint"},
            {"op":"frame.create","asset":"icon","name":"base"},
            {"op":"cel.create","target":"icon/paint/base"},
            {"op":"grid","targets":["icon/paint/base"],"rows":["ABC","DEF"],
             "symbols":{"A":"#050000FF","B":"#06000080","C":"#02030400",
                        "D":"#07000001","E":"#0A0000FF","F":"#03060900"}}
        ]}"##,
    )
    .unwrap();
    let first = run(&root, &["run", "create.json", "--out", "first"]);
    assert!(
        first.status.success(),
        "create: {}",
        String::from_utf8_lossy(&first.stderr)
    );
    fs::write(
        root.join("quantize.json"),
        r#"{"interface":1,"semantics":"1.0","ops":[
            {"op":"palette.quantize","targets":["icon/paint/base"],"palette":"ramp",
             "region":[0,0,3,1],"dither":"none"}
        ],"exports":[{"name":"png","kind":"frame","targets":["icon/base"]}]}"#,
    )
    .unwrap();
    let changed = run(
        &root,
        &[
            "run",
            "quantize.json",
            "--project",
            "first",
            "--out",
            "second",
        ],
    );
    assert!(
        changed.status.success(),
        "quantize: {}",
        String::from_utf8_lossy(&changed.stderr)
    );
    let original = png::decode(
        &fs::read(root.join("first/images/000000.png")).unwrap(),
        Limits::default(),
        &mut Work::default(),
    )
    .unwrap()
    .image;
    let edited = png::decode(
        &fs::read(root.join("second/artifacts/png/image.png")).unwrap(),
        Limits::default(),
        &mut Work::default(),
    )
    .unwrap()
    .image;
    assert_eq!(
        edited.pixels(),
        &[
            [0, 0, 0, 255],
            [10, 0, 0, 128],
            [2, 3, 4, 0],
            [7, 0, 0, 1],
            [10, 0, 0, 255],
            [3, 6, 9, 0],
        ]
    );
    assert_eq!(original.pixel(0, 0), Some([5, 0, 0, 255]));
    let baseline = fs::read(root.join("first/project.json")).unwrap();
    let current = fs::read(root.join("second/project.json")).unwrap();
    assert_eq!(
        baseline, current,
        "quantization silently altered non-raster metadata"
    );

    fs::write(
        root.join("bad.json"),
        r#"{"interface":1,"semantics":"1.0","ops":[
       {"op":"palette.quantize","targets":["icon/paint/base"],
        "palette":"ramp","dither":"bayer4"}]}"#,
    )
    .unwrap();
    let invalid = run(
        &root,
        &["run", "bad.json", "--project", "first", "--out", "rejected"],
    );
    assert!(!invalid.status.success());
    assert!(!root.join("rejected").exists());
    fs::remove_dir_all(&root).unwrap();
}
