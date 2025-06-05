pub mod auth;
pub mod auth_client;

mod auth_proto {
    tonic::include_proto!("auth");
}
