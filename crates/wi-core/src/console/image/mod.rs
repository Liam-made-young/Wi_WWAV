mod assist;
mod edit;
mod model;
mod render;

use super::storage::{self, Actor, Asset, Document, Library, Result, Tool, Version};
use crate::{Core, CoreError};
use base64::{engine::general_purpose::STANDARD, Engine};
pub use edit::Edit;
pub use model::Record;
pub use model::MAX_IMAGE_BYTES;
use model::*;
use serde_json::{json, Value};

pub fn seed(l: &Library, id: &str, asset: &Asset) -> Result<Option<Record>> {
    let mut m = Record::blank();
    match asset.mime.as_str() {
        "application/json" => {}
        "image/png" | "image/jpeg" | "image/webp" => {
            let image = render::decode(&l.bytes(id, asset)?)?;
            m.width = image.width();
            m.height = image.height();
            m.layers[0].name = asset.name.clone();
            m.layers[0].content = Content::Pixel {
                asset: Some(asset.clone()),
            };
        }
        "image/svg+xml" => {
            let bytes = l.bytes(id, asset)?;
            let svg =
                std::str::from_utf8(&bytes).map_err(|_| storage::refused("SVG needs UTF-8."))?;
            let (_, w, h) = render::canonical(svg)?;
            m.width = w;
            m.height = h;
            m.layers = vec![Layer::new(
                &asset.name,
                Content::Vector {
                    objects: vec![],
                    source: Some(asset.clone()),
                    asset: None,
                },
            )];
        }
        _ => return Ok(None),
    }
    validate(&m)?;
    Ok(Some(m))
}
pub fn load(l: &Library, d: &Document, v: &Version) -> Result<Record> {
    if d.tool != Tool::Image {
        return Err(storage::refused(
            "This document belongs to another Console tool.",
        ));
    }
    let m = if let Some(m) = &v.image {
        m.clone()
    } else {
        seed(l, &d.id, &v.asset)?.ok_or_else(|| {
            storage::refused(
                "This image format is read-only. Import PNG, JPEG, WebP or SVG to edit it.",
            )
        })?
    };
    validate(&m)?;
    Ok(m)
}
pub fn read_preview(l: &Library, id: &str, v: &Version) -> Result<Value> {
    let m = v
        .image
        .as_ref()
        .ok_or_else(|| storage::refused("Missing image record."))?;
    render::preview(l, id, m, render::full(m), 1536)
}
pub fn store(l: &Library, source_id: &str, target_id: &str, m: &Record) -> Result<(Record, Asset)> {
    validate(m)?;
    let mut stored = m.clone();
    let copy = |a: &Asset| -> Result<Asset> {
        let bytes = l.bytes(source_id, a)?;
        if source_id == target_id {
            Ok(a.clone())
        } else {
            l.asset(target_id, &a.name, &bytes)
        }
    };
    for layer in &mut stored.layers {
        if let Some(a) = &layer.mask {
            layer.mask = Some(copy(a)?);
        }
        match &mut layer.content {
            Content::Pixel { asset } => {
                *asset = Some(if let Some(a) = asset {
                    if a.mime == "image/png" {
                        copy(a)?
                    } else {
                        l.asset(
                            target_id,
                            &format!("{}.png", layer.id),
                            &render::png(&render::decode(&l.bytes(source_id, a)?)?)?,
                        )?
                    }
                } else {
                    l.asset(
                        target_id,
                        &format!("{}.png", layer.id),
                        &render::png(&::image::RgbaImage::new(m.width, m.height))?,
                    )?
                });
            }
            Content::Vector {
                objects,
                source,
                asset,
            } => {
                let text = if let Some(a) = source {
                    let bytes = l.bytes(source_id, a)?;
                    let s = String::from_utf8(bytes)
                        .map_err(|_| storage::refused("SVG source is not UTF-8."))?;
                    let (canonical, _, _) = render::canonical(&s)?;
                    let a = l.asset(target_id, &a.name, canonical.as_bytes())?;
                    *source = Some(a);
                    Some(canonical)
                } else {
                    None
                };
                let svg = render::vector_svg(m, objects, text.as_deref());
                *asset = Some(l.asset(target_id, &format!("{}.svg", layer.id), svg.as_bytes())?);
            }
            Content::Group => {}
        }
    }
    let image = render::render(
        l,
        target_id,
        &stored,
        render::full(m),
        m.width.max(m.height),
    )?;
    let asset = l.asset(
        target_id,
        "canvas.png",
        &image
            .encode_png()
            .map_err(|e| storage::refused(e.to_string()))?,
    )?;
    Ok((stored, asset))
}
pub fn changes(old: &Record, new: &Record) -> Vec<String> {
    let mut out = Vec::new();
    if (old.width, old.height) != (new.width, new.height) {
        out.push(format!(
            "Canvas resized: {} x {} -> {} x {}",
            old.width, old.height, new.width, new.height
        ));
    }
    if old
        .layers
        .iter()
        .map(|l| (&l.id, &l.parent))
        .collect::<Vec<_>>()
        != new
            .layers
            .iter()
            .map(|l| (&l.id, &l.parent))
            .collect::<Vec<_>>()
    {
        out.push("Layers reordered, grouped, added or removed".into());
    }
    for l in &new.layers {
        if let Some(previous) = old.layers.iter().find(|s| s.id == l.id) {
            if serde_json::to_value(l).ok() != serde_json::to_value(previous).ok() {
                out.push(format!("Layer edited: {}", l.name));
            }
        } else {
            out.push(format!("Layer added: {}", l.name));
        }
    }
    if old.background != new.background {
        out.push("Canvas background changed".into());
    }
    if old.palette != new.palette {
        out.push("Saved palette changed".into());
    }
    out
}
pub fn selected(l: &Library, id: &str) -> Result<Option<Asset>> {
    let w = super::workspace(l)?;
    let s = w.tools.get("image").map(|t| &t.selection);
    if let Some(s) = s.filter(|s| s["document"] == id) {
        if !s["mask"].is_null() {
            return serde_json::from_value(s["mask"].clone())
                .map(Some)
                .map_err(|_| storage::refused("Invalid selection mask."));
        }
    }
    Ok(None)
}
pub fn apply(
    l: &Library,
    id: &str,
    m: &mut Record,
    action: Edit,
    actor: Actor,
) -> Result<Option<String>> {
    let mask = selected(l, id)?;
    edit::apply(l, id, m, action, actor, mask.as_ref())
}

pub fn invoke(_core: &Core, l: &Library, cmd: &str, a: &Value, actor: Actor) -> Result<Value> {
    let id = super::string(a, "id")?;
    let d = l.load(id)?;
    let v = if let Some(version) = super::opt_string(a, "version") {
        d.versions
            .iter()
            .find(|v| v.id == version)
            .ok_or_else(|| storage::refused("That image version is missing."))?
    } else {
        d.current()?
    };
    let mut m = load(l, &d, v)?;
    match cmd {
        "console.image.read" => {
            let w = super::workspace(l)?;
            let t = w.tools.get("image");
            let view = t
                .and_then(|t| t.views.get(id))
                .cloned()
                .unwrap_or_else(|| json!({}));
            Ok(
                json!({"document":d.summary()?,"base":v.id,"image":m,"view":view,"selection":t.filter(|t|t.selection["document"]==id).map(|t|&t.selection),"preview":render::preview(l,id,&m,render::full(&m),1536)?}),
            )
        }
        "console.image.preview" => {
            if let (Some(layer), Some(values)) =
                (super::opt_string(a, "layer"), a.get("adjustments"))
            {
                let values: Adjustments = serde_json::from_value(values.clone())
                    .map_err(|e| storage::refused(e.to_string()))?;
                model::adjustments(&values)?;
                m.layer_mut(layer)?.adjustments = values;
            }
            let region = if let Some(region) = a.get("region") {
                serde_json::from_value(region.clone())
                    .map_err(|_| storage::refused("Invalid viewport region."))?
            } else {
                render::full(&m)
            };
            render::preview(
                l,
                id,
                &m,
                region,
                a["maxEdge"].as_u64().unwrap_or(1536).min(2048) as u32,
            )
        }
        "console.image.save" => super::reply(&l.save_image(
            id,
            super::string(a, "base")?,
            super::opt_string(a, "title"),
            &m,
            actor,
            "save image",
        )?),
        "console.image.edit" => {
            d.check_base(super::string(a, "base")?)?;
            let action: Edit = serde_json::from_value(a["action"].clone())
                .map_err(|e| storage::refused(format!("Invalid Image action: {e}")))?;
            let label = format!("image {}", a["action"]["type"].as_str().unwrap_or("edit"));
            let layer = apply(l, id, &mut m, action, actor)?;
            let saved = l.save_image(id, super::string(a, "base")?, None, &m, actor, &label)?;
            Ok(json!({"document":saved.summary()?,"layerId":layer}))
        }
        "console.image.selection" => {
            let mut w = super::workspace(l)?;
            let t = w.tools.entry("image".into()).or_default();
            if a["shape"].is_null() {
                t.selection = json!({"document":id,"layer":a["layer"]});
            } else {
                let shape: Selection = serde_json::from_value(a["shape"].clone())
                    .map_err(|_| storage::refused("Invalid selection."))?;
                let mask = edit::selection(l, id, &m, &shape)?;
                let asset = l.asset(id, "selection.png", &render::png(&mask)?)?;
                t.selection = json!({"document":id,"layer":a["layer"],"shape":shape,"mask":asset});
            }
            let selection = t.selection.clone();
            super::write_workspace(l, &w)?;
            Ok(json!({"workspace":w,"selection":selection}))
        }
        "console.image.sample" => {
            let x = a["x"]
                .as_u64()
                .ok_or_else(|| storage::refused("Choose a pixel."))?;
            let y = a["y"]
                .as_u64()
                .ok_or_else(|| storage::refused("Choose a pixel."))?;
            if x >= m.width as u64 || y >= m.height as u64 {
                return Err(storage::refused("Choose a point inside the canvas."));
            }
            let p = render::render(
                l,
                id,
                &m,
                Rect {
                    x: x as f32,
                    y: y as f32,
                    width: 1.,
                    height: 1.,
                },
                1,
            )?;
            let c = p.pixels()[0].demultiply();
            Ok(
                json!({"color":format!("#{:02x}{:02x}{:02x}{:02x}",c.red(),c.green(),c.blue(),c.alpha())}),
            )
        }
        "console.image.view" => {
            let view = a["view"]
                .as_object()
                .ok_or_else(|| storage::refused("Invalid Image view."))?;
            if view.iter().any(|(k, v)| !valid_view(&m, k, v)) {
                return Err(storage::refused("Unknown or invalid Image view setting."));
            }
            let mut w = super::workspace(l)?;
            let t = w.tools.entry("image".into()).or_default();
            let prefs = t.views.entry(id.into()).or_insert_with(|| json!({}));
            for (k, v) in view {
                prefs[k] = v.clone();
            }
            super::write_workspace(l, &w)?;
            Ok(json!({"workspace":w}))
        }
        "console.image.export" => export(l, &d, v, &m, a),
        _ => Err(CoreError::new(
            "unknown_command",
            format!("No Image command called {cmd}."),
        )),
    }
}
fn export(l: &Library, d: &Document, v: &Version, m: &Record, a: &Value) -> Result<Value> {
    let format = super::string(a, "format")?;
    let pixels = render::render(l, &d.id, m, render::full(m), m.width.max(m.height))?;
    let (extension, mime, bytes) = match format {
        "png" => (
            "png",
            "image/png",
            pixels
                .encode_png()
                .map_err(|e| storage::refused(e.to_string()))?,
        ),
        "jpg" => {
            let quality = a["quality"].as_u64().unwrap_or(90);
            if !(1..=100).contains(&quality) {
                return Err(storage::refused("JPEG quality needs 1 to 100."));
            }
            let rgba = render::rgba(&pixels);
            let rgb = ::image::RgbImage::from_fn(rgba.width(), rgba.height(), |x, y| {
                let p = rgba.get_pixel(x, y);
                let alpha = p[3] as u16;
                ::image::Rgb([
                    ((p[0] as u16 * alpha + 255 * (255 - alpha)) / 255) as u8,
                    ((p[1] as u16 * alpha + 255 * (255 - alpha)) / 255) as u8,
                    ((p[2] as u16 * alpha + 255 * (255 - alpha)) / 255) as u8,
                ])
            });
            let mut bytes = Vec::new();
            ::image::codecs::jpeg::JpegEncoder::new_with_quality(&mut bytes, quality as u8)
                .encode_image(&rgb)
                .map_err(|e| storage::refused(e.to_string()))?;
            ("jpg", "image/jpeg", bytes)
        }
        "svg" => (
            "svg",
            "image/svg+xml",
            svg_export(l, &d.id, m)?.into_bytes(),
        ),
        "pdf" => ("pdf", "application/pdf", pdf(&pixels)?),
        _ => return Err(storage::refused("Choose PNG, JPG, SVG or PDF.")),
    };
    let path = l
        .root
        .join("exports")
        .join(format!("{}-{}.{}", d.id, v.id, extension));
    storage::atomic_bytes(&path, &bytes)?;
    Ok(
        json!({"path":path,"name":format!("{}.{}",v.title.chars().map(|c|if c.is_alphanumeric()||" -_".contains(c){c}else{'_'}).collect::<String>(),extension),"mime":mime,"base64":STANDARD.encode(bytes),"version":v.id}),
    )
}
fn svg_export(l: &Library, id: &str, m: &Record) -> Result<String> {
    // Preserve real vector layers. Pixel layers and non-SVG blend/adjustment
    // effects are embedded as lossless PNG, never discarded from an SVG export.
    let mut out=format!("<svg xmlns=\"http://www.w3.org/2000/svg\" xmlns:xlink=\"http://www.w3.org/1999/xlink\" width=\"{}\" height=\"{}\" viewBox=\"0 0 {} {}\">",m.width,m.height,m.width,m.height);
    if m.layers.iter().any(|l| l.visible && l.blend != "normal") {
        let p = render::render(l, id, m, render::full(m), m.width.max(m.height))?;
        out.push_str(&format!(
            "<image width=\"{}\" height=\"{}\" xlink:href=\"data:image/png;base64,{}\"/></svg>",
            m.width,
            m.height,
            STANDARD.encode(
                p.encode_png()
                    .map_err(|e| storage::refused(e.to_string()))?
            )
        ));
        return Ok(out);
    }
    if let Some(c) = &m.background {
        out.push_str(&format!(
            "<rect width=\"100%\" height=\"100%\" fill=\"{c}\"/>"
        ));
    }
    for layer in m
        .layers
        .iter()
        .rev()
        .filter(|l| l.parent.is_none() && l.visible)
    {
        let mut one = m.clone();
        one.background = None;
        let mut keep = std::collections::BTreeSet::from([layer.id.as_str()]);
        for l in &m.layers {
            if l.parent.as_deref().is_some_and(|p| keep.contains(p)) {
                keep.insert(&l.id);
            }
        }
        one.layers.retain(|l| keep.contains(l.id.as_str()));
        if let Content::Vector {
            objects, source, ..
        } = &layer.content
        {
            if layer.mask.is_none()
                && layer.adjustments == Adjustments::default()
                && layer.blend == "normal"
            {
                let source = source
                    .as_ref()
                    .map(|a| l.bytes(id, a))
                    .transpose()?
                    .map(|b| {
                        String::from_utf8(b).map_err(|_| storage::refused("Invalid SVG source."))
                    })
                    .transpose()?;
                let t = layer.transform;
                out.push_str(&format!(
                    "<g opacity=\"{}\" transform=\"matrix({} {} {} {} {} {})\">",
                    layer.opacity, t.a, t.b, t.c, t.d, t.e, t.f
                ));
                out.push_str(&render::vector_svg(m, objects, source.as_deref()));
                out.push_str("</g>");
                continue;
            }
        }
        let p = render::render(l, id, &one, render::full(m), m.width.max(m.height))?;
        out.push_str(&format!(
            "<image width=\"{}\" height=\"{}\" xlink:href=\"data:image/png;base64,{}\"/>",
            m.width,
            m.height,
            STANDARD.encode(
                p.encode_png()
                    .map_err(|e| storage::refused(e.to_string()))?
            )
        ));
    }
    out.push_str("</svg>");
    Ok(out)
}
fn pdf(p: &tiny_skia::Pixmap) -> Result<Vec<u8>> {
    use lopdf::{
        content::{Content, Operation},
        dictionary, Document as Pdf, Object, Stream,
    };
    let rgba = render::rgba(p);
    let mut rgb = Vec::new();
    let mut alpha = Vec::new();
    for pixel in rgba.pixels() {
        rgb.extend_from_slice(&pixel.0[..3]);
        alpha.push(pixel[3]);
    }
    let mut pdf = Pdf::with_version("1.7");
    let pages = pdf.new_object_id();
    let mask=pdf.add_object(Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>"Image","Width"=>p.width() as i64,"Height"=>p.height() as i64,"ColorSpace"=>"DeviceGray","BitsPerComponent"=>8},alpha));
    let image=pdf.add_object(Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>"Image","Width"=>p.width() as i64,"Height"=>p.height() as i64,"ColorSpace"=>"DeviceRGB","BitsPerComponent"=>8,"SMask"=>mask},rgb));
    let width = p.width() as f32 * 0.75;
    let height = p.height() as f32 * 0.75;
    let content = Content {
        operations: vec![
            Operation::new("q", vec![]),
            Operation::new(
                "cm",
                vec![
                    width.into(),
                    0.into(),
                    0.into(),
                    height.into(),
                    0.into(),
                    0.into(),
                ],
            ),
            Operation::new("Do", vec![Object::Name(b"Image".to_vec())]),
            Operation::new("Q", vec![]),
        ],
    }
    .encode()
    .map_err(|e| storage::refused(e.to_string()))?;
    let stream = pdf.add_object(Stream::new(dictionary! {}, content));
    let page=pdf.add_object(dictionary!{"Type"=>"Page","Parent"=>pages,"MediaBox"=>vec![0.into(),0.into(),width.into(),height.into()],"Contents"=>stream,"Resources"=>dictionary!{"XObject"=>dictionary!{"Image"=>image}}});
    pdf.objects.insert(
        pages,
        dictionary! {"Type"=>"Pages","Kids"=>vec![page.into()],"Count"=>1}.into(),
    );
    let root = pdf.add_object(dictionary! {"Type"=>"Catalog","Pages"=>pages});
    pdf.trailer.set("Root", root);
    pdf.compress();
    let mut bytes = Vec::new();
    pdf.save_to(&mut bytes)
        .map_err(|e| storage::refused(e.to_string()))?;
    Ok(bytes)
}
pub use assist::ask;
fn valid_view(m: &Record, key: &str, v: &Value) -> bool {
    let number = |min: f64, max: f64| {
        v.as_f64()
            .is_some_and(|n| n.is_finite() && (min..=max).contains(&n))
    };
    match key {
        "layer"=>v.is_null()||v.as_str().is_some_and(|id|m.layers.iter().any(|l|l.id==id)),
        "object"=>v.is_null()||v.as_str().is_some_and(|id|m.layers.iter().any(|l|matches!(&l.content,Content::Vector{objects,..}if objects.iter().any(|o|o.id==id)))),
        "tool"=>v.as_str().is_some_and(|s|matches!(s,"move"|"brush"|"eraser"|"fill"|"rect"|"ellipse"|"pen"|"text"|"eyedropper"|"select"|"lasso"|"wand"|"pan"|"crop")),
        "mask"|"inspector"=>v.is_boolean(),
        "zoom"=>number(0.1,32.),"panX"|"panY"=>number(-999_999.,999_999.),"rotation"=>number(-360.,360.),"size"=>number(0.5,1000.),"opacity"=>number(0.,1.),"tolerance"=>v.as_u64().is_some_and(|n|n<=255),
        "color"=>v.as_str().is_some_and(|s|colour(s).is_ok()),_=>false,
    }
}
