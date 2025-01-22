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
    #[arg(default_value = "site")]
    pub dir: PathBuf,

    #[arg(long, default_value = "out")]
    pub outdir: PathBuf,

    #[arg(long, default_value = "false")]
    pub copy_older_files: bool,
}

use std::time::SystemTime;

fn setup_logger() -> Result<(), fern::InitError> {
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
        .level(log::LevelFilter::Info)
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

fn write_stub_file(path: &Path) -> io::Result<()> {
    log::info!("Generating stub for '{}'...", path.display());

    write_html(path, indoc! {r###"
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

fn write_html(path: &Path, contents: &str) -> io::Result<()> {
    use minify_html::{Cfg, minify};

    let mut cfg = Cfg::new();
    cfg.keep_comments = false;

    log::info!("Minifying {}", path.display());
    fs::write(path, minify(contents.as_bytes(), &cfg))
}

fn write_css(path: &Path, contents: &str) -> io::Result<()> {
    use css_minify::optimizations::{Level, Minifier};

    log::info!("Minifying {}", path.display());
    fs::write(
        path,
        Minifier::default().minify(contents, Level::Three).unwrap(),
    )
}

fn main() -> io::Result<()> {
    let args = Args::parse();
    setup_logger().map_err(|_| io::Error::new(io::ErrorKind::Other, "Failed to setup logging"))?;

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
                write_stub_file(&out_path.join("index.html"))?;
                out_path.push("_.html");

                let contents =
                    fs::read_to_string(&path).expect("Should have been able to read the file");

                let doc = Org::parse(&contents);
                let html = page::to_html(doc, rel_path)?;

                write_html(&out_path, &html)?;
            }
            Some("html") => write_html(&out_path, &fs::read_to_string(path)?)?,
            Some("css") => write_css(&out_path, &fs::read_to_string(path)?)?,
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

    Ok(())
}
