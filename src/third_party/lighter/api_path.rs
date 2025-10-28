use const_format::concatcp;

// Root
pub const LIGHTER_STREAM_URL: &str = "wss://mainnet.zklighter.elliot.ai/stream";
pub const LIGHTER_API_URL: &str = "https://mainnet.zklighter.elliot.ai";

// Paths
pub const LIGHTER_FUNDING_RATE_API_PATH: &str = "/api/v1/funding-rates";

// Endpoints
pub const LIGHTER_FUNDING_RATE_API: &str =
    concatcp!(LIGHTER_API_URL, LIGHTER_FUNDING_RATE_API_PATH);
