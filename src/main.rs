use cmd_lib::{run_cmd, spawn_with_output};
use colored::Colorize;
use select::document::Document;
use select::predicate::{Attr, Name, Predicate};
use serde::{Deserialize, Serialize};
use std::io::Write;

const BASE_URL: &str = "https://flixhq.to";
const PROVIDER: &str = "Vidcloud";

#[derive(Deserialize, Serialize, Debug)]
struct Iframe {
    link: String,
}

// Add subtitles later
#[derive(Deserialize, Serialize, Debug)]
struct Sources {
    sources: String,
    tracks: Vec<File>
}

#[derive(Deserialize, Serialize, Debug)]
struct File {
    file: String,
    label: Option<String>
}

#[derive(Deserialize, Serialize, Debug)]
struct Video {
    file: String
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let query = get_query();
    let search_url = format!("{}/search/{}", BASE_URL, query);
    let response = reqwest::get(&search_url).await?.text().await?;
    let document = Document::from(response.as_str());

    let nodes: Vec<_> = document
        .find(Attr("class", "film-name").descendant(Name("a")))
        .collect();

    for (i, node) in document
        .find(Attr("class", "film-name").descendant(Name("a")))
        .enumerate()
    {
        println!(
            "({}] ([{}]) |{:?}|",
            i + 1,
            node.text().cyan(),
            node.attr("href").unwrap()
        );
    }

    print!("Enter your choice: ");
    std::io::stdout().flush().unwrap();
    let mut choice = String::new();
    std::io::stdin().read_line(&mut choice).unwrap();
    let choice = choice.trim().parse::<usize>().unwrap();

    let selected_node = &nodes[choice - 1];

    let _movie_title = selected_node.text();
    let movie_id = selected_node.attr("href").unwrap();

    let episode_id = get_movie_page(movie_id).await?;
    let embed_link = get_embed_link(episode_id).await?;
    let iframe: Iframe = serde_json::from_str(&embed_link).unwrap();

    let link_parts: Vec<&str> = iframe.link.split("/").collect();
    let id_parts: Vec<&str> = link_parts[3].split("-").collect();

    let provider_link = link_parts[0..3].join("/");
    let mut source_id = link_parts[4].to_string();

    for _ in 0..3 {
        source_id.pop();
    }

    let embed_type = id_parts[1];
    let key = reqwest::get(format!(
        "https://raw.githubusercontent.com/enimax-anime/key/e{}/key.txt",
        embed_type
    ))
    .await?
    .text()
    .await?;
    let client = reqwest::Client::new();

    let resp = client
        .get(format!(
            "{}/ajax/embed-{}/getSources?id={}",
            provider_link, embed_type, source_id
        ))
        .header("X-Requested-With", "XMLHttpRequest")
        .send()
        .await?
        .text()
        .await?;

    let source: Sources = serde_json::from_str(&resp).unwrap();
    let streaming_link = source.sources;

    let mut proc = spawn_with_output! (
         printf "%s" ${streaming_link} | base64 -d | openssl enc -aes-256-cbc -d -md md5 -k ${key}
    )?;

    let bandwidth = proc.wait_with_output()?.to_string();

    let video: Vec<Video> = serde_json::from_str(&bandwidth).unwrap();
    let mpv_link = &video[0].file;
    run_cmd!(
        mpv ${mpv_link} --fs
    )?;

    Ok(())
}

fn get_query() -> String {
    if let Some(arg) = std::env::args().nth(1) {
        arg
    } else {
        let mut input = String::new();
        print!("Enter a movie name: ");
        std::io::stdout().flush().unwrap();
        std::io::stdin().read_line(&mut input).unwrap();
        input.trim().replace(" ", "-")
    }
}

async fn get_movie_page(movie_id: &str) -> Result<String, Box<dyn std::error::Error>> {
    let idx = movie_id.rfind('-').unwrap();
    let (_before, after) = movie_id.split_at(idx + 1);
    let movie_id = after;
    let movie_page = reqwest::get(format!("{}/ajax/movie/episodes/{}", BASE_URL, movie_id))
        .await?
        .text()
        .await?;
    let document = Document::from(movie_page.as_str());
    let mut episode_id = "";
    for node in document.find(Attr("class", "nav-item").descendant(Name("a"))) {
        let movie_result = (
            node.attr("data-linkid").unwrap(),
            node.attr("title").unwrap().contains(PROVIDER),
        );

        if movie_result.1 == true {
            episode_id = movie_result.0;
        }
    }

    Ok(episode_id.to_string())
}

async fn get_embed_link(episode_id: String) -> Result<String, Box<dyn std::error::Error>> {
    let embed_page = reqwest::get(format!("{}/ajax/sources/{}", BASE_URL, episode_id))
        .await?
        .text()
        .await?;
    let _document = Document::from(embed_page.as_str());

    Ok(embed_page.to_string())
}
