#!/usr/bin/env bash
# Run from MSYS2 UCRT64.
set -euo pipefail
[[ ${MSYSTEM:-} == UCRT64 ]] || { echo "Run this in MSYS2 UCRT64" >&2; exit 1; }
cd "$(dirname "$0")/.."
root=$PWD
version=$(sed -nE 's/^version = "([^"]+)"$/\1/p' Cargo.toml)
name="learn-swedish-$version-windows-x64"
cargo build --locked --release --target-dir target/windows-ucrt64

work=$(mktemp -d)
trap 'rm -rf -- "$work"' EXIT
bundle="$work/$name"
mkdir -p "$bundle/bin" "$bundle/licenses" "$root/dist"
system32="$(cygpath -u "$SYSTEMROOT")/System32"
declare -A packages

copy_from_prefix() {
    local relative=$1 owner
    install -D "/ucrt64/$relative" "$bundle/$relative"
    owner=$(pacman -Qoq "/ucrt64/$relative")
    packages[$owner]=1
}

include_imports() {
    local imports dll source
    imports=$(objdump -p "$1")
    while IFS= read -r dll; do
        if [[ -f "$bundle/bin/$dll" ]]; then
            continue
        fi
        source="/ucrt64/bin/$dll"
        if [[ -f "$source" ]]; then
            copy_from_prefix "bin/$dll"
            include_imports "$bundle/bin/$dll"
        elif [[ ! ${dll,,} =~ ^(api-ms-win-|ext-ms-win-) && ! -f "$system32/$dll" ]]; then
            printf 'Missing Windows import %s (%s)\n' "$dll" "$1" >&2
            return 1
        fi
    done < <(awk '/DLL Name:/ {print $3}' <<< "$imports")
}

cp target/windows-ucrt64/release/learn-swedish.exe "$bundle/bin/"
include_imports "$bundle/bin/learn-swedish.exe"
for plugin in coreelements wavparse audioconvert audioresample directsound; do
    relative="lib/gstreamer-1.0/libgst$plugin.dll"
    copy_from_prefix "$relative"
    include_imports "$bundle/$relative"
done

relative=libexec/gstreamer-1.0/gst-plugin-scanner.exe
copy_from_prefix "$relative"
include_imports "$bundle/$relative"

for theme in Adwaita hicolor; do
    copy_from_prefix "share/icons/$theme/index.theme"
    if [[ -d "/ucrt64/share/icons/$theme/symbolic" ]]; then
        cp -RL "/ucrt64/share/icons/$theme/symbolic" "$bundle/share/icons/$theme/"
    fi
done

for schema in /ucrt64/share/glib-2.0/schemas/org.gtk.gtk4.*.gschema.xml; do
    copy_from_prefix "${schema#/ucrt64/}"
done
glib-compile-schemas --strict "$bundle/share/glib-2.0/schemas"

# SVG icons can be loaded dynamically; these imports are not visible in GTK's PE headers.
for loader in /ucrt64/lib/gdk-pixbuf-2.0/*/loaders/*svg*.dll; do
    [[ -f "$loader" ]] || continue
    relative=${loader#/ucrt64/}
    copy_from_prefix "$relative"
    include_imports "$bundle/$relative"
done

{
    printf 'Bundled MSYS2 UCRT64 packages (including unmodified redistributable DLLs):\n\n'
    for package in $(printf '%s\n' "${!packages[@]}" | LC_ALL=C sort); do
        pacman -Qi "$package" | sed -nE '/^(Name|Version|URL|Licenses) +:/p'
        printf 'Package / corresponding source: https://packages.msys2.org/package/%s\n\n' "$package"
        license_dir="/ucrt64/share/licenses/${package#mingw-w64-ucrt-x86_64-}"
        if [[ -d "$license_dir" ]]; then
            cp -R "$license_dir" "$bundle/licenses/"
        fi
    done
} > "$bundle/THIRD-PARTY.txt"
cp /ucrt64/share/licenses/glib2/COPYING "$bundle/licenses/LGPL-2.1.txt"

cargo vendor --locked --versioned-dirs "$work/crates" > /dev/null
for crate in "$work"/crates/*; do
    while IFS= read -r file; do
        install -D "$file" "$bundle/licenses/rust/${crate##*/}/${file##*/}"
    done < <(find "$crate" -maxdepth 1 -type f \
        \( -iname 'license*' -o -iname 'copying*' -o -iname 'notice*' -o -iname 'copyright*' \))
done
cp README.md LICENSE scripts/install_windows_speech.ps1 "$bundle/"
mkdir -p "$bundle/stories" "$bundle/docs"
cp stories/*.xml stories/*.xsd "$bundle/stories/"
cp docs/story-xml-spec.md "$bundle/docs/"

(cd "$work" && zip -q -r "$name.zip" "$name")
mv -f "$work/$name.zip" "$root/dist/$name.zip"
printf 'Built dist/%s.zip\n' "$name"
