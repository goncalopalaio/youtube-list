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
        #[structopt(short)]
        list_videos: bool,
    },
    // Create a new playlist from a file
    Create {
        title: String,
        #[structopt(parse(from_os_str))]
        playlist: Option<PathBuf>,
    },
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
        Subcommands::List { .. } => {
            let playlists = request_playlists(&client);

            for p in playlists {
                println!("{:?}", p);
            }
        }
        Subcommands::Create { .. } => {}
    }
}
