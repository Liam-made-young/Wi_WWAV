use serde_json::{json, Value};

const TOOLS: &[(&str, &str, &str, &[&str])] = &[
    ("console_image_read","console.image.read","Read canvas dimensions, layer and vector object IDs, view, selection and bounded preview.",&["id"]),
    ("console_image_edit","console.image.edit","Edit layers, transforms, vectors, masks, palette, adjustments or canvas dimensions. One immutable Claude version. Pixel painting, fills, erasing and imports are forbidden for Claude. See docs/CONSOLE.md for action fields.",&["id","base","action"]),
    ("console_image_save","console.image.save","Save the current image and optional title as a new version.",&["id","base"]),
    ("console_image_preview","console.image.preview","Render a bounded preview or full-resolution region in Rust, optionally preview adjustments without saving.",&["id"]),
    ("console_image_selection","console.image.selection","Set or clear a rectangle, lasso or magic-wand selection without changing artwork.",&["id","shape"]),
    ("console_image_sample","console.image.sample","Sample the composite RGBA color at a canvas pixel.",&["id","x","y"]),
    ("console_image_view","console.image.view","Set selected layer/object, tool, zoom, pan, view rotation, brush, color or inspector preferences.",&["id","view"]),
    ("console_image_export","console.image.export","Export the saved image to PNG, JPG, SVG or PDF locally.",&["id","format"]),
    ("console_image_assist","console.image.assist","Preview descriptive adjustments, selections, arrangements or vector shapes with bounded vision context. Never generates pixels.",&["id","base","intent","prompt"]),
    ("console_write_read","console.write.read","Read a Write manuscript, sections, notes, research and counts.",&["id"]),
    ("console_write_save","console.write.save","Save an entire manuscript as a Claude-authored version.",&["id","base","manuscript"]),
    ("console_write_edit","console.write.edit","Add/update/move/delete/reorder sections, set mode or notes, link research, replace a UTF-16 selection or format it. Records one new version.",&["id","base","action"]),
    ("console_write_render","console.write.render","Render Markdown or Fountain safely and count words, lines and estimated English syllables.",&["text","mode"]),
    ("console_write_transform","console.write.transform","Format a UTF-16 selection in supplied text without saving.",&["text","from","to","style"]),
    ("console_write_view","console.write.view","Set focus, typewriter, preview, binder, outline, research or selected-section preferences.",&["id","view"]),
    ("console_write_export","console.write.export","Export a saved manuscript to Markdown, PDF, Fountain or plain text on this Mac.",&["id","format"]),
    ("console_write_assist","console.write.assist","Preview Write assistance: rewrite/tighten, rhymes, alternatives, summarize, reorder, or explicitly requested continuation.",&["id","intent"]),
    ("console_write_research_search","console.write.research.search","Search Learn notes to link as manuscript research.",&[]),
    ("console_write_research_read","console.write.research.read","Read a linked Learn note locally.",&["note"]),
    (
        "console_library",
        "console.library",
        "Search Console's local Library.",
        &[],
    ),
    (
        "console_select_tool",
        "console.selectTool",
        "Switch Console's active tool.",
        &["tool"],
    ),
    (
        "console_selection",
        "console.selection",
        "Set the current tool's selection context.",
        &["tool", "selection"],
    ),
    (
        "console_create",
        "console.create",
        "Create an empty document in one tool.",
        &["tool", "title"],
    ),
    (
        "console_import",
        "console.import",
        "Copy an existing local file into Console; its original authorship is unverified.",
        &[],
    ),
    (
        "console_open",
        "console.open",
        "Open a Library document in its own tool.",
        &["id"],
    ),
    (
        "console_close",
        "console.close",
        "Close a document tab, keeping its disk file.",
        &["id"],
    ),
    (
        "console_read",
        "console.read",
        "Read a document or one of its immutable versions.",
        &["id"],
    ),
    (
        "console_history",
        "console.history",
        "Read all versions, lineage and recorded authorship.",
        &["id"],
    ),
    (
        "console_save",
        "console.save",
        "Save a title or plain-text replacement as a new Claude-authored version.",
        &["id", "base"],
    ),
    (
        "console_variation",
        "console.variation",
        "Fork the current version, keeping a link to its parent.",
        &["id", "base", "title"],
    ),
    (
        "console_undo",
        "console.undo",
        "Restore the previous document state as a new version.",
        &["id", "base"],
    ),
    (
        "console_redo",
        "console.redo",
        "Restore an undone document state as a new version.",
        &["id", "base"],
    ),
    (
        "console_prepare_post",
        "console.post.prepare",
        "Prepare a local export package with lineage. Does not publish.",
        &["id", "base"],
    ),
];
pub fn command(name: &str) -> Option<&'static str> {
    TOOLS.iter().find(|t| t.0 == name).map(|t| t.1)
}
fn image_action_schema() -> Value {
    json!({"type":"object","additionalProperties":false,"required":["type"],"description":"Fields depend on type. Layer and object IDs come from console_image_read. Claude cannot stroke/fill/clear/import. Batch commits up to 32 non-nested actions as one version.","properties":{
        "type":{"type":"string","enum":["batch","add","layer","move","duplicate","delete","transform","vectorAdd","vectorUpdate","vectorDelete","adjust","palette","background","resize","crop","rotate","mask"]},
        "actions":{"type":"array","maxItems":32,"items":{"type":"object"}},
        "kind":{"type":"string","enum":["pixel","vector","group"]},"name":{"type":["string","null"]},"layer":{"type":"string"},"parent":{"type":["string","null"]},"before":{"type":["string","null"]},
        "opacity":{"type":["number","null"],"minimum":0,"maximum":1},"blend":{"type":["string","null"],"enum":[null,"normal","multiply","screen","overlay","darken","lighten","difference"]},"visible":{"type":["boolean","null"]},"locked":{"type":["boolean","null"]},
        "matrix":{"type":"object","required":["a","b","c","d","e","f"],"properties":{"a":{"type":"number"},"b":{"type":"number"},"c":{"type":"number"},"d":{"type":"number"},"e":{"type":"number"},"f":{"type":"number"}}},
        "object":{"oneOf":[{"type":"string","description":"Object ID for vectorDelete"},{"type":"object","required":["id","name","shape","fill","stroke","strokeWidth","opacity","transform"],"description":"id='' creates an ID. shape rect {type,x,y,width,height,radius}, ellipse {type,x,y,width,height}, path {type,d:SVG path}, text {type,x,y,text,size,font:IBM Plex Serif|IBM Plex Mono}. fill/stroke=#rrggbb[aa] or null; transform={a,b,c,d,e,f}.","properties":{"id":{"type":"string"},"name":{"type":"string"},"shape":{"type":"object"},"fill":{"type":["string","null"]},"stroke":{"type":["string","null"]},"strokeWidth":{"type":"number","minimum":0,"maximum":1000},"opacity":{"type":"number","minimum":0,"maximum":1},"transform":{"type":"object"}}}]},
        "values":{"type":"object","required":["exposure","contrast","saturation","temperature","tint","curves"],"properties":{"exposure":{"type":"number","minimum":-5,"maximum":5},"contrast":{"type":"number","minimum":-100,"maximum":100},"saturation":{"type":"number","minimum":-100,"maximum":100},"temperature":{"type":"number","minimum":-100,"maximum":100},"tint":{"type":"number","minimum":-100,"maximum":100},"curves":{"type":"array","minItems":2,"maxItems":16,"items":{"type":"array","minItems":2,"maxItems":2,"items":{"type":"number","minimum":0,"maximum":1}},"description":"Ordered x/y points from x=0 to x=1; default [[0,0],[1,1]]."}}},
        "colors":{"type":"array","maxItems":64,"items":{"type":"string"}},"color":{"type":["string","null"]},"width":{"type":"integer","minimum":1,"maximum":8192},"height":{"type":"integer","minimum":1,"maximum":8192},"x":{"type":"integer","minimum":0},"y":{"type":"integer","minimum":0},"quarter_turns":{"type":"integer","description":"Clockwise quarter turns for rotate"},"operation":{"type":"string","enum":["add","invert","remove"]}
    }})
}
pub fn definitions() -> Value {
    Value::Array(TOOLS.iter().map(|(name,command,description,required)|{
        let mut properties=serde_json::Map::new();
        let fields:&[&str]=match *command {
            "console.image.read"=>&["id","version"],"console.image.edit"=>&["id","base","action"],"console.image.save"=>&["id","base","title"],
            "console.image.preview"=>&["id","region","maxEdge","layer","adjustments"],"console.image.selection"=>&["id","layer","shape"],"console.image.sample"=>&["id","x","y"],"console.image.view"=>&["id","view"],"console.image.export"=>&["id","version","format","quality"],"console.image.assist"=>&["id","base","layer","intent","prompt"],
            "console.write.read"=>&["id","version"],"console.write.save"=>&["id","base","title","manuscript"],"console.write.edit"=>&["id","base","action"],
            "console.write.render"=>&["text","mode"],"console.write.transform"=>&["text","from","to","style"],"console.write.view"=>&["id","view"],
            "console.write.export"=>&["id","version","format"],"console.write.assist"=>&["id","base","section","selection","intent","prompt"],
            "console.write.research.search"=>&["query"],"console.write.research.read"=>&["note"],
            "console.library"=>&["tool","query"],"console.selectTool"=>&["tool"],"console.selection"=>&["tool","selection"],
            "console.create"=>&["tool","title"],"console.import"=>&["path","name","base64","title"],"console.read"=>&["id","version"],"console.save"=>&["id","base","title","text"],
            "console.variation"=>&["id","base","title"],"console.undo"|"console.redo"|"console.post.prepare"=>&["id","base"],_=>&["id"],
        };
        for field in fields {properties.insert((*field).into(),match *field{"action" if *command=="console.image.edit"=>image_action_schema(),"tool"=>json!({"type":"string","enum":["write","image","audiovisual","three"]}),"selection"=>json!({}),"shape"=>json!({"type":["object","null"],"description":"rect {type,x,y,width,height}, lasso {type,points:[{x,y,pressure}]}, wand {type,x,y,tolerance,contiguous}, or null"}),"manuscript"|"action"|"view"|"adjustments"|"region"=>json!({"type":"object"}),"from"|"to"|"x"|"y"|"maxEdge"|"quality"=>json!({"type":"integer","minimum":0}),"mode"=>json!({"type":"string","enum":["prose","lyrics","screenplay"]}),_=>json!({"type":"string"})});}
        json!({"name":name,"command":command,"description":description,"inputSchema":{"type":"object","additionalProperties":false,"properties":properties,"required":required}})
    }).collect())
}
