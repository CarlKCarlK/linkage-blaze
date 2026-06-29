#!/usr/bin/env bash
set -euo pipefail

if [[ $# -lt 1 || $# -gt 2 ]]; then
    printf 'usage: %s <demo-slug> [new-version]\n' "$0" >&2
    exit 1
fi

demo_slug="$1"
requested_version="${2:-}"
manifest_path="pages/demos.tsv"

if [[ ! -f "$manifest_path" ]]; then
    printf 'missing manifest: %s\n' "$manifest_path" >&2
    exit 1
fi

demo_line="$(rg -n "^${demo_slug}\t" "$manifest_path" | cut -d: -f1 || true)"
if [[ -z "$demo_line" ]]; then
    printf 'unknown demo: %s\n' "$demo_slug" >&2
    exit 1
fi

IFS=$'\t' read -r slug title current_version crate_dir source_dir out_name version_list < <(sed -n "${demo_line}p" "$manifest_path")

if [[ -n "$requested_version" ]]; then
    new_version="$requested_version"
else
    if [[ "$current_version" =~ ^v([0-9]+)$ ]]; then
        new_version="v$((BASH_REMATCH[1] + 1))"
    else
        printf 'cannot infer next version from current version: %s\n' "$current_version" >&2
        exit 1
    fi
fi

if [[ ! "$new_version" =~ ^v[0-9]+$ ]]; then
    printf 'version must look like v2 or v17: %s\n' "$new_version" >&2
    exit 1
fi

if [[ -d "pages/demos/$slug/$new_version" ]]; then
    printf 'version already exists: pages/demos/%s/%s\n' "$slug" "$new_version" >&2
    exit 1
fi

if [[ ! -d "$source_dir" ]]; then
    printf 'missing source dir: %s\n' "$source_dir" >&2
    exit 1
fi

mkdir -p "pages/demos/$slug/$new_version"
find "$source_dir" -mindepth 1 -maxdepth 1 ! -name pkg -exec cp -R {} "pages/demos/$slug/$new_version/" \;

new_version_list="$version_list"
case ",$version_list," in
    *",$new_version,"*) ;;
    *) new_version_list="${version_list},${new_version}" ;;
esac

new_record="$(printf '%s\t%s\t%s\t%s\t%s\t%s\t%s' "$slug" "$title" "$new_version" "$crate_dir" "$source_dir" "$out_name" "$new_version_list")"
perl -0pi -e "s{^\\Q$slug\\E\\t.*\$}{$new_record}m" "$manifest_path"

printf 'Created pages/demos/%s/%s from %s\n' "$slug" "$new_version" "$current_version"
printf 'Updated %s current version to %s\n' "$manifest_path" "$new_version"
