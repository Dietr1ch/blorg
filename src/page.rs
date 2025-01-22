use std::cmp::{max, min};
use std::path::Path;
use std::path::PathBuf;

use orgize::Org;
use orgize::export::HtmlEscape;
use orgize::export::HtmlExport;
use orgize::export::{Container, Event, Traverser, from_fn_with_ctx};
use slugify::slugify;

const HEADING_HTML_ELEMENT: [&str; 6] = ["h1", "h2", "h3", "h4", "h5", "h6"];

pub fn to_html(doc: Org, file_rel_path: &Path) -> Result<String, std::io::Error> {
    let mut html_export = HtmlExport::default();

    assert!(file_rel_path.is_relative());

    let mut base_depth = 0i8;
    if let Some(properties) = doc.document().properties() {
        if let Some(base_depth_token) = properties.get("base_depth") {
            base_depth = base_depth_token.as_ref().parse::<i8>().unwrap_or(0);
        }
    }

    let mut handler = from_fn_with_ctx(|event, ctx| {
        match event {
            Event::Enter(Container::Link(link)) => {
                let path = link.path();
                let mut path: &str = path.trim_start_matches("file:");

                let local_link_prefix = format!(
                    "./{}/",
                    PathBuf::from(path).iter().nth(1).unwrap().to_str().unwrap()
                );

                // Handle local links
                let is_local_link = path.starts_with(&local_link_prefix);
                if is_local_link {
                    path = path.strip_prefix(&local_link_prefix).unwrap();

                    if path.ends_with(".org") {
                        path = path.strip_suffix(".org").unwrap();
                    }
                }

                // TODO Use _target blank on remote links
                let target = HtmlEscape(&path);

                if link.is_image() {
                    html_export.push_str(format!(r#"<img src="{}">"#, target));
                    return ctx.skip();
                }

                html_export.push_str(if is_local_link {
                    format!(
                        r###"<a hx-get="{0}/_.html"
                                preload
                                hx-target="#content"
                                hx-push-url="{0}/"
                                hx-history-target="{0}/"
                                aria-controls="content"
                                href="{0}"
                                class="">"###,
                        target
                    )
                } else {
                    format!(
                        r#"<a href="{}"
                              preload
                              target="_blank">"#,
                        target
                    )
                });

                if !link.has_description() {
                    html_export.push_str(format!("{}</a>", target));
                    ctx.skip();
                }
            }

            Event::Enter(Container::SourceBlock(block)) => {
                if let Some(language) = block.language() {
                    html_export.push_str(format!(
                        r#"<pre><code class="lang-{}">"#,
                        HtmlEscape(&language)
                    ));
                } else {
                    html_export.push_str("<pre><code>");
                }

                // if let Some(results) = block.results() {
                //     println!("RESULTS: {:?}", results);
                // }
            }
            Event::Leave(Container::SourceBlock(_)) => html_export.push_str("</code></pre>"),

            Event::Enter(Container::ExportBlock(e)) => {
                // Don't enter this block
                ctx.skip();

                // TODO: Check that the export type is "html"
                html_export.push_str(e.value());
            }
            // Event::Enter(Container::Results(results)) => {
            //     ctx.skip();
            //     html_export.push_str(format!("<pre>"));
            //     html_export.push_str(e.value());
            //     html_export.push_str(format!("</pre>"));
            // }
            Event::Enter(Container::PropertyDrawer(properties)) => {
                ctx.skip();

                for (k, v) in properties.iter() {
                    match (k.as_ref(), v.as_ref()) {
                        ("ID", id) => {
                            html_export.push_str(format!(
                                "<meta property=\"article:id\" content=\"{}\">",
                                id,
                            ));
                        }
                        ("modified_time", modified_time) => {
                            html_export.push_str(format!(
                                "<meta property=\"article:modified_time\" content=\"{}\">",
                                modified_time,
                            ));
                        }
                        (k, v) => {
                            log::debug!("Ignoring property {}:{}", k, v);
                        }
                    }
                }
            }

            Event::Enter(Container::Headline(headline)) => {
                let depth = (headline.level() as i8) + base_depth;
                let tag = HEADING_HTML_ELEMENT[max(0, min(depth, 6) - 1) as usize];
                let title = headline.title().map(|e| e.to_string()).collect::<String>();
                let slug = slugify!(&title);

                if title.starts_with(".") {
                    base_depth -= 1;
                    html_export.push_str(format!(
                        "<div class=\"{}\">",
                        title.replace(".", " ").trim()
                    ));
                } else {
                    // <section id="$SLUG(TITLE)">
                    html_export.push_str(format!("<section id=\"{}\">", slug));
                    //   <$TAG>
                    html_export.push_str(format!("<{tag}>"));
                    //     <a href="SLUG($TITLE)">
                    html_export.push_str(format!("<a href=\"#{0}\">", slug));
                    //     $HEADLINE.title
                    for elem in headline.title() {
                        html_export.element(elem, ctx);
                    }

                    //     </a>
                    html_export.push_str("</a>");
                    //   </$TAG>
                    html_export.push_str(format!("</{tag}>"));
                }
            }
            Event::Leave(Container::Headline(headline)) => {
                let title = headline.title().map(|e| e.to_string()).collect::<String>();
                if title.starts_with(".") {
                    base_depth += 1;
                    html_export.push_str("</div>");
                } else {
                    // </section>
                    html_export.push_str("</section>");
                }
            }

            _ => {
                log::debug!("Default handling for {:?}...", event);
                html_export.event(event, ctx);
            }
        }
    });

    doc.traverse(&mut handler);

    Ok(html_export.finish())
}