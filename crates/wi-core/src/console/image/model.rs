use super::super::storage::{self, Asset, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use tiny_skia::Transform;

pub const MAX_PIXELS: u64 = 16 * 1024 * 1024;
pub const MAX_EDGE: u32 = 8192;
pub const MAX_IMAGE_BYTES: usize = 128 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub struct Affine {
    pub a: f32,
    pub b: f32,
    pub c: f32,
    pub d: f32,
    pub e: f32,
    pub f: f32,
}
impl Default for Affine {
    fn default() -> Self {
        Self {
            a: 1.,
            b: 0.,
            c: 0.,
            d: 1.,
            e: 0.,
            f: 0.,
        }
    }
}
impl Affine {
    pub fn matrix(self) -> Transform {
        Transform::from_row(self.a, self.b, self.c, self.d, self.e, self.f)
    }
    pub fn from(t: Transform) -> Self {
        Self {
            a: t.sx,
            b: t.ky,
            c: t.kx,
            d: t.sy,
            e: t.tx,
            f: t.ty,
        }
    }
    pub fn valid(self) -> bool {
        [self.a, self.b, self.c, self.d, self.e, self.f]
            .iter()
            .all(|n| n.is_finite() && n.abs() < 1_000_000.)
            && (self.a * self.d - self.b * self.c).abs() > 0.000001
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Adjustments {
    pub exposure: f32,
    pub contrast: f32,
    pub saturation: f32,
    pub temperature: f32,
    pub tint: f32,
    pub curves: Vec<[f32; 2]>,
}
impl Default for Adjustments {
    fn default() -> Self {
        Self {
            exposure: 0.,
            contrast: 0.,
            saturation: 0.,
            temperature: 0.,
            tint: 0.,
            curves: vec![[0., 0.], [1., 1.]],
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Shape {
    Rect {
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        radius: f32,
    },
    Ellipse {
        x: f32,
        y: f32,
        width: f32,
        height: f32,
    },
    Path {
        d: String,
    },
    Text {
        x: f32,
        y: f32,
        text: String,
        size: f32,
        font: String,
    },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Vector {
    pub id: String,
    pub name: String,
    pub shape: Shape,
    pub fill: Option<String>,
    pub stroke: Option<String>,
    pub stroke_width: f32,
    pub opacity: f32,
    pub transform: Affine,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Content {
    Pixel {
        asset: Option<Asset>,
    },
    Vector {
        objects: Vec<Vector>,
        source: Option<Asset>,
        asset: Option<Asset>,
    },
    Group,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Layer {
    pub id: String,
    pub parent: Option<String>,
    pub name: String,
    pub opacity: f32,
    pub blend: String,
    pub visible: bool,
    pub locked: bool,
    pub transform: Affine,
    pub mask: Option<Asset>,
    pub adjustments: Adjustments,
    #[serde(flatten)]
    pub content: Content,
}
impl Layer {
    pub fn new(name: &str, content: Content) -> Self {
        Self {
            id: wwav_ids::ulid(),
            parent: None,
            name: name.into(),
            opacity: 1.,
            blend: "normal".into(),
            visible: true,
            locked: false,
            transform: Affine::default(),
            mask: None,
            adjustments: Adjustments::default(),
            content,
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Record {
    pub format: String,
    pub width: u32,
    pub height: u32,
    pub background: Option<String>,
    pub layers: Vec<Layer>,
    pub palette: Vec<String>,
}
impl Record {
    pub fn blank() -> Self {
        Self {
            format: "wi-image/1".into(),
            width: 1920,
            height: 1080,
            background: None,
            layers: vec![Layer::new("Pixel layer", Content::Pixel { asset: None })],
            palette: vec![
                "#171717".into(),
                "#ffffff".into(),
                "#d94747".into(),
                "#e4c84a".into(),
                "#39a58f".into(),
                "#407bd0".into(),
            ],
        }
    }
    pub fn layer(&self, id: &str) -> Result<&Layer> {
        self.layers
            .iter()
            .find(|l| l.id == id)
            .ok_or_else(|| storage::refused("That image layer is missing."))
    }
    pub fn layer_mut(&mut self, id: &str) -> Result<&mut Layer> {
        self.layers
            .iter_mut()
            .find(|l| l.id == id)
            .ok_or_else(|| storage::refused("That image layer is missing."))
    }
    pub fn editable(&self, id: &str) -> Result<()> {
        let mut l = self.layer(id)?;
        loop {
            if l.locked {
                return Err(storage::refused("Unlock the layer before editing it."));
            }
            if let Some(parent) = &l.parent {
                l = self.layer(parent)?;
            } else {
                break;
            }
        }
        Ok(())
    }
    pub fn world(&self, id: &str) -> Result<Transform> {
        let l = self.layer(id)?;
        let local = l.transform.matrix();
        Ok(if let Some(p) = &l.parent {
            self.world(p)?.pre_concat(local)
        } else {
            local
        })
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Point {
    pub x: f32,
    pub y: f32,
    #[serde(default = "one")]
    pub pressure: f32,
}
fn one() -> f32 {
    1.
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Selection {
    Rect {
        x: f32,
        y: f32,
        width: f32,
        height: f32,
    },
    Lasso {
        points: Vec<Point>,
    },
    Wand {
        x: u32,
        y: u32,
        tolerance: u8,
        contiguous: bool,
    },
}

pub fn dimensions(w: u32, h: u32) -> Result<()> {
    if w == 0 || h == 0 || w > MAX_EDGE || h > MAX_EDGE || u64::from(w) * u64::from(h) > MAX_PIXELS
    {
        return Err(storage::refused(
            "Use nonzero dimensions up to 8,192 per edge and 16 megapixels total.",
        ));
    }
    Ok(())
}
pub fn colour(s: &str) -> Result<[u8; 4]> {
    let value = s
        .strip_prefix('#')
        .ok_or_else(|| storage::refused("Use a hexadecimal color."))?;
    if value.len() != 6 && value.len() != 8 {
        return Err(storage::refused(
            "Colors need six or eight hexadecimal digits.",
        ));
    }
    let bytes = hex::decode(value).map_err(|_| storage::refused("Invalid color."))?;
    Ok([bytes[0], bytes[1], bytes[2], *bytes.get(3).unwrap_or(&255)])
}
pub fn adjustments(a: &Adjustments) -> Result<()> {
    if !a.exposure.is_finite()
        || a.exposure.abs() > 5.
        || [a.contrast, a.saturation, a.temperature, a.tint]
            .iter()
            .any(|n| !n.is_finite() || n.abs() > 100.)
    {
        return Err(storage::refused(
            "Adjustment values are outside their range.",
        ));
    }
    if a.curves.len() < 2
        || a.curves.len() > 16
        || a.curves[0][0] != 0.
        || a.curves.last().unwrap()[0] != 1.
        || a.curves
            .iter()
            .flatten()
            .any(|v| !v.is_finite() || !(0. ..=1.).contains(v))
        || a.curves.windows(2).any(|v| v[0][0] >= v[1][0])
    {
        return Err(storage::refused(
            "A curve needs ordered points from zero to one.",
        ));
    }
    Ok(())
}
pub fn validate(m: &Record) -> Result<()> {
    dimensions(m.width, m.height)?;
    if m.format != "wi-image/1" || m.layers.is_empty() || m.layers.len() > 64 {
        return Err(storage::refused("An image needs 1 to 64 layers."));
    }
    if let Some(c) = &m.background {
        colour(c)?;
    }
    if m.palette.len() > 64 {
        return Err(storage::refused("Keep up to 64 saved colors."));
    }
    for c in &m.palette {
        colour(c)?;
    }
    let mut ids = BTreeSet::new();
    let mut ancestors: Vec<&Layer> = Vec::new();
    let mut objects = 0;
    for l in &m.layers {
        storage::check_id(&l.id)?;
        storage::title_checked(&l.name)?;
        if !ids.insert(&l.id)
            || !l.transform.valid()
            || !l.opacity.is_finite()
            || !(0. ..=1.).contains(&l.opacity)
            || !matches!(
                l.blend.as_str(),
                "normal" | "multiply" | "screen" | "overlay" | "darken" | "lighten" | "difference"
            )
        {
            return Err(storage::refused(
                "Invalid layer identity, transform, opacity or blend mode.",
            ));
        }
        if let Some(p) = &l.parent {
            while ancestors.last().is_some_and(|l| &l.id != p) {
                ancestors.pop();
            }
            if ancestors
                .last()
                .is_none_or(|l| !matches!(l.content, Content::Group))
            {
                return Err(storage::refused(
                    "Layer groups must precede their contiguous children.",
                ));
            }
        } else {
            ancestors.clear();
        }
        if ancestors.len() >= 4 {
            return Err(storage::refused(
                "Layer groups can be nested up to four levels.",
            ));
        }
        ancestors.push(l);
        adjustments(&l.adjustments)?;
        if let Content::Vector { objects: v, .. } = &l.content {
            objects += v.len();
            let mut seen = BTreeSet::new();
            for o in v {
                storage::check_id(&o.id)?;
                storage::title_checked(&o.name)?;
                if !seen.insert(&o.id)
                    || !o.transform.valid()
                    || !o.opacity.is_finite()
                    || !(0. ..=1.).contains(&o.opacity)
                    || !o.stroke_width.is_finite()
                    || !(0. ..=1000.).contains(&o.stroke_width)
                {
                    return Err(storage::refused("Invalid vector style or transform."));
                }
                for c in [&o.fill, &o.stroke].into_iter().flatten() {
                    colour(c)?;
                }
                let finite = |n: f32| n.is_finite() && n.abs() < 1_000_000.;
                let valid = match &o.shape {
                    Shape::Rect {
                        x,
                        y,
                        width,
                        height,
                        radius,
                    } => {
                        [x, y, width, height, radius].iter().all(|n| finite(**n))
                            && *width > 0.
                            && *height > 0.
                            && *radius >= 0.
                    }
                    Shape::Ellipse {
                        x,
                        y,
                        width,
                        height,
                    } => {
                        [x, y, width, height].iter().all(|n| finite(**n))
                            && *width > 0.
                            && *height > 0.
                    }
                    Shape::Path { d } => {
                        !d.is_empty()
                            && d.len() <= 128_000
                            && svgtypes::PathParser::from(d.as_str()).all(|p| p.is_ok())
                    }
                    Shape::Text {
                        x,
                        y,
                        text,
                        size,
                        font,
                    } => {
                        finite(*x)
                            && finite(*y)
                            && size.is_finite()
                            && (1. ..=1000.).contains(size)
                            && text.len() <= 16_000
                            && matches!(font.as_str(), "IBM Plex Serif" | "IBM Plex Mono")
                    }
                };
                if !valid {
                    return Err(storage::refused("Invalid vector geometry or text."));
                }
            }
        }
    }
    if objects > 4096 {
        return Err(storage::refused(
            "Keep up to 4,096 editable vector objects.",
        ));
    }
    if serde_json::to_vec(m)
        .map_err(|e| storage::refused(e.to_string()))?
        .len()
        > 2 * 1024 * 1024
    {
        return Err(storage::refused("Image metadata is limited to 2 MiB."));
    }
    Ok(())
}
