use ethers::providers::{Http, Middleware, Provider};
use std::{env, time::Duration};
use tracing::info;

pub fn urls_env(principal: &str, fallbacks: &str, predeterminados: &[&str]) -> Vec<String> {
    let mut urls = Vec::new();

    for nombre in [principal, fallbacks] {
        if let Ok(valor) = env::var(nombre) {
            for url in valor.split([',', ';']) {
                let url = url.trim();
                if !url.is_empty() && !urls.iter().any(|existente| existente == url) {
                    urls.push(url.to_string());
                }
            }
        }
    }

    for url in predeterminados {
        if !urls.iter().any(|existente| existente == url) {
            urls.push((*url).to_string());
        }
    }

    urls
}

pub async fn seleccionar_http(urls: &[String]) -> Option<String> {
    for url in urls {
        let proveedor = match Provider::<Http>::try_from(url.as_str()) {
            Ok(proveedor) => proveedor.interval(Duration::from_millis(2000)),
            Err(error) => {
                info!("RPC HTTP invalido, se prueba el siguiente: {}", error);
                continue;
            }
        };

        match proveedor.get_chainid().await {
            Ok(chain_id) => {
                info!("RPC HTTP activo en chain id {}", chain_id);
                return Some(url.clone());
            }
            Err(error) => {
                info!("RPC HTTP no responde, se prueba el siguiente: {}", error);
            }
        }
    }

    None
}
