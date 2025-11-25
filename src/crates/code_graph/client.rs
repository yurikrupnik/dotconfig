use anyhow::{Context, Result};
use neo4rs::{Graph, Query};
use std::path::Path;
use syn::visit::Visit;
use walkdir::WalkDir;

use super::types::{CodeNode, GraphStats, Relationship, ScanProgress};
use super::visitor::CodeVisitor;

pub struct CodeGraphClient {
    graph: Graph,
}

impl CodeGraphClient {
    pub async fn new(uri: &str, user: &str, pass: &str) -> Result<Self> {
        let graph = Graph::new(uri, user, pass)
            .await
            .context("Failed to connect to Neo4j")?;

        Ok(Self { graph })
    }

    pub async fn init(&self) -> Result<()> {
        tracing::info!("Initializing knowledge graph schema...");

        let constraints = vec![
            "CREATE CONSTRAINT IF NOT EXISTS FOR (p:Project) REQUIRE p.name IS UNIQUE",
            "CREATE CONSTRAINT IF NOT EXISTS FOR (f:File) REQUIRE f.path IS UNIQUE",
            "CREATE CONSTRAINT IF NOT EXISTS FOR (fn:Function) REQUIRE (fn.name, fn.file) IS UNIQUE",
            "CREATE CONSTRAINT IF NOT EXISTS FOR (s:Struct) REQUIRE (s.name, s.file) IS UNIQUE",
            "CREATE CONSTRAINT IF NOT EXISTS FOR (t:Trait) REQUIRE (t.name, t.file) IS UNIQUE",
        ];

        for constraint in constraints {
            self.graph
                .run(Query::new(constraint.into()))
                .await
                .context(format!("Failed to create constraint: {}", constraint))?;
        }

        tracing::info!("Schema initialized successfully");
        Ok(())
    }

    pub async fn clear(&self) -> Result<(i32, i32)> {
        tracing::info!("Clearing all nodes and relationships...");

        self.graph
            .run(Query::new("MATCH (n) DETACH DELETE n".into()))
            .await
            .context("Failed to clear graph")?;

        tracing::info!("Graph cleared");
        Ok((0, 0))
    }

    pub async fn query(&self, cypher: &str) -> Result<String> {
        tracing::info!("Executing query: {}", cypher);

        let _ = self
            .graph
            .execute(Query::new(cypher.into()))
            .await
            .context("Failed to execute query")?;

        Ok("Query executed successfully".into())
    }

    pub async fn get_stats(&self) -> Result<GraphStats> {
        let stats = GraphStats::default();
        Ok(stats)
    }

    pub async fn scan_workspace(&self, workspace_root: &Path) -> Result<ScanProgress> {
        tracing::info!("Scanning workspace at: {}", workspace_root.display());

        let mut progress = ScanProgress::default();

        let cargo_toml = workspace_root.join("Cargo.toml");
        if !cargo_toml.exists() {
            anyhow::bail!("No Cargo.toml found at workspace root");
        }

        let content = tokio::fs::read_to_string(&cargo_toml).await?;
        let toml_value: toml::Value = toml::from_str(&content)?;

        if let Some(workspace) = toml_value.get("workspace") {
            if let Some(members) = workspace.get("members").and_then(|m| m.as_array()) {
                progress.total_projects = members.len() as i32;

                for member in members {
                    if let Some(member_path) = member.as_str() {
                        let full_path = workspace_root.join(member_path);
                        if let Err(e) = self.scan_project(&full_path, member_path).await {
                            tracing::warn!("Failed to scan project {}: {}", member_path, e);
                        } else {
                            progress.projects_scanned += 1;
                        }
                    }
                }
            }
        }

        Ok(progress)
    }

    async fn scan_project(&self, project_path: &Path, project_name: &str) -> Result<()> {
        tracing::info!("Scanning project: {}", project_name);

        let cargo_toml = project_path.join("Cargo.toml");
        if cargo_toml.exists() {
            let content = tokio::fs::read_to_string(&cargo_toml).await?;
            let toml_value: toml::Value = toml::from_str(&content)?;

            let package_name = toml_value
                .get("package")
                .and_then(|p| p.get("name"))
                .and_then(|n| n.as_str())
                .unwrap_or(project_name);

            self.create_node(&CodeNode::Project {
                name: package_name.to_string(),
                path: project_name.to_string(),
                node_type: "rust".to_string(),
            })
            .await?;
        }

        for entry in WalkDir::new(project_path)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("rs") {
                if let Err(e) = self.scan_rust_file(path, project_name).await {
                    tracing::warn!("Failed to scan {}: {}", path.display(), e);
                }
            }
        }

        Ok(())
    }

    async fn scan_rust_file(&self, file_path: &Path, project_name: &str) -> Result<()> {
        let relative_path = file_path.to_string_lossy().to_string();

        let (nodes, relationships) = {
            let content = tokio::fs::read_to_string(file_path).await?;
            let relative_path_clone = relative_path.clone();
            tokio::task::spawn_blocking(move || {
                let syntax_tree = syn::parse_file(&content)?;
                let mut visitor = CodeVisitor::new(relative_path_clone);
                visitor.visit_file(&syntax_tree);
                Ok::<_, anyhow::Error>((visitor.nodes().to_vec(), visitor.relationships().to_vec()))
            })
            .await??
        };

        self.create_node(&CodeNode::File {
            path: relative_path.clone(),
            project: project_name.to_string(),
        })
        .await?;

        self.create_relationship(&Relationship {
            from: relative_path.clone(),
            to: project_name.to_string(),
            rel_type: "IN_PROJECT".to_string(),
        })
        .await?;

        use futures::future::join_all;

        const BATCH_SIZE: usize = 50;

        for chunk in nodes.chunks(BATCH_SIZE) {
            let futures: Vec<_> = chunk.iter().map(|node| self.create_node(node)).collect();
            let results = join_all(futures).await;
            for result in results {
                if let Err(e) = result {
                    tracing::warn!("Failed to create node: {}", e);
                }
            }
        }

        for chunk in relationships.chunks(BATCH_SIZE) {
            let futures: Vec<_> = chunk
                .iter()
                .map(|rel| self.create_relationship(rel))
                .collect();
            let results = join_all(futures).await;
            for result in results {
                if let Err(e) = result {
                    tracing::warn!("Failed to create relationship: {}", e);
                }
            }
        }

        Ok(())
    }

    async fn create_node(&self, node: &CodeNode) -> Result<()> {
        let query = match node {
            CodeNode::Project {
                name,
                path,
                node_type,
            } => Query::new("MERGE (p:Project {name: $name, path: $path, type: $type})".into())
                .param("name", name.as_str())
                .param("path", path.as_str())
                .param("type", node_type.as_str()),

            CodeNode::File { path, project } => {
                Query::new("MERGE (f:File {path: $path, project: $project})".into())
                    .param("path", path.as_str())
                    .param("project", project.as_str())
            }

            CodeNode::Function {
                name,
                file,
                visibility,
                is_async,
            } => Query::new(
                "MERGE (f:Function {name: $name, file: $file, visibility: $vis, async: $async})"
                    .into(),
            )
            .param("name", name.as_str())
            .param("file", file.as_str())
            .param("vis", visibility.as_str())
            .param("async", *is_async),

            CodeNode::Struct {
                name,
                file,
                visibility,
            } => Query::new("MERGE (s:Struct {name: $name, file: $file, visibility: $vis})".into())
                .param("name", name.as_str())
                .param("file", file.as_str())
                .param("vis", visibility.as_str()),

            CodeNode::Trait {
                name,
                file,
                visibility,
            } => Query::new("MERGE (t:Trait {name: $name, file: $file, visibility: $vis})".into())
                .param("name", name.as_str())
                .param("file", file.as_str())
                .param("vis", visibility.as_str()),

            CodeNode::Module { name, file } => {
                Query::new("MERGE (m:Module {name: $name, file: $file})".into())
                    .param("name", name.as_str())
                    .param("file", file.as_str())
            }

            CodeNode::Impl {
                target,
                trait_name,
                file,
            } => {
                let mut q = Query::new("MERGE (i:Impl {target: $target, file: $file})".into())
                    .param("target", target.as_str())
                    .param("file", file.as_str());

                if let Some(trait_impl) = trait_name {
                    q = q.param("trait", trait_impl.as_str());
                }
                q
            }
        };

        self.graph.run(query).await?;
        Ok(())
    }

    async fn create_relationship(&self, rel: &Relationship) -> Result<()> {
        let query = match rel.rel_type.as_str() {
            "DEFINED_IN" => Query::new(
                "MATCH (n {name: $from})
                 MATCH (f:File {path: $to})
                 MERGE (n)-[:DEFINED_IN]->(f)"
                    .into(),
            )
            .param("from", rel.from.as_str())
            .param("to", rel.to.as_str()),

            "IN_PROJECT" => Query::new(
                "MATCH (f:File {path: $from})
                 MATCH (p:Project {path: $to})
                 MERGE (f)-[:IN_PROJECT]->(p)"
                    .into(),
            )
            .param("from", rel.from.as_str())
            .param("to", rel.to.as_str()),

            _ => Query::new(format!(
                "MATCH (a {{name: $from}})
                 MATCH (b {{name: $to}})
                 MERGE (a)-[:{}]->(b)",
                rel.rel_type
            ))
            .param("from", rel.from.as_str())
            .param("to", rel.to.as_str()),
        };

        self.graph.run(query).await?;
        Ok(())
    }
}
