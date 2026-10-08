use serde_json::{json, Value};

const TOOLS: &[(&str, &str, &str, &[&str])] = &[
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
pub fn definitions() -> Value {
    Value::Array(TOOLS.iter().map(|(name,command,description,required)|{
        let mut properties=serde_json::Map::new();
        let fields:&[&str]=match *command {
            "console.write.read"=>&["id","version"],"console.write.save"=>&["id","base","title","manuscript"],"console.write.edit"=>&["id","base","action"],
            "console.write.render"=>&["text","mode"],"console.write.transform"=>&["text","from","to","style"],"console.write.view"=>&["id","view"],
            "console.write.export"=>&["id","version","format"],"console.write.assist"=>&["id","base","section","selection","intent","prompt"],
            "console.write.research.search"=>&["query"],"console.write.research.read"=>&["note"],
            "console.library"=>&["tool","query"],"console.selectTool"=>&["tool"],"console.selection"=>&["tool","selection"],
            "console.create"=>&["tool","title"],"console.import"=>&["path","name","base64","title"],"console.read"=>&["id","version"],"console.save"=>&["id","base","title","text"],
            "console.variation"=>&["id","base","title"],"console.undo"|"console.redo"|"console.post.prepare"=>&["id","base"],_=>&["id"],
        };
        for field in fields {properties.insert((*field).into(),match *field{"tool"=>json!({"type":"string","enum":["write","image","audiovisual","three"]}),"selection"=>json!({}),"manuscript"|"action"|"view"=>json!({"type":"object"}),"from"|"to"=>json!({"type":"integer","minimum":0}),"mode"=>json!({"type":"string","enum":["prose","lyrics","screenplay"]}),_=>json!({"type":"string"})});}
        json!({"name":name,"command":command,"description":description,"inputSchema":{"type":"object","additionalProperties":false,"properties":properties,"required":required}})
    }).collect())
}
