# ic-solidity-bindgen

This is a tool to generate Rust bindings for Solidity contracts to use with the Internet Computer.

## Usage

```rust
contract_abis!("../abi");

thread_local! {
    static ORACLE_ADDRESSES: RefCell<BTreeMap<SupportedNetwork,Address>> = RefCell::new(BTreeMap::new());
}

#[update]
#[candid_method(update)]
async fn set_value(symbol: String, value: WrappedU256) {
    struct Dist {
        nw: SupportedNetwork,
        addr: Address,
    }

    for d in ORACLE_ADDRESSES.with(|addresses| {
        addresses
            .borrow()
            .iter()
            .map(|(&k, &v)| Dist { nw: k, addr: v })
            .collect::<Vec<Dist>>()
    }) {
        let context = ctx(d.nw).unwrap();
        let oracle = IPriceOracle::new(d.addr.clone(), &context); // This is the generated binding
        let res = oracle
            .set_price(
                symbol.to_string().clone(),
                value.value(),
                Some(call_options()),
            )
            .await.unwrap();
        ic_cdk::println!("set_value: {:?}", res);
    }
}
fn call_options() -> Options {
    let call_options = CallOptionsBuilder::default()
        .transform(Some(TransformContext {
            function: TransformFunc(candid::Func {
                principal: ic_cdk::api::id(),
                method: "transform_request".to_string(),
            }),
            context: vec![],
        }))
        .max_resp(None)
        .cycles(None)
        .build()
        .unwrap();
    let mut opts = Options::default();
    opts.call_options = Some(call_options);
    opts
}
```