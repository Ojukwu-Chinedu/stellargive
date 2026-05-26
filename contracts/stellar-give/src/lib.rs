#![no_std]

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, token, Address, Env, String,
    Symbol,
};

#[contract]
pub struct StellarGiveContract;

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub enum CampaignStatus {
    Active,
    Funded,
    Claimed,
    Expired,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct CreatedEvent {
    pub id: u64,
    pub creator: Address,
    pub target_amount: i128,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct Campaign {
    pub id: u64,
    pub creator: Address,
    pub beneficiary: Address,
    pub title: String,
    pub target_amount: i128,
    pub raised_amount: i128,
    pub deadline: u64,
    pub accepted_token: Address,
    pub status: CampaignStatus,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[contracterror]
#[repr(u32)]
pub enum ContractError {
    Unauthorized = 1,
    InvalidDeadline = 2,
    InvalidAmount = 3,
    CampaignNotFound = 4,
    InvalidToken = 5,
    CampaignNotActive = 6,
    ClaimNotAllowed = 7,
    AlreadyClaimed = 8,
    ReentrancyDetected = 9,
    EmptyTitle = 10,
    NothingToClaim = 11,
    /// Token transfer failed — revert donation, no state changes persist.
    TokenTransferFailed = 12,
    NotInitialized = 13,
    AlreadyInitialized = 14,
}

fn next_id_key() -> Symbol {
    symbol_short!("NEXT")
}

fn lock_key() -> Symbol {
    symbol_short!("LOCK")
}

fn admin_key() -> Symbol {
    symbol_short!("ADMIN")
}

/// Platform fee, in basis points. 100 = 1.00%.
const FEE_BPS: i128 = 100;
/// Basis-point denominator (10_000 = 100%).
const FEE_DENOMINATOR: i128 = 10_000;

fn read_admin(env: &Env) -> Result<Address, ContractError> {
    env.storage()
        .persistent()
        .get(&admin_key())
        .ok_or(ContractError::NotInitialized)
}

fn write_admin(env: &Env, admin: &Address) {
    env.storage().persistent().set(&admin_key(), admin);
}

/// Computes the platform fee for a settlement of `amount`. Uses round-half-up
/// against `FEE_DENOMINATOR` so a half-stroop remainder accrues to the
/// platform rather than the beneficiary.
fn calculate_platform_fee(amount: i128) -> Result<i128, ContractError> {
    let scaled = amount
        .checked_mul(FEE_BPS)
        .ok_or(ContractError::InvalidAmount)?;
    let biased = scaled
        .checked_add(FEE_DENOMINATOR / 2)
        .ok_or(ContractError::InvalidAmount)?;
    Ok(biased / FEE_DENOMINATOR)
}

fn campaign_key(id: u64) -> (Symbol, u64) {
    (symbol_short!("CMP"), id)
}

fn read_next_id(env: &Env) -> u64 {
    env.storage()
        .persistent()
        .get(&next_id_key())
        .unwrap_or(1_u64)
}

fn write_next_id(env: &Env, next_id: u64) {
    env.storage().persistent().set(&next_id_key(), &next_id);
}

fn read_campaign(env: &Env, id: u64) -> Result<Campaign, ContractError> {
    env.storage()
        .persistent()
        .get(&campaign_key(id))
        .ok_or(ContractError::CampaignNotFound)
}

fn write_campaign(env: &Env, campaign: &Campaign) {
    env.storage()
        .persistent()
        .set(&campaign_key(campaign.id), campaign);
}

/// Sets a temporary-storage lock for the current invocation context.
/// Returns `ReentrancyDetected` if the lock is already held, preventing
/// re-entrant state mutations.
fn enter_lock(env: &Env) -> Result<(), ContractError> {
    let key = lock_key();
    if env.storage().temporary().get::<_, bool>(&key).unwrap_or(false) {
        return Err(ContractError::ReentrancyDetected);
    }
    env.storage().temporary().set(&key, &true);
    Ok(())
}

/// Releases the reentrancy lock unconditionally.  Called on every exit path
/// (success and failure) to guarantee the lock is not left held.
fn exit_lock(env: &Env) {
    env.storage().temporary().remove(&lock_key());
}

fn derive_status(now: u64, campaign: &Campaign) -> CampaignStatus {
    // Claimed is terminal and must not be downgraded by timestamp checks.
    if campaign.status == CampaignStatus::Claimed {
        return CampaignStatus::Claimed;
    }

    if campaign.raised_amount >= campaign.target_amount {
        return CampaignStatus::Funded;
    }

    if now > campaign.deadline {
        return CampaignStatus::Expired;
    }

    CampaignStatus::Active
}

fn sync_status(env: &Env, campaign: &mut Campaign) {
    let updated = derive_status(env.ledger().timestamp(), campaign);
    if updated != campaign.status {
        campaign.status = updated;
        write_campaign(env, campaign);
    }
}

/// Validates that `token_address` implements the Soroban Asset Contract (SEP-41)
/// interface by calling two lightweight read methods.  Returns `InvalidToken`
/// if either call fails, preventing campaigns from being created with
/// non-compliant or malicious token contracts.
fn validate_token_contract(env: &Env, token_address: &Address) -> Result<(), ContractError> {
    let client = token::TokenClient::new(env, token_address);
    // Both calls must succeed — a malicious contract that panics on either
    // will cause this to return InvalidToken.
    if client.try_decimals().is_err() {
        return Err(ContractError::InvalidToken);
    }
    if client.try_symbol().is_err() {
        return Err(ContractError::InvalidToken);
    }
    Ok(())
}

#[contractimpl]
impl StellarGiveContract {
    /// One-shot initializer. Sets the platform admin address that receives
    /// the fee portion of every successful claim. Must be called before any
    /// `claim_funds` invocation.
    pub fn initialize(env: Env, admin: Address) -> Result<(), ContractError> {
        if env.storage().persistent().has(&admin_key()) {
            return Err(ContractError::AlreadyInitialized);
        }
        admin.require_auth();
        write_admin(&env, &admin);
        Ok(())
    }

    pub fn create_campaign(
        env: Env,
        creator: Address,
        beneficiary: Address,
        title: String,
        target_amount: i128,
        deadline: u64,
        accepted_token: Address,
    ) -> Result<u64, ContractError> {
        creator.require_auth();

        if title.len() == 0 {
            return Err(ContractError::EmptyTitle);
        }
        if target_amount <= 0 {
            return Err(ContractError::InvalidAmount);
        }
        if deadline <= env.ledger().timestamp() {
            return Err(ContractError::InvalidDeadline);
        }

        // Lock before any cross-contract calls to block re-entrant creation
        // attempts from a malicious token's validate path.
        enter_lock(&env)?;
        let result = (|| {
            validate_token_contract(&env, &accepted_token)?;

            let id = read_next_id(&env);
            let next_id = id.checked_add(1).ok_or(ContractError::InvalidAmount)?;
            write_next_id(&env, next_id);

            let campaign = Campaign {
                id,
                creator: creator.clone(),
                beneficiary: beneficiary.clone(),
                title,
                target_amount,
                raised_amount: 0,
                deadline,
                accepted_token: accepted_token.clone(),
                status: CampaignStatus::Active,
            };

            write_campaign(&env, &campaign);
            env.events().publish(
                (symbol_short!("created"),),
                CreatedEvent {
                    id,
                    creator,
                    target_amount: campaign.target_amount,
                },
            );

            Ok(id)
        })();

        exit_lock(&env);
        result
    }

    pub fn donate(
        env: Env,
        donor: Address,
        campaign_id: u64,
        amount: i128,
    ) -> Result<(), ContractError> {
        donor.require_auth();
        if amount <= 0 {
            return Err(ContractError::InvalidAmount);
        }

        enter_lock(&env)?;
        let result = (|| {
            let mut campaign = read_campaign(&env, campaign_id)?;
            sync_status(&env, &mut campaign);

            if campaign.status != CampaignStatus::Active {
                return Err(ContractError::CampaignNotActive);
            }

            // Use try_transfer so a failing token contract reverts the donation
            // cleanly instead of propagating a raw panic.
            if token::TokenClient::new(&env, &campaign.accepted_token)
                .try_transfer(&donor, &env.current_contract_address(), &amount)
                .is_err()
            {
                return Err(ContractError::TokenTransferFailed);
            }

            campaign.raised_amount = campaign
                .raised_amount
                .checked_add(amount)
                .ok_or(ContractError::InvalidAmount)?;

            campaign.status = if campaign.raised_amount >= campaign.target_amount {
                CampaignStatus::Funded
            } else {
                CampaignStatus::Active
            };

            write_campaign(&env, &campaign);
            env.events().publish(
                (symbol_short!("donation"), symbol_short!("received")),
                (
                    campaign.id,
                    donor,
                    amount,
                    campaign.raised_amount,
                    campaign.accepted_token.clone(),
                ),
            );
            Ok(())
        })();

        exit_lock(&env);
        result
    }

    pub fn claim_funds(env: Env, caller: Address, campaign_id: u64) -> Result<i128, ContractError> {
        let mut campaign = read_campaign(&env, campaign_id)?;
        sync_status(&env, &mut campaign);

        if campaign.status == CampaignStatus::Claimed {
            return Err(ContractError::AlreadyClaimed);
        }

        if caller != campaign.creator && caller != campaign.beneficiary {
            return Err(ContractError::Unauthorized);
        }
        caller.require_auth();

        let now = env.ledger().timestamp();
        let can_claim = campaign.raised_amount >= campaign.target_amount || now > campaign.deadline;
        if !can_claim {
            return Err(ContractError::ClaimNotAllowed);
        }
        if campaign.raised_amount <= 0 {
            return Err(ContractError::NothingToClaim);
        }

        enter_lock(&env)?;
        let result = (|| {
            let admin = read_admin(&env)?;
            let amount = campaign.raised_amount;
            let fee = calculate_platform_fee(amount)?;
            let net = amount
                .checked_sub(fee)
                .ok_or(ContractError::InvalidAmount)?;

            // Two-leg payout: fee → admin, net → beneficiary. The fee leg is
            // skipped entirely when rounding produces a zero fee, so small
            // claims do not pay the runtime cost of a no-op transfer.
            let token = token::TokenClient::new(&env, &campaign.accepted_token);
            if fee > 0 {
                token.transfer(&env.current_contract_address(), &admin, &fee);
            }
            token.transfer(&env.current_contract_address(), &campaign.beneficiary, &net);

            campaign.raised_amount = 0;
            campaign.status = CampaignStatus::Claimed;
            write_campaign(&env, &campaign);

            // `amount` in the event continues to represent the gross settled
            // amount (fee + net), preserving the existing indexer contract.
            env.events().publish(
                (symbol_short!("funds"), symbol_short!("claimed")),
                (campaign.id, caller, campaign.beneficiary, amount, campaign.accepted_token),
            );

            Ok(amount)
        })();

        exit_lock(&env);
        result
    }

    pub fn get_campaign(env: Env, campaign_id: u64) -> Result<Campaign, ContractError> {
        let mut campaign = read_campaign(&env, campaign_id)?;
        campaign.status = derive_status(env.ledger().timestamp(), &campaign);
        Ok(campaign)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::testutils::{Address as _, Events as _, Ledger};
    use soroban_sdk::{token, Address, Env, String, TryFromVal};

    fn set_timestamp(env: &Env, timestamp: u64) {
        let mut ledger = env.ledger().get();
        ledger.timestamp = timestamp;
        env.ledger().set(ledger);
    }

    fn setup() -> (
        Env,
        StellarGiveContractClient<'static>,
        Address,
        Address,
        Address,
        Address,
        token::Client<'static>,
        token::StellarAssetClient<'static>,
    ) {
        let env = Env::default();
        env.mock_all_auths();

        let creator = Address::generate(&env);
        let beneficiary = Address::generate(&env);
        let donor = Address::generate(&env);
        let platform_admin = Address::generate(&env);
        let token_admin = Address::generate(&env);

        let token_id = env.register_stellar_asset_contract_v2(token_admin.clone());
        let token_client = token::Client::new(&env, &token_id.address());
        let token_admin_client = token::StellarAssetClient::new(&env, &token_id.address());

        token_admin_client.mint(&donor, &1_000_000);
        token_admin_client.mint(&creator, &1_000_000);

        let contract_id = env.register_contract(None, StellarGiveContract);
        let client = StellarGiveContractClient::new(&env, &contract_id);
        client.initialize(&platform_admin);

        (
            env,
            client,
            creator,
            beneficiary,
            donor,
            platform_admin,
            token_client,
            token_admin_client,
        )
    }

    #[test]
    fn create_and_get_campaign() {
        let (env, client, creator, beneficiary, _donor, _admin, token_client, _) = setup();
        set_timestamp(&env, 1_000);

        let id = client.create_campaign(
            &creator,
            &beneficiary,
            &String::from_str(&env, "Flood Relief"),
            &500_000,
            &2_000,
            &token_client.address,
        );

        let campaign = client.get_campaign(&id);
        assert_eq!(campaign.id, 1);
        assert_eq!(campaign.status, CampaignStatus::Active);
        assert_eq!(campaign.creator, creator);
        assert_eq!(campaign.beneficiary, beneficiary);
        assert_eq!(campaign.target_amount, 500_000);
        assert_eq!(campaign.raised_amount, 0);
    }

    #[test]
    fn create_campaign_emits_created_event() {
        let (env, client, creator, beneficiary, _donor, _admin, token_client, _) = setup();
        set_timestamp(&env, 1_000);

        let target_amount: i128 = 500_000;
        let id = client.create_campaign(
            &creator,
            &beneficiary,
            &String::from_str(&env, "Flood Relief"),
            &target_amount,
            &2_000,
            &token_client.address,
        );

        let event = env
            .events()
            .all()
            .iter()
            .find(|(addr, topics, _)| {
                addr == &client.address
                    && topics
                        .get(0)
                        .and_then(|t| Symbol::try_from_val(&env, &t).ok())
                        == Some(symbol_short!("created"))
            })
            .expect("CreatedEvent was not emitted by create_campaign");

        let payload = CreatedEvent::try_from_val(&env, &event.2)
            .expect("event data did not decode as CreatedEvent");
        assert_eq!(payload.id, id);
        assert_eq!(payload.creator, creator);
        assert_eq!(payload.target_amount, target_amount);
    }

    #[test]
    fn donate_updates_raised_and_status() {
        let (env, client, creator, beneficiary, donor, _admin, token_client, _) = setup();
        set_timestamp(&env, 5_000);

        let campaign_id = client.create_campaign(
            &creator,
            &beneficiary,
            &String::from_str(&env, "Medical Aid"),
            &100_000,
            &10_000,
            &token_client.address,
        );

        client.donate(&donor, &campaign_id, &40_000);
        let campaign_after_first = client.get_campaign(&campaign_id);
        assert_eq!(campaign_after_first.raised_amount, 40_000);
        assert_eq!(campaign_after_first.status, CampaignStatus::Active);

        client.donate(&donor, &campaign_id, &60_000);
        let campaign_after_second = client.get_campaign(&campaign_id);
        assert_eq!(campaign_after_second.raised_amount, 100_000);
        assert_eq!(campaign_after_second.status, CampaignStatus::Funded);
    }

    #[test]
    fn claim_when_target_met_transfers_to_beneficiary() {
        let (env, client, creator, beneficiary, donor, admin, token_client, _) = setup();
        set_timestamp(&env, 10_000);

        let campaign_id = client.create_campaign(
            &creator,
            &beneficiary,
            &String::from_str(&env, "School Rebuild"),
            &120_000,
            &20_000,
            &token_client.address,
        );

        client.donate(&donor, &campaign_id, &120_000);

        let beneficiary_before = token_client.balance(&beneficiary);
        let admin_before = token_client.balance(&admin);
        let claimed = client.claim_funds(&creator, &campaign_id);
        let beneficiary_after = token_client.balance(&beneficiary);
        let admin_after = token_client.balance(&admin);
        let campaign = client.get_campaign(&campaign_id);

        // Gross return value is unchanged; the 1% fee is split off internally.
        assert_eq!(claimed, 120_000);
        assert_eq!(beneficiary_after - beneficiary_before, 118_800);
        assert_eq!(admin_after - admin_before, 1_200);
        assert_eq!(campaign.status, CampaignStatus::Claimed);
        assert_eq!(campaign.raised_amount, 0);
    }

    #[test]
    fn claim_after_deadline_when_target_not_met() {
        let (env, client, creator, beneficiary, donor, _admin, token_client, _) = setup();
        set_timestamp(&env, 100);

        let campaign_id = client.create_campaign(
            &creator,
            &beneficiary,
            &String::from_str(&env, "Emergency Shelter"),
            &500_000,
            &500,
            &token_client.address,
        );

        client.donate(&donor, &campaign_id, &50_000);
        set_timestamp(&env, 600);

        let claimed = client.claim_funds(&beneficiary, &campaign_id);
        let campaign = client.get_campaign(&campaign_id);

        assert_eq!(claimed, 50_000);
        assert_eq!(campaign.status, CampaignStatus::Claimed);
    }

    #[test]
    fn unauthorized_claim_fails() {
        let (env, client, creator, beneficiary, donor, _admin, token_client, _) = setup();
        set_timestamp(&env, 200);

        let campaign_id = client.create_campaign(
            &creator,
            &beneficiary,
            &String::from_str(&env, "Food Support"),
            &100_000,
            &1_000,
            &token_client.address,
        );
        client.donate(&donor, &campaign_id, &10_000);
        set_timestamp(&env, 1_100);

        let attacker = Address::generate(&env);
        let error = client.try_claim_funds(&attacker, &campaign_id);
        assert!(error.is_err());
    }

    // -----------------------------------------------------------------------
    // Issue #54 — token validation tests
    // -----------------------------------------------------------------------

    #[test]
    fn valid_sac_token_passes_validation() {
        let env = Env::default();
        env.mock_all_auths();
        let token_admin = Address::generate(&env);
        let token_id = env.register_stellar_asset_contract_v2(token_admin);
        // Should not panic or return error.
        validate_token_contract(&env, &token_id.address())
            .expect("valid SAC should pass token validation");
    }

    #[test]
    fn non_sac_contract_rejected_at_create_campaign() {
        let env = Env::default();
        env.mock_all_auths();
        set_timestamp(&env, 1_000);

        // Register the StellarGiveContract itself as the "token" — it has no
        // decimals() or symbol(), so validation must fail.
        let bogus_token = env.register_contract(None, StellarGiveContract);
        let contract_id = env.register_contract(None, StellarGiveContract);
        let client = StellarGiveContractClient::new(&env, &contract_id);

        let creator = Address::generate(&env);
        let beneficiary = Address::generate(&env);

        let result = client.try_create_campaign(
            &creator,
            &beneficiary,
            &String::from_str(&env, "Test"),
            &100_000,
            &5_000,
            &bogus_token,
        );
        assert!(
            result.is_err(),
            "create_campaign must reject a non-SAC token"
        );
    }

    // -----------------------------------------------------------------------
    // Issue #55 — reentrancy lock tests
    // -----------------------------------------------------------------------

    #[test]
    fn reentrancy_lock_blocks_concurrent_entry() {
        let env = Env::default();
        let contract_id = env.register_contract(None, StellarGiveContract);

        // Storage access must happen inside the contract's execution context.
        env.as_contract(&contract_id, || {
            assert!(enter_lock(&env).is_ok(), "first enter_lock should succeed");

            let second = enter_lock(&env);
            assert_eq!(
                second,
                Err(ContractError::ReentrancyDetected),
                "second enter_lock must return ReentrancyDetected"
            );

            exit_lock(&env);
            assert!(
                enter_lock(&env).is_ok(),
                "enter_lock must succeed after exit_lock"
            );
            exit_lock(&env);
        });
    }

    #[test]
    fn donate_blocked_when_lock_already_held() {
        let (env, client, creator, beneficiary, donor, _admin, token_client, _) = setup();
        set_timestamp(&env, 5_000);

        let campaign_id = client.create_campaign(
            &creator,
            &beneficiary,
            &String::from_str(&env, "Re-entry Test"),
            &100_000,
            &10_000,
            &token_client.address,
        );

        // Set the lock from inside the contract context to simulate a held lock.
        env.as_contract(&client.address, || {
            env.storage().temporary().set(&lock_key(), &true);
        });

        let result = client.try_donate(&donor, &campaign_id, &1_000);
        assert!(
            result.is_err(),
            "donate must fail when reentrancy lock is already held"
        );

        env.as_contract(&client.address, || {
            env.storage().temporary().remove(&lock_key());
        });
    }

    #[test]
    fn create_campaign_blocked_when_lock_already_held() {
        let env = Env::default();
        env.mock_all_auths();
        set_timestamp(&env, 1_000);

        let token_admin = Address::generate(&env);
        let token_id = env.register_stellar_asset_contract_v2(token_admin);
        let contract_id = env.register_contract(None, StellarGiveContract);
        let client = StellarGiveContractClient::new(&env, &contract_id);

        env.as_contract(&contract_id, || {
            env.storage().temporary().set(&lock_key(), &true);
        });

        let result = client.try_create_campaign(
            &Address::generate(&env),
            &Address::generate(&env),
            &String::from_str(&env, "Test"),
            &100_000,
            &5_000,
            &token_id.address(),
        );
        assert!(
            result.is_err(),
            "create_campaign must fail when reentrancy lock is already held"
        );

        env.as_contract(&contract_id, || {
            env.storage().temporary().remove(&lock_key());
        });
    }

    // -----------------------------------------------------------------------
    // Issue — Fee on claim tests
    // -----------------------------------------------------------------------

    #[test]
    fn calculate_platform_fee_round_half_up() {
        // Below the half-stroop threshold: fee rounds down to 0.
        assert_eq!(calculate_platform_fee(0).unwrap(), 0);
        assert_eq!(calculate_platform_fee(49).unwrap(), 0);

        // Exact half-stroop remainder: rounds up to favor the platform.
        assert_eq!(calculate_platform_fee(50).unwrap(), 1);

        // Exact 1% with no remainder.
        assert_eq!(calculate_platform_fee(100).unwrap(), 1);
        assert_eq!(calculate_platform_fee(100_000).unwrap(), 1_000);

        // Remainder above the half threshold rounds up; below rounds down.
        assert_eq!(calculate_platform_fee(149).unwrap(), 1);
        assert_eq!(calculate_platform_fee(150).unwrap(), 2);
    }

    #[test]
    fn claim_funds_fee_round_half_up_boundary() {
        let (env, client, creator, beneficiary, donor, admin, token_client, _) = setup();
        set_timestamp(&env, 10_000);

        // raised_amount = 50 → scaled 5000, biased 10000, fee = 1.
        let campaign_id = client.create_campaign(
            &creator,
            &beneficiary,
            &String::from_str(&env, "Boundary"),
            &50,
            &20_000,
            &token_client.address,
        );
        client.donate(&donor, &campaign_id, &50);

        let beneficiary_before = token_client.balance(&beneficiary);
        let admin_before = token_client.balance(&admin);
        let claimed = client.claim_funds(&beneficiary, &campaign_id);

        assert_eq!(claimed, 50);
        assert_eq!(token_client.balance(&admin) - admin_before, 1);
        assert_eq!(token_client.balance(&beneficiary) - beneficiary_before, 49);
    }

    #[test]
    fn claim_funds_fee_rounds_to_zero_for_tiny_settlement() {
        let (env, client, creator, beneficiary, donor, admin, token_client, _) = setup();
        set_timestamp(&env, 100);

        // raised_amount = 49 → scaled 4900, biased 9900, fee = 0.
        // Force the deadline path because raised < target.
        let campaign_id = client.create_campaign(
            &creator,
            &beneficiary,
            &String::from_str(&env, "Tiny"),
            &10_000,
            &500,
            &token_client.address,
        );
        client.donate(&donor, &campaign_id, &49);
        set_timestamp(&env, 600);

        let beneficiary_before = token_client.balance(&beneficiary);
        let admin_before = token_client.balance(&admin);
        let claimed = client.claim_funds(&beneficiary, &campaign_id);

        assert_eq!(claimed, 49);
        // Admin must receive nothing when the fee rounds to zero.
        assert_eq!(token_client.balance(&admin), admin_before);
        // Beneficiary receives the full raised amount.
        assert_eq!(token_client.balance(&beneficiary) - beneficiary_before, 49);
    }

    #[test]
    fn claim_funds_preserves_gross_amount_across_split() {
        // Property check: for any successful claim, fee + net == gross.
        let (env, client, creator, beneficiary, donor, admin, token_client, _) = setup();
        set_timestamp(&env, 10_000);

        let gross: i128 = 333_333;
        let campaign_id = client.create_campaign(
            &creator,
            &beneficiary,
            &String::from_str(&env, "Property"),
            &gross,
            &20_000,
            &token_client.address,
        );
        client.donate(&donor, &campaign_id, &gross);

        let beneficiary_before = token_client.balance(&beneficiary);
        let admin_before = token_client.balance(&admin);
        client.claim_funds(&beneficiary, &campaign_id);

        let fee_delta = token_client.balance(&admin) - admin_before;
        let net_delta = token_client.balance(&beneficiary) - beneficiary_before;
        assert_eq!(fee_delta + net_delta, gross);
        // 333_333 * 100 = 33_333_300; +5000 = 33_338_300; /10_000 = 3_333.
        assert_eq!(fee_delta, 3_333);
    }

    #[test]
    fn claim_funds_fails_when_admin_not_initialized() {
        // Build a contract without calling initialize, then attempt a claim.
        let env = Env::default();
        env.mock_all_auths();
        set_timestamp(&env, 1_000);

        let creator = Address::generate(&env);
        let beneficiary = Address::generate(&env);
        let donor = Address::generate(&env);
        let token_admin = Address::generate(&env);

        let token_id = env.register_stellar_asset_contract_v2(token_admin);
        let token_client = token::Client::new(&env, &token_id.address());
        let token_admin_client = token::StellarAssetClient::new(&env, &token_id.address());
        token_admin_client.mint(&donor, &1_000_000);

        let contract_id = env.register_contract(None, StellarGiveContract);
        let client = StellarGiveContractClient::new(&env, &contract_id);

        let campaign_id = client.create_campaign(
            &creator,
            &beneficiary,
            &String::from_str(&env, "Uninit"),
            &100_000,
            &5_000,
            &token_client.address,
        );
        client.donate(&donor, &campaign_id, &100_000);

        let result = client.try_claim_funds(&creator, &campaign_id);
        assert!(
            result.is_err(),
            "claim must fail when the platform admin has not been initialized"
        );
    }

    #[test]
    fn initialize_rejects_second_call() {
        // setup() already calls initialize once; a second call must fail.
        let (env, client, _creator, _beneficiary, _donor, _admin, _token_client, _) = setup();
        let _ = &env;

        let other_admin = Address::generate(&env);
        let result = client.try_initialize(&other_admin);
        assert!(
            result.is_err(),
            "initialize must reject a second call once the admin is set"
        );
    }
}
