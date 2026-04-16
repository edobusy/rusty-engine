use crate::types::{OrderId, Price, ProductId, Quantity, Timestamp};

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Side {
    Bid,
    Ask,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OrderStatus {
    New,
    Accepted,
    PartiallyFilled { remaining: Quantity },
    Filled,
    Cancelled,
    Rejected { reason: String },
}

// Data carrier, let's keep fields public, getters/setters would only add noise
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Order {
    pub id: OrderId,
    pub status: OrderStatus,
    pub side: Side,
    pub price: Price,
    pub quantity: Quantity,
    pub timestamp: Timestamp,
    pub product_id: ProductId,
}

impl Order {
    pub fn new(
        id: OrderId,
        side: Side,
        price: Price,
        quantity: Quantity,
        timestamp: Timestamp,
        product_id: ProductId,
    ) -> Self {
        Self {
            id,
            side,
            status: OrderStatus::New,
            price,
            quantity,
            timestamp,
            product_id,
        }
    }

    pub fn describe_status(&self) -> String {
        match self.status {
            OrderStatus::New => String::from("New Order"),
            OrderStatus::Accepted => String::from("Order Accepted"),
            OrderStatus::PartiallyFilled { ref remaining } => {
                format!("Order Partially Filled, remaining: {}", remaining.0)
            }
            OrderStatus::Filled => String::from("Order Filled!"),
            OrderStatus::Cancelled => String::from("Order Cancelled"),
            OrderStatus::Rejected { ref reason } => format!("Order Rejected: {reason}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::*;
    use rust_decimal::Decimal;

    #[test]
    fn test_new_order_status() {
        let order: Order = Order::new(
            OrderId(1),
            Side::Bid,
            Price(Decimal::from(1250)),
            Quantity(8),
            Timestamp(100202010),
            ProductId(String::from("GEO")),
        );
        assert!(OrderStatus::New == order.status);
    }

    #[test]
    fn test_order_equality() {
        let bid_order: Order = Order::new(
            OrderId(1),
            Side::Bid,
            Price(Decimal::from(1250)),
            Quantity(8),
            Timestamp(100202010),
            ProductId(String::from("GEO")),
        );
        let ask_order: Order = Order::new(
            OrderId(2),
            Side::Ask,
            Price(Decimal::from(1250)),
            Quantity(8),
            Timestamp(100202007),
            ProductId(String::from("GEO")),
        );
        assert_ne!(bid_order, ask_order);
    }

    #[test]
    fn test_order_side() {
        let order: Order = Order::new(
            OrderId(1),
            Side::Bid,
            Price(Decimal::from(1250)),
            Quantity(8),
            Timestamp(100202010),
            ProductId(String::from("GEO")),
        );
        assert_eq!(Side::Bid, order.side);
    }

    #[test]
    fn test_describe_status_new() {
        let order: Order = Order::new(
            OrderId(1),
            Side::Bid,
            Price(Decimal::from(1250)),
            Quantity(8),
            Timestamp(100202010),
            ProductId(String::from("GEO")),
        );
        assert_eq!("New Order", order.describe_status());
    }

    #[test]
    fn test_describe_status_accepted() {
        let mut order: Order = Order::new(
            OrderId(1),
            Side::Bid,
            Price(Decimal::from(1250)),
            Quantity(8),
            Timestamp(100202010),
            ProductId(String::from("GEO")),
        );
        order.status = OrderStatus::Accepted;
        assert_eq!("Order Accepted", order.describe_status());
    }

    #[test]
    fn test_describe_status_partially_filled() {
        let mut order: Order = Order::new(
            OrderId(1),
            Side::Bid,
            Price(Decimal::from(1250)),
            Quantity(8),
            Timestamp(100202010),
            ProductId(String::from("GEO")),
        );
        order.status = OrderStatus::PartiallyFilled {
            remaining: Quantity(2),
        };
        assert_eq!(
            "Order Partially Filled, remaining: 2",
            order.describe_status()
        );
    }

    #[test]
    fn test_describe_status_filled() {
        let mut order: Order = Order::new(
            OrderId(1),
            Side::Bid,
            Price(Decimal::from(1250)),
            Quantity(8),
            Timestamp(100202010),
            ProductId(String::from("GEO")),
        );
        order.status = OrderStatus::Filled;
        assert_eq!("Order Filled!", order.describe_status());
    }

    #[test]
    fn test_describe_status_cancelled() {
        let mut order: Order = Order::new(
            OrderId(1),
            Side::Bid,
            Price(Decimal::from(1250)),
            Quantity(8),
            Timestamp(100202010),
            ProductId(String::from("GEO")),
        );
        order.status = OrderStatus::Cancelled;
        assert_eq!("Order Cancelled", order.describe_status());
    }

    #[test]
    fn test_describe_status_rejected() {
        let mut order: Order = Order::new(
            OrderId(1),
            Side::Bid,
            Price(Decimal::from(1250)),
            Quantity(8),
            Timestamp(100202010),
            ProductId(String::from("GEO")),
        );
        order.status = OrderStatus::Rejected {
            reason: String::from("test"),
        };
        assert_eq!("Order Rejected: test", order.describe_status());
    }
}
