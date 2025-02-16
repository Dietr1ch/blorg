use std::fs;
use std::io;
use std::path::Path;
use std::path::PathBuf;

use clap::Parser;
use indoc::indoc;
use orgize::Org;
use walkdir::WalkDir;

use forg::page;

/// Command line arguments
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct Args {
    // Input
    #[arg(default_value = "site")]
    pub dir: PathBuf,

    // Configuration
    #[arg(long)]
    pub root_address: String,
    #[arg(long)]
    pub title: String,
    #[arg(long)]
    pub description: String,
    #[arg(long, default_value = "en-GB")]
    pub language: String,

    #[arg(long, default_value = "Info")]
    pub log_level: log::LevelFilter,

    // Output
    #[arg(long, default_value = "out")]
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

fn write_stub_file(args: &Args, path: &Path) -> io::Result<()> {
    log::info!("Generating stub for '{}'...", path.display());

    write_html(args, path, indoc! {r###"
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
    "###})
}

fn write_html(args: &Args, path: &Path, contents: &str) -> io::Result<()> {
    if args.minify_html {
        use minify_html::{Cfg, minify};
        let mut cfg = Cfg::new();
        cfg.keep_comments = false;

        log::info!("Minifying {}", path.display());
        fs::write(path, minify(contents.as_bytes(), &cfg))
    } else {
        fs::write(path, contents.as_bytes())
    }
}

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
        let rel_path: &Path = path.strip_prefix(&args.dir).unwrap();
        let mut out_path: PathBuf = args.outdir.clone();
        out_path.push(rel_path);

        if path.is_dir() {
            try_mkdir(&out_path)?;
            continue;
        }

        match path.extension().and_then(|s| s.to_str()) {
            Some("org") => {
                log::info!("Generating '{}'...", out_path.display());
                out_path.set_extension("");
                try_mkdir(&out_path)?;
                write_stub_file(&args, &out_path.join("index.html"))?;
                out_path.push("_.html");

                let contents =
                    fs::read_to_string(&path).expect("Should have been able to read the file");

                let doc = Org::parse(&contents);

                if let Some(rss_entry) = page::to_rss_item(&rss_config, &doc, rel_path) {
                    rss_entries.push(rss_entry);
                }
                let html = page::to_html(doc, rel_path)?;

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
