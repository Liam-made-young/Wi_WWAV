use super::*;
use std::io::Cursor;

fn read(app: &App, d: &Value) -> Value {
    app.call("console.image.read", json!({"id":d["id"]}))
}
fn edit(app: &App, d: &Value, action: Value) -> Value {
    app.call(
        "console.image.edit",
        json!({"id":d["id"],"base":d["head"],"action":action}),
    )["document"]
        .clone()
}
fn canvas(app: &App) -> Value {
    let d = app.create("image", "Image");
    edit(
        app,
        &d,
        json!({"type":"crop","x":0,"y":0,"width":64,"height":64}),
    )
}
fn layer(app: &App, d: &Value) -> String {
    read(app, d)["image"]["layers"][0]["id"]
        .as_str()
        .unwrap()
        .into()
}
fn pixels(app: &App, d: &Value) -> ::image::RgbaImage {
    let p = read(app, d);
    ::image::load_from_memory(
        &STANDARD
            .decode(p["preview"]["base64"].as_str().unwrap())
            .unwrap(),
    )
    .unwrap()
    .to_rgba8()
}
fn png(w: u32, h: u32, color: [u8; 4]) -> String {
    let image = ::image::RgbaImage::from_pixel(w, h, ::image::Rgba(color));
    let mut bytes = Cursor::new(Vec::new());
    ::image::DynamicImage::ImageRgba8(image)
        .write_to(&mut bytes, ::image::ImageFormat::Png)
        .unwrap();
    STANDARD.encode(bytes.into_inner())
}
fn stroke(l: &str, x: f32, y: f32, size: f32, color: &str) -> Value {
    json!({"type":"stroke","layer":l,"points":[{"x":x,"y":y,"pressure":1}],"size":size,"color":color,"opacity":1,"eraser":false,"mask":false})
}
fn object() -> Value {
    json!({"id":"","name":"Red square","shape":{"type":"rect","x":8,"y":8,"width":32,"height":32,"radius":0},"fill":"#ff0000","stroke":null,"strokeWidth":0,"opacity":1,"transform":{"a":1,"b":0,"c":0,"d":1,"e":0,"f":0}})
}
fn adjustments() -> Value {
    json!({"exposure":0,"contrast":0,"saturation":0,"temperature":0,"tint":0,"curves":[[0,0],[1,1]]})
}

#[test]
fn native_bundle_versions_and_full_variations_preserve_pixel_vector_and_masks() {
    let app = App::new();
    let d = canvas(&app);
    let l = layer(&app, &d);
    let d = edit(&app, &d, stroke(&l, 15., 15., 12., "#00ff00"));
    let d = edit(
        &app,
        &d,
        json!({"type":"add","kind":"vector","name":"Shapes","parent":null}),
    );
    let v = layer(&app, &d);
    let d = edit(
        &app,
        &d,
        json!({"type":"vectorAdd","layer":v,"object":object()}),
    );
    let d = edit(&app, &d, json!({"type":"mask","layer":v,"operation":"add"}));
    let fork = app.call(
        "console.variation",
        json!({"id":d["id"],"base":d["head"],"title":"Variation"}),
    )["document"]
        .clone();
    assert_eq!(pixels(&app, &d), pixels(&app, &fork));
    assert_eq!(
        read(&app, &fork)["image"]["layers"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(fork["parent"]["versionId"], d["head"]);
    let r = app.read(&fork);
    let folder = std::path::Path::new(r["path"].as_str().unwrap())
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    let manifest: Value =
        serde_json::from_slice(&fs::read(folder.join("manifest.json")).unwrap()).unwrap();
    let m = &manifest["versions"][0]["image"];
    assert_eq!(m["format"], "wi-image/1");
    assert!(m["layers"][0]["asset"]["file"]
        .as_str()
        .unwrap()
        .ends_with(".svg"));
    assert!(m["layers"][1]["asset"]["file"]
        .as_str()
        .unwrap()
        .ends_with(".png"));
    assert!(folder
        .join(m["layers"][0]["mask"]["file"].as_str().unwrap())
        .exists());
    assert!(app.core.invoke("console.image.edit",json!({"id":d["id"],"base":"stale","action":{"type":"background","color":"#ffffff"}})).unwrap_err().code=="conflict");
}
#[test]
fn selections_clip_strokes_fill_and_eraser_and_masks_follow_layer_transforms() {
    let app = App::new();
    let d = canvas(&app);
    let l = layer(&app, &d);
    app.call(
        "console.image.selection",
        json!({"id":d["id"],"layer":l,"shape":{"type":"rect","x":0,"y":0,"width":32,"height":64}}),
    );
    let d = edit(
        &app,
        &d,
        json!({"type":"fill","layer":l,"x":4,"y":4,"color":"#00ff00","tolerance":0,"contiguous":true,"mask":false}),
    );
    let p = pixels(&app, &d);
    assert_eq!(p.get_pixel(20, 20).0, [0, 255, 0, 255]);
    assert_eq!(p.get_pixel(40, 20)[3], 0);
    let d = edit(&app, &d, json!({"type":"mask","layer":l,"operation":"add"}));
    app.call(
        "console.image.selection",
        json!({"id":d["id"],"shape":null}),
    );
    let d = edit(
        &app,
        &d,
        json!({"type":"transform","layer":l,"matrix":{"a":1,"b":0,"c":0,"d":1,"e":20,"f":0}}),
    );
    let p = pixels(&app, &d);
    assert_eq!(p.get_pixel(40, 20)[3], 255);
    assert_eq!(p.get_pixel(55, 20)[3], 0);
    let mut s = stroke(&l, 25., 20., 10., "#000000");
    s["eraser"] = json!(true);
    let d = edit(&app, &d, s);
    assert_eq!(pixels(&app, &d).get_pixel(25, 20)[3], 0);
}
#[test]
fn wand_lasso_pressure_and_transparent_brush_are_real_raster_operations() {
    let app = App::new();
    let d = canvas(&app);
    let l = layer(&app, &d);
    let mut s = stroke(&l, 16., 32., 24., "#ff000080");
    s["points"] = json!([{"x":16,"y":32,"pressure":0.2},{"x":48,"y":32,"pressure":1}]);
    let d = edit(&app, &d, s);
    let p = pixels(&app, &d);
    assert!(p.get_pixel(48, 40)[3] > 0);
    assert_eq!(p.get_pixel(16, 40)[3], 0);
    assert!(p.get_pixel(16, 32)[3] < 255);
    app.call(
        "console.image.selection",
        json!({"id":d["id"],"shape":{"type":"wand","x":0,"y":0,"tolerance":0,"contiguous":true}}),
    );
    let d = edit(
        &app,
        &d,
        json!({"type":"fill","layer":l,"x":0,"y":0,"color":"#0000ff","tolerance":0,"contiguous":true,"mask":false}),
    );
    assert_eq!(pixels(&app, &d).get_pixel(0, 0).0, [0, 0, 255, 255]);
    app.call("console.image.selection",json!({"id":d["id"],"shape":{"type":"lasso","points":[{"x":0,"y":0},{"x":20,"y":0},{"x":0,"y":20}]}}));
    let d = edit(&app, &d, json!({"type":"clear","layer":l}));
    let p = pixels(&app, &d);
    assert_eq!(p.get_pixel(2, 2)[3], 0);
    assert_eq!(p.get_pixel(60, 60)[3], 255);
}
#[test]
fn groups_reorder_lock_duplicate_blend_and_hide_without_losing_content() {
    let app = App::new();
    let d = canvas(&app);
    let pixel = layer(&app, &d);
    let d = edit(&app, &d, stroke(&pixel, 20., 20., 20., "#808080"));
    let d = edit(
        &app,
        &d,
        json!({"type":"add","kind":"group","name":"Group","parent":null}),
    );
    let group = layer(&app, &d);
    let d = edit(
        &app,
        &d,
        json!({"type":"move","layer":pixel,"parent":group,"before":null}),
    );
    let d = edit(
        &app,
        &d,
        json!({"type":"layer","layer":group,"locked":true}),
    );
    assert!(app
        .core
        .invoke(
            "console.image.edit",
            json!({"id":d["id"],"base":d["head"],"action":stroke(&pixel,2.,2.,2.,"#ffffff")})
        )
        .is_err());
    let d = edit(
        &app,
        &d,
        json!({"type":"layer","layer":group,"locked":false}),
    );
    let d = edit(&app, &d, json!({"type":"duplicate","layer":group}));
    assert_eq!(
        read(&app, &d)["image"]["layers"].as_array().unwrap().len(),
        4
    );
    let copied = layer(&app, &d);
    let d = edit(
        &app,
        &d,
        json!({"type":"layer","layer":copied,"blend":"multiply"}),
    );
    assert!(pixels(&app, &d).get_pixel(20, 20)[0] < 100);
    let d = edit(
        &app,
        &d,
        json!({"type":"layer","layer":copied,"visible":false}),
    );
    assert_eq!(pixels(&app, &d).get_pixel(20, 20)[0], 128);
    assert!(app.core.invoke("console.image.edit",json!({"id":d["id"],"base":d["head"],"action":{"type":"move","layer":group,"parent":pixel,"before":null}})).is_err());
}
#[test]
fn photo_import_adjustments_curves_crop_rotate_resize_and_sample_work_in_rust() {
    let app = App::new();
    let d = app.call(
        "console.import",
        json!({"name":"photo.png","base64":png(80,40,[80,100,120,255])}),
    )["document"]
        .clone();
    let l = layer(&app, &d);
    assert_eq!(d["marker"], "Origin unverified");
    let mut a = adjustments();
    a["exposure"] = json!(1);
    a["temperature"] = json!(30);
    let d = edit(&app, &d, json!({"type":"adjust","layer":l,"values":a}));
    let p = pixels(&app, &d);
    assert!(p.get_pixel(0, 0)[0] > 160);
    let sample = app.call("console.image.sample", json!({"id":d["id"],"x":2,"y":2}));
    assert!(sample["color"].as_str().unwrap().starts_with('#'));
    let d = edit(
        &app,
        &d,
        json!({"type":"crop","x":10,"y":5,"width":60,"height":30}),
    );
    let d = edit(&app, &d, json!({"type":"rotate","quarter_turns":1}));
    let d = edit(&app, &d, json!({"type":"resize","width":60,"height":120}));
    assert_eq!(pixels(&app, &d).dimensions(), (60, 120));
    assert_eq!(pixels(&app, &d).get_pixel(40, 80)[3], 255);
    let mut a = adjustments();
    a["curves"] = json!([[0, 0], [0.5, 0.9], [1, 1]]);
    let d = edit(&app, &d, json!({"type":"adjust","layer":l,"values":a}));
    assert!(pixels(&app, &d).get_pixel(40, 80)[0] > 80);
}
#[test]
fn vector_and_mixed_exports_are_readable_and_preserve_blends() {
    let app = App::new();
    let d = canvas(&app);
    let d = edit(
        &app,
        &d,
        json!({"type":"add","kind":"vector","name":"Shapes","parent":null}),
    );
    let l = layer(&app, &d);
    let d = edit(
        &app,
        &d,
        json!({"type":"vectorAdd","layer":l,"object":object()}),
    );
    for format in ["png", "jpg", "svg", "pdf"] {
        let e = app.call(
            "console.image.export",
            json!({"id":d["id"],"format":format}),
        );
        let bytes = fs::read(e["path"].as_str().unwrap()).unwrap();
        assert_eq!(
            STANDARD.decode(e["base64"].as_str().unwrap()).unwrap(),
            bytes
        );
        match format {
            "png" | "jpg" => assert_eq!(::image::load_from_memory(&bytes).unwrap().width(), 64),
            "svg" => {
                let text = String::from_utf8(bytes).unwrap();
                assert!(text.contains("<rect"));
                assert!(roxmltree::Document::parse(&text).is_ok());
            }
            _ => {
                assert!(bytes.starts_with(b"%PDF-1.7"));
                assert_eq!(
                    lopdf::Document::load_mem(&bytes).unwrap().get_pages().len(),
                    1
                );
            }
        }
    }
    let d = edit(&app, &d, json!({"type":"background","color":"#808080"}));
    let d = edit(
        &app,
        &d,
        json!({"type":"layer","layer":l,"blend":"multiply"}),
    );
    let e = app.call("console.image.export", json!({"id":d["id"],"format":"svg"}));
    let text = String::from_utf8(STANDARD.decode(e["base64"].as_str().unwrap()).unwrap()).unwrap();
    assert!(text.contains("data:image/png;base64,"));
    assert!(!text.contains("<rect"));
}
#[test]
fn svg_import_is_sanitized_external_resources_cannot_be_read_and_malformed_inputs_fail() {
    let app = App::new();
    for svg in [
        "<svg xmlns='http://www.w3.org/2000/svg'><script>alert(1)</script></svg>",
        "<svg xmlns='http://www.w3.org/2000/svg'><filter/></svg>",
        "not svg",
    ] {
        assert!(app
            .core
            .invoke(
                "console.import",
                json!({"name":"bad.svg","base64":STANDARD.encode(svg)})
            )
            .is_err());
    }
    let d=app.call("console.import",json!({"name":"safe.svg","base64":STANDARD.encode("<svg xmlns='http://www.w3.org/2000/svg' width='64' height='64'><image href='file:///etc/passwd'/><rect width='32' height='32' fill='red'/></svg>")}))["document"].clone();
    assert_eq!(pixels(&app, &d).get_pixel(10, 10).0, [255, 0, 0, 255]);
    let e = app.call("console.image.export", json!({"id":d["id"],"format":"svg"}));
    assert!(
        !String::from_utf8(STANDARD.decode(e["base64"].as_str().unwrap()).unwrap())
            .unwrap()
            .contains("/etc/passwd")
    );
    assert!(app
        .core
        .invoke(
            "console.import",
            json!({"name":"bad.png","base64":STANDARD.encode("not png")})
        )
        .is_err());
}
#[test]
fn claude_shares_operations_has_provenance_and_cannot_generate_pixels_even_in_batches() {
    let app = App::new();
    let d = canvas(&app);
    let l = layer(&app, &d);
    let action = stroke(&l, 12., 12., 8., "#123456");
    assert!(app.core.invoke("console.tool.call",json!({"name":"console_image_edit","args":{"id":d["id"],"base":d["head"],"action":{"type":"batch","actions":[action]}}})).is_err());
    let mut a = adjustments();
    a["exposure"] = json!(1);
    let d=app.call("console.tool.call",json!({"name":"console_image_edit","args":{"id":d["id"],"base":d["head"],"action":{"type":"adjust","layer":l,"values":a}}}))["document"].clone();
    assert_eq!(d["marker"], "Claude assisted");
    let undo = app.call("console.undo", json!({"id":d["id"],"base":d["head"]}))["document"].clone();
    assert_eq!(
        read(&app, &undo)["image"]["layers"][0]["adjustments"]["exposure"]
            .as_f64()
            .unwrap(),
        0.
    );
    assert_eq!(undo["marker"], "Claude assisted");
    assert!(app.call(
        "console.claude.context",
        json!({"tool":"image","id":d["id"]})
    )["image"]["layers"]
        .is_array());
}
#[test]
fn descriptive_assistance_is_previewed_applied_once_and_stale_checked() {
    let app = App::with_claude(Some(
        json!({"summary":"Warmer","actions":[],"selection":{"type":"rect","x":0,"y":0,"width":16,"height":16}}),
    ));
    let d = canvas(&app);
    let a = app.call(
        "console.image.assist",
        json!({"id":d["id"],"base":d["head"],"intent":"select","prompt":"Select the corner"}),
    );
    app.call(
        "console.claude.apply",
        json!({"proposalId":a["proposal"]["id"]}),
    );
    assert_eq!(
        app.call("console.workspace", json!({}))["workspace"]["tools"]["image"]["selection"]
            ["shape"]["width"]
            .as_f64()
            .unwrap(),
        16.
    );
    assert!(app
        .core
        .invoke(
            "console.claude.apply",
            json!({"proposalId":a["proposal"]["id"]})
        )
        .is_err());
    let id = layer(&app, &d);
    let mut adjustment = adjustments();
    adjustment["temperature"] = json!(20);
    let answer = json!({"summary":"Warmer","actions":[{"type":"adjust","layer":id,"values":adjustment}],"selection":null});
    let path = app.dir.path().join("claude-test");
    fs::write(
        &path,
        format!(
            "#!/bin/sh\ncat >/dev/null\nprintf '%s\\n' '{}'\n",
            json!({"structured_output":answer})
        ),
    )
    .unwrap();
    let a = app.call(
        "console.image.assist",
        json!({"id":d["id"],"base":d["head"],"intent":"adjust","prompt":"Warmer"}),
    );
    assert_eq!(read(&app, &d)["base"], d["head"]);
    let applied = app.call(
        "console.claude.apply",
        json!({"proposalId":a["proposal"]["id"]}),
    )["document"]
        .clone();
    assert_eq!(applied["marker"], "Claude assisted");
    assert_eq!(
        read(&app, &applied)["image"]["layers"][0]["adjustments"]["temperature"]
            .as_f64()
            .unwrap(),
        20.
    );
    let a = app.call(
        "console.image.assist",
        json!({"id":d["id"],"base":applied["head"],"intent":"adjust","prompt":"Warmer"}),
    );
    edit(
        &app,
        &applied,
        json!({"type":"background","color":"#ffffff"}),
    );
    assert_eq!(
        app.core
            .invoke(
                "console.claude.apply",
                json!({"proposalId":a["proposal"]["id"]})
            )
            .unwrap_err()
            .code,
        "conflict"
    );
}
#[test]
fn invalid_dimensions_curves_transforms_and_unsupported_exports_do_not_commit() {
    let app = App::new();
    let d = canvas(&app);
    let l = layer(&app, &d);
    for action in [
        json!({"type":"resize","width":8192,"height":8192}),
        json!({"type":"transform","layer":l,"matrix":{"a":0,"b":0,"c":0,"d":0,"e":0,"f":0}}),
        json!({"type":"delete","layer":l}),
    ] {
        assert!(app
            .core
            .invoke(
                "console.image.edit",
                json!({"id":d["id"],"base":d["head"],"action":action})
            )
            .is_err());
    }
    let mut a = adjustments();
    a["curves"] = json!([[0, 0], [0.5, 1], [0.4, 0], [1, 1]]);
    assert!(app
        .core
        .invoke(
            "console.image.edit",
            json!({"id":d["id"],"base":d["head"],"action":{"type":"adjust","layer":l,"values":a}})
        )
        .is_err());
    assert_eq!(read(&app, &d)["base"], d["head"]);
    assert!(app
        .core
        .invoke("console.image.export", json!({"id":d["id"],"format":"gif"}))
        .is_err());
}
#[test]
fn image_preferences_and_selection_survive_tool_switches_without_artwork_versions() {
    let app = App::new();
    let d = canvas(&app);
    let l = layer(&app, &d);
    app.call(
        "console.image.view",
        json!({"id":d["id"],"view":{"layer":l,"tool":"pen","zoom":2,"panX":42,"rotation":15}}),
    );
    app.call(
        "console.image.selection",
        json!({"id":d["id"],"shape":{"type":"rect","x":0,"y":0,"width":16,"height":16}}),
    );
    app.call("console.selectTool", json!({"tool":"write"}));
    app.call("console.selectTool", json!({"tool":"image"}));
    let r = read(&app, &d);
    assert_eq!(r["base"], d["head"]);
    assert_eq!(r["view"]["zoom"], 2);
    assert_eq!(r["selection"]["shape"]["width"].as_f64().unwrap(), 16.);
}
