use std::path::Path;
use std::path::PathBuf;

use indoc::indoc;
use orgize::Org;
use orgize::export::HtmlEscape;
use orgize::export::HtmlExport;
use orgize::export::{Container, Event, Traverser, from_fn_with_ctx};
use slugify::slugify;

const HEADING_HTML_ELEMENT: [&str; 6] = ["h1", "h2", "h3", "h4", "h5", "h6"];

pub struct RssConfig {
    root_address: String,
}

impl RssConfig {
    pub fn new(root_address: String) -> Self {
        Self { root_address }
    }
}

pub fn to_rss_item(config: &RssConfig, doc: &Org, file_rel_path: &Path) -> Option<rss::Item> {
    let mut item = rss::ItemBuilder::default();

    if let Some(properties) = doc.document().properties() {
        if properties.get("skip_feed").is_some() {
            return None;
        }

        if let Some(title) = properties.get("title") {
            item.title(title.to_string());
        }
        if let Some(description) = properties.get("description") {
            item.description(description.to_string());
        }
        if let Some(publication_date) = properties.get("publication_date") {
            item.pub_date(publication_date.to_string());
        }
        let mut path = file_rel_path.to_path_buf();
        path.set_extension("");
        item.link(Some(format!("{}/{}", config.root_address, path.display())));
    }

    Some(item.build())
}

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
            Event::Enter(Container::Document(_doc)) => {
                html_export.push_str("<main>");

                // Add title if present
                if let Some(title) = doc.title() {
                    // Parse title as Org document
                    let title = Org::parse(title);
                    let title = title.first_node::<orgize::ast::Paragraph>().unwrap();

                    use orgize::rowan::ast::AstNode;
                    let mut html = HtmlExport::default();
                    html.render(title.syntax());
                    let title_html = html.finish();
                    // Drop surrounding <p>...</p> tags
                    let title_html = &title_html[3..title_html.len() - 3 - 1];

                    let depth = base_depth;
                    let tag = HEADING_HTML_ELEMENT[(depth - 1).clamp(0, 5) as usize];

                    html_export.push_str(format!(r#"<{tag}>{title_html}</{tag}>"#));
                }
            }

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

                let target = HtmlEscape(&path);

                if link.is_image() {
                    html_export.push_str(format!(r#"<img src="{}">"#, target));
                    return ctx.skip();
                }

                html_export.push_str(if is_local_link {
                    format!(
                        indoc! {r###"
                                <a hx-get="{0}/_.html"
                                  preload
                                  hx-target="#content"
                                  hx-push-url="{0}/"
                                  hx-history-target="{0}/"
                                  aria-controls="content"
                                  href="{0}"
                                  class="">
                        "###},
                        target
                    )
                } else {
                    format!(
                        indoc! {r###"
                                <a href="{}"
                                  preload
                                  target="_blank">
                        "###},
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
                        r#"<pre><code class="language-{}">"#,
                        HtmlEscape(&language)
                    ));
                } else {
                    html_export.push_str("<pre><code>");
                }
            }
            Event::Leave(Container::SourceBlock(_block)) => {
                html_export.push_str("</code></pre>");
            }

            Event::Enter(Container::FixedWidth(fixed)) => {
                html_export.push_str(format!(
                    r#"<pre><samp class="org_result">{}</samp></pre>"#,
                    HtmlEscape(fixed.value()),
                ));
                ctx.skip();
            }

            Event::Enter(Container::ExportBlock(e)) => {
                // Don't enter this block
                ctx.skip();

                // TODO: Check that the export type is "html"
                html_export.push_str(e.value());
            }
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
                        ("published_time", published_time) => {
                            html_export.push_str(format!(
                                "<meta property=\"article:published_time\" content=\"{}\">",
                                published_time,
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
                let tag = HEADING_HTML_ELEMENT[(depth - 1).clamp(0, 5) as usize];
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
                    if headline.is_todo() {
                        html_export.push_str("<span class=\"org_todo\">TODO</span> ");
                    }
                    if headline.is_done() {
                        html_export.push_str("<span class=\"org_todo_done\">DONE</span> ");
                    }
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
