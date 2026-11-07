//! Menu prices and what a customer is charged for an order
//!
//! Events carry an order's menu subtotal (`total_price`). Every artifact that
//! shows money (the payment authorization, confirmation email, receipt and
//! loyalty points) derives its amounts from that subtotal through
//! [`OrderCharges`], so they agree with each other and with the events.

/// Sales tax applied to every order
pub const TAX_RATE: f64 = 0.085;

/// Tip added to every order
pub const TIP_RATE: f64 = 0.18;

/// Menu price of an item
pub fn menu_price(item: &str) -> f64 {
    match item {
        "burger" => 12.50,
        "fries" => 4.50,
        "salad" => 9.00,
        "pizza" => 15.00,
        _ => 10.00,
    }
}

/// What a customer pays for an order, in dollars rounded to cents
///
/// `total` is the amount authorized when the order is placed and the amount
/// on the receipt.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OrderCharges {
    /// Sum of the menu prices (the events' `total_price`)
    pub subtotal: f64,
    /// Sales tax ([`TAX_RATE`] of the subtotal)
    pub tax: f64,
    /// Tip ([`TIP_RATE`] of the subtotal)
    pub tip: f64,
    /// `subtotal + tax + tip`
    pub total: f64,
}

impl OrderCharges {
    /// The charges for an order whose menu items add up to `subtotal`
    pub fn for_subtotal(subtotal: f64) -> Self {
        let subtotal = cents(subtotal);
        let tax = cents(subtotal * TAX_RATE);
        let tip = cents(subtotal * TIP_RATE);
        Self {
            subtotal,
            tax,
            tip,
            total: cents(subtotal + tax + tip),
        }
    }
}

/// Round to whole cents
fn cents(amount: f64) -> f64 {
    (amount * 100.0).round() / 100.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn charges_add_up_to_the_total() {
        let c = OrderCharges::for_subtotal(12.5);
        assert_eq!(c.subtotal, 12.5);
        assert_eq!(c.tax, 1.06);
        assert_eq!(c.tip, 2.25);
        assert_eq!(c.total, 15.81);
    }

    #[test]
    fn amounts_are_whole_cents() {
        for subtotal in [4.5, 9.0, 12.5, 15.0, 27.0, 33.33] {
            let c = OrderCharges::for_subtotal(subtotal);
            for amount in [c.subtotal, c.tax, c.tip, c.total] {
                assert_eq!(amount, (amount * 100.0).round() / 100.0, "{c:?}");
            }
        }
    }
}
