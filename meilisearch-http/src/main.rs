use std::env;

use actix_web::HttpServer;
use main_error::MainError;
use meilisearch_http::{analytics::Analytics, create_app, Data, Opt};
use structopt::StructOpt;

#[cfg(target_os = "linux")]
#[global_allocator]
static ALLOC: tikv_jemallocator::Jemalloc = tikv_jemallocator::Jemalloc;

#[actix_web::main]
async fn main() -> Result<(), MainError> {
    let opt = Opt::from_args();

    let mut log_builder = env_logger::Builder::new();
    log_builder.parse_filters(&opt.log_level);
    if opt.log_level == "info" {
        // if we are in info we only allow the warn log_level for milli
        log_builder.filter_module("milli", log::LevelFilter::Warn);
    }

    log_builder.init();

    match opt.env.as_ref() {
        "production" => {
            if opt.master_key.is_none() {
                return Err(
                    "In production mode, the environment variable MEILI_MASTER_KEY is mandatory"
                        .into(),
                );
            }
        }
        "development" => (),
        _ => unreachable!(),
    }

    let data = Data::new(opt.clone())?;
    let analytics = Analytics::new(opt.clone()).await;

    if !opt.no_analytics {
        let analytics_data = data.clone();
        // let analytics_opt = opt.clone();
        analytics.tick(analytics_data);
    }

    print_launch_resume(&opt, &data, &analytics);

    run_http(data, opt, analytics).await?;

    Ok(())
}

async fn run_http(
    data: Data,
    opt: Opt,
    analytics: &'static Analytics,
) -> Result<(), Box<dyn std::error::Error>> {
    let _enable_dashboard = &opt.env == "development";
    let http_server = HttpServer::new(move || create_app!(data, analytics, _enable_dashboard))
        // Disable signals allows the server to terminate immediately when a user enter CTRL-C
        .disable_signals();

    if let Some(config) = opt.get_ssl_config()? {
        http_server
            .bind_rustls(opt.http_addr, config)?
            .run()
            .await?;
    } else {
        http_server.bind(opt.http_addr)?.run().await?;
    }
    Ok(())
}

pub fn print_launch_resume(opt: &Opt, data: &Data, analytics: &Analytics) {
    let commit_sha = option_env!("VERGEN_GIT_SHA").unwrap_or("unknown");
    let commit_date = option_env!("VERGEN_GIT_COMMIT_TIMESTAMP").unwrap_or("unknown");

    let ascii_name = r#"
888b     d888          d8b 888 d8b  .d8888b.                                    888
8888b   d8888          Y8P 888 Y8P d88P  Y88b                                   888
88888b.d88888              888     Y88b.                                        888
888Y88888P888  .d88b.  888 888 888  "Y888b.    .d88b.   8888b.  888d888 .d8888b 88888b.
888 Y888P 888 d8P  Y8b 888 888 888     "Y88b. d8P  Y8b     "88b 888P"  d88P"    888 "88b
888  Y8P  888 88888888 888 888 888       "888 88888888 .d888888 888    888      888  888
888   "   888 Y8b.     888 888 888 Y88b  d88P Y8b.     888  888 888    Y88b.    888  888
888       888  "Y8888  888 888 888  "Y8888P"   "Y8888  "Y888888 888     "Y8888P 888  888
"#;

    eprintln!("{}", ascii_name);

    eprintln!("Database path:\t\t{:?}", opt.db_path);
    eprintln!("Server listening on:\t\"http://{}\"", opt.http_addr);
    eprintln!("Environment:\t\t{:?}", opt.env);
    eprintln!("Commit SHA:\t\t{:?}", commit_sha.to_string());
    eprintln!("Commit date:\t\t{:?}", commit_date.to_string());
    eprintln!(
        "Package version:\t{:?}",
        env!("CARGO_PKG_VERSION").to_string()
    );

    if opt.no_analytics {
        eprintln!("Anonymous telemetry:\t\"Disabled\"");
    } else {
        eprintln!(
                "
Thank you for using MeiliSearch!

We collect anonymized analytics to improve our product and your experience. To learn more, including how to turn off analytics, visit our dedicated documentation page: https://docs.meilisearch.com/learn/what_is_meilisearch/telemetry.html

Anonymous telemetry:   \"Enabled\"
Your unique user ID is: {}", analytics
            );
    }

    eprintln!();

    if data.api_keys().master.is_some() {
        eprintln!("A Master Key has been set. Requests to MeiliSearch won't be authorized unless you provide an authentication key.");
    } else {
        eprintln!("No master key found; The server will accept unidentified requests. \
            If you need some protection in development mode, please export a key: export MEILI_MASTER_KEY=xxx");
    }

    eprintln!();
    eprintln!("Documentation:\t\thttps://docs.meilisearch.com");
    eprintln!("Source code:\t\thttps://github.com/meilisearch/meilisearch");
    eprintln!("Contact:\t\thttps://docs.meilisearch.com/resources/contact.html or bonjour@meilisearch.com");
    eprintln!();
}
