// SPDX-License-Identifier: MIT
pragma solidity ^0.8.19;

// Interfaz del par Uniswap V2
interface IUniswapV2Pair {
    function swap(
        uint amount0Out,
        uint amount1Out,
        address to,
        bytes calldata data
    ) external;
    
    function getReserves() external view returns (
        uint112 reserve0,
        uint112 reserve1,
        uint32 blockTimestampLast
    );
    
    function token0() external view returns (address);
    function token1() external view returns (address);
}

// Interfaz ERC20
interface IERC20 {
    function transfer(address to, uint amount) external returns (bool);
    function balanceOf(address account) external view returns (uint);
}

contract FlashArbitrage {
    address public owner;

    constructor() {
        owner = msg.sender;
    }

    modifier soloOwner() {
        require(msg.sender == owner, "No autorizado");
        _;
    }

    // Ejecuta el arbitraje con flash swap
    function ejecutarArbitraje(
        address poolPrestamo,   // Pool del que pedimos prestado
        address poolVenta,      // Pool donde vendemos
        address token0,         // Token que pedimos prestado
        address token1,         // Token que devolvemos
        uint montoPrestamo      // Cantidad a pedir prestada
    ) external soloOwner {
        // Codificar datos para el callback
        bytes memory data = abi.encode(
            poolVenta,
            token0,
            token1,
            montoPrestamo
        );

        // Pedir prestado token0 del poolPrestamo
        IUniswapV2Pair(poolPrestamo).swap(
            montoPrestamo,
            0,
            address(this),
            data
        );
    }

    // Callback que llama el pool después de enviarnos los tokens
    function uniswapV2Call(
        address,
        uint amount0,
        uint,
        bytes calldata data
    ) external {
        // Decodificar datos
        (
            address poolVenta,
            address token0,
            address token1,
            uint montoPrestamo
        ) = abi.decode(data, (address, address, address, uint));

        // Vender token0 en el pool de venta
        require(IERC20(token0).transfer(poolVenta, amount0), "Transfer token0 fallido");
        
        IUniswapV2Pair(poolVenta).swap(
            0,
            type(uint).max,
            address(this),
            new bytes(0)
        );

        // Calcular lo que debemos devolver (0.3% de fee)
        uint deuda = (montoPrestamo * 1003) / 1000;

        // Devolver al pool prestamista
        require(IERC20(token1).transfer(msg.sender, deuda), "Transfer deuda fallido");

        // Ganancia queda en el contrato
    }

    // Retirar ganancias
    function retirar(address token) external soloOwner {
        uint saldo = IERC20(token).balanceOf(address(this));
        require(IERC20(token).transfer(owner, saldo), "Transfer retiro fallido");
    }
}
