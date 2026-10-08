use super::super::storage::{self, Asset, Library, Result};
use super::model::*;
use ::image::{DynamicImage, ImageDecoder, ImageFormat, ImageReader, RgbaImage};
use base64::{engine::general_purpose::STANDARD, Engine};
use resvg::usvg;
use std::{
    collections::VecDeque,
    io::Cursor,
    sync::{Arc, Mutex, OnceLock},
};
use tiny_skia::{BlendMode, FilterQuality, Mask, Pixmap, PixmapPaint, Transform};

pub fn decode(bytes: &[u8]) -> Result<RgbaImage> {
    let mut reader = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|e| storage::refused(e.to_string()))?;
    let mut limits = ::image::Limits::default();
    limits.max_image_width = Some(MAX_EDGE);
    limits.max_image_height = Some(MAX_EDGE);
    limits.max_alloc = Some(MAX_PIXELS * 8);
    reader.limits(limits);
    let mut decoder = reader
        .into_decoder()
        .map_err(|e| storage::refused(format!("The image could not be decoded: {e}")))?;
    let (w, h) = decoder.dimensions();
    dimensions(w, h)?;
    let orientation = decoder
        .orientation()
        .map_err(|e| storage::refused(e.to_string()))?;
    let mut image =
        DynamicImage::from_decoder(decoder).map_err(|e| storage::refused(e.to_string()))?;
    image.apply_orientation(orientation);
    Ok(image.to_rgba8())
}
pub fn png(image: &RgbaImage) -> Result<Vec<u8>> {
    let mut out = Cursor::new(Vec::new());
    DynamicImage::ImageRgba8(image.clone())
        .write_to(&mut out, ImageFormat::Png)
        .map_err(|e| storage::refused(e.to_string()))?;
    Ok(out.into_inner())
}
pub fn rgba(p: &Pixmap) -> RgbaImage {
    let mut bytes = Vec::with_capacity(p.data().len());
    for pixel in p.pixels() {
        let c = pixel.demultiply();
        bytes.extend_from_slice(&[c.red(), c.green(), c.blue(), c.alpha()]);
    }
    RgbaImage::from_raw(p.width(), p.height(), bytes).expect("Pixmap dimensions match its data")
}
pub fn pixmap(image: &RgbaImage) -> Result<Pixmap> {
    let mut p = Pixmap::new(image.width(), image.height())
        .ok_or_else(|| storage::refused("The pixel buffer could not be allocated."))?;
    for (src, dst) in image.pixels().zip(p.pixels_mut()) {
        *dst = tiny_skia::ColorU8::from_rgba(src[0], src[1], src[2], src[3]).premultiply();
    }
    Ok(p)
}

fn fonts() -> Arc<usvg::fontdb::Database> {
    static FONTS: OnceLock<Arc<usvg::fontdb::Database>> = OnceLock::new();
    FONTS
        .get_or_init(|| {
            let mut db = usvg::fontdb::Database::new();
            db.load_font_data(include_bytes!("../fonts/IBMPlexMono-Regular.ttf").to_vec());
            db.load_font_data(include_bytes!("../fonts/IBMPlexSerif-Regular.ttf").to_vec());
            db.set_serif_family("IBM Plex Serif");
            db.set_sans_serif_family("IBM Plex Serif");
            db.set_monospace_family("IBM Plex Mono");
            Arc::new(db)
        })
        .clone()
}
pub fn tree(svg: &str) -> Result<usvg::Tree> {
    if svg.len() > 4 * 1024 * 1024 {
        return Err(storage::refused("An SVG layer is limited to 4 MiB."));
    }
    let doc = roxmltree::Document::parse(svg)
        .map_err(|e| storage::refused(format!("Invalid SVG: {e}")))?;
    if doc.descendants().count() > 20_000
        || doc.descendants().any(|n| {
            n.is_element() && matches!(n.tag_name().name(), "filter" | "script" | "foreignObject")
        })
    {
        return Err(storage::refused(
            "SVG filters, scripts and foreign objects are not supported in editable imports.",
        ));
    }
    let mut options = usvg::Options {
        fontdb: fonts(),
        font_family: "IBM Plex Serif".into(),
        ..Default::default()
    };
    let embedded = std::sync::atomic::AtomicU64::new(0);
    options.image_href_resolver = usvg::ImageHrefResolver {
        resolve_string: Box::new(|_, _| None),
        resolve_data: Box::new(move |mime, data, _| {
            if data.len() > 24 * 1024 * 1024
                || !matches!(
                    mime,
                    "image/png" | "image/jpeg" | "image/jpg" | "image/webp"
                )
            {
                return None;
            }
            let reader = ImageReader::new(Cursor::new(data.as_slice()))
                .with_guessed_format()
                .ok()?;
            let (w, h) = reader.into_dimensions().ok()?;
            dimensions(w, h).ok()?;
            if embedded.fetch_add(
                u64::from(w) * u64::from(h),
                std::sync::atomic::Ordering::Relaxed,
            ) + u64::from(w) * u64::from(h)
                > MAX_PIXELS
            {
                return None;
            }
            match mime {
                "image/png" => Some(usvg::ImageKind::PNG(data)),
                "image/webp" => Some(usvg::ImageKind::WEBP(data)),
                _ => Some(usvg::ImageKind::JPEG(data)),
            }
        }),
    };
    usvg::Tree::from_str(svg, &options).map_err(|e| storage::refused(e.to_string()))
}
pub fn canonical(svg: &str) -> Result<(String, u32, u32)> {
    let t = tree(svg)?;
    let w = t.size().width().ceil() as u32;
    let h = t.size().height().ceil() as u32;
    dimensions(w, h)?;
    Ok((t.to_string(&usvg::WriteOptions::default()), w, h))
}

fn matrix(t: Affine) -> String {
    format!("matrix({} {} {} {} {} {})", t.a, t.b, t.c, t.d, t.e, t.f)
}
pub fn vector_svg(m: &Record, objects: &[Vector], source: Option<&str>) -> String {
    let mut w = xmlwriter::XmlWriter::new(xmlwriter::Options::default());
    w.start_element("svg");
    w.write_attribute("xmlns", "http://www.w3.org/2000/svg");
    w.write_attribute("width", &m.width.to_string());
    w.write_attribute("height", &m.height.to_string());
    if let Some(source) = source {
        if let Ok(doc) = roxmltree::Document::parse(source) {
            copy_xml(&mut w, doc.root_element());
        }
    }
    for o in objects {
        w.start_element("g");
        w.write_attribute("transform", &matrix(o.transform));
        w.write_attribute("opacity", &o.opacity.to_string());
        w.write_attribute("fill", o.fill.as_deref().unwrap_or("none"));
        w.write_attribute("stroke", o.stroke.as_deref().unwrap_or("none"));
        w.write_attribute("stroke-width", &o.stroke_width.to_string());
        match &o.shape {
            Shape::Rect {
                x,
                y,
                width,
                height,
                radius,
            } => {
                w.start_element("rect");
                for (k, v) in [
                    ("x", x),
                    ("y", y),
                    ("width", width),
                    ("height", height),
                    ("rx", radius),
                ] {
                    w.write_attribute(k, &v.to_string());
                }
            }
            Shape::Ellipse {
                x,
                y,
                width,
                height,
            } => {
                w.start_element("ellipse");
                for (k, v) in [
                    ("cx", x + width / 2.),
                    ("cy", y + height / 2.),
                    ("rx", width / 2.),
                    ("ry", height / 2.),
                ] {
                    w.write_attribute(k, &v.to_string());
                }
            }
            Shape::Path { d } => {
                w.start_element("path");
                w.write_attribute("d", d);
                w.write_attribute("stroke-linecap", "round");
                w.write_attribute("stroke-linejoin", "round");
            }
            Shape::Text {
                x,
                y,
                text,
                size,
                font,
            } => {
                w.start_element("text");
                w.write_attribute("x", &x.to_string());
                w.write_attribute("y", &y.to_string());
                w.write_attribute("font-size", &size.to_string());
                w.write_attribute("font-family", font);
                for (i, line) in text.lines().enumerate() {
                    w.start_element("tspan");
                    w.write_attribute("x", &x.to_string());
                    if i > 0 {
                        w.write_attribute("dy", &(size * 1.2).to_string());
                    }
                    w.write_text(line);
                    w.end_element();
                }
            }
        }
        w.end_element();
        w.end_element();
    }
    w.end_element();
    w.end_document()
}
fn copy_xml(w: &mut xmlwriter::XmlWriter, n: roxmltree::Node<'_, '_>) {
    if n.is_text() {
        w.write_text(n.text().unwrap_or(""));
        return;
    }
    if !n.is_element() {
        return;
    }
    w.start_element(n.tag_name().name());
    for ns in n.namespaces() {
        if let Some(name) = ns.name() {
            w.write_attribute(&format!("xmlns:{name}"), ns.uri());
        } else {
            w.write_attribute("xmlns", ns.uri());
        }
    }
    for a in n.attributes() {
        let name = if a.namespace() == Some("http://www.w3.org/1999/xlink") {
            format!("xlink:{}", a.name())
        } else {
            a.name().to_string()
        };
        w.write_attribute(&name, a.value());
    }
    for child in n.children() {
        copy_xml(w, child);
    }
    w.end_element();
}

pub fn curve(value: f32, points: &[[f32; 2]]) -> f32 {
    let v = value.clamp(0., 1.);
    let pair = points
        .windows(2)
        .find(|p| v <= p[1][0])
        .unwrap_or(&points[points.len() - 2..]);
    let f = (v - pair[0][0]) / (pair[1][0] - pair[0][0]);
    pair[0][1] + (pair[1][1] - pair[0][1]) * f
}
pub fn adjust(p: &mut Pixmap, a: &Adjustments) {
    if *a == Adjustments::default() {
        return;
    }
    let exposure = 2f32.powf(a.exposure);
    let contrast = (1. + a.contrast / 100.).max(0.);
    let saturation = 1. + a.saturation / 100.;
    let warm = a.temperature / 500.;
    let tint = a.tint / 500.;
    for p in p.pixels_mut() {
        let original = p.demultiply();
        let mut rgb = [
            original.red() as f32 / 255.,
            original.green() as f32 / 255.,
            original.blue() as f32 / 255.,
        ];
        for (c, balance) in rgb
            .iter_mut()
            .zip([1. + warm + tint, 1. - tint, 1. - warm + tint])
        {
            *c = (*c * exposure * balance - 0.5) * contrast + 0.5;
        }
        let gray = rgb[0] * 0.2126 + rgb[1] * 0.7152 + rgb[2] * 0.0722;
        for c in &mut rgb {
            *c = curve(gray + (*c - gray) * saturation, &a.curves);
        }
        *p = tiny_skia::ColorU8::from_rgba(
            (rgb[0].clamp(0., 1.) * 255.).round() as u8,
            (rgb[1].clamp(0., 1.) * 255.).round() as u8,
            (rgb[2].clamp(0., 1.) * 255.).round() as u8,
            original.alpha(),
        )
        .premultiply();
    }
}
pub fn pixel(l: &Library, id: &str, m: &Record, asset: Option<&Asset>) -> Result<RgbaImage> {
    if let Some(a) = asset {
        decode(&l.bytes(id, a)?)
    } else {
        Ok(RgbaImage::new(m.width, m.height))
    }
}
type PixelCache = VecDeque<(String, Arc<Pixmap>)>;
fn cached_pixel(
    l: &Library,
    id: &str,
    m: &Record,
    asset: Option<&Asset>,
    adjustments: &Adjustments,
) -> Result<Arc<Pixmap>> {
    static CACHE: OnceLock<Mutex<PixelCache>> = OnceLock::new();
    let Some(asset) = asset else {
        return Ok(Arc::new(pixmap(&RgbaImage::new(m.width, m.height))?));
    };
    let folder = l.folder(id)?;
    let path = std::fs::canonicalize(folder.join(&asset.file))?;
    if !path.starts_with(std::fs::canonicalize(folder)?) {
        return Err(storage::refused("Content must stay inside its bundle."));
    }
    let metadata = std::fs::metadata(&path)?;
    let key = format!(
        "{}:{}:{}:{:?}:{}",
        path.display(),
        asset.sha256,
        metadata.len(),
        metadata.modified()?,
        serde_json::to_string(adjustments).map_err(|e| storage::refused(e.to_string()))?
    );
    let cache = CACHE.get_or_init(|| Mutex::new(VecDeque::new()));
    if let Some(found) = cache
        .lock()
        .map_err(|_| storage::refused("The pixel cache is unavailable."))?
        .iter()
        .find(|(k, _)| k == &key)
        .map(|(_, p)| p.clone())
    {
        return Ok(found);
    }
    let source = decode(&l.bytes(id, asset)?)?;
    let mut p = pixmap(&source)?;
    adjust(&mut p, adjustments);
    let p = Arc::new(p);
    let mut cache = cache
        .lock()
        .map_err(|_| storage::refused("The pixel cache is unavailable."))?;
    const BUDGET: usize = 64 * 1024 * 1024;
    while cache.iter().map(|(_, p)| p.data().len()).sum::<usize>() + p.data().len() > BUDGET
        && !cache.is_empty()
    {
        cache.pop_front();
    }
    if p.data().len() <= BUDGET {
        cache.push_back((key, p.clone()));
    }
    Ok(p)
}
pub fn mask(l: &Library, id: &str, asset: &Asset, w: u32, h: u32, view: Transform) -> Result<Mask> {
    let image = decode(&l.bytes(id, asset)?)?;
    let source = pixmap(&image)?;
    let mut out = Pixmap::new(w, h).ok_or_else(|| storage::refused("No mask buffer."))?;
    out.draw_pixmap(
        0,
        0,
        source.as_ref(),
        &PixmapPaint {
            quality: FilterQuality::Bilinear,
            ..Default::default()
        },
        view,
        None,
    );
    Ok(Mask::from_pixmap(out.as_ref(), tiny_skia::MaskType::Alpha))
}
pub fn render(l: &Library, id: &str, m: &Record, region: Rect, max_edge: u32) -> Result<Pixmap> {
    validate(m)?;
    if ![region.x, region.y, region.width, region.height]
        .iter()
        .all(|n| n.is_finite())
        || region.width <= 0.
        || region.height <= 0.
        || max_edge == 0
        || max_edge > MAX_EDGE
    {
        return Err(storage::refused("Invalid image preview region."));
    }
    let scale = (max_edge as f32 / region.width.max(region.height)).min(1.);
    let w = (region.width * scale).ceil().max(1.) as u32;
    let h = (region.height * scale).ceil().max(1.) as u32;
    dimensions(w, h)?;
    let view = Transform::from_row(scale, 0., 0., scale, -region.x * scale, -region.y * scale);
    let mut out =
        Pixmap::new(w, h).ok_or_else(|| storage::refused("The canvas could not be allocated."))?;
    if let Some(c) = &m.background {
        let c = colour(c)?;
        out.fill(tiny_skia::Color::from_rgba8(c[0], c[1], c[2], c[3]));
    }
    render_level(l, id, m, None, view, &mut out)?;
    Ok(out)
}
fn render_level(
    l: &Library,
    id: &str,
    m: &Record,
    parent: Option<&str>,
    view: Transform,
    out: &mut Pixmap,
) -> Result<()> {
    for layer in m
        .layers
        .iter()
        .rev()
        .filter(|s| s.parent.as_deref() == parent && s.visible)
    {
        let mut image = Pixmap::new(out.width(), out.height())
            .ok_or_else(|| storage::refused("The layer preview could not be allocated."))?;
        let world = view.pre_concat(m.world(&layer.id)?);
        match &layer.content {
            Content::Group => {
                render_level(l, id, m, Some(&layer.id), view, &mut image)?;
                adjust(&mut image, &layer.adjustments);
            }
            Content::Pixel { asset } => {
                let p = cached_pixel(l, id, m, asset.as_ref(), &layer.adjustments)?;
                image.draw_pixmap(
                    0,
                    0,
                    p.as_ref().as_ref(),
                    &PixmapPaint {
                        quality: FilterQuality::Bilinear,
                        ..Default::default()
                    },
                    world,
                    None,
                );
            }
            Content::Vector {
                objects, source, ..
            } => {
                let source = source
                    .as_ref()
                    .map(|s| l.bytes(id, s))
                    .transpose()?
                    .map(|b| {
                        String::from_utf8(b)
                            .map_err(|_| storage::refused("SVG source is not UTF-8."))
                    })
                    .transpose()?;
                let svg = vector_svg(m, objects, source.as_deref());
                let t = tree(&svg)?;
                resvg::render(&t, world, &mut image.as_mut());
                adjust(&mut image, &layer.adjustments);
            }
        }
        let clip = layer
            .mask
            .as_ref()
            .map(|a| mask(l, id, a, out.width(), out.height(), world))
            .transpose()?;
        let blend = match layer.blend.as_str() {
            "multiply" => BlendMode::Multiply,
            "screen" => BlendMode::Screen,
            "overlay" => BlendMode::Overlay,
            "darken" => BlendMode::Darken,
            "lighten" => BlendMode::Lighten,
            "difference" => BlendMode::Difference,
            _ => BlendMode::SourceOver,
        };
        out.draw_pixmap(
            0,
            0,
            image.as_ref(),
            &PixmapPaint {
                opacity: layer.opacity,
                blend_mode: blend,
                ..Default::default()
            },
            Transform::identity(),
            clip.as_ref(),
        );
    }
    Ok(())
}
pub fn full(m: &Record) -> Rect {
    Rect {
        x: 0.,
        y: 0.,
        width: m.width as f32,
        height: m.height as f32,
    }
}
pub fn preview(
    l: &Library,
    id: &str,
    m: &Record,
    region: Rect,
    max_edge: u32,
) -> Result<serde_json::Value> {
    let p = render(l, id, m, region, max_edge)?;
    let bytes = p
        .encode_png()
        .map_err(|e| storage::refused(e.to_string()))?;
    Ok(
        serde_json::json!({"base64":STANDARD.encode(bytes),"width":p.width(),"height":p.height(),"region":region,"mime":"image/png"}),
    )
}
