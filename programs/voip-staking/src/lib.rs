use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token::{transfer, Token, TokenAccount, Transfer},
};

declare_id!("FbjoWqP1LBfWNSYv8JQoanCnu8XwNfb738Dqw5aHgEpP");

#[program]
pub mod voip_staking {
    const REWARD_PER_DAY: f64 = 0.004; // 0.4%
    const ONE_HOUR_IN_UNIX: i64 = 3600;
    const ONE_DAY_IN_UNIX: i64 = ONE_HOUR_IN_UNIX * 24;
    const ONE_HUNDRED_DAYS_IN_UNIX: i64 = 100 * ONE_DAY_IN_UNIX;
    const ONE_HUNDRED_AND_EIGHTY_DAYS_IN_UNIX: i64 = 180 * ONE_DAY_IN_UNIX;
    const THREE_HUNDRED_AND_SIXTY_DAYS_IN_UNIX: i64 = 360 * ONE_DAY_IN_UNIX;

    use super::*;

    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        ctx.accounts.state.admin = ctx.accounts.admin.key.clone();
        ctx.accounts.state.paused = false;

        Ok(())
    }

    pub fn stake(ctx: Context<Stake>, amount: u64, stake_time: StakeTime) -> Result<()> {
        // check not paused
        let _is_paused = ctx.accounts.state.paused;
        require!(!_is_paused, VIOPStakingError::ContractPaused);

        // update stake info
        let clock = Clock::get()?;
        let current_timestamp = clock.unix_timestamp;

        // update state
        ctx.accounts.stake_info.stake_balance += amount;
        ctx.accounts.stake_info.staked_at = current_timestamp;
        ctx.accounts.stake_info.last_claim_at = current_timestamp;
        ctx.accounts.stake_info.claim_balance = 0;
        ctx.accounts.stake_info.has_claimed_all = false;

        match stake_time {
            StakeTime::OneHundredDays => {
                ctx.accounts.stake_info.stake_time = current_timestamp + ONE_HUNDRED_DAYS_IN_UNIX;
            }
            StakeTime::OneHundredAndEightyDays => {
                ctx.accounts.stake_info.stake_time =
                    current_timestamp + ONE_HUNDRED_AND_EIGHTY_DAYS_IN_UNIX;
            }
            StakeTime::ThreeHundredAndSixtyDays => {
                ctx.accounts.stake_info.stake_time =
                    current_timestamp + THREE_HUNDRED_AND_SIXTY_DAYS_IN_UNIX;
            }
        }

        // transfer token
        let contract_ata = &mut ctx.accounts.contract_ata;
        let user_ata = &mut ctx.accounts.user_ata;
        let user = &mut ctx.accounts.user;
        let token_program = &ctx.accounts.token_program;

        let cpi_accounts = Transfer {
            from: user_ata.to_account_info(),
            to: contract_ata.to_account_info(),
            authority: user.to_account_info(),
        };
        let cpi_program = token_program.to_account_info();
        transfer(CpiContext::new(cpi_program, cpi_accounts), amount)?;

        Ok(())
    }

    pub fn claim(ctx: Context<Claim>) -> Result<()> {
        // check not paused
        let _is_paused = ctx.accounts.state.paused;
        require!(!_is_paused, VIOPStakingError::ContractPaused);

        let clock = Clock::get()?;
        let current_timestamp = clock.unix_timestamp;

        // check if user has claimed all
        if matches!(ctx.accounts.stake_info.has_claimed_all, true) {
            return  err!(VIOPStakingError::HasClaimedAllReward);
        }
        
        let stake_balance_as_ui_int = ctx.accounts.stake_info.stake_balance;
        let _staked_at = ctx.accounts.stake_info.staked_at;
        let stake_time = ctx.accounts.stake_info.stake_time as i64;
        let last_claim = ctx.accounts.stake_info.last_claim_at;

        // calculate stake period 
        let mut stake_period = current_timestamp - last_claim;
        if current_timestamp > stake_time {
            stake_period = stake_time - last_claim;
        }

        // calculate reward
        let stake_unix_slots = stake_period / ONE_DAY_IN_UNIX;
        let mut stake_reward = stake_unix_slots as f64 * REWARD_PER_DAY * stake_balance_as_ui_int as f64;

        let _one_hundred_days = ONE_HUNDRED_DAYS_IN_UNIX + _staked_at;
        let _one_hundred_and_eighty_days = ONE_HUNDRED_AND_EIGHTY_DAYS_IN_UNIX + _staked_at;
        let _three_hundred_and_sixty_days = THREE_HUNDRED_AND_SIXTY_DAYS_IN_UNIX + _staked_at;

        // check staking period is over        
        if current_timestamp >= stake_time {
            // check if user never claimed
            // give full reward if user never claimed until stake time is over
            if matches!(last_claim, _staked_at) {
                if matches!(stake_time, _one_hundred_days) {
                    stake_reward = stake_balance_as_ui_int as f64 * 0.56;   // 56%
                    ctx.accounts.stake_info.claim_balance = stake_reward as u64;
                }
                if matches!(stake_time, _one_hundred_and_eighty_days) {
                    stake_reward = stake_balance_as_ui_int as f64 * 1.08;   // 108%
                    ctx.accounts.stake_info.claim_balance = stake_reward as u64;
                }
                if matches!(stake_time, _three_hundred_and_sixty_days) {
                    stake_reward = stake_balance_as_ui_int as f64 * 1.8;    // 180%
                    ctx.accounts.stake_info.claim_balance = stake_reward as u64;
                }
            }
        }

        // update state
        ctx.accounts.stake_info.last_claim_at = current_timestamp;
        ctx.accounts.stake_info.claim_balance -= stake_reward as u64;
        if ctx.accounts.stake_info.last_claim_at >= stake_time {
            ctx.accounts.stake_info.has_claimed_all = true;
        }

        // signer seed
        let seeds = &["state".as_bytes(), &[ctx.bumps.state]];
        let signer = [&seeds[..]];

        // transfer token
        let amount = stake_reward as u64;
        let contract_ata = &mut ctx.accounts.contract_ata;
        let user_ata = &mut ctx.accounts.user_ata;
        let state = &mut ctx.accounts.state;
        let token_program = &ctx.accounts.token_program;

        let cpi_accounts = Transfer {
            from: contract_ata.to_account_info(),
            to: user_ata.to_account_info(),
            authority: state.to_account_info(),
        };
        let cpi_program = token_program.to_account_info();
        transfer(
            CpiContext::new_with_signer(cpi_program, cpi_accounts, &signer),
            amount,
        )?;

        Ok(())
    }

    pub fn withdraw(ctx: Context<Withdraw>) -> Result<()> {
        // check not paused
        let _is_paused = ctx.accounts.state.paused;
        require!(!_is_paused, VIOPStakingError::ContractPaused);

        // check user has claimed all rewards
        if matches!(ctx.accounts.stake_info.has_claimed_all, false) {
            return  err!(VIOPStakingError::HasNotClaimedAllReward);
        }

        let clock = Clock::get()?;
        let current_timestamp = clock.unix_timestamp;

        let stake_time = ctx.accounts.stake_info.stake_time as i64;

        // check staking period is over
        require_gte!(
            current_timestamp, stake_time, VIOPStakingError::StakePeriodNotOver
        );

        // signer seed
        let seeds = &["state".as_bytes(), &[ctx.bumps.state]];
        let signer = [&seeds[..]];

        // transfer token
        let amount = ctx.accounts.stake_info.stake_balance;
        let contract_ata = &mut ctx.accounts.contract_ata;
        let user_ata = &mut ctx.accounts.user_ata;
        let state = &mut ctx.accounts.state;
        let token_program = &ctx.accounts.token_program;

        // call transfer on token account
        let cpi_accounts = Transfer {
            from: contract_ata.to_account_info(),
            to: user_ata.to_account_info(),
            authority: state.to_account_info(),
        };
        let cpi_program = token_program.to_account_info();
        transfer(
            CpiContext::new_with_signer(cpi_program, cpi_accounts, &signer),
            amount,
        )?;

        Ok(())
    }

    pub fn pause(ctx: Context<Pause>) -> Result<()> {
        let _is_paused = ctx.accounts.state.paused;
        let _signer = ctx.accounts.admin.key.clone();
        let admin = ctx.accounts.state.admin;

        require!(!_is_paused, VIOPStakingError::ContractPaused);
        require!(matches!(admin, _signer), VIOPStakingError::Unauthorized);

        ctx.accounts.state.paused = true;

        msg!("VOIP Staking Contract Paused");

        Ok(())
    }

    pub fn un_pause(ctx: Context<UnPause>) -> Result<()> {
        let _is_paused = ctx.accounts.state.paused;
        let _signer = ctx.accounts.admin.key.clone();
        let admin = ctx.accounts.state.admin;

        require!(_is_paused, VIOPStakingError::ContractNotPaused);
        require!(matches!(admin, _signer), VIOPStakingError::Unauthorized);

        ctx.accounts.state.paused = false;

        msg!("VOIP Migration Contract Unpaused");

        Ok(())
    }
}

// Instruction accounts
#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(init,
        seeds = [b"state"],
        bump,
        payer = admin,
        space = 8 + State::LEN
    )]
    pub state: Account<'info, State>,

    #[account(mut)]
    pub admin: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct Stake<'info> {
    #[account(
        init_if_needed,
        seeds = [b"stake_info", user.key().as_ref()],
        bump,
        payer = user,
        space = 8 + StakeInfo::LEN
    )]
    pub stake_info: Account<'info, StakeInfo>,

    #[account(mut)]
    pub state: Account<'info, State>,

    #[account(mut)]
    pub user_ata: Account<'info, TokenAccount>,
    #[account(mut)]
    pub contract_ata: Account<'info, TokenAccount>,

    #[account(mut)]
    pub user: Signer<'info>,

    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
    pub associated_token_program: Program<'info, AssociatedToken>,
}

#[derive(Accounts)]
pub struct Claim<'info> {
    #[account(mut)]
    pub stake_info: Account<'info, StakeInfo>,

    #[account(
        init_if_needed,
        seeds = [b"state"],
        bump,
        payer = user,
        space = 8 + State::LEN
    )]
    pub state: Account<'info, State>,

    #[account(mut)]
    pub user_ata: Account<'info, TokenAccount>,
    #[account(mut)]
    pub contract_ata: Account<'info, TokenAccount>,

    #[account(mut)]
    pub user: Signer<'info>,

    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
    pub associated_token_program: Program<'info, AssociatedToken>,
}

#[derive(Accounts)]
pub struct Withdraw<'info> {
    #[account(mut)]
    pub stake_info: Account<'info, StakeInfo>,

    #[account(
        init_if_needed,
        seeds = [b"state"],
        bump,
        payer = user,
        space = 8 + State::LEN
    )]
    pub state: Account<'info, State>,

    #[account(mut)]
    pub user_ata: Account<'info, TokenAccount>,
    #[account(mut)]
    pub contract_ata: Account<'info, TokenAccount>,

    #[account(mut)]
    pub user: Signer<'info>,

    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
    pub associated_token_program: Program<'info, AssociatedToken>,
}

#[derive(Accounts)]
pub struct Pause<'info> {
    #[account(mut)]
    pub state: Account<'info, State>,
    #[account(mut)]
    pub admin: Signer<'info>,
}

#[derive(Accounts)]
pub struct UnPause<'info> {
    #[account(mut)]
    pub state: Account<'info, State>,
    #[account(mut)]
    pub admin: Signer<'info>,
}

// Globl state

#[account]
pub struct State {
    admin: Pubkey,
    paused: bool,
}

impl State {
    const LEN: usize = 32 + 4;
}

// user state
#[account]
pub struct StakeInfo {
    stake_balance: u64,
    staked_at: i64,
    last_claim_at: i64,
    stake_time: i64,
    claim_balance: u64,
    has_claimed_all: bool,
}

impl StakeInfo {
    const LEN: usize = 16 + 16 + 16;
}

#[derive(Debug, Clone, Copy, AnchorDeserialize, AnchorSerialize)]
pub enum StakeTime {
    OneHundredDays,
    OneHundredAndEightyDays,
    ThreeHundredAndSixtyDays,
}

#[error_code]
pub enum VIOPStakingError {
    #[msg("Contract is paused")]
    ContractPaused,
    #[msg("Contract is not paused")]
    ContractNotPaused,
    #[msg("Unauthorized access")]
    Unauthorized,
    #[msg("Has claimed all reward")]
    HasClaimedAllReward,
    #[msg("Has claimed all reward")]
    HasNotClaimedAllReward,
    #[msg("Staking period not over")]
    StakePeriodNotOver
}
