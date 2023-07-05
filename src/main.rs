#![feature(async_closure)]
use dnsclient::{r#async::DNSClient, UpstreamServer};
use indicatif::{ProgressBar, ProgressStyle};
use rayon::{prelude::ParallelIterator, str::ParallelString};
use std::{
    collections::HashMap,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    sync::Arc,
};

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

    let domains: Vec<String> = files
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

    let bar = Arc::new(
        ProgressBar::new(domains.len() as u64).with_style(
            ProgressStyle::default_bar()
                .template("{wide_bar} {msg:>25} {pos}/{len}")
                .expect("template error"),
        ),
    );

    let mut funny_domains = Vec::new();

    let client = Arc::new(DNSClient::new(vec![UpstreamServer::new(SocketAddr::new(
        IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1)),
        53,
    ))]));

    for chunk in domains.chunks(100) {
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
                        log::error!("error querying addresses: {e:?}");
                        None
                    }
                }
            })
        });

        futures::future::join_all(tasks)
            .await
            .into_iter()
            .for_each(|domain| match domain {
                Ok(Some(domain)) => {
                    funny_domains.push(domain);
                }
                Err(e) => {
                    log::error!("error awaiting task: {e:?}");
                }
                _ => {}
            });
    }

    bar.finish();

    funny_domains.dedup();
    funny_domains.sort();

    // let mut funny_domains = Vec::new();

    // for domain in &domains {
    //     bar.inc(1);
    //     let mut buf = vec![0u8; 32768];
    //     match client
    //         .query_raw(
    //             domain,
    //             rsdns::constants::Type::A,
    //             rsdns::constants::Class::In,
    //             &mut buf,
    //         )
    //         .await
    //     {
    //         Ok(len) => {
    //             let iter =
    //                 MessageIterator::new(&buf[0..len]).expect("unable to create iterator. cringe");
    //             // log::info!(
    //             //     "{domain}: Header: {:#?} Questions: {:#?} Records: {:#?}",
    //             //     iter.header(),
    //             //     iter.questions().collect::<Vec<_>>(),
    //             //     iter.records().collect::<Vec<_>>()
    //             // );

    //             if iter.header().flags.response_code() == 3 {
    //                 // NXDOMAIN! YAY!
    //                 log::info!("domain {domain} is not yet registered or has no records.");
    //                 funny_domains.push(domain.clone());
    //             }
    //         }
    //         Err(err) => {
    //             log::error!("error occurred querying {domain}: {err:?}");
    //             continue;
    //         }
    //     }
    // }

    // std::fs::write("funny.domains", funny_domains.join("\n")).expect("unable to write domain list");
}
