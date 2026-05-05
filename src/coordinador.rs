use crate::constructor::{Oportunidad, construir_swap};
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

// Dirección del router de QuickSwap V2 en Polygon
const ROUTER_QUICKSWAP: &str = "0xa5E0829CaCEd8fFDD4De3c43696c57F7D7A678ff";

// Monto de entrada — 1 USDT en unidades mínimas (6 decimales)
const MONTO_ENTRADA: u64 = 1_000_000;

pub async fn ejecutar_oportunidad(
    diferencia: f64,
    pool_compra: String,
    pool_venta: String,
    precio: f64,
    wallet: &LocalWallet,
    rpc_amoy: &str,
) {
    info!(
        "⚙️  Coordinador activado — diferencia: {:.4}% — ejecutando arbitraje...",
        diferencia
    );

    // Calcular monto mínimo con 0.5% de slippage máximo
    let slippage = 0.005_f64;
    let monto_esperado = MONTO_ENTRADA as f64 / precio;
    let monto_minimo = (monto_esperado * (1.0 - slippage)) as u64;

    let oportunidad = Oportunidad {
        pool_compra: pool_compra.clone(),
        pool_venta: pool_venta.clone(),
        token_entrada: TOKEN_USDT.to_string(),
        token_salida: TOKEN_WPOL.to_string(),
        monto_entrada: U256::from(MONTO_ENTRADA),
        monto_minimo_salida: U256::from(monto_minimo),
    };

    // Construir calldata
    let calldata = construir_swap(&oportunidad, wallet.address());

    info!("📦 Transacción construida — {} bytes", calldata.len());

    // Conectar a Amoy para enviar
    let proveedor = Provider::<Http>::try_from(rpc_amoy).expect("Error conectando a Amoy");

    let wallet_amoy = wallet.clone().with_chain_id(80002u64);
    let cliente = Arc::new(SignerMiddleware::new(proveedor, wallet_amoy));

    // Aprobar token antes del swap
    crate::firmante::aprobar_token(
        wallet,
        rpc_amoy,
        TOKEN_USDT,
        ROUTER_QUICKSWAP,
        U256::from(MONTO_ENTRADA),
    )
    .await;

    // Construir transacción al router de QuickSwap
    let router: ethers::types::Address = ROUTER_QUICKSWAP.parse().expect("Router inválido");

    let tx = TransactionRequest::new()
        .to(router)
        .data(calldata)
        .value(U256::zero());

    info!("✍️  Firmando y enviando a Amoy...");

    match cliente.send_transaction(tx, None).await {
        Ok(tx_pendiente) => {
            info!(
                "✅ Transacción enviada — hash: {:?}",
                tx_pendiente.tx_hash()
            );
            info!(
                "🔗 https://amoy.polygonscan.com/tx/{:?}",
                tx_pendiente.tx_hash()
            );
        }
        Err(e) => {
            info!("❌ Error enviando: {}", e);
        }
    }
}
