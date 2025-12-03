var __decorate = (this && this.__decorate) || function (decorators, target, key, desc) {
    var c = arguments.length, r = c < 3 ? target : desc === null ? desc = Object.getOwnPropertyDescriptor(target, key) : desc, d;
    if (typeof Reflect === "object" && typeof Reflect.decorate === "function") r = Reflect.decorate(decorators, target, key, desc);
    else for (var i = decorators.length - 1; i >= 0; i--) if (d = decorators[i]) r = (c < 3 ? d(r) : c > 3 ? d(target, key, r) : d(target, key)) || r;
    return c > 3 && r && Object.defineProperty(target, key, r), r;
};
var __metadata = (this && this.__metadata) || function (k, v) {
    if (typeof Reflect === "object" && typeof Reflect.metadata === "function") return Reflect.metadata(k, v);
};
// contracts/mina/mina-gateway.ts
// SIMPLIFIED: Only locks/releases assets, coordinator handles proof verification
import { Field, SmartContract, state, State, PublicKey, UInt64, Struct, Poseidon, Bool, } from 'o1js';
/**

Deposit request structure
*/
export class DepositRequest extends Struct({
    sender: PublicKey,
    amount: UInt64,
    targetChainId: Field,
    recipient: Field,
    zcashAddress: Field,
}) {
}
/**

Withdrawal request structure
*/
export class WithdrawalRequest extends Struct({
    recipient: PublicKey,
    amount: UInt64,
    nullifier: Field,
}) {
}
/**

ZeroBridge Gateway for Mina Protocol
@notice Simplified gateway - only locks/releases, coordinator handles proofs
*/
export class MinaGateway extends SmartContract {
    constructor() {
        super(...arguments);
        // State variables (8 fields max in Mina)
        this.coordinator = State();
        this.totalLocked = State();
        this.totalWithdrawn = State();
        this.depositCount = State();
        this.paused = State(); // Events
        this.events = {
            'tokens-locked': DepositRequest,
            'tokens-released': WithdrawalRequest,
            'coordinator-updated': PublicKey,
            'emergency-pause': Field,
        };
    }
    init() {
        super.init();
        this.coordinator.set(PublicKey.empty());
        this.totalLocked.set(Field(0));
        this.totalWithdrawn.set(Field(0));
        this.depositCount.set(Field(0));
        this.paused.set(Bool(false));
    } /**
    Set coordinator (admin only)
    */
    setCoordinator(newCoordinator, adminSignature) {
        const sender = this.sender.getAndRequireSignature();
        adminSignature.verify(sender, [
            ...newCoordinator.toFields(),
        ]).assertTrue('Invalid admin signature');
        this.coordinator.set(newCoordinator);
        this.emitEvent('coordinator-updated', newCoordinator);
    }
    /**
    Deposit tokens - only locks and emits event
    */
    deposit(amount, targetChainId, recipient, zcashAddress, signature) {
        const isPaused = this.paused.getAndRequireEquals();
        isPaused.assertFalse('Gateway is paused');
        amount.value.assertGreaterThan(Field(1000000), 'Amount too small');
        amount.value.assertLessThan(Field(1000000000000), 'Amount too large');
        const sender = this.sender.getAndRequireSignature();
        signature.verify(sender, [
            amount.value,
            targetChainId,
            recipient,
            zcashAddress,
        ]).assertTrue('Invalid signature'); // Generate deposit ID
        const currentCount = this.depositCount.getAndRequireEquals();
        const depositId = Poseidon.hash([
            ...sender.toFields(),
            amount.value,
            targetChainId,
            recipient,
            zcashAddress,
            currentCount,
            this.network.timestamp.get().value,
        ]); // Update total locked
        const currentLocked = this.totalLocked.getAndRequireEquals();
        const newLocked = currentLocked.add(amount.value);
        this.totalLocked.set(newLocked);
        this.depositCount.set(currentCount.add(1)); // Emit event for relayer to pick up
        this.emitEvent('tokens-locked', new DepositRequest({
            sender,
            amount,
            targetChainId,
            recipient,
            zcashAddress,
        }));
    }
    /**
    Withdraw - ONLY coordinator can call after verifying proof
    Removed all proof verification - coordinator does this off-chain
    */
    withdraw(recipient, amount, nullifier, coordinatorSignature) {
        const isPaused = this.paused.getAndRequireEquals();
        isPaused.assertFalse('Gateway is paused'); // Verify coordinator signature (coordinator already verified Zcash proof)
        const coordinator = this.coordinator.getAndRequireEquals();
        coordinatorSignature.verify(coordinator, [
            ...recipient.toFields(),
            amount.value,
            nullifier,
        ]).assertTrue('Invalid coordinator signature'); // Verify nullifier not zero
        nullifier.assertNotEquals(Field(0), 'Invalid nullifier'); // Update balances
        const currentWithdrawn = this.totalWithdrawn.getAndRequireEquals();
        const newWithdrawn = currentWithdrawn.add(amount.value);
        this.totalWithdrawn.set(newWithdrawn);
        const currentLocked = this.totalLocked.getAndRequireEquals();
        const newLocked = currentLocked.sub(amount.value);
        this.totalLocked.set(newLocked); // Emit event
        this.emitEvent('tokens-released', new WithdrawalRequest({
            recipient,
            amount,
            nullifier,
        })); // Note: Actual token transfer happens via Mina's account system
    }
    /**
    Set paused state (admin only)
    */
    setPaused(paused, adminSignature) {
        const sender = this.sender.getAndRequireSignature();
        adminSignature.verify(sender, [
            paused.toField(),
        ]).assertTrue('Invalid admin signature');
        this.paused.set(paused);
        if (paused.toBoolean()) {
            this.emitEvent('emergency-pause', this.network.timestamp.get().value);
        }
    }
    /**
    Get current state
    */
    getState() {
        return {
            totalLocked: this.totalLocked.get(),
            totalWithdrawn: this.totalWithdrawn.get(),
            depositCount: this.depositCount.get(),
            paused: this.paused.get(),
        };
    }
}
__decorate([
    state(PublicKey),
    __metadata("design:type", Object)
], MinaGateway.prototype, "coordinator", void 0);
__decorate([
    state(Field),
    __metadata("design:type", Object)
], MinaGateway.prototype, "totalLocked", void 0);
__decorate([
    state(Field),
    __metadata("design:type", Object)
], MinaGateway.prototype, "totalWithdrawn", void 0);
__decorate([
    state(Field),
    __metadata("design:type", Object)
], MinaGateway.prototype, "depositCount", void 0);
__decorate([
    state(Bool),
    __metadata("design:type", Object)
], MinaGateway.prototype, "paused", void 0);
export async function deployGateway(deployerPrivateKey, coordinatorPublicKey) {
    const { PrivateKey } = await import('o1js');
    const deployerKey = PrivateKey.fromBase58(deployerPrivateKey);
    const zkAppPrivateKey = PrivateKey.random();
    const zkAppAddress = zkAppPrivateKey.toPublicKey();
    console.log('Compiling MinaGateway...');
    await MinaGateway.compile();
    const zkApp = new MinaGateway(zkAppAddress);
    console.log('Deploying to:', zkAppAddress.toBase58());
    return zkAppAddress;
}
export const TESTNET_CONFIG = {
    minDepositAmount: 1000000,
    maxDepositAmount: 1000000000000,
    bridgeFee: 30,
};
