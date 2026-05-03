use crate::constructor::{Oportunidad, construir_swap};
use ethers::{
    signers::{LocalWallet, Signer},
    types::U256,
};
use tracing::info;

// Tokens del par WPOL/USDC en Polygon
const TOKEN_WPOL: &str = "0x0d500B1d8E8eF31E21C99d1Db9A6444d3ADf1270";
const TOKEN_USDC: &str = "0x2791Bca1f2de4661ED88A30C99A7a9449Aa84174";

// Monto de entrada — 1 USDC en unidades mínimas (6 decimales)
const MONTO_ENTRADA: u64 = 1_000_000;

// Coordina la ejecución cuando se detecta una oportunidad
pub async fn ejecutar_oportunidad(
    diferencia: f64,
    pool_compra: String,
    pool_venta: String,
    wallet: &ethers::signers::LocalWallet,
) {
    info!(
        "⚙️  Coordinador activado — diferencia: {:.4}% — ejecutando arbitraje...",
        diferencia
    );

    // Construir oportunidad
    let oportunidad = Oportunidad {
        pool_compra: pool_compra.clone(),
        pool_venta: pool_venta.clone(),
        token_entrada: TOKEN_USDC.to_string(),
        token_salida: TOKEN_WPOL.to_string(),
        monto_entrada: U256::from(MONTO_ENTRADA),
        monto_minimo_salida: U256::from(0), // por ahora sin slippage protection
    };

    // Construir transacción
    let calldata = construir_swap(&oportunidad, wallet.address());

    info!(
        "📦 Transacción lista — {} bytes — enviando al firmante...",
        calldata.len()
    );

    // Por ahora solo simulamos el envío
    info!("✅ Coordinador completó el ciclo — listo para firmar");
}
