extern crate google_youtube3 as youtube3;
use google_youtube3::YouTube;
use std::path::Path;

use std::fs;
use yup_oauth2::ApplicationSecret;
use yup_oauth2::Authenticator;
use yup_oauth2::DefaultAuthenticatorDelegate;
use yup_oauth2::MemoryStorage;

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use structopt::StructOpt;

use scraper::{Html, Selector};

// This is the maximum number of items that will be returned per API call
// Some API calls need to be called multiple times to get all the items.
const MAX_RESULTS: u32 = 40;

#[derive(Debug, PartialEq, StructOpt)]
#[structopt(about = "Manages youtube playlists")]
enum Subcommands {
    // Saves all playlists information to a json file. Since the API doesn't have access to the Watch Later playlist, you will have to use the other option.
    SavePlaylistsToJson {
        /// Output file, stdout if not present
        #[structopt(parse(from_os_str))]
        output_file: Option<PathBuf>,
    },
    // Parses an html that was saved from the Watch Later playlist page and saves the available information to a json file.
    SaveWatchLaterHtmlToJson {
        #[structopt(parse(from_os_str))]
        input_file: Option<PathBuf>,
        #[structopt(parse(from_os_str))]
        output_file: Option<PathBuf>,
    },
}

// Only serves a convenience wrapper for the hub type.
struct YoutubeClient {
    hub: youtube3::YouTube<
        hyper::Client,
        yup_oauth2::Authenticator<
            yup_oauth2::DefaultAuthenticatorDelegate,
            yup_oauth2::MemoryStorage,
            hyper::Client,
        >,
    >,
}

#[derive(serde::Serialize, Deserialize, Debug)]
struct Playlist {
    title: String,
    description: String,
    channel_title: String,
    tags: String,
    published_at: String,
    id: String,
    status: String,
    items: Vec<PlaylistItem>,
}

#[derive(Serialize, Deserialize, Debug)]
struct PlaylistItem {
    title: String,
    link: String,
    published_at: String,
    position_in_playlist: u32,
    description: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct SimplePlaylistItem {
    title: String,
    channel_name: String,
    link: String,
    id: String,
}

#[derive(Debug, PartialEq, StructOpt)]
struct Opt {
    /// Application secrets json file. Get it from console.developers.google.com, YouTube Data API v3
    #[structopt(parse(from_os_str))]
    application_secret_file: Option<PathBuf>,

    #[structopt(subcommand)]
    sub: Subcommands,
}

impl Playlist {
    fn new() -> Playlist {
        Playlist {
            title: String::new(),
            description: String::new(),
            channel_title: String::new(),
            tags: String::new(),
            published_at: String::new(),
            id: String::new(),
            status: String::new(),
            items: Vec::new(),
        }
    }
}

impl PlaylistItem {
    fn new() -> PlaylistItem {
        PlaylistItem {
            title: String::new(),
            link: String::new(),
            published_at: String::new(),
            position_in_playlist: 0u32,
            description: String::new(),
        }
    }
}

fn request_playlists(client: &YoutubeClient) -> Vec<youtube3::Playlist> {
    let mut curr_page_token = String::new();
    let mut playlists = Vec::new();
    loop {
        let (_response, result) = client
            .hub
            .playlists()
            .list("snippet")
            .mine(true)
            .max_results(MAX_RESULTS)
            .page_token(&curr_page_token)
            .doit()
            .expect("Error fetching playlists");

        playlists.append(&mut result.items.unwrap());

        match result.next_page_token {
            Some(token) => curr_page_token = token,
            None => break,
        }
    }

    return playlists;
}

fn parse_playlist_items(client: &YoutubeClient, playlist_id: &str) -> Vec<youtube3::PlaylistItem> {
    let mut curr_page_token = String::new();
    let mut playlist_items = Vec::new();

    loop {
        let (_resp, result) = client
            .hub
            .playlist_items()
            .list("snippet,contentDetails")
            .playlist_id(playlist_id)
            .max_results(MAX_RESULTS)
            .page_token(&curr_page_token)
            .doit()
            .expect(&format!(
                "Error requesting playlist items for playlist_id: {}",
                playlist_id
            ));

        playlist_items.append(&mut result.items.unwrap());

        match result.next_page_token {
            Some(s) => curr_page_token = s,
            None => break,
        }
    }

    return playlist_items;
}

fn get_text(option: &Option<String>, default: &str) -> String {
    match option {
        Some(text) => text.clone(),
        None => default.to_string(),
    }
}

fn parse_playlist(playlist: &youtube3::Playlist) -> Playlist {
    let default_status = youtube3::PlaylistStatus {
        privacy_status: Some("Unknown".to_string()),
    };

    let playlist_id = get_text(&playlist.id, "");
    let playlist_status = get_text(
        &playlist
            .status
            .clone()
            .unwrap_or(default_status)
            .privacy_status,
        "Unknown",
    );

    let mut info = Playlist::new();

    if let Some(snippet) = &playlist.snippet {
        info.title = snippet.title.clone().unwrap_or((&"NO_TITLE").to_string());
        info.description = snippet.description.clone().unwrap_or((&"").to_string());
        info.channel_title = snippet
            .channel_title
            .clone()
            .unwrap_or((&"NO_TITLE").to_string());
        info.tags = snippet.tags.clone().unwrap_or(Vec::new()).join(", ");
        info.published_at = snippet
            .published_at
            .clone()
            .unwrap_or((&"NO_TITLE").to_string());
    }

    info.id = playlist_id;
    info.status = playlist_status;

    return info;
}

fn parse_playlist_item(item: &youtube3::PlaylistItem) -> PlaylistItem {
    let mut info = PlaylistItem::new();

    match &item.snippet {
        Some(snippet) => {
            info.title = get_text(&snippet.title, "");

            match &item.content_details {
                Some(details) => {
                    let video_id = get_text(&details.video_id, "");
                    info.published_at = get_text(&details.video_published_at, "");

                    info.link = format!("https://www.youtube.com/watch?v={}", video_id);
                }
                None => {}
            }

            info.position_in_playlist = snippet.position.unwrap_or(0u32);
            info.published_at = get_text(&snippet.published_at, "");
            info.description = get_text(&snippet.description, "");
        }
        None => {}
    }

    return info;
}

fn split_video_id(link: &str) -> String {
    let parts = link.split("watch?v=");
    let parts = parts.collect::<Vec<&str>>();
    let parts = parts[1].split("&list=");
    let parts = parts.collect::<Vec<&str>>();

    return parts[0].to_string();
}

fn main() {
    let opt = Opt::from_args();
    println!("Arguments: {:?}", opt);

    // You will have to create an application in console.developers.google.com to use this.
    // In particular, once you're there, search for YouTube Data API v3 and go to credentials.
    // You can download this file by creating a new OAuth client ID credential.
    let application_secret_file = if let Some(path) = opt.application_secret_file {
        path
    } else {
        std::path::Path::new("../client_secret_console_developers_google_com.json").to_path_buf()
    };

    let secret: ApplicationSecret = yup_oauth2::read_application_secret(&application_secret_file)
        .expect("Secrets file not found");

    let auth = Authenticator::new(
        &secret,
        DefaultAuthenticatorDelegate,
        hyper::Client::with_connector(hyper::net::HttpsConnector::new(
            hyper_rustls::TlsClient::new(),
        )),
        <MemoryStorage as Default>::default(),
        None,
    );

    let hub = YouTube::new(
        hyper::Client::with_connector(hyper::net::HttpsConnector::new(
            hyper_rustls::TlsClient::new(),
        )),
        auth,
    );

    let client = YoutubeClient { hub: hub };

    match opt.sub {
        Subcommands::SavePlaylistsToJson { output_file } => {
            let mut output_playlists = Vec::<Playlist>::new();
            let playlists = request_playlists(&client);

            for p in playlists {
                let mut playlist = parse_playlist(&p);

                match p.id {
                    Some(ref id) => {
                        let items = parse_playlist_items(&client, &id);

                        let mut playlist_items = Vec::<PlaylistItem>::new();
                        for item in items {
                            let playlist_item = parse_playlist_item(&item);
                            playlist_items.push(playlist_item);
                        }

                        playlist.items = playlist_items;
                    }
                    None => {
                        eprintln!("Error: Failed to get playlist id from playlist: {:?}", p);
                        continue;
                    }
                }

                output_playlists.push(playlist);
            }

            let path = if let Some(path) = output_file {
                path
            } else {
                Path::new("youtube-output.json").to_path_buf()
            };

            let json_text = serde_json::to_string(&output_playlists);
            if let Ok(text) = json_text {
                fs::write(path, &text).expect("Unable to write file");
                println!("Wrote {} items", output_playlists.len());
            }
        }
        Subcommands::SaveWatchLaterHtmlToJson {
            input_file,
            output_file,
        } => {
            if let Some(input_path) = input_file {
                let contents = fs::read_to_string(input_path).expect("Failed to read input file");

                let mut playlist_items = Vec::<SimplePlaylistItem>::new();

                let html = Html::parse_fragment(&contents);

                let item_selector = Selector::parse("#content").unwrap();
                let video_title = Selector::parse("#video-title").unwrap();
                let channel_title = Selector::parse("#text").unwrap();
                let video_link = Selector::parse("#content > a").unwrap();

                let items = html.select(&item_selector);
                for item in items {
                    let mut title = item.select(&video_title);
                    let mut channel = item.select(&channel_title);
                    let mut video_link = item.select(&video_link);

                    let item_title = if let Some(a) = title.next() {
                        let item_title = a.text().collect::<String>().trim().to_string();
                        println!("{:?}", item_title);
                        item_title
                    } else {
                        println!("No title?");
                        String::new()
                    };

                    let item_channel = if let Some(a) = channel.next() {
                        let item_channel = a.text().collect::<String>().trim().to_string();
                        println!("{:?}", item_channel);
                        item_channel
                    } else {
                        println!("No channel title?");
                        String::new()
                    };

                    let item_link = if let Some(a) = video_link.next() {
                        let item_link = a.value().attr("href").unwrap_or("").to_string();
                        println!("{:?}", item_link);
                        let video_id = split_video_id(&item_link);
                        println!("{:?}", video_id);
                        (item_link, video_id)
                    } else {
                        println!("No video_link?");
                        (String::new(), String::new())
                    };

                    println!("");

                    let item = SimplePlaylistItem {
                        title: item_title,
                        channel_name: item_channel,
                        id: item_link.1,
                        link: item_link.0,
                    };

                    playlist_items.push(item);
                }

                let playlist_items = playlist_items.iter().filter(|x| !x.id.is_empty()).collect::<Vec<&SimplePlaylistItem>>();

                let path = if let Some(path) = output_file {
                    path
                } else {
                    Path::new("youtube-output-wl.json").to_path_buf()
                };

                let json_text = serde_json::to_string(&playlist_items);
                if let Ok(text) = json_text {
                    fs::write(path, &text).expect("Unable to write file");
                    println!("Wrote {} items", playlist_items.len());
                }
            } else {
                eprintln!("Could not find input file: {:?}", input_file);
            };
        }
    }
}
