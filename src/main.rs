// BLESS THIS MESS

extern crate irc;
extern crate getopts;
extern crate hyper;
extern crate time;
extern crate rustc_serialize;

#[macro_use]
extern crate log;

use irc::client::prelude::*;
use getopts::Options;
use std::env;

mod frog_log;

mod search {
    use rustc_serialize::json;
    use hyper;
    use std::io::Read;
    use std::io;

    pub struct Searcher {
        api_key: String,
        client: hyper::Client,
    }

    type TipNum = u64;

    #[allow(dead_code)]
    #[derive(RustcDecodable)]
    pub struct Tip {
        approved: bool,
        moderated: bool,
        tweeted: Option<u64>,
        pub number: TipNum,
        pub tip: String,
    }

    #[derive(RustcDecodable)]
    struct SearchResults {
        results: Vec<Tip>,
    }

    #[derive(RustcEncodable)]
    struct SearchQuery {
        tip: Option<String>,
    }

    #[derive(Debug)]
    pub enum TipError {
        StatusNotOk(hyper::status::StatusCode),
        Network(hyper::Error),
        Decoding(json::DecoderError),
        Search(json::EncoderError),
        Io(io::Error),
    }

    impl From<hyper::Error> for TipError {
        fn from(err: hyper::Error) -> TipError {
            TipError::Network(err)
        }
    }

    impl From<json::DecoderError> for TipError {
        fn from(err: json::DecoderError) -> TipError {
            TipError::Decoding(err)
        }
    }

    impl From<io::Error> for TipError {
        fn from(err: io::Error) -> TipError {
            TipError::Io(err)
        }
    }

    impl From<json::EncoderError> for TipError {
        fn from(err: json::EncoderError) -> TipError {
            TipError::Search(err)
        }
    }

    impl Searcher {
        pub fn new(api_key: String) -> Searcher {
            let client = hyper::Client::new();
            Searcher {
                api_key: api_key,
                client: client,
            }
        }

        /// Searches for a tip containing the given text. The server may or
        /// may not return the results you want.
        ///
        /// All untweeted, tweeted, approved and unapproved tips are returned.
        ///
        /// # Arguments
        /// * `text` - Some text
        pub fn search(&self, text: String) -> Result<Vec<Tip>, TipError> {
            let query = SearchQuery {
                tip: Some(text),
            };
            let body = try!(json::encode(&query));

            let mut resp = try!(
                self.client.post("https://frog.tips/api/2/tips/search")
                           .body(&body)
                           .header(hyper::header::Authorization(self.api_key.clone()))
                           .header(hyper::header::Connection::close())
                           .send());

            if resp.status != hyper::Ok {
                return Err(TipError::StatusNotOk(resp.status));
            }

            let mut body = String::new();
            try!(resp.read_to_string(&mut body));
            let results: SearchResults = try!(json::decode(&body));
            Ok(results.results)
        }
    }
}

fn main() {
    let matches = {
        let args: Vec<String> = env::args().collect();
        let program = args[0].clone();

        let opts = Options::new();
        let matches = match opts.parse(&args[1..]) {
            Ok(m) => m,
            Err(f) => { panic!(f.to_string()) }
        };

        if matches.free.is_empty() {
            panic!(format!("usage: {} CONFIG API-KEY", program));
        }

        matches
    };

    // INIT FROGGING
    frog_log::init().unwrap();

    // INIT GOGGING
    let searcher = {
        let api_key = matches.free[1].clone();
        search::Searcher::new(api_key)
    };

    // INIT SOBBING
    let server = {
        let config_name = matches.free[0].clone();
        IrcServer::new(config_name).unwrap()
    };

    server.identify().unwrap();

    for message in server.iter() {
        // Let's be real careful in here and not fucking panic, alrights boys?
        if let Ok(message) = message {
            // LOG YOUR FROG
            info!("{:?}", message);

            if let Command::PRIVMSG(ref target, ref msg) = message.command {
                if let Some(source_nickname) = message.source_nickname() {
                    let current_nickname_with_colon = format!("{}:", server.current_nickname());

                    if !msg.starts_with(current_nickname_with_colon.as_str()) {
                        continue;
                    }

                    let message = msg.split(current_nickname_with_colon.as_str())
                                     .collect::<Vec<&str>>()
                                     .join("");

                    match searcher.search(message) {
                        Ok(tips) => {
                            if tips.is_empty() {
                                server.send_privmsg(target, "NO TIPS FOUND <SFX: SAD TROMBONE>");
                                continue;
                            }

                            for tip in tips.iter().take(3) {
                                server.send_privmsg(target, format!("{}: {} - {}", source_nickname, tip.number, tip.tip).as_str());
                            }

                            let rest = tips.iter()
                                           .skip(3)
                                           .take(100)
                                           .map(|t| t.number.to_string())
                                           .collect::<Vec<String>>()
                                           .join(" ");
                            if !rest.is_empty() {
                                server.send_privmsg(target, format!("{}: also {}", source_nickname, rest).as_str());
                            }
                        },
                        Err(why) => {
                            server.send_privmsg(target, format!("{:?}", why).as_str());
                        }
                    }
                }
            }
        }
    }
}
