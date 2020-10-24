use std::io::Write;
use std::io::BufWriter;
use google_youtube3::YouTube;
use yup_oauth2::ApplicationSecret;
use yup_oauth2::Authenticator;
use yup_oauth2::DefaultAuthenticatorDelegate;
use yup_oauth2::MemoryStorage;

extern crate google_youtube3 as youtube3;

use std::path::PathBuf;
use structopt::StructOpt;

// This is the maximum number of items that will be returned per API call
// Some API calls need to be called multiple times to get all the items.
const MAX_RESULTS: u32 = 40;

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

struct TextOutput {
    lines: Vec<String>,
    write_to_file: bool,
    output_file: Option<PathBuf>
}

#[derive(Debug, PartialEq, StructOpt)]
struct Opt {
    /// Application secrets json file. Get it from console.developers.google.com, YouTube Data API v3
    #[structopt(parse(from_os_str))]
    application_secret_file: Option<PathBuf>,

    #[structopt(subcommand)]
    sub: Subcommands,
}

#[derive(Debug, PartialEq, StructOpt)]
#[structopt(about = "Manages youtube playlists")]
enum Subcommands {
    // List playlists
    List {
        /// Output file, stdout if not present
        #[structopt(parse(from_os_str))]
        output_file: Option<PathBuf>,
    },
    // Create a new playlist from a file
    Create {
        title: String,
        #[structopt(parse(from_os_str))]
        playlist: Option<PathBuf>,
    },
}

impl TextOutput {
    fn new(output_file: Option<PathBuf>) -> TextOutput{
        let has_output_file = output_file.is_some();
        TextOutput {
            lines: Vec::new(),
            output_file: output_file,
            write_to_file: has_output_file
        }
    }

    fn print(&mut self, text: String) {
        if self.write_to_file {
            self.lines.push(text);
        } else {
            println!("{}", text);
        }
    }

    fn write_to_output(&mut self) {
        if self.write_to_file {
            let path = self.output_file.as_ref().expect("Failed to get output file path");
            let file = std::fs::File::create(&path).expect("Failed to create file");
            let mut f = BufWriter::new(file);

            for line in &self.lines {
                let _ = write!(f, "{}", line);
            }
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

fn request_playlist_items(
    client: &YoutubeClient,
    playlist_id: &str,
) -> Vec<youtube3::PlaylistItem> {
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

fn print_playlist(output: &mut TextOutput, playlist: &youtube3::Playlist) {
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

    if let Some(snippet) = &playlist.snippet {
        let title = snippet.title.clone().unwrap_or((&"NO_TITLE").to_string());
        let description = snippet.description.clone().unwrap_or((&"").to_string());
        let channel_title = snippet
            .channel_title
            .clone()
            .unwrap_or((&"NO_TITLE").to_string());
        let tags = snippet.tags.clone().unwrap_or(Vec::new()).join("-");
        let published_at = snippet
            .published_at
            .clone()
            .unwrap_or((&"NO_TITLE").to_string());

        output.print(format!("# Playlist title: {}\n", title));
        output.print(format!("Playlist channel: {}\n", channel_title));
        output.print(format!("Description: {}\n", description));
        output.print(format!("Tags: {}\n", tags));
        output.print(format!("Published at: {}\n", published_at));
    } else {
        output.print(format!("# Title: No information"));
    }

    output.print(format!("Id: {}", playlist_id));
    output.print(format!("Status: {}", playlist_status));
    output.print(format!(""));
}

fn print_playlist_item(output: &mut TextOutput, item: &youtube3::PlaylistItem) {
    match &item.snippet {
        Some(snippet) => {
            output.print("\n\n".to_string());
            output.print(format!("### {}", get_text(&snippet.title, "No video title")));

            match &item.content_details {
                Some(details) => {
                    let video_id = get_text(&details.video_id, "No video id");
                    let video_published_at =
                        get_text(&details.video_published_at, "No published date");

                    output.print(format!("[Link](https://www.youtube.com/watch?v={})\n", video_id));
                    output.print(format!("Video published at: {}\n", video_published_at));
                }
                None => {}
            }
            output.print(format!(
                "Position in playlist: {}\n",
                &snippet.position.unwrap_or(0u32)
            ));
            output.print(format!(
                "Added to playlist at: {}\n",
                get_text(&snippet.published_at, "Unknown date")
            ));

            output.print(format!(
                "Description: {}\n\n",
                get_text(&snippet.description, "No description")
            ));
        }
        None => {}
    }

    output.print(format!("------------"));
    output.print(format!(""));
}

fn main() {
    let opt = Opt::from_args();
    println!("{:?}", opt);

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
        Subcommands::List { output_file } => {
            let mut output = TextOutput::new(output_file);
            let playlists = request_playlists(&client);

            for p in playlists {
                print_playlist(&mut output, &p);

                match p.id {
                    Some(ref id) => {
                        let items = request_playlist_items(&client, &id);
                        output.print(format!("Number of items: {}", items.len()));

                        for item in items {
                            print_playlist_item(&mut output, &item);
                        }
                    }
                    None => {
                        eprintln!("Error: Failed to get playlist id from playlist: {:?}", p);
                        continue;
                    }
                }
            }

            output.write_to_output();
        }
        Subcommands::Create { .. } => {}
    }
}
