use std::path::Path;
use std::path::PathBuf;

// TODO: Generate ~/Notes/Personal/Blog/sitemap.xml
// TODO: Move feed generation to here

pub struct Sitemap {
    root_address: String,

    title: String,
    description: String,
    language: String,

    generation_time: chrono::DateTime<chrono::Local>,

    rss_entries: Vec<rss::Item>,
}

impl Sitemap {
    pub fn new(
        root_address: String,
        title: String,
        description: String,
        language: String,
        generation_time: chrono::DateTime<chrono::Local>,
    ) -> Self {
        Self {
            root_address,
            title,
            description,
            language,
            generation_time,
            rss_entries: vec![],
        }
    }

    pub fn push(&mut self, doc: &orgize::Org, out_path: &Path) {
        println!("FIXME: actually push data into the sitemap");
        log::debug!("Generating RSS entry for '{}'...", out_path.display());

        if let Some(properties) = doc.document().properties() {
            if properties.get("skip_feed").is_some() {
                return;
            }
            if properties.get("Draft").is_some() {
                return;
            }
        }

        self.push_rss(doc, out_path);
        self.push_sitemap(doc, out_path);
    }

    fn push_rss(&mut self, doc: &orgize::Org, out_path: &Path) {
        log::debug!("Generating RSS entry for '{}'...", out_path.display());

        let mut item = rss::ItemBuilder::default();

        if let Some(properties) = doc.document().properties() {
            if let Some(title) = properties.get("title") {
                item.title(title.to_string());
            }
            if let Some(description) = properties.get("description") {
                item.description(description.to_string());
            }
            if let Some(publication_date) = properties.get("publication_date") {
                item.pub_date(publication_date.to_string());
            }
            let mut path = out_path.to_path_buf();
            path.set_extension("");
            item.link(Some(format!("{}/{}", self.root_address, path.display())));
        }

        self.rss_entries.push(item.build());
    }
    fn push_sitemap(&mut self, _doc: &orgize::Org, out_dir: &Path) {
        log::debug!("Generating Sitemap entry for '{}'...", out_dir.display());
    }

    pub fn generate(&self, out_dir: &Path) {
        self.generate_rss(out_dir);
        self.generate_sitemap(out_dir);
    }

    fn generate_rss(&self, out_dir: &Path) {
        log::debug!("Generating RSS feed on '{}'...", out_dir.display());

        let channel: rss::Channel = rss::ChannelBuilder::default()
            .title(self.title.clone())
            .link(self.root_address.clone())
            .description(self.description.clone())
            .last_build_date(Some(self.generation_time.to_rfc2822()))
            .language(self.language.clone())
            .items(self.rss_entries.clone())
            .build();

        use rss::validation::Validate;
        channel.validate().unwrap();

        let mut rss_out_path: PathBuf = out_dir.to_path_buf();
        rss_out_path.push("feed.rss");
        log::info!("Will write RSS feed to '{}'", rss_out_path.display());
        std::fs::write(rss_out_path, channel.to_string()).unwrap();
    }

    fn generate_sitemap(&self, out_dir: &Path) {
        log::debug!("Generating Sitemap feed on '{}'...", out_dir.display());

        let mut sitemap_out_path: PathBuf = out_dir.to_path_buf();
        sitemap_out_path.push("sitemap.xml");

        log::info!(
            "Will write Sitemap feed to '{}'",
            sitemap_out_path.display()
        );
        // fs::write(sitemap_out_path, channel.to_string())?;
    }
}

// #[cfg(test)]
// mod tests {
//     use super::*;

//     #[test]
//     fn it_works() {}
// }
