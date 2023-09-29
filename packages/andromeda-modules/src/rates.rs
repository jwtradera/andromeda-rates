use andromeda_std::{
    amp::recipient::Recipient, andr_exec, andr_instantiate, andr_query, error::ContractError,
};
use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{ensure, Coin, Decimal, Fraction, QuerierWrapper};

#[andr_instantiate]
#[cw_serde]
pub struct InstantiateMsg {
    pub rates: Vec<RateInfo>,
}

#[andr_exec]
#[cw_serde]
pub enum ExecuteMsg {
    UpdateRates { rates: Vec<RateInfo> },
    UpdateSaleTimestamp { last_timestamp: u64 },
}

#[cw_serde]
pub struct MigrateMsg {}

#[andr_query]
#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    #[returns(PaymentsResponse)]
    Payments {},
}

#[cw_serde]
pub struct PaymentsResponse {
    pub payments: Vec<RateInfo>,
    pub last_timestamp: u64,
}

#[cw_serde]
pub struct RateInfo {
    pub rate: Rate,
    pub is_additive: bool,
    pub description: Option<String>,
    pub recipients: Vec<Recipient>,

    // Threshold will be applied for fiat rate only for now
    pub threshold: Option<Thredshold>,
}

#[cw_serde]
/// An enum used to define various types of fees
pub enum Rate {
    /// A flat rate fee
    Flat(Coin),
    /// A percentage fee
    Percent(PercentRate),
    // External(PrimitivePointer),
}

#[cw_serde] // This is added such that both Rate::Flat and Rate::Percent have the same level of nesting which
            // makes it easier to work with on the frontend.
pub struct PercentRate {
    pub percent: Decimal,
}

impl From<Decimal> for Rate {
    fn from(decimal: Decimal) -> Self {
        Rate::Percent(PercentRate { percent: decimal })
    }
}

impl Rate {
    /// Validates that a given rate is non-zero. It is expected that the Rate is not an
    /// External Rate.
    pub fn is_non_zero(&self) -> Result<bool, ContractError> {
        match self {
            Rate::Flat(coin) => Ok(!coin.amount.is_zero()),
            Rate::Percent(PercentRate { percent }) => Ok(!percent.is_zero()),
            // Rate::External(_) => Err(ContractError::UnexpectedExternalRate {}),
        }
    }

    /// Validates `self` and returns an "unwrapped" version of itself wherein if it is an External
    /// Rate, the actual rate value is retrieved from the Primitive Contract.
    pub fn validate(&self, querier: &QuerierWrapper) -> Result<Rate, ContractError> {
        let rate = self.clone().get_rate(querier)?;
        ensure!(rate.is_non_zero()?, ContractError::InvalidRate {});

        if let Rate::Percent(PercentRate { percent }) = rate {
            ensure!(percent <= Decimal::one(), ContractError::InvalidRate {});
        }

        Ok(rate)
    }

    /// If `self` is Flat or Percent it returns itself. Otherwise it queries the primitive contract
    /// and retrieves the actual Flat or Percent rate.
    fn get_rate(self, _querier: &QuerierWrapper) -> Result<Rate, ContractError> {
        match self {
            Rate::Flat(_) => Ok(self),
            Rate::Percent(_) => Ok(self),
            // Rate::External(primitive_pointer) => {
            //     let primitive = primitive_pointer.into_value(querier)?;
            //     match primitive {
            //         None => Err(ContractError::ParsingError {
            //             err: "Stored primitive is None".to_string(),
            //         }),
            //         Some(primitive) => match primitive {
            //             Primitive::Coin(coin) => Ok(Rate::Flat(coin)),
            //             Primitive::Decimal(value) => Ok(Rate::from(value)),
            //             _ => Err(ContractError::ParsingError {
            //                 err: "Stored rate is not a coin or Decimal".to_string(),
            //             }),
            //         },
            //     }
            // }
        }
    }
}

#[cw_serde]
/// A struct used to define threshold setting
pub struct Thredshold {
    /// decrease unit like 1 Coin
    pub unit: u64,
    /// decrease duration in seconds like 1 hour
    pub duration: u64,
    // thredshold value
    pub value: u128,
}

/// An attribute struct used for any events that involve a payment
pub struct PaymentAttribute {
    /// The amount paid
    pub amount: Coin,
    /// The address the payment was made to
    pub receiver: String,
}

impl ToString for PaymentAttribute {
    fn to_string(&self) -> String {
        format!("{}<{}", self.receiver, self.amount)
    }
}

/// Calculates a fee amount given a `Rate` and payment amount.
///
/// ## Arguments
/// * `fee_rate` - The `Rate` of the fee to be paid
/// * `payment` - The amount used to calculate the fee
///
/// Returns the fee amount in a `Coin` struct.
pub fn calculate_fee(
    fee_rate: Rate,
    payment: &Coin,
    threshold: Option<Thredshold>,
    current_timestamp: u64,
    last_timestamp: u64,
) -> Result<Coin, ContractError> {
    // Validate timestamp values
    ensure!(
        last_timestamp == 0 || current_timestamp > last_timestamp,
        ContractError::InvalidRate {}
    );

    match fee_rate {
        Rate::Flat(rate) => {
            let amount = rate.amount.u128();

            // If threshold not set or first sale, then return raw value
            if Option::is_none(&threshold) || last_timestamp == 0 {
                Ok(Coin::new(amount, rate.denom))
            } else {
                let _threshold = threshold.unwrap();

                // Calculate final fee based on threshold setting and timestamp
                let time_elapsed = current_timestamp - last_timestamp;
                let decrement_value = time_elapsed
                    .checked_div(_threshold.duration)
                    .unwrap()
                    .checked_mul(_threshold.unit)
                    .unwrap();
                let decremented_amount = amount.checked_sub(decrement_value as u128).unwrap();

                // If calculated value is lower than threshold value
                if decremented_amount < _threshold.value {
                    Ok(Coin::new(_threshold.value, rate.denom))
                } else {
                    Ok(Coin::new(decremented_amount, rate.denom))
                }
            }
        }
        Rate::Percent(PercentRate { percent }) => {
            // [COM-03] Make sure that fee_rate between 0 and 100.
            ensure!(
                // No need for rate >=0 due to type limits (Question: Should add or remove?)
                percent <= Decimal::one() && !percent.is_zero(),
                ContractError::InvalidRate {}
            );
            let mut fee_amount = payment.amount * percent;

            // Always round any remainder up and prioritise the fee receiver.
            // Inverse of percent will always exist.
            let reversed_fee = fee_amount * percent.inv().unwrap();
            if payment.amount > reversed_fee {
                // [COM-1] Added checked add to fee_amount rather than direct increment
                fee_amount = fee_amount.checked_add(1u128.into())?;
            }

            Ok(Coin::new(fee_amount.u128(), payment.denom.clone()))
        } // Rate::External(_) => Err(ContractError::UnexpectedExternalRate {}),
    }
}

#[cfg(test)]
mod tests {

    use cosmwasm_std::{coin, Uint128};

    use super::*;

    // #[test]
    // fn test_validate_external_rate() {
    //     let deps = mock_dependencies_custom(&[]);

    //     let rate = Rate::External(PrimitivePointer {
    //         address: MOCK_PRIMITIVE_CONTRACT.to_owned(),

    //         key: Some("percent".to_string()),
    //     });
    //     let validated_rate = rate.validate(&deps.as_ref().querier).unwrap();
    //     let expected_rate = Rate::from(Decimal::percent(1));
    //     assert_eq!(expected_rate, validated_rate);

    //     let rate = Rate::External(PrimitivePointer {
    //         address: MOCK_PRIMITIVE_CONTRACT.to_owned(),
    //         key: Some("flat".to_string()),
    //     });
    //     let validated_rate = rate.validate(&deps.as_ref().querier).unwrap();
    //     let expected_rate = Rate::Flat(coin(1u128, "uusd"));
    //     assert_eq!(expected_rate, validated_rate);
    // }

    #[test]
    fn test_calculate_fee() {
        let payment = coin(101, "uluna");
        let expected = Ok(coin(5, "uluna"));
        let fee = Rate::from(Decimal::percent(4));

        let received = calculate_fee(fee, &payment, None, 0, 0);

        assert_eq!(expected, received);

        let payment = coin(125, "uluna");
        let fee = Rate::Flat(Coin {
            amount: Uint128::from(5_u128),
            denom: "uluna".to_string(),
        });

        let received = calculate_fee(fee, &payment, None, 0, 0);

        assert_eq!(expected, received);
    }

    #[test]
    fn test_calculate_fee_threshold_fiat() {
        let payment = coin(100, "uluna");
        let fee = Rate::Flat(Coin {
            amount: Uint128::from(10_u128),
            denom: "uluna".to_string(),
        });

        // After 60 seconds, fee will be 8 = (10 - 2 * 1)
        let received = calculate_fee(
            fee.clone(),
            &payment,
            Some(Thredshold {
                unit: 2,
                duration: 60,
                value: 5,
            }),
            61,
            1,
        );

        assert_eq!(Ok(coin(8, "uluna")), received);

        // After 100 seconds, fee will be 8 = (10 - 2 * 1) (Because not till 120s)
        let received = calculate_fee(
            fee.clone(),
            &payment,
            Some(Thredshold {
                unit: 2,
                duration: 60,
                value: 5,
            }),
            101,
            1,
        );

        assert_eq!(Ok(coin(8, "uluna")), received);

        // After 300 seconds, fee will be 5 (limited to thredshold value)
        let received = calculate_fee(
            fee.clone(),
            &payment,
            Some(Thredshold {
                unit: 2,
                duration: 60,
                value: 5,
            }),
            301,
            1,
        );

        assert_eq!(Ok(coin(5, "uluna")), received);
    }
}
