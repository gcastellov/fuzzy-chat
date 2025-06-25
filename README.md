# Fuzzy chat
This is an experimental project aiming to connect users anonymously through a dynamic network of proxies.
Controllers initialize the conversation and dispatch the proxy routing. This way the user connects directly to a proxy chosen by the controller based on different factors. Behind the scenes the conversation happens with several proxies until reaching the final hop.

## Anonymity
Controllers hold routing data strictly necessary for dispatching messages, after that, all routing information expire and is removed from the system. Additionally, all information has associated a TTL and it is removed when is no longer needed.
Connections are established through the network of controllers and proxies and never direcly with users.

## Security
All connections are encrypted using gRPC with TLS.

### In-memory & Redis support
The controllers support data persistance either in memory or in Redis. In-memory is the default choice. However, you can change this setting by switching the environment variable `REPOSITORY` to `1`.

Beware that the in-memory repository is not persistent. If you want to persist the data, you need to use Redis and set up the Redis instance accordingly. A single controller instance must be used when using the in-memory repository. If load balancing is needed, you need to switch to Redis.

### Run the binary crates with Cargo

To run the controller use the following command:
```
# cargo r --bin controller --config .\controller\.cargo\config.toml
```

You can run a couple of proxies by using:
```
# cargo r --bin proxy --config ./proxy/.cargo/config1.toml
# cargo r --bin proxy --config ./proxy/.cargo/config2.toml
```

Additionally, you can run a couple of clients by using:
```
# cargo r --bin client --config ./client/.cargo/config1.toml
# cargo r --bin client --config ./client/.cargo/config2.toml
```

### Run a network with docker-compose
For development purposes as well, there is a docker-compose file that can be used to run the whole network with docker-compose. You can find the file inside the `docker` directory of this solution.

Ensure you set up the volume with the necessary content inside. 
Change the following environment variables pointing to the right paths: `LOGS_DIR`, `CERTS_DIR` and `MEMBERS_CSV_FILE`.


Set up you `hosts` file to give a DNS to all components:
```
127.0.0.1 controller
127.0.0.1 proxy-one
127.0.0.1 proxy-two
127.0.0.1 proxy-three
127.0.0.1 proxy-four
127.0.0.1 proxy-five
127.0.0.1 client1
127.0.0.1 client2
127.0.0.1 client3
127.0.0.1 client4
```

### Generate server certificates

Under `assets/certs` you can find the certificates for the controllers. This is only for development purposes. For production, you need to generate your own certificates. The following commands can be used to generate a self-signed certificate for the controllers.

Create private key for the CA
```
# openssl genrsa -out ca.key 4096
```

Create self-signed certificate for the CA
```
# openssl req -x509 -new -nodes -key ca.key -sha256 -days 3650 -out ca.crt -subj "/C=US/ST=State/L=City/O=ExampleOrg/OU=IT Department/CN=example.com"
```

Create a Private Key for the Domain Controller
```
# openssl genrsa -out server.key 2048
```

Create a Certificate Signing Request (CSR) for the DC
```
# openssl req -new -key server.key -out dc.csr -subj "/C=US/ST=State/L=City/O=ExampleOrg/OU=IT Department/CN=your.domain.controller.com"
```

Create an OpenSSL Config for SAN (Subject Alternative Names). Create a file called dc.ext:
```
authorityKeyIdentifier=keyid,issuer
basicConstraints=CA:FALSE
keyUsage = digitalSignature, keyEncipherment
extendedKeyUsage = serverAuth
subjectAltName = @alt_names

[alt_names]
DNS.1 = your.domain.controller.com
```

Sign the DC Certificate with the Root CA
```
# openssl x509 -req -in dc.csr -CA ca.crt -CAkey ca.key -CAcreateserial -out server.crt -days 825 -sha256 -extfile dc.ext
```


To automate the whole process, you can execute the following script:
```
# sh ./scripts/generate-certs.sh
```
To generate all certificates for the involved components of the docker compose, you can execute the following script. Ensure you copy the scripts to the volume being used and change the settings if required.

```
# sh ./scripts/generate_multiple_certs.sh
```

### Available commands (ATM)

Gets network status:
```
# /status 
```

Send message to another user:
```
# /send client2 hello world!
```

### Run tests
Run tests sequentially by using:
```
# cargo test -- --test-threads=1
```