{
  description = "Rust Template Project";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, nixpkgs, flake-utils, rust-overlay }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs {
          inherit system overlays;
        };

        rustToolchain = pkgs.rust-bin.nightly.latest.default.override {
          extensions = [ "rust-src" "rust-analyzer" ];
        };
      in
      {
        devShells.default = pkgs.mkShell {
          buildInputs = with pkgs; [
            rustToolchain
            pkg-config
            openssl
            go-task
            cargo-watch
            cargo-edit
            cargo-outdated
            cargo-audit
            cargo-expand
            pre-commit
            graphviz
            git
            markdownlint-cli
            yamllint
          ];

          shellHook = ''
            echo "Rust Template Development Shell"
            echo "================================"
            echo ""
            echo "Rust: $(rustc --version)"
            echo "Cargo: $(cargo --version)"
            echo ""
            echo "Commands:"
            echo "  task            - Show available tasks"
            echo "  cargo build     - Build project"
            echo "  cargo test      - Run tests"
            echo "  cargo xtask     - Run xtask commands"
            echo "  pre-commit run  - Run pre-commit hooks"
            echo ""

            export RUST_BACKTRACE=1

            if [ -f .pre-commit-config.yaml ] && [ ! -f .git/hooks/pre-commit ]; then
              echo "Installing pre-commit hooks..."
              pre-commit install
            fi
          '';

          RUST_SRC_PATH = "${rustToolchain}/lib/rustlib/src/rust/library";
        };
      }
    );
}
