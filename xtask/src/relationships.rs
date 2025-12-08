use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use syn::visit::Visit;
use syn::{Expr, ExprCall, ExprMethodCall, ImplItem, ItemFn, ItemImpl, ItemTrait};

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
        let trait_name = node
            .trait_
            .as_ref()
            .map(|(_, path, _)| Self::extract_path_name(path));

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

    fn visit_item_trait(&mut self, node: &'a ItemTrait) {
        let trait_name = node.ident.to_string();

        // Extract supertraits from trait bounds
        let mut supertraits = Vec::new();
        if node.colon_token.is_some() {
            for bound in &node.supertraits {
                if let syn::TypeParamBound::Trait(trait_bound) = bound {
                    let supertrait_name = Self::extract_path_name(&trait_bound.path);
                    supertraits.push(supertrait_name);
                }
            }
        }

        // Store trait definition with its supertraits
        if !supertraits.is_empty() {
            self.inheritance.insert(
                format!("__trait_def::{}", trait_name),
                InheritanceInfo {
                    trait_name: Some(trait_name.clone()),
                    type_name: "__trait_definition__".to_string(),
                    methods: Vec::new(),
                    bounds: Vec::new(),
                    parent_traits: supertraits,
                },
            );
        }

        // Continue visiting trait items
        syn::visit::visit_item_trait(self, node);
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

    // Populate parent_traits for trait implementations
    let trait_definitions: HashMap<String, Vec<String>> = inheritance
        .iter()
        .filter(|(key, info)| {
            key.starts_with("__trait_def::") && info.type_name == "__trait_definition__"
        })
        .map(|(key, info)| {
            let trait_name = key.strip_prefix("__trait_def::").unwrap_or("");
            (trait_name.to_string(), info.parent_traits.clone())
        })
        .collect();

    for (_, info) in inheritance.iter_mut() {
        if let Some(ref trait_name) = info.trait_name {
            if let Some(supertraits) = trait_definitions.get(trait_name) {
                info.parent_traits = supertraits.clone();
            }
        }
    }

    // Remove temporary trait definition entries
    inheritance.retain(|key, _| !key.starts_with("__trait_def::"));

    CodeRelationships {
        call_graph,
        usage_graph,
        inheritance,
        functions,
    }
}

/// Generate an SVG inheritance/trait implementation graph for a specific type
pub fn generate_type_inheritance_graph(
    type_name: &str,
    relationships: &CodeRelationships,
) -> Option<String> {
    // Find all trait implementations for this type
    let trait_impls: Vec<(&String, &InheritanceInfo)> = relationships
        .inheritance
        .iter()
        .filter(|(_, info)| info.type_name == type_name && info.trait_name.is_some())
        .collect();

    // Also check for inherent impl (no trait)
    let inherent_impl = relationships
        .inheritance
        .get(type_name)
        .filter(|info| info.trait_name.is_none());

    if trait_impls.is_empty() && inherent_impl.is_none() {
        return None;
    }

    // If only inherent impl exists (no traits), generate simple diagram
    if trait_impls.is_empty() {
        let width = 400;
        let height = 200;
        let simple_type = type_name.split("::").last().unwrap_or(type_name);

        let mut svg = format!(
            "<svg width=\"{}\" height=\"{}\" xmlns=\"http://www.w3.org/2000/svg\">\n  \
  <style>\n    \
    .type-node {{ fill: rgb(33, 150, 243); stroke: rgb(21, 101, 192); stroke-width: 3; }}\n    \
    .text {{ fill: white; font-family: monospace; font-size: 12px; font-weight: bold; text-anchor: middle; }}\n    \
    .method-text {{ fill: white; font-family: monospace; font-size: 10px; text-anchor: middle; opacity: 0.9; }}\n  \
  </style>\n",
            width, height
        );

        svg.push_str(&format!(
            "  <rect x=\"50\" y=\"75\" width=\"300\" height=\"70\" rx=\"5\" class=\"type-node\" />\n"
        ));
        svg.push_str(&format!(
            "  <text x=\"200\" y=\"105\" class=\"text\">{}</text>\n",
            simple_type
        ));
        svg.push_str(&format!(
            "  <text x=\"200\" y=\"127\" class=\"method-text\">struct (no traits)</text>\n"
        ));
        svg.push_str("</svg>");

        return Some(svg);
    }

    // Build dependency graph: child -> parents
    let mut dependencies: HashMap<String, Vec<String>> = HashMap::new();
    for (_, info) in &trait_impls {
        let trait_name = info.trait_name.as_ref().unwrap().clone();
        dependencies.insert(trait_name, info.parent_traits.clone());
    }

    // Calculate hierarchical layers
    fn calculate_layer(
        trait_name: &str,
        dependencies: &HashMap<String, Vec<String>>,
        layers: &mut HashMap<String, usize>,
    ) -> usize {
        if let Some(&layer) = layers.get(trait_name) {
            return layer;
        }

        let layer = if let Some(parents) = dependencies.get(trait_name) {
            if parents.is_empty() {
                0 // Root trait
            } else {
                parents
                    .iter()
                    .map(|p| calculate_layer(p, dependencies, layers))
                    .max()
                    .unwrap_or(0)
                    + 1
            }
        } else {
            0
        };

        layers.insert(trait_name.to_string(), layer);
        layer
    }

    let mut trait_layers: HashMap<String, usize> = HashMap::new();
    for (_, info) in &trait_impls {
        let trait_name = info.trait_name.as_ref().unwrap();
        calculate_layer(trait_name, &dependencies, &mut trait_layers);
    }

    // Group traits by layer
    let max_layer = trait_layers.values().max().copied().unwrap_or(0);
    let mut layers: Vec<Vec<String>> = vec![Vec::new(); max_layer + 1];
    for (trait_name, &layer) in &trait_layers {
        layers[layer].push(trait_name.clone());
    }

    // Calculate dimensions with enough spacing to prevent overlaps
    let max_nodes_in_layer = layers.iter().map(|layer| layer.len()).max().unwrap_or(1);
    let (node_height, vertical_spacing, node_width) = (70, 80, 280); // Increased spacing to 80px
    let row_height = node_height + vertical_spacing; // Now 150px total

    let width = 1400; // Increased width for better horizontal spacing
    let min_height = max_nodes_in_layer * row_height + 200;
    let height = min_height.max(500).min(1400); // Increased max height

    let left_margin = 100;
    let layer_width = if max_layer > 0 {
        ((width - left_margin - 400) / max_layer).max(node_width + 50)
    } else {
        width - left_margin - 400
    };

    // Calculate positions (grid-based)
    let mut trait_positions: HashMap<String, (usize, usize)> = HashMap::new();
    let total_grid_height = max_nodes_in_layer * row_height;
    let grid_start_y = if total_grid_height < height {
        (height - total_grid_height) / 2
    } else {
        50
    };

    for (layer_idx, layer_traits) in layers.iter().enumerate() {
        let layer_x = left_margin + layer_idx * layer_width;
        let num_nodes = layer_traits.len();
        let empty_rows = max_nodes_in_layer - num_nodes;
        let skip_top = empty_rows / 2;

        for (i, trait_name) in layer_traits.iter().enumerate() {
            let row_index = skip_top + i;
            let y_pos = grid_start_y + row_index * row_height;
            trait_positions.insert(trait_name.clone(), (layer_x, y_pos));
        }
    }

    let type_x = width - 350;
    let type_y = height / 2 - 35;

    // Generate SVG
    let mut svg = format!(
        "<svg width=\"{}\" height=\"{}\" xmlns=\"http://www.w3.org/2000/svg\">\n  \
  <style>\n    \
    .type-node {{ fill: rgb(33, 150, 243); stroke: rgb(21, 101, 192); stroke-width: 3; }}\n    \
    .trait-node {{ fill: rgb(156, 39, 176); stroke: rgb(106, 27, 154); stroke-width: 2; }}\n    \
    .impl-edge {{ stroke: rgb(156, 39, 176); stroke-width: 3; marker-end: url(#impl-arrow); }}\n    \
    .super-edge {{ stroke: rgb(255, 152, 0); stroke-width: 2; stroke-dasharray: 6,4; marker-end: url(#super-arrow); }}\n    \
    .text {{ fill: white; font-family: monospace; font-size: 12px; font-weight: bold; text-anchor: middle; }}\n    \
    .method-text {{ fill: white; font-family: monospace; font-size: 10px; text-anchor: middle; opacity: 0.9; }}\n  \
  </style>\n  \
  <defs>\n    \
    <marker id=\"impl-arrow\" markerWidth=\"12\" markerHeight=\"12\" refX=\"10\" refY=\"3\" orient=\"auto\">\n      \
      <polygon points=\"0 0, 12 3, 0 6\" fill=\"rgb(156, 39, 176)\" />\n    \
    </marker>\n    \
    <marker id=\"super-arrow\" markerWidth=\"12\" markerHeight=\"12\" refX=\"10\" refY=\"3\" orient=\"auto\">\n      \
      <polygon points=\"0 0, 12 3, 0 6\" fill=\"rgb(255, 152, 0)\" />\n    \
    </marker>\n  \
  </defs>\n",
        width, height
    );

    // DRAW ARROWS FIRST (so they appear below/behind nodes)

    // Supertrait arrows (parent -> child, flowing left-to-right)
    let supertraits: HashSet<String> = trait_impls
        .iter()
        .flat_map(|(_, info)| info.parent_traits.iter().cloned())
        .collect();

    for (_, info) in trait_impls.iter() {
        let child_trait = info.trait_name.as_ref().unwrap();
        if let Some((child_x, child_y)) = trait_positions.get(child_trait) {
            for parent_trait in &info.parent_traits {
                if let Some((parent_x, parent_y)) = trait_positions.get(parent_trait) {
                    // parent_traits means "these are my PARENTS/supertraits"
                    // So arrow should flow FROM child (who has the parents) TO parent
                    // This represents the "extends" relationship: child extends parent
                    // Arrow flows child→parent (RIGHT to LEFT for hierarchy display)
                    // START at CENTER of child, END at RIGHT EDGE of parent
                    let start_x = child_x + node_width / 2; // Center of child
                    let start_y = child_y + node_height / 2;
                    let end_x = parent_x + node_width; // Right edge of parent
                    let end_y = parent_y + node_height / 2;

                    svg.push_str(&format!(
                        "  <line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" class=\"super-edge\" />\n",
                        start_x, start_y, end_x, end_y
                    ));
                }
            }
        }
    }

    // Implementation arrows (leaf trait -> type, flowing left-to-right)
    for (_, info) in trait_impls.iter() {
        let trait_name = info.trait_name.as_ref().unwrap();
        // Skip if this is a supertrait (only leaf traits get impl arrows)
        if supertraits.contains(trait_name) {
            continue;
        }

        if let Some((trait_x, trait_y)) = trait_positions.get(trait_name) {
            // Arrow flows trait→type (LEFT to RIGHT)
            // START at CENTER of trait, END at LEFT EDGE of type
            let start_x = trait_x + node_width / 2; // Center of trait
            let start_y = trait_y + node_height / 2;
            let end_x = type_x; // Left edge of type node
            let end_y = type_y + 35; // Center of type node

            svg.push_str(&format!(
                "  <line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" class=\"impl-edge\" />\n",
                start_x, start_y, end_x, end_y
            ));
        }
    }

    // NOW DRAW NODES (so they appear on top)

    // Draw trait nodes
    for (_, info) in trait_impls.iter() {
        let trait_name = info.trait_name.as_ref().unwrap();
        if let Some((x, y)) = trait_positions.get(trait_name) {
            let simple_trait = trait_name.split("::").last().unwrap_or(trait_name);

            svg.push_str(&format!(
                "  <rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"5\" class=\"trait-node\" />\n",
                x, y, node_width, node_height
            ));

            svg.push_str(&format!(
                "  <text x=\"{}\" y=\"{}\" class=\"text\">trait {}</text>\n",
                x + node_width / 2,
                y + 25,
                simple_trait
            ));

            let methods_str = if info.methods.is_empty() {
                "no methods".to_string()
            } else if info.methods.len() <= 2 {
                info.methods.join(", ")
            } else {
                format!("{} methods", info.methods.len())
            };

            svg.push_str(&format!(
                "  <text x=\"{}\" y=\"{}\" class=\"method-text\">{}</text>\n",
                x + node_width / 2,
                y + 50,
                methods_str
            ));
        }
    }

    // Draw type node
    let simple_type = type_name.split("::").last().unwrap_or(type_name);
    svg.push_str(&format!(
        "  <rect x=\"{}\" y=\"{}\" width=\"300\" height=\"70\" rx=\"5\" class=\"type-node\" />\n",
        type_x, type_y
    ));
    svg.push_str(&format!(
        "  <text x=\"{}\" y=\"{}\" class=\"text\">{}</text>\n",
        type_x + 150,
        type_y + 30,
        simple_type
    ));
    svg.push_str(&format!(
        "  <text x=\"{}\" y=\"{}\" class=\"method-text\">struct</text>\n",
        type_x + 150,
        type_y + 52
    ));

    svg.push_str("</svg>");

    Some(svg)
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

    #[test]
    fn test_inheritance_graph_single_trait() {
        let code = r#"
            trait Greeter {
                fn greet(&self);
            }

            struct FriendlyGreeter;

            impl Greeter for FriendlyGreeter {
                fn greet(&self) {}
            }
        "#;

        let rels = parse_and_extract(code);
        let svg = generate_type_inheritance_graph("FriendlyGreeter", &rels);

        assert!(svg.is_some());
        let svg_content = svg.unwrap();
        assert!(svg_content.contains("<svg"));
        assert!(svg_content.contains("FriendlyGreeter"));
        assert!(svg_content.contains("Greeter"));
        assert!(svg_content.contains("trait-node"));
        assert!(svg_content.contains("type-node"));
    }

    #[test]
    fn test_inheritance_graph_multiple_traits() {
        let code = r#"
            trait Trait1 {
                fn method1(&self);
            }

            trait Trait2 {
                fn method2(&self);
            }

            struct MyType;

            impl Trait1 for MyType {
                fn method1(&self) {}
            }

            impl Trait2 for MyType {
                fn method2(&self) {}
            }
        "#;

        let rels = parse_and_extract(code);
        let svg = generate_type_inheritance_graph("MyType", &rels);

        assert!(svg.is_some());
        let svg_content = svg.unwrap();
        assert!(svg_content.contains("MyType"));
        assert!(svg_content.contains("Trait1"));
        assert!(svg_content.contains("Trait2"));
    }

    #[test]
    fn test_inheritance_graph_no_traits() {
        let code = r#"
            struct PlainStruct;

            impl PlainStruct {
                fn method(&self) {}
            }
        "#;

        let rels = parse_and_extract(code);
        let svg = generate_type_inheritance_graph("PlainStruct", &rels);

        // Should return Some because there's an inherent impl
        assert!(svg.is_some());
    }

    #[test]
    fn test_inheritance_graph_nonexistent_type() {
        let code = r#"
            struct Foo;
        "#;

        let rels = parse_and_extract(code);
        let svg = generate_type_inheritance_graph("NonExistent", &rels);

        assert!(svg.is_none());
    }

    #[test]
    fn test_inheritance_info_stored() {
        let code = r#"
            trait MyTrait {
                fn trait_method(&self);
            }

            struct MyStruct;

            impl MyTrait for MyStruct {
                fn trait_method(&self) {}
            }
        "#;

        let rels = parse_and_extract(code);

        assert!(rels.inheritance.contains_key("MyStruct::MyTrait"));
        let info = &rels.inheritance["MyStruct::MyTrait"];
        assert_eq!(info.type_name, "MyStruct");
        assert_eq!(info.trait_name, Some("MyTrait".to_string()));
        assert_eq!(info.methods.len(), 1);
        assert!(info.methods.contains(&"trait_method".to_string()));
    }

    #[test]
    fn test_inheritance_graph_with_enum() {
        let code = r#"
            trait Handler {
                fn handle(&self);
            }

            enum Event {
                Click,
                Hover,
            }

            impl Handler for Event {
                fn handle(&self) {}
            }
        "#;

        let rels = parse_and_extract(code);
        let svg = generate_type_inheritance_graph("Event", &rels);

        assert!(svg.is_some());
        let svg_content = svg.unwrap();
        assert!(svg_content.contains("Event"));
        assert!(svg_content.contains("Handler"));
    }

    #[test]
    fn test_inheritance_graph_many_methods() {
        let code = r#"
            trait LargeTrait {
                fn method1(&self);
                fn method2(&self);
                fn method3(&self);
                fn method4(&self);
                fn method5(&self);
            }

            struct MyType;

            impl LargeTrait for MyType {
                fn method1(&self) {}
                fn method2(&self) {}
                fn method3(&self) {}
                fn method4(&self) {}
                fn method5(&self) {}
            }
        "#;

        let rels = parse_and_extract(code);
        let svg = generate_type_inheritance_graph("MyType", &rels);

        assert!(svg.is_some());
        let svg_content = svg.unwrap();
        // Should show "5 methods" instead of listing all
        assert!(svg_content.contains("5 methods"));
    }

    #[test]
    fn test_inheritance_graph_few_methods_listed() {
        let code = r#"
            trait SmallTrait {
                fn foo(&self);
                fn bar(&self);
            }

            struct MyType;

            impl SmallTrait for MyType {
                fn foo(&self) {}
                fn bar(&self) {}
            }
        "#;

        let rels = parse_and_extract(code);
        let svg = generate_type_inheritance_graph("MyType", &rels);

        assert!(svg.is_some());
        let svg_content = svg.unwrap();
        // Should list method names for 3 or fewer
        assert!(svg_content.contains("foo"));
        assert!(svg_content.contains("bar"));
    }

    #[test]
    fn test_inheritance_graph_generic_trait() {
        let code = r#"
            trait Convert<T> {
                fn convert(&self) -> T;
            }

            struct MyType;

            impl Convert<String> for MyType {
                fn convert(&self) -> String {
                    String::new()
                }
            }
        "#;

        let rels = parse_and_extract(code);
        let svg = generate_type_inheritance_graph("MyType", &rels);

        assert!(svg.is_some());
        let svg_content = svg.unwrap();
        assert!(svg_content.contains("MyType"));
        assert!(svg_content.contains("Convert"));
    }

    #[test]
    fn test_inheritance_graph_std_trait() {
        let code = r#"
            struct MyType;

            impl std::fmt::Display for MyType {
                fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                    Ok(())
                }
            }
        "#;

        let rels = parse_and_extract(code);
        let svg = generate_type_inheritance_graph("MyType", &rels);

        assert!(svg.is_some());
        let svg_content = svg.unwrap();
        assert!(svg_content.contains("MyType"));
        // Should show simple name, not full path
        assert!(svg_content.contains("Display"));
    }

    #[test]
    fn test_inheritance_multiple_impls_same_trait() {
        let code = r#"
            struct TypeA;
            struct TypeB;

            trait Common {
                fn common(&self);
            }

            impl Common for TypeA {
                fn common(&self) {}
            }

            impl Common for TypeB {
                fn common(&self) {}
            }
        "#;

        let rels = parse_and_extract(code);

        // Each type should have its own graph
        let svg_a = generate_type_inheritance_graph("TypeA", &rels);
        let svg_b = generate_type_inheritance_graph("TypeB", &rels);

        assert!(svg_a.is_some());
        assert!(svg_b.is_some());

        let content_a = svg_a.unwrap();
        let content_b = svg_b.unwrap();

        assert!(content_a.contains("TypeA"));
        assert!(content_a.contains("Common"));

        assert!(content_b.contains("TypeB"));
        assert!(content_b.contains("Common"));
    }

    #[test]
    fn test_inheritance_trait_with_no_methods() {
        let code = r#"
            trait Marker {}

            struct MyType;

            impl Marker for MyType {}
        "#;

        let rels = parse_and_extract(code);
        let svg = generate_type_inheritance_graph("MyType", &rels);

        assert!(svg.is_some());
        let svg_content = svg.unwrap();
        assert!(svg_content.contains("MyType"));
        assert!(svg_content.contains("Marker"));
    }

    #[test]
    fn test_inherent_impl_stored() {
        let code = r#"
            struct MyStruct;

            impl MyStruct {
                fn new() -> Self {
                    MyStruct
                }
                fn method(&self) {}
            }
        "#;

        let rels = parse_and_extract(code);

        assert!(rels.inheritance.contains_key("MyStruct"));
        let info = &rels.inheritance["MyStruct"];
        assert_eq!(info.type_name, "MyStruct");
        assert_eq!(info.trait_name, None);
        assert_eq!(info.methods.len(), 2);
        assert!(info.methods.contains(&"new".to_string()));
        assert!(info.methods.contains(&"method".to_string()));
    }

    #[test]
    fn test_inheritance_graph_svg_structure() {
        let code = r#"
            trait MyTrait {
                fn test(&self);
            }

            struct MyType;

            impl MyTrait for MyType {
                fn test(&self) {}
            }
        "#;

        let rels = parse_and_extract(code);
        let svg = generate_type_inheritance_graph("MyType", &rels);

        assert!(svg.is_some());
        let svg_content = svg.unwrap();

        // Verify SVG structure
        assert!(svg_content.contains("<svg"));
        assert!(svg_content.contains("</svg>"));
        assert!(svg_content.contains("<style>"));
        assert!(svg_content.contains("<defs>"));
        assert!(svg_content.contains("<rect"));
        assert!(svg_content.contains("<text"));
        assert!(svg_content.contains("<line"));
        assert!(svg_content.contains("impl-edge"));
        assert!(svg_content.contains("trait-node"));
        assert!(svg_content.contains("type-node"));
    }

    #[test]
    fn test_inheritance_combined_trait_and_inherent() {
        let code = r#"
            trait Greet {
                fn greet(&self);
            }

            struct Person;

            impl Greet for Person {
                fn greet(&self) {}
            }

            impl Person {
                fn new() -> Self {
                    Person
                }
            }
        "#;

        let rels = parse_and_extract(code);

        // Should have both trait impl and inherent impl
        assert!(rels.inheritance.contains_key("Person::Greet"));
        assert!(rels.inheritance.contains_key("Person"));

        let svg = generate_type_inheritance_graph("Person", &rels);
        assert!(svg.is_some());

        let svg_content = svg.unwrap();
        assert!(svg_content.contains("Person"));
        assert!(svg_content.contains("Greet"));
    }

    #[test]
    fn test_inheritance_qualified_type_name() {
        let code = r#"
            mod inner {
                pub struct MyType;
            }

            trait MyTrait {
                fn test(&self);
            }

            impl MyTrait for inner::MyType {
                fn test(&self) {}
            }
        "#;

        let rels = parse_and_extract(code);

        // The type name should include the module path
        let has_qualified = rels
            .inheritance
            .values()
            .any(|info| info.type_name.contains("inner"));

        assert!(has_qualified);
    }
}
