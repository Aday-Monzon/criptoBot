use ethers::providers::Middleware;
use ethers::{
    middleware::SignerMiddleware,
    providers::{Http, Provider},
    signers::{LocalWallet, Signer},
    types::{TransactionRequest, U256},
};
use std::str::FromStr;
use std::sync::Arc;
use tracing::info;

pub async fn crear_wallet(clave_privada: Option<String>) -> LocalWallet {
    match clave_privada {
        Some(clave) => {
            let wallet = LocalWallet::from_str(&clave).expect("Clave privada inválida");
            info!("🔑 Wallet cargada correctamente");
            info!("📬 Dirección: {:?}", wallet.address());
            wallet
        }
        None => {
            let wallet = LocalWallet::new(&mut rand::thread_rng());
            info!("🔑 Wallet nueva generada");
            info!("📬 Dirección: {:?}", wallet.address());
            info!(
                "⚠️  Guarda esta clave en .env como CLAVE_PRIVADA: 0x{}",
                hex::encode(wallet.signer().to_bytes())
            );
            wallet
        }
    }
}

// Envía una transacción de prueba a Amoy testnet
pub async fn enviar_transaccion_prueba(wallet: &LocalWallet, rpc_amoy: &str) {
    info!("📤 Preparando transacción de prueba en Amoy...");

    // Conectar a Amoy via HTTP
    let proveedor = Provider::<Http>::try_from(rpc_amoy).expect("Error conectando a Amoy");

    // Configurar wallet con chain ID de Amoy (80002)
    let wallet = wallet.clone().with_chain_id(80002u64);

    // Crear cliente firmante
    let cliente = Arc::new(SignerMiddleware::new(proveedor, wallet.clone()));

    // Transacción simple: enviarse 0 POL a sí mismo
    let tx = TransactionRequest::new()
        .to(wallet.address())
        .value(U256::zero())
        .gas(21000);

    info!("✍️  Firmando y enviando...");

    match cliente.send_transaction(tx, None).await {
        Ok(tx_pendiente) => {
            info!(
                "✅ Transacción enviada — hash: {:?}",
                tx_pendiente.tx_hash()
            );
            info!(
                "🔗 Ver en: https://amoy.polygonscan.com/tx/{:?}",
                tx_pendiente.tx_hash()
            );
        }
        Err(e) => {
            info!("❌ Error enviando transacción: {}", e);
        }
    }
}
