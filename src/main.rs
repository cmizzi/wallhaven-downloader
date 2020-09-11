#[macro_use]
extern crate log;

use std::fs::File;
use std::io::copy;

use env_logger::Env;
use select::document::Document;
use select::predicate::{Attr, Class};
//use tokio::time::Duration;
use std::collections::VecDeque;
use std::error::Error;
use reqwest::Response;
use ratelimit::Limiter;
use tokio::time::Duration;

#[derive(Debug)]
struct Wallpaper {
    url: String,
    name: String,
}

impl Wallpaper {
    fn new(url: String) -> Self {
        let name = url.clone().split("/").last().unwrap().to_string();

        Self { url, name, }
    }
}

struct Downloader<'a> {
    limit: u16,
    wallpapers: VecDeque<Wallpaper>,
    limiter: &'a mut Limiter,
}

impl<'a> Downloader<'a> {
    fn new(limiter: &'a mut Limiter) -> Self {
        Self {
            limit: 20,
            wallpapers: VecDeque::new(),
            limiter,
        }
    }

    async fn execute(&mut self) -> Result<(), Box<dyn Error>> {
        let mut page = 1;

        'outer: loop {
            let response = self
                .download(
                    format!("https://wallhaven.cc/search?categories=111&purity=100&resolutions=1920x1080&sorting=random&order=desc&seed=oOeWg&page={}", page).as_str()
                )
                .await?
                .text().await?;

            for node in Document::from(response.as_str()).find(Class("preview")) {
                let url = node.attr("href").unwrap();
                let wallpaper = self.extract_wallpaper_url(url).await
                    .and_then(|url| Ok(Wallpaper::new(url)));

                if let Err(e) = wallpaper {
                    error!("{}", e);
                    continue;
                }

                self.wallpapers.push_back(wallpaper?);

                if self.wallpapers.len() >= self.limit as usize {
                    break 'outer;
                }
            }

            page += 1;
        }

        let directory = "/home/cyril/Pictures/Wallpapers";

        loop {
            let wallpaper = self.wallpapers.pop_front();

            if wallpaper.is_none() {
                break;
            }

            let wallpaper = wallpaper.unwrap();

            let mut dest = File::create(format!("{}/{}", directory, &wallpaper.name))?;
            let content = self.download(&wallpaper.url).await?;

            copy(&mut content.text().await?.as_bytes(), &mut dest)?;
        }

        Ok(())
    }

    async fn extract_wallpaper_url(&mut self, url: &str) -> Result<String, Box<dyn Error>> {
        let response = self.download(url).await?;
        let path = response.url().path().to_string();

        // Let's parse the document and extract the full image.
        let document = Document::from(response.text().await?.as_str());
        let src = document.find(Attr("id", "wallpaper")).next()
            .and_then(|n| n.attr("data-cfsrc"))
            .and_then(|src| Some(src.to_string()));

        // If we can't find the source, we can't process, let's return an error.
        if src.is_none() {
            return Err(
                format!("Cannot find the wallpaper source from \"{}\".", path)
                    .into()
            );
        }

        Ok(src.unwrap())
    }

    async fn download(&mut self, url: &str) -> Result<Response, Box<dyn Error>> {
        self.limiter.wait();
        let response = reqwest::get(url).await?;

        // We cannot process if the response is not successful.
        if !response.status().is_success() {
            return Err(
                format!("Cannot fetch the page \"{}\" with status code {}.", response.url().path().to_string(), response.status().as_u16())
                    .into()
            );
        } else {
            info!("Successfully downloaded \"{}\".", response.url().path().to_string());
        }

        Ok(response)
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::from_env(Env::default().default_filter_or("info")).init();

    let mut limiter = ratelimit::Builder::new().capacity(1).quantum(1).interval(Duration::new(1, 0)).build();
    let mut downloader = Downloader::new(&mut limiter);

    downloader.execute().await?;

    Ok(())
}
