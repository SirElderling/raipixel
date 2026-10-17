//! Exact visible-palette constraints; hidden RGB is never silently rewritten.
use raipixel::{Error, Image, Limits, Resource, Work};
use std::{fs, process::Command};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);

#[test]
fn visible_palette_membership_is_bounded_exact_and_alpha_aware() {
    let image = Image::from_pixels(
        3,
        1,
        &[
            [99, 88, 77, 0],
            [1, 2, 3, 1],
            [4, 5, 6, 255],
        ],
        Limits::default(),
        &mut Work::default(),
    )
    .unwrap();
    let mut work = Work::new(3).unwrap();
    assert_eq!(
        image.first_visible_color_outside_palette(
            &[[4, 5, 6], [1, 2, 3]],
            &mut work
        ),
        Ok(None)
    );
    assert_eq!(work.remaining(), 0);
    assert_eq!(
        image.first_visible_color_outside_palette(
            &[[1, 2, 3]],
            &mut Work::default()
        ),
        Ok(Some(([2, 0], [4, 5, 6])))
    );

    let mut limited = Work::new(2).unwrap();
    assert_eq!(
        image.first_visible_color_outside_palette(&[[1, 2, 3]], &mut limited),
        Err(Error::Limit(Resource::Work))
    );
    assert_eq!(limited.remaining(), 2);
    assert_eq!(
        image.first_visible_color_outside_palette(&[], &mut limited),
        Err(Error::PixelCount)
    );
    assert_eq!(limited.remaining(), 2);
    assert_eq!(image.pixel(0, 0), Some([99, 88, 77, 0]));
}

#[test]
fn palette_check_accepts_hidden_rgb_but_rejects_visible_mismatch_before_publish() {
    let id = NEXT.fetch_add(1, Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!(
        "raipixel-palette-check-{}-{id}",
        std::process::id()
    ));
    fs::create_dir(&root).unwrap();
    let run = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_raipixel"))
            .current_dir(&root)
            .args(args)
            .output()
            .unwrap()
    };
    fs::write(
        root.join("create.json"),
        r##"{"interface":1,"semantics":"1.0","ops":[
          {"op":"palette.set","name":"p","entries":[{"name":"only","rgb":"#010203"}]},
          {"op":"asset.create","name":"icon","size":[2,1],"palette":"p"},
          {"op":"layer.create","asset":"icon","name":"paint"},
          {"op":"frame.create","asset":"icon","name":"base"},
          {"op":"cel.create","target":"icon/paint/base"},
          {"op":"grid","targets":["icon/paint/base"],"rows":["AH"],
            "symbols":{"A":"@p/only","H":"#AABBCC00"}}
        ],"checks":["structure","palette","alpha","bounds"]}"##,
    )
    .unwrap();
    let created = run(&["run", "create.json", "--out", "good"]);
    assert!(
        created.status.success(),
        "{}",
        String::from_utf8_lossy(&created.stderr)
    );
    assert!(root.join("good/project.json").exists());

    fs::write(
        root.join("change.json"),
        r##"{"interface":1,"semantics":"1.0","ops":[
          {"op":"draw","targets":["icon/paint/base"],"shape":"pixel",
           "at":[1,0],"color":"#AABBCC01"}
        ],"checks":["palette"]}"##,
    )
    .unwrap();
    let rejected = run(&[
        "run", "change.json", "--project", "good", "--out", "rejected",
    ]);
    assert!(!rejected.status.success());
    assert!(String::from_utf8_lossy(&rejected.stderr).contains("E_PROJECT"));
    assert!(!root.join("rejected").exists());

    // Validation failure never mutates the imported source snapshot.
    assert!(root.join("good/images/000000.png").exists());
    let _ = fs::remove_dir_all(root);
}
