#[derive(Clone, Debug)]
pub struct CoinData {
    pub coin: String,
    pub funding: f64,
    pub open_interest: f64,
    pub oracle_price: f64,
}

impl CoinData {
    pub fn new(coin: String) -> Self {
        Self {
            coin,
            funding: 0.0,
            open_interest: 0.0,
            oracle_price: 0.0,
        }
    }

    pub fn update(&mut self, funding: f64, open_interest: f64, oracle_price: f64) {
        self.funding = funding;
        self.open_interest = open_interest;
        self.oracle_price = oracle_price;
    }

    pub fn has_data(&self) -> bool {
        self.open_interest != 0.0
    }
}
