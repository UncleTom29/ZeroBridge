import { Field, SmartContract, State, PublicKey, Signature, UInt64, Bool } from 'o1js';
declare const MinaDepositRequest_base: (new (value: {
    sender: PublicKey;
    amount: UInt64;
    targetChainId: import("o1js/dist/node/lib/provable/field").Field;
    recipient: import("o1js/dist/node/lib/provable/field").Field;
    zcashAddress: import("o1js/dist/node/lib/provable/field").Field;
}) => {
    sender: PublicKey;
    amount: UInt64;
    targetChainId: import("o1js/dist/node/lib/provable/field").Field;
    recipient: import("o1js/dist/node/lib/provable/field").Field;
    zcashAddress: import("o1js/dist/node/lib/provable/field").Field;
}) & {
    _isStruct: true;
} & Omit<import("o1js/dist/node/lib/provable/types/provable-intf").Provable<{
    sender: PublicKey;
    amount: UInt64;
    targetChainId: import("o1js/dist/node/lib/provable/field").Field;
    recipient: import("o1js/dist/node/lib/provable/field").Field;
    zcashAddress: import("o1js/dist/node/lib/provable/field").Field;
}, {
    sender: {
        x: bigint;
        isOdd: boolean;
    };
    amount: bigint;
    targetChainId: bigint;
    recipient: bigint;
    zcashAddress: bigint;
}>, "fromFields"> & {
    fromFields: (fields: import("o1js/dist/node/lib/provable/field").Field[]) => {
        sender: PublicKey;
        amount: UInt64;
        targetChainId: import("o1js/dist/node/lib/provable/field").Field;
        recipient: import("o1js/dist/node/lib/provable/field").Field;
        zcashAddress: import("o1js/dist/node/lib/provable/field").Field;
    };
} & {
    fromValue: (value: {
        sender: PublicKey | {
            x: Field | bigint;
            isOdd: Bool | boolean;
        };
        amount: number | bigint | UInt64;
        targetChainId: string | number | bigint | import("o1js/dist/node/lib/provable/field").Field;
        recipient: string | number | bigint | import("o1js/dist/node/lib/provable/field").Field;
        zcashAddress: string | number | bigint | import("o1js/dist/node/lib/provable/field").Field;
    }) => {
        sender: PublicKey;
        amount: UInt64;
        targetChainId: import("o1js/dist/node/lib/provable/field").Field;
        recipient: import("o1js/dist/node/lib/provable/field").Field;
        zcashAddress: import("o1js/dist/node/lib/provable/field").Field;
    };
    toInput: (x: {
        sender: PublicKey;
        amount: UInt64;
        targetChainId: import("o1js/dist/node/lib/provable/field").Field;
        recipient: import("o1js/dist/node/lib/provable/field").Field;
        zcashAddress: import("o1js/dist/node/lib/provable/field").Field;
    }) => {
        fields?: Field[] | undefined;
        packed?: [Field, number][] | undefined;
    };
    toJSON: (x: {
        sender: PublicKey;
        amount: UInt64;
        targetChainId: import("o1js/dist/node/lib/provable/field").Field;
        recipient: import("o1js/dist/node/lib/provable/field").Field;
        zcashAddress: import("o1js/dist/node/lib/provable/field").Field;
    }) => {
        sender: string;
        amount: string;
        targetChainId: string;
        recipient: string;
        zcashAddress: string;
    };
    fromJSON: (x: {
        sender: string;
        amount: string;
        targetChainId: string;
        recipient: string;
        zcashAddress: string;
    }) => {
        sender: PublicKey;
        amount: UInt64;
        targetChainId: import("o1js/dist/node/lib/provable/field").Field;
        recipient: import("o1js/dist/node/lib/provable/field").Field;
        zcashAddress: import("o1js/dist/node/lib/provable/field").Field;
    };
    empty: () => {
        sender: PublicKey;
        amount: UInt64;
        targetChainId: import("o1js/dist/node/lib/provable/field").Field;
        recipient: import("o1js/dist/node/lib/provable/field").Field;
        zcashAddress: import("o1js/dist/node/lib/provable/field").Field;
    };
};
/**
 * Deposit request structure
 */
export declare class MinaDepositRequest extends MinaDepositRequest_base {
}
declare const MinaWithdrawalRequest_base: (new (value: {
    withdrawalId: import("o1js/dist/node/lib/provable/field").Field;
    recipient: PublicKey;
    amount: UInt64;
    nullifier: import("o1js/dist/node/lib/provable/field").Field;
}) => {
    withdrawalId: import("o1js/dist/node/lib/provable/field").Field;
    recipient: PublicKey;
    amount: UInt64;
    nullifier: import("o1js/dist/node/lib/provable/field").Field;
}) & {
    _isStruct: true;
} & Omit<import("o1js/dist/node/lib/provable/types/provable-intf").Provable<{
    withdrawalId: import("o1js/dist/node/lib/provable/field").Field;
    recipient: PublicKey;
    amount: UInt64;
    nullifier: import("o1js/dist/node/lib/provable/field").Field;
}, {
    withdrawalId: bigint;
    recipient: {
        x: bigint;
        isOdd: boolean;
    };
    amount: bigint;
    nullifier: bigint;
}>, "fromFields"> & {
    fromFields: (fields: import("o1js/dist/node/lib/provable/field").Field[]) => {
        withdrawalId: import("o1js/dist/node/lib/provable/field").Field;
        recipient: PublicKey;
        amount: UInt64;
        nullifier: import("o1js/dist/node/lib/provable/field").Field;
    };
} & {
    fromValue: (value: {
        withdrawalId: string | number | bigint | import("o1js/dist/node/lib/provable/field").Field;
        recipient: PublicKey | {
            x: Field | bigint;
            isOdd: Bool | boolean;
        };
        amount: number | bigint | UInt64;
        nullifier: string | number | bigint | import("o1js/dist/node/lib/provable/field").Field;
    }) => {
        withdrawalId: import("o1js/dist/node/lib/provable/field").Field;
        recipient: PublicKey;
        amount: UInt64;
        nullifier: import("o1js/dist/node/lib/provable/field").Field;
    };
    toInput: (x: {
        withdrawalId: import("o1js/dist/node/lib/provable/field").Field;
        recipient: PublicKey;
        amount: UInt64;
        nullifier: import("o1js/dist/node/lib/provable/field").Field;
    }) => {
        fields?: Field[] | undefined;
        packed?: [Field, number][] | undefined;
    };
    toJSON: (x: {
        withdrawalId: import("o1js/dist/node/lib/provable/field").Field;
        recipient: PublicKey;
        amount: UInt64;
        nullifier: import("o1js/dist/node/lib/provable/field").Field;
    }) => {
        withdrawalId: string;
        recipient: string;
        amount: string;
        nullifier: string;
    };
    fromJSON: (x: {
        withdrawalId: string;
        recipient: string;
        amount: string;
        nullifier: string;
    }) => {
        withdrawalId: import("o1js/dist/node/lib/provable/field").Field;
        recipient: PublicKey;
        amount: UInt64;
        nullifier: import("o1js/dist/node/lib/provable/field").Field;
    };
    empty: () => {
        withdrawalId: import("o1js/dist/node/lib/provable/field").Field;
        recipient: PublicKey;
        amount: UInt64;
        nullifier: import("o1js/dist/node/lib/provable/field").Field;
    };
};
/**
 * Withdrawal request structure
 */
export declare class MinaWithdrawalRequest extends MinaWithdrawalRequest_base {
}
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
export declare class MinaGateway extends SmartContract {
    coordinator: State<PublicKey>;
    totalLocked: State<import("o1js/dist/node/lib/provable/field").Field>;
    totalWithdrawn: State<import("o1js/dist/node/lib/provable/field").Field>;
    depositCount: State<import("o1js/dist/node/lib/provable/field").Field>;
    withdrawalCount: State<import("o1js/dist/node/lib/provable/field").Field>;
    paused: State<import("o1js/dist/node/lib/provable/bool").Bool>;
    lastWithdrawalId: State<import("o1js/dist/node/lib/provable/field").Field>;
    events: {
        'tokens-locked': typeof MinaDepositRequest;
        'withdrawal-requested': typeof MinaWithdrawalRequest;
        'tokens-released': typeof MinaWithdrawalRequest;
        'coordinator-updated': typeof PublicKey;
        'emergency-pause': typeof import("o1js/dist/node/lib/provable/field").Field & ((x: string | number | bigint | import("o1js/dist/node/lib/provable/core/fieldvar").FieldConst | import("o1js/dist/node/lib/provable/core/fieldvar").FieldVar | import("o1js/dist/node/lib/provable/field").Field) => import("o1js/dist/node/lib/provable/field").Field);
    };
    init(): void;
    /**
     * Set coordinator (admin only)
     */
    setCoordinator(newCoordinator: PublicKey, adminSignature: Signature): Promise<void>;
    /**
     * Deposit - locks tokens and emits event
     */
    deposit(amount: UInt64, targetChainId: Field, recipient: Field, zcashAddress: Field, signature: Signature): Promise<void>;
    /**
     * Request withdrawal - Step 1 (emits event for relayer)
     */
    requestWithdrawal(amount: UInt64, nullifier: Field, zcashProof: Field, merkleRoot: Field, signature: Signature): Promise<void>;
    /**
     * Execute withdrawal - Step 2 (with coordinator signature)
     */
    executeWithdrawal(withdrawalId: Field, recipient: PublicKey, amount: UInt64, nullifier: Field, coordinatorSignature: Signature): Promise<void>;
    /**
     * Set paused state
     */
    setPaused(paused: Bool, adminSignature: Signature): Promise<void>;
}
export declare function deployGateway(deployerPrivateKeyBase58: string, coordinatorPublicKeyBase58: string, network?: 'devnet' | 'testnet' | 'mainnet'): Promise<PublicKey>;
export declare function interactWithGateway(gatewayAddress: string, userPrivateKeyBase58: string, network?: 'devnet' | 'testnet' | 'mainnet'): Promise<{
    deposit: (amount: bigint, targetChainId: bigint, recipient: bigint, zcashAddress: bigint) => Promise<string>;
    requestWithdrawal: (amount: bigint, nullifier: bigint, zcashProof: bigint, merkleRoot: bigint) => Promise<string>;
    getState: () => Promise<{
        totalLocked: string;
        totalWithdrawn: string;
        depositCount: string;
        withdrawalCount: string;
        paused: boolean;
    }>;
}>;
export type { MinaDepositRequest as DepositRequest, MinaWithdrawalRequest as WithdrawalRequest };
export declare const TESTNET_CONFIG: {
    minDepositAmount: bigint;
    maxDepositAmount: bigint;
    bridgeFee: number;
};
//# sourceMappingURL=mina-gateway.d.ts.map