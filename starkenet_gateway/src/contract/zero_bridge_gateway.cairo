#[starknet::contract]
pub mod ZeroBridgeGateway {
    use starknet::storage::StorageMapReadAccess;
use starknet::storage::StorageMapWriteAccess;
use starknet::{
        ContractAddress, get_caller_address, get_contract_address, get_block_timestamp
    };
    use starknet::storage::{Map, StoragePointerReadAccess, StoragePointerWriteAccess};
    use openzeppelin::token::erc20::interface::{IERC20Dispatcher, IERC20DispatcherTrait};
    use core::poseidon::poseidon_hash_span;
    use core::ecdsa::check_ecdsa_signature;
    use core::num::traits::Zero;

    use crate::interfaces::izero_bridge_gateway::IZeroBridgeGateway;
    use crate::types::{DepositInfo, WithdrawalRequestInfo, BridgeStats};

    const MIN_DEPOSIT: u256 = 1000000000000000_u256;
    const MAX_DEPOSIT: u256 = 1000000000000000000000000_u256;

    #[storage]
    struct Storage {
        owner: ContractAddress,
        coordinator: ContractAddress,
        paused: bool,
        deposit_count: u64,
        withdrawal_count: u64,
        locked_balances: Map<ContractAddress, u256>,
        total_deposits: u256,
        total_withdrawals: u256,
        deposits: Map<felt252, DepositInfo>,
        withdrawal_requests: Map<felt252, WithdrawalRequestInfo>,
        nullifiers: Map<felt252, bool>,
        liquidity_providers: Map<ContractAddress, bool>,
        bridge_fee: u16,
    }

    #[event]
    #[derive(Drop, starknet::Event)]
    pub enum Event {
        TokensLocked: TokensLocked,
        WithdrawalRequested: WithdrawalRequested,
        TokensReleased: TokensReleased,
        CoordinatorUpdated: CoordinatorUpdated,
        LiquidityAdded: LiquidityAdded,
        LiquidityRemoved: LiquidityRemoved,
        EmergencyPause: EmergencyPause,
        BridgeFeeUpdated: BridgeFeeUpdated,
    }

    #[derive(Drop, starknet::Event)]
    pub struct TokensLocked {
        #[key]
        pub deposit_id: felt252,
        #[key]
        pub sender: ContractAddress,
        #[key]
        pub token: ContractAddress,
        pub amount: u256,
        pub target_chain_id: u64,
        pub recipient: felt252,
        pub zcash_address: felt252,
        pub timestamp: u64,
    }

    #[derive(Drop, starknet::Event)]
    pub struct WithdrawalRequested {
        #[key]
        pub withdrawal_id: felt252,
        #[key]
        pub recipient: ContractAddress,
        #[key]
        pub token: ContractAddress,
        pub amount: u256,
        pub nullifier: felt252,
        pub timestamp: u64,
    }

    #[derive(Drop, starknet::Event)]
    pub struct TokensReleased {
        #[key]
        pub withdrawal_id: felt252,
        #[key]
        pub recipient: ContractAddress,
        #[key]
        pub token: ContractAddress,
        pub amount: u256,
        pub nullifier: felt252,
        pub timestamp: u64,
    }

    #[derive(Drop, starknet::Event)]
    pub struct CoordinatorUpdated {
        pub old_coordinator: ContractAddress,
        pub new_coordinator: ContractAddress,
        pub timestamp: u64,
    }

    #[derive(Drop, starknet::Event)]
    pub struct LiquidityAdded {
        pub provider: ContractAddress,
        pub token: ContractAddress,
        pub amount: u256,
        pub timestamp: u64,
    }

    #[derive(Drop, starknet::Event)]
    pub struct LiquidityRemoved {
        pub provider: ContractAddress,
        pub token: ContractAddress,
        pub amount: u256,
        pub timestamp: u64,
    }

    #[derive(Drop, starknet::Event)]
    pub struct EmergencyPause {
        pub triggered_by: ContractAddress,
        pub reason: felt252,
        pub timestamp: u64,
    }

    #[derive(Drop, starknet::Event)]
    pub struct BridgeFeeUpdated {
        pub old_fee: u16,
        pub new_fee: u16,
    }

    #[constructor]
    fn constructor(ref self: ContractState, coordinator: ContractAddress) {
        assert(!coordinator.is_zero(), 'Invalid coordinator');
        self.owner.write(get_caller_address());
        self.coordinator.write(coordinator);
        self.bridge_fee.write(30);
        self.paused.write(false);
    }

    #[abi(embed_v0)]
    impl ZeroBridgeGatewayImpl of IZeroBridgeGateway<ContractState> {
        fn deposit(
            ref self: ContractState,
            token: ContractAddress,
            amount: u256,
            target_chain_id: u64,
            recipient: felt252,
            zcash_address: felt252,
        ) -> felt252 {
            assert(!self.paused.read(), 'Bridge paused');
            assert(amount >= MIN_DEPOSIT, 'Amount too small');
            assert(amount <= MAX_DEPOSIT, 'Amount too large');
            assert(recipient != 0, 'Invalid recipient');
            assert(zcash_address != 0, 'Invalid Zcash address');

            let caller = get_caller_address();
            let fee = (amount * self.bridge_fee.read().into()) / 10000_u256;
            let net_amount = amount - fee;

            let erc20 = IERC20Dispatcher { contract_address: token };
            erc20.transfer_from(caller, get_contract_address(), amount);

            let deposit_id = self
                ._generate_deposit_id(caller, token, amount, target_chain_id, recipient);

            let info = DepositInfo {
                deposit_id,
                sender: caller,
                token,
                amount: net_amount,
                target_chain_id,
                recipient,
                zcash_address,
                timestamp: get_block_timestamp(),
                processed: false,
            };

            self.deposits.write(deposit_id, info);

            let locked = self.locked_balances.read(token);
            self.locked_balances.write(token, locked + net_amount);

            let total_deps = self.total_deposits.read();
            self.total_deposits.write(total_deps + net_amount);

            let dep_count = self.deposit_count.read();
            self.deposit_count.write(dep_count + 1);

            self
                .emit(
                    Event::TokensLocked(
                        TokensLocked {
                            deposit_id,
                            sender: caller,
                            token,
                            amount: net_amount,
                            target_chain_id,
                            recipient,
                            zcash_address,
                            timestamp: get_block_timestamp(),
                        }
                    )
                );

            deposit_id
        }

        fn request_withdrawal(
            ref self: ContractState,
            token: ContractAddress,
            amount: u256,
            nullifier: felt252,
            zcash_proof: Array<felt252>,
            merkle_root: felt252,
        ) -> felt252 {
            assert(!self.paused.read(), 'Bridge paused');
            assert(amount > 0, 'Invalid amount');
            assert(nullifier != 0, 'Invalid nullifier');
            assert(merkle_root != 0, 'Invalid merkle root');
            assert(!self.nullifiers.read(nullifier), 'Nullifier already used');

            let caller = get_caller_address();
            let withdrawal_id = self._generate_withdrawal_id(caller, token, amount, nullifier);

            let request = WithdrawalRequestInfo {
                withdrawal_id,
                recipient: caller,
                token,
                amount,
                nullifier,
                timestamp: get_block_timestamp(),
                executed: false,
            };

            self.withdrawal_requests.write(withdrawal_id, request);

            let wd_count = self.withdrawal_count.read();
            self.withdrawal_count.write(wd_count + 1);

            self
                .emit(
                    Event::WithdrawalRequested(
                        WithdrawalRequested {
                            withdrawal_id,
                            recipient: caller,
                            token,
                            amount,
                            nullifier,
                            timestamp: get_block_timestamp(),
                        }
                    )
                );

            withdrawal_id
        }

        fn execute_withdrawal(
            ref self: ContractState,
            withdrawal_id: felt252,
            coordinator_signature_r: felt252,
            coordinator_signature_s: felt252,
        ) -> bool {
            assert(!self.paused.read(), 'Bridge paused');

            let request = self.withdrawal_requests.read(withdrawal_id);
            assert(request.withdrawal_id != 0, 'Withdrawal not found');
            assert(!request.executed, 'Already executed');
            assert(!self.nullifiers.read(request.nullifier), 'Nullifier used');

            let locked = self.locked_balances.read(request.token);
            assert(locked >= request.amount, 'Insufficient locked balance');

            self
                ._verify_coordinator_signature(
                    withdrawal_id,
                    request.recipient,
                    request.token,
                    request.amount,
                    request.nullifier,
                    coordinator_signature_r,
                    coordinator_signature_s,
                );

            let updated = WithdrawalRequestInfo {
                withdrawal_id: request.withdrawal_id,
                recipient: request.recipient,
                token: request.token,
                amount: request.amount,
                nullifier: request.nullifier,
                timestamp: request.timestamp,
                executed: true,
            };

            self.withdrawal_requests.write(withdrawal_id, updated);
            self.nullifiers.write(request.nullifier, true);
            self.locked_balances.write(request.token, locked - request.amount);

            let total_wds = self.total_withdrawals.read();
            self.total_withdrawals.write(total_wds + request.amount);

            let erc20 = IERC20Dispatcher { contract_address: request.token };
            erc20.transfer(request.recipient, request.amount);

            self
                .emit(
                    Event::TokensReleased(
                        TokensReleased {
                            withdrawal_id,
                            recipient: request.recipient,
                            token: request.token,
                            amount: request.amount,
                            nullifier: request.nullifier,
                            timestamp: get_block_timestamp(),
                        }
                    )
                );

            true
        }

        fn add_liquidity(ref self: ContractState, token: ContractAddress, amount: u256) {
            assert(!self.paused.read(), 'Bridge paused');
            let caller = get_caller_address();
            assert(self.liquidity_providers.read(caller), 'Not liquidity provider');
            assert(amount > 0, 'Invalid amount');

            let erc20 = IERC20Dispatcher { contract_address: token };
            erc20.transfer_from(caller, get_contract_address(), amount);

            self
                .emit(
                    Event::LiquidityAdded(
                        LiquidityAdded {
                            provider: caller, token, amount, timestamp: get_block_timestamp(),
                        }
                    )
                );
        }

        fn remove_liquidity(ref self: ContractState, token: ContractAddress, amount: u256) {
            assert(!self.paused.read(), 'Bridge paused');
            let caller = get_caller_address();
            assert(self.liquidity_providers.read(caller), 'Not LP');

            let available = self.get_available_liquidity(token);
            assert(available >= amount, 'Insufficient liquidity');

            let erc20 = IERC20Dispatcher { contract_address: token };
            erc20.transfer(caller, amount);

            self
                .emit(
                    Event::LiquidityRemoved(
                        LiquidityRemoved {
                            provider: caller, token, amount, timestamp: get_block_timestamp(),
                        }
                    )
                );
        }

        fn get_locked_balance(self: @ContractState, token: ContractAddress) -> u256 {
            self.locked_balances.read(token)
        }

        fn get_available_liquidity(self: @ContractState, token: ContractAddress) -> u256 {
            let erc20 = IERC20Dispatcher { contract_address: token };
            let total = erc20.balance_of(get_contract_address());
            let locked = self.locked_balances.read(token);
            if total >= locked {
                total - locked
            } else {
                0
            }
        }

        fn is_nullifier_used(self: @ContractState, nullifier: felt252) -> bool {
            self.nullifiers.read(nullifier)
        }

        fn get_deposit(self: @ContractState, deposit_id: felt252) -> DepositInfo {
            self.deposits.read(deposit_id)
        }

        fn get_withdrawal_request(
            self: @ContractState, withdrawal_id: felt252
        ) -> WithdrawalRequestInfo {
            self.withdrawal_requests.read(withdrawal_id)
        }

        fn get_stats(self: @ContractState) -> BridgeStats {
            let deposits = self.total_deposits.read();
            let withdrawals = self.total_withdrawals.read();
            BridgeStats {
                total_deposits: deposits,
                total_withdrawals: withdrawals,
                total_volume: deposits + withdrawals,
                active_deposits: if deposits > withdrawals {
                    deposits - withdrawals
                } else {
                    0
                },
            }
        }

        fn set_coordinator(ref self: ContractState, new_coordinator: ContractAddress) {
            self._assert_owner();
            assert(!new_coordinator.is_zero(), 'Invalid coordinator');
            let old = self.coordinator.read();
            self.coordinator.write(new_coordinator);

            self
                .emit(
                    Event::CoordinatorUpdated(
                        CoordinatorUpdated {
                            old_coordinator: old,
                            new_coordinator,
                            timestamp: get_block_timestamp(),
                        }
                    )
                );
        }

        fn add_liquidity_provider(ref self: ContractState, provider: ContractAddress) {
            self._assert_owner();
            assert(!provider.is_zero(), 'Invalid provider');
            self.liquidity_providers.write(provider, true);
        }

        fn remove_liquidity_provider(ref self: ContractState, provider: ContractAddress) {
            self._assert_owner();
            self.liquidity_providers.write(provider, false);
        }

        fn set_paused(ref self: ContractState, paused: bool) {
            self._assert_owner();
            self.paused.write(paused);
            if paused {
                self
                    .emit(
                        Event::EmergencyPause(
                            EmergencyPause {
                                triggered_by: get_caller_address(),
                                reason: 'Manual pause',
                                timestamp: get_block_timestamp(),
                            }
                        )
                    );
            }
        }

        fn set_bridge_fee(ref self: ContractState, new_fee: u16) {
            self._assert_owner();
            assert(new_fee <= 100, 'Fee > 1%');
            let old_fee = self.bridge_fee.read();
            self.bridge_fee.write(new_fee);
            self.emit(Event::BridgeFeeUpdated(BridgeFeeUpdated { old_fee, new_fee }));
        }

        fn emergency_withdraw(
            ref self: ContractState, token: ContractAddress, to: ContractAddress, amount: u256
        ) {
            self._assert_owner();
            assert(self.paused.read(), 'Must be paused');
            assert(!to.is_zero(), 'Invalid recipient');

            let erc20 = IERC20Dispatcher { contract_address: token };
            erc20.transfer(to, amount);
        }
    }

    #[generate_trait]
    impl InternalImpl of InternalTrait {
        fn _assert_owner(self: @ContractState) {
            assert(get_caller_address() == self.owner.read(), 'Only owner');
        }

        fn _generate_deposit_id(
            self: @ContractState,
            sender: ContractAddress,
            token: ContractAddress,
            amount: u256,
            target_chain_id: u64,
            recipient: felt252,
        ) -> felt252 {
            let mut data = array![];
            data.append(sender.into());
            data.append(token.into());
            data.append(amount.low.into());
            data.append(amount.high.into());
            data.append(target_chain_id.into());
            data.append(recipient);
            data.append(get_block_timestamp().into());
            poseidon_hash_span(data.span())
        }

        fn _generate_withdrawal_id(
            self: @ContractState,
            recipient: ContractAddress,
            token: ContractAddress,
            amount: u256,
            nullifier: felt252,
        ) -> felt252 {
            let mut data = array![];
            data.append(recipient.into());
            data.append(token.into());
            data.append(amount.low.into());
            data.append(amount.high.into());
            data.append(nullifier);
            data.append(get_block_timestamp().into());
            poseidon_hash_span(data.span())
        }

        fn _verify_coordinator_signature(
            self: @ContractState,
            withdrawal_id: felt252,
            recipient: ContractAddress,
            token: ContractAddress,
            amount: u256,
            nullifier: felt252,
            r: felt252,
            s: felt252,
        ) {
            let mut data = array![];
            data.append(withdrawal_id);
            data.append(recipient.into());
            data.append(token.into());
            data.append(amount.low.into());
            data.append(amount.high.into());
            data.append(nullifier);

            let hash = poseidon_hash_span(data.span());
            let coordinator = self.coordinator.read();
            let coordinator_pubkey: felt252 = coordinator.into();
            assert(
                check_ecdsa_signature(hash, coordinator_pubkey, r, s), 'Invalid coordinator sig'
            );
        }
    }
}