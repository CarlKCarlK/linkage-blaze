#!/usr/bin/env bash
set -euo pipefail

pages_dir="target/pages"
manifest_path="pages/demos.tsv"
selected_demo="${1:-}"

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

write_index_file() {
    local path="$1"
    local body="$2"

    cat > "$path" <<HTML
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
$body
    </ul>
  </main>
</body>
</html>
HTML
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

site_index_body=""
demos_index_body=""

rm -rf "$pages_dir"
mkdir -p "$pages_dir/demos"

if [[ ! -f "$manifest_path" ]]; then
    printf 'missing manifest: %s\n' "$manifest_path" >&2
    exit 1
fi

while IFS=$'\t' read -r slug title current_version crate_dir source_dir out_name version_list; do
    if [[ -z "$slug" ]]; then
        continue
    fi

    if [[ -n "$selected_demo" && "$slug" != "$selected_demo" ]]; then
        continue
    fi

    demo_versions_html=""
    IFS=',' read -ra versions <<< "$version_list"
    for version in "${versions[@]}"; do
        if [[ -z "$version" ]]; then
            continue
        fi
        demo_versions_html="${demo_versions_html}<a href=\"./$slug/$version/\">$version</a> "
    done

    site_index_body="${site_index_body}      <li><a href=\"./demos/$slug/\">$title</a> (latest: <a href=\"./demos/$slug/$current_version/\">$current_version</a>)</li>
"

    demos_index_body="${demos_index_body}      <li><a href=\"./$slug/\">$title</a> (${demo_versions_html% })</li>
"

    build_demo "$slug" "$title" "$current_version" "$crate_dir" "$out_name" "${versions[@]}"
done < "$manifest_path"

if [[ -z "$site_index_body" ]]; then
    printf 'no demos selected for build\n' >&2
    exit 1
fi

write_index_file "$pages_dir/index.html" "$site_index_body"
write_index_file "$pages_dir/demos/index.html" "$demos_index_body"

printf 'Wrote %s\n' "$pages_dir"
