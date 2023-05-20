use std::collections::HashMap;

fn main() {}
// A user can submit a `MultiSend` transaction (similar to bank.MultiSend in cosmos sdk) to transfer multiple
// coins (denoms) from multiple input addresses to multiple output addresses. A denom is the name or symbol
// for a coin type, e.g USDT and USDC can be considered different denoms; in cosmos ecosystem they are called
// denoms, in ethereum world they are called symbols.
// The sum of input coins and output coins must match for every transaction.
struct MultiSend {
    // inputs contain the list of accounts that want to send coins from, and how many coins from each account we want to send.
    inputs: Vec<Balance>,
    // outputs contains the list of accounts that we want to deposit coins into, and how many coins to deposit into
    // each account
    outputs: Vec<Balance>,
}

impl MultiSend {
  fn get_inputs_amounts_per_token(&self) -> HashMap<String, i128> { MultiSend::sum_amounts(&self.inputs) }
  fn get_output_amounts_per_token(&self) -> HashMap<String, i128> { MultiSend::sum_amounts(&self.outputs) }
  fn assert_input_output_amounts_should_same(&self) -> Result<(), String> {
    let input_amounts = self.get_inputs_amounts_per_token();
    let output_amounts = self.get_output_amounts_per_token();
    for (denom, value) in input_amounts.iter() {
      let output_value = output_amounts.get(denom);
      if output_value != Some(value) {
        return Err(format!("Input and Output token amount mismatches for token: {}", denom));
      }
    }
    Ok(())
  }
  
  fn get_nonissuer_input_output_amounts(&self, token_map: &HashMap<String, DenomDefinition>) -> (HashMap<String, i128>, HashMap<String, i128>) { 
    let mut input_amounts = MultiSend::sum_amounts_non_issuer(&self.inputs, token_map);
    let mut output_amounts = MultiSend::sum_amounts_non_issuer(&self.outputs, token_map);
    (input_amounts, output_amounts)
  }

  fn get_inputs_amounts_per_address(&self) -> HashMap<String, HashMap<String, i128>> { self.get_amounts_per_address(&self.inputs) }
  fn get_outputs_amounts_per_address(&self) -> HashMap<String, HashMap<String, i128>> { self.get_amounts_per_address(&self.outputs) }
  fn get_amounts_per_address(&self, balances: &Vec<Balance>) -> HashMap<String, HashMap<String, i128>> { 
    let mut amounts_per_address: HashMap<String, HashMap<String, i128>> = HashMap::new();
    for balance in balances {
      for coin in &balance.coins {
        if let Some(coins) = amounts_per_address.get_mut(&balance.address) {
          if let Some(coin_amount) = coins.get_mut(&coin.denom) {
            *coin_amount = *coin_amount + coin.amount;
          } else {
            coins.insert(coin.denom.clone(), coin.amount);
          }
        } else {
          let mut new_coins = HashMap::new();
          new_coins.insert(coin.denom.clone(), coin.amount);
          amounts_per_address.insert(balance.address.clone(), new_coins);
        }
      }
    }
    amounts_per_address
  }
  
  fn assert_balances_should_bigger_than_input(&self, account_map: &HashMap<String, Vec<Coin>>, token_map: &HashMap<String, DenomDefinition>) -> Result<(), String> {
    let amounts_per_account = self.get_inputs_amounts_per_address();
    let (non_issuer_input_sum, non_issuer_output_sum) = self.get_nonissuer_input_output_amounts(token_map);
    
    for (address, account_token_map) in amounts_per_account.iter() {
      let account_coins = account_map.get(address);
      if account_coins.is_none() {
        return Err(format!("Address not found in original balances {}", address));
      }
      for (denom, value) in account_token_map.iter() {
        let token_denom = token_map.get(denom).expect("Invalid Token Found");
        let res = account_coins.unwrap().iter().find(|&coin| coin.denom == *denom 
          && coin.amount >= token_denom.calculated_amount(*value, &non_issuer_input_sum, &non_issuer_output_sum));
        if res.is_none() {
          return Err(format!("Insufficient balance for token: {} in address: {}", denom, address));
        }
      }
    }

    Ok(())
  }

  fn sum_amounts(balances: &Vec<Balance>) -> HashMap<String, i128> {
    let mut amounts_per_coin: HashMap<String, i128> = HashMap::new();
    for balance in balances {
      for coin in &balance.coins {
        if let Some(coin_amount) = amounts_per_coin.get_mut(&coin.denom) {
          *coin_amount = *coin_amount + coin.amount;
        } else {
          amounts_per_coin.insert(coin.denom.clone(), coin.amount);
        }
      }
    }
    amounts_per_coin
  }

  fn sum_amounts_non_issuer(balances: &Vec<Balance>, token_map: &HashMap<String, DenomDefinition>) -> HashMap<String, i128> {
    let mut issuer_amounts_per_coin: HashMap<String, i128> = HashMap::new();
    for (token_denom, def) in token_map.iter() {
      if let Some(issuer_balance) = balances.iter().find(|&b| b.address == def.issuer) {
        if let Some(issuer_coin) = issuer_balance.coins.iter().find(|&c| c.denom == *token_denom) {
          if let Some(coin_amount) = issuer_amounts_per_coin.get_mut(token_denom) {
            *coin_amount = *coin_amount + issuer_coin.amount;
          } else {
            issuer_amounts_per_coin.insert(token_denom.clone(), issuer_coin.amount);
          }
        }
      }
    }
    
    let mut amounts_per_coin: HashMap<String, i128> = HashMap::new();
    for balance in balances {
      for coin in &balance.coins {
        let issuer_amount = issuer_amounts_per_coin.get(&coin.denom).unwrap_or(&0);
        if let Some(coin_amount) = amounts_per_coin.get_mut(&coin.denom) {
          *coin_amount = *coin_amount + coin.amount - issuer_amount;
        } else {
          amounts_per_coin.insert(coin.denom.clone(), coin.amount - issuer_amount);
        }
      }
    }
    amounts_per_coin
  }

}
#[derive(Debug)]
pub struct Coin {
    pub denom: String,
    pub amount: i128,
}

#[derive(Debug)]
struct Balance {
    address: String,
    coins: Vec<Coin>,
}

// A Denom has a definition (`CoinDefinition`) which contains different attributes related to the denom:
struct DenomDefinition {
    // the unique identifier for the token (e.g `core`, `eth`, `usdt`, etc.)
    denom: String,
    // The address that created the token
    issuer: String,
    // burn_rate is a number between 0 and 1. If it is above zero, in every transfer,
    // some additional tokens will be burnt on top of the transferred value, from the senders address.
    // The tokens to be burnt are calculated by multiplying the TransferAmount by burn rate, and
    // rounding it up to an integer value. For example if an account sends 100 token and burn_rate is
    // 0.2, then 120 (100 + 100 * 0.2) will be deducted from sender account and 100 will be deposited to the recipient
    // account (i.e 20 tokens will be burnt)
    burn_rate: f64,
    // commission_rate is exactly same as the burn_rate, but the calculated value will be transferred to the
    // issuer's account address instead of being burnt.
    commission_rate: f64,
}

// Implement `calculate_balance_changes` with the following requirements.
// - Output of the function is the balance changes that must be applied to different accounts
//   (negative means deduction, positive means addition), or an error. the error indicates that the transaction must be rejected.
// - If sum of inputs and outputs in multi_send_tx does not match the tx must be rejected(i.e return error).
// - Apply burn_rate and commission_rate as described by their definition.
// - If the sender does not have enough balances (in the original_balances) to cover the input amount on top of burn_rate and
// commission_rate, the transaction must be rejected.
// - burn_rate and commission_rate does not apply to the issuer. So to calculate the correct values you must do this for every denom:
//      - sum all the inputs coming from accounts that are not an issuer (let's call it non_issuer_input_sum)
//      - sum all the outputs going to accounts that are not an issuer (let's call it non_issuer_output_sum)
//      - total burn amount is total_burn = min(non_issuer_input_sum, non_issuer_output_sum)
//      - total_burn is distributed between all input accounts as: account_share = roundup(total_burn * input_from_account / non_issuer_input_sum)
//      - total_burn_amount = sum (account_shares) // notice that in previous step we rounded up, so we need to recalculate the total again.
//      - commission_rate is exactly the same, but we send the calculate value to issuer, and not burn.
//      - Example:
//          burn_rate: 10%
//
//          inputs:
//          60, 90
//          25 <-- issuer
//
//          outputs:
//          50
//          100 <-- issuer
//          25
//          In this case burn amount is: min(non_issuer_inputs, non_issuer_outputs) = min(75+75, 50+25) = 75
//          Expected burn: 75 * 10% = 7.5
//          And now we divide it proportionally between all input sender: first_sender_share  = 7.5 * 60 / 150  = 3
//                                                                        second_sender_share = 7.5 * 90 / 150  = 4.5
// - In README.md we have provided more examples to help you better understand the requirements.
// - Write different unit tests to cover all the edge cases, we would like to see how you structure your tests.
//   There are examples in README.md, you can convert them into tests, but you should add more cases.

impl DenomDefinition {
  fn calculated_amount(&self, amount: i128, non_issuer_input_sum: &HashMap<String, i128>, non_issuer_output_sum: &HashMap<String, i128>) -> i128 {
    amount + self.burn_amount(amount) + self.commission_amount(amount)
  }

  fn burn_amount(&self, amount: i128) -> i128 {
      (self.burn_rate * (amount as f64)).ceil() as i128
  }

  fn commission_amount(&self, amount: i128) -> i128 {
      (self.commission_rate * (amount as f64)).ceil() as i128
  }
  
}

fn calculate_balance_changes(
    original_balances: Vec<Balance>,
    definitions: Vec<DenomDefinition>,
    multi_send_tx: MultiSend,
) -> Result<Vec<Balance>, String> {
    let token_map: HashMap<String, DenomDefinition> = definitions.into_iter().map(|def| (def.denom.clone(), def)).collect();
    let account_map: HashMap<String, Vec<Coin>> = original_balances.into_iter().map(|balance| (balance.address.clone(), balance.coins)).collect();

    // check the input amounts and output amounts
    multi_send_tx.assert_input_output_amounts_should_same()?;
    multi_send_tx.assert_balances_should_bigger_than_input(&account_map, &token_map)?;

    let (non_issuer_input_sum, non_issuer_output_sum) = multi_send_tx.get_nonissuer_input_output_amounts(&token_map);
    
    let input_amounts_per_account = multi_send_tx.get_inputs_amounts_per_address();
    let mut result_balance_map: HashMap<String, HashMap<String, i128>> = HashMap::new();

    // calculate balances by subtracting input amounts
    for (address, account_token_map) in input_amounts_per_account.iter() {

      if let None = result_balance_map.get(address) {
        result_balance_map.insert(address.clone(), HashMap::new());
      }

      for (denom, value) in account_token_map.iter() {
        let token_denom: &DenomDefinition = token_map.get(denom).expect("Invalid Token Found");
        let caculated_value = token_denom.calculated_amount(*value, &non_issuer_input_sum, &non_issuer_output_sum);
        
        let result_account_token_map = result_balance_map.get_mut(address).unwrap();
        if let Some(token_value) = result_account_token_map.get_mut(denom) {
          *token_value = *token_value - caculated_value;
        } else {
          result_account_token_map.insert(denom.clone(), -caculated_value);
        }

        // add commission_rate to issuer
        let commission_value = token_denom.commission_amount(*value);
        
        if let Some(issuer_account_token_map) = result_balance_map.get_mut(&token_denom.issuer) {
          if let Some(token_value) = issuer_account_token_map.get_mut(denom) {
            *token_value = *token_value + commission_value;
          } else {
            issuer_account_token_map.insert(denom.clone(), commission_value);
          }
        } else {
          let mut new_coins = HashMap::new();
          new_coins.insert(denom.clone(), commission_value);
          result_balance_map.insert(token_denom.issuer.clone(), new_coins);
        }
        
      }
    }
    
    println!("first result_balance_map {:?}", result_balance_map);

    // calculate balances by adding output amounts
    let output_amounts_per_account = multi_send_tx.get_outputs_amounts_per_address();
    for (address, account_token_map) in output_amounts_per_account.iter() {

      if let None = result_balance_map.get(address) {
        result_balance_map.insert(address.clone(), HashMap::new());
      }

      for (denom, value) in account_token_map.iter() {
        let token_denom: &DenomDefinition = token_map.get(denom).expect("Invalid Token Found");
        //let value = token_denom.calculated_amount(*value);
        
        let _result_account_token_map = result_balance_map.get_mut(address).unwrap();
        if let Some(token_value) = _result_account_token_map.get_mut(denom) {
          *token_value = *token_value + value;
        } else {
          _result_account_token_map.insert(denom.clone(), *value);
        }
      }
    }

    println!("second result_balance_map {:?}", result_balance_map);

    // convert result_balance_map to Balance vector
    let result_balances: Vec<Balance> = result_balance_map.iter().map(|(address, account_token_map)| 
      Balance {
        address: address.clone(),
        coins: account_token_map.iter().map(|(denom, amount)| Coin { denom: denom.clone(), amount: *amount })
          .collect::<Vec<Coin>>()
          .into_iter()
          .filter(| coin | coin.amount != 0)
          .collect()
      }
    ).collect();
    
    Ok(result_balances.into_iter().filter(|balance| balance.coins.len() > 0).collect())
}


#[cfg(test)]
mod tests {
  use super::*;

  fn check_results(results: &Vec<Balance>, expected_results: &Vec<Balance>) -> bool {
    for balance in results.iter() {
      if let Some(expected_balance) = expected_results.iter().find(|&bal| bal.address == balance.address && bal.coins.len() == balance.coins.len()) {
        for coin in &balance.coins {
          if let None = expected_balance.coins.iter().find(|&c| c.amount == coin.amount) {
            return false;
          }
        }
      } else {
        return false;
      }
    }
    results.len() == expected_results.len()
  }

  #[test]
  fn check_test_case_1() {
    let definitions: Vec<DenomDefinition> = vec![
      DenomDefinition {
          denom: "denom1".to_string(),
          issuer: "issuer_account_A".to_string(),
          burn_rate: 0.08,
          commission_rate: 0.12,
      },
      DenomDefinition {
          denom: "denom2".to_string(),
          issuer: "issuer_account_B".to_string(),
          burn_rate: 1.0,
          commission_rate: 0.0,
      }
    ];

    let orig_balances: Vec<Balance> = vec![
      Balance {
        address: "account1".to_string(),
        coins: vec![ Coin { denom: "denom1".to_string(), amount: 1000_000} ]
      },
      
      Balance {
        address: "account2".to_string(),
        coins: vec![ Coin { denom: "denom2".to_string(), amount: 1000_000} ]
      }
    ];

    let multi_send = MultiSend {
      inputs: vec![
        Balance {
          address: "account1".to_string(),
          coins: vec![
            Coin { denom: "denom1".to_string(), amount: 1000}
          ]
        },
        Balance {
          address: "account2".to_string(),
          coins: vec![
            Coin { denom: "denom2".to_string(), amount: 1000}
          ]
        }
      ],
      outputs: vec![
        Balance{
          address: "account_recipient".to_string(),
          coins: vec![
            Coin { denom: "denom1".to_string(), amount: 1000},
            Coin { denom: "denom2".to_string(), amount: 1000}
          ]
        }
      ]
    };

    let expected_result = vec![
      Balance {
          address: "account_recipient".to_string(),
          coins: vec![
            Coin { denom: "denom1".to_string(), amount: 1000},
            Coin { denom: "denom2".to_string(), amount: 1000}
          ]
      },
      Balance {
          address: "issuer_account_A".to_string(),
          coins: vec![
            Coin { denom: "denom1".to_string(), amount: 120},
          ]
      },
      Balance {
          address: "account1".to_string(),
          coins: vec![
            Coin { denom: "denom1".to_string(), amount: -1200},
          ]
      },
      Balance {
          address: "account2".to_string(),
          coins: vec![
            Coin { denom: "denom2".to_string(), amount: -2000},
          ]
      }
    ];
    
    let result = calculate_balance_changes(
      orig_balances,
      definitions,
      multi_send
    );
    
    assert!(result.is_ok() && check_results(&result.unwrap(), &expected_result) == true, "Result Mismatch");

  }

  
  #[test]
  fn check_test_case_2() {
    let definitions: Vec<DenomDefinition> = vec![
      DenomDefinition {
          denom: "denom1".to_string(),
          issuer: "issuer_account_A".to_string(),
          burn_rate: 0.08,
          commission_rate: 0.12,
      }
    ];

    let orig_balances: Vec<Balance> = vec![
      Balance {
        address: "account1".to_string(),
        coins: vec![ Coin { denom: "denom1".to_string(), amount: 1000_000} ]
      },
      
      Balance {
        address: "account2".to_string(),
        coins: vec![ Coin { denom: "denom1".to_string(), amount: 1000_000} ]
      }
    ];

    let multi_send = MultiSend {
      inputs: vec![
        Balance {
          address: "account1".to_string(),
          coins: vec![
            Coin { denom: "denom1".to_string(), amount: 650}
          ]
        },
        Balance {
          address: "account2".to_string(),
          coins: vec![
            Coin { denom: "denom1".to_string(), amount: 350}
          ]
        }
      ],
      outputs: vec![
        Balance {
          address: "account_recipient".to_string(),
          coins: vec![
            Coin { denom: "denom1".to_string(), amount: 500},
          ]
        },
        Balance {
          address: "issuer_account_A".to_string(),
          coins: vec![
            Coin { denom: "denom1".to_string(), amount: 500},
          ]
        }
      ]
    };

    let expected_result = vec![
      Balance {
          address: "account_recipient".to_string(),
          coins: vec![
            Coin { denom: "denom1".to_string(), amount: 500},
          ]
      },
      Balance {
          address: "issuer_account_A".to_string(),
          coins: vec![
            Coin { denom: "denom1".to_string(), amount: 560},
          ]
      },
      Balance {
          address: "account1".to_string(),
          coins: vec![
            Coin { denom: "denom1".to_string(), amount: -715},
          ]
      },
      Balance {
          address: "account2".to_string(),
          coins: vec![
            Coin { denom: "denom1".to_string(), amount: -385},
          ]
      }
    ];
    
    let result = calculate_balance_changes(
      orig_balances,
      definitions,
      multi_send
    );
    
    println!("{:?}", result);
    assert!(result.is_ok() && check_results(&result.unwrap(), &expected_result) == true, "Result Mismatch");

  }
}