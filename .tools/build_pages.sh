#!/usr/bin/env bash
set -euo pipefail

pages_dir="target/pages"

build_demo() {
    local slug="$1"
    local title="$2"
    local current_version="$3"
    local crate_dir="$4"
    local out_name="$5"
    shift 5

    local demo_dir="$pages_dir/demos/$slug"
    mkdir -p "$demo_dir"

    write_redirect "$demo_dir/index.html" "$title" "./$current_version/"
    printf '{"version":"%s","url":"./%s/"}\n' "$current_version" "$current_version" > "$demo_dir/current.json"

    for version in "$@"; do
        local source_dir="pages/demos/$slug/$version"
        local output_dir="$demo_dir/$version"

        if [[ ! -d "$source_dir" ]]; then
            printf 'missing page source: %s\n' "$source_dir" >&2
            exit 1
        fi

        mkdir -p "$output_dir"
        cp -R "$source_dir/." "$output_dir/"
        env RUSTFLAGS="-D warnings" wasm-pack build "$crate_dir" \
            --target web \
            --out-dir "$(pwd)/$output_dir/pkg" \
            --out-name "$out_name"
    done
}

write_redirect() {
    local path="$1"
    local title="$2"
    local target="$3"

    cat > "$path" <<HTML
<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1" />
  <meta http-equiv="refresh" content="0; url=$target" />
  <link rel="canonical" href="$target" />
  <title>$title</title>
</head>
<body>
  <p><a href="$target">Open $title</a></p>
</body>
</html>
HTML
}

write_site_index() {
    local path="$pages_dir/index.html"

    cat > "$path" <<'HTML'
<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1" />
  <title>Linkage Blaze Demos</title>
</head>
<body>
  <main>
    <h1>Linkage Blaze Demos</h1>
    <ul>
      <li><a href="./demos/armatron/">Armatron</a></li>
      <li><a href="./demos/skeleton-clock/">Skeleton Clock</a></li>
      <li><a href="./demos/dancer/">Dancer</a></li>
    </ul>
  </main>
</body>
</html>
HTML
}

write_demos_index() {
    local path="$pages_dir/demos/index.html"

    cat > "$path" <<'HTML'
<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1" />
  <title>Linkage Blaze Demos</title>
</head>
<body>
  <main>
    <h1>Linkage Blaze Demos</h1>
    <ul>
      <li><a href="./armatron/">Armatron</a> (<a href="./armatron/v1/">v1</a>)</li>
      <li><a href="./skeleton-clock/">Skeleton Clock</a> (<a href="./skeleton-clock/v1/">v1</a>)</li>
      <li><a href="./dancer/">Dancer</a> (<a href="./dancer/v1/">v1</a>)</li>
    </ul>
  </main>
</body>
</html>
HTML
}

rm -rf "$pages_dir"
mkdir -p "$pages_dir/demos"

write_site_index
write_demos_index

build_demo \
    "armatron" \
    "Armatron" \
    "v1" \
    "crates/linkage-blaze-armatron-wasm" \
    "linkage_blaze_armatron_wasm" \
    "v1"

build_demo \
    "skeleton-clock" \
    "Skeleton Clock" \
    "v1" \
    "crates/linkage-blaze-skeleton-clock-wasm" \
    "linkage_blaze_skeleton_clock_wasm" \
    "v1"

build_demo \
    "dancer" \
    "Dancer" \
    "v1" \
    "crates/linkage-blaze-classic-wasm" \
    "linkage_blaze_classic_wasm" \
    "v1"

printf 'Wrote %s\n' "$pages_dir"
