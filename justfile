nightly := "nightly-2026-06-16"
wild-version := "0.10.0"
wgsl-test-version := "0.2.35"
cargo-deny-version := "0.20.2"
cargo-about-version := "0.9.2"
reuse-version := "6.2.0"

version := `cargo metadata --format-version 1 --no-deps | jq -r '.packages[] | select(.name == "lapiz_app") | .version'`
sha := `git rev-parse HEAD | cut -c1-7`
target := `rustc -vV | sed -n 's/^host: //p'`
os-name := os()

default:
    @just --list

setup: setup-rust setup-format

setup-rust:
    cargo --version

setup-format:
    rustup toolchain install {{nightly}} --profile minimal --component rustfmt --no-self-update

setup-package:
    #!/usr/bin/env bash
    set -euo pipefail
    if [ "$(cargo about --version 2>/dev/null || true)" != "cargo-about {{cargo-about-version}}" ]; then
        RUSTFLAGS="" cargo install cargo-about --locked --version {{cargo-about-version}}
    fi

setup-deny:
    #!/usr/bin/env bash
    set -euo pipefail
    if [ "$(cargo deny --version 2>/dev/null || true)" != "cargo-deny {{cargo-deny-version}}" ]; then
        RUSTFLAGS="" cargo install cargo-deny --locked --version {{cargo-deny-version}}
    fi

setup-reuse:
    #!/usr/bin/env bash
    set -euo pipefail
    if ! reuse --version 2>/dev/null | grep -Fq "{{reuse-version}}"; then
        pipx install --force "reuse[charset-normalizer]=={{reuse-version}}"
    fi

setup-linux:
    #!/usr/bin/env bash
    set -euo pipefail
    if [ "{{os-name}}" != "linux" ]; then
        echo "setup-linux: skipped on {{os-name}}"
        exit 0
    fi
    sudo apt-get update
    sudo apt-get install -y --no-install-recommends \
        clang pkg-config libx11-dev libxkbcommon-dev libxkbcommon-x11-dev \
        libwayland-dev libxcb1-dev libxcb-render0-dev libxcb-shape0-dev \
        libxcb-xfixes0-dev libfontconfig1-dev libudev-dev libdbus-1-dev \
        libasound2-dev libegl1-mesa-dev libgbm-dev
    if ! wild --version 2>/dev/null | grep -Fq "{{wild-version}}"; then
        RUSTFLAGS="" cargo install wild-linker --locked --version {{wild-version}}
    fi
    wild --version

fmt: setup-format
    cargo +{{nightly}} fmt --all

fmt-check: setup-format
    cargo +{{nightly}} fmt --all -- --check

lint:
    cargo clippy --workspace --all-targets --locked -- -D warnings

test:
    cargo test --workspace --all-targets --locked

test-doc:
    cargo test --workspace --doc --locked

test-wgsl:
    npx --yes wgsl-test@{{wgsl-test-version}} run --projectDir crates/lapiz_color

check: fmt-check lint test test-doc test-wgsl

deny: setup-deny
    cargo deny check advisories bans licenses sources

reuse: setup-reuse
    reuse lint

build profile:
    #!/usr/bin/env bash
    set -euo pipefail
    case "{{profile}}" in
        dev) cargo build --locked ;;
        release) cargo build --release --locked ;;
        *) echo "profile must be dev or release" >&2; exit 2 ;;
    esac

run profile:
    #!/usr/bin/env bash
    set -euo pipefail
    case "{{profile}}" in
        dev) cargo run --locked ;;
        release) cargo run --release --locked ;;
        *) echo "profile must be dev or release" >&2; exit 2 ;;
    esac

verify-release-tag tag:
    #!/usr/bin/env bash
    set -euo pipefail
    expected="v{{version}}"
    if [ "{{tag}}" != "$expected" ]; then
        echo "tag {{tag}} does not match lapiz_app version $expected" >&2
        exit 1
    fi

package profile: check (build profile)
    #!/usr/bin/env bash
    set -euo pipefail

    case "{{profile}}" in
        dev)
            bindir=target/debug
            ver="{{version}}-dev"
            ;;
        release)
            bindir=target/release
            ver="{{version}}"
            ;;
        *)
            echo "profile must be dev or release" >&2
            exit 2
            ;;
    esac

    host_target="{{target}}"
    case "$host_target" in
        x86_64-*) arch=x86_64 ;;
        aarch64-*) arch=arm64 ;;
        *) arch="${host_target%%-*}" ;;
    esac

    if [ "{{os-name}}" = "windows" ]; then
        binary=lapiz_app.exe
        ext=zip
    else
        binary=lapiz_app
        ext=tar.gz
    fi

    name="lapiz-$ver-{{sha}}-{{os-name}}-$arch"
    staging="target/pack/$name"
    archive="target/pack/$name.$ext"
    checksum="target/pack/$name.sha256"
    third_party="target/pack/THIRD_PARTY_LICENSES.html"

    case "$staging" in
        target/pack/lapiz-*) ;;
        *) echo "unsafe staging path: $staging" >&2; exit 1 ;;
    esac

    mkdir -p target/pack
    if [ -e "$staging" ]; then
        find "$staging" -depth -delete
    fi
    mkdir -p "$staging"

    cp "$bindir/$binary" "$staging/"
    cp README.md LICENSE "$staging/"
    cp LICENSES/MIT.txt "$staging/MIT.txt"

    cargo about generate about.hbs --output-file "$third_party"
    cp "$third_party" "$staging/"

    # Include only tracked assets that are not matched by .gitignore.
    # During local development, we may introduce some external assets for testing
    # like bundles created by someone else. They should not be included in the
    # packaged output.
    while IFS= read -r -d '' source; do
        if git check-ignore --no-index -q -- "$source"; then
            echo "Excluded ignored asset: $source"
            continue
        fi
        destination="$staging/$source"
        mkdir -p "$(dirname "$destination")"
        cp "$source" "$destination"
    done < <(git ls-files -z -- assets)

    rm -f "$archive" "$checksum"
    if [ "$ext" = zip ]; then
        /c/Windows/System32/tar.exe -C target/pack -caf "$archive" "$name"
    else
        tar -C target/pack -czf "$archive" "$name"
    fi

    if command -v sha256sum >/dev/null; then
        (cd target/pack && sha256sum "$name.$ext" > "$name.sha256")
    else
        (cd target/pack && shasum -a 256 "$name.$ext" > "$name.sha256")
    fi

    echo "Packaged: $archive + $checksum"
