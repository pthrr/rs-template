use clap::Parser;
use rust_template::greet;

#[derive(Parser)]
#[command(name = "rust-template")]
#[command(about = "A minimal Rust template example", long_about = None)]
#[command(version)]
struct Cli {
    #[arg(short, long, default_value = "World")]
    name: String,
}

fn main() {
    let cli = Cli::parse();
    println!("{}", greet(&cli.name));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cli_parsing() {
        let cli = Cli {
            name: "Test".to_string(),
        };

        assert_eq!(cli.name, "Test");
    }
}
