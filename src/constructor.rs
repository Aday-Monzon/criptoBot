use ethers::{
    abi::{Abi, Token},
    types::{Address, U256},
};
use tracing::info;

// Dirección del router de QuickSwap en Amoy testnet
const ROUTER_QUICKSWAP: &str = "0x8954AfA98594b838bda56FE4C12a09D7739D179b";

// Estructura que representa una oportunidad de arbitraje
pub struct Oportunidad {
    pub pool_compra: String,
    pub pool_venta: String,
    pub token_entrada: String,
    pub token_salida: String,
    pub monto_entrada: U256,
    pub monto_minimo_salida: U256,
}

// Construye los datos de la transacción de swap
pub fn construir_swap(oportunidad: &Oportunidad, wallet: Address) -> Vec<u8> {
    info!(
        "🔨 Construyendo transacción — monto: {} — pool: {}",
        oportunidad.monto_entrada, oportunidad.pool_compra
    );

    // Dirección de entrada y salida
    let token_entrada: Address = oportunidad
        .token_entrada
        .parse()
        .expect("Token entrada inválido");

    let token_salida: Address = oportunidad
        .token_salida
        .parse()
        .expect("Token salida inválido");

    // Deadline — 5 minutos desde ahora
    let deadline = U256::from(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
            + 300,
    );

    // Codificar llamada a swapExactTokensForTokens
    let tokens = vec![
        Token::Uint(oportunidad.monto_entrada),
        Token::Uint(oportunidad.monto_minimo_salida),
        Token::Array(vec![
            Token::Address(token_entrada),
            Token::Address(token_salida),
        ]),
        Token::Address(wallet),
        Token::Uint(deadline),
    ];

    // Selector de función swapExactTokensForTokens
    let selector = &ethers::utils::keccak256(
        "swapExactTokensForTokens(uint256,uint256,address[],address,uint256)",
    )[..4];

    let encoded = ethers::abi::encode(&tokens);

    let mut calldata = selector.to_vec();
    calldata.extend_from_slice(&encoded);

    info!("✅ Transacción construida — {} bytes", calldata.len());

    calldata
}
