use core::net::SocketAddr;
use std::error::Error;
use tonic::transport::Uri;

pub fn to_http_endpoint(ip: &str, port: u32) -> Result<Uri, Box<dyn Error>> {
    to_endpoint("http", ip, port)
}

pub fn to_https_endpoint(ip: &str, port: u32) -> Result<Uri, Box<dyn Error>> {
    to_endpoint("https", ip, port)
}

pub fn to_socket_address(ip: &str, port: u16) -> Result<SocketAddr, Box<dyn Error>> {
    let address = format!("{}:{}", ip, port);
    let socket_address: SocketAddr = address.parse()?;
    Ok(socket_address)
}

fn to_endpoint(scheme: &str, ip: &str, port: u32) -> Result<Uri, Box<dyn Error>> {
    let endpoint = format!("{}://{}:{}", scheme, ip, port);
    let uri = Uri::try_from(endpoint).map_err(|_| "Invalid endpoint URI")?;
    Ok(uri)
}
