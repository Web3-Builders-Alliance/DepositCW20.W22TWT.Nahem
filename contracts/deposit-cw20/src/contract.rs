#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    coin, from_binary, to_binary, BankMsg, Binary, Deps, DepsMut, Env, MessageInfo, Order,
    Response, StdResult, Uint128,
};
use cw2::set_contract_version;
use cw20::Cw20ReceiveMsg;
// use cw2::set_contract_version;

use crate::error::ContractError;
use crate::msg::{
    Cw20DepositResponse, Cw20HookMsg, DepositResponse, ExecuteMsg, InstantiateMsg, QueryMsg,
};
use crate::state::{Cw20Deposits, Deposits, CW20_DEPOSITS, DEPOSITS};

const CONTRACT_NAME: &str = "deposit-cw20-example";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    _msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    Ok(Response::default())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::Deposit {} => execute_deposit(deps, info),
        ExecuteMsg::Withdraw { amount, denom } => execute_withdraw(deps, info, amount, denom),
        ExecuteMsg::Receive(cw20_msg) => receive_cw20(deps, env, info, cw20_msg),
        ExecuteMsg::WithdrawCw20 { address, amount } => {
            execute_cw20_withdraw(deps, env, info, address, amount)
        }
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Deposits { address } => to_binary(&query_deposits(deps, address)?),
        QueryMsg::Cw20Deposits { address } => to_binary(&query_cw20_deposits(deps, address)?),
    }
}

pub fn receive_cw20(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    cw20_msg: Cw20ReceiveMsg,
) -> Result<Response, ContractError> {
    match from_binary(&cw20_msg.msg) {
        Ok(Cw20HookMsg::Deposit {}) => {
            execute_cw20_deposit(deps, env, info, cw20_msg.sender, cw20_msg.amount)
        }
        _ => Err(ContractError::CustomError {
            val: "Invalid Cw20HookMsg".to_string(),
        }),
    }
}

pub fn execute_deposit(deps: DepsMut, info: MessageInfo) -> Result<Response, ContractError> {
    let sender = info.sender.clone().into_string();

    let d_coins = info.funds[0].clone();

    //check to see if deposit exists
    match DEPOSITS.load(deps.storage, (&sender, d_coins.denom.as_str())) {
        Ok(mut deposit) => {
            //add coins to their account
            deposit.coins.amount += d_coins.amount;
            deposit.coins.amount = deposit.coins.amount.checked_add(d_coins.amount).unwrap();
            deposit.count = deposit.count.checked_add(1).unwrap();
            DEPOSITS
                .save(deps.storage, (&sender, d_coins.denom.as_str()), &deposit)
                .unwrap();
        }
        Err(_) => {
            //user does not exist, add them.
            let deposit = Deposits {
                count: 1,
                owner: info.sender,
                coins: d_coins.clone(),
            };
            DEPOSITS
                .save(deps.storage, (&sender, d_coins.denom.as_str()), &deposit)
                .unwrap();
        }
    }
    Ok(Response::new()
        .add_attribute("execute", "deposit")
        .add_attribute("denom", d_coins.denom)
        .add_attribute("amount", d_coins.amount))
}

pub fn execute_withdraw(
    deps: DepsMut,
    info: MessageInfo,
    amount: u128,
    denom: String,
) -> Result<Response, ContractError> {
    let sender = info.sender.clone().into_string();

    let mut deposit = DEPOSITS
        .load(deps.storage, (&sender, denom.as_str()))
        .unwrap();
    deposit.coins.amount = deposit
        .coins
        .amount
        .checked_sub(Uint128::from(amount))
        .unwrap();
    deposit.count = deposit.count.checked_sub(1).unwrap();
    DEPOSITS
        .save(deps.storage, (&sender, denom.as_str()), &deposit)
        .unwrap();

    let msg = BankMsg::Send {
        to_address: sender.clone(),
        amount: vec![coin(amount, denom.clone())],
    };

    Ok(Response::new()
        .add_attribute("execute", "withdraw")
        .add_attribute("denom", denom)
        .add_attribute("amount", amount.to_string())
        .add_message(msg))
}

pub fn execute_cw20_deposit(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    owner: String,
    amount: Uint128,
) -> Result<Response, ContractError> {
    // not sure if this check is necesary
    if amount == Uint128::zero() {
        return Err(ContractError::NotEnoughTokensSent {});
    }

    let cw20_contract_address = info.sender.clone().to_string();

    let mut cw20_deposits = CW20_DEPOSITS
        .may_load(deps.storage, (&owner, &cw20_contract_address))?
        .unwrap_or(Cw20Deposits {
            count: 0,
            owner: owner.clone(),
            contract: cw20_contract_address.clone(),
            amount: amount,
            lockdown: env.block.height,
        });

    if cw20_deposits.lockdown <= env.block.height {
        cw20_deposits.lockdown = env.block.height + 20;
    } else {
        cw20_deposits.lockdown = cw20_deposits.lockdown - env.block.height + 20;
    }

    cw20_deposits.count += 1;

    CW20_DEPOSITS.save(
        deps.storage,
        (&owner, &cw20_contract_address),
        &cw20_deposits,
    )?;

    Ok(Response::new()
        .add_attribute("execute", "cw20_deposit")
        .add_attribute("token_address", cw20_contract_address)
        .add_attribute("amount", amount))
}

//use WasmMsg::Execute instead of BankMsg::Send
pub fn execute_cw20_withdraw(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    contract: String,
    amount: Uint128,
) -> Result<Response, ContractError> {
    let sender = info.sender.clone().into_string();
    match CW20_DEPOSITS.load(deps.storage, (&sender, &contract)) {
        Ok(mut deposit) => {
            if amount < deposit.amount {
                return Err(ContractError::NotEnoughTokensToWithdraw {});
            }
            if deposit.lockdown < env.block.height {
                return Err(ContractError::LockdownIsNotOver {
                    blocks: (deposit.lockdown - env.block.height).to_string(),
                });
            }
            deposit.amount -= amount;
            CW20_DEPOSITS.save(deps.storage, (&sender, &contract), &deposit)?;
            Ok(Response::new()
                .add_attribute("execute", "cw20_withdraw")
                .add_attribute("token_address", contract)
                .add_attribute("sender", sender)
                .add_attribute("withdrawn_amount", amount))
        }
        Err(_) => {
            return Err(ContractError::NoCw20ToWithdraw {});
        }
    }
}

pub fn query_deposits(deps: Deps, address: String) -> StdResult<DepositResponse> {
    let res: StdResult<Vec<_>> = DEPOSITS
        .prefix(&address)
        .range(deps.storage, None, None, Order::Ascending)
        .collect();
    let deposits = res?;
    Ok(DepositResponse { deposits })
}

fn query_cw20_deposits(deps: Deps, address: String) -> StdResult<Cw20DepositResponse> {
    let res: StdResult<Vec<_>> = CW20_DEPOSITS
        .prefix(&address)
        .range(deps.storage, None, None, Order::Ascending)
        .collect();
    let deposits = res?;
    Ok(Cw20DepositResponse { deposits })
}
