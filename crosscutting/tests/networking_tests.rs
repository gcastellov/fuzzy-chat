use crosscutting::networking;
use std::net::SocketAddr;
use tonic::transport::Uri;

#[test]
fn to_http_endpoint_valid() {
    let ip = "127.0.0.1";
    let port = 8080;
    let result = networking::to_http_endpoint(ip, port);
    assert!(result.is_ok());
    assert_eq!(
        result.unwrap(),
        Uri::try_from("http://127.0.0.1:8080").unwrap()
    );
}

#[test]
fn to_https_endpoint_valid() {
    let ip = "127.0.0.1";
    let port = 443;
    let result = networking::to_https_endpoint(ip, port);
    assert!(result.is_ok());
    assert_eq!(
        result.unwrap(),
        Uri::try_from("https://127.0.0.1:443").unwrap()
    );
}

#[test]
fn to_socket_address_valid() {
    let ip = "127.0.0.1";
    let port = 8080;
    let result = networking::to_socket_address(ip, port);
    assert!(result.is_ok());
    assert_eq!(
        result.unwrap(),
        "127.0.0.1:8080".parse::<SocketAddr>().unwrap()
    );
}

#[test]
fn to_socket_address_invalid_ip() {
    let ip = "invalid_ip";
    let port = 8080;
    let result = networking::to_socket_address(ip, port);
    assert!(result.is_err());
}

#[test]
fn to_http_endpoint_empty_ip() {
    let ip = "";
    let port = 8080;
    let result = networking::to_http_endpoint(ip, port);
    assert!(result.is_ok());
}

#[test]
fn to_https_endpoint_empty_ip() {
    let ip = "";
    let port = 443;
    let result = networking::to_https_endpoint(ip, port);
    assert!(result.is_ok());
}

#[test]
fn to_socket_address_empty_ip() {
    let ip = "";
    let port = 8080;
    let result = networking::to_socket_address(ip, port);
    assert!(result.is_err());
}

#[test]
fn to_http_endpoint_large_port() {
    let ip = "127.0.0.1";
    let port = u32::MAX; // Large port number
    let result = networking::to_http_endpoint(ip, port);
    assert!(result.is_ok());
}

#[test]
fn to_https_endpoint_large_port() {
    let ip = "127.0.0.1";
    let port = u32::MAX; // Large port number
    let result = networking::to_https_endpoint(ip, port);
    assert!(result.is_ok());
}
