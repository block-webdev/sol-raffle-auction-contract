use anchor_lang::prelude::*;
use chainlink_solana as chainlink;
use anchor_spl::{
    associated_token::AssociatedToken,
    token::{self, Mint, Token, TokenAccount, Transfer},
};
use std::mem::size_of;

pub mod account;
pub mod constants;
pub mod errors;

use account::*;
use constants::*;
use errors::*;

declare_id!("LrE6QxmkADt25V4H3ULGt8MZMNxTXYNs3fxQXQMgyX7");

#[program]
pub mod raffle {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        ctx.accounts.global_state.admin = ctx.accounts.admin.key();
        ctx.accounts.global_state.zzz_mint = ctx.accounts.zzz_mint.key();

        Ok(())
    }

    #[access_control(is_admin(&ctx.accounts.global_state, &ctx.accounts.admin))]
    pub fn create_raffle(ctx: Context<CreateRaffle>, raffle_id: u32, ticket_count: u32, ticket_price: u64, start_time: i64, end_time: i64, project_name: String, project_description: String, discord_link: String, twitter_link: String, wl_spot: u32, image: String) -> Result<()>  {
        ctx.accounts.raffle.raffle_id = raffle_id;
        ctx.accounts.raffle.ticket_count = ticket_count;
        ctx.accounts.raffle.ticket_price = ticket_price;
        ctx.accounts.raffle.start_time = start_time;
        ctx.accounts.raffle.end_time = end_time;
        ctx.accounts.raffle.reward_mint = ctx.accounts.reward_mint.key();

        ctx.accounts.raffle.project_name = project_name;
        ctx.accounts.raffle.project_description = project_description;
        ctx.accounts.raffle.discord_link = discord_link;
        ctx.accounts.raffle.twitter_link = twitter_link;
        ctx.accounts.raffle.wl_spot = wl_spot;
        ctx.accounts.raffle.image = image;

        ctx.accounts.global_state.raffle_count += 1;

        // Transfer reward tokens into the vault.
        let cpi_ctx = CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            anchor_spl::token::Transfer {
                from: ctx.accounts.source_account.to_account_info(),
                to: ctx.accounts.reward_vault.to_account_info(),
                authority: ctx.accounts.admin.to_account_info(),
            },
        );

        anchor_spl::token::transfer(cpi_ctx, 1)?;

        Ok(())
    }

    #[access_control(is_admin(&ctx.accounts.global_state, &ctx.accounts.admin))]
    pub fn set_raffle(ctx: Context<SetRaffle>, _raffle_id: u32, ticket_count: u32, ticket_price: u64, start_time: i64, end_time: i64, reward_mint: Pubkey, project_name: String, project_description: String, discord_link: String, twitter_link: String, wl_spot: u32, image: String) -> Result<()>  {

        ctx.accounts.raffle.ticket_count = ticket_count;
        ctx.accounts.raffle.ticket_price = ticket_price;
        ctx.accounts.raffle.start_time = start_time;
        ctx.accounts.raffle.end_time = end_time;
        ctx.accounts.raffle.reward_mint = reward_mint;

        ctx.accounts.raffle.project_name = project_name;
        ctx.accounts.raffle.project_description = project_description;
        ctx.accounts.raffle.discord_link = discord_link;
        ctx.accounts.raffle.twitter_link = twitter_link;
        ctx.accounts.raffle.wl_spot = wl_spot;
        ctx.accounts.raffle.image = image;

        Ok(())
    }

    #[access_control(is_admin(&ctx.accounts.global_state, &ctx.accounts.admin))]
    pub fn delete_raffle(ctx: Context<DeleteRaffle>) -> Result<()>  {

        ctx.accounts.global_state.raffle_count -= 1;

        Ok(())
    }

    pub fn buy_ticket(ctx: Context<BuyTicket>, raffle_id: u32, count: u32) -> Result<()>  {
        require!(
            ctx.accounts.raffle.ticket_count >= ctx.accounts.raffle.sold_tickets + count,
            RaffleError::InsufficientTickets
        );

        // pay to place a bid
        let amount = ctx.accounts.raffle.ticket_price * count as u64;
        let cpi_ctx = CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            anchor_spl::token::Transfer {
                from: ctx.accounts.source_account.to_account_info(),
                to: ctx.accounts.zzz_vault.to_account_info(),
                authority: ctx.accounts.user.to_account_info(),
            },
        );

        anchor_spl::token::transfer(cpi_ctx, amount)?;

        // set raffle info
        ctx.accounts.buyer_state.raffle_id = raffle_id;
        ctx.accounts.buyer_state.buyer = ctx.accounts.user.key();
        ctx.accounts.buyer_state.sold_time = Clock::get()?.unix_timestamp;

        ctx.accounts.buyer_state.ticket_num_start = ctx.accounts.raffle.sold_tickets + 1;
        ctx.accounts.buyer_state.ticket_num_end = ctx.accounts.raffle.sold_tickets + count;

        ctx.accounts.raffle.sold_tickets += count;

        Ok(())
    }

    #[access_control(is_admin(&ctx.accounts.global_state, &ctx.accounts.admin))]
    pub fn finish_raffle(ctx: Context<FinishRaffle>, _raffle_id : u32) -> Result<()>  {
        let pyth_price_info = &ctx.accounts.pyth_account;
        let pyth_price_data = &pyth_price_info.try_borrow_data()?;
        let pyth_price = pyth_client::cast::<pyth_client::Price>(pyth_price_data);
        let agg_price = pyth_price.agg.price as u64;

        let ctime = Clock::get().unwrap();
        let c = agg_price + ctime.unix_timestamp as u64;
        let r = (c % ctx.accounts.raffle.ticket_count as u64) as u32;

        ctx.accounts.raffle.win_ticket_num = r + 1;
        ctx.accounts.raffle.closed = 1;

        Ok(())
    }

    pub fn gen_wl_winners(ctx: Context<GenWlWinners>, ticket_count: u32) -> Result<u32>  {
        let pyth_price_info = &ctx.accounts.pyth_account;
        let pyth_price_data = &pyth_price_info.try_borrow_data()?;
        let pyth_price = pyth_client::cast::<pyth_client::Price>(pyth_price_data);
        let agg_price = pyth_price.agg.price as u64;

        let ctime = Clock::get().unwrap();
        let c = agg_price + ctime.unix_timestamp as u64;
        let r = (c % ticket_count as u64) as u32;

        Ok(r)
    }

    pub fn create_auction(ctx: Context<CreateAuction>, auction_id: u32, seller: Pubkey, nft_mint: Pubkey, min_bid_amount: u32, start_time: i64, end_time: i64, start_price: u64) -> Result<()>  {

        ctx.accounts.auction.auction_id = auction_id;
        ctx.accounts.auction.seller = seller;
        ctx.accounts.auction.nft_mint = nft_mint;
        ctx.accounts.auction.min_bid_amount = min_bid_amount;
        ctx.accounts.auction.start_time = start_time;
        ctx.accounts.auction.end_time = end_time;
        ctx.accounts.auction.bid_mint = ctx.accounts.global_state.zzz_mint;
        ctx.accounts.auction.price = start_price;

        ctx.accounts.global_state.auction_count += 1;

        Ok(())
    }

    pub fn delete_auction(ctx: Context<DeleteAuction>) -> Result<()>  {

        ctx.accounts.global_state.auction_count -= 1;

        Ok(())
    }

    pub fn bid(ctx: Context<Bid>, _auction_id: u32, price: u64) -> Result<()>  {
        let auction = &mut ctx.accounts.auction;

        // check bid price
        if price <= auction.price {
            return Err(AuctionError::BidPirceTooLow.into());
        }

        // if refund_receiver exist, return money back to it
        if auction.refund_receiver != Pubkey::default() {
            let (_pool_account_seed, _pool_account_bump) = Pubkey::find_program_address(&[&(GLOBAL_STATE_SEED.as_bytes())], ctx.program_id);
            let seeds = &[GLOBAL_STATE_SEED.as_bytes(), &[_pool_account_bump]];
            let signer = &[&seeds[..]];

            let cpi_accounts = Transfer {
                from: ctx.accounts.vault.to_account_info(),
                to: ctx.accounts.ori_refund_receiver.to_account_info(),
                authority: ctx.accounts.global_state.to_account_info(),
            };
            let token_program = ctx.accounts.token_program.to_account_info();
            let cpi_ctx = CpiContext::new_with_signer(token_program, cpi_accounts, signer);
            token::transfer(cpi_ctx, auction.price)?;
        }

        // transfer bid pirce to custodial currency holder
        let cpi_accounts = Transfer {
            from: ctx.accounts.from.to_account_info(),
            to: ctx.accounts.vault.to_account_info(),
            authority: ctx.accounts.user.to_account_info(),
        };

        let token_program = ctx.accounts.token_program.to_account_info();
        let cpi_ctx = CpiContext::new(token_program, cpi_accounts);
        token::transfer(cpi_ctx, price)?;

        // update auction info
        let auction = &mut ctx.accounts.auction;
        auction.bidder = *ctx.accounts.user.key;
        auction.refund_receiver = *ctx.accounts.from.to_account_info().key;
        auction.price = price;

        Ok(())
    }

    // pub fn cancel_bid(ctx: Context<CancelBid>) -> Result<()> {

    //     Ok(())
    // }

    pub fn finish_auction(ctx: Context<FinishAuction>) -> Result<()> {
        let auction = &mut ctx.accounts.auction;

        let (_pool_account_seed, _pool_account_bump) = Pubkey::find_program_address(&[&(GLOBAL_STATE_SEED.as_bytes())], ctx.program_id);
        let seeds = &[GLOBAL_STATE_SEED.as_bytes(), &[_pool_account_bump]];
        let signer = &[&seeds[..]];

        // item ownership transfer
        let cpi_accounts = Transfer {
            from: ctx.accounts.nft_holder.to_account_info(),
            to: ctx.accounts.nft_receiver.to_account_info(),
            authority: ctx.accounts.global_state.to_account_info(),
        };
        let token_program = ctx.accounts.token_program.to_account_info();
        let cpi_ctx = CpiContext::new_with_signer(token_program, cpi_accounts, signer);
        token::transfer(cpi_ctx, ctx.accounts.nft_holder.amount)?;

        auction.closed = 1;

        Ok(())
    }

    pub fn claim_rewards(ctx: Context<ClaimRewards>, raffle_id : u32) -> Result<()> {
        // Transfer rewards from the pool reward vaults to user reward vaults.
        let vault_amount = ctx.accounts.reward_vault.amount;
        if vault_amount > 0 {
            let (_pool_account_seed, _bump) =
                Pubkey::find_program_address(&[GLOBAL_STATE_SEED.as_bytes()], ctx.program_id);

            let pool_seeds = &[GLOBAL_STATE_SEED.as_bytes(), &[_bump]];
            let signer = &[&pool_seeds[..]];

            let token_program = ctx.accounts.token_program.to_account_info().clone();
            let token_accounts = anchor_spl::token::Transfer {
                from: ctx.accounts.reward_vault.to_account_info().clone(),
                to: ctx.accounts.reward_to_account.to_account_info().clone(),
                authority: ctx.accounts.global_state.to_account_info().clone(),
            };
            let cpi_ctx = CpiContext::new(token_program, token_accounts);
            let reward = 1; // nft
            msg!(
                "Calling the token program to transfer reward {} to the user",
                reward
            );
            anchor_spl::token::transfer(cpi_ctx.with_signer(signer), reward)?;

            ctx.accounts.raffle.claimed = 1;
        }

        Ok(())
    }

    pub fn deposit_reward(ctx: Context<DepositReward>, amount: u64) -> Result<()>  {
        let cpi_ctx = CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            anchor_spl::token::Transfer {
                from: ctx.accounts.source_account.to_account_info(),
                to: ctx.accounts.dest_account.to_account_info(),
                authority: ctx.accounts.owner.to_account_info(),
            },
        );

        anchor_spl::token::transfer(cpi_ctx, amount)?;

        Ok(())
    }

}

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(
        init,
        payer = admin,
        seeds = [GLOBAL_STATE_SEED.as_bytes()],
        bump,
        space = 8 + size_of::<GlobalState>(),
    )]
    pub global_state: Account<'info, GlobalState>,

    // mint
    pub zzz_mint: Account<'info, Mint>,

    // reward vault that holds the reward mint for distribution
    #[account(
        init,
        token::mint = zzz_mint,
        token::authority = global_state,
        seeds = [ ZZZ_VAULT_SEED.as_bytes(), zzz_mint.key().as_ref() ],
        bump,
        payer = admin,
    )]
    pub zzz_vault: Box<Account<'info, TokenAccount>>,

    pub system_program: Program<'info, System>,

    // The rent sysvar
    pub rent: Sysvar<'info, Rent>,

    // token program
    /// CHECK: This is not dangerous because we don't read or write from this account
    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
#[instruction(raffle_id : u32)]
pub struct CreateRaffle<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(
        mut,
        constraint = global_state.admin == admin.key(),
    )]
    pub global_state: Account<'info, GlobalState>,

    #[account(
        init,
        payer = admin,
        seeds = [RAFFLE_SEED.as_bytes(), &raffle_id.to_le_bytes()],
        bump,
        space = 8 + size_of::<Raffle>(),
    )]
    pub raffle: Account<'info, Raffle>,

    // reward mint
    pub reward_mint: Account<'info, Mint>,

    // reward vault that holds the reward mint for distribution
    #[account(
        init_if_needed,
        token::mint = reward_mint,
        token::authority = global_state,
        seeds = [ REWARD_VAULT_SEED.as_bytes(), reward_mint.key().as_ref() ],
        bump,
        payer = admin,
    )]
    pub reward_vault: Box<Account<'info, TokenAccount>>,

    // source account
    #[account(mut)]
    source_account: Account<'info, TokenAccount>,

    // The rent sysvar
    pub rent: Sysvar<'info, Rent>,
    // system program
    /// CHECK: This is not dangerous because we don't read or write from this account
    pub system_program: Program<'info, System>,

    // token program
    /// CHECK: This is not dangerous because we don't read or write from this account
    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
#[instruction(raffle_id : u32)]
pub struct SetRaffle<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(
        mut,
        constraint = global_state.admin == admin.key(),
    )]
    pub global_state: Account<'info, GlobalState>,

    #[account(
        mut,
        seeds = [RAFFLE_SEED.as_bytes(), &raffle_id.to_le_bytes()],
        bump,
    )]
    pub raffle: Account<'info, Raffle>,
}

#[derive(Accounts)]
#[instruction(raffle_id : u32)]
pub struct DeleteRaffle<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(
        mut,
        constraint = global_state.admin == admin.key(),
    )]
    pub global_state: Account<'info, GlobalState>,

    #[account(
        mut,
        seeds = [RAFFLE_SEED.as_bytes(), &raffle_id.to_le_bytes()],
        bump,
        close = admin
    )]
    pub raffle: Account<'info, Raffle>,
}

#[derive(Accounts)]
#[instruction(raffle_id : u32)]
pub struct FinishRaffle<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(
        mut,
        constraint = global_state.admin == admin.key(),
    )]
    pub global_state: Account<'info, GlobalState>,

    #[account(
        mut,
        seeds = [RAFFLE_SEED.as_bytes(), &raffle_id.to_le_bytes()],
        bump,
    )]
    pub raffle: Account<'info, Raffle>,

    /// CHECK: We're reading data from this chainlink feed account
    pub pyth_account: AccountInfo<'info>,
}


#[derive(Accounts)]
pub struct GenWlWinners<'info> {
    /// CHECK: We're reading data from this chainlink feed account
    pub pyth_account: AccountInfo<'info>,
}

#[derive(Accounts)]
#[instruction(raffle_id: u32, count: u32)]
pub struct BuyTicket<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(
        mut,
        seeds = [GLOBAL_STATE_SEED.as_bytes()],
        bump,
    )]
    pub global_state: Account<'info, GlobalState>,

    #[account(
        mut,
        seeds = [RAFFLE_SEED.as_bytes(), &raffle_id.to_le_bytes()],
        bump,
    )]
    pub raffle: Account<'info, Raffle>,

    #[account(
        init,
        payer = user,
        seeds = [ BUYER_STATE_SEED.as_bytes(), user.key().as_ref(), &raffle_id.to_le_bytes(), &(raffle.sold_tickets + 1).to_le_bytes() ],
        bump,
        space = 8 + size_of::<BuyerState>(),
    )]
    pub buyer_state: Account<'info, BuyerState>,

    #[account(address = global_state.zzz_mint)]
    pub zzz_mint: Box<Account<'info, Mint>>,

    #[account(
        mut,
        token::mint = zzz_mint,
        token::authority = global_state,
    )]
    zzz_vault: Box<Account<'info, TokenAccount>>,

    // funder account
    #[account(mut)]
    source_account: Account<'info, TokenAccount>,

    pub system_program: Program<'info, System>,

    // The Token Program
    token_program: Program<'info, Token>,
}


#[derive(Accounts)]
#[instruction(raffle_id : u32)]
pub struct ClaimRewards<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(
        mut,
        seeds = [RAFFLE_SEED.as_bytes(), &raffle_id.to_le_bytes()],
        bump,
        constraint = raffle.claimed != 1,
    )]
    pub raffle: Account<'info, Raffle>,

    #[account(mut,
        constraint = user.key() == buyer_state.buyer,
        constraint = buyer_state.ticket_num_start <= raffle.win_ticket_num && raffle.win_ticket_num <= buyer_state.ticket_num_end)]
    pub buyer_state: Account<'info, BuyerState>,

    #[account(mut)]
    pub global_state : Account<'info, GlobalState>,

    // reward mint
    #[account(address = raffle.reward_mint)]
    reward_mint: Account<'info, Mint>,

    #[account(
        mut,
        seeds = [ REWARD_VAULT_SEED.as_bytes(), reward_mint.key().as_ref() ],
        bump,
        token::mint = reward_mint,
        token::authority = global_state,
    )]
    pub reward_vault: Box<Account<'info, TokenAccount>>,

    // send reward to user reward vault
    #[account(
        init_if_needed,
        payer = user,
        associated_token::mint = reward_mint,
        associated_token::authority = user
      )]
    pub reward_to_account: Box<Account<'info, TokenAccount>>,

    // The Token Program
    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
pub struct DepositReward<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,

    #[account(mut,owner=spl_token::id())]
    /// CHECK: unsafe
    pub source_account : AccountInfo<'info>,

    #[account(mut,owner=spl_token::id())]
    /// CHECK: unsafe
    pub dest_account : AccountInfo<'info>,

    pub token_program: Program<'info, Token>,
}


#[derive(Accounts)]
#[instruction(auction_id : u32)]
pub struct CreateAuction<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(
        mut,
        constraint = global_state.admin == admin.key(),
    )]
    pub global_state: Account<'info, GlobalState>,

    #[account(
        init,
        payer = admin,
        seeds = [AUCTION_SEED.as_bytes(), &auction_id.to_le_bytes()],
        bump,
        space = 8 + size_of::<Auction>(),
    )]
    pub auction: Account<'info, Auction>,

    pub system_program: Program<'info, System>,
    
    // token program
    /// CHECK: This is not dangerous because we don't read or write from this account
    pub token_program: Program<'info, Token>,
    
    // The rent sysvar
    pub rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
#[instruction(auction_id : u32)]
pub struct Bid<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(mut)]
    pub global_state: Account<'info, GlobalState>,

    #[account(
        mut,
        seeds = [AUCTION_SEED.as_bytes(), &auction_id.to_le_bytes()],
        bump,
        constraint = auction.closed == 0
    )]
    pub auction: Account<'info, Auction>,

    #[account(
        mut,
        token::mint = bid_mint,
        token::authority = global_state,
    )]
    vault: Box<Account<'info, TokenAccount>>,

    #[account(address = auction.bid_mint)]
    pub bid_mint: Box<Account<'info, Mint>>,

    // funder account
    #[account(mut)]
    from: Account<'info, TokenAccount>,

    // funder account
    #[account(
        mut,
        constraint = ori_refund_receiver.key() == auction.refund_receiver
    )]
    ori_refund_receiver: Account<'info, TokenAccount>,

    // token program
    /// CHECK: This is not dangerous because we don't read or write from this account
    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
#[instruction(auction_id : u32)]
pub struct FinishAuction<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(mut)]
    pub global_state: Account<'info, GlobalState>,

    #[account(
        mut,
        seeds = [AUCTION_SEED.as_bytes(), &auction_id.to_le_bytes()],
        bump,
    )]
    pub auction: Account<'info, Auction>,

    #[account(
        mut,
        token::mint = nft_mint,
        token::authority = global_state,
    )]
    nft_holder: Box<Account<'info, TokenAccount>>,

    #[account(address = auction.bid_mint)]
    pub nft_mint: Box<Account<'info, Mint>>,

    // funder account
    #[account(
        mut,
        constraint = nft_receiver.owner == auction.bidder
    )]
    nft_receiver: Account<'info, TokenAccount>,

    // token program
    /// CHECK: This is not dangerous because we don't read or write from this account
    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
#[instruction(auction_id : u32)]
pub struct DeleteAuction<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(
        mut,
        constraint = global_state.admin == admin.key(),
    )]
    pub global_state: Account<'info, GlobalState>,

    #[account(
        mut,
        seeds = [AUCTION_SEED.as_bytes(), &auction_id.to_le_bytes()],
        bump,
        close = admin
    )]
    pub auction: Account<'info, Auction>,
}

fn is_admin<'info> (
    global_state: &Account<'info, GlobalState>,
    signer: &Signer<'info>,
) -> Result<()> {
    require!(global_state.admin.eq(&signer.key()), RaffleError::InvalidAdmin);

    Ok(())
}
