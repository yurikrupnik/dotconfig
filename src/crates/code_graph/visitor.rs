use std::sync::Arc;
use syn::{visit::Visit, Item};
use super::types::{CodeNode, Relationship};

pub struct CodeVisitor {
    file_path: Arc<str>,
    nodes: Vec<CodeNode>,
    relationships: Vec<Relationship>,
}

impl CodeVisitor {
    pub fn new(file_path: String) -> Self {
        Self {
            file_path: Arc::from(file_path.as_str()),
            nodes: Vec::new(),
            relationships: Vec::new(),
        }
    }

    pub fn nodes(&self) -> &[CodeNode] {
        &self.nodes
    }

    pub fn relationships(&self) -> &[Relationship] {
        &self.relationships
    }
}

impl<'ast> Visit<'ast> for CodeVisitor {
    fn visit_item(&mut self, item: &'ast Item) {
        match item {
            Item::Fn(func) => {
                let name = func.sig.ident.to_string();
                let visibility = match &func.vis {
                    syn::Visibility::Public(_) => "pub",
                    _ => "private",
                }
                .to_string();
                let is_async = func.sig.asyncness.is_some();

                self.nodes.push(CodeNode::Function {
                    name: name.clone(),
                    file: self.file_path.to_string(),
                    visibility,
                    is_async,
                });

                self.relationships.push(Relationship {
                    from: name,
                    to: self.file_path.to_string(),
                    rel_type: "DEFINED_IN".to_string(),
                });
            }
            Item::Struct(s) => {
                let name = s.ident.to_string();
                let visibility = match &s.vis {
                    syn::Visibility::Public(_) => "pub",
                    _ => "private",
                }
                .to_string();

                self.nodes.push(CodeNode::Struct {
                    name: name.clone(),
                    file: self.file_path.to_string(),
                    visibility,
                });

                self.relationships.push(Relationship {
                    from: name,
                    to: self.file_path.to_string(),
                    rel_type: "DEFINED_IN".to_string(),
                });
            }
            Item::Trait(t) => {
                let name = t.ident.to_string();
                let visibility = match &t.vis {
                    syn::Visibility::Public(_) => "pub",
                    _ => "private",
                }
                .to_string();

                self.nodes.push(CodeNode::Trait {
                    name: name.clone(),
                    file: self.file_path.to_string(),
                    visibility,
                });

                self.relationships.push(Relationship {
                    from: name,
                    to: self.file_path.to_string(),
                    rel_type: "DEFINED_IN".to_string(),
                });
            }
            Item::Mod(m) => {
                let name = m.ident.to_string();

                self.nodes.push(CodeNode::Module {
                    name: name.clone(),
                    file: self.file_path.to_string(),
                });

                self.relationships.push(Relationship {
                    from: name,
                    to: self.file_path.to_string(),
                    rel_type: "DEFINED_IN".to_string(),
                });
            }
            Item::Impl(i) => {
                let target = if let syn::Type::Path(ref p) = *i.self_ty {
                    p.path
                        .segments
                        .last()
                        .map(|s| s.ident.to_string())
                        .unwrap_or_else(|| "Unknown".to_string())
                } else {
                    "Unknown".to_string()
                };

                let trait_name = i.trait_.as_ref().map(|(_, path, _)| {
                    path.segments
                        .last()
                        .map(|s| s.ident.to_string())
                        .unwrap_or_else(|| "Unknown".to_string())
                });

                self.nodes.push(CodeNode::Impl {
                    target: target.clone(),
                    trait_name: trait_name.clone(),
                    file: self.file_path.to_string(),
                });

                self.relationships.push(Relationship {
                    from: target,
                    to: self.file_path.to_string(),
                    rel_type: "IMPLEMENTED_IN".to_string(),
                });

                if let Some(trait_impl) = trait_name {
                    self.relationships.push(Relationship {
                        from: format!("{}_impl", trait_impl),
                        to: trait_impl,
                        rel_type: "IMPLEMENTS".to_string(),
                    });
                }
            }
            _ => {}
        }

        syn::visit::visit_item(self, item);
    }
}
