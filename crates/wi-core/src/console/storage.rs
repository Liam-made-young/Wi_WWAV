use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::CoreError;

pub type Result<T> = std::result::Result<T, CoreError>;
pub const MAX_CONTENT: usize = 24 * 1024 * 1024;
pub const MAX_TEXT: usize = 1024 * 1024;

pub fn refused(message: impl Into<String>) -> CoreError {
    CoreError::new("console", message)
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Tool {
    Write,
    Image,
    Audiovisual,
    Three,
}

impl Tool {
    pub fn parse(s: &str) -> Result<Self> {
        match s {
            "write" => Ok(Self::Write),
            "image" => Ok(Self::Image),
            "audiovisual" => Ok(Self::Audiovisual),
            "three" => Ok(Self::Three),
            _ => Err(refused("Choose Write, Image, Audiovisual, or 3D.")),
        }
    }
    pub fn id(self) -> &'static str {
        match self {
            Self::Write => "write",
            Self::Image => "image",
            Self::Audiovisual => "audiovisual",
            Self::Three => "three",
        }
    }
    fn blank(self) -> (&'static str, Vec<u8>) {
        match self {
            Self::Write => ("draft.md", Vec::new()),
            Self::Image => (
                "image.json",
                b"{\n  \"format\": \"wi-console-image/0.1\",\n  \"layers\": []\n}\n".to_vec(),
            ),
            Self::Audiovisual => (
                "timeline.json",
                b"{\n  \"format\": \"wi-console-av/0.1\",\n  \"tracks\": []\n}\n".to_vec(),
            ),
            Self::Three => (
                "scene.json",
                b"{\n  \"format\": \"wi-console-3d/0.1\",\n  \"objects\": []\n}\n".to_vec(),
            ),
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Actor {
    Hand,
    Claude,
    Import,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Asset {
    pub name: String,
    pub file: String,
    pub mime: String,
    pub bytes: usize,
    pub sha256: String,
    pub preview: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Version {
    pub id: String,
    pub previous: Option<String>,
    pub at: u64,
    pub actor: Actor,
    pub action: String,
    pub title: String,
    pub asset: Asset,
    pub restored_from: Option<String>,
    pub changes: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub write: Option<super::write::Record>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image: Option<super::image::Record>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Parent {
    pub document_id: String,
    pub version_id: String,
    pub title: String,
    pub claude_assisted: bool,
    pub origin_unverified: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Document {
    pub format: String,
    pub id: String,
    pub tool: Tool,
    pub created_at: u64,
    pub head: String,
    pub parent: Option<Parent>,
    pub versions: Vec<Version>,
    pub undo: Vec<String>,
    pub redo: Vec<String>,
}

impl Document {
    pub fn current(&self) -> Result<&Version> {
        self.versions
            .iter()
            .find(|v| v.id == self.head)
            .ok_or_else(|| refused("The document's current version is missing."))
    }
    pub fn assisted(&self) -> bool {
        self.parent.as_ref().is_some_and(|p| p.claude_assisted)
            || self.versions.iter().any(|v| v.actor == Actor::Claude)
    }
    pub fn unverified(&self) -> bool {
        self.parent.as_ref().is_some_and(|p| p.origin_unverified)
            || self.versions.iter().any(|v| v.actor == Actor::Import)
    }
    pub fn marker(&self) -> &'static str {
        if self.assisted() {
            "Claude assisted"
        } else if self.unverified() {
            "Origin unverified"
        } else {
            "Made by hand"
        }
    }
    pub fn summary(&self) -> Result<Value> {
        let v = self.current()?;
        Ok(
            json!({"id":self.id,"tool":self.tool,"title":v.title,"head":self.head,
            "createdAt":self.created_at,"updatedAt":v.at,"versions":self.versions.len(),
            "parent":self.parent,"marker":self.marker(),"asset":v.asset,
            "canUndo":!self.undo.is_empty(),"canRedo":!self.redo.is_empty()}),
        )
    }
    pub(super) fn check_base(&self, base: &str) -> Result<()> {
        if self.head != base {
            return Err(CoreError::new(
                "conflict",
                "This document changed. Reopen it before saving; your draft is still here.",
            ));
        }
        Ok(())
    }
}

pub struct Library {
    pub root: PathBuf,
    _lock: File,
}

impl Library {
    /// One OS lock covers the manifest commit and workspace across processes.
    pub fn open(root: &Path) -> Result<Self> {
        fs::create_dir_all(root)?;
        let root = fs::canonicalize(root)?;
        let lock_path = root.join(".console.lock");
        reject_symlink(&lock_path)?;
        let lock = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(lock_path)?;
        #[cfg(unix)]
        {
            use std::os::fd::AsRawFd;
            if unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX) } != 0 {
                return Err(std::io::Error::last_os_error().into());
            }
        }
        for dir in ["documents", "exports", "proposals"] {
            let p = root.join(dir);
            reject_symlink(&p)?;
            fs::create_dir_all(p)?;
        }
        Ok(Self { root, _lock: lock })
    }
    pub fn folder(&self, id: &str) -> Result<PathBuf> {
        check_id(id)?;
        let path = self.root.join("documents").join(format!("{id}.wwwork"));
        reject_symlink(&path)?;
        Ok(path)
    }
    pub fn load(&self, id: &str) -> Result<Document> {
        let path = self.folder(id)?.join("manifest.json");
        let doc: Document = read_json(&path)?;
        if doc.id != id || doc.format != "wi-console/0.1" {
            return Err(refused("This Console bundle has an unsupported manifest."));
        }
        doc.current()?;
        Ok(doc)
    }
    pub fn list(&self, tool: Option<Tool>, query: &str) -> Result<Value> {
        let mut documents = Vec::new();
        let mut issues = Vec::new();
        let query = query.trim().to_lowercase();
        for entry in fs::read_dir(self.root.join("documents"))? {
            let entry = entry?;
            let name = entry.file_name().to_string_lossy().into_owned();
            let Some(id) = name.strip_suffix(".wwwork") else {
                continue;
            };
            match self.load(id) {
                Ok(d) => {
                    let v = d.current()?;
                    if tool.is_some_and(|t| t != d.tool) {
                        continue;
                    }
                    if !format!("{} {}", v.title, v.asset.preview)
                        .to_lowercase()
                        .contains(&query)
                    {
                        continue;
                    }
                    documents.push(d.summary()?);
                }
                Err(e) => issues.push(json!({"file":name,"message":e.message})),
            }
        }
        documents.sort_by(|a, b| {
            b["updatedAt"]
                .as_u64()
                .cmp(&a["updatedAt"].as_u64())
                .then_with(|| a["id"].as_str().cmp(&b["id"].as_str()))
        });
        Ok(json!({"root":self.root,"documents":documents,"issues":issues}))
    }
    pub(super) fn write_manifest(&self, doc: &Document) -> Result<()> {
        atomic_json(&self.folder(&doc.id)?.join("manifest.json"), doc)
    }
    pub(super) fn asset(&self, id: &str, name: &str, bytes: &[u8]) -> Result<Asset> {
        let limit = if name.ends_with(".png") || name.ends_with(".svg") {
            super::image::MAX_IMAGE_BYTES
        } else {
            MAX_CONTENT
        };
        if bytes.len() > limit {
            return Err(refused("Phase 0 files are limited to 24 MiB. Large media streaming arrives with the editors."));
        }
        let (name, mime) = file_kind(name)?;
        let text = mime.starts_with("text/") || mime == "application/json";
        if text && std::str::from_utf8(bytes).is_err() {
            return Err(refused("Text files must be UTF-8."));
        }
        let hash = hex::encode(Sha256::digest(bytes));
        let ext = name
            .rsplit('.')
            .next()
            .unwrap_or("txt")
            .to_ascii_lowercase();
        let file = format!("assets/{hash}.{ext}");
        let folder = self.folder(id)?;
        reject_symlink(&folder.join("assets"))?;
        fs::create_dir_all(folder.join("assets"))?;
        let path = folder.join(&file);
        if !path.exists() {
            atomic_bytes(&path, bytes)?;
        } else if read_limited(&path, limit)? != bytes {
            return Err(refused("A content file no longer matches its hash."));
        }
        let preview = if mime.starts_with("text/") {
            String::from_utf8_lossy(bytes).chars().take(240).collect()
        } else {
            String::new()
        };
        Ok(Asset {
            name,
            file,
            mime: mime.into(),
            bytes: bytes.len(),
            sha256: hash,
            preview,
        })
    }
    pub fn bytes(&self, id: &str, asset: &Asset) -> Result<Vec<u8>> {
        let folder = self.folder(id)?;
        let relative = Path::new(&asset.file);
        if relative
            .components()
            .any(|p| !matches!(p, std::path::Component::Normal(_)))
        {
            return Err(refused("Invalid content path."));
        }
        let path = fs::canonicalize(folder.join(relative))?;
        if !path.starts_with(fs::canonicalize(&folder)?) {
            return Err(refused("Content must stay inside its document bundle."));
        }
        let limit = if asset.mime == "image/png" || asset.mime == "image/svg+xml" {
            super::image::MAX_IMAGE_BYTES
        } else {
            MAX_CONTENT
        };
        let bytes = read_limited(&path, limit)?;
        if hex::encode(Sha256::digest(&bytes)) != asset.sha256 {
            return Err(refused("This file was changed outside Console. Import it as a new file to preserve its history."));
        }
        Ok(bytes)
    }
    pub fn create(&self, tool: Tool, title: &str, actor: Actor) -> Result<Document> {
        let (name, bytes) = tool.blank();
        self.create_with(tool, title, name, &bytes, actor, None)
    }
    pub fn create_with(
        &self,
        tool: Tool,
        title: &str,
        name: &str,
        bytes: &[u8],
        actor: Actor,
        parent: Option<Parent>,
    ) -> Result<Document> {
        let title = title_checked(title)?;
        file_kind(name)?;
        let id = wwav_ids::ulid();
        let folder = self.folder(&id)?;
        fs::create_dir(&folder)?;
        let result = (|| {
            let mut asset = self.asset(&id, name, bytes)?;
            let write = if tool == Tool::Write
                && asset.mime.starts_with("text/")
                && bytes.len() <= MAX_TEXT
            {
                let manuscript = super::write::seed(&id, name, bytes)?;
                Some(super::write::store(self, &id, &manuscript)?.0)
            } else {
                None
            };
            let image = if tool == Tool::Image {
                super::image::seed(self, &id, &asset)?
                    .map(|m| super::image::store(self, &id, &id, &m))
                    .transpose()?
                    .map(|(record, rendered)| {
                        asset = rendered;
                        record
                    })
            } else {
                None
            };
            let version = Version {
                id: wwav_ids::ulid(),
                previous: None,
                at: wwav_ids::now_ms(),
                actor,
                action: if parent.is_some() {
                    "variation"
                } else if actor == Actor::Import {
                    "import"
                } else {
                    "create"
                }
                .into(),
                title,
                asset,
                restored_from: None,
                changes: vec!["Document created".into()],
                write,
                image,
            };
            let doc = Document {
                format: "wi-console/0.1".into(),
                id,
                tool,
                created_at: version.at,
                head: version.id.clone(),
                parent,
                versions: vec![version],
                undo: vec![],
                redo: vec![],
            };
            self.write_manifest(&doc)?;
            Ok(doc)
        })();
        if result.is_err() {
            let _ = fs::remove_dir_all(folder);
        }
        result
    }
    pub fn save(
        &self,
        id: &str,
        base: &str,
        title: Option<&str>,
        text: Option<&str>,
        actor: Actor,
        reason: &str,
    ) -> Result<Document> {
        let mut doc = self.load(id)?;
        doc.check_base(base)?;
        let old = doc.current()?.clone();
        if let Some(text) = text.filter(|_| old.write.is_some()) {
            let mut manuscript = super::write::load(self, &doc, &old)?;
            if manuscript.sections.len() != 1 {
                return Err(refused(
                    "Choose a section for a text edit in this multi-section manuscript.",
                ));
            }
            manuscript.sections[0].text = text.into();
            return self.save_write(id, base, title, &manuscript, actor, reason);
        }
        let mut changes = Vec::new();
        let title = title
            .map(title_checked)
            .transpose()?
            .unwrap_or_else(|| old.title.clone());
        if title != old.title {
            changes.push(format!("Title: {} -> {}", old.title, title));
        }
        let asset = if let Some(text) = text {
            if !old.asset.mime.starts_with("text/") {
                return Err(refused(
                    "This tool's content editor is not available in Phase 0.",
                ));
            }
            if text.len() > MAX_TEXT {
                return Err(refused("Text edits are limited to 1 MiB."));
            }
            changes.push("Text replaced".into());
            self.asset(id, &old.asset.name, text.as_bytes())?
        } else {
            old.asset
        };
        let v = Version {
            id: wwav_ids::ulid(),
            previous: Some(doc.head.clone()),
            at: wwav_ids::now_ms(),
            actor,
            action: reason.chars().take(500).collect(),
            title,
            asset,
            restored_from: None,
            changes,
            write: old.write,
            image: old.image,
        };
        doc.undo.push(doc.head.clone());
        doc.redo.clear();
        doc.head = v.id.clone();
        doc.versions.push(v);
        self.write_manifest(&doc)?;
        Ok(doc)
    }
    pub fn variation(&self, id: &str, base: &str, title: &str, actor: Actor) -> Result<Document> {
        let doc = self.load(id)?;
        doc.check_base(base)?;
        let current = doc.current()?;
        let manuscript = current
            .write
            .as_ref()
            .map(|_| super::write::load(self, &doc, current))
            .transpose()?;
        let image = if doc.tool == Tool::Image
            && (current.image.is_some()
                || matches!(
                    current.asset.mime.as_str(),
                    "image/png"
                        | "image/jpeg"
                        | "image/webp"
                        | "image/svg+xml"
                        | "application/json"
                )) {
            Some(super::image::load(self, &doc, current)?)
        } else {
            None
        };
        let parent = Parent {
            document_id: doc.id.clone(),
            version_id: doc.head.clone(),
            title: current.title.clone(),
            claude_assisted: doc.assisted(),
            origin_unverified: doc.unverified(),
        };
        let mut fork = self.create_with(
            doc.tool,
            title,
            &current.asset.name,
            &self.bytes(id, &current.asset)?,
            actor,
            Some(parent),
        )?;
        if let Some(manuscript) = manuscript {
            let result = (|| -> Result<()> {
                let (record, asset) = super::write::store(self, &fork.id, &manuscript)?;
                fork.versions[0].write = Some(record);
                fork.versions[0].asset = asset;
                self.write_manifest(&fork)
            })();
            if result.is_err() {
                let _ = fs::remove_dir_all(self.folder(&fork.id)?);
            }
            result?;
        }
        if let Some(image) = image {
            let result = (|| -> Result<()> {
                let (record, asset) = super::image::store(self, id, &fork.id, &image)?;
                fork.versions[0].image = Some(record);
                fork.versions[0].asset = asset;
                self.write_manifest(&fork)
            })();
            if result.is_err() {
                let _ = fs::remove_dir_all(self.folder(&fork.id)?);
            }
            result?;
        }
        Ok(fork)
    }
    pub(super) fn save_write(
        &self,
        id: &str,
        base: &str,
        title: Option<&str>,
        manuscript: &super::write::Manuscript,
        actor: Actor,
        reason: &str,
    ) -> Result<Document> {
        let mut doc = self.load(id)?;
        doc.check_base(base)?;
        let old = doc.current()?;
        let before = super::write::load(self, &doc, old)?;
        let title = title
            .map(title_checked)
            .transpose()?
            .unwrap_or_else(|| old.title.clone());
        let mut changes = super::write::changes(&before, manuscript);
        if old.title != title {
            changes.push(format!("Title: {} -> {}", old.title, title));
        }
        let (record, asset) = super::write::store(self, id, manuscript)?;
        let v = Version {
            id: wwav_ids::ulid(),
            previous: Some(doc.head.clone()),
            at: wwav_ids::now_ms(),
            actor,
            action: reason.chars().take(500).collect(),
            title,
            asset,
            restored_from: None,
            changes,
            write: Some(record),
            image: None,
        };
        doc.undo.push(doc.head.clone());
        doc.redo.clear();
        doc.head = v.id.clone();
        doc.versions.push(v);
        self.write_manifest(&doc)?;
        Ok(doc)
    }
    pub(super) fn save_image(
        &self,
        id: &str,
        base: &str,
        title: Option<&str>,
        image: &super::image::Record,
        actor: Actor,
        reason: &str,
    ) -> Result<Document> {
        let mut doc = self.load(id)?;
        doc.check_base(base)?;
        let old = doc.current()?;
        let before = super::image::load(self, &doc, old)?;
        let title = title
            .map(title_checked)
            .transpose()?
            .unwrap_or_else(|| old.title.clone());
        let mut changes = super::image::changes(&before, image);
        if old.title != title {
            changes.push(format!("Title: {} -> {}", old.title, title));
        }
        let (record, asset) = super::image::store(self, id, id, image)?;
        let v = Version {
            id: wwav_ids::ulid(),
            previous: Some(doc.head.clone()),
            at: wwav_ids::now_ms(),
            actor,
            action: reason.chars().take(500).collect(),
            title,
            asset,
            restored_from: None,
            changes,
            write: None,
            image: Some(record),
        };
        doc.undo.push(doc.head.clone());
        doc.redo.clear();
        doc.head = v.id.clone();
        doc.versions.push(v);
        self.write_manifest(&doc)?;
        Ok(doc)
    }
    /// Undo is another immutable version, so neither actor history nor files disappear.
    pub fn restore(&self, id: &str, base: &str, redo: bool, actor: Actor) -> Result<Document> {
        let mut doc = self.load(id)?;
        doc.check_base(base)?;
        let target = if redo { doc.redo.pop() } else { doc.undo.pop() }.ok_or_else(|| {
            refused(if redo {
                "Nothing to redo."
            } else {
                "Nothing to undo."
            })
        })?;
        let old = doc
            .versions
            .iter()
            .find(|v| v.id == target)
            .ok_or_else(|| refused("That version is missing."))?
            .clone();
        if redo {
            doc.undo.push(doc.head.clone())
        } else {
            doc.redo.push(doc.head.clone())
        }
        let v = Version {
            id: wwav_ids::ulid(),
            previous: Some(doc.head.clone()),
            at: wwav_ids::now_ms(),
            actor,
            action: if redo { "redo" } else { "undo" }.into(),
            title: old.title,
            asset: old.asset,
            restored_from: Some(target),
            changes: vec!["Earlier version restored".into()],
            write: old.write,
            image: old.image,
        };
        doc.head = v.id.clone();
        doc.versions.push(v);
        self.write_manifest(&doc)?;
        Ok(doc)
    }
    pub fn package(&self, id: &str, base: &str) -> Result<Value> {
        let doc = self.load(id)?;
        doc.check_base(base)?;
        let current = doc.current()?;
        if current.asset.mime == "application/json" {
            return Err(refused(
                "Export a finished work from this tool before preparing a Space post.",
            ));
        }
        // Share the edit ledger, never private manuscript notes or research snapshots.
        let ledger: Vec<Value> = doc
            .versions
            .iter()
            .map(|v| {
                json!({
                    "id":v.id,"at":v.at,"actor":v.actor,"action":v.action,
                    "changes":v.changes,"restoredFrom":v.restored_from,
                    "title":v.title,"sha256":v.asset.sha256
                })
            })
            .collect();
        let bytes = self.bytes(id, &current.asset)?;
        let dir = self
            .root
            .join("exports")
            .join(format!("{}-{}.space", doc.id, doc.head));
        reject_symlink(&dir)?;
        if !dir.exists() {
            let stage = self
                .root
                .join("exports")
                .join(format!(".{}", wwav_ids::ulid()));
            fs::create_dir(&stage)?;
            let result = (|| -> Result<()> {
                atomic_bytes(&stage.join(&current.asset.name), &bytes)?;
                atomic_json(
                    &stage.join("space.json"),
                    &json!({"format":"wi-console-post/0.1","documentId":doc.id,"versionId":doc.head,"tool":doc.tool,"title":current.title,"file":current.asset,"lineage":doc.parent,"provenance":{"marker":doc.marker(),"versions":ledger},"published":false}),
                )?;
                fs::rename(&stage, &dir)?;
                Ok(())
            })();
            if result.is_err() {
                let _ = fs::remove_dir_all(stage);
            }
            result?;
        }
        Ok(
            json!({"path":dir,"published":false,"status":"prepared","message":"Package saved on this Mac. Posting to Space is not connected yet."}),
        )
    }
}

impl Drop for Library {
    fn drop(&mut self) {
        #[cfg(unix)]
        {
            use std::os::fd::AsRawFd;
            unsafe {
                libc::flock(self._lock.as_raw_fd(), libc::LOCK_UN);
            }
        }
    }
}

pub fn check_id(id: &str) -> Result<()> {
    if !wwav_ids::is_ulid(id) || id != id.to_ascii_uppercase() {
        Err(refused("Invalid Console id."))
    } else {
        Ok(())
    }
}
pub fn title_checked(title: &str) -> Result<String> {
    let title = title.trim();
    if title.is_empty() || title.chars().count() > 200 {
        return Err(refused("A title must contain 1 to 200 characters."));
    }
    Ok(title.into())
}
pub fn reject_symlink(path: &Path) -> Result<()> {
    if fs::symlink_metadata(path).is_ok_and(|m| m.file_type().is_symlink()) {
        return Err(refused(
            "Console does not follow links in its storage folders.",
        ));
    }
    Ok(())
}
pub fn read_limited(path: &Path, max: usize) -> Result<Vec<u8>> {
    reject_symlink(path)?;
    let f = File::open(path)?;
    if !f.metadata()?.is_file() {
        return Err(refused("Choose an ordinary file."));
    }
    let mut bytes = Vec::new();
    f.take(max as u64 + 1).read_to_end(&mut bytes)?;
    if bytes.len() > max {
        return Err(refused(format!(
            "The file exceeds the {} MiB Phase 0 limit.",
            max / (1024 * 1024)
        )));
    }
    Ok(bytes)
}
pub fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T> {
    serde_json::from_slice(&read_limited(path, 16 * 1024 * 1024)?).map_err(|e| {
        refused(format!(
            "Console couldn't read {}: {e}",
            path.file_name().unwrap_or_default().to_string_lossy()
        ))
    })
}
pub fn atomic_json(path: &Path, value: &impl Serialize) -> Result<()> {
    let bytes = serde_json::to_vec_pretty(value).map_err(|e| refused(e.to_string()))?;
    if bytes.len() > 16 * 1024 * 1024 {
        return Err(refused(
            "This document's history exceeds the Phase 0 limit. Save a variation to continue.",
        ));
    }
    atomic_bytes(path, &bytes)
}
pub fn atomic_bytes(path: &Path, bytes: &[u8]) -> Result<()> {
    reject_symlink(path)?;
    let parent = path
        .parent()
        .ok_or_else(|| refused("The file needs a folder."))?;
    let temp = parent.join(format!(".{}.tmp", wwav_ids::ulid()));
    let result = (|| -> Result<()> {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        fs::rename(&temp, path)?;
        File::open(parent)?.sync_all()?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(temp);
    }
    result
}
pub fn file_kind(name: &str) -> Result<(String, &'static str)> {
    let name = Path::new(name)
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| refused("The file needs a name."))?;
    if name.is_empty() || name.chars().count() > 200 || name.contains(['\\', '\0']) {
        return Err(refused("Invalid file name."));
    }
    let ext = name.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
    let mime = match ext.as_str() {
        "md" | "txt" | "fountain" | "obj" => "text/plain",
        "json" | "gltf" => "application/json",
        "png" => "image/png",
        "svg" => "image/svg+xml",
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        "gif" => "image/gif",
        "wav" | "wwav" => "audio/wav",
        "mp3" => "audio/mpeg",
        "m4a" => "audio/mp4",
        "ogg" => "audio/ogg",
        "flac" => "audio/flac",
        "mp4" | "swav" => "video/mp4",
        "webm" => "video/webm",
        _ => return Err(refused("This file type has no Phase 0 reader yet.")),
    };
    Ok((name.into(), mime))
}
