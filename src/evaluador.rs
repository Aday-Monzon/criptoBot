use std::collections::HashMap;
use tracing::{debug, info};

pub struct DatosSwap {
    pub pool: String,
    pub amount0_in: u128,
    pub amount1_in: u128,
    pub amount0_out: u128,
    pub amount1_out: u128,
}

const UMBRAL_PORCENTAJE_DEFECTO: f64 = 0.8;
const DECIMALES_WPOL: f64 = 1e18;
const DECIMALES_USDT: f64 = 1e6;

fn umbral_porcentaje() -> f64 {
    std::env::var("UMBRAL_PORCENTAJE")
        .ok()
        .and_then(|valor| valor.trim().replace(',', ".").parse::<f64>().ok())
        .filter(|valor| *valor > 0.0)
        .unwrap_or(UMBRAL_PORCENTAJE_DEFECTO)
}

// Precio observado entre tokens, normalizando decimales de ambos tokens.
pub fn calcular_precio(swap: &DatosSwap) -> Option<f64> {
    if swap.amount1_in > 0 && swap.amount0_out > 0 {
        let usdt_entrada = swap.amount1_in as f64 / DECIMALES_USDT;
        let wpol_salida = swap.amount0_out as f64 / DECIMALES_WPOL;
        return Some(usdt_entrada / wpol_salida);
    }

    if swap.amount0_in > 0 && swap.amount1_out > 0 {
        let wpol_entrada = swap.amount0_in as f64 / DECIMALES_WPOL;
        let usdt_salida = swap.amount1_out as f64 / DECIMALES_USDT;
        return Some(usdt_salida / wpol_entrada);
    }

    None
}

pub fn evaluar_arbitraje(
    swap: &DatosSwap,
    precios: &mut HashMap<String, f64>,
    tokens_por_pool: &HashMap<String, (String, String)>,
) -> Option<(f64, String, String, f64)> {
    let tokens_actual = tokens_por_pool.get(&swap.pool)?;
    let precio_actual = calcular_precio(swap)?;

    precios.insert(swap.pool.clone(), precio_actual);

    for (pool_guardado, precio_guardado) in precios.iter() {
        if *pool_guardado == swap.pool {
            continue;
        }

        let tokens_guardado = match tokens_por_pool.get(pool_guardado) {
            Some(t) => t,
            None => continue,
        };

        let mismo_par = (tokens_actual.0 == tokens_guardado.0
            && tokens_actual.1 == tokens_guardado.1)
            || (tokens_actual.0 == tokens_guardado.1 && tokens_actual.1 == tokens_guardado.0);

        if !mismo_par {
            continue;
        }

        let diferencia = ((precio_actual - precio_guardado) / precio_guardado).abs() * 100.0;

        debug!(
            "Mismo par encontrado - Pool A: {} - Pool B: {} - diferencia: {:.6}%",
            swap.pool, pool_guardado, diferencia
        );

        let umbral = umbral_porcentaje();

        if diferencia >= umbral {
            let (pool_compra, pool_venta) = if precio_actual < *precio_guardado {
                (swap.pool.clone(), pool_guardado.clone())
            } else {
                (pool_guardado.clone(), swap.pool.clone())
            };

            info!(
                "OPORTUNIDAD - Par: {}/{} - Diferencia: {:.4}% - Compra: {} - Venta: {}",
                tokens_actual.0, tokens_actual.1, diferencia, pool_compra, pool_venta
            );

            return Some((diferencia, pool_compra, pool_venta, precio_actual));
        } else {
            debug!(
                "Diferencia sin oportunidad: {:.6}% < umbral {:.6}%",
                diferencia, umbral
            );
        }
    }

    None
}
