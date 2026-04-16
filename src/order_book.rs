use rust_decimal::Decimal;

use crate::{
    order::{Order, OrderStatus, Side},
    types::{OrderId, Price},
};
use std::collections::{BTreeMap, HashMap, VecDeque};

/// Reasons a cancel request can fail.
#[derive(Debug, PartialEq, Eq)]
pub enum CancelError {
    /// No order with the given ID exists in the book.
    NotFound,
    /// The order exists but has already been cancelled.
    /// Not returned by `OrderBook::cancel_order` as successful cancels remove the
    /// `OrderId` from `order_index`, so a repeat cancel hits `NotFound` first.
    /// It is reachable via `Level::cancel_order`.
    AlreadyCancelled,
}

pub struct Level {
    orders: VecDeque<Order>,
    // usize so comparisons against .len() don't require casting.
    tombstone_count: usize,
}

impl Level {
    /// Ratio of tombstoned orders to total orders at which compaction triggers.
    /// Range: 0.0 (compact on every cancel) to 1.0 (never compact).
    const COMPACTION_THRESHOLD: f64 = 0.5;

    pub fn new() -> Self {
        Self {
            orders: VecDeque::new(),
            tombstone_count: 0,
        }
    }

    fn find_order(&self, order_id: OrderId) -> Result<usize, CancelError> {
        for (i, order) in self.orders.iter().enumerate() {
            if order.id == order_id {
                return Ok(i);
            }
        }
        Err(CancelError::NotFound)
    }

    fn update_order_status(&mut self, order_idx: usize, status: OrderStatus) {
        self.orders
            .get_mut(order_idx)
            .expect("find_order returned an invalid index")
            .status = status;
    }

    fn compact_if_needed(&mut self) {
        if !self.orders.is_empty()
            && (self.tombstone_count as f64) / (self.orders.len() as f64)
                >= Self::COMPACTION_THRESHOLD
        {
            self.orders
                .retain(|order| order.status != OrderStatus::Cancelled);

            self.tombstone_count = 0;
        }
    }

    pub fn cancel_order(&mut self, order_id: OrderId) -> Result<(), CancelError> {
        let order_idx = self.find_order(order_id)?;

        if self.orders[order_idx].status == OrderStatus::Cancelled {
            return Err(CancelError::AlreadyCancelled);
        }

        self.update_order_status(order_idx, OrderStatus::Cancelled);
        self.tombstone_count += 1;

        self.compact_if_needed();

        Ok(())
    }

    pub fn add_order(&mut self, order: Order) {
        // --- Alternative Implementation ---
        // To destructure this, and keep the fields mutable, you need to label each individual one as a ref mut (mutable reference, like &mut),
        // Then you need to dereference self, which at this point is just a pointer to the data. Doing *self delivers the actual data the
        // pointer leads to
        // let Self { ref mut orders, .. } = *self;
        // orders.push_back(order);

        self.orders.push_back(order);
    }

    pub fn get_tombstone_count(&self) -> usize {
        self.tombstone_count
    }

    pub fn is_empty(&self) -> bool {
        self.orders.is_empty()
    }
}

pub struct OrderBook {
    bids: BTreeMap<Price, Level>,
    asks: BTreeMap<Price, Level>,
    order_index: HashMap<OrderId, (Side, Price)>,
}

impl OrderBook {
    pub fn new() -> Self {
        Self {
            bids: BTreeMap::new(),
            asks: BTreeMap::new(),
            order_index: HashMap::new(),
        }
    }

    pub fn add_order(&mut self, order: Order) {
        // Unique IDs are the caller's (OMS/gateway) responsibility; debug-only guard to catch integration bugs.
        debug_assert!(!self.order_index.contains_key(&order.id));

        let Order {
            id, side, price, ..
        } = order;

        let side_book = if side == Side::Ask {
            &mut self.asks
        } else {
            &mut self.bids
        };

        let level = side_book.entry(price).or_insert_with(Level::new);
        level.add_order(order);

        self.order_index.insert(id, (side, price));
    }

    fn remove_level_if_empty(&mut self, side: Side, price: Price) {
        let side_book = if side == Side::Ask {
            &mut self.asks
        } else {
            &mut self.bids
        };

        if side_book.get(&price).is_some_and(|level| level.is_empty()) {
            side_book.remove(&price);
        }
    }

    pub fn cancel_order(&mut self, order_id: OrderId) -> Result<(), CancelError> {
        let (side, price) = *self
            .order_index
            .get(&order_id)
            .ok_or(CancelError::NotFound)?;

        let level_book = if side == Side::Ask {
            &mut self.asks
        } else {
            &mut self.bids
        };

        // expect(): if the index points to a missing level, that's an invariant bug, not a user error.
        let level = level_book
            .get_mut(&price)
            .expect("order_index references a Price-Level that does not exist");

        level.cancel_order(order_id)?;

        self.remove_level_if_empty(side, price);

        self.order_index.remove(&order_id);

        Ok(())
    }

    pub fn best_bid(&self) -> Option<Price> {
        self.bids.last_key_value().map(|(price, _level)| *price)
    }

    pub fn best_ask(&self) -> Option<Price> {
        self.asks.first_key_value().map(|(price, _level)| *price)
    }

    /// Best ask minus best bid. `None` if either side is empty.
    ///
    /// Returns `Decimal` rather than `Price`: a price difference is semantically a
    /// spread, not a unit price. A dedicated `Spread` newtype isn't justified yet
    /// (one call site, no downstream consumers), we can promote `Decimal` → `Spread`
    /// if a second caller of spread-like arithmetic appears.
    pub fn spread(&self) -> Option<Decimal> {
        self.best_bid()
            .zip(self.best_ask())
            .map(|(bid, ask)| ask.0 - bid.0)
    }
}

#[cfg(test)]
mod tests {
    use rust_decimal_macros::dec;

    use super::*;
    use crate::{order_book, types::{ProductId, Quantity, Timestamp}};

    fn create_order(id: OrderId, side: Side, price: Price) -> Order {
        Order {
            id,
            side,
            status: OrderStatus::New,
            price,
            quantity: Quantity(100),
            timestamp: Timestamp(120323020310),
            product_id: ProductId(String::from("GEO")),
        }
    }

    #[test]
    fn best_bid_of_empty_book_is_none() {
        let order_book = OrderBook::new();
        assert_eq!(order_book.best_bid(), None);
    }

    #[test]
    fn level_cancel_unknown_order_returns_not_found() {
        let order = create_order(OrderId(1), Side::Ask, Price(dec!(12.5)));
        let mut level = Level::new();
        assert_eq!(level.cancel_order(order.id), Err(CancelError::NotFound));
    }

    #[test]
    fn cancel_order_removes_it_from_index() {
        let order = create_order(OrderId(1), Side::Ask, Price(dec!(12.5)));
        let id = order.id;
        let mut order_book = OrderBook::new();
        order_book.add_order(order);
        let _ = order_book.cancel_order(id);
        assert_eq!(order_book.cancel_order(id), Err(CancelError::NotFound));
    }

    #[test]
    fn cancel_only_order_at_price_removes_level() {
        let order = create_order(OrderId(1), Side::Ask, Price(dec!(12.5)));
        let id = order.id;
        let mut order_book = OrderBook::new();
        order_book.add_order(order);
        let _ = order_book.cancel_order(id);
        assert_eq!(order_book.best_ask(), None);
    }

    #[test]
    fn level_compaction_triggers_at_threshold() {
        let order1 = create_order(OrderId(1), Side::Ask, Price(dec!(12.5)));
        let id1 = order1.id;
        let order2 = create_order(OrderId(2), Side::Ask, Price(dec!(12.5)));
        let id2 = order2.id;
        let order3 = create_order(OrderId(3), Side::Ask, Price(dec!(12.5)));
        let order4 = create_order(OrderId(4), Side::Ask, Price(dec!(12.5)));

        let mut level = Level::new();

        for order in [order1, order2, order3, order4] {
            level.add_order(order);
        }

        for id in [id1, id2] {
            let _ = level.cancel_order(id);
        }

        assert_eq!(level.get_tombstone_count(), 0);
    }

    #[test]
    fn level_compaction_does_not_trigger_below_threshold() {
        let order1 = create_order(OrderId(1), Side::Ask, Price(dec!(12.5)));
        let id1 = order1.id;
        let order2 = create_order(OrderId(2), Side::Ask, Price(dec!(12.5)));
        let order3 = create_order(OrderId(3), Side::Ask, Price(dec!(12.5)));
        let order4 = create_order(OrderId(4), Side::Ask, Price(dec!(12.5)));

        let mut level = Level::new();

        for order in [order1, order2, order3, order4] {
            level.add_order(order);
        }

        let _ = level.cancel_order(id1);

        assert_eq!(level.get_tombstone_count(), 1);
    }

    #[test]
    fn spread_returns_ask_minus_bid() {
        let order1 = create_order(OrderId(1), Side::Ask, Price(dec!(14.5)));
        let order2 = create_order(OrderId(2), Side::Bid, Price(dec!(12.5)));

        let mut order_book = OrderBook::new();

        order_book.add_order(order1);
        order_book.add_order(order2);

        assert_eq!(order_book.spread(), Some(dec!(2)));
    }

    #[test]
    fn level_cancel_same_order_twice_returns_already_cancelled() {
        let order1 = create_order(OrderId(1), Side::Bid, Price(dec!(10)));
        let id1 = order1.id;
        let order2 = create_order(OrderId(2), Side::Bid, Price(dec!(10)));
        let order3 = create_order(OrderId(3), Side::Bid, Price(dec!(10)));
        let mut level = Level::new();
        level.add_order(order1);
        level.add_order(order2);
        level.add_order(order3);
        let _ = level.cancel_order(id1);
        assert_eq!(level.cancel_order(id1), Err(CancelError::AlreadyCancelled));
    }

    #[test]
    fn best_ask_of_empty_book_is_none() {
        let order_book = OrderBook::new();
        assert_eq!(order_book.best_ask(), None);
    }

    #[test]
    fn spread_of_empty_book_is_none() {
        let order_book = OrderBook::new();
        assert_eq!(order_book.spread(), None);
    }

    #[test]
    fn spread_when_only_bids_populated_is_none() {
        let mut order_book = OrderBook::new();
        order_book.add_order(create_order(OrderId(1), Side::Bid, Price(dec!(100))));
        assert_eq!(order_book.spread(), None);
    }

    #[test]
    fn spread_when_only_asks_populated_is_none() {
        let mut order_book = OrderBook::new();
        order_book.add_order(create_order(OrderId(1), Side::Ask, Price(dec!(100))));
        assert_eq!(order_book.spread(), None);
    }

    #[test]
    fn add_bid_makes_it_appear_as_best_bid() {
        let mut order_book = OrderBook::new();
        order_book.add_order(create_order(OrderId(1), Side::Bid, Price(dec!(50))));
        assert_eq!(order_book.best_bid(), Some(Price(dec!(50))));
    }

    #[test]
    fn add_ask_makes_it_appear_as_best_ask() {
        let mut order_book = OrderBook::new();
        order_book.add_order(create_order(OrderId(1), Side::Ask, Price(dec!(75))));
        assert_eq!(order_book.best_ask(), Some(Price(dec!(75))));
    }

    #[test]
    fn best_bid_returns_highest_of_multiple_bids() {
        let mut order_book = OrderBook::new();
        order_book.add_order(create_order(OrderId(1), Side::Bid, Price(dec!(90))));
        order_book.add_order(create_order(OrderId(2), Side::Bid, Price(dec!(95))));
        order_book.add_order(create_order(OrderId(3), Side::Bid, Price(dec!(85))));
        assert_eq!(order_book.best_bid(), Some(Price(dec!(95))));
    }

    #[test]
    fn best_ask_returns_lowest_of_multiple_asks() {
        let mut order_book = OrderBook::new();
        order_book.add_order(create_order(OrderId(1), Side::Ask, Price(dec!(110))));
        order_book.add_order(create_order(OrderId(2), Side::Ask, Price(dec!(105))));
        order_book.add_order(create_order(OrderId(3), Side::Ask, Price(dec!(115))));
        assert_eq!(order_book.best_ask(), Some(Price(dec!(105))));
    }

    #[test]
    fn cancel_unknown_order_on_orderbook_returns_not_found() {
        let mut order_book = OrderBook::new();
        assert_eq!(order_book.cancel_order(OrderId(999)), Err(CancelError::NotFound));
    }
}
