use std::fs;
use std::io;
use std::path::Path;
use std::path::PathBuf;

use clap::Parser;
use indoc::indoc;
use orgize::Org;
use walkdir::WalkDir;

use blorg::page;

/// Command line arguments
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct Args {
    // Input
    #[arg(default_value = "site", env = "BLOG_DIR")]
    pub dir: PathBuf,

    // Configuration
    #[arg(long, env = "BLOG_ROOT_ADDRESS")]
    pub root_address: String,
    #[arg(long, env = "BLOG_TITLE")]
    pub title: String,
    #[arg(long, env = "BLOG_DESCRIPTION")]
    pub description: String,
    #[arg(long, default_value = "en-GB")]
    pub language: String,

    #[arg(long, default_value = "Info")]
    pub log_level: log::LevelFilter,

    // Output
    #[arg(long, default_value = "out", env = "OUTDIR")]
    pub outdir: PathBuf,

    #[arg(long, default_value = "false")]
    pub copy_older_files: bool,

    #[arg(long, default_value = "false")]
    pub minify_html: bool,
    #[arg(long, default_value = "false")]
    pub minifier_copy_on_failure: bool,
}

use std::time::SystemTime;

fn setup_logger(args: &Args) -> Result<(), fern::InitError> {
    fern::Dispatch::new()
        .format(|out, message, record| {
            out.finish(format_args!(
                "[{} {} {}] {}",
                humantime::format_rfc3339_seconds(SystemTime::now()),
                record.level(),
                record.target(),
                message
            ))
        })
        .level(args.log_level)
        .chain(io::stdout())
        .chain(fern::log_file("output.log")?)
        .apply()?;
    Ok(())
}

fn try_mkdir(path: &Path) -> io::Result<()> {
    if !fs::exists(path)? {
        log::info!("Creating directory '{}'", path.display());
        fs::create_dir(path)?;
    }
    Ok(())
}

/// Writes an index.html file redirecting to the root SPA with the path as an argument
fn write_stub_file(args: &Args, path: &Path) -> io::Result<()> {
    write_html(
        args,
        path,
        indoc! {r###"
        <!DOCTYPE html>
        <html xmlns="http://www.w3.org/1999/xhtml" xml:lang="en" lang="en">
        	<head>
        		<title>Redirecting to root page...</title>
        		<script type="text/javascript">
        var l = window.location;
        l.replace(
          l.protocol + '//' + l.hostname + (l.port ? ':' + l.port : '') +
          '/?/' + l.pathname.slice(1) + l.hash
        );
        		</script>
        	</head>
        	<body>
        	</body>
        </html>
    "###},
    )
}

/// Writes an HTML file. May minify the file.
fn write_html(args: &Args, path: &Path, contents: &str) -> io::Result<()> {
    if args.minify_html {
        use minify_html::{Cfg, minify};
        let mut cfg = Cfg::new();
        cfg.minify_css = true;
        cfg.minify_js = true;
        cfg.ensure_spec_compliant_unquoted_attribute_values = true;
        cfg.keep_comments = false;
        cfg.keep_closing_tags = true;

        log::info!("Minifying {}", path.display());
        fs::write(path, minify(contents.as_bytes(), &cfg))
    } else {
        fs::write(path, contents.as_bytes())
    }
}

/// Writes a CSS file. May minify the file.
fn write_css(args: &Args, path: &Path, contents: &str) -> io::Result<()> {
    use css_minify::optimizations::{Level, Minifier};

    log::info!("Minifying {}", path.display());
    match Minifier::default().minify(contents, Level::Three) {
        Ok(minified_css) => fs::write(path, minified_css),
        Err(e) => {
            log::error!("Failed to minify CSS; {}", e);
            if args.minifier_copy_on_failure {
                log::info!("Copying '{}' instead.", path.display());
                fs::write(path, contents)
            } else {
                Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("Couldn't minify {}", path.display()),
                ))
            }
        }
    }
}

/// Files to avoid processing
static SKIP: &[&str] = &[
    ".dir-locals.el",
    ".env",
    ".gitignore",
    ".projectile",
    "Justfile",
];
static PREFIX_SKIP: &[&str] = &[
    // Temporary files
    ".#",
];
static SUFFIX_SKIP: &[&str] = &[
    // Backups
    ".bak", ".tmp",
];

#[inline(always)]
fn should_be_skipped(file_name: &str) -> bool {
    for s in SKIP {
        if file_name == *s {
            return true;
        }
    }

    for ps in PREFIX_SKIP {
        if file_name.starts_with(*ps) {
            return true;
        }
    }
    for ss in SUFFIX_SKIP {
        if file_name.starts_with(*ss) {
            return true;
        }
    }
    return false;
}

fn main() -> io::Result<()> {
    let args = Args::parse();
    setup_logger(&args)
        .map_err(|_| io::Error::new(io::ErrorKind::Other, "Failed to setup logging"))?;

    let rss_config = page::RssConfig::new(args.root_address.clone());

    let now = chrono::Local::now();

    let mut rss_entries: Vec<rss::Item> = vec![];
    for path in WalkDir::new(&args.dir)
        .same_file_system(true)
        .min_depth(1)
        .into_iter()
        .filter_map(|e| e.ok().map(|e| e.into_path()))
    {
        log::debug!("Processing '{}'", path.display());
        let rel_path: &Path = path.strip_prefix(&args.dir).unwrap();
        let mut out_path: PathBuf = args.outdir.clone();
        out_path.push(rel_path);

        if path.is_dir() {
            try_mkdir(&out_path)?;
            continue;
        }

        let file_name = rel_path.file_name().unwrap().to_str().unwrap();

        if should_be_skipped(file_name) {
            log::info!("skipping {rel_path:?}");
            continue;
        }

        match path.extension().and_then(|s| s.to_str()) {
            Some("org") => {
                log::info!("Generating '{}'...", out_path.display());
                out_path.set_extension("");
                try_mkdir(&out_path)?;
                log::debug!(
                    "Generating index.html redirect for '{}'...",
                    out_path.display()
                );
                write_stub_file(&args, &out_path.join("index.html"))?;

                let contents =
                    fs::read_to_string(&path).expect("Should have been able to read the file");

                let doc = Org::parse(&contents);

                if let Some(rss_entry) = page::to_rss_item(&rss_config, &doc, rel_path) {
                    rss_entries.push(rss_entry);
                }
                let html = page::to_html(doc, rel_path)?;

                // Write HTML fragment
                log::debug!(
                    "Generating HTML fragment (_.html) for '{}'...",
                    out_path.display()
                );
                out_path.push("_.html");
                write_html(&args, &out_path, &html)?;
            }
            Some("html") => write_html(&args, &out_path, &fs::read_to_string(path)?)?,
            Some("css") => write_css(&args, &out_path, &fs::read_to_string(path)?)?,
            Some(_ext) => {
                if !args.copy_older_files && fs::exists(&out_path)? {
                    // Get metadata
                    let new_file = fs::metadata(&path)?;
                    let old_file = fs::metadata(&out_path)?;

                    if (new_file.len() == old_file.len())
                        && (new_file.modified()? <= old_file.modified()?)
                    {
                        log::debug!("Skipping write '{}'", out_path.display());
                        continue;
                    }
                }
                log::info!("Will write '{}'", out_path.display());
                fs::copy(path, out_path)?;
            }
            _ => {}
        }
    }

    let channel: rss::Channel = rss::ChannelBuilder::default()
        .title(args.title)
        .link(args.root_address)
        .description(args.description)
        .last_build_date(Some(now.to_rfc2822()))
        .language(args.language)
        .items(rss_entries)
        .build();

    use rss::validation::Validate;
    channel.validate().unwrap();

    let mut rss_out_path: PathBuf = args.outdir.clone();
    rss_out_path.push("feed.rss");
    log::info!("Will write RSS feed to '{}'", rss_out_path.display());
    fs::write(rss_out_path, channel.to_string())?;

    Ok(())
}
