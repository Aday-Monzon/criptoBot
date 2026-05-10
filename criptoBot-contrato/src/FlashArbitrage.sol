// SPDX-License-Identifier: MIT
pragma solidity ^0.8.19;

interface IUniswapV2Pair {
    function swap(uint amount0Out, uint amount1Out, address to, bytes calldata data) external;
    function getReserves() external view returns (uint112 reserve0, uint112 reserve1, uint32);
    function token0() external view returns (address);
    function token1() external view returns (address);
}

interface IUniswapV3Pool {
    function swap(
        address recipient,
        bool zeroForOne,
        int256 amountSpecified,
        uint160 sqrtPriceLimitX96,
        bytes calldata data
    ) external returns (int256 amount0, int256 amount1);
    function token0() external view returns (address);
    function token1() external view returns (address);
}

interface IERC20 {
    function balanceOf(address account) external view returns (uint);
}

contract FlashArbitrage {
    address public owner;

    struct CallbackData {
        uint8 routeType;
        address poolPrestamo;
        address poolCompra;
        address poolVenta;
        address tokenPrestamo;
        address tokenIntermedio;
        uint montoPrestamo;
        uint minProfit;
    }

    struct V3SwapData {
        address pool;
        address tokenIn;
    }

    modifier soloOwner() {
        require(msg.sender == owner, "No autorizado");
        _;
    }

    constructor() {
        owner = msg.sender;
    }

    function ejecutarArbitraje(
        address poolPrestamo,
        address poolVenta,
        address tokenPrestamo,
        address tokenRepago,
        uint montoPrestamo,
        uint minProfit
    ) external soloOwner {
        require(montoPrestamo > 0, "Monto cero");
        require(poolPrestamo != poolVenta, "Pools iguales");

        IUniswapV2Pair pair = IUniswapV2Pair(poolPrestamo);
        require(
            _contienePar(poolPrestamo, tokenPrestamo, tokenRepago),
            "Pool prestamo invalido"
        );
        require(
            _contienePar(poolVenta, tokenPrestamo, tokenRepago),
            "Pool venta invalido"
        );

        bytes memory data = abi.encode(
            CallbackData({
                poolPrestamo: poolPrestamo,
                poolCompra: poolPrestamo,
                poolVenta: poolVenta,
                routeType: 0,
                tokenPrestamo: tokenPrestamo,
                tokenIntermedio: tokenRepago,
                montoPrestamo: montoPrestamo,
                minProfit: minProfit
            })
        );

        if (pair.token0() == tokenPrestamo) {
            pair.swap(montoPrestamo, 0, address(this), data);
        } else {
            pair.swap(0, montoPrestamo, address(this), data);
        }
    }

    function ejecutarArbitrajeV3(
        address poolPrestamoV2,
        address poolCompraV3,
        address poolVentaV3,
        address tokenPrestamo,
        address tokenIntermedio,
        uint montoPrestamo,
        uint minProfit
    ) external soloOwner {
        require(montoPrestamo > 0, "Monto cero");
        require(poolCompraV3 != poolVentaV3, "Pools iguales");
        require(
            _contieneParV2(poolPrestamoV2, tokenPrestamo, tokenIntermedio),
            "Pool prestamo invalido"
        );
        require(
            _contieneParV3(poolCompraV3, tokenPrestamo, tokenIntermedio),
            "Pool compra invalido"
        );
        require(
            _contieneParV3(poolVentaV3, tokenPrestamo, tokenIntermedio),
            "Pool venta invalido"
        );

        IUniswapV2Pair pair = IUniswapV2Pair(poolPrestamoV2);
        bytes memory data = abi.encode(
            CallbackData({
                routeType: 1,
                poolPrestamo: poolPrestamoV2,
                poolCompra: poolCompraV3,
                poolVenta: poolVentaV3,
                tokenPrestamo: tokenPrestamo,
                tokenIntermedio: tokenIntermedio,
                montoPrestamo: montoPrestamo,
                minProfit: minProfit
            })
        );

        if (pair.token0() == tokenPrestamo) {
            pair.swap(montoPrestamo, 0, address(this), data);
        } else {
            pair.swap(0, montoPrestamo, address(this), data);
        }
    }

    function uniswapV2Call(address, uint amount0, uint amount1, bytes calldata data) external {
        CallbackData memory decoded = abi.decode(data, (CallbackData));
        require(msg.sender == decoded.poolPrestamo, "Callback no autorizado");

        uint recibido = amount0 > 0 ? amount0 : amount1;
        require(recibido == decoded.montoPrestamo, "Monto inesperado");

        if (decoded.routeType == 0) {
            _ejecutarCallbackV2(decoded);
        } else if (decoded.routeType == 1) {
            _ejecutarCallbackV3(decoded);
        } else {
            revert("Ruta invalida");
        }
    }

    function uniswapV3SwapCallback(int256 amount0Delta, int256 amount1Delta, bytes calldata data) external {
        V3SwapData memory decoded = abi.decode(data, (V3SwapData));
        require(msg.sender == decoded.pool, "Callback V3 no autorizado");

        uint amountToPay;
        if (amount0Delta > 0) {
            require(IUniswapV3Pool(decoded.pool).token0() == decoded.tokenIn, "Token0 inesperado");
            amountToPay = uint(amount0Delta);
        } else if (amount1Delta > 0) {
            require(IUniswapV3Pool(decoded.pool).token1() == decoded.tokenIn, "Token1 inesperado");
            amountToPay = uint(amount1Delta);
        } else {
            revert("Delta invalido");
        }

        _safeTransfer(decoded.tokenIn, decoded.pool, amountToPay);
    }

    function _ejecutarCallbackV2(CallbackData memory decoded) internal {
        uint deuda = _calcularRepagoV2(
            decoded.poolPrestamo,
            decoded.tokenPrestamo,
            decoded.tokenIntermedio,
            decoded.montoPrestamo
        );

        uint salida = _venderEnPool(
            decoded.poolVenta,
            decoded.tokenPrestamo,
            decoded.tokenIntermedio,
            decoded.montoPrestamo
        );

        require(salida > deuda + decoded.minProfit, "Sin beneficio suficiente");
        _safeTransfer(decoded.tokenIntermedio, decoded.poolPrestamo, deuda);
    }

    function _ejecutarCallbackV3(CallbackData memory decoded) internal {
        uint deuda = _calcularRepagoMismoTokenV2(
            decoded.poolPrestamo,
            decoded.tokenPrestamo,
            decoded.montoPrestamo
        );

        uint cantidadIntermedia = _swapV3ExactInput(
            decoded.poolCompra,
            decoded.tokenPrestamo,
            decoded.tokenIntermedio,
            decoded.montoPrestamo
        );

        uint salida = _swapV3ExactInput(
            decoded.poolVenta,
            decoded.tokenIntermedio,
            decoded.tokenPrestamo,
            cantidadIntermedia
        );

        require(salida > deuda + decoded.minProfit, "Sin beneficio suficiente");
        _safeTransfer(decoded.tokenPrestamo, decoded.poolPrestamo, deuda);
    }

    function retirar(address token) external soloOwner {
        uint saldo = IERC20(token).balanceOf(address(this));
        _safeTransfer(token, owner, saldo);
    }

    function _venderEnPool(
        address pool,
        address tokenIn,
        address tokenOut,
        uint amountIn
    ) internal returns (uint amountOut) {
        (uint reserveIn, uint reserveOut) = _reservasOrdenadas(pool, tokenIn, tokenOut);
        amountOut = _getAmountOut(amountIn, reserveIn, reserveOut);

        _safeTransfer(tokenIn, pool, amountIn);

        IUniswapV2Pair pair = IUniswapV2Pair(pool);
        if (pair.token0() == tokenOut) {
            pair.swap(amountOut, 0, address(this), new bytes(0));
        } else {
            pair.swap(0, amountOut, address(this), new bytes(0));
        }
    }

    function _calcularRepagoV2(
        address pool,
        address tokenPrestamo,
        address tokenRepago,
        uint montoPrestamo
    ) internal view returns (uint) {
        (uint reserveRepago, uint reservePrestamo) =
            _reservasOrdenadas(pool, tokenRepago, tokenPrestamo);

        return _getAmountIn(montoPrestamo, reserveRepago, reservePrestamo);
    }

    function _calcularRepagoMismoTokenV2(
        address pool,
        address tokenPrestamo,
        uint montoPrestamo
    ) internal view returns (uint) {
        IUniswapV2Pair pair = IUniswapV2Pair(pool);
        require(pair.token0() == tokenPrestamo || pair.token1() == tokenPrestamo, "Token invalido");

        return (montoPrestamo * 1000) / 997 + 1;
    }

    function _swapV3ExactInput(
        address pool,
        address tokenIn,
        address tokenOut,
        uint amountIn
    ) internal returns (uint amountOut) {
        bool zeroForOne = _zeroForOneV3(pool, tokenIn, tokenOut);
        bytes memory callbackData = abi.encode(V3SwapData({pool: pool, tokenIn: tokenIn}));

        (int256 amount0, int256 amount1) = IUniswapV3Pool(pool).swap(
            address(this),
            zeroForOne,
            int256(amountIn),
            _sqrtPriceLimit(zeroForOne),
            callbackData
        );

        return _salidaV3(zeroForOne, amount0, amount1);
    }

    function _zeroForOneV3(
        address pool,
        address tokenIn,
        address tokenOut
    ) internal view returns (bool) {
        IUniswapV3Pool pair = IUniswapV3Pool(pool);

        if (pair.token0() == tokenIn && pair.token1() == tokenOut) {
            return true;
        }

        if (pair.token1() == tokenIn && pair.token0() == tokenOut) {
            return false;
        }

        revert("Par V3 invalido");
    }

    function _sqrtPriceLimit(bool zeroForOne) internal pure returns (uint160) {
        return zeroForOne
            ? uint160(4295128740)
            : uint160(1461446703485210103287273052203988822378723970341);
    }

    function _salidaV3(
        bool zeroForOne,
        int256 amount0,
        int256 amount1
    ) internal pure returns (uint) {
        int256 deltaOut = zeroForOne ? amount1 : amount0;
        require(deltaOut < 0, "Sin salida V3");

        return uint(-deltaOut);
    }

    function _reservasOrdenadas(
        address pool,
        address tokenIn,
        address tokenOut
    ) internal view returns (uint reserveIn, uint reserveOut) {
        IUniswapV2Pair pair = IUniswapV2Pair(pool);
        (uint reserve0, uint reserve1,) = pair.getReserves();

        if (pair.token0() == tokenIn && pair.token1() == tokenOut) {
            return (reserve0, reserve1);
        }

        if (pair.token1() == tokenIn && pair.token0() == tokenOut) {
            return (reserve1, reserve0);
        }

        revert("Par invalido");
    }

    function _contienePar(address pool, address tokenA, address tokenB) internal view returns (bool) {
        return _contieneParV2(pool, tokenA, tokenB);
    }

    function _contieneParV2(address pool, address tokenA, address tokenB) internal view returns (bool) {
        IUniswapV2Pair pair = IUniswapV2Pair(pool);
        return (pair.token0() == tokenA && pair.token1() == tokenB)
            || (pair.token0() == tokenB && pair.token1() == tokenA);
    }

    function _contieneParV3(address pool, address tokenA, address tokenB) internal view returns (bool) {
        IUniswapV3Pool pair = IUniswapV3Pool(pool);
        return (pair.token0() == tokenA && pair.token1() == tokenB)
            || (pair.token0() == tokenB && pair.token1() == tokenA);
    }

    function _getAmountOut(
        uint amountIn,
        uint reserveIn,
        uint reserveOut
    ) internal pure returns (uint) {
        require(amountIn > 0, "Entrada cero");
        require(reserveIn > 0 && reserveOut > 0, "Liquidez insuficiente");

        uint amountInWithFee = amountIn * 997;
        uint numerator = amountInWithFee * reserveOut;
        uint denominator = reserveIn * 1000 + amountInWithFee;

        return numerator / denominator;
    }

    function _getAmountIn(
        uint amountOut,
        uint reserveIn,
        uint reserveOut
    ) internal pure returns (uint) {
        require(amountOut > 0, "Salida cero");
        require(reserveIn > 0 && reserveOut > amountOut, "Liquidez insuficiente");

        uint numerator = reserveIn * amountOut * 1000;
        uint denominator = (reserveOut - amountOut) * 997;

        return numerator / denominator + 1;
    }

    function _safeTransfer(address token, address to, uint amount) internal {
        (bool ok, bytes memory data) =
            token.call(abi.encodeWithSelector(bytes4(keccak256("transfer(address,uint256)")), to, amount));
        require(ok && (data.length == 0 || abi.decode(data, (bool))), "Transfer fallido");
    }
}
