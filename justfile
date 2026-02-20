set shell := ["bash", "-cu"]

app_name := "Pixie"
bundle_dir := "dist" / app_name + ".app"
binary_path := "target/release/pixie"
template_path := "Pixie.app"
sign_identity := env_var_or_default("PIXIE_SIGN_IDENTITY", "-")

default:
    @just --list

build:
    cargo build --release

app: build
    rm -rf "{{bundle_dir}}"
    mkdir -p dist
    ditto "{{template_path}}" "{{bundle_dir}}"
    cp "{{binary_path}}" "{{bundle_dir}}/Contents/MacOS/pixie"
    chmod +x "{{bundle_dir}}/Contents/MacOS/pixie"

sign identity=sign_identity: app
    codesign --force --deep --sign "{{identity}}" "{{bundle_dir}}"
    codesign --verify --deep --strict --verbose=2 "{{bundle_dir}}"

signed-app: sign
