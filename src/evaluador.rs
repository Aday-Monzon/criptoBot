use std::collections::HashMap;
use tracing::info;

// Estructura que representa los datos de un swap
pub struct DatosSwap {
    pub pool: String,
    pub amount0_in: u128,
    pub amount1_in: u128,
    pub amount0_out: u128,
    pub amount1_out: u128,
}

// Decimales de cada token
const DECIMALES_TOKEN0: f64 = 1e18; // MATIC — 18 decimales
const DECIMALES_TOKEN1: f64 = 1e6; // USDC — 6 decimales

// Umbral mínimo de rentabilidad
const UMBRAL_PORCENTAJE: f64 = 0.1;

// Pools que vamos a comparar
const POOL_QUICKSWAP: &str = "0x6669b4706cc152f359e947bca68e263a87c52634";
const POOL_UNISWAP: &str = "0xb6e57ed85c4c9dbfef2a68711e9d6f36c56e0fcb";

// Calcula el precio implícito del swap
pub fn calcular_precio(swap: &DatosSwap) -> Option<f64> {
    // Caso 1: entraron token1 (USDC) y salieron token0 (MATIC)
    if swap.amount1_in > 0 && swap.amount0_out > 0 {
        let usdc_entrada = swap.amount1_in as f64 / DECIMALES_TOKEN1;
        let matic_salida = swap.amount0_out as f64 / DECIMALES_TOKEN0;
        return Some(usdc_entrada / matic_salida);
    }

    // Caso 2: entraron token0 (MATIC) y salieron token1 (USDC)
    if swap.amount0_in > 0 && swap.amount1_out > 0 {
        let matic_entrada = swap.amount0_in as f64 / DECIMALES_TOKEN0;
        let usdc_salida = swap.amount1_out as f64 / DECIMALES_TOKEN1;
        return Some(usdc_salida / matic_entrada);
    }

    None
}

// Evalúa si hay oportunidad de arbitraje entre dos pools
pub fn evaluar_arbitraje(swap: &DatosSwap, precios: &mut HashMap<String, f64>) -> Option<f64> {
    // Verificar que el swap es de uno de nuestros pools
    let pool_lower = swap.pool.to_lowercase();
    if pool_lower != POOL_QUICKSWAP && pool_lower != POOL_UNISWAP {
        return None;
    }

    // Calcular precio del swap actual
    if let Some(precio_actual) = calcular_precio(swap) {
        // Guardar precio del pool actual
        precios.insert(swap.pool.clone(), precio_actual);

        // Determinar el pool contrario
        let pool_contrario = if pool_lower == POOL_QUICKSWAP {
            POOL_UNISWAP
        } else {
            POOL_QUICKSWAP
        };

        // Si tenemos precio del pool contrario, comparar
        if let Some(precio_contrario) = precios.get(pool_contrario) {
            let diferencia = ((precio_actual - precio_contrario) / precio_contrario).abs() * 100.0;

            if diferencia >= UMBRAL_PORCENTAJE {
                info!(
                    "🚨 OPORTUNIDAD DETECTADA — Diferencia: {:.4}% — QuickSwap: {:.6} — Uniswap: {:.6}",
                    diferencia,
                    precios.get(POOL_QUICKSWAP).unwrap_or(&0.0),
                    precios.get(POOL_UNISWAP).unwrap_or(&0.0)
                );

                return Some(diferencia);
            }
        }
    }

    None
}
