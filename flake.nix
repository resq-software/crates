# Copyright 2026 ResQ
#
# Licensed under the Apache License, Version 2.0 (the "License");
# you may not use this file except in compliance with the License.
# You may obtain a copy of the License at
#
#     http://www.apache.org/licenses/LICENSE-2.0
#
# Unless required by applicable law or agreed to in writing, software
# distributed under the License is distributed on an "AS IS" BASIS,
# WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
# See the License for the specific language governing permissions and
# limitations under the License.

{
  description = "ResQ CLI — Rust workspace of developer and ops tooling";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-25.11";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay.url = "github:oxalica/rust-overlay";
  };

  outputs =
    { self, nixpkgs, flake-utils, rust-overlay, ... }:
    let
      supportedSystems = [
        "x86_64-linux"
        "aarch64-linux"
        "x86_64-darwin"
        "aarch64-darwin"
      ];

      mkDevShell = pkgs: system:
        let
          rustToolchain = pkgs.rust-bin.stable.latest.default.override {
            extensions = [ "rust-src" "rust-analyzer" "rustfmt" "clippy" ];
          };

          devPackages = with pkgs; [
            # Rust
            rustToolchain
            cargo-watch
            cargo-nextest

            # Native deps for reqwest (rustls handles TLS, but openssl needed for some crates)
            pkg-config
            openssl

            # Dev utilities
            git
            ripgrep
            fd
            jq
            osv-scanner
            osv-scanner
          ] ++ lib.optionals stdenv.isLinux [
            libudev-zero   # needed by some sys crates
          ] ++ lib.optionals stdenv.isDarwin [
            darwin.apple_sdk.frameworks.Security
            darwin.apple_sdk.frameworks.CoreFoundation
            darwin.apple_sdk.frameworks.SystemConfiguration
          ];

          shellHook = ''
            echo "--- ResQ CLI Dev Environment (${system}) ---"

            version_check() {
              local cmd="$1" name="$2"
              if command -v "$cmd" >/dev/null 2>&1; then
                echo "$name: $("$cmd" --version 2>/dev/null | head -n1 | xargs)"
              else
                echo "$name: NOT FOUND"
              fi
            }

            version_check rustc  "Rust"
            version_check cargo  "Cargo"
            version_check clippy-driver "Clippy"

            echo "Workspace members: resq, resq-deploy, resq-health, resq-logs, resq-perf, resq-flame, resq-tui, bin_explorer, resq-clean"
            echo "Build all:  cargo build --release --workspace"
            echo "Watch:      cargo watch -x check"
            echo "Test:       cargo nextest run"
            echo "Lint:       cargo clippy --workspace -- -D warnings"
            echo "-------------------------------------------"

            export CARGO_HOME="$PWD/.cargo"
            export PATH="$CARGO_HOME/bin:$PATH"
            export PKG_CONFIG_PATH="${pkgs.openssl.dev}/lib/pkgconfig''${PKG_CONFIG_PATH:+:$PKG_CONFIG_PATH}"
          '';
        in
        {
          default = pkgs.mkShell {
            packages = devPackages;
            inherit shellHook;
          };
        };
    in
    flake-utils.lib.eachSystem supportedSystems (system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [ rust-overlay.overlays.default ];
          config.allowUnfree = true;
        };
      in
      {
        formatter = pkgs.alejandra or pkgs.nixpkgs-fmt;
        devShells = mkDevShell pkgs system;
      }
    );
}
