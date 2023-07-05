#![feature(async_closure)]
use dnsclient::{r#async::DNSClient, UpstreamServer};
use indicatif::{ProgressBar, ProgressStyle};
use rayon::{prelude::ParallelIterator, str::ParallelString};
use std::{
    collections::HashMap,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    sync::Arc,
};
use tokio::io::AsyncWriteExt;

const TLD: &str = "rs";

#[tokio::main]
async fn main() {
    std::env::set_var("RUST_LOG", "debug");
    pretty_env_logger::init();

    log::info!("initializing");

    let mut files: HashMap<String, Vec<String>> = HashMap::new();

    for path in glob::glob("wordlists/*.txt").unwrap() {
        let Ok(path) = path else {
            continue;
        };

        log::info!("reading {}", &path.file_name().unwrap().to_string_lossy());

        files.insert(
            path.file_name()
                .unwrap()
                .to_string_lossy()
                .replace(".txt", ""),
            tokio::fs::read_to_string(path)
                .await
                .unwrap()
                .par_lines()
                .filter(|word| word.ends_with(TLD) && word.len() >= 5)
                .map(|word| word.to_string())
                .collect(),
        );
    }

    log::info!(
        "{} files, total {} domain hacks utilizing {TLD}",
        files.len(),
        files.values().map(|file| file.len()).sum::<usize>()
    );

    let mut domains: Vec<String> = files
        .into_values()
        .flat_map(|file| {
            file.into_iter()
                .map(|result| {
                    let mut str = result[..result.len() - 2].to_owned();
                    str.push_str(&format!(".{TLD}"));
                    str
                })
                .collect::<Vec<_>>()
        })
        .collect();

    domains.sort();
    domains.dedup();

    let bar = Arc::new(
        ProgressBar::new(domains.len() as u64).with_style(
            ProgressStyle::default_bar()
                .template("{wide_bar} {msg:>25} {pos}/{len}")
                .expect("template error"),
        ),
    );

    let mut funny_domains = Vec::new();

    let client = Arc::new(DNSClient::new(vec![
        UpstreamServer::new(SocketAddr::new(IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1)), 53)),
        UpstreamServer::new(SocketAddr::new(IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8)), 53)),
        UpstreamServer::new(SocketAddr::new(IpAddr::V4(Ipv4Addr::new(8, 8, 4, 4)), 53)),
        UpstreamServer::new(SocketAddr::new(IpAddr::V4(Ipv4Addr::new(9, 9, 9, 9)), 53)),
    ]));

    let mut out_file = tokio::fs::File::create(&format!("{TLD}.domains"))
        .await
        .expect("unable to open domain file for writing");

    for chunk in domains.chunks(50) {
        let tasks = chunk.iter().map(|domain| {
            let domain = domain.clone();
            let bar = bar.clone();
            let client = client.clone();

            tokio::spawn(async move {
                match client.query_addrs(&domain).await {
                    Ok(x) => {
                        if x.is_empty() {
                            // log::info!("found unused domain: {domain}");
                            bar.inc(1);
                            bar.set_message(domain.clone());
                            Some(domain)
                        } else {
                            bar.inc(1);
                            None
                        }
                    }
                    Err(e) => {
                        bar.inc(1);
                        log::error!("error querying domain {domain}: {e:?}");
                        None
                    }
                }
            })
        });

        for domain in futures::future::join_all(tasks).await {
            match domain {
                Ok(Some(domain)) => {
                    out_file
                        .write_all(format!("{domain}\n").as_bytes())
                        .await
                        .expect("unable to write domain to file");
                    funny_domains.push(domain);
                }
                Err(e) => {
                    log::error!("error awaiting task: {e:?}");
                }
                _ => {}
            }
        }
    }

    bar.finish();

    // funny_domains.dedup();
    // funny_domains.sort();

    // std::fs::write(&format!("{TLD}.domains"), funny_domains.join("\n"))
    //     .expect("unable to write domain list");
}
