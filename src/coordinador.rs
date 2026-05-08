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
const TOKEN_USDT: &str = "0xc2132D05D31c914a87C6611C10748AEb04B58e8F";

// Contrato FlashSwapArbitrage desplegado en Polygon mainnet
const CONTRATO_ARBITRAGE: &str = "0x99822a9C9A22DB1F3a7ABa5a996d04314435492f";

// Monto a pedir prestado — 1000 WPOL
const MONTO_ENTRADA: u128 = 1_000_000_000_000_000_000_000;

// Dirección de MetaMask para recibir ganancias
const WALLET_DESTINO: &str = "0x15a9361ECFC5552eE2040aE72eB7B2402b646E65";

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

            let token = std::env::var("TELEGRAM_TOKEN").unwrap_or_default();
            let chat_id = std::env::var("TELEGRAM_CHAT_ID").unwrap_or_default();

            // Notificar éxito
            enviar_telegram(&token, &chat_id, &format!(
                "🚨 Arbitraje ejecutado!\nDiferencia: {:.4}%\nHash: {}\nhttps://polygonscan.com/tx/{}",
                diferencia, hash, hash
            )).await;

            // Retirar ganancia del contrato a la wallet del bot
            info!("💰 Retirando ganancia a wallet...");
            let selector_retirar = &ethers::utils::keccak256("retirar(address)")[..4];
            let usdt_addr: ethers::types::Address = TOKEN_USDT.parse().expect("USDT inválido");

            let mut calldata_retirar = selector_retirar.to_vec();
            calldata_retirar.extend_from_slice(&ethers::abi::encode(&[
                ethers::abi::Token::Address(usdt_addr),
            ]));

            let tx_retirar = TransactionRequest::new()
                .to(contrato_addr)
                .data(calldata_retirar)
                .value(U256::zero());

            match cliente.send_transaction(tx_retirar, None).await {
                Ok(tx_ret) => {
                    let hash_ret = format!("{:?}", tx_ret.tx_hash());
                    info!("✅ Ganancia retirada — hash: {}", hash_ret);

                    // Transferir USDT de la wallet del bot a MetaMask
                    info!("📤 Transfiriendo ganancia a MetaMask...");
                    let usdt_contrato: ethers::types::Address =
                        TOKEN_USDT.parse().expect("USDT inválido");
                    let destino: ethers::types::Address =
                        WALLET_DESTINO.parse().expect("Destino inválido");

                    // Consultar saldo USDT de la wallet del bot
                    let selector_balance = &ethers::utils::keccak256("balanceOf(address)")[..4];
                    let mut calldata_balance = selector_balance.to_vec();
                    calldata_balance.extend_from_slice(&ethers::abi::encode(&[
                        ethers::abi::Token::Address(wallet.address()),
                    ]));

                    let tx_balance = TransactionRequest::new()
                        .to(usdt_contrato)
                        .data(calldata_balance);

                    if let Ok(saldo_bytes) = cliente.call(&tx_balance.into(), None).await {
                        let saldo = U256::from_big_endian(&saldo_bytes);
                        if saldo > U256::zero() {
                            let selector_transfer =
                                &ethers::utils::keccak256("transfer(address,uint256)")[..4];
                            let mut calldata_transfer = selector_transfer.to_vec();
                            calldata_transfer.extend_from_slice(&ethers::abi::encode(&[
                                ethers::abi::Token::Address(destino),
                                ethers::abi::Token::Uint(saldo),
                            ]));

                            let tx_transfer = TransactionRequest::new()
                                .to(usdt_contrato)
                                .data(calldata_transfer)
                                .value(U256::zero());

                            match cliente.send_transaction(tx_transfer, None).await {
                                Ok(tx_t) => {
                                    let hash_t = format!("{:?}", tx_t.tx_hash());
                                    info!("✅ USDT enviado a MetaMask — hash: {}", hash_t);
                                    enviar_telegram(&token, &chat_id, &format!(
                                        "💸 USDT enviados a tu MetaMask!\nhttps://polygonscan.com/tx/{}",
                                        hash_t
                                    )).await;
                                }
                                Err(e) => info!("❌ Error transfiriendo a MetaMask: {}", e),
                            }
                        }
                    }
                }
                Err(e) => info!("❌ Error retirando: {}", e),
            }
        }
        Err(e) => {
            info!("❌ Error enviando: {}", e);
        }
    }
}
