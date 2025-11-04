use std::cmp::Ordering;
use std::collections::HashMap;
use std::fs;
use std::iter::FromIterator;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct BIDSPath {
    root: Option<PathBuf>,
    entities: HashMap<String, String>,
}

impl BIDSPath {
    pub fn new() -> Self {
        Self {
            root: None,
            entities: HashMap::new(),
        }
    }

    pub fn with_root<P: AsRef<Path>>(mut self, root: P) -> Self {
        self.root = Some(root.as_ref().to_path_buf());
        self
    }

    pub fn with_entity(mut self, key: &str, value: &str) -> Self {
        self.entities.insert(key.to_string(), value.to_string());
        self
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.entities.get(key).map(|s| s.as_str())
    }

    pub fn contains(&self, key: &str) -> bool {
        self.entities.contains_key(key)
    }

    pub fn entities(&self) -> &HashMap<String, String> {
        &self.entities
    }

    pub fn root(&self) -> Option<&Path> {
        self.root.as_deref()
    }

    pub fn subject(&self) -> Option<&str> {
        self.get("subject")
    }

    pub fn session(&self) -> Option<&str> {
        self.get("session")
    }

    pub fn datatype(&self) -> Option<&str> {
        self.get("datatype")
    }

    pub fn suffix(&self) -> Option<&str> {
        self.get("suffix")
    }

    pub fn extension(&self) -> Option<&str> {
        self.get("extension")
    }

    pub fn fpath(&self) -> PathBuf {
        if self.root.is_none() {
            panic!("Root directory must be set to generate full path");
        }

        let mut path = self.root.as_ref().unwrap().to_path_buf();

        // Add subject directory
        if let Some(subject) = self.subject() {
            path.push(format!("sub-{}", subject));
        }

        // Add session directory if present
        if let Some(session) = self.session() {
            path.push(format!("ses-{}", session));
        }

        // Add datatype directory if present
        if let Some(datatype) = self.datatype() {
            path.push(datatype);
        }

        // Add filename
        path.push(self.basename());

        path
    }

    pub fn dirname(&self) -> PathBuf {
        if self.root.is_none() {
            panic!("Root directory must be set to generate directory path");
        }

        let mut path = self.root.as_ref().unwrap().to_path_buf();

        // Add subject directory
        if let Some(subject) = self.subject() {
            path.push(format!("sub-{}", subject));
        }

        // Add session directory if present
        if let Some(session) = self.session() {
            path.push(format!("ses-{}", session));
        }

        // Add datatype directory if present
        if let Some(datatype) = self.datatype() {
            path.push(datatype);
        }

        path
    }

    pub fn basename(&self) -> String {
        let mut parts = Vec::new();

        // Add standard entities in order
        for key in Self::standard_entities() {
            if let Some(value) = self.entities.get(*key) {
                let formatted = match *key {
                    "subject" => format!("sub-{}", value),
                    "session" => format!("ses-{}", value),
                    "run" => {
                        // Try to parse as number for zero-padding
                        if let Ok(num) = value.parse::<u32>() {
                            format!("run-{:02}", num)
                        } else {
                            format!("run-{}", value)
                        }
                    }
                    _ => format!("{}-{}", key, value),
                };
                parts.push(formatted);
            }
        }

        // Add non-standard entities sorted alphabetically
        let mut non_standard: Vec<(&String, &String)> = self
            .entities
            .iter()
            .filter(|(k, _)| !Self::is_standard_entity(k))
            .collect();

        non_standard.sort_by(|a, b| a.0.cmp(b.0));

        for (key, value) in non_standard {
            parts.push(format!("{}-{}", key, value));
        }

        // Add suffix and extension
        if let Some(suffix) = self.suffix() {
            parts.push(suffix.to_string());
        }

        let mut filename = parts.join("_");

        if let Some(ext) = self.extension() {
            filename.push_str(ext);
        }

        filename
    }

    pub fn update(&self, key: &str, value: &str) -> Self {
        let mut new_entities = self.entities.clone();
        new_entities.insert(key.to_string(), value.to_string());
        Self {
            root: self.root.clone(),
            entities: new_entities,
        }
    }

    pub fn parse_from_path<P: AsRef<Path>>(root: P, path: P) -> Option<Self> {
        let root = root.as_ref();
        let path = path.as_ref();

        if !path.starts_with(root) {
            return None;
        }

        let mut entities = HashMap::new();

        // Parse directory structure
        let relative = path.strip_prefix(root).ok()?;
        let mut parts = relative.iter();

        // Parse subject
        if let Some(subject) = parts.next() {
            if subject.to_str()?.starts_with("sub-") {
                entities.insert("subject".to_string(), subject.to_str()?.to_string().replace("sub-", ""));
            }
        }

        // Parse session
        if let Some(session) = parts.next() {
            if session.to_str()?.starts_with("ses-") {
                entities.insert("session".to_string(), session.to_str()?.to_string().replace("ses-", ""));
            }
        }

        // Parse datatype
        if let Some(datatype) = parts.next() {
            if !datatype.to_str()?.contains('.') {
                entities.insert("datatype".to_string(), datatype.to_str()?.to_string());
            }
        }

        // Parse filename
        let filename = path.file_name()?.to_str()?;
        let parsed = Self::parse_filename(filename)?;
        entities.extend(parsed);

        Some(Self {
            root: Some(root.to_path_buf()),
            entities,
        })
    }

    fn parse_filename(filename: &str) -> Option<HashMap<String, String>> {
        let mut entities = HashMap::new();

        // Extract extension
        let (name, ext) = filename.rsplit_once('.')?;
        entities.insert("extension".to_string(), format!(".{}", ext));

        // Extract suffix
        let parts: Vec<&str> = name.split('_').collect();
        if !parts.is_empty() {
            entities.insert("suffix".to_string(), parts[parts.len() - 1].to_string());

            // Parse entities
            for part in &parts[..parts.len() - 1] {
                if let Some((key, value)) = part.split_once('-') {
                    // Handle standard entity prefixes
                    let key = match key {
                        "sub" => "subject",
                        "ses" => "session",
                        "run" => "run",
                        k => k,
                    };
                    entities.insert(key.to_string(), value.to_string());
                }
            }
        }

        Some(entities)
    }

    fn is_standard_entity(key: &str) -> bool {
        Self::standard_entities().contains(&key)
    }

    fn standard_entities() -> &'static [&'static str] {
        &[
            "subject",
            "session",
            "task",
            "acquisition",
            "ceagent",
            "reconstruction",
            "direction",
            "run",
            "modality",
            "echo",
            "flip",
            "inversion",
            "mt",
            "part",
            "processing",
            "space",
            "split",
            "recording",
            "chunk",
            "atlas",
            "resolution",
            "density",
            "label",
            "description",
        ]
    }
}

#[derive(Debug)]
pub struct BIDSLayout {
    root_dir: PathBuf,
}

impl BIDSLayout {
    pub fn new<P: AsRef<Path>>(root_dir: P) -> Self {
        Self {
            root_dir: root_dir.as_ref().to_path_buf(),
        }
    }

    pub fn generate_path(&self, path: &BIDSPath) -> BIDSPath {
        BIDSPath {
            root: Some(self.root_dir.clone()),
            entities: path.entities().clone(),
        }
    }

    pub fn query(&self, path: &BIDSPath) -> Vec<BIDSPath> {
        let mut results = Vec::new();

        // Get subject directories
        let subject_dirs = if let Some(subject) = path.subject() {
            vec![self.root_dir.join(format!("sub-{}", subject))]
        } else {
            self.read_dir(&self.root_dir)
                .into_iter()
                .filter(|p| {
                    p.is_dir()
                        && p.file_name()
                            .map(|n| n.to_str().unwrap_or("").starts_with("sub-"))
                            .unwrap_or(false)
                })
                .collect()
        };

        for subject_dir in subject_dirs {
            // Get session directories
            let session_dirs = if let Some(session) = path.session() {
                vec![subject_dir.join(format!("ses-{}", session))]
            } else {
                self.read_dir(&subject_dir)
                    .into_iter()
                    .filter(|p| {
                        p.is_dir()
                            && p.file_name()
                                .map(|n| n.to_str().unwrap_or("").starts_with("ses-"))
                                .unwrap_or(false)
                    })
                    .collect()
            };

            for session_dir in session_dirs {
                // Get datatype directories
                let datatype_dirs = if let Some(datatype) = path.datatype() {
                    vec![session_dir.join(datatype)]
                } else {
                    self.read_dir(&session_dir).into_iter().filter(|p| p.is_dir()).collect()
                };

                for datatype_dir in datatype_dirs {
                    // Get files in directory
                    let files = self.read_dir(&datatype_dir).into_iter().filter(|p| p.is_file());

                    for file in files {
                        if let Some(bids_path) = BIDSPath::parse_from_path(&self.root_dir, &file) {
                            // Check if this file matches our query criteria
                            let mut match_all = true;

                            for (key, value) in path.entities() {
                                if key == "datatype" {
                                    continue; // Already handled by directory structure
                                }

                                if let Some(entity_value) = bids_path.get(key) {
                                    if entity_value != value {
                                        match_all = false;
                                        break;
                                    }
                                } else {
                                    match_all = false;
                                    break;
                                }
                            }

                            if match_all {
                                results.push(bids_path);
                            }
                        }
                    }
                }
            }
        }

        results.sort_by(|a, b| {
            let a_path = a.fpath();
            let b_path = b.fpath();
            a_path.cmp(&b_path)
        });

        results
    }

    /// Retruns all subject identifiers in the BIDS Layout.
    pub fn subjects(&self) -> Vec<String> {
        let mut subjects = Vec::new();
        if !self.root_dir.exists() {
            return subjects;
        }
        if let Ok(entries) = fs::read_dir(&self.root_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                        if name.starts_with("sub-") {
                            subjects.push(name[4..].to_string());
                        }
                    }
                }
            }
        }
        subjects.sort();
        subjects.dedup();
        subjects
    }

    /// Returns all session identifiers for a given subject in the BIDS Layout.
    pub fn sessions(&self, subject: &str) -> Vec<String> {
        let subject_path = self.root_dir.join(format!("sub-{}", subject));
        if !subject_path.exists() {
            return Vec::new();
        }

        let mut sessions = Vec::new();
        if let Ok(entries) = fs::read_dir(&subject_path) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                        if name.starts_with("ses-") {
                            sessions.push(name[4..].to_string());
                        }
                    }
                }
            }
        }
        sessions.sort();
        sessions.dedup();
        sessions
    }

    fn read_dir(&self, path: &Path) -> Vec<PathBuf> {
        if !path.exists() {
            return Vec::new();
        }

        fs::read_dir(path)
            .map(|entries| entries.filter_map(|e| e.ok().map(|e| e.path())).collect())
            .unwrap_or_default()
    }
}
