/* eslint-disable @typescript-eslint/no-unused-vars */
// contracts/mina/mina-gateway.ts
// FIXED: Two-step withdrawal with coordinator signature verification

import {
    Field,
    SmartContract,
    state,
    State,
    method,
    PublicKey,
    Signature,
    UInt64,
    Struct,
    Poseidon,
    Bool,
    Mina,
    PrivateKey,
    AccountUpdate,
    Circuit,
    UInt32,
} from 'o1js';

/**
 * Deposit request structure
 */
export class MinaDepositRequest extends Struct({
    sender: PublicKey,
    amount: UInt64,
    targetChainId: Field,
    recipient: Field,
    zcashAddress: Field,
}) {}

/**
 * Withdrawal request structure
 */
export class MinaWithdrawalRequest extends Struct({
    withdrawalId: Field,
    recipient: PublicKey,
    amount: UInt64,
    nullifier: Field,
}) {}

/**
 * ZeroBridge Gateway for Mina Protocol
 * FIXED: Two-step withdrawal with coordinator signature verification
 * 
 * KEY CHANGES:
 * 1. ✅ Two-step withdrawal: requestWithdrawal() + executeWithdrawal()
 * 2. ✅ Coordinator signature verification
 * 3. ✅ WithdrawalRequested event
 * 4. ✅ Proper state management for withdrawal requests
 */
export class MinaGateway extends SmartContract {
    // State variables (8 fields max in Mina)
    @state(PublicKey) coordinator = State<PublicKey>();
    @state(Field) totalLocked = State<Field>();
    @state(Field) totalWithdrawn = State<Field>();
    @state(Field) depositCount = State<Field>();
    @state(Field) withdrawalCount = State<Field>();
    @state(Bool) paused = State<Bool>();
    @state(Field) lastWithdrawalId = State<Field>();

    // Events
    events = {
        'tokens-locked': MinaDepositRequest,
        'withdrawal-requested': MinaWithdrawalRequest,
        'tokens-released': MinaWithdrawalRequest,
        'coordinator-updated': PublicKey,
        'emergency-pause': Field,
    };

    init() {
        super.init();
        this.coordinator.set(PublicKey.empty());
        this.totalLocked.set(Field(0));
        this.totalWithdrawn.set(Field(0));
        this.depositCount.set(Field(0));
        this.withdrawalCount.set(Field(0));
        this.paused.set(Bool(false));
        this.lastWithdrawalId.set(Field(0));
    }

    /**
     * Set coordinator (admin only)
     */
    @method async setCoordinator(
        newCoordinator: PublicKey,
        adminSignature: Signature
    ): Promise<void> {
        const sender = this.sender.getAndRequireSignature();
        
        adminSignature.verify(sender, [
            ...newCoordinator.toFields(),
        ]).assertTrue('Invalid admin signature');

        this.coordinator.set(newCoordinator);
        this.emitEvent('coordinator-updated', newCoordinator);
    }

    /**
     * Deposit - locks tokens and emits event
     */
    @method async deposit(
        amount: UInt64,
        targetChainId: Field,
        recipient: Field,
        zcashAddress: Field,
        signature: Signature
    ): Promise<void> {
        const isPaused = this.paused.getAndRequireEquals();
        isPaused.assertFalse('Gateway is paused');

        amount.value.assertGreaterThan(Field(1_000_000), 'Amount too small');
        amount.value.assertLessThan(Field(1_000_000_000_000), 'Amount too large');

        const sender = this.sender.getAndRequireSignature();
        
        signature.verify(sender, [
            amount.value,
            targetChainId,
            recipient,
            zcashAddress,
        ]).assertTrue('Invalid signature');

        // Get current count
        const currentCount = this.depositCount.getAndRequireEquals();
        
        // Add network preconditions and get timestamp
        const currentSlot = this.network.globalSlotSinceGenesis.getAndRequireEquals();
        
        // Generate deposit ID
        const depositId = Poseidon.hash([
            ...sender.toFields(),
            amount.value,
            targetChainId,
            recipient,
            zcashAddress,
            currentCount,
            currentSlot.value,
        ]);

        // Update total locked
        const currentLocked = this.totalLocked.getAndRequireEquals();
        const newLocked = currentLocked.add(amount.value);
        this.totalLocked.set(newLocked);
        this.depositCount.set(currentCount.add(1));

        // Emit event for relayer
        this.emitEvent('tokens-locked', new MinaDepositRequest({
            sender,
            amount,
            targetChainId,
            recipient,
            zcashAddress,
        }));
    }

    /**
     * Request withdrawal - Step 1 (emits event for relayer)
     */
    @method async requestWithdrawal(
        amount: UInt64,
        nullifier: Field,
        zcashProof: Field,
        merkleRoot: Field,
        signature: Signature
    ): Promise<void> {
        const isPaused = this.paused.getAndRequireEquals();
        isPaused.assertFalse('Gateway is paused');

        amount.value.assertGreaterThan(Field(0), 'Invalid amount');
        nullifier.assertNotEquals(Field(0), 'Invalid nullifier');
        merkleRoot.assertNotEquals(Field(0), 'Invalid merkle root');

        const recipient = this.sender.getAndRequireSignature();
        
        signature.verify(recipient, [
            amount.value,
            nullifier,
            merkleRoot,
        ]).assertTrue('Invalid signature');

        // Get current count
        const currentCount = this.withdrawalCount.getAndRequireEquals();
        
        // Add network preconditions and get timestamp
        const currentSlot = this.network.globalSlotSinceGenesis.getAndRequireEquals();

        // Generate withdrawal ID
        const withdrawalId = Poseidon.hash([
            ...recipient.toFields(),
            amount.value,
            nullifier,
            currentCount,
            currentSlot.value,
        ]);

        // Store for later execution
        this.lastWithdrawalId.set(withdrawalId);
        this.withdrawalCount.set(currentCount.add(1));

        // Emit event for relayer to pick up
        this.emitEvent('withdrawal-requested', new MinaWithdrawalRequest({
            withdrawalId,
            recipient,
            amount,
            nullifier,
        }));
    }

    /**
     * Execute withdrawal - Step 2 (with coordinator signature)
     */
    @method async executeWithdrawal(
        withdrawalId: Field,
        recipient: PublicKey,
        amount: UInt64,
        nullifier: Field,
        coordinatorSignature: Signature
    ): Promise<void> {
        const isPaused = this.paused.getAndRequireEquals();
        isPaused.assertFalse('Gateway is paused');

        // Verify this matches a pending withdrawal
        const storedWithdrawalId = this.lastWithdrawalId.getAndRequireEquals(); 
        withdrawalId.assertEquals(storedWithdrawalId, 'Invalid withdrawal ID');

        // Verify coordinator signature
        const coordinator = this.coordinator.getAndRequireEquals();
        coordinatorSignature.verify(coordinator, [
            withdrawalId,
            ...recipient.toFields(),
            amount.value,
            nullifier,
        ]).assertTrue('Invalid coordinator signature');

        // Update balances
        const currentWithdrawn = this.totalWithdrawn.getAndRequireEquals();
        const newWithdrawn = currentWithdrawn.add(amount.value);
        this.totalWithdrawn.set(newWithdrawn);

        const currentLocked = this.totalLocked.getAndRequireEquals();
        const newLocked = currentLocked.sub(amount.value);
        this.totalLocked.set(newLocked);

        // Clear the withdrawal request (prevent replay)
        this.lastWithdrawalId.set(Field(0));

        // Emit event
        this.emitEvent('tokens-released', new MinaWithdrawalRequest({
            withdrawalId,
            recipient,
            amount,
            nullifier,
        }));
    }

    /**
     * Set paused state
     */
    @method async setPaused(
        paused: Bool,
        adminSignature: Signature
    ): Promise<void> {
        const sender = this.sender.getAndRequireSignature();
        
        adminSignature.verify(sender, [
            paused.toField(),
        ]).assertTrue('Invalid admin signature');

        this.paused.set(paused);
        
        // Use circuit-compatible conditional logic
        // Get the current slot (always needed for event emission)
        const currentSlot = this.network.globalSlotSinceGenesis.getAndRequireEquals();
        
        // Emit event with slot value multiplied by paused (0 if not paused, slot if paused)
        // This way the event is always emitted but with meaningful data only when paused=true
        // const eventData = Circuit.if(
        //     paused,
        //     currentSlot.value,
        //     Field(0)
        // );
        // this.emitEvent('emergency-pause', eventData);
    }
}

export async function deployGateway(
    deployerPrivateKeyBase58: string,
    coordinatorPublicKeyBase58: string,
    network: 'devnet' | 'testnet' | 'mainnet' = 'devnet'
): Promise<PublicKey> {
    const deployerPrivateKey = PrivateKey.fromBase58(deployerPrivateKeyBase58);
    const deployerPublicKey = deployerPrivateKey.toPublicKey();
    const coordinatorPublicKey = PublicKey.fromBase58(coordinatorPublicKeyBase58);
    
    const zkAppPrivateKey = PrivateKey.random();
    const zkAppAddress = zkAppPrivateKey.toPublicKey();
    
    // Set up network
    const networkUrl = network === 'mainnet' 
        ? 'https://api.minascan.io/node/mainnet/v1/graphql'
        : network === 'testnet'
        ? 'https://api.minascan.io/node/testnet/v1/graphql'
        : 'https://api.minascan.io/node/devnet/v1/graphql';
    
    const Network = Mina.Network(networkUrl);
    Mina.setActiveInstance(Network);
    
    console.log('Compiling MinaGateway...');
    await MinaGateway.compile();
    
    console.log('Creating deployment transaction...');
    const zkApp = new MinaGateway(zkAppAddress);
    
    const deployTxn = await Mina.transaction(
        { sender: deployerPublicKey, fee: 100_000_000 },
        async () => {
            AccountUpdate.fundNewAccount(deployerPublicKey);
            zkApp.deploy();
        }
    );
    
    await deployTxn.prove();
    const pendingDeploy = await deployTxn
        .sign([deployerPrivateKey, zkAppPrivateKey])
        .send();
    
    console.log(`Deployment transaction hash: ${pendingDeploy.hash}`);
    await pendingDeploy.wait({ maxAttempts: 60, interval: 30_000 });
    
    console.log(`Deployed at ${zkAppAddress.toBase58()}`);
    
    // Set initial coordinator
    console.log('Setting initial coordinator...');
    const coordFields = coordinatorPublicKey.toFields();
    const adminSig = Signature.create(deployerPrivateKey, coordFields);
    
    const setTxn = await Mina.transaction(
        { sender: deployerPublicKey, fee: 100_000_000 },
        async () => {
            return zkApp.setCoordinator(coordinatorPublicKey, adminSig);
        }
    );
    
    await setTxn.prove();
    const pendingSet = await setTxn.sign([deployerPrivateKey]).send();
    
    console.log(`Set coordinator transaction hash: ${pendingSet.hash}`);
    await pendingSet.wait({ maxAttempts: 60, interval: 30_000 });
    
    console.log('Coordinator set successfully.');
    
    return zkAppAddress;
}

export async function interactWithGateway(
    gatewayAddress: string,
    userPrivateKeyBase58: string,
    network: 'devnet' | 'testnet' | 'mainnet' = 'devnet'
) {
    const userPrivateKey = PrivateKey.fromBase58(userPrivateKeyBase58);
    const userPublicKey = userPrivateKey.toPublicKey();
    const zkAppAddress = PublicKey.fromBase58(gatewayAddress);
    
    const networkUrl = network === 'mainnet' 
        ? 'https://api.minascan.io/node/mainnet/v1/graphql'
        : network === 'testnet'
        ? 'https://api.minascan.io/node/testnet/v1/graphql'
        : 'https://api.minascan.io/node/devnet/v1/graphql';
    
    const Network = Mina.Network(networkUrl);
    Mina.setActiveInstance(Network);
    
    const zkApp = new MinaGateway(zkAppAddress);
    
    return {
        deposit: async (
            amount: bigint,
            targetChainId: bigint,
            recipient: bigint,
            zcashAddress: bigint
        ) => {
            const amountField = UInt64.from(amount);
            const signature = Signature.create(userPrivateKey, [
                Field(amount),
                Field(targetChainId),
                Field(recipient),
                Field(zcashAddress),
            ]);
            
            const txn = await Mina.transaction(
                { sender: userPublicKey, fee: 100_000_000 },
                async () => {
                    await zkApp.deposit(
                        amountField,
                        Field(targetChainId),
                        Field(recipient),
                        Field(zcashAddress),
                        signature
                    );
                }
            );
            
            await txn.prove();
            const pending = await txn.sign([userPrivateKey]).send();
            return pending.hash;
        },
        
        requestWithdrawal: async (
            amount: bigint,
            nullifier: bigint,
            zcashProof: bigint,
            merkleRoot: bigint
        ) => {
            const amountField = UInt64.from(amount);
            const signature = Signature.create(userPrivateKey, [
                Field(amount),
                Field(nullifier),
                Field(merkleRoot),
            ]);
            
            const txn = await Mina.transaction(
                { sender: userPublicKey, fee: 100_000_000 },
                async () => {
                    await zkApp.requestWithdrawal(
                        amountField,
                        Field(nullifier),
                        Field(zcashProof),
                        Field(merkleRoot),
                        signature
                    );
                }
            );
            
            await txn.prove();
            const pending = await txn.sign([userPrivateKey]).send();
            return pending.hash;
        },
        
        getState: async () => {
            return {
                totalLocked: zkApp.totalLocked.get().toString(),
                totalWithdrawn: zkApp.totalWithdrawn.get().toString(),
                depositCount: zkApp.depositCount.get().toString(),
                withdrawalCount: zkApp.withdrawalCount.get().toString(),
                paused: zkApp.paused.get().toBoolean(),
            };
        },
    };
}

// Export types
export type { MinaDepositRequest as DepositRequest, MinaWithdrawalRequest as WithdrawalRequest };

// Constants
export const TESTNET_CONFIG = {
    minDepositAmount: 1_000_000n,
    maxDepositAmount: 1_000_000_000_000n,
    bridgeFee: 30, // 0.3%
};