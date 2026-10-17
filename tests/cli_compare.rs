//! Exact snapshot comparison through the AI inspection interface.
use raipixel::{Work, json};
use std::{
    collections::BTreeMap,
    fs,
    path::PathBuf,
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT: AtomicU64 = AtomicU64::new(0);

struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        let id = NEXT.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "raipixel-compare-{}-{id}",
            std::process::id()
        ));
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
    fn make_projects(&self) {
        fs::write(
            self.path("first.json"),
            r##"{"interface":1,"semantics":"1.0","ops":[
                {"op":"asset.create","name":"sample","size":[3,2]},
                {"op":"layer.create","asset":"sample","name":"ink"},
                {"op":"frame.create","asset":"sample","name":"base"},
                {"op":"cel.create","target":"sample/ink/base"},
                {"op":"grid","targets":["sample/ink/base"],
                  "rows":["ABC","DEF"],"symbols":{
                  "A":"#01020300","B":"#040506FF","C":"#060606FF",
                  "D":"#08090A00","E":"#0B0C0D5A","F":"#0E0F1000"}}
             ]}"##,
        )
        .unwrap();
        let result = self.run(&["run", "first.json", "--out", "first"]);
        assert!(result.status.success(), "{}", String::from_utf8_lossy(&result.stderr));
        fs::write(
            self.path("edit.json"),
            r##"{"interface":1,"semantics":"1.0","ops":[
                {"op":"draw","targets":["sample/ink/base"],"shape":"pixel","at":[0,0],"color":"#03020100"},
                {"op":"draw","targets":["sample/ink/base"],"shape":"pixel","at":[1,0],"color":"#0405065A"},
                {"op":"draw","targets":["sample/ink/base"],"shape":"pixel","at":[2,0],"color":"#070606FF"},
                {"op":"draw","targets":["sample/ink/base"],"shape":"pixel","at":[0,1],"color":"#08090AFF"}
             ]}"##,
        )
        .unwrap();
        let result = self.run(&[
            "run", "edit.json", "--project", "first", "--out", "second",
        ]);
        assert!(result.status.success(), "{}", String::from_utf8_lossy(&result.stderr));
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn obj(value: &json::Value) -> &BTreeMap<String, json::Value> {
    match value {
        json::Value::Object(v) => v,
        _ => panic!("expected JSON object"),
    }
}
fn number(value: &json::Value) -> u64 {
    match value {
        json::Value::Number(v) => v.as_u64().unwrap(),
        _ => panic!("expected integer"),
    }
}
fn compare_output(out: &[u8]) -> json::Value {
    json::parse(out, json::Limits::default(), &mut Work::default()).unwrap()
}
#[test]
fn against_reports_exact_changed_rgba_and_preserves_both_snapshots() {
    let root = Scratch::new();
    root.make_projects();
    let first_png = fs::read(root.path("first/images/000000.png")).unwrap();
    let second_png = fs::read(root.path("second/images/000000.png")).unwrap();
    fs::write(
        root.path("compare.json"),
        r#"{"target":"sample/ink/base","against":{"project":"first","target":"sample/ink/base"}}"#,
    )
    .unwrap();
    let output = root.run(&["inspect", "second", "--query", "compare.json"]);
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let parsed = compare_output(&output.stdout);
    let result = obj(obj(&parsed).get("result").unwrap());
    let diff = obj(result.get("against").unwrap());
    assert_eq!(number(diff.get("changed_samples").unwrap()), 4);
    assert_eq!(number(diff.get("changed_visible_colors").unwrap()), 2);
    assert_eq!(number(diff.get("changed_alpha").unwrap()), 2);
    let bbox = match diff.get("changed_bbox").unwrap() {
        json::Value::Array(values) => values.iter().map(number).collect::<Vec<_>>(),
        _ => panic!("expected changed rectangle"),
    };
    assert_eq!(bbox, [0, 0, 3, 2]);
    assert_eq!(diff.get("unrelated_state_equal"), Some(&json::Value::Bool(true)));

    fs::write(
        root.path("region.json"),
        r#"{"target":"sample/ink/base","region":[0,0,2,1],
            "against":{"project":"first","target":"sample/ink/base"}}"#,
    )
    .unwrap();
    let partial = root.run(&["inspect", "second", "--query", "region.json"]);
    assert!(partial.status.success(), "{}", String::from_utf8_lossy(&partial.stderr));
    let parsed = compare_output(&partial.stdout);
    let partial_diff = obj(obj(obj(&parsed).get("result").unwrap()).get("against").unwrap());
    assert_eq!(number(partial_diff.get("changed_samples").unwrap()), 2);
    assert_eq!(number(partial_diff.get("changed_visible_colors").unwrap()), 0);
    assert_eq!(number(partial_diff.get("changed_alpha").unwrap()), 1);
    assert_eq!(fs::read(root.path("first/images/000000.png")).unwrap(), first_png);
    assert_eq!(fs::read(root.path("second/images/000000.png")).unwrap(), second_png);
}

#[test]
fn against_rejects_traversal_mismatched_target_and_missing_owner() {
    let root = Scratch::new();
    root.make_projects();
    for request in [
        r#"{"target":"sample/ink/base","against":{"project":"../secret","target":"sample/ink/base"}}"#,
        r#"{"target":"sample/ink/base","against":{"project":"first","target":"sample/base"}}"#,
        r#"{"target":"sample/ink/base","against":{"project":"first","target":"unknown"}}"#,
        r#"{"against":{"project":"first","target":"sample/ink/base"}}"#,
        r#"{"target":"sample/ink/base","against":{"project":"first","target":"sample/ink/base","oops":true}}"#,
    ] {
        fs::write(root.path("invalid.json"), request).unwrap();
        let output = root.run(&["inspect", "second", "--query", "invalid.json"]);
        assert!(!output.status.success(), "invalid query accepted: {request}");
        assert!(output.stdout.is_empty());
    }
}
