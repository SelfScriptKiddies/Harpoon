use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};

use rcgen::{CertificateParams, DistinguishedName, KeyPair};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
use rustls::ServerConfig;

use crate::error::HarpoonError;

pub struct CertAuthority {
    ca_key: KeyPair,
    ca_cert_der: CertificateDer<'static>,
    ca_cert: rcgen::Certificate,
    cache: Mutex<HashMap<String, Arc<ServerConfig>>>,
}

impl CertAuthority {
    pub fn from_pem_files(cert_path: &Path, key_path: &Path) -> Result<Self, HarpoonError> {
        let cert_pem = std::fs::read_to_string(cert_path)
            .map_err(|e| HarpoonError::Config(format!("reading CA cert: {e}")))?;
        let key_pem = std::fs::read_to_string(key_path)
            .map_err(|e| HarpoonError::Config(format!("reading CA key: {e}")))?;

        let ca_key = KeyPair::from_pem(&key_pem)
            .map_err(|e| HarpoonError::Config(format!("parsing CA key: {e}")))?;

        // Parse DER from PEM for rustls
        let mut cert_reader = std::io::BufReader::new(cert_pem.as_bytes());
        let certs: Vec<CertificateDer<'static>> = rustls_pemfile::certs(&mut cert_reader)
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| HarpoonError::Config(format!("parsing CA cert DER: {e}")))?;

        let ca_cert_der = certs
            .into_iter()
            .next()
            .ok_or_else(|| HarpoonError::Config("no certificate found in PEM".into()))?;

        // Create a self-signed CA params for signing leaf certs
        let mut ca_params = CertificateParams::new(vec![])
            .map_err(|e| HarpoonError::Config(format!("creating CA params: {e}")))?;
        ca_params.is_ca = rcgen::IsCa::Ca(rcgen::BasicConstraints::Unconstrained);
        let mut dn = DistinguishedName::new();
        dn.push(rcgen::DnType::CommonName, "Harpoon CA");
        ca_params.distinguished_name = dn;

        let ca_cert = ca_params
            .self_signed(&ca_key)
            .map_err(|e| HarpoonError::Config(format!("self-signing CA: {e}")))?;

        Ok(Self {
            ca_key,
            ca_cert_der,
            ca_cert,
            cache: Mutex::new(HashMap::new()),
        })
    }

    pub fn get_or_create_server_config(
        &self,
        server_name: &str,
    ) -> Result<Arc<ServerConfig>, HarpoonError> {
        {
            let cache = self.cache.lock().unwrap();
            if let Some(config) = cache.get(server_name) {
                return Ok(config.clone());
            }
        }

        let config = self.generate_server_config(server_name)?;
        let config = Arc::new(config);

        {
            let mut cache = self.cache.lock().unwrap();
            cache.insert(server_name.to_string(), config.clone());
        }

        Ok(config)
    }

    fn generate_server_config(
        &self,
        server_name: &str,
    ) -> Result<ServerConfig, HarpoonError> {
        let leaf_key = KeyPair::generate()
            .map_err(|e| HarpoonError::Config(format!("generating leaf key: {e}")))?;

        let mut params = CertificateParams::new(vec![server_name.to_string()])
            .map_err(|e| HarpoonError::Config(format!("creating cert params: {e}")))?;

        let mut dn = DistinguishedName::new();
        dn.push(rcgen::DnType::CommonName, server_name);
        params.distinguished_name = dn;

        let leaf_cert = params
            .signed_by(&leaf_key, &self.ca_cert, &self.ca_key)
            .map_err(|e| HarpoonError::Config(format!("signing leaf cert: {e}")))?;

        let leaf_cert_der = CertificateDer::from(leaf_cert.der().to_vec());
        let leaf_key_der =
            PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(leaf_key.serialize_der()));

        let config = ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(
                vec![leaf_cert_der, self.ca_cert_der.clone()],
                leaf_key_der,
            )
            .map_err(|e| HarpoonError::Config(format!("building server config: {e}")))?;

        Ok(config)
    }
}

impl std::fmt::Debug for CertAuthority {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CertAuthority")
            .field("cache_size", &self.cache.lock().unwrap().len())
            .finish()
    }
}
