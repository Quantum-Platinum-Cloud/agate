use gemini_fetch::{Header, Page, Status};
use std::io::Read;
use std::net::{SocketAddr, ToSocketAddrs};
use std::process::{Command, Stdio};
use url::Url;

static BINARY_PATH: &'static str = env!("CARGO_BIN_EXE_agate");

fn addr(port: u16) -> SocketAddr {
    use std::net::{IpAddr, Ipv4Addr};

    (IpAddr::V4(Ipv4Addr::LOCALHOST), port)
        .to_socket_addrs()
        .unwrap()
        .next()
        .unwrap()
}

fn get(args: &[&str], addr: SocketAddr, url: &str) -> Result<Page, anyhow::Error> {
    // start the server
    let mut server = Command::new(BINARY_PATH)
        .stderr(Stdio::piped())
        .current_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data"))
        .args(args)
        .spawn()
        .expect("failed to start binary");

    // the first output is when Agate is listening, so we only have to wait
    // until the first byte is output
    let mut buffer = [0; 1];
    server
        .stderr
        .as_mut()
        .unwrap()
        .read_exact(&mut buffer)
        .unwrap();

    // actually perform the request
    let page = tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(async { Page::fetch_from(&Url::parse(url).unwrap(), addr, None).await });

    // try to stop the server again
    match server.try_wait() {
        Err(e) => panic!("cannot access orchestrated program: {:?}", e),
        // everything fine, still running as expected, kill it now
        Ok(None) => server.kill().unwrap(),
        Ok(Some(_)) => {
            // forward stderr so we have a chance to understand the problem
            let buffer = std::iter::once(Ok(buffer[0]))
                .chain(server.stderr.take().unwrap().bytes())
                .collect::<Result<Vec<u8>, _>>()
                .unwrap();

            eprintln!("{}", String::from_utf8_lossy(&buffer));
            // make the test fail
            panic!("program had crashed");
        }
    }

    page
}

#[test]
fn index_page() {
    let page = get(&[], addr(1965), "gemini://localhost").expect("could not get page");

    assert_eq!(
        page.header,
        Header {
            status: Status::Success,
            meta: "text/gemini".to_string(),
        }
    );

    assert_eq!(
        page.body,
        Some(
            std::fs::read_to_string(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/tests/data/content/index.gmi"
            ))
            .unwrap()
        )
    );
}

#[test]
fn meta() {
    let page = get(
        &["--addr", "[::]:1966"],
        addr(1966),
        "gemini://localhost/test",
    )
    .expect("could not get page");

    assert_eq!(
        page.header,
        Header {
            status: Status::Success,
            meta: "text/html".to_string(),
        }
    );
}

#[test]
fn meta_param() {
    let page = get(
        &["--addr", "[::]:1967"],
        addr(1967),
        "gemini://localhost/test.gmi",
    )
    .expect("could not get page");

    assert_eq!(
        page.header,
        Header {
            status: Status::Success,
            meta: "text/gemini;lang=en ;charset=us-ascii".to_string(),
        }
    );
}

#[test]
fn glob() {
    let page = get(
        &["--addr", "[::]:1968"],
        addr(1968),
        "gemini://localhost/test.de.gmi",
    )
    .expect("could not get page");

    assert_eq!(
        page.header,
        Header {
            status: Status::Success,
            meta: "text/gemini;lang=de".to_string(),
        }
    );
}

#[test]
fn doubleglob() {
    let page = get(
        &["--addr", "[::]:1969", "-C"],
        addr(1969),
        "gemini://localhost/testdir/a.nl.gmi",
    )
    .expect("could not get page");

    assert_eq!(
        page.header,
        Header {
            status: Status::Success,
            meta: "text/gemini;lang=nl".to_string(),
        }
    );
}

#[test]
fn full_header_preset() {
    let page = get(
        &["--addr", "[::]:1970"],
        addr(1970),
        "gemini://localhost/gone.txt",
    )
    .expect("could not get page");

    assert_eq!(
        page.header,
        Header {
            status: Status::Gone,
            meta: "This file is no longer available.".to_string(),
        }
    );
}

#[test]
fn hostname_check() {
    let page = get(
        &["--addr", "[::]:1971", "--hostname", "example.org"],
        addr(1971),
        "gemini://example.com/",
    )
    .expect("could not get page");

    assert_eq!(page.header.status, Status::ProxyRequestRefused);
}

#[test]
fn port_check() {
    let page = get(
        &["--addr", "[::]:1972", "--hostname", "example.org"],
        addr(1972),
        "gemini://example.org:1971/",
    )
    .expect("could not get page");

    assert_eq!(page.header.status, Status::ProxyRequestRefused);
}

#[test]
fn secret_nonexistent() {
    let page = get(
        &["--addr", "[::]:1973"],
        addr(1973),
        "gemini://localhost/.secret",
    )
    .expect("could not get page");

    assert_eq!(page.header.status, Status::Gone);
}

#[test]
fn secret_exists() {
    let page = get(
        &["--addr", "[::]:1974"],
        addr(1974),
        "gemini://localhost/.meta",
    )
    .expect("could not get page");

    assert_eq!(page.header.status, Status::Gone);
}

#[test]
fn serve_secret() {
    let page = get(
        &["--addr", "[::]:1975", "--serve-secret"],
        addr(1975),
        "gemini://localhost/.meta",
    )
    .expect("could not get page");

    assert_eq!(page.header.status, Status::Success);
}