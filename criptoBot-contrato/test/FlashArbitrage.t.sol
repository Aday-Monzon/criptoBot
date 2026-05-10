// SPDX-License-Identifier: MIT
pragma solidity ^0.8.19;

import "forge-std/Test.sol";
import "../src/FlashArbitrage.sol";

contract MockERC20 {
    string public name;
    string public symbol;
    uint8 public decimals;
    mapping(address => uint) public balanceOf;

    constructor(string memory name_, string memory symbol_, uint8 decimals_) {
        name = name_;
        symbol = symbol_;
        decimals = decimals_;
    }

    function mint(address to, uint amount) external {
        balanceOf[to] += amount;
    }

    function transfer(address to, uint amount) external returns (bool) {
        require(balanceOf[msg.sender] >= amount, "balance");
        balanceOf[msg.sender] -= amount;
        balanceOf[to] += amount;
        return true;
    }
}

contract MockV2Pair {
    address public token0;
    address public token1;
    uint112 private reserve0;
    uint112 private reserve1;

    constructor(address token0_, address token1_) {
        token0 = token0_;
        token1 = token1_;
    }

    function setReserves(uint112 reserve0_, uint112 reserve1_) external {
        reserve0 = reserve0_;
        reserve1 = reserve1_;
    }

    function getReserves() external view returns (uint112, uint112, uint32) {
        return (reserve0, reserve1, 0);
    }

    function swap(uint amount0Out, uint amount1Out, address to, bytes calldata data) external {
        require(amount0Out == 0 || amount1Out == 0, "one side");

        if (amount0Out > 0) {
            MockERC20(token0).transfer(to, amount0Out);
        }

        if (amount1Out > 0) {
            MockERC20(token1).transfer(to, amount1Out);
        }

        if (data.length > 0) {
            FlashArbitrage(to).uniswapV2Call(address(this), amount0Out, amount1Out, data);
        }
    }
}

contract MockV3Pool {
    address public token0;
    address public token1;
    uint112 private reserve0;
    uint112 private reserve1;

    constructor(address token0_, address token1_) {
        token0 = token0_;
        token1 = token1_;
    }

    function setReserves(uint112 reserve0_, uint112 reserve1_) external {
        reserve0 = reserve0_;
        reserve1 = reserve1_;
    }

    function swap(
        address recipient,
        bool zeroForOne,
        int256 amountSpecified,
        uint160,
        bytes calldata data
    ) external returns (int256 amount0, int256 amount1) {
        require(amountSpecified > 0, "exact input only");
        uint amountIn = uint(amountSpecified);

        if (zeroForOne) {
            uint amountOut = getAmountOut(amountIn, reserve0, reserve1);
            FlashArbitrage(msg.sender).uniswapV3SwapCallback(
                int256(amountIn),
                -int256(amountOut),
                data
            );
            MockERC20(token1).transfer(recipient, amountOut);
            return (int256(amountIn), -int256(amountOut));
        } else {
            uint amountOut = getAmountOut(amountIn, reserve1, reserve0);
            FlashArbitrage(msg.sender).uniswapV3SwapCallback(
                -int256(amountOut),
                int256(amountIn),
                data
            );
            MockERC20(token0).transfer(recipient, amountOut);
            return (-int256(amountOut), int256(amountIn));
        }
    }

    function getAmountOut(
        uint amountIn,
        uint reserveIn,
        uint reserveOut
    ) internal pure returns (uint) {
        uint amountInWithFee = amountIn * 997;
        return (amountInWithFee * reserveOut) / (reserveIn * 1000 + amountInWithFee);
    }
}

contract FlashArbitrageTest is Test {
    MockERC20 private wpol;
    MockERC20 private usdt;
    MockV2Pair private poolPrestamo;
    MockV2Pair private poolVenta;
    MockV3Pool private poolCompraV3;
    MockV3Pool private poolVentaV3;
    FlashArbitrage private arbitrage;

    function setUp() public {
        wpol = new MockERC20("Wrapped POL", "WPOL", 18);
        usdt = new MockERC20("Tether USD", "USDT", 6);

        poolPrestamo = new MockV2Pair(address(wpol), address(usdt));
        poolVenta = new MockV2Pair(address(wpol), address(usdt));
        poolCompraV3 = new MockV3Pool(address(wpol), address(usdt));
        poolVentaV3 = new MockV3Pool(address(wpol), address(usdt));
        arbitrage = new FlashArbitrage();

        wpol.mint(address(poolPrestamo), 1_000_000 ether);
        usdt.mint(address(poolPrestamo), 200_000_000e6);
        poolPrestamo.setReserves(1_000_000 ether, 200_000_000e6);

        wpol.mint(address(poolVenta), 1_000_000 ether);
        usdt.mint(address(poolVenta), 250_000_000e6);
        poolVenta.setReserves(1_000_000 ether, 250_000_000e6);

        wpol.mint(address(poolCompraV3), 1_000_000 ether);
        usdt.mint(address(poolCompraV3), 200_000_000e6);
        poolCompraV3.setReserves(1_000_000 ether, 200_000_000e6);

        wpol.mint(address(poolVentaV3), 1_000_000 ether);
        usdt.mint(address(poolVentaV3), 250_000_000e6);
        poolVentaV3.setReserves(1_000_000 ether, 250_000_000e6);
    }

    function testEjecutaArbitrajeRentableYAcumulaUsdt() public {
        arbitrage.ejecutarArbitraje(
            address(poolPrestamo),
            address(poolVenta),
            address(wpol),
            address(usdt),
            100 ether,
            1e6
        );

        assertGt(usdt.balanceOf(address(arbitrage)), 1e6);
    }

    function testRevierteSiNoHayBeneficioMinimo() public {
        vm.expectRevert("Sin beneficio suficiente");

        arbitrage.ejecutarArbitraje(
            address(poolPrestamo),
            address(poolVenta),
            address(wpol),
            address(usdt),
            100 ether,
            1_000_000e6
        );
    }

    function testSoloOwnerPuedeEjecutar() public {
        vm.prank(address(0xBEEF));
        vm.expectRevert("No autorizado");

        arbitrage.ejecutarArbitraje(
            address(poolPrestamo),
            address(poolVenta),
            address(wpol),
            address(usdt),
            100 ether,
            1e6
        );
    }

    function testOwnerPuedeRetirarGanancias() public {
        arbitrage.ejecutarArbitraje(
            address(poolPrestamo),
            address(poolVenta),
            address(wpol),
            address(usdt),
            100 ether,
            1e6
        );

        uint saldoAntes = usdt.balanceOf(address(this));
        arbitrage.retirar(address(usdt));

        assertGt(usdt.balanceOf(address(this)), saldoAntes);
        assertEq(usdt.balanceOf(address(arbitrage)), 0);
    }

    function testEjecutaArbitrajeV3ConPrestamoV2YAcumulaUsdt() public {
        arbitrage.ejecutarArbitrajeV3(
            address(poolPrestamo),
            address(poolCompraV3),
            address(poolVentaV3),
            address(usdt),
            address(wpol),
            10_000e6,
            1e6
        );

        assertGt(usdt.balanceOf(address(arbitrage)), 1e6);
    }

    function testRevierteV3SiNoHayBeneficioMinimo() public {
        vm.expectRevert("Sin beneficio suficiente");

        arbitrage.ejecutarArbitrajeV3(
            address(poolPrestamo),
            address(poolCompraV3),
            address(poolVentaV3),
            address(usdt),
            address(wpol),
            10_000e6,
            1_000_000e6
        );
    }
}
