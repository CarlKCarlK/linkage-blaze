use std::env;
use std::ffi::OsStr;
use std::fmt::{self, Display, Formatter};
use std::fs;
use std::path::Path;
use std::process::Command;

const PAGES_DIR: &str = "target/pages";
const MANIFEST_PATH: &str = "pages/demos.tsv";
const PREVIEW_EXAMPLE_CRATE: &str = "linkage-blaze-example-core";

fn main() {
    if let Err(error) = inner_main() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn inner_main() -> Result<()> {
    let mut arguments = env::args().skip(1);
    let Some(subcommand) = arguments.next() else {
        return Err(Error::message(usage()));
    };

    match subcommand.as_str() {
        "build-pages" => {
            let selected_demo = arguments
                .next()
                .filter(|selected_demo| !selected_demo.is_empty());
            if arguments.next().is_some() {
                return Err(Error::message(
                    "usage: cargo run -p linkage-blaze-xtask -- build-pages [demo-slug]",
                ));
            }
            build_pages(selected_demo.as_deref())
        }
        "bump-demo-version" => {
            let Some(demo_slug) = arguments.next() else {
                return Err(Error::message(
                    "usage: cargo run -p linkage-blaze-xtask -- bump-demo-version <demo-slug> [new-version]",
                ));
            };
            let requested_version = arguments
                .next()
                .filter(|requested_version| !requested_version.is_empty());
            if arguments.next().is_some() {
                return Err(Error::message(
                    "usage: cargo run -p linkage-blaze-xtask -- bump-demo-version <demo-slug> [new-version]",
                ));
            }
            bump_demo_version(&demo_slug, requested_version.as_deref())
        }
        _ => Err(Error::message(usage())),
    }
}

fn usage() -> &'static str {
    "usage:\n  cargo run -p linkage-blaze-xtask -- build-pages [demo-slug]\n  cargo run -p linkage-blaze-xtask -- bump-demo-version <demo-slug> [new-version]"
}

fn build_pages(selected_demo: Option<&str>) -> Result<()> {
    let repo_root = env::current_dir()?;
    let demos = load_manifest(Path::new(MANIFEST_PATH))?;
    let pages_dir = Path::new(PAGES_DIR);

    remove_dir_if_exists(pages_dir)?;
    fs::create_dir_all(pages_dir.join("demos"))?;

    let mut demos_index_body = String::new();

    for demo_record in demos {
        if selected_demo.is_some_and(|selected_demo| demo_record.slug != selected_demo) {
            continue;
        }

        demos_index_body.push_str(&demo_record.demo_card_html()?);
        build_demo(&repo_root, pages_dir, &demo_record)?;
        capture_demo_preview(&repo_root, pages_dir, &demo_record)?;
    }

    if demos_index_body.is_empty() {
        return Err(Error::message("no demos selected for build"));
    }

    write_redirect(
        &pages_dir.join("index.html"),
        "Linkage Blaze Demos",
        "./demos/",
    )?;
    write_demos_index_file(&pages_dir.join("demos/index.html"), &demos_index_body)?;

    println!("Wrote {}", pages_dir.display());
    Ok(())
}

fn bump_demo_version(demo_slug: &str, requested_version: Option<&str>) -> Result<()> {
    let manifest_path = Path::new(MANIFEST_PATH);
    let mut demos = load_manifest(manifest_path)?;
    let Some(demo_record) = demos
        .iter_mut()
        .find(|demo_record_ref| demo_record_ref.slug == demo_slug)
    else {
        return Err(Error::message(format!("unknown demo: {demo_slug}")));
    };
    let previous_version = demo_record.current_version.clone();

    let new_version = if let Some(requested_version) = requested_version {
        requested_version.to_owned()
    } else {
        infer_next_version(&demo_record.current_version).map_err(Error::message)?
    };

    validate_version(&new_version).map_err(Error::message)?;

    let new_snapshot_dir = Path::new("pages/demos")
        .join(&demo_record.slug)
        .join(&new_version);
    if new_snapshot_dir.exists() {
        return Err(Error::message(format!(
            "version already exists: {}",
            new_snapshot_dir.display()
        )));
    }

    let source_dir = Path::new(&demo_record.source_dir);
    if !source_dir.is_dir() {
        return Err(Error::message(format!(
            "missing source dir: {}",
            source_dir.display()
        )));
    }

    fs::create_dir_all(&new_snapshot_dir)?;
    copy_directory_contents_filtered(source_dir, &new_snapshot_dir, |entry_name| {
        entry_name != OsStr::new("pkg")
    })?;

    if !demo_record
        .versions
        .iter()
        .any(|version| version == &new_version)
    {
        demo_record.versions.push(new_version.clone());
    }
    demo_record.current_version = new_version.clone();

    write_manifest(manifest_path, &demos)?;

    println!(
        "Created {} from {}",
        new_snapshot_dir.display(),
        previous_version
    );
    println!(
        "Updated {} current version to {}",
        manifest_path.display(),
        new_version
    );

    Ok(())
}

fn load_manifest(path: &Path) -> Result<Vec<DemoRecord>> {
    if !path.is_file() {
        return Err(Error::message(format!(
            "missing manifest: {}",
            path.display()
        )));
    }

    let manifest = fs::read_to_string(path)?;
    manifest
        .lines()
        .enumerate()
        .filter(|(_, line)| !line.trim().is_empty())
        .map(|(line_index, line)| DemoRecord::from_tsv_line(line, line_index + 1))
        .collect()
}

fn write_manifest(path: &Path, demos: &[DemoRecord]) -> Result<()> {
    let mut manifest = String::new();
    for demo_record in demos {
        manifest.push_str(&demo_record.to_tsv_line());
        manifest.push('\n');
    }
    fs::write(path, manifest)?;
    Ok(())
}

fn build_demo(repo_root: &Path, pages_dir: &Path, demo_record: &DemoRecord) -> Result<()> {
    let demo_dir = pages_dir.join("demos").join(&demo_record.slug);
    fs::create_dir_all(&demo_dir)?;

    write_redirect(
        &demo_dir.join("index.html"),
        &demo_record.title,
        &format!("./{}/", demo_record.current_version),
    )?;
    fs::write(
        demo_dir.join("current.json"),
        format!(
            "{{\"version\":\"{}\",\"url\":\"./{}/\"}}\n",
            demo_record.current_version, demo_record.current_version
        ),
    )?;

    for version in &demo_record.versions {
        let source_dir = Path::new("pages/demos")
            .join(&demo_record.slug)
            .join(version);
        if !source_dir.is_dir() {
            return Err(Error::message(format!(
                "missing page source: {}",
                source_dir.display()
            )));
        }

        let output_dir = demo_dir.join(version);
        fs::create_dir_all(&output_dir)?;
        copy_directory_contents(&source_dir, &output_dir)?;

        let output_dir = repo_root.join(&output_dir);
        let mut command = Command::new("wasm-pack");
        command.env("RUSTFLAGS", "-D warnings");
        command.arg("build");
        command.arg(&demo_record.crate_dir);
        command.arg("--target");
        command.arg("web");
        command.arg("--out-dir");
        command.arg(&output_dir.join("pkg"));
        command.arg("--out-name");
        command.arg(&demo_record.out_name);
        run_command(
            &mut command,
            &format!("wasm-pack build {}", demo_record.crate_dir),
        )?;
    }

    Ok(())
}

fn capture_demo_preview(
    repo_root: &Path,
    pages_dir: &Path,
    demo_record: &DemoRecord,
) -> Result<()> {
    let preview_spec = demo_record.preview_spec()?;
    let preview_path = repo_root
        .join(pages_dir)
        .join("demos")
        .join(&demo_record.slug)
        .join("preview.png");

    let mut command = Command::new("cargo");
    command.env("LINKAGE_BLAZE_PREVIEW_OUTPUT_PATH", &preview_path);
    command.arg("test");
    command.arg("--quiet");
    command.arg("-p");
    command.arg(PREVIEW_EXAMPLE_CRATE);
    command.arg("--features");
    command.arg(preview_spec.feature);
    command.arg("--lib");
    command.arg("--");
    command.arg("--exact");
    command.arg(preview_spec.test_name);
    run_command(
        &mut command,
        &format!("cargo test preview {}", demo_record.slug),
    )?;

    let preview_metadata = fs::metadata(&preview_path).map_err(|_| {
        Error::message(format!("failed to render preview for {}", demo_record.slug))
    })?;
    if preview_metadata.len() == 0 {
        return Err(Error::message(format!(
            "failed to render preview for {}",
            demo_record.slug
        )));
    }

    Ok(())
}

fn write_demos_index_file(path: &Path, body: &str) -> Result<()> {
    let html = DEMOS_INDEX_TEMPLATE.replace("$body", body);
    fs::write(path, html)?;
    Ok(())
}

fn write_redirect(path: &Path, title: &str, target: &str) -> Result<()> {
    let html = REDIRECT_TEMPLATE
        .replace("$title", title)
        .replace("$target", target);
    fs::write(path, html)?;
    Ok(())
}

fn copy_directory_contents(source_dir: &Path, destination_dir: &Path) -> Result<()> {
    copy_directory_contents_filtered(source_dir, destination_dir, |_| true)
}

fn copy_directory_contents_filtered<F>(
    source_dir: &Path,
    destination_dir: &Path,
    include_entry: F,
) -> Result<()>
where
    F: Fn(&OsStr) -> bool,
{
    for entry in fs::read_dir(source_dir)? {
        let entry = entry?;
        if !include_entry(&entry.file_name()) {
            continue;
        }

        let entry_type = entry.file_type()?;
        let destination_path = destination_dir.join(entry.file_name());
        if entry_type.is_dir() {
            fs::create_dir_all(&destination_path)?;
            copy_directory_contents(&entry.path(), &destination_path)?;
        } else if entry_type.is_file() {
            fs::copy(entry.path(), destination_path)?;
        } else {
            return Err(Error::message(format!(
                "unsupported filesystem entry: {}",
                entry.path().display()
            )));
        }
    }

    Ok(())
}

fn remove_dir_if_exists(path: &Path) -> Result<()> {
    match fs::remove_dir_all(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(Error::from(error)),
    }
}

fn run_command(command: &mut Command, description: &str) -> Result<()> {
    let status = command.status()?;
    if status.success() {
        return Ok(());
    }

    Err(Error::message(format!(
        "command failed ({description}): {status}"
    )))
}

fn infer_next_version(current_version: &str) -> std::result::Result<String, String> {
    let Some(version_digits) = current_version.strip_prefix('v') else {
        return Err(format!(
            "cannot infer next version from current version: {current_version}"
        ));
    };
    let version_number = version_digits.parse::<u32>().map_err(|_| {
        format!("cannot infer next version from current version: {current_version}")
    })?;
    Ok(format!("v{}", version_number + 1))
}

fn validate_version(version: &str) -> std::result::Result<(), String> {
    let Some(version_digits) = version.strip_prefix('v') else {
        return Err(format!("version must look like v2 or v17: {version}"));
    };
    if version_digits.is_empty() || !version_digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(format!("version must look like v2 or v17: {version}"));
    }
    Ok(())
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct DemoRecord {
    slug: String,
    title: String,
    current_version: String,
    crate_dir: String,
    source_dir: String,
    out_name: String,
    versions: Vec<String>,
}

impl DemoRecord {
    fn from_tsv_line(line: &str, line_number: usize) -> Result<Self> {
        let fields: Vec<_> = line.split('\t').collect();
        if fields.len() != 7 {
            return Err(Error::message(format!(
                "invalid manifest record on line {line_number}: expected 7 tab-separated fields"
            )));
        }

        Ok(Self {
            slug: fields[0].to_owned(),
            title: fields[1].to_owned(),
            current_version: fields[2].to_owned(),
            crate_dir: fields[3].to_owned(),
            source_dir: fields[4].to_owned(),
            out_name: fields[5].to_owned(),
            versions: fields[6]
                .split(',')
                .filter(|version| !version.is_empty())
                .map(ToOwned::to_owned)
                .collect(),
        })
    }

    fn to_tsv_line(&self) -> String {
        format!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}",
            self.slug,
            self.title,
            self.current_version,
            self.crate_dir,
            self.source_dir,
            self.out_name,
            self.versions.join(","),
        )
    }

    fn preview_spec(&self) -> Result<PreviewSpec> {
        let preview_spec = match self.slug.as_str() {
            "armatron" => PreviewSpec {
                feature: "armatron",
                test_name: "armatron::tests::armatron_renders_expected_frame",
                orientation: PreviewOrientation::Landscape,
            },
            "skeleton-clock" => PreviewSpec {
                feature: "skeleton-clock",
                test_name: "skeleton_clock::tests::skeleton_clock_renders_expected_frame",
                orientation: PreviewOrientation::Portrait,
            },
            "ballet" => PreviewSpec {
                feature: "ballet",
                test_name: "ballet::tests::ballet_renders_expected_frame",
                orientation: PreviewOrientation::Portrait,
            },
            "clock" => PreviewSpec {
                feature: "clock",
                test_name: "clock::tests::clock_renders_expected_frame",
                orientation: PreviewOrientation::Landscape,
            },
            _ => {
                return Err(Error::message(format!(
                    "missing preview metadata for demo: {}",
                    self.slug
                )));
            }
        };
        Ok(preview_spec)
    }

    fn demo_card_html(&self) -> Result<String> {
        let preview_spec = self.preview_spec()?;
        let latest_url = format!("./{}/{}/", self.slug, self.current_version);
        let versions: Vec<_> = self
            .versions
            .iter()
            .rev()
            .map(|version| {
                format!(
                    "            <option value=\"./{}/{}/\">{}</option>\n",
                    self.slug, version, version
                )
            })
            .collect();

        let mut html = format!(
            "      <article class=\"demo-card demo-card--{slug}\">\n\
        <div class=\"demo-card__header\">\n\
          <div>\n\
            <p class=\"demo-card__eyebrow\">Preview</p>\n\
            <h2><a href=\"{latest_url}\">{title}</a></h2>\n\
          </div>\n\
          <a class=\"demo-card__open\" href=\"{latest_url}\">Open latest</a>\n\
        </div>\n\
        <a class=\"demo-card__preview demo-card__preview--{orientation}\" href=\"{latest_url}\">\n\
          <img\n\
            src=\"./{slug}/preview.png\"\n\
            alt=\"{title} preview\"\n\
            loading=\"lazy\"\n\
          />\n\
        </a>\n",
            slug = self.slug,
            latest_url = latest_url,
            title = self.title,
            orientation = preview_spec.orientation.class_name(),
        );

        if self.versions.len() > 1 {
            html.push_str(
                "        <div class=\"demo-card__footer\">\n\
          <label class=\"demo-card__versions\">\n\
            <span>Versions</span>\n\
            <select onchange=\"if (this.value) window.location.href = this.value;\">\n",
            );
            for version in versions {
                html.push_str(&version);
            }
            html.push_str(
                "            </select>\n\
          </label>\n\
        </div>\n",
            );
        } else {
            html.push_str(
                "        <div class=\"demo-card__footer\">\n\
          <span class=\"demo-card__latest\">Latest: ",
            );
            html.push_str(&self.current_version);
            html.push_str(
                "</span>\n\
        </div>\n",
            );
        }

        html.push_str("      </article>\n");

        Ok(html)
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum PreviewOrientation {
    Landscape,
    Portrait,
}

impl PreviewOrientation {
    fn class_name(self) -> &'static str {
        match self {
            Self::Landscape => "landscape",
            Self::Portrait => "portrait",
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
struct PreviewSpec {
    feature: &'static str,
    test_name: &'static str,
    orientation: PreviewOrientation,
}

type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
enum Error {
    Io(std::io::Error),
    Message(String),
}

impl Error {
    fn message(message: impl Into<String>) -> Self {
        Self::Message(message.into())
    }
}

impl From<std::io::Error> for Error {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

impl Display for Error {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "{error}"),
            Self::Message(message) => formatter.write_str(message),
        }
    }
}

impl std::error::Error for Error {}

const REDIRECT_TEMPLATE: &str = r#"<!doctype html>
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
"#;

const DEMOS_INDEX_TEMPLATE: &str = r#"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1" />
  <title>Linkage Blaze Demos</title>
  <style>
    :root {
      color-scheme: light;
      --bg: rgb(245, 242, 234); /* warm linen */
      --panel: rgba(255, 251, 245, 0.82); /* parchment white */
      --panel-strong: rgb(255, 252, 247); /* ivory */
      --line: rgba(68, 48, 34, 0.16); /* soft umber */
      --ink: rgb(34, 28, 24); /* espresso brown */
      --muted: rgb(102, 86, 71); /* walnut taupe */
      --accent: rgb(184, 90, 42); /* burnt orange */
      --accent-deep: rgb(134, 58, 22); /* rust */
      --shadow: 0 24px 70px rgba(88, 62, 43, 0.14);
    }

    * {
      box-sizing: border-box;
    }

    html {
      min-height: 100%;
      background:
        radial-gradient(circle at top left, rgba(214, 132, 70, 0.18), transparent 36%),
        radial-gradient(circle at top right, rgba(83, 123, 114, 0.16), transparent 32%),
        linear-gradient(180deg, rgb(250, 247, 241) 0%, var(--bg) 100%);
    }

    body {
      margin: 0;
      min-height: 100vh;
      color: var(--ink);
      font-family: "Iowan Old Style", "Palatino Linotype", "Book Antiqua", Palatino, serif;
    }

    .page {
      width: min(1280px, calc(100vw - 32px));
      margin: 0 auto;
      padding: 48px 0 72px;
    }

    .hero {
      display: grid;
      gap: 18px;
      margin-bottom: 36px;
      padding: 28px;
      border: 1px solid var(--line);
      border-radius: 28px;
      background: linear-gradient(135deg, rgba(255, 252, 247, 0.92), rgba(245, 238, 228, 0.78));
      box-shadow: var(--shadow);
      backdrop-filter: blur(10px);
    }

    .hero__eyebrow {
      margin: 0;
      color: var(--accent-deep);
      font: 700 0.78rem/1.2 "Trebuchet MS", "Gill Sans", sans-serif;
      letter-spacing: 0.14em;
      text-transform: uppercase;
    }

    .hero h1 {
      margin: 0;
      font-size: clamp(2.4rem, 5vw, 4.8rem);
      line-height: 0.96;
      letter-spacing: -0.04em;
    }

    .hero p {
      max-width: 58rem;
      margin: 0;
      color: var(--muted);
      font-size: 1.05rem;
      line-height: 1.55;
    }

    .hero__links {
      display: flex;
      flex-wrap: wrap;
      gap: 12px 18px;
    }

    .hero__links a {
      color: var(--accent-deep);
      font: 700 0.95rem/1.2 "Trebuchet MS", "Gill Sans", sans-serif;
    }

    .demo-grid {
      display: grid;
      grid-template-columns: repeat(auto-fit, minmax(280px, 1fr));
      gap: 22px;
    }

    .demo-card {
      --preview-aspect-ratio: 4 / 3;
      --preview-backdrop-start: rgb(250, 246, 239); /* warm off-white */
      --preview-backdrop-end: rgb(240, 228, 212); /* pale apricot cream */
      --preview-glow: rgba(196, 132, 78, 0.12); /* light amber wash */
      display: grid;
      gap: 12px;
      padding: 18px;
      border: 1px solid rgba(66, 44, 31, 0.14);
      border-radius: 24px;
      background: linear-gradient(180deg, rgba(255, 253, 249, 0.96), rgba(249, 244, 236, 0.9));
      box-shadow: 0 16px 38px rgba(83, 58, 38, 0.12);
    }

    .demo-card--skeleton-clock {
      --preview-aspect-ratio: 3 / 4;
    }

    .demo-card--ballet {
      --preview-aspect-ratio: 3 / 4;
    }

    .demo-card__header,
    .demo-card__footer {
      display: flex;
      align-items: center;
      justify-content: space-between;
      gap: 12px;
    }

    .demo-card__header {
      min-height: 64px;
      align-items: flex-start;
    }

    .demo-card__header > div {
      min-height: 64px;
      display: flex;
      flex-direction: column;
      justify-content: flex-start;
    }

    .demo-card__eyebrow {
      margin: 0 0 6px;
      color: var(--muted);
      font: 700 0.72rem/1.2 "Trebuchet MS", "Gill Sans", sans-serif;
      letter-spacing: 0.14em;
      text-transform: uppercase;
    }

    .demo-card h2 {
      margin: 0;
      font-size: 1.45rem;
      line-height: 1.05;
    }

    a {
      color: inherit;
      text-decoration: none;
    }

    a:hover {
      text-decoration: underline;
      text-decoration-thickness: 0.08em;
      text-underline-offset: 0.16em;
    }

    .demo-card__open,
    .demo-card__latest {
      flex: 0 0 auto;
      padding: 10px 14px;
      border: 1px solid rgba(123, 73, 42, 0.16);
      border-radius: 999px;
      background: rgba(255, 248, 239, 0.88);
      color: var(--accent-deep);
      font: 700 0.88rem/1.2 "Trebuchet MS", "Gill Sans", sans-serif;
      white-space: nowrap;
    }

    .demo-card__open {
      background: linear-gradient(180deg, rgb(211, 106, 53), rgb(171, 75, 35));
      color: rgb(255, 246, 239);
      border-color: rgba(128, 56, 24, 0.36);
      box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.22);
    }

    .demo-card__preview {
      position: relative;
      overflow: hidden;
      display: grid;
      place-items: center;
      padding: clamp(8px, 1.8vw, 12px);
      border: 1px solid rgba(70, 48, 34, 0.16);
      border-radius: 20px;
      background:
        radial-gradient(circle at top, rgba(255, 255, 255, 0.42), transparent 42%),
        radial-gradient(circle at bottom, var(--preview-glow), transparent 52%),
        linear-gradient(160deg, var(--preview-backdrop-start), var(--preview-backdrop-end));
      aspect-ratio: var(--preview-aspect-ratio);
      box-shadow:
        inset 0 1px 0 rgba(255, 255, 255, 0.76),
        inset 0 0 0 1px rgba(198, 154, 118, 0.16);
    }

    .demo-card__preview--portrait {
      width: 82%;
      justify-self: center;
    }

    .demo-card__preview::before {
      content: "";
      position: absolute;
      inset: clamp(5px, 1vw, 7px);
      border-radius: 15px;
      box-shadow:
        inset 0 0 0 1px rgba(68, 44, 28, 0.14),
        inset 0 0 0 2px rgba(14, 10, 8, 0.05);
      pointer-events: none;
    }

    .demo-card__preview img {
      width: 100%;
      height: 100%;
      object-fit: contain;
      display: block;
      filter: drop-shadow(0 16px 28px rgba(54, 34, 20, 0.18));
    }

    .demo-card__versions {
      display: inline-flex;
      align-items: center;
      gap: 8px;
      color: var(--muted);
      font: 700 0.82rem/1.2 "Trebuchet MS", "Gill Sans", sans-serif;
      white-space: nowrap;
    }

    .demo-card__versions select {
      padding: 9px 32px 9px 12px;
      border: 1px solid rgba(93, 64, 45, 0.18);
      border-radius: 999px;
      background: rgb(255, 251, 246);
      color: var(--ink);
      font: inherit;
    }

    @media (max-width: 720px) {
      .page {
        width: min(100vw - 20px, 1280px);
        padding: 24px 0 48px;
      }

      .hero {
        padding: 22px;
        border-radius: 22px;
      }

      .demo-card {
        padding: 16px;
      }

      .demo-card__header,
      .demo-card__footer {
        flex-direction: column;
        align-items: stretch;
      }

      .demo-card__open,
      .demo-card__latest {
        text-align: center;
      }

      .demo-card__versions {
        justify-content: space-between;
      }

      .demo-card__versions select {
        width: 100%;
      }
    }
  </style>
</head>
<body>
  <main class="page">
    <section class="hero">
      <p class="hero__eyebrow">Interactive Demo Gallery</p>
      <h1>Linkage Blaze</h1>
      <p>Preview the current browser builds directly in the catalog, then jump into the latest version of each simulation. Version selectors list newer snapshots first so older builds stay available without dominating the page.</p>
      <div class="hero__links">
        <a href="https://github.com/CarlKCarlK/linkage-blaze" target="_blank" rel="noopener">GitHub: CarlKCarlK/linkage-blaze</a>
        <a href="https://medium.com/@carlmkadie" target="_blank" rel="noopener">Articles: @carlmkadie on Medium</a>
      </div>
    </section>
    <section class="demo-grid">
$body
    </section>
  </main>
</body>
</html>
"#;

#[cfg(test)]
mod tests {
    use super::{DemoRecord, PreviewOrientation, infer_next_version, validate_version};

    #[test]
    fn parses_manifest_line() {
        let demo_record =
            DemoRecord::from_tsv_line("armatron\tArmatron\tv2\tcrate\twww\toutput\tv1,v2", 1)
                .expect("manifest line should parse");

        assert_eq!(demo_record.slug, "armatron");
        assert_eq!(demo_record.current_version, "v2");
        assert_eq!(demo_record.versions, ["v1", "v2"]);
    }

    #[test]
    fn infers_next_version() {
        assert_eq!(
            infer_next_version("v17").expect("version should increment"),
            "v18"
        );
    }

    #[test]
    fn rejects_invalid_version() {
        assert!(validate_version("17").is_err());
        assert!(validate_version("vx").is_err());
    }

    #[test]
    fn preview_orientation_matches_slug() {
        let demo_record = DemoRecord::from_tsv_line("clock\tClock\tv2\tcrate\twww\toutput\tv2", 1)
            .expect("manifest line should parse");

        assert_eq!(
            demo_record
                .preview_spec()
                .expect("preview metadata should exist")
                .orientation,
            PreviewOrientation::Landscape
        );
    }
}
