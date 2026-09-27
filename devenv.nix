{ lib, pkgs, ... }:

{
  env = {
    RUSTC_WRAPPER = "sccache";
    # GPUI loads its graphics backends dynamically, so they do not appear in
    # the development binary's RUNPATH until it is packaged.
    LD_LIBRARY_PATH = lib.makeLibraryPath [
      pkgs.libGL
      pkgs.wayland
      pkgs.vulkan-loader
    ];
    LIBCLANG_PATH = lib.makeLibraryPath [ pkgs.llvmPackages.libclang ];
  };

  packages = with pkgs; [
    git
    cmake
    sccache
    prek
    typos
    # The `ast-grep` CI job and the prek hook of the same name.
    ast-grep
    # The `shell` CI job and the prek hooks of the same name.
    shellcheck
    shfmt
    # The build and GUI runtime use these libraries directly; declare them
    # instead of relying on transitive packages or the host environment.
    pkg-config
    fontconfig
    freetype
    libGL
    nfpm
    libxcb
    libxkbcommon
    wayland
    vulkan-loader
  ];

  languages.rust = {
    enable = true;
    channel = "stable";
    components = [
      "rustc"
      "cargo"
      "clippy"
      "rustfmt"
      "rust-analyzer"
      "rust-src"
    ];
    # `wasm32-unknown-unknown` is not a shipping target: nothing here is built
    # for the browser. It exists so `cargo check` can *prove* the portable
    # layer stays portable — a crate that has no business touching the host
    # (protocol codec, device model) fails to compile here the moment it picks
    # up a dependency that does. Discipline drifts; a compiler does not.
    targets = [
      "wasm32-unknown-unknown"
    ];
  };

  tasks = {
    "openlogi:run" = {
      description = "List connected Logitech HID++ devices.";
      exec = "cargo run -p openlogi -- list";
    };
    "openlogi:gui" = {
      description = "Run the desktop app.";
      exec = "cargo run -p openlogi-desktop";
    };
    "openlogi:check" = {
      description = "Run fmt, clippy, tests, and rustdoc.";
      exec = ''
        set -e
        cargo fmt --all -- --check
        cargo clippy --workspace --all-targets -- -D warnings
        cargo test --workspace
        # Mirrors CI's `rustdoc (non-GUI crates)` job: a broken intra-doc link
        # is neither a compile error nor a clippy lint, so nothing above catches
        # it. The GPUI crates are excluded — documenting them would pull in the
        # whole graphics toolchain.
        RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps \
          --document-private-items --exclude openlogi-ui \
          --exclude openlogi-desktop --exclude openlogi-overlay \
          --exclude openlogi-agent
      '';
    };
    "openlogi:ci" = {
      description = "Run every GitHub Actions CI job this host can reproduce.";
      exec = "cargo run -p xtask -- ci";
    };
    "openlogi:assets" = {
      description = "Sync device assets.";
      exec = "cargo run -p openlogi --release -- assets sync";
    };
  };
}
