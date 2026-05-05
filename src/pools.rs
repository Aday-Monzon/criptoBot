// Lista de pools preconfigurados para WPOL en Polygon
pub struct Pool {
    pub direccion: &'static str,
    pub dex: &'static str,
    pub version: u8,
}

// WPOL/USDT — pools activos
pub const POOLS_WPOL_USDT: &[Pool] = &[
    Pool {
        direccion: "0x9b08288c3be4f62bbf8d1c20ac9c5e6f9467d8b7",
        dex: "Uniswap V3",
        version: 3,
    },
    Pool {
        direccion: "0x604229c960e5cacf2aaeac8be68ac07ba9df81c3",
        dex: "QuickSwap V2",
        version: 2,
    },
    Pool {
        direccion: "0x781067ef296e5c4a4203f81c593274824b7c185d",
        dex: "Uniswap V3",
        version: 3,
    },
    Pool {
        direccion: "0x93ca061a80bfb622e7b529f6de1fde4a9129cf8e",
        dex: "Uniswap V2",
        version: 2,
    },
    Pool {
        direccion: "0x55ff76bffc3cdd9d5fdbbc2ece4528ecce45047e",
        dex: "SushiSwap V2",
        version: 2,
    },
    Pool {
        direccion: "0x65d43b64e3b31965cd5ea367d4c2b94c03084797",
        dex: "ApeSwap V2",
        version: 2,
    },
    Pool {
        direccion: "0x5b41eedcfc8e0ae47493d4945aa1ae4fe05430ff",
        dex: "QuickSwap V3",
        version: 3,
    },
];
