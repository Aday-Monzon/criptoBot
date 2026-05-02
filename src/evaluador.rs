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

    // Si los datos no son válidos
    None
}
