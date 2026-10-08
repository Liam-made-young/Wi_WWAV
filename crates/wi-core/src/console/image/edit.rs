use super::super::storage::{self, Actor, Asset, Library, Result};
use super::{model::*, render};
use ::image::{GrayImage, Luma, RgbaImage};
use imageproc::region_labelling::{connected_components, Connectivity};
use serde::{Deserialize, Serialize};
use tiny_skia::{BlendMode, FillRule, Mask, Paint, PathBuilder, Pixmap, Transform};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase", deny_unknown_fields)]
pub enum Edit {
    Batch {
        actions: Vec<Edit>,
    },
    Add {
        kind: String,
        name: String,
        parent: Option<String>,
    },
    Layer {
        layer: String,
        name: Option<String>,
        opacity: Option<f32>,
        blend: Option<String>,
        visible: Option<bool>,
        locked: Option<bool>,
    },
    Move {
        layer: String,
        parent: Option<String>,
        before: Option<String>,
    },
    Duplicate {
        layer: String,
    },
    Delete {
        layer: String,
    },
    Transform {
        layer: String,
        matrix: Affine,
    },
    Import {
        name: String,
        path: Option<String>,
        base64: Option<String>,
        fit: bool,
    },
    Stroke {
        layer: String,
        points: Vec<Point>,
        size: f32,
        color: String,
        opacity: f32,
        eraser: bool,
        mask: bool,
    },
    Fill {
        layer: String,
        x: u32,
        y: u32,
        color: String,
        tolerance: u8,
        contiguous: bool,
        mask: bool,
    },
    Clear {
        layer: String,
    },
    VectorAdd {
        layer: String,
        object: Vector,
    },
    VectorUpdate {
        layer: String,
        object: Vector,
    },
    VectorDelete {
        layer: String,
        object: String,
    },
    Adjust {
        layer: String,
        values: Adjustments,
    },
    Palette {
        colors: Vec<String>,
    },
    Background {
        color: Option<String>,
    },
    Resize {
        width: u32,
        height: u32,
    },
    Crop {
        x: u32,
        y: u32,
        width: u32,
        height: u32,
    },
    Rotate {
        quarter_turns: i32,
    },
    Mask {
        layer: String,
        operation: String,
    },
}
pub fn selection(l: &Library, id: &str, m: &Record, shape: &Selection) -> Result<RgbaImage> {
    if let Selection::Wand {
        x,
        y,
        tolerance,
        contiguous,
    } = shape
    {
        let p = render::render(l, id, m, render::full(m), m.width.max(m.height))?;
        let image = render::rgba(&p);
        return region(&image, *x, *y, *tolerance, *contiguous);
    }
    let mut p =
        Pixmap::new(m.width, m.height).ok_or_else(|| storage::refused("No selection buffer."))?;
    let mut path = PathBuilder::new();
    match shape {
        Selection::Rect {
            x,
            y,
            width,
            height,
        } => {
            if ![x, y, width, height].iter().all(|n| n.is_finite()) || *width <= 0. || *height <= 0.
            {
                return Err(storage::refused("Drag a nonempty selection."));
            }
            path.push_rect(
                tiny_skia::Rect::from_xywh(*x, *y, *width, *height)
                    .ok_or_else(|| storage::refused("Invalid selection rectangle."))?,
            );
        }
        Selection::Lasso { points } => {
            if points.len() < 3
                || points.len() > 4096
                || points.iter().any(|p| {
                    !p.x.is_finite()
                        || !p.y.is_finite()
                        || p.x.abs() > 1_000_000.
                        || p.y.abs() > 1_000_000.
                })
            {
                return Err(storage::refused("A lasso needs 3 to 4,096 finite points."));
            }
            path.move_to(points[0].x, points[0].y);
            for point in &points[1..] {
                path.line_to(point.x, point.y);
            }
            path.close();
        }
        _ => unreachable!(),
    }
    let path = path
        .finish()
        .ok_or_else(|| storage::refused("Empty selection."))?;
    let mut paint = Paint::default();
    paint.set_color_rgba8(255, 255, 255, 255);
    p.fill_path(
        &path,
        &paint,
        FillRule::Winding,
        Transform::identity(),
        None,
    );
    Ok(render::rgba(&p))
}
fn region(image: &RgbaImage, x: u32, y: u32, tolerance: u8, contiguous: bool) -> Result<RgbaImage> {
    if x >= image.width() || y >= image.height() {
        return Err(storage::refused("Choose a point inside the image."));
    }
    let seed = *image.get_pixel(x, y);
    let allowed = GrayImage::from_fn(image.width(), image.height(), |xx, yy| {
        let c = image.get_pixel(xx, yy);
        let distance = seed
            .0
            .iter()
            .zip(c.0)
            .map(|(a, b)| a.abs_diff(b))
            .max()
            .unwrap();
        Luma([u8::from(distance <= tolerance)])
    });
    if contiguous {
        let components = connected_components(&allowed, Connectivity::Four, Luma([0]));
        let label = components.get_pixel(x, y)[0];
        Ok(RgbaImage::from_fn(
            image.width(),
            image.height(),
            |xx, yy| {
                ::image::Rgba([
                    255,
                    255,
                    255,
                    if components.get_pixel(xx, yy)[0] == label {
                        255
                    } else {
                        0
                    },
                ])
            },
        ))
    } else {
        Ok(RgbaImage::from_fn(
            image.width(),
            image.height(),
            |xx, yy| ::image::Rgba([255, 255, 255, allowed.get_pixel(xx, yy)[0] * 255]),
        ))
    }
}
fn selected_mask(
    l: &Library,
    id: &str,
    m: &Record,
    layer: &str,
    w: u32,
    h: u32,
    selected: Option<&Asset>,
) -> Result<Option<Mask>> {
    selected
        .map(|asset| {
            render::mask(
                l,
                id,
                asset,
                w,
                h,
                m.world(layer)?
                    .invert()
                    .ok_or_else(|| storage::refused("The layer transform is not invertible."))?,
            )
        })
        .transpose()
}
fn body(l: &Library, id: &str, m: &Record, layer: &str, mask: bool) -> Result<RgbaImage> {
    let target = m.layer(layer)?;
    if mask {
        if let Some(a) = &target.mask {
            return render::decode(&l.bytes(id, a)?);
        }
        return Ok(RgbaImage::from_pixel(
            m.width,
            m.height,
            ::image::Rgba([255, 255, 255, 255]),
        ));
    }
    match &target.content {
        Content::Pixel { asset } => render::pixel(l, id, m, asset.as_ref()),
        _ => Err(storage::refused(
            "Choose a pixel layer for painting. Vector layers have their own shapes and pen.",
        )),
    }
}
fn set_body(
    l: &Library,
    id: &str,
    m: &mut Record,
    layer: &str,
    image: &RgbaImage,
    mask: bool,
) -> Result<()> {
    let asset = l.asset(
        id,
        &format!("{layer}{}.png", if mask { "-mask" } else { "" }),
        &render::png(image)?,
    )?;
    let target = m.layer_mut(layer)?;
    if mask {
        target.mask = Some(asset);
    } else if let Content::Pixel { asset: a } = &mut target.content {
        *a = Some(asset);
    } else {
        return Err(storage::refused("Choose a pixel layer."));
    }
    Ok(())
}
fn index(m: &Record, id: &str) -> Result<usize> {
    m.layers
        .iter()
        .position(|l| l.id == id)
        .ok_or_else(|| storage::refused("That layer is missing."))
}
fn end(m: &Record, at: usize) -> usize {
    let mut ids = std::collections::BTreeSet::from([m.layers[at].id.as_str()]);
    let mut end = at + 1;
    while end < m.layers.len()
        && m.layers[end]
            .parent
            .as_deref()
            .is_some_and(|p| ids.contains(p))
    {
        ids.insert(&m.layers[end].id);
        end += 1;
    }
    end
}
pub fn apply(
    l: &Library,
    id: &str,
    m: &mut Record,
    action: Edit,
    actor: Actor,
    selected: Option<&Asset>,
) -> Result<Option<String>> {
    if actor == Actor::Claude
        && matches!(
            action,
            Edit::Stroke { .. } | Edit::Fill { .. } | Edit::Clear { .. } | Edit::Import { .. }
        )
    {
        return Err(storage::refused("Claude may manipulate your existing work and make vectors, but cannot paint, fill, erase or import new pixels."));
    }
    let mut active = None;
    match action {
        Edit::Batch { actions } => {
            if actions.is_empty()
                || actions.len() > 32
                || actions.iter().any(|a| matches!(a, Edit::Batch { .. }))
            {
                return Err(storage::refused("Use one to 32 non-nested Image actions."));
            }
            for action in actions {
                apply(l, id, m, action, actor, selected)?;
            }
        }
        Edit::Add { kind, name, parent } => {
            let content = match kind.as_str() {
                "pixel" => Content::Pixel { asset: None },
                "vector" => Content::Vector {
                    objects: vec![],
                    source: None,
                    asset: None,
                },
                "group" => Content::Group,
                _ => return Err(storage::refused("Choose pixel, vector or group.")),
            };
            let mut layer = Layer::new(&storage::title_checked(&name)?, content);
            layer.parent = parent.clone();
            let at = if let Some(p) = parent {
                m.editable(&p)?;
                index(m, &p)? + 1
            } else {
                0
            };
            active = Some(layer.id.clone());
            m.layers.insert(at, layer);
        }
        Edit::Layer {
            layer,
            name,
            opacity,
            blend,
            visible,
            locked,
        } => {
            let target = m.layer_mut(&layer)?;
            if let Some(v) = name {
                target.name = storage::title_checked(&v)?;
            }
            if let Some(v) = opacity {
                target.opacity = v;
            }
            if let Some(v) = blend {
                target.blend = v;
            }
            if let Some(v) = visible {
                target.visible = v;
            }
            if let Some(v) = locked {
                target.locked = v;
            }
        }
        Edit::Move {
            layer,
            parent,
            before,
        } => {
            m.editable(&layer)?;
            let at = index(m, &layer)?;
            let stop = end(m, at);
            if parent
                .as_ref()
                .is_some_and(|p| m.layers[at..stop].iter().any(|l| &l.id == p))
            {
                return Err(storage::refused("A group cannot contain itself."));
            }
            if let Some(p) = &parent {
                m.editable(p)?;
            }
            let mut moved: Vec<_> = m.layers.drain(at..stop).collect();
            moved[0].parent = parent.clone();
            let dest = if let Some(b) = before {
                let at = index(m, &b)?;
                if m.layers[at].parent != parent {
                    return Err(storage::refused("Reorder layers at the same group level."));
                }
                at
            } else if let Some(p) = parent {
                end(m, index(m, &p)?)
            } else {
                m.layers.len()
            };
            m.layers.splice(dest..dest, moved);
        }
        Edit::Duplicate { layer } => {
            let at = index(m, &layer)?;
            let stop = end(m, at);
            let mut copies = m.layers[at..stop].to_vec();
            let mut ids = std::collections::BTreeMap::new();
            for l in &mut copies {
                let new = wwav_ids::ulid();
                ids.insert(l.id.clone(), new.clone());
                l.id = new;
                l.name = format!("{} copy", l.name).chars().take(200).collect();
                if let Content::Vector { objects, .. } = &mut l.content {
                    for o in objects {
                        o.id = wwav_ids::ulid();
                    }
                }
            }
            for l in &mut copies {
                if let Some(p) = &l.parent {
                    if let Some(new) = ids.get(p) {
                        l.parent = Some(new.clone());
                    }
                }
            }
            active = Some(copies[0].id.clone());
            m.layers.splice(at..at, copies);
        }
        Edit::Delete { layer } => {
            m.editable(&layer)?;
            let at = index(m, &layer)?;
            let stop = end(m, at);
            if stop - at == m.layers.len() {
                return Err(storage::refused("Keep at least one image layer."));
            }
            m.layers.drain(at..stop);
        }
        Edit::Transform { layer, matrix } => {
            m.editable(&layer)?;
            m.layer_mut(&layer)?.transform = matrix;
        }
        Edit::Import {
            name,
            path,
            base64,
            fit,
        } => {
            use base64::{engine::general_purpose::STANDARD, Engine};
            let bytes = if let Some(path) = path {
                storage::read_limited(std::path::Path::new(&path), 64 * 1024 * 1024)?
            } else {
                let s = base64.ok_or_else(|| storage::refused("Choose an image file."))?;
                if s.len() > 90 * 1024 * 1024 {
                    return Err(storage::refused("Image imports are limited to 64 MiB."));
                }
                STANDARD
                    .decode(s)
                    .map_err(|_| storage::refused("Invalid image bytes."))?
            };
            let (mut layer, w, h) = if name.to_ascii_lowercase().ends_with(".svg") {
                let s = std::str::from_utf8(&bytes)
                    .map_err(|_| storage::refused("SVG must be UTF-8."))?;
                let (s, w, h) = render::canonical(s)?;
                let a = l.asset(id, &name, s.as_bytes())?;
                (
                    Layer::new(
                        &name,
                        Content::Vector {
                            objects: vec![],
                            source: Some(a),
                            asset: None,
                        },
                    ),
                    w,
                    h,
                )
            } else {
                let image = render::decode(&bytes)?;
                let (w, h) = image.dimensions();
                let a = l.asset(
                    id,
                    &format!("{}.png", wwav_ids::ulid()),
                    &render::png(&image)?,
                )?;
                (Layer::new(&name, Content::Pixel { asset: Some(a) }), w, h)
            };
            if fit {
                m.width = w;
                m.height = h;
            } else {
                let scale = (m.width as f32 / w as f32)
                    .min(m.height as f32 / h as f32)
                    .min(1.);
                layer.transform = Affine {
                    a: scale,
                    d: scale,
                    e: (m.width as f32 - w as f32 * scale) / 2.,
                    f: (m.height as f32 - h as f32 * scale) / 2.,
                    ..Default::default()
                };
            }
            active = Some(layer.id.clone());
            m.layers.insert(0, layer);
        }
        Edit::Stroke {
            layer,
            points,
            size,
            color,
            opacity,
            eraser,
            mask,
        } => {
            m.editable(&layer)?;
            if points.is_empty()
                || points.len() > 8192
                || !size.is_finite()
                || !(0.5..=1000.).contains(&size)
                || !opacity.is_finite()
                || !(0. ..=1.).contains(&opacity)
                || points.iter().any(|p| {
                    !p.x.is_finite()
                        || !p.y.is_finite()
                        || p.x.abs() > 1_000_000.
                        || p.y.abs() > 1_000_000.
                        || !p.pressure.is_finite()
                        || !(0. ..=1.).contains(&p.pressure)
                })
            {
                return Err(storage::refused("Invalid brush gesture or size."));
            }
            let original = body(l, id, m, &layer, mask)?;
            let mut p = render::pixmap(&original)?;
            let inverse = m
                .world(&layer)?
                .invert()
                .ok_or_else(|| storage::refused("Invalid layer transform."))?;
            let scale = (inverse.sx.hypot(inverse.ky) + inverse.kx.hypot(inverse.sy)) / 2.;
            let color = if mask {
                [255, 255, 255, 255]
            } else {
                colour(&color)?
            };
            let clip = selected_mask(l, id, m, &layer, p.width(), p.height(), selected)?;
            let mut paint = Paint::default();
            paint.set_color_rgba8(
                color[0],
                color[1],
                color[2],
                (color[3] as f32 * opacity).round() as u8,
            );
            if eraser {
                paint.blend_mode = BlendMode::DestinationOut;
            }
            let points: Vec<_> = points
                .into_iter()
                .map(|p| {
                    let mut pos = tiny_skia::Point::from_xy(p.x, p.y);
                    inverse.map_point(&mut pos);
                    Point {
                        x: pos.x,
                        y: pos.y,
                        pressure: p.pressure.max(0.05),
                    }
                })
                .collect();
            let mut stamps = 0;
            for (i, next) in points.iter().enumerate() {
                let prev = if i == 0 { next } else { &points[i - 1] };
                let distance = (next.x - prev.x).hypot(next.y - prev.y);
                let steps = (distance / (size * scale * 0.15).max(0.5)).ceil().max(1.) as usize;
                stamps += steps;
                if stamps > 200_000 {
                    return Err(storage::refused(
                        "This stroke is too long. Use shorter gestures.",
                    ));
                }
                for j in 0..steps {
                    let t = (j + 1) as f32 / steps as f32;
                    let radius =
                        size * scale * (prev.pressure + (next.pressure - prev.pressure) * t) / 2.;
                    if let Some(path) = PathBuilder::from_circle(
                        prev.x + (next.x - prev.x) * t,
                        prev.y + (next.y - prev.y) * t,
                        radius.max(0.1),
                    ) {
                        p.fill_path(
                            &path,
                            &paint,
                            FillRule::Winding,
                            Transform::identity(),
                            clip.as_ref(),
                        );
                    }
                }
            }
            set_body(l, id, m, &layer, &render::rgba(&p), mask)?;
        }
        Edit::Fill {
            layer,
            x,
            y,
            color,
            tolerance,
            contiguous,
            mask,
        } => {
            m.editable(&layer)?;
            let mut image = body(l, id, m, &layer, mask)?;
            let mut p = tiny_skia::Point::from_xy(x as f32, y as f32);
            m.world(&layer)?
                .invert()
                .ok_or_else(|| storage::refused("Invalid layer transform."))?
                .map_point(&mut p);
            if p.x < 0. || p.y < 0. {
                return Err(storage::refused("Choose a point on the pixel layer."));
            }
            let area = region(&image, p.x as u32, p.y as u32, tolerance, contiguous)?;
            let clip = selected_mask(l, id, m, &layer, image.width(), image.height(), selected)?;
            let c = if mask {
                [255, 255, 255, 255]
            } else {
                colour(&color)?
            };
            for (i, (pixel, area)) in image.pixels_mut().zip(area.pixels()).enumerate() {
                let selected = clip.as_ref().map_or(255, |c| c.data()[i]);
                if area[3] > 0 && selected > 0 {
                    let amount = selected as f32 / 255.;
                    for (k, v) in pixel.0.iter_mut().enumerate() {
                        *v = (*v as f32 * (1. - amount) + c[k] as f32 * amount).round() as u8;
                    }
                }
            }
            set_body(l, id, m, &layer, &image, mask)?;
        }
        Edit::Clear { layer } => {
            m.editable(&layer)?;
            let original = body(l, id, m, &layer, false)?;
            let mut p = render::pixmap(&original)?;
            let clip = selected_mask(l, id, m, &layer, p.width(), p.height(), selected)?;
            let paint = Paint {
                blend_mode: BlendMode::Clear,
                ..Default::default()
            };
            let rect =
                tiny_skia::Rect::from_xywh(0., 0., p.width() as f32, p.height() as f32).unwrap();
            p.fill_rect(rect, &paint, Transform::identity(), clip.as_ref());
            set_body(l, id, m, &layer, &render::rgba(&p), false)?;
        }
        Edit::VectorAdd { layer, mut object } => {
            m.editable(&layer)?;
            if object.id.is_empty() {
                object.id = wwav_ids::ulid();
            }
            if let Content::Vector { objects, .. } = &mut m.layer_mut(&layer)?.content {
                objects.push(object);
            } else {
                return Err(storage::refused("Choose a vector layer."));
            }
        }
        Edit::VectorUpdate { layer, object } => {
            m.editable(&layer)?;
            if let Content::Vector { objects, .. } = &mut m.layer_mut(&layer)?.content {
                let target = objects
                    .iter_mut()
                    .find(|o| o.id == object.id)
                    .ok_or_else(|| storage::refused("That vector object is missing."))?;
                *target = object;
            } else {
                return Err(storage::refused("Choose a vector layer."));
            }
        }
        Edit::VectorDelete { layer, object } => {
            m.editable(&layer)?;
            if let Content::Vector { objects, .. } = &mut m.layer_mut(&layer)?.content {
                let old = objects.len();
                objects.retain(|o| o.id != object);
                if old == objects.len() {
                    return Err(storage::refused("That vector object is missing."));
                }
            } else {
                return Err(storage::refused("Choose a vector layer."));
            }
        }
        Edit::Adjust { layer, values } => {
            m.editable(&layer)?;
            m.layer_mut(&layer)?.adjustments = values;
        }
        Edit::Palette { colors } => m.palette = colors,
        Edit::Background { color } => m.background = color,
        Edit::Resize { width, height } => {
            dimensions(width, height)?;
            let scale = Transform::from_scale(
                width as f32 / m.width as f32,
                height as f32 / m.height as f32,
            );
            for l in m.layers.iter_mut().filter(|l| l.parent.is_none()) {
                l.transform = Affine::from(scale.pre_concat(l.transform.matrix()));
            }
            m.width = width;
            m.height = height;
        }
        Edit::Crop {
            x,
            y,
            width,
            height,
        } => {
            dimensions(width, height)?;
            if x.checked_add(width).is_none_or(|r| r > m.width)
                || y.checked_add(height).is_none_or(|b| b > m.height)
            {
                return Err(storage::refused("Crop inside the current canvas."));
            }
            let move_by = Transform::from_translate(-(x as f32), -(y as f32));
            for layer in m.layers.iter_mut().filter(|l| l.parent.is_none()) {
                layer.transform = Affine::from(move_by.pre_concat(layer.transform.matrix()));
            }
            m.width = width;
            m.height = height;
        }
        Edit::Rotate { quarter_turns } => {
            let q = quarter_turns.rem_euclid(4);
            let (w, h) = (m.width as f32, m.height as f32);
            let matrix = match q {
                1 => Transform::from_row(0., 1., -1., 0., h, 0.),
                2 => Transform::from_row(-1., 0., 0., -1., w, h),
                3 => Transform::from_row(0., -1., 1., 0., 0., w),
                _ => Transform::identity(),
            };
            for l in m.layers.iter_mut().filter(|l| l.parent.is_none()) {
                l.transform = Affine::from(matrix.pre_concat(l.transform.matrix()));
            }
            if q % 2 == 1 {
                std::mem::swap(&mut m.width, &mut m.height);
            }
        }
        Edit::Mask { layer, operation } => {
            m.editable(&layer)?;
            match operation.as_str() {
                "remove" => m.layer_mut(&layer)?.mask = None,
                "invert" => {
                    let mut image = body(l, id, m, &layer, true)?;
                    for p in image.pixels_mut() {
                        p[3] = 255 - p[3];
                    }
                    set_body(l, id, m, &layer, &image, true)?;
                }
                "add" => {
                    let image = body(l, id, m, &layer, false)
                        .unwrap_or_else(|_| RgbaImage::new(m.width, m.height));
                    let (w, h) = image.dimensions();
                    let mask = selected_mask(l, id, m, &layer, w, h, selected)?;
                    let image = RgbaImage::from_fn(w, h, |x, y| {
                        ::image::Rgba([
                            255,
                            255,
                            255,
                            mask.as_ref()
                                .map_or(255, |m| m.data()[(y * w + x) as usize]),
                        ])
                    });
                    set_body(l, id, m, &layer, &image, true)?;
                }
                _ => return Err(storage::refused("Choose add, invert or remove mask.")),
            }
        }
    }
    validate(m)?;
    Ok(active)
}
