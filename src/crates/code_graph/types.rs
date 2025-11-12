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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_code_node_project() {
        let node = CodeNode::Project {
            name: "test_project".into(),
            path: "/path/to/project".into(),
            node_type: "rust".into(),
        };

        match node {
            CodeNode::Project {
                name,
                path,
                node_type,
            } => {
                assert_eq!(name, "test_project");
                assert_eq!(path, "/path/to/project");
                assert_eq!(node_type, "rust");
            }
            _ => panic!("Expected Project node"),
        }
    }

    #[test]
    fn test_code_node_file() {
        let node = CodeNode::File {
            path: "src/main.rs".into(),
            project: "myproject".into(),
        };

        match node {
            CodeNode::File { path, project } => {
                assert_eq!(path, "src/main.rs");
                assert_eq!(project, "myproject");
            }
            _ => panic!("Expected File node"),
        }
    }

    #[test]
    fn test_code_node_function() {
        let node = CodeNode::Function {
            name: "test_fn".into(),
            file: "test.rs".into(),
            visibility: "pub".into(),
            is_async: true,
        };

        match node {
            CodeNode::Function {
                name,
                file,
                visibility,
                is_async,
            } => {
                assert_eq!(name, "test_fn");
                assert_eq!(file, "test.rs");
                assert_eq!(visibility, "pub");
                assert!(is_async);
            }
            _ => panic!("Expected Function node"),
        }
    }

    #[test]
    fn test_code_node_struct() {
        let node = CodeNode::Struct {
            name: "MyStruct".into(),
            file: "lib.rs".into(),
            visibility: "pub".into(),
        };

        match node {
            CodeNode::Struct {
                name,
                file,
                visibility,
            } => {
                assert_eq!(name, "MyStruct");
                assert_eq!(file, "lib.rs");
                assert_eq!(visibility, "pub");
            }
            _ => panic!("Expected Struct node"),
        }
    }

    #[test]
    fn test_code_node_trait() {
        let node = CodeNode::Trait {
            name: "MyTrait".into(),
            file: "traits.rs".into(),
            visibility: "pub".into(),
        };

        match node {
            CodeNode::Trait {
                name,
                file,
                visibility,
            } => {
                assert_eq!(name, "MyTrait");
                assert_eq!(file, "traits.rs");
                assert_eq!(visibility, "pub");
            }
            _ => panic!("Expected Trait node"),
        }
    }

    #[test]
    fn test_code_node_module() {
        let node = CodeNode::Module {
            name: "utils".into(),
            file: "utils/mod.rs".into(),
        };

        match node {
            CodeNode::Module { name, file } => {
                assert_eq!(name, "utils");
                assert_eq!(file, "utils/mod.rs");
            }
            _ => panic!("Expected Module node"),
        }
    }

    #[test]
    fn test_code_node_impl() {
        let node = CodeNode::Impl {
            target: "MyStruct".into(),
            trait_name: Some("Display".into()),
            file: "lib.rs".into(),
        };

        match node {
            CodeNode::Impl {
                target,
                trait_name,
                file,
            } => {
                assert_eq!(target, "MyStruct");
                assert_eq!(trait_name, Some("Display".into()));
                assert_eq!(file, "lib.rs");
            }
            _ => panic!("Expected Impl node"),
        }
    }

    #[test]
    fn test_relationship() {
        let rel = Relationship {
            from: "function1".into(),
            to: "file.rs".into(),
            rel_type: "DEFINED_IN".into(),
        };

        assert_eq!(rel.from, "function1");
        assert_eq!(rel.to, "file.rs");
        assert_eq!(rel.rel_type, "DEFINED_IN");
    }

    #[test]
    fn test_graph_stats_default() {
        let stats = GraphStats::default();

        assert_eq!(stats.total_projects, 0);
        assert_eq!(stats.total_files, 0);
        assert_eq!(stats.total_functions, 0);
        assert_eq!(stats.total_structs, 0);
        assert_eq!(stats.total_traits, 0);
    }

    #[test]
    fn test_scan_progress_default() {
        let progress = ScanProgress::default();

        assert_eq!(progress.total_projects, 0);
        assert_eq!(progress.projects_scanned, 0);
        assert_eq!(progress.stage, "");
        assert_eq!(progress.message, "");
    }

    #[test]
    fn test_code_node_cloneable() {
        let node = CodeNode::Function {
            name: "test".into(),
            file: "test.rs".into(),
            visibility: "pub".into(),
            is_async: false,
        };

        let cloned = node.clone();

        match (node, cloned) {
            (
                CodeNode::Function { name: n1, .. },
                CodeNode::Function { name: n2, .. },
            ) => {
                assert_eq!(n1, n2);
            }
            _ => panic!("Clone failed"),
        }
    }

    #[test]
    fn test_relationship_cloneable() {
        let rel = Relationship {
            from: "a".into(),
            to: "b".into(),
            rel_type: "TEST".into(),
        };

        let cloned = rel.clone();

        assert_eq!(rel.from, cloned.from);
        assert_eq!(rel.to, cloned.to);
        assert_eq!(rel.rel_type, cloned.rel_type);
    }
}
