#[cfg(test)]
mod tests {
    use crate::helpers::DepositContract;
    use crate::msg::{
        Cw20DepositResponse, Cw20HookMsg, DepositResponse, ExecuteMsg, InstantiateMsg, QueryMsg,
    };
    use crate::state::Cw20Deposits;
    use cosmwasm_std::{coin, to_binary, Addr, Coin, Empty, Uint128};
    use cw20::{BalanceResponse, Cw20Coin, Cw20Contract};
    use cw20_base::msg::ExecuteMsg as Cw20ExecuteMsg;
    use cw20_base::msg::InstantiateMsg as Cw20InstantiateMsg;
    use cw20_base::msg::QueryMsg as Cw20QueryMsg;
    use cw_multi_test::{App, AppBuilder, Contract, ContractWrapper, Executor};

    use cw20_example::{self};

    pub fn contract_deposit_cw20() -> Box<dyn Contract<Empty>> {
        let contract = ContractWrapper::new(
            crate::contract::execute,
            crate::contract::instantiate,
            crate::contract::query,
        );
        Box::new(contract)
    }

    pub fn contract_cw20() -> Box<dyn Contract<Empty>> {
        let contract = ContractWrapper::new(
            cw20_example::contract::execute,
            cw20_example::contract::instantiate,
            cw20_example::contract::query,
        );
        Box::new(contract)
    }

    const USER: &str = "juno10c3slrqx3369mfsr9670au22zvq082jaej8ve4";
    const ADMIN: &str = "ADMIN";
    const NATIVE_DENOM: &str = "denom";

    fn mock_app() -> App {
        AppBuilder::new().build(|router, _, storage| {
            router
                .bank
                .init_balance(
                    storage,
                    &Addr::unchecked(USER),
                    vec![Coin {
                        denom: NATIVE_DENOM.to_string(),
                        amount: Uint128::new(1000),
                    }],
                )
                .unwrap();
        })
    }

    fn store_code() -> (App, u64, u64) {
        let mut app = mock_app();
        let deposit_id = app.store_code(contract_deposit_cw20());
        let cw20_id = app.store_code(contract_cw20());
        (app, deposit_id, cw20_id)
    }

    fn deposit_instantiate(app: &mut App, deposit_id: u64) -> DepositContract {
        let deposit_contract_addr = app
            .instantiate_contract(
                deposit_id,
                Addr::unchecked(ADMIN),
                &InstantiateMsg {},
                &[],
                "deposit_cw20",
                None,
            )
            .unwrap();
        DepositContract(deposit_contract_addr)
    }

    fn cw_20_instantiate(app: &mut App, cw20_id: u64) -> Cw20Contract {
        let coin = Cw20Coin {
            address: USER.to_string(),
            amount: Uint128::from(10000u64),
        };
        let msg: Cw20InstantiateMsg = Cw20InstantiateMsg {
            decimals: 10,
            name: "Token".to_string(),
            symbol: "TKN".to_string(),
            initial_balances: vec![coin],
            marketing: None,
            mint: None,
        };
        let cw20_contract_address = app
            .instantiate_contract(
                cw20_id,
                Addr::unchecked(ADMIN),
                &msg,
                &[],
                "cw20-example",
                None,
            )
            .unwrap();
        Cw20Contract(cw20_contract_address)
    }

    fn get_deposits(app: &App, deposit_contract: &DepositContract) -> DepositResponse {
        app.wrap()
            .query_wasm_smart(
                deposit_contract.addr(),
                &QueryMsg::Deposits {
                    address: USER.to_string(),
                },
            )
            .unwrap()
    }

    fn get_balance(app: &App, user: String, denom: String) -> Coin {
        app.wrap().query_balance(user, denom).unwrap()
    }

    fn get_cw20_deposits(app: &App, deposit_contract: &DepositContract) -> Cw20DepositResponse {
        app.wrap()
            .query_wasm_smart(
                deposit_contract.addr(),
                &QueryMsg::Cw20Deposits {
                    address: USER.to_string(),
                },
            )
            .unwrap()
    }

    fn get_cw20_balance(app: &App, cw20_contract: &Cw20Contract, user: String) -> BalanceResponse {
        app.wrap()
            .query_wasm_smart(
                cw20_contract.addr(),
                &Cw20QueryMsg::Balance { address: user },
            )
            .unwrap()
    }

    #[test]
    fn deposit_native() {
        let (mut app, deposit_id, _cw20_id) = store_code();
        let deposit_contract = deposit_instantiate(&mut app, deposit_id);

        let balance = get_balance(&app, USER.to_string(), "denom".to_string());
        println!("Intial Balance {:?}", balance);

        let msg = ExecuteMsg::Deposit {};

        let cosmos_msg = deposit_contract
            .call(msg, vec![coin(1000, "denom")])
            .unwrap();
        app.execute(Addr::unchecked(USER), cosmos_msg).unwrap();

        let balance = get_balance(
            &app,
            deposit_contract.addr().into_string(),
            deposit_contract.addr().into_string(),
        );
        println!("Deposit Contract {:?}", balance);

        let balance = get_balance(&app, USER.to_string(), "denom".to_string());
        println!("Post {:?}", balance);

        let _deposits = get_deposits(&app, &deposit_contract);
    }

    #[test]
    fn deposit_cw20() {
        let (mut app, deposit_id, cw20_id) = store_code();

        // each time we do deposit_instantiate, contract0 becomes contract1 and so on
        let deposit_contract = deposit_instantiate(&mut app, deposit_id);

        let cw20_contract = cw_20_instantiate(&mut app, cw20_id);

        let initial_balance = get_cw20_balance(&app, &cw20_contract, USER.to_string());

        let hook_msg = Cw20HookMsg::Deposit {};

        let msg = Cw20ExecuteMsg::Send {
            contract: deposit_contract.addr().to_string(),
            amount: Uint128::from(500u64),
            msg: to_binary(&hook_msg).unwrap(),
        };
        let cosmos_msg = cw20_contract.call(msg).unwrap();

        app.execute(Addr::unchecked(USER), cosmos_msg).unwrap();

        //verify the deposit amount on the contract
        let cw20_deposit_response = get_cw20_deposits(&app, &deposit_contract);
        assert_eq!(
            cw20_deposit_response.deposits,
            vec![(
                "contract1".to_string(),
                Cw20Deposits {
                    count: 1,
                    owner: USER.to_string(),
                    contract: "contract1".to_string(),
                    amount: Uint128::from(500u64),
                    lockdown: 12345 + 20, // default block height is 12345
                }
            )]
        );

        //verify the users left over token balance
        let balance = get_cw20_balance(&app, &cw20_contract, USER.to_string());
        assert_eq!(
            balance.balance,
            initial_balance.balance - Uint128::from(500u64)
        );
    }
}
