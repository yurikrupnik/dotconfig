use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum CodeNode {
    Project {
        name: String,
        path: String,
        node_type: String,
    },
    File {
        path: String,
        project: String,
    },
    Function {
        name: String,
        file: String,
        visibility: String,
        is_async: bool,
    },
    Struct {
        name: String,
        file: String,
        visibility: String,
    },
    Trait {
        name: String,
        file: String,
        visibility: String,
    },
    Module {
        name: String,
        file: String,
    },
    Impl {
        target: String,
        trait_name: Option<String>,
        file: String,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Relationship {
    pub from: String,
    pub to: String,
    pub rel_type: String,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct GraphStats {
    pub total_projects: i32,
    pub total_files: i32,
    pub total_functions: i32,
    pub total_structs: i32,
    pub total_traits: i32,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct ScanProgress {
    pub total_projects: i32,
    pub projects_scanned: i32,
    pub stage: String,
    pub message: String,
}
