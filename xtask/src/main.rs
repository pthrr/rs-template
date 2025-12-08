mod relationships;

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use walkdir::WalkDir;

use base64::{Engine as _, engine::general_purpose};
use relationships::{
    CodeRelationships, extract_relationships, generate_function_call_graph,
    generate_type_inheritance_graph,
};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        print_help();
        return Ok(());
    }

    match args[1].as_str() {
        "doc" => generate_and_process_docs(false)?,
        "doc-open" => generate_and_process_docs(true)?,
        "help" | "--help" | "-h" => print_help(),
        _ => {
            eprintln!("Unknown command: {}", args[1]);
            print_help();
            std::process::exit(1);
        }
    }

    Ok(())
}

fn print_help() {
    println!("xtask - Documentation generator and processor");
    println!();
    println!("USAGE:");
    println!("    cargo xtask <COMMAND>");
    println!();
    println!("COMMANDS:");
    println!("    doc         Generate documentation with call graphs");
    println!("    doc-open    Generate documentation and open in browser");
    println!("    help        Print this help message");
}

fn generate_and_process_docs(open: bool) -> Result<()> {
    println!("Generating documentation...");

    // Find the workspace root by looking for Cargo.toml with [workspace]
    let workspace_root = find_workspace_root()?;

    // Run cargo doc from the workspace root
    let status = Command::new("cargo")
        .args(["doc", "--no-deps"])
        .current_dir(&workspace_root)
        .status()?;

    if !status.success() {
        eprintln!("Failed to generate documentation");
        std::process::exit(1);
    }

    println!("Documentation generated successfully!");

    // Extract relationships from source files
    println!("\nExtracting code relationships...");
    let source_files = collect_source_files(&workspace_root)?;
    let relationships = extract_relationships(source_files);

    println!("  Found {} functions", relationships.functions.len());
    println!("  Call graph edges: {}", relationships.call_graph.len());
    println!("  Inheritance entries: {}", relationships.inheritance.len());

    // Find and process all HTML files
    let doc_dir = workspace_root.join("target/doc");

    if !doc_dir.exists() {
        eprintln!("Documentation directory not found: {}", doc_dir.display());
        return Ok(());
    }

    println!("\nProcessing documentation files...");

    let mut file_count = 0;
    for entry in WalkDir::new(&doc_dir)
        .follow_links(true)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();

        if path.extension().and_then(|s| s.to_str()) == Some("html") {
            process_html_file(path, &relationships)?;
            file_count += 1;
        }
    }

    println!("\nProcessed {} HTML files", file_count);
    let index_path = "target/doc/rust_template/index.html";
    println!("\nDocumentation available at: {}", index_path);

    // Open in browser if requested
    if open {
        println!("Opening documentation in browser...");
        let full_path = workspace_root.join(index_path);

        #[cfg(target_os = "linux")]
        let _ = Command::new("xdg-open").arg(&full_path).spawn();

        #[cfg(target_os = "macos")]
        let _ = Command::new("open").arg(&full_path).spawn();

        #[cfg(target_os = "windows")]
        let _ = Command::new("cmd")
            .args(["/C", "start"])
            .arg(&full_path)
            .spawn();
    }

    Ok(())
}

fn collect_source_files(workspace_root: &Path) -> Result<Vec<PathBuf>> {
    let mut source_files = Vec::new();

    // Find all .rs files in src/ directories
    for entry in WalkDir::new(workspace_root.join("src"))
        .follow_links(true)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) == Some("rs") {
            source_files.push(path.to_path_buf());
        }
    }

    Ok(source_files)
}

fn process_html_file(path: &Path, relationships: &CodeRelationships) -> Result<()> {
    println!("  Processing: {}", path.display());

    // Read the HTML file
    let content = fs::read_to_string(path)?;

    // Check if already processed to avoid double injection
    if is_already_processed(&content) {
        println!("    ⊘ Already processed, skipping");
        return Ok(());
    }

    // Add custom footer and inject graphs
    let mut modified = add_custom_footer(&content);
    modified = inject_call_graphs(&modified, relationships);
    modified = inject_inheritance_graphs(&modified, relationships, path);

    // Write back if modified
    if modified != content {
        fs::write(path, modified)?;
        println!("    ✓ Modified");
    }

    Ok(())
}

fn is_already_processed(html: &str) -> bool {
    // Check for our processing marker
    html.contains("<!-- Documentation processed by xtask -->")
}

fn add_custom_footer(html: &str) -> String {
    // Add a custom HTML comment before the closing body tag
    // Only add if not already present
    if html.contains("</body>") && !is_already_processed(html) {
        html.replace(
            "</body>",
            "<!-- Documentation processed by xtask -->\n</body>",
        )
    } else {
        html.to_string()
    }
}

fn inject_call_graphs(html: &str, relationships: &CodeRelationships) -> String {
    let mut result = html.to_string();

    // Look for function documentation sections
    for (func_name, _metadata) in &relationships.functions {
        // Try to generate a call graph for this function
        if let Some(svg) = generate_function_call_graph(func_name, relationships) {
            // Encode SVG as base64 for embedding
            let svg_base64 = general_purpose::STANDARD.encode(&svg);
            let data_uri = format!("data:image/svg+xml;base64,{}", svg_base64);

            // Create a call graph section
            let call_graph_html = format!(
                "<h2 id=\"call-graph\"><a class=\"doc-anchor\" href=\"#call-graph\">§</a>Call Graph</h2>\n\
<div class=\"docblock\">\n    \
    <img src=\"{}\" alt=\"Call graph for {}\" style=\"max-width: 100%; height: auto; margin: 10px 0;\" />\n    \
    <p style=\"font-size: 0.9em; color: rgb(102, 102, 102);\">\n        \
        <strong>Legend:</strong>\n        \
        <span style=\"color: rgb(245, 124, 0);\">■</span> Callers →\n        \
        <span style=\"color: rgb(21, 101, 192);\">■</span> This function →\n        \
        <span style=\"color: rgb(46, 125, 50);\">■</span> Callees\n    \
    </p>\n\
</div>\n",
                data_uri, func_name
            );

            // Extract simple function name for matching
            let simple_name = func_name.split("::").last().unwrap_or(func_name);

            // Check if this HTML page is for this specific function
            if result.contains(&format!(
                "Function <span class=\"fn\">{}</span>",
                simple_name
            )) {
                // Insert before </div></details> (the closing of the docblock)
                if let Some(pos) = result.find("</div></details></section>") {
                    result.insert_str(pos, &call_graph_html);
                }
            }
        }
    }

    result
}

fn inject_inheritance_graphs(html: &str, relationships: &CodeRelationships, path: &Path) -> String {
    let mut result = html.to_string();

    // Extract unique type names from inheritance info
    let mut type_names: Vec<&String> = relationships
        .inheritance
        .values()
        .map(|info| &info.type_name)
        .collect();
    type_names.sort();
    type_names.dedup();

    // Look for struct/enum documentation sections
    for type_name in type_names {
        // Check if this file is for this type by looking at the filename
        let file_name = path.file_name().and_then(|f| f.to_str()).unwrap_or("");
        let simple_name = type_name.split("::").last().unwrap_or(type_name);

        // Match struct.TypeName.html or enum.TypeName.html
        let matches_file = file_name == format!("struct.{}.html", simple_name)
            || file_name == format!("enum.{}.html", simple_name);

        if matches_file {
            // Try to generate an inheritance graph for this type
            if let Some(svg) = generate_type_inheritance_graph(type_name, relationships) {
                // Encode SVG as base64 for embedding
                let svg_base64 = general_purpose::STANDARD.encode(&svg);
                let data_uri = format!("data:image/svg+xml;base64,{}", svg_base64);

                // Create an inheritance graph section
                let inheritance_graph_html = format!(
                    "<h2 id=\"trait-graph\"><a class=\"doc-anchor\" href=\"#trait-graph\">§</a>Trait Implementation Graph</h2>\n\
<div class=\"docblock\">\n    \
    <img src=\"{}\" alt=\"Trait implementations for {}\" style=\"max-width: 100%; height: auto; margin: 10px 0;\" />\n    \
    <p style=\"font-size: 0.9em; color: rgb(102, 102, 102);\">\n        \
        <strong>Legend:</strong>\n        \
        <span style=\"color: rgb(106, 27, 154);\">■</span> Traits\n        \
        | <span style=\"color: rgb(21, 101, 192);\">■</span> This type\n        \
        | <span style=\"color: rgb(255, 152, 0);\">⟶</span> Supertrait (extends)\n        \
        | <span style=\"color: rgb(106, 27, 154);\">→</span> Implementation\n    \
    </p>\n\
</div>\n",
                    data_uri, type_name
                );

                // Insert after struct/enum description, before trait implementations section
                // Try multiple patterns because the structure varies:
                // - Structs without methods: </div></details><h2 id="trait-implementations"
                // - Structs with methods: </div></details></div></details></div><h2 id="trait-implementations"

                let inserted = if let Some(pos) = result
                    .find("</div></details></div></details></div><h2 id=\"trait-implementations\"")
                {
                    // Struct with methods (implementations section)
                    result.insert_str(
                        pos + "</div></details></div></details></div>".len(),
                        &inheritance_graph_html,
                    );
                    true
                } else if let Some(pos) =
                    result.find("</div></details><h2 id=\"trait-implementations\"")
                {
                    // Simple struct without methods
                    result.insert_str(pos + "</div></details>".len(), &inheritance_graph_html);
                    true
                } else {
                    false
                };

                if !inserted {
                    eprintln!(
                        "  Warning: Could not find insertion point for {} trait graph",
                        simple_name
                    );
                }
            }
        }
    }

    result
}

fn find_workspace_root() -> Result<std::path::PathBuf> {
    let mut current = env::current_dir()?;

    // Start from current directory and go up
    loop {
        let cargo_toml = current.join("Cargo.toml");

        if cargo_toml.exists() {
            // Check if this Cargo.toml contains [workspace]
            let content = fs::read_to_string(&cargo_toml)?;
            if content.contains("[workspace]") {
                return Ok(current);
            }
        }

        // Move to parent directory
        if let Some(parent) = current.parent() {
            current = parent.to_path_buf();
        } else {
            return Err("Could not find workspace root".into());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_custom_footer() {
        let html = "<html><body><p>Content</p></body></html>";
        let result = add_custom_footer(html);
        assert!(result.contains("<!-- Documentation processed by xtask -->"));
    }

    #[test]
    fn test_is_already_processed_not_processed() {
        let html = "<html><body><p>Content</p></body></html>";
        assert!(!is_already_processed(html));
    }

    #[test]
    fn test_is_already_processed_already_processed() {
        let html =
            "<html><body><p>Content</p><!-- Documentation processed by xtask -->\n</body></html>";
        assert!(is_already_processed(html));
    }

    #[test]
    fn test_idempotent_processing() {
        let html = "<html><body><p>Content</p></body></html>";
        let first = add_custom_footer(html);
        let second = add_custom_footer(&first);

        // Should only add marker once
        assert_eq!(first, second);
        assert_eq!(
            first
                .matches("<!-- Documentation processed by xtask -->")
                .count(),
            1
        );
    }
}
