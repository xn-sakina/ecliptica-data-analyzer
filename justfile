version := `awk -F '"' '/^version = "/ { print $2; exit }' Cargo.toml`

test:
    cargo test

run:
    cargo run

# Bump the package version. Defaults to patch; accepts minor, major, or X.Y.Z.
up-version part="patch":
    bash scripts/up-version.sh {{part}}

windows:
    RC=x86_64-w64-mingw32-windres cargo xwin build --release --target x86_64-pc-windows-msvc
    mkdir -p dist
    cp target/x86_64-pc-windows-msvc/release/ecliptica-data-analyzer.exe dist/ecliptica-data-analyzer-v{{version}}-windows-x64.exe
    @echo "dist/ecliptica-data-analyzer-v{{version}}-windows-x64.exe"
