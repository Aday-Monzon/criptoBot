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
const UMBRAL_PORCENTAJE: f64 = 0.1;

// Calcula el precio implícito del swap
pub fn calcular_precio(swap: &DatosSwap) -> Option<f64> {
    if swap.amount1_in > 0 && swap.amount0_out > 0 {
        let entrada = swap.amount1_in as f64;
        let salida = swap.amount0_out as f64;
        return Some(entrada / salida);
    }
    if swap.amount0_in > 0 && swap.amount1_out > 0 {
        let entrada = swap.amount0_in as f64;
        let salida = swap.amount1_out as f64;
        return Some(salida / entrada);
    }
    None
}

// Evalúa si hay oportunidad de arbitraje entre pools del mismo par
pub fn evaluar_arbitraje(
    swap: &DatosSwap,
    precios: &mut HashMap<String, f64>,
    tokens_por_pool: &HashMap<String, (String, String)>,
) -> Option<(f64, String, String, f64)> {
    // Obtener tokens del pool actual
    let tokens_actual = tokens_por_pool.get(&swap.pool)?;

    // Calcular precio del swap actual
    let precio_actual = calcular_precio(swap)?;

    // Guardar precio PRIMERO
    precios.insert(swap.pool.clone(), precio_actual);

    // Luego comparar con todos los demás pools
    for (pool_guardado, precio_guardado) in precios.iter() {
        // No comparar el pool consigo mismo
        if *pool_guardado == swap.pool {
            continue;
        }

        // Obtener tokens del pool guardado
        let tokens_guardado = match tokens_por_pool.get(pool_guardado) {
            Some(t) => t,
            None => continue,
        };

        // Solo comparar si tienen el mismo par de tokens
        let mismo_par = (tokens_actual.0 == tokens_guardado.0
            && tokens_actual.1 == tokens_guardado.1)
            || (tokens_actual.0 == tokens_guardado.1 && tokens_actual.1 == tokens_guardado.0);

        if !mismo_par {
            continue;
        }

        let diferencia = ((precio_actual - precio_guardado) / precio_guardado).abs() * 100.0;

        info!(
            "🔎 Mismo par encontrado — Pool A: {} — Pool B: {} — diferencia: {:.6}%",
            swap.pool, pool_guardado, diferencia
        );

        if diferencia >= UMBRAL_PORCENTAJE {
            info!(
                "🚨 OPORTUNIDAD — Par: {}/{} — Diferencia: {:.4}% — Compra: {} — Venta: {}",
                tokens_actual.0, tokens_actual.1, diferencia, swap.pool, pool_guardado
            );
            return Some((
                diferencia,
                swap.pool.clone(),
                pool_guardado.clone(),
                precio_actual,
            ));
        }
    }

    None
}
