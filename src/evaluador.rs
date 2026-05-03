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

// Umbral mínimo de rentabilidad
const UMBRAL_PORCENTAJE: f64 = 0.0001;

// Calcula el precio implícito del swap
pub fn calcular_precio(swap: &DatosSwap) -> Option<f64> {
    // Caso 1: entraron token1 y salieron token0
    if swap.amount1_in > 0 && swap.amount0_out > 0 {
        let entrada = swap.amount1_in as f64;
        let salida = swap.amount0_out as f64;
        return Some(entrada / salida);
    }

    // Caso 2: entraron token0 y salieron token1
    if swap.amount0_in > 0 && swap.amount1_out > 0 {
        let entrada = swap.amount0_in as f64;
        let salida = swap.amount1_out as f64;
        return Some(salida / entrada);
    }

    None
}

// Evalúa si hay oportunidad de arbitraje entre cualquier par de pools
pub fn evaluar_arbitraje(
    swap: &DatosSwap,
    precios: &mut HashMap<String, f64>,
) -> Option<(f64, String, String)> {
    // Calcular precio del swap actual
    if let Some(precio_actual) = calcular_precio(swap) {
        // Comparar con todos los pools que ya tenemos guardados
        for (pool_guardado, precio_guardado) in precios.iter() {
            // No comparar el pool consigo mismo
            if *pool_guardado == swap.pool {
                continue;
            }

            let diferencia = ((precio_actual - precio_guardado) / precio_guardado).abs() * 100.0;

            if diferencia >= UMBRAL_PORCENTAJE {
                info!(
                    "🚨 OPORTUNIDAD DETECTADA — Diferencia: {:.6}% — Pool A: {} — Pool B: {}",
                    diferencia, swap.pool, pool_guardado
                );

                return Some((diferencia, swap.pool.clone(), pool_guardado.clone()));
            }
        }

        // Guardar precio de este pool para comparaciones futuras
        precios.insert(swap.pool.clone(), precio_actual);
    }

    None
}
