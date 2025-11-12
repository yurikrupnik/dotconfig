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

#[cfg(test)]
mod tests {
    use super::*;
    use syn::parse_quote;

    #[test]
    fn test_visitor_new() {
        let visitor = CodeVisitor::new("test.rs".into());

        assert_eq!(visitor.nodes().len(), 0);
        assert_eq!(visitor.relationships().len(), 0);
    }

    #[test]
    fn test_visitor_function_public() {
        let mut visitor = CodeVisitor::new("test.rs".into());
        let item: Item = parse_quote! {
            pub fn test_func() {}
        };

        visitor.visit_item(&item);

        assert_eq!(visitor.nodes().len(), 1);
        assert_eq!(visitor.relationships().len(), 1);

        match &visitor.nodes()[0] {
            CodeNode::Function {
                name,
                file,
                visibility,
                is_async,
            } => {
                assert_eq!(name, "test_func");
                assert_eq!(file, "test.rs");
                assert_eq!(visibility, "pub");
                assert!(!is_async);
            }
            _ => panic!("Expected Function node"),
        }
    }

    #[test]
    fn test_visitor_function_private() {
        let mut visitor = CodeVisitor::new("test.rs".into());
        let item: Item = parse_quote! {
            fn private_func() {}
        };

        visitor.visit_item(&item);

        match &visitor.nodes()[0] {
            CodeNode::Function { visibility, .. } => {
                assert_eq!(visibility, "private");
            }
            _ => panic!("Expected Function node"),
        }
    }

    #[test]
    fn test_visitor_async_function() {
        let mut visitor = CodeVisitor::new("test.rs".into());
        let item: Item = parse_quote! {
            pub async fn async_func() {}
        };

        visitor.visit_item(&item);

        match &visitor.nodes()[0] {
            CodeNode::Function { is_async, .. } => {
                assert!(is_async);
            }
            _ => panic!("Expected Function node"),
        }
    }

    #[test]
    fn test_visitor_struct() {
        let mut visitor = CodeVisitor::new("test.rs".into());
        let item: Item = parse_quote! {
            pub struct TestStruct {
                field: String,
            }
        };

        visitor.visit_item(&item);

        assert_eq!(visitor.nodes().len(), 1);
        match &visitor.nodes()[0] {
            CodeNode::Struct {
                name,
                file,
                visibility,
            } => {
                assert_eq!(name, "TestStruct");
                assert_eq!(file, "test.rs");
                assert_eq!(visibility, "pub");
            }
            _ => panic!("Expected Struct node"),
        }
    }

    #[test]
    fn test_visitor_trait() {
        let mut visitor = CodeVisitor::new("test.rs".into());
        let item: Item = parse_quote! {
            pub trait TestTrait {
                fn method(&self);
            }
        };

        visitor.visit_item(&item);

        assert_eq!(visitor.nodes().len(), 1);
        match &visitor.nodes()[0] {
            CodeNode::Trait {
                name,
                file,
                visibility,
            } => {
                assert_eq!(name, "TestTrait");
                assert_eq!(file, "test.rs");
                assert_eq!(visibility, "pub");
            }
            _ => panic!("Expected Trait node"),
        }
    }

    #[test]
    fn test_visitor_module() {
        let mut visitor = CodeVisitor::new("test.rs".into());
        let item: Item = parse_quote! {
            mod test_module {}
        };

        visitor.visit_item(&item);

        assert_eq!(visitor.nodes().len(), 1);
        match &visitor.nodes()[0] {
            CodeNode::Module { name, file } => {
                assert_eq!(name, "test_module");
                assert_eq!(file, "test.rs");
            }
            _ => panic!("Expected Module node"),
        }
    }

    #[test]
    fn test_visitor_impl_block() {
        let mut visitor = CodeVisitor::new("test.rs".into());
        let item: Item = parse_quote! {
            impl MyStruct {
                fn method(&self) {}
            }
        };

        visitor.visit_item(&item);

        assert_eq!(visitor.nodes().len(), 1);
        match &visitor.nodes()[0] {
            CodeNode::Impl {
                target,
                trait_name,
                file,
            } => {
                assert_eq!(target, "MyStruct");
                assert!(trait_name.is_none());
                assert_eq!(file, "test.rs");
            }
            _ => panic!("Expected Impl node"),
        }
    }

    #[test]
    fn test_visitor_trait_impl() {
        let mut visitor = CodeVisitor::new("test.rs".into());
        let item: Item = parse_quote! {
            impl Display for MyStruct {
                fn fmt(&self, f: &mut Formatter) -> Result {
                    write!(f, "MyStruct")
                }
            }
        };

        visitor.visit_item(&item);

        match &visitor.nodes()[0] {
            CodeNode::Impl {
                target,
                trait_name,
                ..
            } => {
                assert_eq!(target, "MyStruct");
                assert_eq!(trait_name, &Some("Display".into()));
            }
            _ => panic!("Expected Impl node"),
        }

        assert!(visitor.relationships().len() >= 2);
    }

    #[test]
    fn test_visitor_multiple_items() {
        let mut visitor = CodeVisitor::new("test.rs".into());

        let items: Vec<Item> = vec![
            parse_quote! { pub fn func1() {} },
            parse_quote! { pub fn func2() {} },
            parse_quote! { pub struct MyStruct {} },
        ];

        for item in items {
            visitor.visit_item(&item);
        }

        assert_eq!(visitor.nodes().len(), 3);
        assert_eq!(visitor.relationships().len(), 3);
    }

    #[test]
    fn test_visitor_relationship_defined_in() {
        let mut visitor = CodeVisitor::new("test.rs".into());
        let item: Item = parse_quote! {
            pub fn test_func() {}
        };

        visitor.visit_item(&item);

        let rel = &visitor.relationships()[0];
        assert_eq!(rel.from, "test_func");
        assert_eq!(rel.to, "test.rs");
        assert_eq!(rel.rel_type, "DEFINED_IN");
    }
}
