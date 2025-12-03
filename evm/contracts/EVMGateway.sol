// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import "@openzeppelin/contracts/token/ERC20/utils/SafeERC20.sol";
import "@openzeppelin/contracts/utils/ReentrancyGuard.sol";
import "@openzeppelin/contracts/utils/Pausable.sol";
import "@openzeppelin/contracts/access/AccessControl.sol";
import "@openzeppelin/contracts/utils/cryptography/ECDSA.sol";
import "@openzeppelin/contracts/utils/cryptography/MessageHashUtils.sol";

/**
 * @title EVMGateway
 * @notice ZeroBridge gateway for EVM chains (Ethereum, Base, Polygon)
 * @dev FIXED: Two-step withdrawal with coordinator signature verification
 * 
 * KEY CHANGES FROM ORIGINAL:
 * 1. ✅ Two-step withdrawal: requestWithdrawal() + executeWithdrawal()
 * 2. ✅ Coordinator signature verification (not caller check)
 * 3. ✅ WithdrawalRequested event for relayer notification
 * 4. ✅ Removed on-chain proof verification (coordinator does this)
 * 5. ✅ Liquidity provider functions implemented
 */
contract EVMGateway is ReentrancyGuard, Pausable, AccessControl {
    using SafeERC20 for IERC20;
    using ECDSA for bytes32;
    using MessageHashUtils for bytes32;
    
    // ============ Constants ============
    
    bytes32 public constant PAUSER_ROLE = keccak256("PAUSER_ROLE");
    bytes32 public constant LIQUIDITY_PROVIDER_ROLE = keccak256("LIQUIDITY_PROVIDER_ROLE");
    
    address public constant NATIVE_ASSET = address(0);
    uint256 public constant MIN_DEPOSIT = 0.001 ether;
    uint256 public constant MAX_DEPOSIT = 1000000 ether;
    
    // ============ State Variables ============
    
    address public coordinator;
    uint256 public depositNonce;
    uint256 public withdrawalNonce;
    uint256 public bridgeFee = 30; // 0.3% in basis points
    
    // Token => locked balance
    mapping(address => uint256) public lockedBalances;
    
    // Nullifier => used (prevents double-spend)
    mapping(bytes32 => bool) public usedNullifiers;
    
    // Deposit ID => Deposit info
    mapping(bytes32 => DepositInfo) public deposits;
    
    // Withdrawal ID => Withdrawal request info
    mapping(bytes32 => WithdrawalRequestInfo) public withdrawalRequests;
    
    // Statistics
    uint256 public totalDeposits;
    uint256 public totalWithdrawals;
    uint256 public totalVolume;
    
    // ============ Structs ============
    
    struct DepositInfo {
        address sender;
        address token;
        uint256 amount;
        uint64 targetChainId;
        bytes32 recipient;
        bytes32 zcashAddress;
        uint256 timestamp;
        bool processed;
    }
    
    struct WithdrawalRequestInfo {
        address recipient;
        address token;
        uint256 amount;
        bytes32 nullifier;
        uint256 timestamp;
        bool executed;
    }
    
    // ============ Events ============
    
    event TokensLocked(
        bytes32 indexed depositId,
        address indexed sender,
        address indexed token,
        uint256 amount,
        uint64 targetChainId,
        bytes32 recipient,
        bytes32 zcashAddress,
        uint256 timestamp
    );
    
    event WithdrawalRequested(
        bytes32 indexed withdrawalId,
        address indexed recipient,
        address indexed token,
        uint256 amount,
        bytes32 nullifier,
        bytes zcashProof,
        bytes32 merkleRoot,
        uint256 timestamp
    );
    
    event TokensReleased(
        bytes32 indexed withdrawalId,
        address indexed recipient,
        address indexed token,
        uint256 amount,
        bytes32 nullifier,
        uint256 timestamp
    );
    
    event LiquidityAdded(
        address indexed provider,
        address indexed token,
        uint256 amount,
        uint256 timestamp
    );
    
    event LiquidityRemoved(
        address indexed provider,
        address indexed token,
        uint256 amount,
        uint256 timestamp
    );
    
    event CoordinatorUpdated(
        address indexed oldCoordinator,
        address indexed newCoordinator,
        uint256 timestamp
    );
    
    event BridgeFeeUpdated(
        uint256 oldFee,
        uint256 newFee
    );
    
    event EmergencyPause(
        address indexed triggeredBy,
        string reason,
        uint256 timestamp
    );
    
    // ============ Modifiers ============
    
    modifier validAmount(uint256 amount) {
        require(
            amount >= MIN_DEPOSIT && amount <= MAX_DEPOSIT,
            "Amount out of range"
        );
        _;
    }
    
    // ============ Constructor ============
    
    constructor(address _coordinator) {
        require(_coordinator != address(0), "Invalid coordinator");
        
        coordinator = _coordinator;
        
        _grantRole(DEFAULT_ADMIN_ROLE, msg.sender);
        _grantRole(PAUSER_ROLE, msg.sender);
    }
    
    // ============ DEPOSIT FUNCTION ============
    
    /**
     * @notice Deposit tokens for bridging to another chain
     * @dev Locks tokens and emits event - coordinator creates Zcash note
     * @param token Token address (address(0) for native asset)
     * @param amount Amount to bridge
     * @param targetChainId Destination chain ID
     * @param recipient Recipient address on destination chain
     * @param zcashAddress Shielded address on Zcash
     * @return depositId Unique identifier for this deposit
     */
    function deposit(
        address token,
        uint256 amount,
        uint64 targetChainId,
        bytes32 recipient,
        bytes32 zcashAddress
    )
        external
        payable
        nonReentrant
        whenNotPaused
        validAmount(amount)
        returns (bytes32 depositId)
    {
        require(targetChainId != block.chainid, "Cannot bridge to same chain");
        require(recipient != bytes32(0), "Invalid recipient");
        require(zcashAddress != bytes32(0), "Invalid Zcash address");
        
        // Calculate fee
        uint256 fee = (amount * bridgeFee) / 10000;
        uint256 netAmount = amount - fee;
        
        // Handle native asset vs ERC20
        if (token == NATIVE_ASSET) {
            require(msg.value == amount, "Incorrect ETH amount");
        } else {
            require(msg.value == 0, "ETH not accepted for ERC20");
            IERC20(token).safeTransferFrom(
                msg.sender,
                address(this),
                amount
            );
        }
        
        // Generate unique deposit ID
        depositId = keccak256(abi.encodePacked(
            msg.sender,
            token,
            amount,
            targetChainId,
            recipient,
            zcashAddress,
            depositNonce++,
            block.number,
            block.timestamp,
            blockhash(block.number - 1)
        ));
        
        // Store deposit info
        deposits[depositId] = DepositInfo({
            sender: msg.sender,
            token: token,
            amount: netAmount,
            targetChainId: targetChainId,
            recipient: recipient,
            zcashAddress: zcashAddress,
            timestamp: block.timestamp,
            processed: false
        });
        
        // Update balances
        lockedBalances[token] += netAmount;
        totalDeposits += netAmount;
        totalVolume += netAmount;
        
        emit TokensLocked(
            depositId,
            msg.sender,
            token,
            netAmount,
            targetChainId,
            recipient,
            zcashAddress,
            block.timestamp
        );
        
        return depositId;
    }
    
    // ============ WITHDRAWAL REQUEST (Step 1) ============
    
    /**
     * @notice Request withdrawal using Zcash proof
     * @dev Emits event for relayer - does NOT release tokens yet
     * @param token Token address to withdraw
     * @param amount Amount to withdraw
     * @param nullifier Zcash nullifier (prevents double-spend)
     * @param zcashProof Halo2 proof from Zcash
     * @param merkleRoot Zcash merkle root at time of proof
     * @return withdrawalId Unique identifier for this withdrawal
     */
    function requestWithdrawal(
        address token,
        uint256 amount,
        bytes32 nullifier,
        bytes calldata zcashProof,
        bytes32 merkleRoot
    )
        external
        nonReentrant
        whenNotPaused
        returns (bytes32 withdrawalId)
    {
        require(amount > 0, "Invalid amount");
        require(nullifier != bytes32(0), "Invalid nullifier");
        require(!usedNullifiers[nullifier], "Nullifier already used");
        require(merkleRoot != bytes32(0), "Invalid merkle root");
        
        // Generate withdrawal ID
        withdrawalId = keccak256(abi.encodePacked(
            msg.sender,
            token,
            amount,
            nullifier,
            withdrawalNonce++,
            block.timestamp
        ));
        
        // Store withdrawal request
        withdrawalRequests[withdrawalId] = WithdrawalRequestInfo({
            recipient: msg.sender,
            token: token,
            amount: amount,
            nullifier: nullifier,
            timestamp: block.timestamp,
            executed: false
        });
        
        // Emit event for relayer to pick up
        // Relayer will notify coordinator who verifies the proof
        emit WithdrawalRequested(
            withdrawalId,
            msg.sender,
            token,
            amount,
            nullifier,
            zcashProof,
            merkleRoot,
            block.timestamp
        );
        
        return withdrawalId;
    }
    
    // ============ WITHDRAWAL EXECUTION (Step 2) ============
    
    /**
     * @notice Execute withdrawal with coordinator authorization
     * @dev Called by relayer with coordinator's signature
     * @param withdrawalId Withdrawal identifier from requestWithdrawal
     * @param coordinatorSignature Coordinator's signature authorizing withdrawal
     * @return success Whether withdrawal succeeded
     */
    function executeWithdrawal(
        bytes32 withdrawalId,
        bytes calldata coordinatorSignature
    )
        external
        nonReentrant
        whenNotPaused
        returns (bool success)
    {
        WithdrawalRequestInfo storage request = withdrawalRequests[withdrawalId];
        
        require(request.timestamp > 0, "Withdrawal not found");
        require(!request.executed, "Already executed");
        require(!usedNullifiers[request.nullifier], "Nullifier already used");
        require(
            lockedBalances[request.token] >= request.amount,
            "Insufficient locked balance"
        );
        
        // Verify coordinator signature
        bytes32 messageHash = keccak256(abi.encodePacked(
            withdrawalId,
            request.recipient,
            request.token,
            request.amount,
            request.nullifier,
            block.chainid // Prevent replay attacks across chains
        ));
        
        bytes32 ethSignedMessageHash = messageHash.toEthSignedMessageHash();
        address signer = ethSignedMessageHash.recover(coordinatorSignature);
        
        require(signer == coordinator, "Invalid coordinator signature");
        
        // Mark as executed
        request.executed = true;
        usedNullifiers[request.nullifier] = true;
        
        // Update balances
        lockedBalances[request.token] -= request.amount;
        totalWithdrawals += request.amount;
        
        // Transfer tokens to recipient
        if (request.token == NATIVE_ASSET) {
            (bool sent, ) = payable(request.recipient).call{value: request.amount}("");
            require(sent, "ETH transfer failed");
        } else {
            IERC20(request.token).safeTransfer(request.recipient, request.amount);
        }
        
        emit TokensReleased(
            withdrawalId,
            request.recipient,
            request.token,
            request.amount,
            request.nullifier,
            block.timestamp
        );
        
        return true;
    }
    
    // ============ LIQUIDITY MANAGEMENT ============
    
    /**
     * @notice Add liquidity to the gateway
     * @param token Token to add (address(0) for native asset)
     * @param amount Amount to add
     */
    function addLiquidity(address token, uint256 amount)
        external
        payable
        nonReentrant
        whenNotPaused
        onlyRole(LIQUIDITY_PROVIDER_ROLE)
    {
        require(amount > 0, "Invalid amount");
        
        if (token == NATIVE_ASSET) {
            require(msg.value == amount, "Incorrect ETH amount");
        } else {
            require(msg.value == 0, "ETH not accepted for ERC20");
            IERC20(token).safeTransferFrom(msg.sender, address(this), amount);
        }
        
        emit LiquidityAdded(msg.sender, token, amount, block.timestamp);
    }
    
    /**
     * @notice Remove liquidity from the gateway
     * @param token Token to remove
     * @param amount Amount to remove
     */
    function removeLiquidity(address token, uint256 amount)
        external
        nonReentrant
        whenNotPaused
        onlyRole(LIQUIDITY_PROVIDER_ROLE)
    {
        require(amount > 0, "Invalid amount");
        
        uint256 available = getAvailableLiquidity(token);
        require(available >= amount, "Insufficient available liquidity");
        
        if (token == NATIVE_ASSET) {
            (bool sent, ) = payable(msg.sender).call{value: amount}("");
            require(sent, "ETH transfer failed");
        } else {
            IERC20(token).safeTransfer(msg.sender, amount);
        }
        
        emit LiquidityRemoved(msg.sender, token, amount, block.timestamp);
    }
    
    /**
     * @notice Add liquidity provider role
     * @param provider Address to grant role
     */
    function addLiquidityProvider(address provider)
        external
        onlyRole(DEFAULT_ADMIN_ROLE)
    {
        require(provider != address(0), "Invalid provider");
        _grantRole(LIQUIDITY_PROVIDER_ROLE, provider);
    }
    
    /**
     * @notice Remove liquidity provider role
     * @param provider Address to revoke role
     */
    function removeLiquidityProvider(address provider)
        external
        onlyRole(DEFAULT_ADMIN_ROLE)
    {
        _revokeRole(LIQUIDITY_PROVIDER_ROLE, provider);
    }
    
    // ============ VIEW FUNCTIONS ============
    
    /**
     * @notice Get locked balance for a token
     */
    function getLockedBalance(address token)
        external
        view
        returns (uint256)
    {
        return lockedBalances[token];
    }
    
    /**
     * @notice Get available liquidity (total balance - locked)
     */
    function getAvailableLiquidity(address token)
        public
        view
        returns (uint256)
    {
        uint256 totalBalance;
        if (token == NATIVE_ASSET) {
            totalBalance = address(this).balance;
        } else {
            totalBalance = IERC20(token).balanceOf(address(this));
        }
        
        uint256 locked = lockedBalances[token];
        return totalBalance > locked ? totalBalance - locked : 0;
    }
    
    /**
     * @notice Check if nullifier has been used
     */
    function isNullifierUsed(bytes32 nullifier)
        external
        view
        returns (bool)
    {
        return usedNullifiers[nullifier];
    }
    
    /**
     * @notice Get deposit details
     */
    function getDeposit(bytes32 depositId)
        external
        view
        returns (
            bool exists,
            address sender,
            address token,
            uint256 amount,
            uint64 targetChainId,
            bool processed
        )
    {
        DepositInfo memory info = deposits[depositId];
        return (
            info.timestamp > 0,
            info.sender,
            info.token,
            info.amount,
            info.targetChainId,
            info.processed
        );
    }
    
    /**
     * @notice Get withdrawal request details
     */
    function getWithdrawalRequest(bytes32 withdrawalId)
        external
        view
        returns (
            bool exists,
            address recipient,
            address token,
            uint256 amount,
            bytes32 nullifier,
            bool executed
        )
    {
        WithdrawalRequestInfo memory info = withdrawalRequests[withdrawalId];
        return (
            info.timestamp > 0,
            info.recipient,
            info.token,
            info.amount,
            info.nullifier,
            info.executed
        );
    }
    
    /**
     * @notice Get bridge statistics
     */
    function getStats()
        external
        view
        returns (
            uint256 totalDepositAmount,
            uint256 totalWithdrawalAmount,
            uint256 volume,
            uint256 activeDeposits
        )
    {
        return (
            totalDeposits,
            totalWithdrawals,
            totalVolume,
            totalDeposits - totalWithdrawals
        );
    }
    
    // ============ ADMIN FUNCTIONS ============
    
    /**
     * @notice Update coordinator address
     */
    function setCoordinator(address newCoordinator)
        external
        onlyRole(DEFAULT_ADMIN_ROLE)
    {
        require(newCoordinator != address(0), "Invalid coordinator");
        
        address oldCoordinator = coordinator;
        coordinator = newCoordinator;
        
        emit CoordinatorUpdated(oldCoordinator, newCoordinator, block.timestamp);
    }
    
    /**
     * @notice Set bridge fee
     */
    function setBridgeFee(uint256 newFee)
        external
        onlyRole(DEFAULT_ADMIN_ROLE)
    {
        require(newFee <= 100, "Fee too high"); // Max 1%
        
        uint256 oldFee = bridgeFee;
        bridgeFee = newFee;
        
        emit BridgeFeeUpdated(oldFee, newFee);
    }
    
    /**
     * @notice Pause/unpause contract
     */
    function setPaused(bool paused)
        external
        onlyRole(PAUSER_ROLE)
    {
        if (paused) {
            _pause();
            emit EmergencyPause(msg.sender, "Manual pause", block.timestamp);
        } else {
            _unpause();
        }
    }
    
    /**
     * @notice Emergency withdraw (only when paused)
     */
    function emergencyWithdraw(
        address token,
        address to,
        uint256 amount
    )
        external
        onlyRole(DEFAULT_ADMIN_ROLE)
        whenPaused
    {
        require(to != address(0), "Invalid recipient");
        require(amount > 0, "Invalid amount");
        
        if (token == NATIVE_ASSET) {
            (bool sent, ) = payable(to).call{value: amount}("");
            require(sent, "ETH transfer failed");
        } else {
            IERC20(token).safeTransfer(to, amount);
        }
        
        emit EmergencyPause(
            msg.sender,
            "Emergency withdrawal executed",
            block.timestamp
        );
    }
    
    // ============ RECEIVE ETH ============
    
    receive() external payable {
        // Accept ETH for liquidity and withdrawals
    }
}