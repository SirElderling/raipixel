//! Minimal executable Foundation vertical slice: one editable cel and frame.
//! Deliberately rejects unsupported v1 structures instead of dropping state.
use raipixel::{Blend, Image, Limits, Pixel, Rect, Work, json, png};
use std::collections::BTreeMap;
use std::fs;
use std::io::Read;
use std::path::{Component, Path, PathBuf};

type Map = BTreeMap<String, json::Value>;
type PaletteEntries = Vec<(String, [u8; 3])>;
type Palettes = BTreeMap<String, PaletteEntries>;
type Result<T> = std::result::Result<T, Fault>;

#[derive(Debug)]
struct Fault {
    code: &'static str,
    message: String,
}
fn fail<T>(code: &'static str, message: impl Into<String>) -> Result<T> {
    Err(Fault {
        code,
        message: message.into(),
    })
}
fn argument<T>(message: impl Into<String>) -> Result<T> {
    fail("E_ARGUMENT", message)
}
fn capability<T>(message: impl Into<String>) -> Result<T> {
    fail("E_CAPABILITY", message)
}
fn io<T>(error: std::io::Error) -> Result<T> {
    fail("E_IO", error.to_string())
}
fn obj(value: &json::Value) -> Result<&Map> {
    if let json::Value::Object(object) = value {
        Ok(object)
    } else {
        argument("expected object")
    }
}
fn arr(value: &json::Value) -> Result<&[json::Value]> {
    if let json::Value::Array(array) = value {
        Ok(array)
    } else {
        argument("expected array")
    }
}
fn string(value: &json::Value) -> Result<&str> {
    if let json::Value::String(s) = value {
        Ok(s)
    } else {
        argument("expected string")
    }
}
fn field<'a>(object: &'a Map, key: &str) -> Result<&'a json::Value> {
    object.get(key).ok_or_else(|| Fault {
        code: "E_ARGUMENT",
        message: format!("missing {key}"),
    })
}
fn text<'a>(object: &'a Map, key: &str) -> Result<&'a str> {
    string(field(object, key)?)
}
fn unsigned(value: &json::Value) -> Result<u32> {
    if let json::Value::Number(number) = value
        && let Some(n) = number.as_u64().and_then(|n| u32::try_from(n).ok())
    {
        return Ok(n);
    }
    argument("expected unsigned 32-bit integer")
}
fn signed(value: &json::Value) -> Result<i32> {
    if let json::Value::Number(number) = value
        && let Some(n) = number.as_i64().and_then(|n| i32::try_from(n).ok())
    {
        return Ok(n);
    }
    argument("expected signed 32-bit integer")
}
fn point(value: &json::Value) -> Result<[i32; 2]> {
    let pair = arr(value)?;
    if pair.len() != 2 {
        return argument("expected coordinate pair");
    }
    Ok([signed(&pair[0])?, signed(&pair[1])?])
}
fn size(value: &json::Value) -> Result<[u32; 2]> {
    let pair = arr(value)?;
    if pair.len() != 2 {
        return argument("expected size pair");
    }
    let result = [unsigned(&pair[0])?, unsigned(&pair[1])?];
    if result.contains(&0) {
        return argument("zero dimension");
    }
    Ok(result)
}
fn rect(value: &json::Value) -> Result<Rect> {
    let a = arr(value)?;
    if a.len() != 4 {
        return argument("expected [x,y,width,height]");
    }
    Rect::new(
        signed(&a[0])?,
        signed(&a[1])?,
        unsigned(&a[2])?,
        unsigned(&a[3])?,
    )
    .map_err(|e| Fault {
        code: e.code(),
        message: e.to_string(),
    })
}
fn name(value: &str) -> Result<String> {
    if value.is_empty()
        || value.len() > 64
        || !value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
    {
        return argument("names must contain 1..64 ASCII letters, digits, _ or -");
    }
    Ok(value.to_owned())
}
fn color(value: &json::Value) -> Result<Pixel> {
    let text = string(value)?;
    let Some(hex) = text.strip_prefix('#') else {
        return capability("palette references are not yet supported");
    };
    if !matches!(hex.len(), 6 | 8) || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return argument("color must be #RRGGBB or #RRGGBBAA");
    }
    let mut rgba = [0, 0, 0, 255];
    for (i, chunk) in hex.as_bytes().chunks(2).enumerate() {
        let pair = std::str::from_utf8(chunk).map_err(|_| Fault {
            code: "E_ARGUMENT",
            message: "invalid hex color".into(),
        })?;
        rgba[i] = u8::from_str_radix(pair, 16).map_err(|_| Fault {
            code: "E_ARGUMENT",
            message: "invalid hex color".into(),
        })?;
    }
    Ok(rgba)
}
fn color_literal(value: &json::Value, rgb_only: bool) -> Result<Pixel> {
    let literal = string(value)?;
    if rgb_only && (literal.len() != 7 || !literal.starts_with('#')) {
        return argument("RGB recolor mappings require #RRGGBB literals");
    }
    color(value)
}
fn read_palette_entries(values: &[json::Value]) -> Result<PaletteEntries> {
    if values.is_empty() || values.len() > 256 {
        return argument("palette entries must contain 1..256 named RGB colors");
    }
    let mut entries = Vec::new();
    entries.try_reserve_exact(values.len()).map_err(|_| Fault {
        code: "E_ALLOC",
        message: "palette entry allocation".into(),
    })?;
    for value in values {
        let item = obj(value)?;
        allowed(item, &["name", "rgb"])?;
        let entry_name = name(text(item, "name")?)?;
        if entries.iter().any(|(other, _)| other == &entry_name) {
            return argument("duplicate palette entry name");
        }
        let rgb = color_literal(field(item, "rgb")?, true)?;
        let rgb = [rgb[0], rgb[1], rgb[2]];
        if entries.iter().any(|(_, existing)| *existing == rgb) {
            return argument("duplicate palette RGB value");
        }
        entries.push((entry_name, rgb));
    }
    Ok(entries)
}
fn read_palettes(values: &[json::Value]) -> Result<Palettes> {
    if values.len() > 256 {
        return fail("E_LIMIT", "too many named palettes");
    }
    let mut palettes = BTreeMap::new();
    for value in values {
        let item = obj(value)?;
        allowed(item, &["name", "entries"])?;
        let palette_name = name(text(item, "name")?)?;
        let entries = read_palette_entries(arr(field(item, "entries")?)?)?;
        if palettes.insert(palette_name, entries).is_some() {
            return argument("duplicate palette name");
        }
    }
    Ok(palettes)
}
fn allowed(map: &Map, names: &[&str]) -> Result<()> {
    for key in map.keys() {
        if !names.contains(&key.as_str()) {
            return capability(format!("unsupported field: {key}"));
        }
    }
    Ok(())
}
fn boolean(map: &Map, key: &str, default: bool) -> Result<bool> {
    match map.get(key) {
        None => Ok(default),
        Some(json::Value::Bool(v)) => Ok(*v),
        _ => argument(format!("{key} must be boolean")),
    }
}
fn optional_point(map: &Map, key: &str, default: [i32; 2]) -> Result<[i32; 2]> {
    map.get(key)
        .map(point)
        .transpose()
        .map(|v| v.unwrap_or(default))
}
fn parse_json(bytes: &[u8], work: &mut Work) -> Result<json::Value> {
    json::parse(bytes, json::Limits::default(), work).map_err(|e| Fault {
        code: e.code(),
        message: e.to_string(),
    })
}
fn raster<T>(result: std::result::Result<T, raipixel::Error>) -> Result<T> {
    result.map_err(|e| Fault {
        code: e.code(),
        message: e.to_string(),
    })
}
fn codec<T>(result: std::result::Result<T, png::Error>) -> Result<T> {
    result.map_err(|e| Fault {
        code: e.code(),
        message: e.to_string(),
    })
}

#[derive(Default)]
struct Document {
    asset: Option<String>,
    layer: Option<String>,
    frame: Option<String>,
    size: Option<[u32; 2]>,
    pivot: [i32; 2],
    palette_ref: Option<String>,
    palettes: Palettes,
    image: Option<Image>,
}
impl Document {
    fn resolve_color(&self, value: &json::Value) -> Result<Pixel> {
        let raw = string(value)?;
        if raw.starts_with('#') {
            return color(value);
        }
        let Some((palette, entry)) = raw.strip_prefix('@').and_then(|s| s.split_once('/')) else {
            return argument("color must be a literal or @palette/entry");
        };
        let rgb = self.palettes.get(palette).and_then(|entries| {
            entries
                .iter()
                .find(|(name, _)| name == entry)
                .map(|(_, rgb)| rgb)
        });
        match rgb {
            Some(&[r, g, b]) => Ok([r, g, b, 255]),
            None => fail("E_REFERENCE", format!("unknown palette color: {raw}")),
        }
    }

    fn palettes_json(&self) -> String {
        use std::fmt::Write as _;
        let mut json = String::from("[");
        for (index, (palette, entries)) in self.palettes.iter().enumerate() {
            if index != 0 {
                json.push(',');
            }
            let _ = write!(json, r#"{{"name":"{palette}","entries":["#);
            for (position, (name, rgb)) in entries.iter().enumerate() {
                if position != 0 {
                    json.push(',');
                }
                let _ = write!(
                    json,
                    r##"{{"name":"{name}","rgb":"#{:02X}{:02X}{:02X}"}}"##,
                    rgb[0], rgb[1], rgb[2]
                );
            }
            json.push_str("]}");
        }
        json.push(']');
        json
    }

    fn cel_name(&self) -> Result<String> {
        match (&self.asset, &self.layer, &self.frame) {
            (Some(a), Some(l), Some(f)) => Ok(format!("{a}/{l}/{f}")),
            _ => argument("asset, layer and frame must be created first"),
        }
    }
    fn frame_name(&self) -> Result<String> {
        match (&self.asset, &self.frame) {
            (Some(a), Some(f)) => Ok(format!("{a}/{f}")),
            _ => argument("asset and frame must exist"),
        }
    }
    fn image_mut(&mut self, target: &str) -> Result<&mut Image> {
        if target != self.cel_name()? {
            return argument("unknown cel target");
        }
        self.image.as_mut().ok_or_else(|| Fault {
            code: "E_ARGUMENT",
            message: "cel does not exist".into(),
        })
    }
    fn apply(&mut self, operation: &Map, base: &Path, work: &mut Work) -> Result<()> {
        match text(operation, "op")? {
            "palette.set" => {
                allowed(operation, &["op", "name", "entries", "replace"])?;
                let palette = name(text(operation, "name")?)?;
                let replace = boolean(operation, "replace", false)?;
                let exists = self.palettes.contains_key(&palette);
                if replace && !exists {
                    return fail("E_REFERENCE", "cannot replace an unknown palette");
                }
                if !replace && exists {
                    return fail("E_EXISTS", "palette already exists");
                }
                if !exists && self.palettes.len() >= 256 {
                    return fail("E_LIMIT", "too many named palettes");
                }
                let entries = read_palette_entries(arr(field(operation, "entries")?)?)?;
                self.palettes.insert(palette, entries);
                Ok(())
            }
            "palette.extract" => {
                allowed(operation, &["op", "source", "name", "max_colors"])?;
                let source = text(operation, "source")?;
                if source != self.cel_name()? {
                    return fail("E_REFERENCE", "unknown palette extraction source");
                }
                let palette = name(text(operation, "name")?)?;
                if self.palettes.contains_key(&palette) {
                    return fail("E_EXISTS", "palette already exists");
                }
                if self.palettes.len() >= 256 {
                    return fail("E_LIMIT", "too many named palettes");
                }
                let max_colors = operation
                    .get("max_colors")
                    .map(unsigned)
                    .transpose()?
                    .unwrap_or(256);
                if !(1..=256).contains(&max_colors) {
                    return argument("max_colors must be 1..256");
                }
                let image = self.image.as_ref().ok_or_else(|| Fault {
                    code: "E_REFERENCE",
                    message: "cel does not exist".into(),
                })?;
                let extracted = raster(image.extract_visible_palette(
                    max_colors as usize,
                    work,
                ))?;
                let mut entries = PaletteEntries::new();
                entries.try_reserve_exact(extracted.len()).map_err(|_| Fault {
                    code: "E_ALLOC",
                    message: "extracted palette allocation".into(),
                })?;
                for (index, color) in extracted.into_iter().enumerate() {
                    entries.push((format!("c{index:03}"), color.rgb));
                }
                self.palettes.insert(palette, entries);
                Ok(())
            }
            "asset.create" => {
                allowed(operation, &["op", "name", "size", "pivot", "palette"])?;
                if self.asset.is_some() {
                    return capability("this first slice supports one asset");
                }
                let extent = size(field(operation, "size")?)?;
                let pivot = optional_point(operation, "pivot", [0, 0])?;
                let selected_palette = match operation.get("palette") {
                    None | Some(json::Value::Null) => None,
                    Some(value) => {
                        let selected = name(string(value)?)?;
                        if !self.palettes.contains_key(&selected) {
                            return fail("E_REFERENCE", "asset refers to unknown palette");
                        }
                        Some(selected)
                    }
                };
                self.asset = Some(name(text(operation, "name")?)?);
                self.size = Some(extent);
                self.pivot = pivot;
                self.palette_ref = selected_palette;
                Ok(())
            }
            "layer.create" => {
                allowed(operation, &["op", "asset", "name"])?;
                if self.asset.as_deref() != Some(text(operation, "asset")?) {
                    return argument("unknown asset");
                }
                if self.layer.is_some() {
                    return capability("this first slice supports one layer");
                }
                self.layer = Some(name(text(operation, "name")?)?);
                Ok(())
            }
            "frame.create" => {
                allowed(operation, &["op", "asset", "name"])?;
                if self.asset.as_deref() != Some(text(operation, "asset")?) {
                    return argument("unknown asset");
                }
                if self.frame.is_some() {
                    return capability("this first slice supports one frame");
                }
                self.frame = Some(name(text(operation, "name")?)?);
                Ok(())
            }
            "cel.create" => {
                allowed(operation, &["op", "target"])?;
                if text(operation, "target")? != self.cel_name()? {
                    return argument("unknown cel target");
                }
                if self.image.is_some() {
                    return argument("cel already exists");
                }
                let extent = self.size.ok_or_else(|| Fault {
                    code: "E_ARGUMENT",
                    message: "missing canvas".into(),
                })?;
                self.image = Some(raster(Image::new(
                    extent[0],
                    extent[1],
                    Limits::default(),
                    work,
                ))?);
                Ok(())
            }
            "png.import" => {
                allowed(operation, &["op", "target", "file"])?;
                if text(operation, "target")? != self.cel_name()? {
                    return argument("unknown cel target");
                }
                if self.image.is_some() {
                    return argument("cel exists; replacement is unsupported");
                }
                let input = read_input(base, text(operation, "file")?, 128 * 1024 * 1024)?;
                let decoded = codec(png::decode(&input, Limits::default(), work))?;
                if Some(decoded.image.size()) != self.size {
                    return argument("imported PNG must equal canvas dimensions in this slice");
                }
                self.image = Some(decoded.image);
                Ok(())
            }
            "draw" => {
                allowed(
                    operation,
                    &[
                        "op", "targets", "shape", "at", "points", "rect", "color", "clip",
                    ],
                )?;
                let target = single_target(operation)?;
                let clip = boolean(operation, "clip", false)?;
                let sample = self.resolve_color(field(operation, "color")?)?;
                let image = self.image_mut(&target)?;
                match text(operation, "shape")? {
                    "pixel" => {
                        if operation.contains_key("points") || operation.contains_key("rect") {
                            return argument("pixel uses at only");
                        }
                        let pos = point(field(operation, "at")?)?;
                        raster(image.fill(
                            raster(Rect::new(pos[0], pos[1], 1, 1))?,
                            sample,
                            clip,
                            work,
                        ))?;
                    }
                    "rect" => {
                        if operation.contains_key("at") || operation.contains_key("points") {
                            return argument("rect uses rect only");
                        }
                        raster(image.fill(rect(field(operation, "rect")?)?, sample, clip, work))?;
                    }
                    "line" => {
                        if operation.contains_key("at") || operation.contains_key("rect") {
                            return argument("line uses points only");
                        }
                        let points = arr(field(operation, "points")?)?;
                        if points.len() != 2 {
                            return argument("line needs two points");
                        }
                        raster(image.line(
                            point(&points[0])?,
                            point(&points[1])?,
                            sample,
                            clip,
                            work,
                        ))?;
                    }
                    other => return capability(format!("unsupported shape: {other}")),
                }
                Ok(())
            }
            "grid" => {
                allowed(
                    operation,
                    &["op", "targets", "at", "rows", "symbols", "skip", "clip"],
                )?;
                let target = single_target(operation)?;
                let at = optional_point(operation, "at", [0, 0])?;
                let clip = boolean(operation, "clip", false)?;
                let rows = arr(field(operation, "rows")?)?
                    .iter()
                    .map(string)
                    .collect::<Result<Vec<_>>>()?;
                let mapping = obj(field(operation, "symbols")?)?;
                let mut symbols = Vec::new();
                for (key, val) in mapping {
                    if key.len() != 1 || !key.is_ascii() {
                        return argument("grid symbols must be one ASCII byte");
                    }
                    symbols.push((key.as_bytes()[0], Some(self.resolve_color(val)?)));
                }
                if let Some(v) = operation.get("skip") {
                    let skip = string(v)?;
                    if skip.len() != 1 || !skip.is_ascii() {
                        return argument("skip must be one ASCII byte");
                    }
                    symbols.push((skip.as_bytes()[0], None));
                }
                raster(
                    self.image_mut(&target)?
                        .draw_grid(at, &rows, &symbols, clip, work),
                )?;
                Ok(())
            }
            "fill" => {
                allowed(operation, &["op", "targets", "at", "color", "region"])?;
                let target = single_target(operation)?;
                let at = point(field(operation, "at")?)?;
                let at = [
                    u32::try_from(at[0]).map_err(|_| Fault {
                        code: "E_BOUNDS",
                        message: "negative fill x".into(),
                    })?,
                    u32::try_from(at[1]).map_err(|_| Fault {
                        code: "E_BOUNDS",
                        message: "negative fill y".into(),
                    })?,
                ];
                let region = operation.get("region").map(rect).transpose()?;
                let pixel = self.resolve_color(field(operation, "color")?)?;
                raster(self.image_mut(&target)?.flood_fill(at, pixel, region, work))?;
                Ok(())
            }
            "stamp" => {
                allowed(
                    operation,
                    &["op", "targets", "source", "at", "rect", "mode", "clip"],
                )?;
                let target = single_target(operation)?;
                if text(operation, "source")? != target {
                    return capability("the current single-cel slice stamps from its own cel only");
                }
                let at = optional_point(operation, "at", [0, 0])?;
                let clip = boolean(operation, "clip", false)?;
                let mode = match operation.get("mode") {
                    None => Blend::Over,
                    Some(value) => match string(value)? {
                        "over" => Blend::Over,
                        "copy" => Blend::Copy,
                        _ => return argument("stamp mode must be copy or over"),
                    },
                };
                let source = if let Some(value) = operation.get("rect") {
                    rect(value)?
                } else {
                    let [width, height] = self.image_mut(&target)?.size();
                    raster(Rect::new(0, 0, width, height))?
                };
                raster(
                    self.image_mut(&target)?
                        .stamp_within(source, at, mode, 255, clip, work),
                )?;
                Ok(())
            }
            "palette.map" => {
                allowed(
                    operation,
                    &["op", "targets", "region", "pairs", "preserve_alpha"],
                )?;
                let target = single_target(operation)?;
                let region = operation.get("region").map(rect).transpose()?;
                let preserve_alpha = boolean(operation, "preserve_alpha", true)?;
                let values = arr(field(operation, "pairs")?)?;
                if values.is_empty() || values.len() > 256 {
                    return argument("recolor needs 1..256 color pairs");
                }
                let mut pairs = Vec::new();
                pairs.try_reserve_exact(values.len()).map_err(|_| Fault {
                    code: "E_ALLOC",
                    message: "color pair allocation".into(),
                })?;
                for value in values {
                    let colors = arr(value)?;
                    if colors.len() != 2 {
                        return argument("each recolor pair requires two colors");
                    }
                    pairs.push((
                        color_literal(&colors[0], preserve_alpha)?,
                        color_literal(&colors[1], preserve_alpha)?,
                    ));
                }
                raster(
                    self.image_mut(&target)?
                        .map_colors(&pairs, preserve_alpha, region, work),
                )?;
                Ok(())
            }
            "alpha" => {
                allowed(
                    operation,
                    &[
                        "op",
                        "targets",
                        "kind",
                        "color",
                        "match",
                        "threshold",
                        "region",
                    ],
                )?;
                let target = single_target(operation)?;
                let region = operation.get("region").map(rect).transpose()?;
                match text(operation, "kind")? {
                    "key" => {
                        if operation.contains_key("threshold") {
                            return argument("alpha key does not accept threshold");
                        }
                        let match_alpha = match operation.get("match") {
                            None => false,
                            Some(v) if string(v)? == "rgb" => false,
                            Some(v) if string(v)? == "rgba" => true,
                            _ => return argument("alpha match must be rgb or rgba"),
                        };
                        let key = self.resolve_color(field(operation, "color")?)?;
                        raster(
                            self.image_mut(&target)?
                                .alpha_key(key, match_alpha, region, work),
                        )?;
                    }
                    "threshold" => {
                        if operation.contains_key("color") || operation.contains_key("match") {
                            return argument("alpha threshold cannot accept key color or match");
                        }
                        let threshold = u8::try_from(unsigned(field(operation, "threshold")?)?)
                            .map_err(|_| Fault {
                                code: "E_ARGUMENT",
                                message: "threshold must be 0..255".into(),
                            })?;
                        raster(
                            self.image_mut(&target)?
                                .alpha_threshold(threshold, region, work),
                        )?;
                    }
                    other => return capability(format!("unsupported alpha operation: {other}")),
                }
                Ok(())
            }
            other => capability(format!("unsupported operation: {other}")),
        }
    }
    fn manifest(&self) -> Result<String> {
        let asset = self.asset.as_deref().ok_or_else(|| Fault {
            code: "E_ARGUMENT",
            message: "missing asset".into(),
        })?;
        let layer = self.layer.as_deref().ok_or_else(|| Fault {
            code: "E_ARGUMENT",
            message: "missing layer".into(),
        })?;
        let frame = self.frame.as_deref().ok_or_else(|| Fault {
            code: "E_ARGUMENT",
            message: "missing frame".into(),
        })?;
        let [w, h] = self.size.ok_or_else(|| Fault {
            code: "E_ARGUMENT",
            message: "missing size".into(),
        })?;
        if self.image.as_ref().is_some_and(|i| i.size() != [w, h]) {
            return argument("mismatched editable cel");
        }
        let palettes = self.palettes_json();
        let asset_palette = match &self.palette_ref {
            Some(value) => format!(r#""{value}""#),
            None => String::from("null"),
        };
        // Names are validated restricted ASCII, so insertion into JSON is safe.
        Ok(format!(
            r#"{{"format":"raipixel-project","version":1,"semantics":"1.0","images":[{{"id":"i000000","file":"images/000000.png","size":[{w},{h}]}}],"palettes":{palettes},"styles":[],"assets":[{{"name":"{asset}","size":[{w},{h}],"pivot":[{px},{py}],"palette":{asset_palette},"style":null,"layers":[{{"name":"{layer}","visible":true,"opacity":255}}],"frames":[{{"name":"{frame}","duration_ms":100,"cels":[{{"layer":"{layer}","image":"i000000","offset":[0,0]}}],"anchors":{{}},"metadata":{{}}}}],"clips":[],"metadata":{{}}}}],"metadata":{{}}}}"#,
            px = self.pivot[0],
            py = self.pivot[1]
        ) + "\n")
    }
}
fn single_target(operation: &Map) -> Result<String> {
    let targets = arr(field(operation, "targets")?)?;
    if targets.len() != 1 {
        return capability("one target per edit is supported");
    }
    Ok(string(&targets[0])?.to_owned())
}
fn read_input(base: &Path, file: &str, limit: u64) -> Result<Vec<u8>> {
    let path = Path::new(file);
    if path.as_os_str().is_empty()
        || path
            .components()
            .any(|c| !matches!(c, Component::Normal(_) | Component::CurDir))
    {
        return argument("source file must be a relative path without parent traversal");
    }
    let root = base.canonicalize().map_err(|e| Fault {
        code: "E_IO",
        message: e.to_string(),
    })?;
    let full = base.join(path).canonicalize().map_err(|e| Fault {
        code: "E_IO",
        message: e.to_string(),
    })?;
    if !full.starts_with(root) {
        return argument("source file escapes request directory");
    }
    let meta = fs::metadata(&full).map_err(|e| Fault {
        code: "E_IO",
        message: e.to_string(),
    })?;
    if !meta.is_file() || meta.len() > limit {
        return fail("E_LIMIT", "invalid or oversized input file");
    }
    let file = fs::File::open(&full).map_err(|e| Fault {
        code: "E_IO",
        message: e.to_string(),
    })?;
    let mut bytes = Vec::new();
    file.take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| Fault {
            code: "E_IO",
            message: e.to_string(),
        })?;
    if bytes.len() as u64 > limit {
        return fail("E_LIMIT", "input grew beyond limit");
    }
    Ok(bytes)
}
fn load_snapshot(dir: &Path, work: &mut Work) -> Result<Document> {
    let manifest = read_input(dir, "project.json", 8 * 1024 * 1024)?;
    let root = parse_json(&manifest, work)?;
    let object = obj(&root)?;
    if text(object, "format")? != "raipixel-project"
        || unsigned(field(object, "version")?)? != 1
        || text(object, "semantics")? != "1.0"
    {
        return capability("unsupported project snapshot version");
    }
    let palettes = read_palettes(arr(field(object, "palettes")?)?)?;
    let assets = arr(field(object, "assets")?)?;
    if assets.len() != 1 {
        return capability("only one asset snapshot is supported");
    }
    let a = obj(&assets[0])?;
    let layers = arr(field(a, "layers")?)?;
    let frames = arr(field(a, "frames")?)?;
    if layers.len() != 1 || frames.len() != 1 {
        return capability("layered or animated snapshots are not supported yet");
    }
    let l = obj(&layers[0])?;
    let f = obj(&frames[0])?;
    let mut document = Document {
        asset: Some(name(text(a, "name")?)?),
        layer: Some(name(text(l, "name")?)?),
        frame: Some(name(text(f, "name")?)?),
        size: Some(size(field(a, "size")?)?),
        pivot: point(field(a, "pivot")?)?,
        palette_ref: match field(a, "palette")? {
            json::Value::Null => None,
            value => Some(name(string(value)?)?),
        },
        palettes,
        image: None,
    };
    if document
        .palette_ref
        .as_ref()
        .is_some_and(|selected| !document.palettes.contains_key(selected))
    {
        return fail("E_REFERENCE", "snapshot refers to an unknown palette");
    }
    // Round-trip only snapshots whose complete semantic structure is supported.
    // This prevents accidentally erasing unknown palettes, metadata or clips.
    let expected = document.manifest()?;
    let baseline = parse_json(expected.as_bytes(), work)?;
    if baseline != root {
        return capability("snapshot contains unsupported or altered fields");
    }
    let bytes = read_input(dir, "images/000000.png", 128 * 1024 * 1024)?;
    let decoded = codec(png::decode(&bytes, Limits::default(), work))?;
    if Some(decoded.image.size()) != document.size {
        return argument("snapshot image dimensions disagree with manifest");
    }
    document.image = Some(decoded.image);
    Ok(document)
}
fn export_image(image: &Image, scale: u32, work: &mut Work) -> Result<Vec<u8>> {
    if !(1..=16).contains(&scale) {
        return argument("scale must be 1..16");
    }
    // A normal-size export needs no second RGBA allocation or copy.
    if scale == 1 {
        let mut png_bytes = Vec::new();
        codec(png::encode_compressed(image, &mut png_bytes, work))?;
        return Ok(png_bytes);
    }
    let [width, height] = image.size();
    let scaled = [width.checked_mul(scale), height.checked_mul(scale)];
    let [Some(w), Some(h)] = scaled else {
        return fail("E_LIMIT", "scaled dimensions overflow");
    };
    // Preflight the per-side limit before materializing any expanded pixels.
    if w > 4096 || h > 4096 {
        return fail("E_LIMIT", "scaled dimensions exceed image side ceiling");
    }
    let count = u64::from(w) * u64::from(h);
    if count > 16_777_216 {
        return fail("E_LIMIT", "scaled image exceeds pixel ceiling");
    }
    let mut pixels = Vec::new();
    pixels
        .try_reserve_exact(count as usize)
        .map_err(|_| Fault {
            code: "E_ALLOC",
            message: "pixel allocation".into(),
        })?;
    for y in 0..h {
        for x in 0..w {
            pixels.push(image.pixel(x / scale, y / scale).ok_or_else(|| Fault {
                code: "E_INTERNAL",
                message: "pixel index".into(),
            })?);
        }
    }
    let scaled_image = raster(Image::from_pixels(w, h, &pixels, Limits::default(), work))?;
    let mut png_bytes = Vec::new();
    codec(png::encode_compressed(&scaled_image, &mut png_bytes, work))?;
    Ok(png_bytes)
}
fn build_outputs(
    document: &Document,
    job: &Map,
    work: &mut Work,
) -> Result<Vec<(String, Vec<u8>)>> {
    let mut outputs = Vec::new();
    if let Some(exports) = job.get("exports") {
        for spec in arr(exports)? {
            let m = obj(spec)?;
            allowed(m, &["name", "kind", "targets", "scale"])?;
            if text(m, "kind")? != "frame" {
                return capability("only frame export is supported");
            }
            let n = name(text(m, "name")?)?;
            if outputs.iter().any(|(other, _)| other == &n) {
                return argument("duplicate export name");
            }
            let targets = arr(field(m, "targets")?)?;
            if targets.len() != 1 || string(&targets[0])? != document.frame_name()? {
                return argument("unknown frame export target");
            }
            let scale = m.get("scale").map(unsigned).transpose()?.unwrap_or(1);
            let bytes = export_image(
                document.image.as_ref().ok_or_else(|| Fault {
                    code: "E_ARGUMENT",
                    message: "missing cel".into(),
                })?,
                scale,
                work,
            )?;
            outputs.push((n, bytes));
        }
    }
    Ok(outputs)
}
/// Validate only visible pixels against the asset palette. Hidden RGB is
/// deliberately unconstrained, so transparent source fidelity is retained.
fn visible_palette_violation(
    document: &Document,
    work: &mut Work,
) -> Result<Option<raipixel::PaletteMismatch>> {
    let Some(selected) = document.palette_ref.as_ref() else {
        return Ok(None);
    };
    let entries = document.palettes.get(selected).ok_or_else(|| Fault {
        code: "E_REFERENCE",
        message: "asset refers to a missing palette".into(),
    })?;
    let mut colors = [[0u8; 3]; 256];
    for (slot, (_, color)) in colors.iter_mut().zip(entries) {
        *slot = *color;
    }
    let image = document.image.as_ref().ok_or_else(|| Fault {
        code: "E_ARGUMENT",
        message: "missing editable cel".into(),
    })?;
    raster(image.first_visible_color_outside_palette(&colors[..entries.len()], work))
}

/// Export and explicitly requested palette checks must not publish invalid
/// visible colors. Fully transparent RGB remains unconstrained.
fn enforce_visible_palette(document: &Document, work: &mut Work) -> Result<()> {
    if let Some((at, rgb)) = visible_palette_violation(document, work)? {
        return fail(
            "E_PROJECT",
            format!(
                "visible pixel at [{},{}] has RGB #{:02X}{:02X}{:02X} outside asset palette",
                at[0], at[1], rgb[0], rgb[1], rgb[2]
            ),
        );
    }
    Ok(())
}

fn run_job(
    job_file: &Path,
    output_dir: &Path,
    project: Option<&Path>,
    dry_run: bool,
) -> Result<()> {
    if output_dir.exists() {
        return argument("output directory already exists");
    }
    let mut work = Work::default();
    let job_base = job_file
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let bytes = read_input(
        job_base,
        job_file
            .file_name()
            .and_then(|s| s.to_str())
            .ok_or_else(|| Fault {
                code: "E_ARGUMENT",
                message: "invalid job filename".into(),
            })?,
        8 * 1024 * 1024,
    )?;
    let input = parse_json(&bytes, &mut work)?;
    let job = obj(&input)?;
    allowed(job, &["interface", "semantics", "ops", "checks", "exports"])?;
    if unsigned(field(job, "interface")?)? != 1 || text(job, "semantics")? != "1.0" {
        return capability("unsupported job version");
    }
    let mut require_palette = false;
    if let Some(checks) = job.get("checks") {
        for check in arr(checks)? {
            match string(check)? {
                "structure" | "bounds" | "dimensions" | "alpha" => {}
                "palette" => require_palette = true,
                _ => {
                    return capability(
                        "this Foundation slice supports structure, dimensions, bounds, alpha and palette checks",
                    );
                }
            }
        }
    }
    let mut document = if let Some(path) = project {
        load_snapshot(path, &mut work)?
    } else {
        Document::default()
    };
    for op in arr(field(job, "ops")?)? {
        document.apply(obj(op)?, job_base, &mut work)?;
    }
    if document.image.is_none() {
        return argument("missing editable cel");
    }
    // Snapshot-only jobs may keep deliberately incomplete artwork with
    // checks=["structure"]. A game PNG export must never silently violate
    // the asset's declared visible-color palette.
    let exporting = job
        .get("exports")
        .is_some_and(|value| matches!(value, json::Value::Array(exports) if !exports.is_empty()));
    if require_palette || exporting {
        enforce_visible_palette(&document, &mut work)?;
    }
    let manifest = document.manifest()?;
    let exports = build_outputs(&document, job, &mut work)?;
    let snapshot = export_image(
        document.image.as_ref().ok_or_else(|| Fault {
            code: "E_ARGUMENT",
            message: "missing cel".into(),
        })?,
        1,
        &mut work,
    )?;
    if dry_run {
        println!(
            r#"{{"schema":1,"ok":true,"action":"run","result":{{"dry_run":true,"exports":{},"snapshot_png_bytes":{}}}}}"#,
            exports.len(),
            snapshot.len()
        );
        return Ok(());
    }
    let parent = output_dir
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let stem = output_dir
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or_else(|| Fault {
            code: "E_ARGUMENT",
            message: "invalid destination".into(),
        })?;
    let mut stage = None;
    for i in 0..16 {
        let candidate = parent.join(format!(".{stem}.raipixel-stage-{}-{i}", std::process::id()));
        match fs::create_dir(&candidate) {
            Ok(()) => {
                stage = Some(candidate);
                break;
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return io(e),
        }
    }
    let stage = stage.ok_or_else(|| Fault {
        code: "E_IO",
        message: "unable to allocate staging directory".into(),
    })?;
    let result = (|| {
        fs::create_dir(stage.join("images")).map_err(|e| Fault {
            code: "E_IO",
            message: e.to_string(),
        })?;
        fs::write(stage.join("images/000000.png"), snapshot).map_err(|e| Fault {
            code: "E_IO",
            message: e.to_string(),
        })?;
        fs::write(stage.join("project.json"), manifest).map_err(|e| Fault {
            code: "E_IO",
            message: e.to_string(),
        })?;
        for (n, bytes) in &exports {
            let artifact = stage.join("artifacts").join(n);
            fs::create_dir_all(&artifact).map_err(|e| Fault {
                code: "E_IO",
                message: e.to_string(),
            })?;
            fs::write(artifact.join("image.png"), bytes).map_err(|e| Fault {
                code: "E_IO",
                message: e.to_string(),
            })?;
        }
        // A separate same-parent staging directory keeps ordinary encode/IO
        // failures unpublished. Cross-process destination races are NOT yet
        // a certified no-clobber guarantee for std::fs::rename.
        if output_dir.exists() {
            return argument("output directory already exists");
        }
        fs::rename(&stage, output_dir).map_err(|e| Fault {
            code: "E_IO",
            message: e.to_string(),
        })?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_dir_all(&stage);
    }
    result?;
    println!(
        r#"{{"schema":1,"ok":true,"action":"run","result":{{"exports":{},"snapshot":true}}}}"#,
        exports.len()
    );
    Ok(())
}
/// Read-only single-frame query. The image-statistics implementation is shared
/// with callers of the library, so CLI and programmatic results use the same
/// pixel traversal and deterministic histogram ordering.
fn inspect_snapshot(project: &Path, query_file: Option<&Path>) -> Result<()> {
    use std::fmt::Write as _;

    let mut work = Work::default();
    let document = load_snapshot(project, &mut work)?;
    let parsed = if let Some(path) = query_file {
        let base = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        let filename = path
            .file_name()
            .and_then(|s| s.to_str())
            .ok_or_else(|| Fault {
                code: "E_ARGUMENT",
                message: "invalid query filename".into(),
            })?;
        let bytes = read_input(base, filename, 8 * 1024 * 1024)?;
        parse_json(&bytes, &mut work)?
    } else {
        json::Value::Object(Map::new())
    };
    let query = obj(&parsed)?;
    allowed(
        query,
        &["target", "region", "pixels", "symbols", "histogram_limit"],
    )?;
    let want_symbols = boolean(query, "symbols", false)?;
    let target = query.get("target").map(string).transpose()?;
    if target.is_none() {
        if want_symbols
            || query.contains_key("region")
            || query.contains_key("pixels")
            || query.contains_key("histogram_limit")
        {
            return argument("inspection region and pixel options require a target");
        }
        let [w, h] = document.size.ok_or_else(|| Fault {
            code: "E_ARGUMENT",
            message: "missing canvas".into(),
        })?;
        let asset = document.asset.as_deref().ok_or_else(|| Fault {
            code: "E_ARGUMENT",
            message: "missing asset".into(),
        })?;
        let layer = document.layer.as_deref().ok_or_else(|| Fault {
            code: "E_ARGUMENT",
            message: "missing layer".into(),
        })?;
        let frame = document.frame.as_deref().ok_or_else(|| Fault {
            code: "E_ARGUMENT",
            message: "missing frame".into(),
        })?;
        let mut palettes = String::new();
        for (index, name) in document.palettes.keys().enumerate() {
            if index != 0 {
                palettes.push(',');
            }
            let _ = write!(palettes, r#""{name}""#);
        }
        println!(
            r#"{{"schema":1,"ok":true,"action":"inspect","result":{{"assets":["{asset}"],"layers":["{layer}"],"frames":["{frame}"],"palettes":[{palettes}],"size":[{w},{h}],"pivot":[{},{}]}}}}"#,
            document.pivot[0], document.pivot[1]
        );
        return Ok(());
    }

    let target = target.unwrap();
    if target != document.cel_name()? && target != document.frame_name()? {
        return fail("E_REFERENCE", "unknown cel or frame inspection target");
    }
    let region = query.get("region").map(rect).transpose()?;
    let raw = boolean(query, "pixels", false)?;
    let histogram_limit = query
        .get("histogram_limit")
        .map(unsigned)
        .transpose()?
        .unwrap_or(16) as usize;
    let image = document.image.as_ref().ok_or_else(|| Fault {
        code: "E_ARGUMENT",
        message: "missing cel".into(),
    })?;
    let stats = raster(image.inspect(region, histogram_limit, raw || want_symbols, &mut work))?;

    let mut histogram = String::new();
    for (index, entry) in stats.histogram.iter().enumerate() {
        if index != 0 {
            histogram.push(',');
        }
        let [r, g, b, a] = entry.rgba;
        let _ = write!(
            histogram,
            r#"{{"rgba":[{r},{g},{b},{a}],"count":{}}}"#,
            entry.count
        );
    }
    let bbox = match stats.visible_bbox {
        Some([x, y, w, h]) => format!("[{x},{y},{w},{h}]"),
        None => String::from("null"),
    };
    let [x, y, w, h] = stats.region;
    let [image_width, image_height] = stats.size;
    let mut pixels = String::new();
    if raw && let Some(raw_pixels) = stats.pixels.as_ref() {
        pixels.push_str(r#","pixels":["#);
        for (index, &[r, g, b, a]) in raw_pixels.iter().enumerate() {
            if index != 0 {
                pixels.push(',');
            }
            let _ = write!(pixels, "[{r},{g},{b},{a}]");
        }
        pixels.push(']');
    }

    let mut symbols = String::new();
    if want_symbols {
        let raw_pixels = stats.pixels.as_ref().ok_or_else(|| Fault {
            code: "E_INTERNAL",
            message: "symbol query missing bounded pixels".into(),
        })?;
        let mut colors = Vec::new();
        colors
            .try_reserve_exact(raw_pixels.len())
            .map_err(|_| Fault {
                code: "E_ALLOC",
                message: "symbol palette allocation".into(),
            })?;
        colors.extend_from_slice(raw_pixels);
        colors.sort_unstable();
        colors.dedup();
        if colors.len() > 94 {
            return fail("E_LIMIT", "symbol query exceeds 94 distinct RGBA colors");
        }
        symbols.push_str(r#","symbols":{"rows":["#);
        for (row, pixels) in raw_pixels.chunks(w as usize).enumerate() {
            if row != 0 {
                symbols.push(',');
            }
            symbols.push('"');
            for pixel in pixels {
                let index = colors.binary_search(pixel).map_err(|_| Fault {
                    code: "E_INTERNAL",
                    message: "missing symbol color".into(),
                })?;
                match (33 + index as u8) as char {
                    '"' => symbols.push_str("\\\""),
                    '\\' => symbols.push_str("\\\\"),
                    symbol => symbols.push(symbol),
                }
            }
            symbols.push('"');
        }
        symbols.push_str(r#"],"legend":["#);
        for (index, &[r, g, b, a]) in colors.iter().enumerate() {
            if index != 0 {
                symbols.push(',');
            }
            let symbol = match (33 + index as u8) as char {
                '"' => String::from("\\\""),
                '\\' => String::from("\\\\"),
                symbol => symbol.to_string(),
            };
            let _ = write!(
                symbols,
                r#"{{"symbol":"{symbol}","rgba":[{r},{g},{b},{a}]}}"#
            );
        }
        symbols.push_str("]}");
    }

    println!(
        r#"{{"schema":1,"ok":true,"action":"inspect","result":{{"target":"{target}","size":[{image_width},{image_height}],"region":[{x},{y},{w},{h}],"pixel_count":{},"zero_alpha":{},"partial_alpha":{},"opaque_alpha":{},"visible_bbox":{bbox},"unique_colors":{},"histogram_truncated":{},"histogram":[{histogram}]{pixels}{symbols}}}}}"#,
        stats.pixel_count,
        stats.zero_alpha,
        stats.partial_alpha,
        stats.opaque_alpha,
        stats.unique_colors,
        stats.histogram_truncated
    );
    Ok(())
}

/// Validate the supported single-cel snapshot without editing or publishing it.
/// Every reported check has an explicit pass/fail result for AI callers.
fn validate_snapshot(project: &Path, query_file: Option<&Path>) -> Result<()> {
    use std::fmt::Write as _;

    let mut work = Work::default();
    let document = load_snapshot(project, &mut work)?;
    let parsed = if let Some(path) = query_file {
        let base = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        let filename = path
            .file_name()
            .and_then(|s| s.to_str())
            .ok_or_else(|| Fault {
                code: "E_ARGUMENT",
                message: "invalid validation query filename".into(),
            })?;
        parse_json(&read_input(base, filename, 8 * 1024 * 1024)?, &mut work)?
    } else {
        json::Value::Object(Map::new())
    };
    let query = obj(&parsed)?;
    allowed(query, &["asset", "checks"])?;
    if let Some(selected) = query.get("asset")
        && string(selected)? != document.asset.as_deref().unwrap_or("")
    {
        return fail("E_REFERENCE", "unknown validation asset");
    }
    let names = [
        "structure",
        "dimensions",
        "palette",
        "alpha",
        "bounds",
        "metadata",
    ];
    let requested: Vec<&str> = match query.get("checks") {
        Some(value) => arr(value)?.iter().map(string).collect::<Result<Vec<_>>>()?,
        None => names.to_vec(),
    };
    if requested.is_empty() {
        return argument("at least one validation check is required");
    }
    let mut seen = Vec::new();
    for check in &requested {
        if !names.contains(check) {
            return capability(format!("unsupported validation check: {check}"));
        }
        if seen.contains(check) {
            return argument("duplicate validation check");
        }
        seen.push(*check);
    }
    let [width, height] = document.size.ok_or_else(|| Fault {
        code: "E_PROJECT",
        message: "missing canvas".into(),
    })?;
    let image = document.image.as_ref().ok_or_else(|| Fault {
        code: "E_PROJECT",
        message: "missing cel".into(),
    })?;
    let palette_invalid = if requested.contains(&"palette") {
        visible_palette_violation(&document, &mut work)?.map(|(at, _)| at)
    } else {
        None
    };
    let dimensions_valid = image.size() == [width, height];
    let mut checks_json = String::from("[");
    for (index, check) in requested.iter().enumerate() {
        if index != 0 {
            checks_json.push(',');
        }
        let passed = match *check {
            "structure" | "alpha" | "metadata" => true,
            "bounds" | "dimensions" => dimensions_valid,
            "palette" => palette_invalid.is_none(),
            _ => unreachable!(),
        };
        let _ = write!(checks_json, r#"{{"name":"{check}","passed":{passed}}}"#);
    }
    checks_json.push(']');
    let failed = palette_invalid.is_some() || !dimensions_valid;
    let mut diagnostics = String::from("[");
    if let Some([x, y]) = palette_invalid {
        let _ = write!(
            diagnostics,
            r#"{{"severity":"error","code":"E_STYLE","check":"palette","message":"Visible RGB not present in the assigned palette","at":[{x},{y}]}}"#
        );
    }
    if !dimensions_valid {
        if diagnostics.len() > 1 {
            diagnostics.push(',');
        }
        diagnostics.push_str(r#"{"severity":"error","code":"E_BOUNDS","check":"dimensions","message":"Canvas and cel dimensions disagree"}"#);
    }
    diagnostics.push(']');
    let error_count = usize::from(palette_invalid.is_some()) + usize::from(!dimensions_valid);
    let ok = !failed;
    println!(
        r#"{{"schema":1,"ok":{ok},"action":"validate","result":{{"passed":{ok},"checks":{checks_json},"error_count":{error_count},"warning_count":0}},"diagnostics":{diagnostics}}}"#
    );
    if failed {
        std::process::exit(3);
    }
    Ok(())
}

fn execute() -> Result<()> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() == 1 && args[0] == "version" {
        println!("raipixel 0.1.0 (Foundation single-cel preview)");
        return Ok(());
    }
    if args.len() == 1 && args[0] == "help" {
        println!(
            "raipixel run JOB.json --out DIRECTORY [--project SNAPSHOT] [--dry-run]\nraipixel inspect SNAPSHOT [--query QUERY.json]\nraipixel validate SNAPSHOT [--query QUERY.json]\nraipixel version"
        );
        return Ok(());
    }
    if args.len() >= 2 && args[0] == "inspect" {
        let snapshot = PathBuf::from(&args[1]);
        if args.len() == 2 {
            return inspect_snapshot(&snapshot, None);
        }
        if args.len() == 4 && args[2] == "--query" {
            return inspect_snapshot(&snapshot, Some(Path::new(&args[3])));
        }
        return argument("inspect usage: inspect SNAPSHOT [--query QUERY.json]");
    }
    if args.len() >= 2 && args[0] == "validate" {
        let snapshot = PathBuf::from(&args[1]);
        if args.len() == 2 {
            return validate_snapshot(&snapshot, None);
        }
        if args.len() == 4 && args[2] == "--query" {
            return validate_snapshot(&snapshot, Some(Path::new(&args[3])));
        }
        return argument("validate usage: validate SNAPSHOT [--query QUERY.json]");
    }
    if args.len() < 4 || args[0] != "run" {
        return capability("supported commands: help, version, run, inspect, validate");
    }
    let job = PathBuf::from(&args[1]);
    let mut output = None;
    let mut project = None;
    let mut dry_run = false;
    let mut i = 2;
    while i < args.len() {
        if args[i] == "--dry-run" {
            dry_run = true;
            i += 1;
            continue;
        }
        if args[i] == "--out" && i + 1 < args.len() {
            if output.replace(PathBuf::from(&args[i + 1])).is_some() {
                return argument("duplicate --out");
            }
            i += 2;
            continue;
        }
        if args[i] == "--project" && i + 1 < args.len() {
            if project.replace(PathBuf::from(&args[i + 1])).is_some() {
                return argument("duplicate --project");
            }
            i += 2;
            continue;
        }
        return argument("unknown or incomplete command-line argument");
    }
    run_job(
        &job,
        output.as_deref().ok_or_else(|| Fault {
            code: "E_ARGUMENT",
            message: "missing --out".into(),
        })?,
        project.as_deref(),
        dry_run,
    )
}
fn main() {
    if let Err(error) = execute() {
        let safe = error
            .message
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('\n', " ");
        eprintln!(
            r#"{{"schema":1,"ok":false,"error":{{"code":"{}","message":"{}"}}}}"#,
            error.code, safe
        );
        std::process::exit(1);
    }
}
