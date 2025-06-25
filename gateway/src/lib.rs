pub mod proxy_client;
pub mod route_client;
pub mod auth_client;
pub mod auth;

mod auth_proto {
    tonic::include_proto!("auth");
}