set shell := ["bash", "-cu"]

app_name := "Pixie"
bundle_dir := "dist/{{app_name}}.app"
binary_path := "target/release/pixie"
plist_path := "Pixie.app/Contents/Info.plist"
icon_path := "Pixie.app/Contents/Resources/AppIcon.icns"
sign_identity := env_var_or_default("PIXIE_SIGN_IDENTITY", "-")

default:
    @just --list

build:
    cargo build --release

app: build
    rm -rf "{{bundle_dir}}"
    mkdir -p "{{bundle_dir}}/Contents/MacOS" "{{bundle_dir}}/Contents/Resources"
    cp "{{binary_path}}" "{{bundle_dir}}/Contents/MacOS/pixie"
    cp "{{plist_path}}" "{{bundle_dir}}/Contents/Info.plist"
    if [[ -f "{{icon_path}}" ]]; then cp "{{icon_path}}" "{{bundle_dir}}/Contents/Resources/AppIcon.icns"; fi
    chmod +x "{{bundle_dir}}/Contents/MacOS/pixie"

sign identity=sign_identity: app
    codesign --force --deep --sign "{{identity}}" "{{bundle_dir}}"
    codesign --verify --deep --strict --verbose=2 "{{bundle_dir}}"

signed-app: sign
