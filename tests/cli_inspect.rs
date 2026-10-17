//! Read-only AI inspection workflows against persisted exact-RGBA snapshots.
use raipixel::{Work, json};
use std::{collections::BTreeMap, fs, path::PathBuf, process::Command};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);

struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        let i = NEXT.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "raipixel-inspect-{}-{i}",
            std::process::id()
        ));
        fs::create_dir(&root).unwrap();
        Self(root)
    }
    fn file(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
    fn run(&self, args: &[&str]) -> std::process::Output {
        Command::new(env!("CARGO_BIN_EXE_raipixel"))
            .current_dir(&self.0)
            .args(args)
            .output()
            .expect("launch raipixel")
    }
    fn project(&self) {
        fs::write(
            self.file("author.json"),
            r##"{"interface":1,"semantics":"1.0","ops":[
              {"op":"palette.set","name":"p","entries":[{"name":"red","rgb":"#FF0000"}]},
              {"op":"asset.create","name":"test","size":[3,2],"palette":"p"},
              {"op":"layer.create","asset":"test","name":"paint"},
              {"op":"frame.create","asset":"test","name":"base"},
              {"op":"cel.create","target":"test/paint/base"},
              {"op":"grid","targets":["test/paint/base"],"rows":["AB.","CAB"],
                "symbols":{"A":"#0A141E00","B":"@p/red","C":"#04050680",".":"#00000000"}}
            ]}"##,
        )
        .unwrap();
        let result = self.run(&["run", "author.json", "--out", "project"]);
        assert!(
            result.status.success(),
            "authoring failed: {}",
            String::from_utf8_lossy(&result.stderr)
        );
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn parsed(data: &[u8]) -> json::Value {
    json::parse(data, json::Limits::default(), &mut Work::default()).unwrap()
}
fn object(value: &json::Value) -> &BTreeMap<String, json::Value> {
    match value {
        json::Value::Object(map) => map,
        _ => panic!("expected object"),
    }
}
fn number(value: &json::Value) -> u64 {
    match value {
        json::Value::Number(n) => n.as_u64().unwrap(),
        _ => panic!("expected number"),
    }
}
fn array(value: &json::Value) -> &[json::Value] {
    match value {
        json::Value::Array(values) => values,
        _ => panic!("expected array"),
    }
}

#[test]
fn inspect_project_and_canvas_region_report_exact_pixels_and_histogram() {
    let root = Scratch::new();
    root.project();
    let manifest = fs::read(root.file("project/project.json")).unwrap();
    let png = fs::read(root.file("project/images/000000.png")).unwrap();
    let simple = root.run(&["inspect", "project"]);
    assert!(
        simple.status.success(),
        "{}",
        String::from_utf8_lossy(&simple.stderr)
    );
    let value = parsed(&simple.stdout);
    let result = object(object(&value).get("result").unwrap());
    assert_eq!(
        result.get("assets"),
        Some(&json::Value::Array(vec![json::Value::String("test".into())]))
    );
    assert_eq!(
        result.get("palettes"),
        Some(&json::Value::Array(vec![json::Value::String("p".into())]))
    );

    fs::write(
        root.file("query.json"),
        r#"{"target":"test/paint/base","region":[0,0,3,2],
             "pixels":true,"histogram_limit":2}"#,
    )
    .unwrap();
    let inspected = root.run(&["inspect", "project", "--query", "query.json"]);
    assert!(
        inspected.status.success(),
        "{}",
        String::from_utf8_lossy(&inspected.stderr)
    );
    let inspected_value = parsed(&inspected.stdout);
    let stats = object(object(&inspected_value).get("result").unwrap());
    assert_eq!(number(stats.get("pixel_count").unwrap()), 6);
    assert_eq!(number(stats.get("zero_alpha").unwrap()), 3);
    assert_eq!(number(stats.get("partial_alpha").unwrap()), 1);
    assert_eq!(number(stats.get("opaque_alpha").unwrap()), 2);
    assert_eq!(number(stats.get("unique_colors").unwrap()), 4);
    assert_eq!(stats.get("histogram_truncated"), Some(&json::Value::Bool(true)));
    let entries = array(stats.get("histogram").unwrap());
    assert_eq!(entries.len(), 2);
    assert_eq!(number(object(&entries[0]).get("count").unwrap()), 2);
    assert_eq!(number(object(&entries[1]).get("count").unwrap()), 2);
    let pixels = array(stats.get("pixels").unwrap());
    assert_eq!(pixels.len(), 6);
    assert_eq!(
        array(&pixels[0]).iter().map(number).collect::<Vec<_>>(),
        [10, 20, 30, 0]
    );
    assert_eq!(
        array(&pixels[3]).iter().map(number).collect::<Vec<_>>(),
        [4, 5, 6, 128]
    );
    assert_eq!(fs::read(root.file("project/project.json")).unwrap(), manifest);
    assert_eq!(fs::read(root.file("project/images/000000.png")).unwrap(), png);
}

#[test]
fn inspect_rejects_invalid_targets_regions_and_unimplemented_features() {
    let root = Scratch::new();
    root.project();
    for (i, query) in [
        r#"{"target":"missing/layer/frame"}"#,
        r#"{"target":"test/paint/base","region":[-1,0,1,1]}"#,
        r#"{"target":"test/paint/base","histogram_limit":0}"#,
        r#"{"target":"test/paint/base","symbols":true}"#,
        r#"{"target":"test/paint/base","unrecognized":true}"#,
        r#"{"pixels":true}"#,
    ]
    .iter()
    .enumerate()
    {
        fs::write(root.file("query.json"), query).unwrap();
        let result = root.run(&["inspect", "project", "--query", "query.json"]);
        assert!(!result.status.success(), "invalid query {i} was accepted");
        assert!(result.stdout.is_empty(), "failed query wrote stdout");
        assert!(String::from_utf8_lossy(&result.stderr).contains(r#""ok":false"#));
    }
}
