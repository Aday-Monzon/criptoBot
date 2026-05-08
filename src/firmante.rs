use ethers::providers::Middleware;
use ethers::types::U256;
use ethers::{
    middleware::SignerMiddleware,
    providers::{Http, Provider},
    signers::{LocalWallet, Signer},
    types::TransactionRequest,
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
    let wallet = wallet.clone().with_chain_id(137u64);

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

// Verifica y aprueba el gasto de tokens al router
pub async fn aprobar_token(
    wallet: &LocalWallet,
    rpc_amoy: &str,
    token: &str,
    router: &str,
    monto: U256,
) {
    info!("🔐 Verificando allowance del token...");

    let proveedor = Provider::<Http>::try_from(rpc_amoy).expect("Error conectando");

    let wallet_amoy = wallet.clone().with_chain_id(137u64);
    let cliente = Arc::new(SignerMiddleware::new(proveedor, wallet_amoy.clone()));

    let token_addr: ethers::types::Address = token.parse().expect("Token inválido");
    let router_addr: ethers::types::Address = router.parse().expect("Router inválido");

    // Selector de allowance(address,address)
    let allowance_data = {
        let mut data = ethers::utils::keccak256("allowance(address,address)")[..4].to_vec();
        data.extend_from_slice(&[0u8; 12]);
        data.extend_from_slice(wallet_amoy.address().as_bytes());
        data.extend_from_slice(&[0u8; 12]);
        data.extend_from_slice(router_addr.as_bytes());
        data
    };

    let tx_allowance = TransactionRequest::new()
        .to(token_addr)
        .data(allowance_data);

    match cliente.call(&tx_allowance.into(), None).await {
        Ok(resultado) => {
            let allowance = U256::from_big_endian(&resultado);
            info!("💰 Allowance actual: {}", allowance);

            if allowance < monto {
                info!("📝 Aprobando tokens al router...");

                // Selector de approve(address,uint256)
                let mut approve_data =
                    ethers::utils::keccak256("approve(address,uint256)")[..4].to_vec();
                approve_data.extend_from_slice(&[0u8; 12]);
                approve_data.extend_from_slice(router_addr.as_bytes());
                // Aprobar cantidad máxima
                let max = U256::MAX;
                let mut max_bytes = [0u8; 32];
                max.to_big_endian(&mut max_bytes);
                approve_data.extend_from_slice(&max_bytes);

                let tx_approve = TransactionRequest::new().to(token_addr).data(approve_data);

                match cliente.send_transaction(tx_approve, None).await {
                    Ok(tx_pendiente) => {
                        info!("✅ Approve enviado — hash: {:?}", tx_pendiente.tx_hash());
                        // Esperar confirmación antes de continuar
                        match tx_pendiente.await {
                            Ok(Some(recibo)) => {
                                info!("✅ Approve confirmado en bloque {:?}", recibo.block_number)
                            }
                            Ok(None) => info!("⚠️ Approve sin recibo"),
                            Err(e) => info!("❌ Error esperando approve: {}", e),
                        }
                    }
                    Err(e) => info!("❌ Error en approve: {}", e),
                }
            } else {
                info!("✅ Allowance suficiente, no necesita approve");
            }
        }
        Err(e) => info!("❌ Error verificando allowance: {}", e),
    }
}
