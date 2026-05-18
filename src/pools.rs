// Lista de pools preconfigurados para Polygon.
pub struct Pool {
    pub direccion: &'static str,
    pub dex: &'static str,
    pub version: u8,
    pub par: &'static str,
    pub token_base: &'static str,
    pub token_cotizacion: &'static str,
    pub simbolo_base: &'static str,
    pub simbolo_cotizacion: &'static str,
    pub decimales_base: u32,
    pub decimales_cotizacion: u32,
}

#[derive(Clone, Copy)]
pub struct PasoTriangularV2 {
    pub pool: &'static str,
    pub dex: &'static str,
    pub token_in: &'static str,
    pub token_out: &'static str,
}

pub struct RutaTriangularV2Generada {
    pub nombre: String,
    pub token_inicio: &'static str,
    pub simbolo_inicio: &'static str,
    pub decimales_inicio: u32,
    pub pasos: Vec<PasoTriangularV2>,
}

pub const TOKEN_WPOL: &str = "0x0d500B1d8E8eF31E21C99d1Db9A6444d3ADf1270";
pub const TOKEN_USDT: &str = "0xc2132D05D31c914a87C6611C10748AEb04B58e8F";
pub const TOKEN_USDC: &str = "0x2791Bca1f2de4661ED88A30C99A7a9449Aa84174";
pub const TOKEN_DAI: &str = "0x8f3Cf7ad23Cd3CaDbD9735AFf958023239c6A063";

// Pares activos monitorizados. Los pares V2 con el mismo `par` pueden ejecutarse
// sin cambiar el contrato actual.
pub const POOLS_WPOL_USDT: &[Pool] = &[
    Pool {
        direccion: "0x9b08288c3be4f62bbf8d1c20ac9c5e6f9467d8b7",
        dex: "Uniswap V3",
        version: 3,
        par: "WPOL/USDT",
        token_base: TOKEN_WPOL,
        token_cotizacion: TOKEN_USDT,
        simbolo_base: "WPOL",
        simbolo_cotizacion: "USDT",
        decimales_base: 18,
        decimales_cotizacion: 6,
    },
    Pool {
        direccion: "0x604229c960e5cacf2aaeac8be68ac07ba9df81c3",
        dex: "QuickSwap V2",
        version: 2,
        par: "WPOL/USDT",
        token_base: TOKEN_WPOL,
        token_cotizacion: TOKEN_USDT,
        simbolo_base: "WPOL",
        simbolo_cotizacion: "USDT",
        decimales_base: 18,
        decimales_cotizacion: 6,
    },
    Pool {
        direccion: "0x781067ef296e5c4a4203f81c593274824b7c185d",
        dex: "Uniswap V3",
        version: 3,
        par: "WPOL/USDT",
        token_base: TOKEN_WPOL,
        token_cotizacion: TOKEN_USDT,
        simbolo_base: "WPOL",
        simbolo_cotizacion: "USDT",
        decimales_base: 18,
        decimales_cotizacion: 6,
    },
    Pool {
        direccion: "0x93ca061a80bfb622e7b529f6de1fde4a9129cf8e",
        dex: "Uniswap V2",
        version: 2,
        par: "WPOL/USDT",
        token_base: TOKEN_WPOL,
        token_cotizacion: TOKEN_USDT,
        simbolo_base: "WPOL",
        simbolo_cotizacion: "USDT",
        decimales_base: 18,
        decimales_cotizacion: 6,
    },
    Pool {
        direccion: "0x55ff76bffc3cdd9d5fdbbc2ece4528ecce45047e",
        dex: "SushiSwap V2",
        version: 2,
        par: "WPOL/USDT",
        token_base: TOKEN_WPOL,
        token_cotizacion: TOKEN_USDT,
        simbolo_base: "WPOL",
        simbolo_cotizacion: "USDT",
        decimales_base: 18,
        decimales_cotizacion: 6,
    },
    Pool {
        direccion: "0x65d43b64e3b31965cd5ea367d4c2b94c03084797",
        dex: "ApeSwap V2",
        version: 2,
        par: "WPOL/USDT",
        token_base: TOKEN_WPOL,
        token_cotizacion: TOKEN_USDT,
        simbolo_base: "WPOL",
        simbolo_cotizacion: "USDT",
        decimales_base: 18,
        decimales_cotizacion: 6,
    },
    Pool {
        direccion: "0x5b41eedcfc8e0ae47493d4945aa1ae4fe05430ff",
        dex: "QuickSwap V3",
        version: 3,
        par: "WPOL/USDT",
        token_base: TOKEN_WPOL,
        token_cotizacion: TOKEN_USDT,
        simbolo_base: "WPOL",
        simbolo_cotizacion: "USDT",
        decimales_base: 18,
        decimales_cotizacion: 6,
    },
    Pool {
        direccion: "0x6e7a5fafcec6bb1e78bae2a1f0b612012bf14827",
        dex: "QuickSwap V2",
        version: 2,
        par: "WPOL/USDC",
        token_base: TOKEN_WPOL,
        token_cotizacion: TOKEN_USDC,
        simbolo_base: "WPOL",
        simbolo_cotizacion: "USDC",
        decimales_base: 18,
        decimales_cotizacion: 6,
    },
    Pool {
        direccion: "0xcd353f79d9fade311fc3119b841e1f456b54e858",
        dex: "SushiSwap V2",
        version: 2,
        par: "WPOL/USDC",
        token_base: TOKEN_WPOL,
        token_cotizacion: TOKEN_USDC,
        simbolo_base: "WPOL",
        simbolo_cotizacion: "USDC",
        decimales_base: 18,
        decimales_cotizacion: 6,
    },
    Pool {
        direccion: "0xeef611894ceae652979c9d0dae1deb597790c6ee",
        dex: "QuickSwap V2",
        version: 2,
        par: "WPOL/DAI",
        token_base: TOKEN_WPOL,
        token_cotizacion: TOKEN_DAI,
        simbolo_base: "WPOL",
        simbolo_cotizacion: "DAI",
        decimales_base: 18,
        decimales_cotizacion: 18,
    },
    Pool {
        direccion: "0xd32f3139a214034a0f9777c87ee0a064c1ff6ae2",
        dex: "ApeSwap V2",
        version: 2,
        par: "WPOL/DAI",
        token_base: TOKEN_WPOL,
        token_cotizacion: TOKEN_DAI,
        simbolo_base: "WPOL",
        simbolo_cotizacion: "DAI",
        decimales_base: 18,
        decimales_cotizacion: 18,
    },
    Pool {
        direccion: "0x8929d3fea77398f64448c85015633c2d6472fb29",
        dex: "SushiSwap V2",
        version: 2,
        par: "WPOL/DAI",
        token_base: TOKEN_WPOL,
        token_cotizacion: TOKEN_DAI,
        simbolo_base: "WPOL",
        simbolo_cotizacion: "DAI",
        decimales_base: 18,
        decimales_cotizacion: 18,
    },
];

pub const POOL_QUICKSWAP_V2_USDC_USDT: &str = "0xe43ab6540c0929ef29d216a34ab1f0eacc5c3825";
pub const POOL_SUSHISWAP_V2_USDC_USDT: &str = "0x4B1F1e2435A9C96f7330FAea190Ef6A7C8D70001";
pub const POOL_APESWAP_V2_USDC_USDT: &str = "0xaf6dd86c35f573d1061ed214c6a90899fdad95b5";
pub const POOL_QUICKSWAP_V2_DAI_USDT: &str = "0x59153f27eeFE07E5eCE4f9304EBBa1DA6F53CA88";
pub const POOL_SUSHISWAP_V2_DAI_USDT: &str = "0x3b31bb4b6ba4f67f4ef54e78bcb0aaa4f53dc7ff";
pub const POOL_APESWAP_V2_DAI_USDT: &str = "0xede04e0cd393a076c49deb95d3686a52ccc49c71";
pub const POOL_QUICKSWAP_V2_DAI_USDC: &str = "0xf04adbf75cdfc5ed26eea4bbbb991db002036bdd";
pub const POOL_SUSHISWAP_V2_DAI_USDC: &str = "0xcd578f016888b57f1b1e3f887f392f0159e26747";
pub const POOL_APESWAP_V2_DAI_USDC: &str = "0x5b13B583D4317aB15186Ed660A1E4C65C10da659";
pub const POOL_APESWAP_V2_WPOL_USDC: &str = "0x019011032a7ac3a87ee885b6c08467ac46ad11cd";

fn rutas_cruzadas(
    nombre: &str,
    token_inicio: &'static str,
    simbolo_inicio: &'static str,
    decimales_inicio: u32,
    primer_salto: &[PasoTriangularV2],
    segundo_salto: &[PasoTriangularV2],
    tercer_salto: &[PasoTriangularV2],
) -> Vec<RutaTriangularV2Generada> {
    let mut rutas = Vec::new();

    for paso_1 in primer_salto {
        for paso_2 in segundo_salto {
            for paso_3 in tercer_salto {
                rutas.push(RutaTriangularV2Generada {
                    nombre: format!(
                        "{} [{} -> {} -> {}]",
                        nombre, paso_1.dex, paso_2.dex, paso_3.dex
                    ),
                    token_inicio,
                    simbolo_inicio,
                    decimales_inicio,
                    pasos: vec![*paso_1, *paso_2, *paso_3],
                });
            }
        }
    }

    rutas
}

pub fn rutas_triangulares_v2() -> Vec<RutaTriangularV2Generada> {
    let usdt_wpol = [
        PasoTriangularV2 {
            pool: "0x604229c960e5cacf2aaeac8be68ac07ba9df81c3",
            dex: "QuickSwap V2",
            token_in: TOKEN_USDT,
            token_out: TOKEN_WPOL,
        },
        PasoTriangularV2 {
            pool: "0x55ff76bffc3cdd9d5fdbbc2ece4528ecce45047e",
            dex: "SushiSwap V2",
            token_in: TOKEN_USDT,
            token_out: TOKEN_WPOL,
        },
        PasoTriangularV2 {
            pool: "0x65d43b64e3b31965cd5ea367d4c2b94c03084797",
            dex: "ApeSwap V2",
            token_in: TOKEN_USDT,
            token_out: TOKEN_WPOL,
        },
    ];
    let usdc_wpol = [
        PasoTriangularV2 {
            pool: "0x6e7a5fafcec6bb1e78bae2a1f0b612012bf14827",
            dex: "QuickSwap V2",
            token_in: TOKEN_USDC,
            token_out: TOKEN_WPOL,
        },
        PasoTriangularV2 {
            pool: "0xcd353f79d9fade311fc3119b841e1f456b54e858",
            dex: "SushiSwap V2",
            token_in: TOKEN_USDC,
            token_out: TOKEN_WPOL,
        },
        PasoTriangularV2 {
            pool: POOL_APESWAP_V2_WPOL_USDC,
            dex: "ApeSwap V2",
            token_in: TOKEN_USDC,
            token_out: TOKEN_WPOL,
        },
    ];
    let wpol_usdc = [
        PasoTriangularV2 {
            pool: "0x6e7a5fafcec6bb1e78bae2a1f0b612012bf14827",
            dex: "QuickSwap V2",
            token_in: TOKEN_WPOL,
            token_out: TOKEN_USDC,
        },
        PasoTriangularV2 {
            pool: "0xcd353f79d9fade311fc3119b841e1f456b54e858",
            dex: "SushiSwap V2",
            token_in: TOKEN_WPOL,
            token_out: TOKEN_USDC,
        },
        PasoTriangularV2 {
            pool: POOL_APESWAP_V2_WPOL_USDC,
            dex: "ApeSwap V2",
            token_in: TOKEN_WPOL,
            token_out: TOKEN_USDC,
        },
    ];
    let wpol_dai = [
        PasoTriangularV2 {
            pool: "0xeef611894ceae652979c9d0dae1deb597790c6ee",
            dex: "QuickSwap V2",
            token_in: TOKEN_WPOL,
            token_out: TOKEN_DAI,
        },
        PasoTriangularV2 {
            pool: "0x8929d3fea77398f64448c85015633c2d6472fb29",
            dex: "SushiSwap V2",
            token_in: TOKEN_WPOL,
            token_out: TOKEN_DAI,
        },
        PasoTriangularV2 {
            pool: "0xd32f3139a214034a0f9777c87ee0a064c1ff6ae2",
            dex: "ApeSwap V2",
            token_in: TOKEN_WPOL,
            token_out: TOKEN_DAI,
        },
    ];
    let usdc_usdt = [
        PasoTriangularV2 {
            pool: POOL_QUICKSWAP_V2_USDC_USDT,
            dex: "QuickSwap V2",
            token_in: TOKEN_USDC,
            token_out: TOKEN_USDT,
        },
        PasoTriangularV2 {
            pool: POOL_SUSHISWAP_V2_USDC_USDT,
            dex: "SushiSwap V2",
            token_in: TOKEN_USDC,
            token_out: TOKEN_USDT,
        },
        PasoTriangularV2 {
            pool: POOL_APESWAP_V2_USDC_USDT,
            dex: "ApeSwap V2",
            token_in: TOKEN_USDC,
            token_out: TOKEN_USDT,
        },
    ];
    let dai_usdt = [
        PasoTriangularV2 {
            pool: POOL_QUICKSWAP_V2_DAI_USDT,
            dex: "QuickSwap V2",
            token_in: TOKEN_DAI,
            token_out: TOKEN_USDT,
        },
        PasoTriangularV2 {
            pool: POOL_SUSHISWAP_V2_DAI_USDT,
            dex: "SushiSwap V2",
            token_in: TOKEN_DAI,
            token_out: TOKEN_USDT,
        },
        PasoTriangularV2 {
            pool: POOL_APESWAP_V2_DAI_USDT,
            dex: "ApeSwap V2",
            token_in: TOKEN_DAI,
            token_out: TOKEN_USDT,
        },
    ];
    let dai_usdc = [
        PasoTriangularV2 {
            pool: POOL_QUICKSWAP_V2_DAI_USDC,
            dex: "QuickSwap V2",
            token_in: TOKEN_DAI,
            token_out: TOKEN_USDC,
        },
        PasoTriangularV2 {
            pool: POOL_SUSHISWAP_V2_DAI_USDC,
            dex: "SushiSwap V2",
            token_in: TOKEN_DAI,
            token_out: TOKEN_USDC,
        },
        PasoTriangularV2 {
            pool: POOL_APESWAP_V2_DAI_USDC,
            dex: "ApeSwap V2",
            token_in: TOKEN_DAI,
            token_out: TOKEN_USDC,
        },
    ];

    let mut rutas = Vec::new();
    rutas.extend(rutas_cruzadas(
        "USDT -> WPOL -> USDC -> USDT",
        TOKEN_USDT,
        "USDT",
        6,
        &usdt_wpol,
        &wpol_usdc,
        &usdc_usdt,
    ));
    rutas.extend(rutas_cruzadas(
        "USDT -> WPOL -> DAI -> USDT",
        TOKEN_USDT,
        "USDT",
        6,
        &usdt_wpol,
        &wpol_dai,
        &dai_usdt,
    ));
    rutas.extend(rutas_cruzadas(
        "USDC -> WPOL -> DAI -> USDC",
        TOKEN_USDC,
        "USDC",
        6,
        &usdc_wpol,
        &wpol_dai,
        &dai_usdc,
    ));

    rutas
}
