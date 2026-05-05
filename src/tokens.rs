use ethers::{
    abi::Abi,
    contract::Contract,
    providers::{Provider, Ws},
    types::Address,
};
use std::sync::Arc;
use tracing::info;

// ABI mínimo para consultar token0 y token1 de un pool
const ABI_POOL: &str = r#"[
    {
        "inputs": [],
        "name": "token0",
        "outputs": [{"internalType": "address", "name": "", "type": "address"}],
        "stateMutability": "view",
        "type": "function"
    },
    {
        "inputs": [],
        "name": "token1",
        "outputs": [{"internalType": "address", "name": "", "type": "address"}],
        "stateMutability": "view",
        "type": "function"
    }
]"#;

// Consulta los tokens de un pool directamente en la blockchain
pub async fn obtener_tokens(pool: &str, proveedor: Arc<Provider<Ws>>) -> Option<(String, String)> {
    let direccion: Address = match pool.parse() {
        Ok(d) => d,
        Err(e) => {
            info!("❌ Error parseando dirección {}: {}", pool, e);
            return None;
        }
    };

    let abi: Abi = match serde_json::from_str(ABI_POOL) {
        Ok(a) => a,
        Err(e) => {
            info!("❌ Error parseando ABI: {}", e);
            return None;
        }
    };

    let contrato = Contract::new(direccion, abi, proveedor);

    let token0: Address = match contrato
        .method::<_, Address>("token0", ())
        .ok()?
        .call()
        .await
    {
        Ok(t) => t,
        Err(e) => {
            info!("❌ Error llamando token0 en {}: {}", pool, e);
            return None;
        }
    };

    let token1: Address = match contrato
        .method::<_, Address>("token1", ())
        .ok()?
        .call()
        .await
    {
        Ok(t) => t,
        Err(e) => {
            info!("❌ Error llamando token1 en {}: {}", pool, e);
            return None;
        }
    };

    Some((
        format!("{:?}", token0).to_lowercase(),
        format!("{:?}", token1).to_lowercase(),
    ))
}
