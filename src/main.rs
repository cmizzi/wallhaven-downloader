#[macro_use]
extern crate log;

use std::{
    collections::VecDeque,
    error::Error,
    fs::File,
    io::{
        copy,
        Write,
    },
};

use clap::Clap;
use env_logger::Env;
use rand::{
    distributions::Alphanumeric,
    Rng,
    thread_rng,
};
use ratelimit::Limiter;
use reqwest::Response;
use select::{
    document::Document,
    predicate::{Attr, Class},
};
use tokio::time::Duration;

#[derive(Clap, Debug)]
#[clap(version = "1.0", author = "Cyril Mizzi <me@p1ngouin.com")]
struct Opts {
    /// Based on the following format : [General, Anime, People].
    #[clap(short, long, default_value = "111")]
    categories: String,

    /// Based on the following format : [SFW, Sketchy].
    #[clap(short, long, default_value = "100")]
    purity: String,

    /// Resolution is exact. The format should match the following pattern: <width>x<height>.
    resolutions: String,

    /// Directory to store wallpapers.
    output: String,

    /// Limit the number of wallpapers to download.
    #[clap(short, long, default_value = "10")]
    limit: u8,

    /// Default sort to apply.
    #[clap(short, long, default_value = "random")]
    sorting: String,

    /// Sort direction.
    #[clap(short, long, default_value = "desc")]
    direction: String,

    /// Configure verbosity.
    #[clap(short, long, parse(from_occurrences))]
    verbose: i32,
}

#[derive(Debug)]
struct Wallpaper {
    url: String,
    name: String,
}

impl Wallpaper {
    fn new(url: String) -> Self {
        let name = url.clone().split("/").last().unwrap().to_string();

        Self { url, name }
    }
}

struct Downloader<'a> {
    wallpapers: VecDeque<Wallpaper>,
    limiter: &'a mut Limiter,
    opts: &'a Opts,
    seed: String,
}

impl<'a> Downloader<'a> {
    /// Constructor.
    fn new(limiter: &'a mut Limiter, opts: &'a Opts) -> Self {
        Self {
            wallpapers: VecDeque::new(),
            limiter,
            opts,
            seed: thread_rng().sample_iter(&Alphanumeric).take(5).collect(),
        }
    }

    /// Build the endpoint URL using CLI arguments.
    fn build_url(&self, page: i32) -> String {
        format!(
            "https://wallhaven.cc/search?categories={}&purity={}&resolutions={}&sorting={}&order={}&seed={}&page={}",
            self.opts.categories,
            self.opts.purity,
            self.opts.resolutions,
            self.opts.sorting,
            self.opts.direction,
            self.seed,
            page,
        )
    }

    /// Execute the main loop.
    async fn execute(&mut self) -> Result<(), Box<dyn Error>> {
        let mut page = 1;

        info!("Fetching wallpapers indexes.");

        'outer: loop {
            let url = self.build_url(page);
            let response = self
                .download(&url)
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

                if self.wallpapers.len() >= self.opts.limit as usize {
                    break 'outer;
                }
            }

            page += 1;
        }

        info!("Download wallpapers.");

        loop {
            let wallpaper = self.wallpapers.pop_front();

            if wallpaper.is_none() {
                break;
            }

            let wallpaper = wallpaper.unwrap();

            let mut dest = File::create(format!("{}/{}", self.opts.output, &wallpaper.name))?;
            let response = self.download(&wallpaper.url).await?;

            let copied = copy(&mut response.bytes().await?.as_ref(), &mut dest);

            match copied {
                Ok(_) => info!("Wallpaper \"{}\" stored.", wallpaper.name),
                Err(e) => error!("Error while storing the file: {}", e)
            }
        }

        Ok(())
    }

    /// Extract a wallpaper URL from the main picture URL.
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

    /// Execute a request.
    ///
    /// This method must use a rate limiter because Wallhaven would return a 429 for too many
    /// requests.
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
            debug!("Successfully requested \"{}\".", response.url().path().to_string());
        }

        Ok(response)
    }
}

/// Initialize the logger.
fn init_logger(opts: &Opts) {
    let env = Env::default().default_filter_or(
        match opts.verbose {
            0 => "wallhaven_downloader=info",
            1 => "wallhaven_downloader=debug",
            2 => "debug",
            _ => "trace",
        }
    );

    env_logger::from_env(env)
        .format(|buf, record| {
            let level_style = buf.default_level_style(record.level());
            writeln!(buf, "[{} {:>5}]: {}", buf.timestamp(), level_style.value(record.level()), record.args())
        })
        .init();
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let opts: Opts = Opts::parse();
    init_logger(&opts);

    let mut limiter = ratelimit::Builder::new().capacity(1).quantum(1).interval(Duration::new(1, 0)).build();
    let mut downloader = Downloader::new(&mut limiter, &opts);

    downloader.execute().await?;

    Ok(())
}
