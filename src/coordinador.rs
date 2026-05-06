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
const CONTRATO_ARBITRAGE: &str = "0x8e4523546efab22bce9b7001507aeba8304e57fb";

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
    let proveedor = Provider::<Http>::try_from(rpc_amoy).expect("Error conectando a Polygon");

    let wallet_amoy = wallet.clone().with_chain_id(137u64);
    let cliente = Arc::new(SignerMiddleware::new(proveedor, wallet_amoy));

    // Aprobar token antes del swap
    crate::firmante::aprobar_token(
        wallet,
        rpc_amoy,
        TOKEN_USDT,
        CONTRATO_ARBITRAGE,
        U256::from(MONTO_ENTRADA),
    )
    .await;

    // Construir transacción al router de QuickSwap
    // Selector de ejecutarArbitraje(address,address,address,address,uint256)
    let selector =
        &ethers::utils::keccak256("ejecutarArbitraje(address,address,address,address,uint256)")
            [..4];

    let pool_compra_addr: ethers::types::Address =
        pool_compra.parse().expect("Pool compra inválido");
    let pool_venta_addr: ethers::types::Address = pool_venta.parse().expect("Pool venta inválido");
    let token0_addr: ethers::types::Address = TOKEN_WPOL.parse().expect("Token0 inválido");
    let token1_addr: ethers::types::Address = TOKEN_USDT.parse().expect("Token1 inválido");

    let mut calldata_contrato = selector.to_vec();
    calldata_contrato.extend_from_slice(&ethers::abi::encode(&[
        ethers::abi::Token::Address(pool_compra_addr),
        ethers::abi::Token::Address(pool_venta_addr),
        ethers::abi::Token::Address(token0_addr),
        ethers::abi::Token::Address(token1_addr),
        ethers::abi::Token::Uint(U256::from(MONTO_ENTRADA)),
    ]));

    let contrato_addr: ethers::types::Address =
        CONTRATO_ARBITRAGE.parse().expect("Contrato inválido");

    let tx = TransactionRequest::new()
        .to(contrato_addr)
        .data(calldata_contrato)
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
