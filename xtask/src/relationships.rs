use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use syn::visit::Visit;
use syn::{Expr, ExprCall, ExprMethodCall, ImplItem, ItemFn, ItemImpl};

/// Complete code relationship data extracted from source files
#[derive(Debug, Default)]
pub struct CodeRelationships {
    /// Function → Set of functions it calls (forward dependencies)
    pub call_graph: HashMap<String, HashSet<String>>,

    /// Function → Set of functions that call it (reverse dependencies)
    pub usage_graph: HashMap<String, HashSet<String>>,

    /// Type/Trait → Implementation details
    pub inheritance: HashMap<String, InheritanceInfo>,

    /// Function → Complete metadata
    pub functions: HashMap<String, FunctionMetadata>,
}

/// Information about trait implementations and inheritance
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct InheritanceInfo {
    pub trait_name: Option<String>,
    pub type_name: String,
    pub methods: Vec<String>,
    pub bounds: Vec<String>,
    pub parent_traits: Vec<String>,
}

/// Complete metadata about a function
#[derive(Debug, Clone)]
pub struct FunctionMetadata {
    pub name: String,
    pub fully_qualified_name: String,
    pub is_method: bool,
    pub is_public: bool,
    #[allow(dead_code)]
    pub parent_type: Option<String>,
    #[allow(dead_code)]
    pub parent_trait: Option<String>,
    pub file_path: PathBuf,
}

/// Context for tracking scope during AST traversal
#[derive(Debug, Default)]
struct Context {
    current_function: Option<String>,
    current_type: Option<String>,
    current_trait: Option<String>,
    current_impl_trait: Option<String>,
    module_path: Vec<String>,
}

impl Context {
    fn qualified_name(&self, name: &str) -> String {
        let mut parts = self.module_path.clone();

        if let Some(type_name) = &self.current_type {
            parts.push(type_name.clone());
        }

        parts.push(name.to_string());
        parts.join("::")
    }
}

/// AST visitor for extracting relationships
struct RelationshipVisitor<'a> {
    context: Context,
    call_graph: &'a mut HashMap<String, HashSet<String>>,
    inheritance: &'a mut HashMap<String, InheritanceInfo>,
    functions: &'a mut HashMap<String, FunctionMetadata>,
    file_path: PathBuf,
}

impl<'a> RelationshipVisitor<'a> {
    fn new(
        call_graph: &'a mut HashMap<String, HashSet<String>>,
        inheritance: &'a mut HashMap<String, InheritanceInfo>,
        functions: &'a mut HashMap<String, FunctionMetadata>,
        file_path: PathBuf,
    ) -> Self {
        RelationshipVisitor {
            context: Context::default(),
            call_graph,
            inheritance,
            functions,
            file_path,
        }
    }

    fn add_call(&mut self, caller: &str, callee: &str) {
        self.call_graph
            .entry(caller.to_string())
            .or_insert_with(HashSet::new)
            .insert(callee.to_string());
    }

    fn extract_path_name(path: &syn::Path) -> String {
        path.segments
            .iter()
            .map(|s| s.ident.to_string())
            .collect::<Vec<_>>()
            .join("::")
    }
}

impl<'a> Visit<'a> for RelationshipVisitor<'a> {
    fn visit_item_fn(&mut self, node: &'a ItemFn) {
        let fn_name = node.sig.ident.to_string();
        let qualified_name = self.context.qualified_name(&fn_name);

        // Store function metadata
        self.functions.insert(
            qualified_name.clone(),
            FunctionMetadata {
                name: fn_name.clone(),
                fully_qualified_name: qualified_name.clone(),
                is_method: false,
                is_public: matches!(node.vis, syn::Visibility::Public(_)),
                parent_type: self.context.current_type.clone(),
                parent_trait: self.context.current_trait.clone(),
                file_path: self.file_path.clone(),
            },
        );

        // Set current function context
        let prev_function = self.context.current_function.clone();
        self.context.current_function = Some(qualified_name.clone());

        // Visit function body
        syn::visit::visit_item_fn(self, node);

        // Restore previous context
        self.context.current_function = prev_function;
    }

    fn visit_item_impl(&mut self, node: &'a ItemImpl) {
        // Extract type name
        let type_name = if let syn::Type::Path(type_path) = &*node.self_ty {
            Self::extract_path_name(&type_path.path)
        } else {
            "Unknown".to_string()
        };

        // Extract trait name if this is a trait impl
        let trait_name = node.trait_.as_ref().map(|(_, path, _)| Self::extract_path_name(path));

        // Set context
        let prev_type = self.context.current_type.clone();
        let prev_impl_trait = self.context.current_impl_trait.clone();
        self.context.current_type = Some(type_name.clone());
        self.context.current_impl_trait = trait_name.clone();

        // Collect methods
        let mut methods = Vec::new();
        for item in &node.items {
            if let ImplItem::Fn(method) = item {
                methods.push(method.sig.ident.to_string());
            }
        }

        // Store inheritance info
        let key = if let Some(ref t) = trait_name {
            format!("{}::{}", type_name, t)
        } else {
            type_name.clone()
        };

        self.inheritance.insert(
            key,
            InheritanceInfo {
                trait_name,
                type_name,
                methods,
                bounds: Vec::new(),
                parent_traits: Vec::new(),
            },
        );

        // Visit impl items
        syn::visit::visit_item_impl(self, node);

        // Restore context
        self.context.current_type = prev_type;
        self.context.current_impl_trait = prev_impl_trait;
    }

    fn visit_impl_item_fn(&mut self, node: &'a syn::ImplItemFn) {
        let fn_name = node.sig.ident.to_string();
        let qualified_name = self.context.qualified_name(&fn_name);

        // Store function metadata
        self.functions.insert(
            qualified_name.clone(),
            FunctionMetadata {
                name: fn_name.clone(),
                fully_qualified_name: qualified_name.clone(),
                is_method: true,
                is_public: matches!(node.vis, syn::Visibility::Public(_)),
                parent_type: self.context.current_type.clone(),
                parent_trait: self.context.current_impl_trait.clone(),
                file_path: self.file_path.clone(),
            },
        );

        // Set current function context
        let prev_function = self.context.current_function.clone();
        self.context.current_function = Some(qualified_name.clone());

        // Visit method body
        syn::visit::visit_impl_item_fn(self, node);

        // Restore previous context
        self.context.current_function = prev_function;
    }

    fn visit_expr(&mut self, node: &'a Expr) {
        if let Some(current_fn) = self.context.current_function.clone() {
            match node {
                // Method calls: obj.method()
                Expr::MethodCall(ExprMethodCall { method, .. }) => {
                    let callee = method.to_string();
                    self.add_call(&current_fn, &callee);
                }
                // Direct function calls: foo()
                Expr::Call(ExprCall { func, .. }) => {
                    if let Expr::Path(expr_path) = &**func {
                        let callee = Self::extract_path_name(&expr_path.path);
                        self.add_call(&current_fn, &callee);
                    }
                }
                _ => {}
            }
        }

        // Continue visiting
        syn::visit::visit_expr(self, node);
    }
}

/// Extract relationships from Rust source files
pub fn extract_relationships(source_files: Vec<PathBuf>) -> CodeRelationships {
    let mut call_graph = HashMap::new();
    let mut inheritance = HashMap::new();
    let mut functions = HashMap::new();

    for file_path in source_files {
        if let Ok(content) = std::fs::read_to_string(&file_path) {
            if let Ok(ast) = syn::parse_file(&content) {
                let mut visitor = RelationshipVisitor::new(
                    &mut call_graph,
                    &mut inheritance,
                    &mut functions,
                    file_path.clone(),
                );
                visitor.visit_file(&ast);
            }
        }
    }

    // Build usage graph (reverse call graph)
    let mut usage_graph: HashMap<String, HashSet<String>> = HashMap::new();
    for (caller, callees) in &call_graph {
        for callee in callees {
            usage_graph
                .entry(callee.clone())
                .or_insert_with(HashSet::new)
                .insert(caller.clone());
        }
    }

    CodeRelationships {
        call_graph,
        usage_graph,
        inheritance,
        functions,
    }
}

/// Generate a simple SVG call graph for a specific function
pub fn generate_function_call_graph(
    function_name: &str,
    relationships: &CodeRelationships,
) -> Option<String> {
    // Check if function exists
    if !relationships.functions.contains_key(function_name) {
        return None;
    }

    let callees = relationships
        .call_graph
        .get(function_name)
        .map(|set| set.iter().cloned().collect::<Vec<_>>())
        .unwrap_or_default();

    let callers = relationships
        .usage_graph
        .get(function_name)
        .map(|set| set.iter().cloned().collect::<Vec<_>>())
        .unwrap_or_default();

    if callees.is_empty() && callers.is_empty() {
        return None;
    }

    // Simple vertical layout
    let width = 800;
    let height = 200 + (callees.len().max(callers.len()) * 40);
    let center_x = width / 2;
    let center_y = height / 2;

    let mut svg = format!(
        "<svg width=\"{}\" height=\"{}\" xmlns=\"http://www.w3.org/2000/svg\">\n  \
  <style>\n    \
    .node {{ fill: rgb(76, 175, 80); stroke: rgb(46, 125, 50); stroke-width: 2; }}\n    \
    .current {{ fill: rgb(33, 150, 243); stroke: rgb(21, 101, 192); stroke-width: 3; }}\n    \
    .caller {{ fill: rgb(255, 193, 7); stroke: rgb(245, 124, 0); stroke-width: 2; }}\n    \
    .edge {{ stroke: rgb(102, 102, 102); stroke-width: 2; marker-end: url(#arrowhead); }}\n    \
    .caller-edge {{ stroke: rgb(245, 124, 0); stroke-width: 2; marker-end: url(#arrowhead); }}\n    \
    .text {{ fill: white; font-family: monospace; font-size: 12px; text-anchor: middle; }}\n  \
  </style>\n  \
  <defs>\n    \
    <marker id=\"arrowhead\" markerWidth=\"10\" markerHeight=\"10\" refX=\"9\" refY=\"3\" orient=\"auto\">\n      \
      <polygon points=\"0 0, 10 3, 0 6\" fill=\"rgb(102, 102, 102)\" />\n    \
    </marker>\n  \
  </defs>\n",
        width, height
    );

    // Draw edges from callers to current function
    for (i, _caller) in callers.iter().enumerate() {
        let y = 50 + i * 40;
        svg.push_str(&format!(
            "  <line x1=\"150\" y1=\"{}\" x2=\"{}\" y2=\"{}\" class=\"caller-edge\" />\n",
            y + 15,
            center_x - 120,
            center_y + 15
        ));
    }

    // Draw edges from current function to callees
    for (i, _callee) in callees.iter().enumerate() {
        let y = 50 + i * 40;
        svg.push_str(&format!(
            "  <line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" class=\"edge\" />\n",
            center_x + 120,
            center_y + 15,
            width - 150,
            y + 15
        ));
    }

    // Draw caller nodes
    for (i, caller) in callers.iter().enumerate() {
        let y = 50 + i * 40;
        let label = caller.split("::").last().unwrap_or(caller);
        svg.push_str(&format!(
            "  <rect x=\"20\" y=\"{}\" width=\"260\" height=\"30\" rx=\"5\" class=\"caller\" />\n  \
  <text x=\"150\" y=\"{}\" class=\"text\">{}</text>\n",
            y,
            y + 20,
            label
        ));
    }

    // Draw current function node
    let label = function_name.split("::").last().unwrap_or(function_name);
    svg.push_str(&format!(
        "  <rect x=\"{}\" y=\"{}\" width=\"240\" height=\"30\" rx=\"5\" class=\"current\" />\n  \
  <text x=\"{}\" y=\"{}\" class=\"text\">{}</text>\n",
        center_x - 120,
        center_y,
        center_x,
        center_y + 20,
        label
    ));

    // Draw callee nodes
    for (i, callee) in callees.iter().enumerate() {
        let y = 50 + i * 40;
        let label = callee.split("::").last().unwrap_or(callee);
        svg.push_str(&format!(
            "  <rect x=\"{}\" y=\"{}\" width=\"260\" height=\"30\" rx=\"5\" class=\"node\" />\n  \
  <text x=\"{}\" y=\"{}\" class=\"text\">{}</text>\n",
            width - 280,
            y,
            width - 150,
            y + 20,
            label
        ));
    }

    svg.push_str("</svg>");

    Some(svg)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_and_extract(code: &str) -> CodeRelationships {
        let ast = syn::parse_file(code).expect("Failed to parse code");
        let mut call_graph = HashMap::new();
        let mut inheritance = HashMap::new();
        let mut functions = HashMap::new();

        let mut visitor = RelationshipVisitor::new(
            &mut call_graph,
            &mut inheritance,
            &mut functions,
            PathBuf::from("test.rs"),
        );
        visitor.visit_file(&ast);

        // Build usage graph
        let mut usage_graph = HashMap::new();
        for (caller, callees) in &call_graph {
            for callee in callees {
                usage_graph
                    .entry(callee.clone())
                    .or_insert_with(HashSet::new)
                    .insert(caller.clone());
            }
        }

        CodeRelationships {
            call_graph,
            usage_graph,
            inheritance,
            functions,
        }
    }

    #[test]
    fn test_simple_function_call() {
        let code = r#"
            fn foo() {
                bar();
            }
            fn bar() {}
        "#;

        let rels = parse_and_extract(code);

        assert!(rels.call_graph.contains_key("foo"));
        assert!(rels.call_graph["foo"].contains("bar"));
        assert_eq!(rels.functions.len(), 2);
    }

    #[test]
    fn test_method_call() {
        let code = r#"
            fn foo() {
                let s = String::new();
                s.len();
            }
        "#;

        let rels = parse_and_extract(code);

        assert!(rels.call_graph.contains_key("foo"));
        assert!(rels.call_graph["foo"].contains("len"));
        assert!(rels.call_graph["foo"].contains("String::new"));
    }

    #[test]
    fn test_usage_graph() {
        let code = r#"
            fn caller1() { target(); }
            fn caller2() { target(); }
            fn target() {}
        "#;

        let rels = parse_and_extract(code);

        assert!(rels.usage_graph.contains_key("target"));
        assert!(rels.usage_graph["target"].contains("caller1"));
        assert!(rels.usage_graph["target"].contains("caller2"));
        assert_eq!(rels.usage_graph["target"].len(), 2);
    }

    #[test]
    fn test_no_calls() {
        let code = r#"
            fn standalone() {
                let x = 42;
            }
        "#;

        let rels = parse_and_extract(code);

        assert!(!rels.call_graph.contains_key("standalone"));
        assert_eq!(rels.functions.len(), 1);
    }

    #[test]
    fn test_impl_methods() {
        let code = r#"
            struct MyStruct;
            impl MyStruct {
                fn method(&self) {
                    helper();
                }
            }
            fn helper() {}
        "#;

        let rels = parse_and_extract(code);

        assert!(rels.functions.contains_key("MyStruct::method"));
        assert!(rels.call_graph.contains_key("MyStruct::method"));
        assert!(rels.call_graph["MyStruct::method"].contains("helper"));
    }

    #[test]
    fn test_public_private_functions() {
        let code = r#"
            pub fn public_fn() {}
            fn private_fn() {}
        "#;

        let rels = parse_and_extract(code);

        assert!(rels.functions["public_fn"].is_public);
        assert!(!rels.functions["private_fn"].is_public);
    }

    #[test]
    fn test_function_metadata() {
        let code = r#"
            pub fn my_function() {}
        "#;

        let rels = parse_and_extract(code);

        let metadata = &rels.functions["my_function"];
        assert_eq!(metadata.name, "my_function");
        assert_eq!(metadata.fully_qualified_name, "my_function");
        assert!(!metadata.is_method);
        assert!(metadata.is_public);
        assert_eq!(metadata.file_path, PathBuf::from("test.rs"));
    }

    #[test]
    fn test_generate_call_graph_with_calls() {
        let code = r#"
            fn foo() { bar(); }
            fn bar() {}
        "#;

        let rels = parse_and_extract(code);
        let svg = generate_function_call_graph("foo", &rels);

        assert!(svg.is_some());
        let svg_content = svg.unwrap();
        assert!(svg_content.contains("<svg"));
        assert!(svg_content.contains("foo"));
        assert!(svg_content.contains("bar"));
    }

    #[test]
    fn test_generate_call_graph_no_calls() {
        let code = r#"
            fn standalone() {
                let x = 42;
            }
        "#;

        let rels = parse_and_extract(code);
        let svg = generate_function_call_graph("standalone", &rels);

        assert!(svg.is_none());
    }

    #[test]
    fn test_generate_call_graph_nonexistent_function() {
        let code = r#"
            fn foo() {}
        "#;

        let rels = parse_and_extract(code);
        let svg = generate_function_call_graph("nonexistent", &rels);

        assert!(svg.is_none());
    }

    #[test]
    fn test_multiple_calls() {
        let code = r#"
            fn foo() {
                bar();
                baz();
                qux();
            }
            fn bar() {}
            fn baz() {}
            fn qux() {}
        "#;

        let rels = parse_and_extract(code);

        assert_eq!(rels.call_graph["foo"].len(), 3);
        assert!(rels.call_graph["foo"].contains("bar"));
        assert!(rels.call_graph["foo"].contains("baz"));
        assert!(rels.call_graph["foo"].contains("qux"));
    }

    #[test]
    fn test_call_chain() {
        let code = r#"
            fn a() { b(); }
            fn b() { c(); }
            fn c() {}
        "#;

        let rels = parse_and_extract(code);

        assert!(rels.call_graph["a"].contains("b"));
        assert!(rels.call_graph["b"].contains("c"));
        assert!(rels.usage_graph["b"].contains("a"));
        assert!(rels.usage_graph["c"].contains("b"));
    }

    #[test]
    fn test_trait_impl() {
        let code = r#"
            trait MyTrait {
                fn trait_method(&self);
            }

            struct MyStruct;

            impl MyTrait for MyStruct {
                fn trait_method(&self) {
                    helper();
                }
            }

            fn helper() {}
        "#;

        let rels = parse_and_extract(code);

        assert!(rels.functions.contains_key("MyStruct::trait_method"));
        assert!(rels.inheritance.contains_key("MyStruct::MyTrait"));
    }

    #[test]
    fn test_nested_calls() {
        let code = r#"
            fn outer() {
                inner(middle());
            }
            fn middle() {}
            fn inner(x: ()) {}
        "#;

        let rels = parse_and_extract(code);

        assert!(rels.call_graph["outer"].contains("middle"));
        assert!(rels.call_graph["outer"].contains("inner"));
        assert_eq!(rels.call_graph["outer"].len(), 2);
    }

    #[test]
    fn test_self_recursion() {
        let code = r#"
            fn recursive(n: i32) {
                if n > 0 {
                    recursive(n - 1);
                }
            }
        "#;

        let rels = parse_and_extract(code);

        assert!(rels.call_graph.contains_key("recursive"));
        assert!(rels.call_graph["recursive"].contains("recursive"));
        assert!(rels.usage_graph["recursive"].contains("recursive"));
    }

    #[test]
    fn test_mutual_recursion() {
        let code = r#"
            fn foo(n: i32) {
                if n > 0 { bar(n - 1); }
            }
            fn bar(n: i32) {
                if n > 0 { foo(n - 1); }
            }
        "#;

        let rels = parse_and_extract(code);

        assert!(rels.call_graph["foo"].contains("bar"));
        assert!(rels.call_graph["bar"].contains("foo"));
    }

    #[test]
    fn test_qualified_path_call() {
        let code = r#"
            fn test() {
                std::mem::drop(42);
            }
        "#;

        let rels = parse_and_extract(code);

        assert!(rels.call_graph.contains_key("test"));
        assert!(rels.call_graph["test"].contains("std::mem::drop"));
    }

    #[test]
    fn test_method_on_type() {
        let code = r#"
            struct Foo;
            impl Foo {
                fn new() -> Self { Foo }
                fn method(&self) {
                    Self::new();
                }
            }
        "#;

        let rels = parse_and_extract(code);

        assert!(rels.functions.contains_key("Foo::new"));
        assert!(rels.functions.contains_key("Foo::method"));
        assert!(rels.call_graph["Foo::method"].contains("Self::new"));
    }

    #[test]
    fn test_closure_calls() {
        let code = r#"
            fn outer() {
                let closure = || {
                    inner();
                };
                closure();
            }
            fn inner() {}
        "#;

        let rels = parse_and_extract(code);

        // Closures are captured within the outer function's scope
        assert!(rels.call_graph.contains_key("outer"));
        assert!(rels.call_graph["outer"].contains("inner"));
    }

    #[test]
    fn test_generic_function() {
        let code = r#"
            fn generic<T>(x: T) {
                helper();
            }
            fn helper() {}
        "#;

        let rels = parse_and_extract(code);

        assert!(rels.functions.contains_key("generic"));
        assert!(rels.call_graph["generic"].contains("helper"));
    }

    #[test]
    fn test_async_function() {
        let code = r#"
            async fn async_fn() {
                other().await;
            }
            async fn other() {}
        "#;

        let rels = parse_and_extract(code);

        assert!(rels.functions.contains_key("async_fn"));
        assert!(rels.call_graph["async_fn"].contains("other"));
    }

    #[test]
    fn test_const_function() {
        let code = r#"
            const fn const_fn() {
                helper();
            }
            const fn helper() {}
        "#;

        let rels = parse_and_extract(code);

        assert!(rels.functions.contains_key("const_fn"));
        assert!(rels.call_graph["const_fn"].contains("helper"));
    }

    #[test]
    fn test_multiple_impls_same_type() {
        let code = r#"
            struct Foo;

            impl Foo {
                fn method1(&self) {}
            }

            impl Foo {
                fn method2(&self) {}
            }
        "#;

        let rels = parse_and_extract(code);

        assert!(rels.functions.contains_key("Foo::method1"));
        assert!(rels.functions.contains_key("Foo::method2"));
    }

    #[test]
    fn test_empty_function() {
        let code = r#"
            fn empty() {}
        "#;

        let rels = parse_and_extract(code);

        assert_eq!(rels.functions.len(), 1);
        assert!(!rels.call_graph.contains_key("empty"));
        assert!(!rels.usage_graph.contains_key("empty"));
    }

    #[test]
    fn test_call_graph_with_callers_and_callees() {
        let code = r#"
            fn caller1() { middle(); }
            fn caller2() { middle(); }
            fn middle() { callee(); }
            fn callee() {}
        "#;

        let rels = parse_and_extract(code);
        let svg = generate_function_call_graph("middle", &rels);

        assert!(svg.is_some());
        let svg_content = svg.unwrap();

        // Should contain both callers and callees
        assert!(svg_content.contains("caller1") || svg_content.contains("caller2"));
        assert!(svg_content.contains("callee"));
        assert!(svg_content.contains("middle"));

        // Should have caller-edge class for incoming edges
        assert!(svg_content.contains("caller-edge"));

        // Should have regular edge class for outgoing edges
        assert!(svg_content.contains("class=\"edge\""));
    }

    #[test]
    fn test_function_with_only_callers() {
        let code = r#"
            fn caller1() { target(); }
            fn caller2() { target(); }
            fn target() {}
        "#;

        let rels = parse_and_extract(code);
        let svg = generate_function_call_graph("target", &rels);

        assert!(svg.is_some());
        let svg_content = svg.unwrap();
        assert!(svg_content.contains("caller"));
        assert!(svg_content.contains("target"));
    }

    #[test]
    fn test_function_with_only_callees() {
        let code = r#"
            fn caller() {
                callee1();
                callee2();
            }
            fn callee1() {}
            fn callee2() {}
        "#;

        let rels = parse_and_extract(code);
        let svg = generate_function_call_graph("caller", &rels);

        assert!(svg.is_some());
        let svg_content = svg.unwrap();
        assert!(svg_content.contains("callee"));
        assert!(svg_content.contains("caller"));
    }

    #[test]
    fn test_extract_path_name() {
        let code = r#"
            fn test() {
                std::collections::HashMap::new();
            }
        "#;

        let rels = parse_and_extract(code);

        assert!(rels.call_graph.contains_key("test"));
        assert!(rels.call_graph["test"].contains("std::collections::HashMap::new"));
    }

    #[test]
    fn test_method_is_marked_as_method() {
        let code = r#"
            struct Foo;
            impl Foo {
                fn is_method(&self) {}
            }
            fn is_function() {}
        "#;

        let rels = parse_and_extract(code);

        assert!(rels.functions["Foo::is_method"].is_method);
        assert!(!rels.functions["is_function"].is_method);
    }

    #[test]
    fn test_turbofish_syntax() {
        let code = r#"
            fn caller() {
                helper::<i32>();
            }
            fn helper<T>() {}
        "#;

        let rels = parse_and_extract(code);

        assert!(rels.call_graph["caller"].contains("helper"));
    }

    #[test]
    fn test_match_with_calls() {
        let code = r#"
            fn test(x: Option<i32>) {
                match x {
                    Some(_) => handle_some(),
                    None => handle_none(),
                }
            }
            fn handle_some() {}
            fn handle_none() {}
        "#;

        let rels = parse_and_extract(code);

        assert!(rels.call_graph["test"].contains("handle_some"));
        assert!(rels.call_graph["test"].contains("handle_none"));
    }

    #[test]
    fn test_if_else_with_calls() {
        let code = r#"
            fn test(condition: bool) {
                if condition {
                    branch_true();
                } else {
                    branch_false();
                }
            }
            fn branch_true() {}
            fn branch_false() {}
        "#;

        let rels = parse_and_extract(code);

        assert!(rels.call_graph["test"].contains("branch_true"));
        assert!(rels.call_graph["test"].contains("branch_false"));
    }

    #[test]
    fn test_loop_with_calls() {
        let code = r#"
            fn test() {
                loop {
                    if condition() {
                        break;
                    }
                    action();
                }
            }
            fn condition() -> bool { true }
            fn action() {}
        "#;

        let rels = parse_and_extract(code);

        assert!(rels.call_graph["test"].contains("condition"));
        assert!(rels.call_graph["test"].contains("action"));
    }

    #[test]
    fn test_chained_method_calls() {
        let code = r#"
            fn test() {
                vec![1, 2, 3]
                    .iter()
                    .map()
                    .collect();
            }
        "#;

        let rels = parse_and_extract(code);

        assert!(rels.call_graph.contains_key("test"));
        assert!(rels.call_graph["test"].contains("iter"));
        assert!(rels.call_graph["test"].contains("map"));
        assert!(rels.call_graph["test"].contains("collect"));
    }
}
