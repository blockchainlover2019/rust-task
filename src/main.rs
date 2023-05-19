use std::collections::HashMap;

fn main() {
    println!("Hello, Coreum!");
}

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

  fn get_inputs_amounts_per_address(&self) -> HashMap<String, HashMap<String, i128>> { 
    let mut amounts_per_address: HashMap<String, HashMap<String, i128>> = HashMap::new();
    for balance in &self.inputs {
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
    
    for (address, account_token_map) in amounts_per_account.iter() {
      let account_coins = account_map.get(address);
      if account_coins.is_none() {
        return Err(format!("Address not found in original balances {}", address));
      }
      for (denom, value) in account_token_map.iter() {
        let token_denom = token_map.get(denom).expect("Invalid Token Found");
        let res = account_coins.unwrap().iter().find(|&coin| coin.denom == *denom && coin.amount >= token_denom.calculated_amount(*value));
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
}
pub struct Coin {
    pub denom: String,
    pub amount: i128,
}

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
  fn calculated_amount(&self, amount: i128) -> i128 {
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

    Err("Error".to_string())
}


#[cfg(test)]
mod tests {
  use super::*;
}