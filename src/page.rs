use std::path::Path;

use indoc::indoc;
use orgize::Org;
use orgize::export::HtmlEscape;
use orgize::export::HtmlExport;
use orgize::export::{Container, Event, Traverser, from_fn_with_ctx};
use slugify::slugify;

const HTML_HEADING_LEVELS: [&str; 6] = ["h1", "h2", "h3", "h4", "h5", "h6"];

pub fn org_tags(_doc: &Org, contents: &str) -> Vec<String> {
    let mut tags = vec![];

    const FILETAGS_PREFIX: &str = "#+filetags: ";
    for l in contents.lines() {
        if l.starts_with(FILETAGS_PREFIX) {
            for t in l.trim_start_matches(FILETAGS_PREFIX).split(":") {
                if t.is_empty() {
                    continue;
                }
                if t.chars().next().unwrap().is_uppercase() {
                    tags.push(String::from(t));
                } else {
                    log::debug!("Ignoring '{t}' tag because doesn't start with Uppercase");
                    continue;
                }
            }
        }
    }

    tags
}

#[derive(Debug, Default)]
struct PageRequirements {
    has_code: bool,
}

pub fn to_html(doc: Org, tags: &[String], file_rel_path: &Path) -> Result<String, std::io::Error> {
    let mut html_export = HtmlExport::default();
    let file_name = file_rel_path.file_name().unwrap().to_str().unwrap();
    let file_stem = file_name.trim_end_matches(".org");
    let mut requirements = PageRequirements::default();

    assert!(file_rel_path.is_relative());

    let mut base_depth = 0i8;
    if let Some(properties) = doc.document().properties()
        && let Some(base_depth_token) = properties.get("base_depth")
    {
        base_depth = base_depth_token.as_ref().parse::<i8>().unwrap_or(0);
    }

    let mut handler = from_fn_with_ctx(|event, ctx| {
        match event {
            Event::Enter(Container::Document(_doc)) => {
                // Add title if present
                if let Some(title) = doc.title() {
                    // Parse title as Org document
                    let title = Org::parse(title);
                    let title = title.first_node::<orgize::ast::Paragraph>().unwrap();

                    use orgize::rowan::ast::AstNode;
                    let mut html = HtmlExport::default();
                    html.render(title.syntax());
                    let title_html = html.finish();
                    // Drop surrounding <p>...</p>
                    let title_html = &title_html[3..title_html.len() - 3 - 1];

                    let depth = base_depth;
                    let heading = HTML_HEADING_LEVELS[(depth - 1).clamp(0, 5) as usize];

                    if tags.is_empty() {
                        // <H*>$TITLE</H*>
                        html_export.push_str(format!(r#"<{heading}>{title_html}</{heading}>"#));
                    } else {
                        // <hgroup>
                        //   <H*>$TITLE</H*>
                        //   <p>Tags: <dd-tag>TAG</dd-tag>...</p>
                        // </hgroup>
                        html_export.push_str(format!(
                            r#"<hgroup><{heading}>{title_html}</{heading}><p>Tags:"#
                        ));
                        for t in tags {
                            html_export.push_str(format!(r#" <dd-tag>{t}</dd-tag>"#));
                        }
                        html_export.push_str(r#"</p></hgroup>"#);
                    }
                }
            }
            Event::Leave(Container::Document(_doc)) => {
                if requirements.has_code {
                    html_export.push_str("<script src=\"https://cdn.jsdelivr.net/npm/@arborium/arborium@1/dist/arborium.iife.js\" data-theme=\"ayu-dark\"></script>");
                }
            }

            Event::Enter(Container::Link(link)) => {
                let path = link.path();
                let mut path: &str = path.trim_start_matches("file:");
                log::debug!("Linking to: {path:?}");

                let local_link_prefix = format!("./{file_stem}/");

                // Handle local links
                let is_local_org_link = path.ends_with(".org");
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
                    if is_local_org_link {
                        format!(
                            indoc! {r###"
                                <a hx-get="{0}/_.html"
                                  preload
                                  hx-target="#content"
                                  hx-push-url="{0}/"
                                  hx-history-target="{0}/"
                                  aria-controls="content"
                                  href="{0}" />
                        "###},
                            target
                        )
                    } else {
                        // Not a local .org link
                        format!(
                            indoc! {r###"
                                <a target="blank"
                                  preload
                                  href="{0}" />
                        "###},
                            target
                        )
                    }
                } else {
                    format!(
                        indoc! {r###"
                                <a href="{}"
                                  preload
                                  target="_blank" />
                        "###},
                        target
                    )
                });

                if !link.has_description() {
                    html_export.push_str(format!("{}</a>", target.0.trim()));
                    ctx.skip();
                }
            }

            Event::Enter(Container::SourceBlock(block)) => {
                requirements.has_code = true;

                // FIXME: Avoid weird prefix spacing? Check https://docs.rs/indoc
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
                                "<meta property=\"article:id\" content=\"{}\" />",
                                id,
                            ));
                        }
                        ("published_time", published_time) => {
                            html_export.push_str(format!(
                                "<meta property=\"article:published_time\" content=\"{}\" />",
                                published_time,
                            ));
                        }
                        ("modified_time", modified_time) => {
                            html_export.push_str(format!(
                                "<meta property=\"article:modified_time\" content=\"{}\" />",
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
                let heading = HTML_HEADING_LEVELS[(depth - 1).clamp(0, 5) as usize];
                let title = headline.title().map(|e| e.to_string()).collect::<String>();
                let slug = slugify!(&title);

                if title.starts_with(".") {
                    base_depth -= 1;

                    html_export.push_str(format!(
                        "<div class=\"s{} {}\">",
                        headline.level(),
                        title.replace(".", " ").trim()
                    ));
                } else {
                    // <section id="$SLUG(TITLE)">
                    html_export.push_str(format!(
                        "<section id=\"{}\" class=\"s{}\">",
                        slug,
                        headline.level(),
                    ));
                    //   <hgroup>
                    //   <$HEADING>
                    // TODO: Add hgroup support
                    // html_export.push_str(format!("<hgroup>"));
                    html_export.push_str(format!("<{heading}>"));
                    //     <a href="SLUG($TITLE)" />
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
                    //   </$HEADING>
                    //   </hgroup>
                    html_export.push_str(format!("</{heading}>"));
                    // TODO: Add hgroup support
                    // html_export.push_str(format!("</hgroup>"));
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

#[cfg(test)]
mod tests {
    use super::*;
    use googletest::prelude::*;

    #[gtest]
    fn simple_doc() {
        let contents = indoc! {r###"
          #+title: TITLE
          * Heading
          Hi
        "###};
        let doc = Org::parse(contents);
        let tags = org_tags(&doc, contents);

        let rel_path = Path::new("simple_doc.org");
        let html = to_html(doc, &tags, rel_path);

        expect_that!(
            html,
            ok(eq(indoc! {r###"
              <h1>TITLE</h1><section></section><section id="heading" class="s1"><h1><a href="#heading">Heading</a></h1><section><p>Hi
              </p></section></section>"###})),
        );
    }

    #[gtest]
    fn some_code() {
        let contents = indoc! {r###"
          #+title: TITLE
          * Heading
          Hi

          #+begin_src rust
          pub fn main() {
            println!("Hi");
          }
          #+end_src
        "###};
        let doc = Org::parse(contents);
        let tags = org_tags(&doc, contents);

        let rel_path = Path::new("simple_doc.org");
        let html = to_html(doc, &tags, rel_path);

        expect_that!(
            html,
            ok(eq(indoc! {r###"
              <h1>TITLE</h1><section></section><section id="heading" class="s1"><h1><a href="#heading">Heading</a></h1><section><p>Hi
              </p><pre><code class="language-rust">pub fn main() {
                println!(&quot;Hi&quot;);
              }
              </code></pre></section></section><script src="https://cdn.jsdelivr.net/npm/@arborium/arborium@1/dist/arborium.iife.js" data-theme="ayu-dark"></script>"###})),
        );
    }
}
