use ethers::{
    middleware::SignerMiddleware,
    providers::{Http, Middleware, Provider},
    signers::{LocalWallet, Signer},
    types::{TransactionRequest, U256},
};
use std::sync::Arc;
use tracing::info;

// Tokens del par WPOL/USDT en Polygon
const TOKEN_WPOL: &str = "0x0d500B1d8E8eF31E21C99d1Db9A6444d3ADf1270";

// Contrato FlashSwapArbitrage desplegado en Polygon mainnet
const CONTRATO_ARBITRAGE: &str = "0x99822a9C9A22DB1F3a7ABa5a996d04314435492f";

// Monto a pedir prestado — 1000 WPOL
const MONTO_ENTRADA: u128 = 1_000_000_000_000_000_000_000;

pub async fn enviar_telegram(token: &str, chat_id: &str, mensaje: &str) {
    let url = format!("https://api.telegram.org/bot{}/sendMessage", token);
    let cliente = reqwest::Client::new();
    let _ = cliente
        .post(&url)
        .json(&serde_json::json!({
            "chat_id": chat_id,
            "text": mensaje
        }))
        .send()
        .await;
}

pub async fn ejecutar_oportunidad(
    diferencia: f64,
    pool_compra: String,
    pool_venta: String,
    _precio: f64,
    wallet: &LocalWallet,
    rpc_mainnet: &str,
) {
    info!(
        "⚙️  Coordinador activado — diferencia: {:.4}% — ejecutando arbitraje...",
        diferencia
    );

    // Conectar a Polygon mainnet
    let proveedor = Provider::<Http>::try_from(rpc_mainnet).expect("Error conectando a Polygon");

    let wallet_mainnet = wallet.clone().with_chain_id(137u64);
    let cliente = Arc::new(SignerMiddleware::new(proveedor, wallet_mainnet));

    // Selector de ejecutarArbitraje(address,address,address,uint256)
    let selector =
        &ethers::utils::keccak256("ejecutarArbitraje(address,address,address,uint256)")[..4];

    let pool_compra_addr: ethers::types::Address =
        pool_compra.parse().expect("Pool compra inválido");
    let pool_venta_addr: ethers::types::Address = pool_venta.parse().expect("Pool venta inválido");
    let token_prestamo_addr: ethers::types::Address = TOKEN_WPOL.parse().expect("Token inválido");

    let mut calldata = selector.to_vec();
    calldata.extend_from_slice(&ethers::abi::encode(&[
        ethers::abi::Token::Address(pool_compra_addr),
        ethers::abi::Token::Address(pool_venta_addr),
        ethers::abi::Token::Address(token_prestamo_addr),
        ethers::abi::Token::Uint(U256::from(MONTO_ENTRADA)),
    ]));

    let contrato_addr: ethers::types::Address =
        CONTRATO_ARBITRAGE.parse().expect("Contrato inválido");

    let tx = TransactionRequest::new()
        .to(contrato_addr)
        .data(calldata)
        .value(U256::zero());

    // Simular primero
    info!("🧪 Simulando transacción...");
    match cliente.call(&tx.clone().into(), None).await {
        Ok(_) => info!("✅ Simulación exitosa, enviando..."),
        Err(e) => {
            info!("❌ Simulación falló: {}", e);

            // Notificar simulación fallida por Telegram
            let token = std::env::var("TELEGRAM_TOKEN").unwrap_or_default();
            let chat_id = std::env::var("TELEGRAM_CHAT_ID").unwrap_or_default();
            let mensaje = format!(
                "⚠️ Oportunidad detectada pero simulación falló\nDiferencia: {:.4}%\nError: {}",
                diferencia, e
            );
            enviar_telegram(&token, &chat_id, &mensaje).await;
            return;
        }
    }

    info!("✍️  Firmando y enviando a Polygon mainnet...");
    match cliente.send_transaction(tx, None).await {
        Ok(tx_pendiente) => {
            let hash = format!("{:?}", tx_pendiente.tx_hash());
            info!("✅ Transacción enviada — hash: {}", hash);
            info!("🔗 https://polygonscan.com/tx/{}", hash);

            // Notificar éxito por Telegram
            let token = std::env::var("TELEGRAM_TOKEN").unwrap_or_default();
            let chat_id = std::env::var("TELEGRAM_CHAT_ID").unwrap_or_default();
            let mensaje = format!(
                "🚨 Arbitraje ejecutado!\nDiferencia: {:.4}%\nHash: {}\nhttps://polygonscan.com/tx/{}",
                diferencia, hash, hash
            );
            enviar_telegram(&token, &chat_id, &mensaje).await;
        }
        Err(e) => {
            info!("❌ Error enviando: {}", e);
        }
    }
}
