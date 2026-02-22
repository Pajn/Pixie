set shell := ["bash", "-cu"]

app_name := "Pixie"
bundle_output := "target/release/bundle/osx" / app_name + ".app"
dist_dir := "dist"
dist_bundle := dist_dir / app_name + ".app"
sign_identity := env_var_or_default("PIXIE_SIGN_IDENTITY", "-")

default:
    @just --list

# Build the release binary
build:
    cargo build --release

# Create the app bundle using cargo-bundle
bundle: build
    cargo bundle --release

# Create the app bundle and copy to dist/
app: bundle
    rm -rf "{{dist_bundle}}"
    mkdir -p "{{dist_dir}}"
    cp -R "{{bundle_output}}" "{{dist_bundle}}"

# Sign the app bundle in dist/
sign identity=sign_identity:
    codesign --force --deep --sign "{{identity}}" "{{dist_bundle}}"
    codesign --verify --deep --strict --verbose=2 "{{dist_bundle}}"

# Build, bundle, and sign the app (default ad-hoc signing)
signed-app: app sign

# Install cargo-bundle if not already installed
install-bundler:
    cargo install cargo-bundle

# Benchmark picker window enumeration latency
bench-picker:
    cargo test benchmark_get_all_windows -- --ignored --nocapture
