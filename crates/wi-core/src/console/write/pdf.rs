use super::super::storage::{refused, Result};
use super::{parse, Manuscript, Mode};
use lopdf::{
    content::{Content, Operation},
    dictionary, Document, Object, ObjectId, Stream, StringFormat,
};
use rustybuzz::{Face, UnicodeBuffer};
use std::collections::BTreeMap;

const MONO: &[u8] = include_bytes!("../fonts/IBMPlexMono-Regular.ttf");
const SERIF: &[u8] = include_bytes!("../fonts/IBMPlexSerif-Regular.ttf");

struct Font {
    data: &'static [u8],
    face: Face<'static>,
    name: &'static str,
    used: BTreeMap<u16, String>,
}
impl Font {
    fn new(data: &'static [u8], name: &'static str) -> Result<Self> {
        Ok(Self {
            data,
            face: Face::from_slice(data, 0)
                .ok_or_else(|| refused("The bundled PDF font could not be read."))?,
            name,
            used: BTreeMap::new(),
        })
    }
    fn shape(&self, text: &str) -> rustybuzz::GlyphBuffer {
        let mut b = UnicodeBuffer::new();
        b.push_str(text);
        b.guess_segment_properties();
        rustybuzz::shape(&self.face, &[], b)
    }
    fn width(&self, text: &str, size: f32) -> f32 {
        self.shape(text)
            .glyph_positions()
            .iter()
            .map(|p| p.x_advance as f32)
            .sum::<f32>()
            * size
            / self.face.units_per_em() as f32
    }
    fn write(
        &mut self,
        ops: &mut Vec<Operation>,
        text: &str,
        x: f32,
        y: f32,
        size: f32,
        key: &str,
    ) -> Result<()> {
        for c in text.chars() {
            if !c.is_control() && self.face.glyph_index(c).is_none() {
                return Err(refused(format!("The bundled PDF fonts cannot display '{c}'. Export Markdown or text to preserve it.")));
            }
        }
        let shaped = self.shape(text);
        let infos = shaped.glyph_infos();
        let positions = shaped.glyph_positions();
        let scale = size / self.face.units_per_em() as f32;
        let mut boundaries: Vec<_> = infos.iter().map(|g| g.cluster as usize).collect();
        boundaries.push(text.len());
        boundaries.sort_unstable();
        boundaries.dedup();
        for info in infos {
            let at = info.cluster as usize;
            let end = *boundaries.iter().find(|n| **n > at).unwrap_or(&text.len());
            if let Some(s) = text.get(at..end) {
                self.used
                    .entry(info.glyph_id as u16)
                    .or_insert_with(|| s.into());
            }
        }
        ops.push(Operation::new(
            "BDC",
            vec![
                Object::Name(b"Span".to_vec()),
                dictionary! {"ActualText"=>unicode(text)}.into(),
            ],
        ));
        ops.push(Operation::new("BT", vec![]));
        ops.push(Operation::new(
            "Tf",
            vec![Object::Name(key.as_bytes().to_vec()), size.into()],
        ));
        ops.push(Operation::new(
            "Tm",
            vec![1.into(), 0.into(), 0.into(), 1.into(), x.into(), y.into()],
        ));
        let offsets = positions.iter().any(|p| p.x_offset != 0 || p.y_offset != 0);
        if offsets {
            let mut cursor = x;
            for (info, pos) in infos.iter().zip(positions) {
                ops.push(Operation::new(
                    "Tm",
                    vec![
                        1.into(),
                        0.into(),
                        0.into(),
                        1.into(),
                        (cursor + pos.x_offset as f32 * scale).into(),
                        (y + pos.y_offset as f32 * scale).into(),
                    ],
                ));
                ops.push(Operation::new(
                    "Tj",
                    vec![Object::String(
                        (info.glyph_id as u16).to_be_bytes().to_vec(),
                        StringFormat::Hexadecimal,
                    )],
                ));
                cursor += pos.x_advance as f32 * scale;
            }
        } else {
            let mut array = Vec::new();
            let mut bytes = Vec::new();
            for (info, pos) in infos.iter().zip(positions) {
                bytes.extend_from_slice(&(info.glyph_id as u16).to_be_bytes());
                let width = self
                    .face
                    .glyph_hor_advance(rustybuzz::ttf_parser::GlyphId(info.glyph_id as u16))
                    .unwrap_or(0) as f32;
                let adjustment =
                    (width - pos.x_advance as f32) * 1000.0 / self.face.units_per_em() as f32;
                if adjustment.abs() > 0.01 {
                    array.push(Object::String(
                        std::mem::take(&mut bytes),
                        StringFormat::Hexadecimal,
                    ));
                    array.push(adjustment.into());
                }
            }
            if !bytes.is_empty() {
                array.push(Object::String(bytes, StringFormat::Hexadecimal));
            }
            ops.push(Operation::new("TJ", vec![Object::Array(array)]));
        }
        ops.push(Operation::new("ET", vec![]));
        ops.push(Operation::new("EMC", vec![]));
        Ok(())
    }
    fn embed(&self, pdf: &mut Document) -> ObjectId {
        let unit = self.face.units_per_em() as f32;
        let scale = |n: i16| n as f32 * 1000.0 / unit;
        let bbox = self.face.global_bounding_box();
        let file = pdf.add_object(Stream::new(
            dictionary! {"Length1"=>self.data.len() as i64},
            self.data.to_vec(),
        ));
        let descriptor=pdf.add_object(dictionary!{"Type"=>"FontDescriptor","FontName"=>self.name,"Flags"=>32,"FontBBox"=>vec![scale(bbox.x_min).into(),scale(bbox.y_min).into(),scale(bbox.x_max).into(),scale(bbox.y_max).into()],"ItalicAngle"=>0,"Ascent"=>scale(self.face.ascender()),"Descent"=>scale(self.face.descender()),"CapHeight"=>scale(self.face.capital_height().unwrap_or(self.face.ascender())),"StemV"=>80,"FontFile2"=>file});
        let mut widths = Vec::new();
        let mut cmap=String::from("/CIDInit /ProcSet findresource begin\n12 dict begin\nbegincmap\n/CIDSystemInfo << /Registry (Adobe) /Ordering (UCS) /Supplement 0 >> def\n/CMapName /WiWriteUnicode def\n/CMapType 2 def\n1 begincodespacerange\n<0000> <FFFF>\nendcodespacerange\n");
        for group in self.used.iter().collect::<Vec<_>>().chunks(100) {
            cmap.push_str(&format!("{} beginbfchar\n", group.len()));
            for (glyph, text) in group {
                let bytes: text_utf16::Hex = text_utf16::Hex(text);
                cmap.push_str(&format!("<{glyph:04X}> <{bytes}>\n"));
                let w = self
                    .face
                    .glyph_hor_advance(rustybuzz::ttf_parser::GlyphId(**glyph))
                    .unwrap_or(0) as f32
                    * 1000.0
                    / unit;
                widths.push(Object::Integer(**glyph as i64));
                widths.push(Object::Array(vec![w.into()]));
            }
            cmap.push_str("endbfchar\n");
        }
        cmap.push_str("endcmap\nCMapName currentdict /CMap defineresource pop\nend\nend\n");
        let unicode = pdf.add_object(Stream::new(dictionary! {}, cmap.into_bytes()));
        let cid=pdf.add_object(dictionary!{"Type"=>"Font","Subtype"=>"CIDFontType2","BaseFont"=>self.name,"CIDSystemInfo"=>dictionary!{"Registry"=>Object::string_literal("Adobe"),"Ordering"=>Object::string_literal("Identity"),"Supplement"=>0},"FontDescriptor"=>descriptor,"CIDToGIDMap"=>"Identity","W"=>widths});
        pdf.add_object(dictionary!{"Type"=>"Font","Subtype"=>"Type0","BaseFont"=>self.name,"Encoding"=>"Identity-H","DescendantFonts"=>vec![cid.into()],"ToUnicode"=>unicode})
    }
}

mod text_utf16 {
    pub struct Hex<'a>(pub &'a str);
    impl std::fmt::Display for Hex<'_> {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            for n in self.0.encode_utf16() {
                write!(f, "{n:04X}")?;
            }
            Ok(())
        }
    }
}
fn unicode(text: &str) -> Object {
    let mut bytes = vec![0xfe, 0xff];
    for n in text.encode_utf16() {
        bytes.extend_from_slice(&n.to_be_bytes());
    }
    Object::String(bytes, StringFormat::Hexadecimal)
}

fn wrap(font: &Font, text: &str, size: f32, width: f32) -> Vec<String> {
    let mut lines = Vec::new();
    let mut line = String::new();
    for word in text.split_whitespace() {
        let candidate = if line.is_empty() {
            word.into()
        } else {
            format!("{line} {word}")
        };
        if font.width(&candidate, size) <= width {
            line = candidate;
            continue;
        }
        if !line.is_empty() {
            lines.push(std::mem::take(&mut line));
        }
        if font.width(word, size) <= width {
            line = word.into();
        } else {
            let mut used = 0.0;
            for c in word.chars() {
                let w = font.width(&c.to_string(), size);
                if used + w > width && !line.is_empty() {
                    lines.push(std::mem::take(&mut line));
                    used = 0.0;
                }
                line.push(c);
                used += w;
            }
        }
    }
    if !line.is_empty() {
        lines.push(line);
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

pub fn export(title: &str, m: &Manuscript) -> Result<Vec<u8>> {
    let mut serif = Font::new(SERIF, "IBMPlexSerif-Regular")?;
    let mut mono = Font::new(MONO, "IBMPlexMono-Regular")?;
    let mut pages: Vec<Vec<Operation>> = Vec::new();
    let mut ops = Vec::new();
    let mut y = 720.0;
    let script = m.mode == Mode::Screenplay;
    let text = super::joined(m, m.sections.len() > 1);
    let blocks = parse::blocks(&text, m.mode)?;
    if script {
        let shown = blocks
            .iter()
            .find(|b| b.kind == "title")
            .map(|b| b.text.as_str())
            .unwrap_or(title);
        for (i, line) in wrap(&mono, shown, 16.0, 396.0).iter().enumerate() {
            let x = (612.0 - mono.width(line, 16.0)) / 2.0;
            mono.write(&mut ops, line, x, 400.0 - i as f32 * 20.0, 16.0, "Mono")?;
        }
        let mut credit_y = 350.0;
        for b in blocks.iter().filter(|b| b.kind == "credit") {
            for line in wrap(&mono, &b.text, 12.0, 396.0) {
                let x = (612.0 - mono.width(&line, 12.0)) / 2.0;
                mono.write(&mut ops, &line, x, credit_y, 12.0, "Mono")?;
                credit_y -= 16.0;
            }
        }
        pages.push(std::mem::take(&mut ops));
    } else {
        for line in wrap(&serif, title, 24.0, 468.0) {
            serif.write(&mut ops, &line, 72.0, y, 24.0, "Serif")?;
            y -= 30.0;
        }
        y -= 20.0;
    }
    for b in blocks {
        if b.kind == "title" || b.kind == "credit" {
            continue;
        }
        let (x, width, size, leading, gap, use_mono) = if script {
            match b.kind.as_str() {
                "character" => (266.0, 274.0, 12.0, 12.0, 0.0, true),
                "dialogue" => (180.0, 252.0, 12.0, 12.0, 0.0, true),
                "parenthetical" => (216.0, 216.0, 12.0, 12.0, 0.0, true),
                "transition" => (360.0, 180.0, 12.0, 12.0, 12.0, true),
                _ => (108.0, 432.0, 12.0, 12.0, 12.0, true),
            }
        } else {
            match b.kind.as_str() {
                "h1" => (72.0, 468.0, 18.0, 24.0, 14.0, false),
                "h2" | "h3" | "h4" | "h5" | "h6" => (72.0, 468.0, 14.0, 20.0, 12.0, false),
                "code" => (84.0, 444.0, 10.0, 14.0, 10.0, true),
                _ => (72.0, 468.0, 12.0, 18.0, 12.0, false),
            }
        };
        let font = if use_mono { &mut mono } else { &mut serif };
        let lines = b
            .text
            .split('\n')
            .flat_map(|p| wrap(font, p, size, width))
            .collect::<Vec<_>>();
        // Keep a heading or a character cue with the next line when possible.
        let reserve = if b.kind == "scene" || b.kind == "character" || b.kind.starts_with('h') {
            leading * 3.0
        } else {
            leading
        };
        if y - reserve < 72.0 {
            pages.push(std::mem::take(&mut ops));
            y = 720.0;
        }
        for line in lines {
            if y - leading < 72.0 {
                pages.push(std::mem::take(&mut ops));
                y = 720.0;
            }
            if pages.len() > 2000 {
                return Err(refused("This PDF exceeds the 2,000-page export limit."));
            }
            font.write(
                &mut ops,
                &line,
                x,
                y,
                size,
                if use_mono { "Mono" } else { "Serif" },
            )?;
            y -= leading;
        }
        y -= gap;
    }
    if !ops.is_empty() || pages.is_empty() {
        pages.push(ops);
    }
    for (i, page) in pages.iter_mut().enumerate() {
        if !script || i > 0 {
            mono.write(
                page,
                &if script {
                    i.to_string()
                } else {
                    (i + 1).to_string()
                },
                528.0,
                42.0,
                9.0,
                "Mono",
            )?;
        }
    }
    let mut pdf = Document::with_version("1.7");
    let root = pdf.new_object_id();
    let serif_id = serif.embed(&mut pdf);
    let mono_id = mono.embed(&mut pdf);
    let mut kids = Vec::new();
    for ops in pages {
        let content = Content { operations: ops }
            .encode()
            .map_err(|e| refused(e.to_string()))?;
        let stream = pdf.add_object(Stream::new(dictionary! {}, content));
        kids.push(pdf.add_object(dictionary!{"Type"=>"Page","Parent"=>root,"MediaBox"=>vec![0.into(),0.into(),612.into(),792.into()],"Contents"=>stream,"Resources"=>dictionary!{"Font"=>dictionary!{"Serif"=>serif_id,"Mono"=>mono_id}}}));
    }
    pdf.objects.insert(root,dictionary!{"Type"=>"Pages","Kids"=>kids.iter().copied().map(Object::Reference).collect::<Vec<_>>(),"Count"=>kids.len() as i64}.into());
    let catalog = pdf.add_object(dictionary! {"Type"=>"Catalog","Pages"=>root});
    pdf.trailer.set("Root", catalog);
    let info=pdf.add_object(dictionary!{"Title"=>unicode(title),"Creator"=>Object::string_literal("Wi-WWAV Console"),"Producer"=>Object::string_literal("Wi-WWAV Rust / lopdf")});
    pdf.trailer.set("Info", info);
    pdf.compress();
    let mut bytes = Vec::new();
    pdf.save_to(&mut bytes)
        .map_err(|e| refused(e.to_string()))?;
    Ok(bytes)
}
